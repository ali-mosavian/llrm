//! Available loads and dead stores, as LLVM's GVN load forwarding and DSE find
//! them through MemoryDependence.
//!
//! # How it works
//! Two independent queries share the memory model (`MemRef` cells and
//! [`crate::memoryssa`]'s `Accesses`).
//!
//! Forwarding. `forwardable_walk` takes a set of loads and, for each plain
//! (non-volatile) load, asks MemorySSA for the nearest writes that may clobber
//! its bytes and tries three sources in order:
//!
//! 1. One clobber that is a plain store to the same bytes, defined before the
//!    load, whose value has the load's type: the load is that value.
//! 2. Two or more clobbers (at most `MAX_NEAREST_WRITES`) that are all exact
//!    stores of one value defined where the load can read it, such as the arms
//!    of a join that each stored the same thing.
//! 3. A dominating load of the same bytes, nearest first, with nothing between
//!    the two that may change them (`MemorySSA::unchanged`). A candidate whose
//!    walk fails also fails every load that dominates it.
//!
//! A value serves a read only of its own type. Each answer is a `Forward { at,
//! value }`.
//!
//! ```text
//!    store i32 %v, p          %a = load p     <- 1: %a is %v
//!    ...                      ...
//!    %a = load p              %b = load p     <- 3: %b is %a
//!    ...
//!    br c, T, F      T: store %v, p   F: store %v, p
//!    J: %b = load p                                 <- 2: %b is %v
//! ```
//!
//! Dead stores. `dead_stores` is a backward must-overwrite analysis over the
//! CFG. Every cell a function stores to is numbered once (`Stored`), as LLVM's
//! DSE numbers its memory locations, and a block's state is a set of numbered
//! cells overwritten before any read. Walking a block backward:
//!
//! - a plain store whose cell is covered by one overwritten cell, or whose
//!   bytes the overwritten cells together write (`pieced`), is dead; the cell
//!   then joins the set;
//! - an instruction that reads or writes anything else removes the cells it may
//!   touch (`clobber`, using `may_clobber` and `EscapedBefore` where given);
//! - a fill (what a call writes before it reads) adds its cells.
//!
//! At a join the state is the intersection over the successors, computed per
//! cell with "some cell naming the same bytes" (`alike`) so a spelling
//! difference does not lose it. The fixed point starts from every stored cell
//! and shrinks, with a worklist in postorder, and only the last verdicts stand.
//! A block with no successor starts from nothing, since the caller may read the
//! cell; with `private` (a predicate for cells nothing outside the function can
//! read) it starts from the private cells; a block ending in `unreachable`
//! starts from every cell. A call whose accesses are not known keeps only the
//! private cells in the set (nothing but a named access can read those); any
//! other unknown instruction empties it.
//!
//! Cells that share bytes share an overlap bucket (`regions`), so a write asks
//! its bucket and not all cells; a cell is related to another (covers, names
//! the same bytes, is placed against) only where `placed` can place them, which
//! takes one displaced frame or one base pointer and bytes that meet, so it is
//! compared with those (`Members::meeting`) and not with its object's cells.
//!
//! # Example
//! Forwarding. Both arms of a join store `%v` to the cell, so the load at the
//! join is `%v` (with `%w` on one arm instead, it is not forwarded):
//!
//! ```mir
//! define i16 @f(i1 %c, i16 %v, i16 %w) {
//! b0:
//!   br i1 %c, label %b1, label %b2
//!
//! b1:
//!   store i16 %v, ptr getelementptr (i8, ptr @g, i16 32)
//!   br label %b3
//!
//! b2:
//!   store i16 %v, ptr getelementptr (i8, ptr @g, i16 32)
//!   br label %b3
//!
//! b3:
//!   %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
//!   ret i16 %x
//! }
//! ```
//!
//! `forwardable_walk` returns `Forward { at: the load in b3, value: %v }`.
//!
//! From unit test `test_a_join_keeps_what_every_predecessor_agrees_on` in
//! `avail_tests.rs`.
//!
//! Dead store. The first store is overwritten before any read, so `dead_stores`
//! returns it:
//!
//! ```mir
//! define void @f(ptr %p) {
//! b0:
//!   store i16 1, ptr getelementptr (i8, ptr @g, i16 32)
//!   store i16 2, ptr getelementptr (i8, ptr @g, i16 32)
//!   ret void
//! }
//! ```
//!
//! From unit test `test_a_store_is_dead_only_when_overwritten_before_any_read`
//! in `avail_tests.rs`.
//!
//! # Provides
//! `forwardable_walk`, `dead_stores`, `dead_stores_escaping`, and the
//! accessors `loaded_into`, `stored_from`, `stored_cell`. Consumers:
//! `transform` (load forwarding), `loadjoins` and `dse` in `llrm-transforms`.
//!
//! # Needs
//! [`crate::memoryssa`], [`crate::alias`] (`EscapedBefore`), [`crate::ranges`]
//! (register constants), [`crate::cellmap`], [`crate::regions`] and
//! [`crate::cfg`].
//!
//! # See also
//! LLVM `MemoryDependenceAnalysis`, GVN `processLoad` and
//! `DeadStoreElimination`.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::dense::IdSet;
use llrm_mir::module::{InstId, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::bits::Bits;
use llrm_support::hash::{HashMap, IndexMap};

use crate::alias::EscapedBefore;
use crate::cellmap::CellMap;
use crate::cfg;
use crate::memory::{MemRef, Unit};
use crate::memoryssa::{self, Accesses, covered, covers, may_clobber, placed, same_bytes};
use crate::ranges;
use crate::regions::{OverlapBucket, OverlapBuckets, overlap_bucket, overlap_buckets, overlap_span};

/// The cell `inst` purely loads, and the value it lands in.
pub fn loaded_into(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, ValueId)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Load { volatile: false, .. } => Some((accesses.references.get(&inst)?.clone(), instruction.result?)),
        _ => None,
    }
}

/// The cell `inst` purely stores, and what it wrote there.
pub fn stored_from(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<(MemRef, Operand)> {
    let instruction = unit.function.instruction(inst);
    match instruction.opcode {
        Opcode::Store { volatile: false, .. } => {
            Some((accesses.references.get(&inst)?.clone(), instruction.operands[0]))
        }
        _ => None,
    }
}

/// The cell `inst` purely stores to, whatever it put there: a store's, or
/// the bytes a `memset` fills.
pub fn stored_cell(
    unit: &Unit,
    accesses: &Accesses,
    inst: InstId,
) -> Option<MemRef> {
    stored_from(unit, accesses, inst).map(|(cell, _)| cell).or_else(|| {
        MemRef::filled(unit, inst).filter(|one| !one.volatile).and_then(|_| accesses.references.get(&inst).cloned())
    })
}

/// What a set of blocks may write: each write once by the bytes it names, and
/// whether any instruction may write anything.
#[derive(Default)]
struct Written {
    unknown: bool,
    by_frame: HashMap<crate::regions::Frame, Vec<MemRef>>,
    spans: std::collections::HashSet<(crate::regions::Frame, i128, i128)>,
    wild: Vec<MemRef>,
}

impl Written {
    fn add(
        &mut self,
        unit: &Unit,
        accesses: &Accesses,
        block: llrm_mir::module::BlockId,
    ) {
        for &inst in unit.function.block(block).instructions() {
            match accesses.writes(inst) {
                Some(writes) => {
                    for write in writes {
                        match crate::regions::displaced_span(write) {
                            Some((frame, low, high)) => {
                                if self.spans.insert((frame.clone(), low, high)) {
                                    self.by_frame.entry(frame).or_default().push(write.clone());
                                }
                            }
                            None => self.wild.push(write.clone()),
                        }
                    }
                }
                None => self.unknown = true,
            }
        }
    }
}

/// What one walk down the dominator tree knows of memory: the cells a store or
/// a load left a value in, the writes that may have changed one taken out.
struct Scope<'a> {
    unit: &'a Unit<'a>,
    entries: Vec<Held>,
    /// By the frame a cell's displacement counts from; the cells with none
    /// are `wild`.
    buckets: HashMap<crate::regions::Frame, Vec<usize>>,
    wild: Vec<usize>,
    /// What the paths into a join may write, checked against an entry only if
    /// a load asks for it.
    epochs: Vec<Written>,
    log: Vec<Undo>,
}

struct Held {
    cell: MemRef,
    value: Operand,
    ty: llrm_mir::types::TypeId,
    frame: Option<crate::regions::Frame>,
    alive: bool,
    /// How many `epochs` there were when it was added: the later ones are
    /// writes it has not been checked against.
    epoch: usize,
}

enum Undo {
    Added(usize),
    Killed(usize),
    Epoch,
}

impl Scope<'_> {
    fn frame(cell: &MemRef) -> Option<crate::regions::Frame> {
        crate::regions::displaced_span(cell).map(|(frame, _, _)| frame)
    }

    fn add(
        &mut self,
        cell: MemRef,
        value: Operand,
        ty: llrm_mir::types::TypeId,
    ) {
        let frame = Self::frame(&cell);
        let at = self.entries.len();
        match &frame {
            Some(frame) => self.buckets.entry(frame.clone()).or_default().push(at),
            None => self.wild.push(at),
        }
        let epoch = self.epochs.len();
        self.entries.push(Held { cell, value, ty, frame, alive: true, epoch });
        self.log.push(Undo::Added(at));
    }

    fn kill(
        &mut self,
        at: usize,
    ) {
        if self.entries[at].alive {
            self.entries[at].alive = false;
            self.log.push(Undo::Killed(at));
        }
    }

    fn kill_all(&mut self) {
        for at in 0..self.entries.len() {
            self.kill(at);
        }
    }

    /// Writes that may have happened since the entries held were made.
    fn push_epoch(
        &mut self,
        written: Written,
    ) {
        if written.unknown {
            self.kill_all();
            return;
        }
        self.epochs.push(written);
        self.log.push(Undo::Epoch);
    }

    /// Whether `held` may have been changed by a write of an epoch after it.
    fn changed_since(
        &self,
        held: &Held,
    ) -> bool {
        self.epochs[held.epoch..].iter().any(|written| {
            let writes: Box<dyn Iterator<Item = &MemRef>> = match &held.frame {
                Some(frame) => Box::new(written.by_frame.get(frame).into_iter().flatten().chain(written.wild.iter())),
                None => Box::new(written.by_frame.values().flatten().chain(written.wild.iter())),
            };
            let mut writes = writes;
            writes.any(|write| may_clobber(self.unit, None, &held.cell, write))
        })
    }

    /// Takes out what writing `write` may change.
    fn clobber(
        &mut self,
        write: &MemRef,
    ) {
        let reached: Vec<usize> = match Self::frame(write) {
            Some(frame) => self.buckets.get(&frame).into_iter().flatten().chain(self.wild.iter()).copied().collect(),
            None => (0..self.entries.len()).collect(),
        };
        for at in reached {
            let held = &self.entries[at];
            if held.alive && !held.cell.unwritable() && may_clobber(self.unit, None, &held.cell, write) {
                self.kill(at);
            }
        }
    }

    /// The value a load of `cell` of type `ty` reads, if one is held.
    fn lookup(
        &mut self,
        cell: &MemRef,
        ty: llrm_mir::types::TypeId,
    ) -> Option<Operand> {
        let candidates: Vec<usize> = match Self::frame(cell) {
            Some(frame) => self.buckets.get(&frame).into_iter().flatten().copied().collect(),
            None => self.wild.clone(),
        };
        let at = candidates
            .into_iter()
            .rev()
            .find(
                |&at| {
                    let held = &self.entries[at];
                    held.alive && held.ty == ty && same_bytes(self.unit, &held.cell, cell)
                },
            )?;
        let held = &self.entries[at];
        if !held.cell.unwritable() && self.changed_since(held) {
            self.kill(at);
            return None;
        }
        Some(self.entries[at].value)
    }

    fn undo(
        &mut self,
        mark: usize,
    ) {
        while self.log.len() > mark {
            match self.log.pop().expect("a logged change") {
                Undo::Killed(at) => self.entries[at].alive = true,
                Undo::Epoch => {
                    self.epochs.pop();
                }
                Undo::Added(at) => {
                    let held = self.entries.pop().expect("the last added");
                    debug_assert_eq!(self.entries.len(), at);
                    match held.frame {
                        Some(frame) => {
                            self.buckets.get_mut(&frame).expect("its bucket").pop();
                        }
                        None => {
                            self.wild.pop();
                        }
                    }
                }
            }
        }
    }
}

/// Loads in `want` that a known value can serve, in one walk down the
/// dominator tree and no walk of MemorySSA for any load: LLVM's EarlyCSE.
/// What a block holds of memory is carried to a block that only it reaches and
/// it dominates; a block with other ways in starts with nothing. A load of
/// cells a store or an earlier load left a value in is that value, until a
/// write that may change those bytes. The sources `forwardable_walk` has
/// that this has not are the join of equal stores and a load across a join.
pub fn forwardable_scoped(
    unit: &Unit,
    accesses: &Accesses,
    want: &IdSet<InstId>,
) -> Vec<Forward> {
    WALKED.with(|walked| walked.set(walked.get() + 1));
    let function = unit.function;
    let shape = unit.shape();
    let tree = shape.dominance.tree();
    let mut children: HashMap<llrm_mir::module::BlockId, Vec<llrm_mir::module::BlockId>> = HashMap::default();
    let mut roots = Vec::new();
    for &block in function.layout() {
        match tree.immediate_dominator(block).filter(|_| tree.is_reachable(block)) {
            Some(parent) => children.entry(parent).or_default().push(block),
            None => roots.push(block),
        }
    }
    let mut scope = Scope {
        unit,
        entries: Vec::new(),
        buckets: HashMap::default(),
        wild: Vec::new(),
        epochs: Vec::new(),
        log: Vec::new(),
    };
    let mut found = Vec::new();
    // Blocks to enter, each with the block it is entered from.
    let mut stack: Vec<(llrm_mir::module::BlockId, Option<llrm_mir::module::BlockId>, Option<usize>)> =
        roots.into_iter().rev().map(|block| (block, None, None)).collect();
    while let Some((block, from, mark)) = stack.pop() {
        if let Some(mark) = mark {
            // The block's subtree is done.
            scope.undo(mark);
            continue;
        }
        let mark = scope.log.len();
        stack.push((block, None, Some(mark)));
        match from {
            // The way in is the dominator's alone: its state holds.
            Some(parent) if function.predecessors(block) == [parent] => {}
            // Other ways in: what the dominator held, less what any path from
            // it to here may write.
            Some(parent) => {
                let mut seen: std::collections::BTreeSet<llrm_mir::module::BlockId> = Default::default();
                let mut work: Vec<llrm_mir::module::BlockId> = function.predecessors(block).to_vec();
                while let Some(at) = work.pop() {
                    if at == parent || !seen.insert(at) {
                        continue;
                    }
                    work.extend(function.predecessors(at).iter().copied());
                }
                let mut written = Written::default();
                for &between in &seen {
                    written.add(unit, accesses, between);
                }
                scope.push_epoch(written);
            }
            None => scope.kill_all(),
        }
        for &inst in function.block(block).instructions() {
            if let Some((cell, result)) = loaded_into(unit, accesses, inst) {
                let ty = function.instruction(inst).ty;
                if !want.contains(&inst) {
                    continue;
                }
                match scope.lookup(&cell, ty) {
                    Some(value) => found.push(Forward { at: inst, value }),
                    None => scope.add(cell, Operand::Value(result), ty),
                }
            } else if let Some((cell, value)) = stored_from(unit, accesses, inst) {
                scope.clobber(&cell);
                if let Some(ty) = unit.operand_type(value) {
                    scope.add(cell, value, ty);
                }
            } else {
                match accesses.writes(inst) {
                    Some([]) => {}
                    Some(writes) => {
                        for write in writes {
                            scope.clobber(write);
                        }
                    }
                    None => scope.kill_all(),
                }
            }
        }
        if let Some(kids) = children.get(&block) {
            stack.extend(kids.iter().rev().map(|&kid| (kid, Some(block), None)));
        }
    }
    found
}

/// Loads in `want` that a known value can serve, by the MemorySSA walk.
pub fn forwardable_walk(
    unit: &Unit,
    accesses: &Accesses,
    want: &IdSet<InstId>,
) -> Vec<Forward> {
    WALKED.with(|walked| walked.set(walked.get() + 1));
    let missing: Vec<InstId> = unit
        .function
        .layout()
        .iter()
        .flat_map(|&block| unit.function.block(block).instructions().to_vec())
        .filter(|inst| want.contains(inst) && loaded_into(unit, accesses, *inst).is_some())
        .collect();
    if missing.is_empty() { Vec::new() } else { memory_providers(unit, accesses, &missing) }
}

thread_local! {
    static WALKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has walked for every load, for a test that a
/// function numbered twice walks once.
pub fn walked() -> usize {
    WALKED.with(std::cell::Cell::get)
}

/// A load whose bytes equal a known value.
#[derive(Clone, Debug, PartialEq)]
pub struct Forward {
    pub at: InstId,
    // Which register holds it is the allocator's answer.
    pub value: Operand,
}

/// Every cell a function stores to, numbered once for its dead-store solve.
///
/// A block's state is a set of these, not a map rebuilt per block: what is
/// overwritten only ever names a cell some store here writes, as LLVM's
/// DSE numbers its MemoryLocations. `index` buckets them as `overlapping`
/// rules writes out; a write asks only the live cells of the buckets it
/// reaches.
struct Stored {
    cells: Vec<MemRef>,
    number: IndexMap<MemRef, usize>,
    index: CellMap<usize, (), OverlapBucket>,
    /// The cells that can share bytes with a cell, by what `placed` needs to
    /// place two of them: one displaced frame, or one base pointer.
    groups: HashMap<Group, Members>,
    /// A cell's groups and its bytes in each.
    group_of: Vec<GroupKeys>,
    private: Bits,
    /// The cells that do not certainly name their own bytes.
    selfless: Bits,
    /// Per cell, lazily: the cells whose store writes all its bytes, the
    /// cells naming its bytes, and those placed to write some of them.
    covering: RefCell<Vec<Option<Rc<Bits>>>>,
    alike: RefCell<Vec<Option<Rc<Bits>>>>,
    /// `alike`, as the cells it holds, for a test that asks whether one is in a
    /// set.
    alike_cells: RefCell<Vec<Option<Rc<[usize]>>>>,
    pieces: RefCell<Vec<Option<Rc<Bits>>>>,
    /// The three relations above by what `placed` reads of a cell, which the
    /// cells of one address share: one set for all of them.
    twins: RefCell<HashMap<(usize, GroupKeys), Rc<Bits>>>,
}

/// A cell's groups and its bytes in each.
type GroupKeys = [Option<(Group, i128, i128)>; 2];

/// What two cells share when `placed` can place one against the other: both at
/// displacements of one frame, or both at constant offsets from one pointer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Group {
    Frame(crate::regions::Frame),
    Base(Operand),
}

/// The cells of one group by where their bytes begin, and the widest of them.
#[derive(Default)]
struct Members {
    /// (first byte, last byte plus one, cell), in order of the first.
    by_first: Vec<(i128, i128, usize)>,
    widest: i128,
}

impl Members {
    /// The cells whose bytes meet `low..high` (and a cell with none, which is
    /// never ruled out by where it is).
    fn meeting(
        &self,
        low: i128,
        high: i128,
    ) -> impl Iterator<Item = usize> + '_ {
        let from = self.by_first.partition_point(|(first, _, _)| *first < low - self.widest);
        self.by_first[from..]
            .iter()
            .take_while(move |(first, _, _)| *first <= high)
            .filter(move |(first, last, _)| *last >= low && *first <= high)
            .map(|(_, _, cell)| *cell)
    }
}

impl Stored {
    fn new(
        unit: &Unit,
        accesses: &Accesses,
        private: Option<&dyn Fn(&MemRef) -> bool>,
    ) -> Self {
        let mut number: IndexMap<MemRef, usize> = IndexMap::default();
        for (_, inst) in unit.function.walk() {
            for cell in stored_cell(unit, accesses, inst).into_iter().chain(accesses.fills(inst).iter().cloned()) {
                let next = number.len();
                number.entry(cell).or_insert(next);
            }
        }
        let cells: Vec<MemRef> = number.keys().cloned().collect();
        let mut buckets = OverlapBuckets::default();
        let index = CellMap::new((0..cells.len()).map(|at| (at, ())).collect(), |at| {
            (overlap_bucket(&mut buckets, &cells[*at]), overlap_span(&cells[*at]))
        });
        let mut marked = Bits::new(cells.len());
        if let Some(private) = private {
            cells.iter().enumerate().filter(|(_, one)| private(one)).for_each(|(at, _)| marked.insert(at));
        }
        let size = cells.len();
        let mut groups: HashMap<Group, Members> = HashMap::default();
        let mut group_of = Vec::with_capacity(size);
        for (at, cell) in cells.iter().enumerate() {
            let keys = memoryssa::byte_keys(unit, cell);
            let mine = [
                keys.frame.map(|(frame, low, high)| (Group::Frame(frame), low, high)),
                keys.pointer.map(|(base, offset, bytes)| {
                    (Group::Base(base), i128::from(offset), i128::from(offset) + i128::from(bytes))
                }),
            ];
            for (group, low, high) in mine.iter().flatten() {
                let members = groups.entry(group.clone()).or_default();
                members.by_first.push((*low, *high, at));
                members.widest = members.widest.max(high - low);
            }
            group_of.push(mine);
        }
        for members in groups.values_mut() {
            members.by_first.sort_unstable();
        }
        let mut selfless = Bits::new(size);
        (0..size).filter(|&at| !same_bytes(unit, &cells[at], &cells[at])).for_each(|at| selfless.insert(at));
        Self {
            selfless,
            cells,
            number,
            index,
            groups,
            group_of,
            private: marked,
            covering: RefCell::new(vec![None; size]),
            alike: RefCell::new(vec![None; size]),
            alike_cells: RefCell::new(vec![None; size]),
            pieces: RefCell::new(vec![None; size]),
            twins: RefCell::default(),
        }
    }

    fn none(&self) -> Bits {
        Bits::new(self.cells.len())
    }

    fn related(
        &self,
        memo: &RefCell<Vec<Option<Rc<Bits>>>>,
        kind: usize,
        at: usize,
        related: impl Fn(&MemRef, &MemRef) -> bool,
    ) -> Rc<Bits> {
        if let Some(known) = &memo.borrow()[at] {
            return known.clone();
        }
        // `placed` reads of a cell only the keys it is found by, so cells with
        // the same keys are related to the same cells: the N stores to one
        // address of a block are one set, not N.
        let key = self.group_of[at].iter().any(Option::is_some).then(|| (kind, self.group_of[at].clone()));
        if let Some(shared) = key.as_ref().and_then(|key| self.twins.borrow().get(key).cloned()) {
            if llrm_support::env_flag!("LLRM_CHECK_CLOBBERS") {
                let all =
                    (0..self.cells.len()).filter(|&one| related(&self.cells[at], &self.cells[one])).collect::<Vec<_>>();
                assert!(
                    shared.iter().collect::<Vec<_>>() == all,
                    "LLRM_CHECK_CLOBBERS: cells with the same keys are related to different cells"
                );
            }
            memo.borrow_mut()[at] = Some(shared.clone());
            return shared;
        }
        let mut found = self.none();
        let mut test = |one: usize| {
            RELATED.with(|asked| asked.set(asked.get() + 1));
            if related(&self.cells[at], &self.cells[one]) {
                found.insert(one);
            }
        };
        // Every relation asked here needs `placed` to place the two cells,
        // which takes one frame or one base pointer in common
        // (`ByteKeys`) and bytes that meet: the cell itself and the
        // cells of its groups whose bytes meet its own are the ones
        // that can be related.
        let mut asked: BTreeSet<usize> = BTreeSet::from([at]);
        for (group, low, high) in self.group_of[at].iter().flatten() {
            asked.extend(self.groups[group].meeting(*low, *high));
        }
        asked.into_iter().for_each(&mut test);
        if llrm_support::env_flag!("LLRM_CHECK_CLOBBERS") {
            let all =
                (0..self.cells.len()).filter(|&one| related(&self.cells[at], &self.cells[one])).collect::<Vec<_>>();
            assert!(
                found.iter().collect::<Vec<_>>() == all,
                "LLRM_CHECK_CLOBBERS: cells sharing bytes were in no shared bucket"
            );
        }
        let found = Rc::new(found);
        if let Some(key) = key {
            self.twins.borrow_mut().insert(key, found.clone());
        }
        memo.borrow_mut()[at] = Some(found.clone());
        found
    }

    /// The cells a store to which writes every byte of cell `at`.
    fn covering(
        &self,
        unit: &Unit,
        at: usize,
    ) -> Rc<Bits> {
        self.related(&self.covering, 0, at, |one, other| covers(unit, other, one))
    }

    fn alike(
        &self,
        unit: &Unit,
        at: usize,
    ) -> Rc<Bits> {
        self.related(&self.alike, 1, at, |one, other| same_bytes(unit, one, other))
    }

    /// Whether a cell naming the bytes of cell `at` is in `set`.
    fn alike_in(
        &self,
        unit: &Unit,
        at: usize,
        set: &Bits,
    ) -> bool {
        ALIKE_ASKS.with(|asked| asked.set(asked.get() + 1));
        let known = self.alike_cells.borrow()[at].clone();
        let cells = known.unwrap_or_else(|| {
            let cells: Rc<[usize]> = self.alike(unit, at).iter().collect();
            self.alike_cells.borrow_mut()[at] = Some(cells.clone());
            cells
        });
        cells.iter().any(|&one| set.contains(one))
    }

    /// Whether the cells in `overwritten` together write every byte of
    /// cell `at`, no one of them all.
    fn pieced(
        &self,
        unit: &Unit,
        at: usize,
        overwritten: &Bits,
    ) -> bool {
        let pieces = self.related(&self.pieces, 2, at, |one, other| {
            placed(unit, other, one)
                .zip(placed(unit, one, one))
                .is_some_and(|((low, high), (_, size))| low < size && high > 0)
        });
        if pieces.iter().filter(|piece| overwritten.contains(*piece)).nth(1).is_none() {
            return false;
        }
        let written: Vec<&MemRef> =
            pieces.iter().filter(|piece| overwritten.contains(*piece)).map(|piece| &self.cells[piece]).collect();
        covered(unit, &written, &self.cells[at])
    }
}

/// The cells in the buckets an instruction's accesses reach.
struct Reached {
    all: Bits,
    each: Vec<Bits>,
}

/// What a dead-store solve reads of the function besides its instructions.
struct Solve<'a, 'u> {
    unit: &'a Unit<'u>,
    accesses: &'a Accesses,
    private: Option<&'a dyn Fn(&MemRef) -> bool>,
    escaped: Option<&'a EscapedBefore>,
    stored: Stored,
    /// Per instruction, lazily: the cells in the buckets each of its accesses
    /// reaches, and all of them together. They do not move with the round, so
    /// a round asks the live ones of these and not the buckets again.
    reached: RefCell<HashMap<InstId, Rc<Reached>>>,
    /// Per instruction, lazily: the cell it purely stores to, as numbered.
    cell_of: RefCell<HashMap<InstId, Option<usize>>>,
}

impl Solve<'_, '_> {
    /// One block, backward, from what its successors have already overwritten.
    ///
    /// Returns the stores it found dead and what is overwritten on entry, so
    /// the caller can carry the second to the predecessors.
    fn dead_in(
        &self,
        block: i64,
        overwritten: &Bits,
    ) -> (Vec<InstId>, Bits) {
        let unit = self.unit;
        let stored = &self.stored;
        let mut found = Vec::new();
        let mut overwritten = overwritten.clone();
        for &inst in unit.function.block(cfg::block(block)).instructions().iter().rev() {
            let opcode = &unit.function.instruction(inst).opcode;
            let call = matches!(opcode, Opcode::Call(_) | Opcode::Invoke(_));
            // Nothing can read a private cell but by its name: not a call,
            // and not an address this cannot resolve.
            let shielded = self.private.is_some();
            let (Some(loads), Some(stores)) = (self.accesses.reads(inst), self.accesses.writes(inst)) else {
                if shielded && call {
                    overwritten.intersect_with(&stored.private);
                } else {
                    overwritten = stored.none();
                }
                continue;
            };

            let cell = *self
                .cell_of
                .borrow_mut()
                .entry(inst)
                .or_insert_with(|| stored_cell(unit, self.accesses, inst).map(|cell| stored.number[&cell]));
            if let Some(at) = cell {
                if stored.covering(unit, at).intersects(&overwritten) || stored.pieced(unit, at, &overwritten) {
                    found.push(inst);
                }
                overwritten.insert(at);
                continue;
            }

            // Anything this reads or writes puts the cells it may touch back in
            // doubt.
            if !overwritten.is_empty() {
                let reached = self.reached(inst, loads, stores);
                if reached.all.intersects(&overwritten) {
                    for (reference, cells) in loads.iter().chain(stores).zip(&reached.each) {
                        self.clobber(
                            &mut overwritten,
                            inst,
                            cells,
                            reference,
                            shielded && (call || !reference.named()),
                        );
                    }
                }
            }
            // What a call fills it writes before reading, unless it may read it
            // otherwise.
            for fill in self.accesses.fills(inst) {
                let at = stored.number[fill];
                if !loads.iter().any(|one| self.reaches(inst, one, at, shielded && (call || !one.named()))) {
                    overwritten.insert(at);
                }
            }
        }
        (found, overwritten)
    }

    /// Whether an access through `reference` at `inst` may touch cell `at`,
    /// for all the index knows; an `unnamed` one cannot reach a private cell.
    fn within(
        &self,
        inst: InstId,
        reference: &MemRef,
        at: usize,
        unnamed: bool,
    ) -> bool {
        let stored = &self.stored;
        !(unnamed && stored.private.contains(at))
            && !self.escaped.is_some_and(|escaped| escaped.apart(inst, reference, &stored.cells[at]))
    }

    /// Whether an access through `reference` at `inst` may touch cell `at`.
    fn reaches(
        &self,
        inst: InstId,
        reference: &MemRef,
        at: usize,
        unnamed: bool,
    ) -> bool {
        self.within(inst, reference, at, unnamed) && may_clobber(self.unit, None, &self.stored.cells[at], reference)
    }

    fn reached(
        &self,
        inst: InstId,
        loads: &[MemRef],
        stores: &[MemRef],
    ) -> Rc<Reached> {
        let stored = &self.stored;
        self.reached
            .borrow_mut()
            .entry(inst)
            .or_insert_with(|| {
                let each: Vec<Bits> = loads
                    .iter()
                    .chain(stores)
                    .map(|reference| {
                        let mut cells = stored.none();
                        let mut take = |at: &usize| cells.insert(*at);
                        match overlap_buckets(reference, &stored.index.parts) {
                            None => stored.index.buckets.values().flat_map(IndexMap::keys).for_each(&mut take),
                            Some(buckets) => buckets
                                .iter()
                                .filter_map(|bucket| stored.index.buckets.get(bucket))
                                .flat_map(IndexMap::keys)
                                .for_each(&mut take),
                        }
                        cells
                    })
                    .collect();
                let mut all = stored.none();
                each.iter().for_each(|cells| all.union_with(cells));
                Rc::new(Reached { all, each })
            })
            .clone()
    }

    /// Forget the cells an access through `reference` at `inst` may touch,
    /// among those of `reached`; an `unnamed` one cannot reach a private cell.
    fn clobber(
        &self,
        overwritten: &mut Bits,
        inst: InstId,
        reached: &Bits,
        reference: &MemRef,
        unnamed: bool,
    ) {
        if !reached.intersects(overwritten) {
            return;
        }
        let mut live = reached.clone();
        live.intersect_with(overwritten);
        for at in live.iter() {
            if self.within(inst, reference, at, unnamed)
                && may_clobber(self.unit, None, &self.stored.cells[at], reference)
            {
                overwritten.remove(at);
            }
        }
    }
}

/// Stores whose bytes are overwritten before anything reads them.
///
/// Instructions, not addresses: two stores may share an address with an
/// unrelated live load.
///
/// Backward through each block: a store to a cell that a later store
/// overwrites, with nothing in between that could have read it, computed
/// nothing. Across edges a cell has to be overwritten on *every* successor
/// path, so what a block starts from is the intersection of what its
/// successors have. The fixed point starts from every stored cell and
/// shrinks; only the last round's verdicts stand.
///
/// A block with no successor starts from nothing: the caller may read the
/// cell -- unless `private` says nothing outside the function can, in which
/// case it starts from every such cell stored here. One ending in
/// `unreachable` starts from every cell.
pub fn dead_stores(
    unit: &Unit,
    accesses: &Accesses,
    private: Option<&dyn Fn(&MemRef) -> bool>,
) -> Vec<InstId> {
    dead_stores_escaping(unit, accesses, private, None)
}

/// `dead_stores`, an access through a pointer no fact follows sparing a
/// frame cell whose object had not escaped by then, as `escaped` says.
pub fn dead_stores_escaping(
    unit: &Unit,
    accesses: &Accesses,
    private: Option<&dyn Fn(&MemRef) -> bool>,
    escaped: Option<&EscapedBefore>,
) -> Vec<InstId> {
    let solve = Solve {
        unit,
        accesses,
        private,
        escaped,
        stored: Stored::new(unit, accesses, private),
        reached: RefCell::default(),
        cell_of: RefCell::default(),
    };
    let stored = &solve.stored;
    let mut every = stored.none();
    (0..stored.cells.len()).for_each(|at| every.insert(at));
    let graph = cfg::graph(unit.function);
    let mut entry: IndexMap<i64, Bits> = graph.iter().map(|block| (block.at, every.clone())).collect();
    let unread = if private.is_some() { stored.private.clone() } else { stored.none() };

    // A block's answer depends only on what is overwritten after it: worked
    // out again when a successor's entry shrank, the blocks in the order of a
    // backward sweep.
    let by_rank = postorder(&graph);
    let rank: HashMap<i64, usize> = by_rank.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut readers: HashMap<i64, Vec<usize>> = HashMap::default();
    for block in &graph {
        for successor in &block.succ {
            readers.entry(*successor).or_default().push(rank[&block.at]);
        }
    }
    let mut mines: HashMap<i64, Vec<InstId>> = HashMap::default();
    let mut last: HashMap<i64, Bits> = HashMap::default();
    let mut work: std::collections::BinaryHeap<std::cmp::Reverse<usize>> =
        (0..by_rank.len()).map(std::cmp::Reverse).collect();
    let mut queued = vec![true; by_rank.len()];
    while let Some(std::cmp::Reverse(at)) = work.pop() {
        queued[at] = false;
        let block = by_rank[at];
        // What a cell is overwritten by on every successor path: the first's
        // entry, less the cells no cell naming their bytes is overwritten in
        // another's. A cell that names its own bytes and is in the entry
        // stays, so only the others are asked.
        let mut haves = block.succ.iter().map(|successor| &entry[successor]);
        let out = haves
            .next()
            .map(
                |first| {
                    let mut out = first.clone();
                    for have in haves {
                        let mut doubt = out.clone();
                        doubt.subtract(have);
                        let mut selfless = out.clone();
                        selfless.intersect_with(&stored.selfless);
                        doubt.union_with(&selfless);
                        let gone: Vec<usize> = doubt.iter().filter(|&one| !stored.alike_in(unit, one, have)).collect();
                        gone.into_iter().for_each(|one| out.remove(one));
                    }
                    out
                },
            );
        // No successor at all: only the caller may read it -- and after an
        // `unreachable` nothing does, as LLVM's DSE skips such exits; the
        // noreturn call before it reads what escaped by then.
        let out = out.unwrap_or_else(|| if aborts(unit, block.at) { every.clone() } else { unread.clone() });
        if last.get(&block.at) == Some(&out) {
            continue;
        }
        let (mine, start) = solve.dead_in(block.at, &out);
        last.insert(block.at, out);
        mines.insert(block.at, mine);
        if start.len() != entry[&block.at].len() {
            for &reader in readers.get(&block.at).into_iter().flatten() {
                if !std::mem::replace(&mut queued[reader], true) {
                    work.push(std::cmp::Reverse(reader));
                }
            }
        }
        entry.insert(block.at, start);
    }
    let found: BTreeSet<InstId> = mines.into_values().flatten().collect();
    unit.function.walk().map(|(_, inst)| inst).filter(|inst| found.contains(inst)).collect()
}

/// The blocks, each after the ones it reaches (a depth-first postorder from
/// the first block; those it does not reach, last, in reverse layout).
fn postorder(graph: &[cfg::Block]) -> Vec<&cfg::Block> {
    let index: HashMap<i64, usize> = graph.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut seen = vec![false; graph.len()];
    let mut order = Vec::with_capacity(graph.len());
    let roots = (0..graph.len().min(1)).chain((0..graph.len()).rev());
    for root in roots {
        if std::mem::replace(&mut seen[root], true) {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        while let Some((at, next)) = stack.pop() {
            match graph[at].succ.get(next).map(|successor| index[successor]) {
                Some(one) => {
                    stack.push((at, next + 1));
                    if !std::mem::replace(&mut seen[one], true) {
                        stack.push((one, 0));
                    }
                }
                None => order.push(&graph[at]),
            }
        }
    }
    order
}

/// Whether block `at` ends in `unreachable`.
fn aborts(
    unit: &Unit,
    at: i64,
) -> bool {
    unit.function
        .terminator(cfg::block(at))
        .is_some_and(|last| unit.function.instruction(last).opcode == Opcode::Unreachable)
}

thread_local! {
    static ALIKE_ASKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has asked whether a cell naming a cell's bytes is
/// overwritten on a path, for a test that a join asks of the cells that differ.
pub fn alike_asks() -> usize {
    ALIKE_ASKS.with(std::cell::Cell::get)
}

thread_local! {
    static RELATED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many pairs of cells this thread has asked whether they share bytes, for
/// a test that a cell is compared with those in its buckets and not with all.
pub fn related_pairs() -> usize {
    RELATED.with(std::cell::Cell::get)
}

thread_local! {
    static SAMES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has compared a load's bytes with a missing one's
/// in `memory_providers`.
pub fn same_runs() -> usize {
    SAMES.with(std::cell::Cell::get)
}

thread_local! {
    static COLLECTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many loads this thread has collected as candidates for a missing one in
/// `memory_providers`, for a test that a load blocked by a write collects none.
pub fn collected_candidates() -> usize {
    COLLECTED.with(std::cell::Cell::get)
}

/// Whether `holder` can stand for `value`: it has its type.
fn serves(
    unit: &Unit,
    holder: Operand,
    value: ValueId,
) -> bool {
    unit.operand_type(holder) == Some(unit.function.value(value).ty)
}

/// Recover dominating memory values the forward lattice lost at loops.
fn memory_providers(
    unit: &Unit,
    accesses: &Accesses,
    missing: &[InstId],
) -> Vec<Forward> {
    let function = unit.function;
    // Register facts only, as the availability map reads them: a memory-aware
    // solve per query costs more than it finds.
    let graph = memoryssa::built(unit, accesses).with_known(ranges::constants(unit).into_iter().collect());
    let shape = unit.shape();
    let dominance = &shape.dominance;
    let places: HashMap<InstId, (i64, usize)> = function
        .layout()
        .iter()
        .flat_map(|&block| {
            function
                .block(block)
                .instructions()
                .iter()
                .enumerate()
                .map(move |(index, &inst)| (inst, (cfg::id(block), index)))
        })
        .collect();
    let loads: Vec<(InstId, (MemRef, ValueId))> =
        graph.sites.keys().filter_map(|&site| loaded_into(unit, accesses, site).map(|loaded| (site, loaded))).collect();
    // Where each load is among them, which is the order of its block.
    let mut load_of: llrm_mir::dense::IdMap<InstId, usize> = llrm_mir::dense::IdMap::new();
    for (at, (site, _)) in loads.iter().enumerate() {
        load_of.insert(*site, at);
    }
    let available = |source: InstId, site: InstId| -> bool {
        let ((source_block, source_index), (block, index)) = (places[&source], places[&site]);
        dominance.dominates(source_block, block) && (source_block != block || source_index < index)
    };

    // Whether `value` is defined where `site` can read it.
    let available_value = |value: Operand, site: InstId| -> bool {
        match value {
            Operand::Value(one) => match function.value(one).def {
                llrm_mir::module::ValueDef::Instruction(def) => available(def, site),
                _ => true,
            },
            _ => true,
        }
    };

    let mut groups: Vec<(&MemRef, Vec<usize>)> = Vec::new();
    let mut group_of: HashMap<&MemRef, usize> = HashMap::default();
    // The groups by what a group naming the same bytes shares, so a load
    // compares itself with those and not with all.
    let mut group_keys: Vec<Vec<memoryssa::ByteKey>> = Vec::new();
    let mut load_group: Vec<usize> = Vec::with_capacity(loads.len());
    for (at, (_, (loaded, _))) in loads.iter().enumerate() {
        let group = *group_of
            .entry(loaded)
            .or_insert_with(
                || {
                    groups.push((loaded, Vec::new()));
                    group_keys.push(memoryssa::byte_keys(unit, loaded).list());
                    groups.len() - 1
                },
            );
        groups[group].1.push(at);
        load_group.push(group);
    }
    // The loads by the bytes they name, in the blocks that hold them: those
    // that dominate a site are found from its block upward, nearest first,
    // where every load of the same bytes was taken and sorted for each site
    // (9.5 M candidates for 7,000 sites at N=1024).
    let tree = shape.dominance.tree();
    let nearest = crate::nearest::Nearest::of(
        tree,
        loads
            .iter()
            .enumerate()
            .filter_map(|(at, (site, _))| function.parent(*site).map(|block| (at, block)))
            .flat_map(|(at, block)| group_keys[load_group[at]].iter().cloned().map(move |key| (key, block, at))),
    );
    let mut found = Vec::new();
    for &site in missing {
        let Some((cell, result)) = loaded_into(unit, accesses, site) else {
            continue;
        };
        // A load that is invariant is clobbered by nothing; the store before it
        // that initialised what it reads is still the value it has.
        let clobbers = graph.nearest_writes(site, &cell);
        let single = if clobbers.len() == 1 { clobbers.first().map(|id| graph.access(*id)) } else { None };
        if let Some(source) = single.filter(|access| access.kind == memoryssa::Kind::Def).and_then(|access| access.site)
            && let Some((stored, value)) = stored_from(unit, accesses, source)
            && available(source, site)
            && same_bytes(unit, &stored, &cell)
            && serves(unit, value, result)
        {
            found.push(Forward { at: site, value });
            continue;
        }
        // Several clobbers, each an exact store of the one value, and that
        // value defined where it reaches the load: the arms of a join
        // that stored the same thing.
        if (2..=memoryssa::MAX_NEAREST_WRITES).contains(&clobbers.len())
            && let Some(value) = joined_store(unit, accesses, &graph, &clobbers, &cell, site, &available_value)
            && serves(unit, value, result)
        {
            found.push(Forward { at: site, value });
            continue;
        }
        // Loads of one address are one group, whose bytes are compared with
        // `cell` once; the candidates are then taken in load order, as
        // when each was compared.
        let keys = memoryssa::byte_keys(unit, &cell).list();
        // Whether the loads of a group name the bytes of `cell`, asked once of
        // each.
        let mut same: HashMap<usize, bool> = HashMap::default();
        let mut names = |group: usize| -> bool {
            *same
                .entry(group)
                .or_insert_with(
                    || {
                        SAMES.with(|runs| runs.set(runs.get() + 1));
                        same_bytes(unit, groups[group].0, &cell)
                    },
                )
        };
        // The loads that dominate the site and name its bytes, in load order.
        // None for a site the entry does not reach: nothing is `available`
        // there.
        let reached = function.parent(site).filter(|&block| tree.is_reachable(block));
        let below = load_of.get(&site).copied().unwrap_or(usize::MAX);
        let walk = || {
            reached.into_iter().flat_map(|block| {
                nearest.dominating_below(tree, &keys.iter().collect::<Vec<_>>(), block, below).map(|(_, at)| at)
            })
        };
        // Nearest first (the order `dominating` yields), the first load that is
        // usable here: a write between it and the site is between every
        // load above it and the site too, so where its walk finds one, no other
        // load can serve, and the loads above it, tens to thousands of
        // them on a chain of branches, are neither collected nor sorted (338 of
        // the 707 Minstr `memory_providers` spent on `branches` at
        // N=1024).
        let usable = |&at: &usize| {
            let (source, (_, value)) = &loads[at];
            available(*source, site) && serves(unit, Operand::Value(*value), result)
        };
        let nearest_usable = walk().filter(|&at| names(load_group[at])).find(|at| usable(at));
        let checked = llrm_support::env_flag!("LLRM_CHECK_GROUPS");
        let served = nearest_usable.filter(|&at| {
            let (source, (loaded, _)) = &loads[at];
            graph.unchanged(*source, site, loaded)
        });
        if !checked {
            if let Some(at) = served {
                found.push(Forward { at: site, value: Operand::Value(loads[at].1.1) });
            }
            continue;
        }
        let none_can_serve = served.is_none();
        let mut candidates: Vec<usize> = walk().filter(|&at| names(load_group[at])).collect();
        candidates.sort_unstable();
        COLLECTED.with(|runs| runs.set(runs.get() + candidates.len()));
        if checked {
            let every: Vec<usize> = groups
                .iter()
                .filter(|(loaded, _)| same_bytes(unit, loaded, &cell))
                .flat_map(|(_, members)| members.iter().copied())
                .filter(|&at| available(loads[at].0, site))
                .collect();
            let mut every = every;
            every.sort_unstable();
            let kept: Vec<usize> = candidates.iter().copied().filter(|&at| available(loads[at].0, site)).collect();
            assert_eq!(
                kept, every,
                "LLRM_CHECK_GROUPS: the loads that dominate a site, found from its block up, are not those a scan of every group finds"
            );
        }
        // Nearest first: a walk from the load stops at the first value that
        // holds, and the loads before it, each a walk to fail, are not tried.
        // Every candidate holds the same bytes, so the one a chain of served
        // loads ends in is the same. A candidate whose walk finds a write
        // fails every load that dominates it too: the write lies on a path
        // from the farther one through the nearer, so those are not walked.
        let mut failed: Vec<InstId> = Vec::new();
        let before = found.len();
        for at in candidates.into_iter().rev() {
            let (source, (loaded, value)) = &loads[at];
            if !available(*source, site)
                || !serves(unit, Operand::Value(*value), result)
                || failed.iter().any(|&nearer| available(*source, nearer))
            {
                continue;
            }
            if graph.unchanged(*source, site, loaded) {
                found.push(Forward { at: site, value: Operand::Value(*value) });
                break;
            }
            failed.push(*source);
        }
        assert!(
            !(none_can_serve && found.len() > before),
            "LLRM_CHECK_GROUPS: the nearest usable load's walk found a write, and another load served"
        );
    }
    found
}

/// The one value every clobber of a load stores into its exact bytes, when each
/// clobber is such a store and the value is defined where `site` reads it.
fn joined_store(
    unit: &Unit,
    accesses: &Accesses,
    graph: &memoryssa::MemorySSA,
    clobbers: &BTreeSet<usize>,
    cell: &MemRef,
    site: InstId,
    available_value: &dyn Fn(Operand, InstId) -> bool,
) -> Option<Operand> {
    let mut one = None;
    for id in clobbers {
        let access = graph.access(*id);
        let source = access.site.filter(|_| access.kind == memoryssa::Kind::Def)?;
        let (stored, value) = stored_from(unit, accesses, source)?;
        if !same_bytes(unit, &stored, cell) || one.is_some_and(|before| before != value) {
            return None;
        }
        one = Some(value);
    }
    one.filter(|&value| available_value(value, site))
}

#[cfg(test)]
#[path = "avail_tests.rs"]
mod tests;
