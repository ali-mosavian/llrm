//! Port of `qbopt/analysis/avail.py`: which value holds a cell's contents,
//! across blocks.
//!
//! A forward dataflow whose fact is
//!
//!     MemRef -> Value
//!
//! meaning "these bytes are this SSA value". Keyed on MemRef rather than on
//! Addr because MemRef carries the SSA values its own address is reached
//! through, so the fact survives anything that reallocates registers -- which
//! is the whole reason to state it over MIR instead of over machine code.
//!
//! An entry identifies an SSA value, not a physical register. Forwarding adds
//! a use and extends that value's lifetime; allocation preserves or spills it
//! as needed. Requiring it to be live already would retain BC's statement-local
//! lifetimes instead of optimizing them.
//!
//! This lattice intersects at joins. Where predecessor values differ,
//! `optimize/loadjoins.rs` can form a value phi using MemorySSA's per-edge
//! availability proof. Lowering and allocation handle that phi normally.

#![allow(private_interfaces)] // `RegionLayout` and `Interval` are crate-private types.

use std::cell::RefCell;
use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::{HashMap, IndexMap};

use super::memoryssa;
use super::ranges::{self, Interval};
use super::cellmap::CellMap;
use super::regions::{OverlapBucket, RegionLayout, overlap_bucket, overlap_buckets, overlapping};
use super::{effects, loops};
use crate::model::mir::{self, Arg, Const, Kind, MemRef, MirBlock, MirBody, Op, Symbol, Value};
use crate::objectfile::module::Space;
use crate::support::bits::Bits;

/// What a cell maps to: the value, or the constant a store wrote outright.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Holder {
    Value(Value),
    Const(Const),
    Symbol(Symbol),
}

// What a cell maps to, and the whole lattice element.
pub type Holders = IndexMap<MemRef, Holder>;

/// The map on entry to and exit from each block.
#[derive(Clone, Debug)]
pub struct Held {
    pub into: IndexMap<i64, Holders>,
    pub outof: IndexMap<i64, Holders>,
    // Each value's constant, so a far access through a literal selector is
    // not taken to reach the frame.
    pub known: BTreeMap<Value, Interval>,
}

/// Values used only to reach an operand, not read as data.
fn _addressing(op: &Op) -> BTreeSet<Value> {
    let mut found = BTreeSet::new();
    for r#ref in op.loads.iter().chain(&op.stores) {
        if let Some(base) = r#ref.base {
            found.insert(base);
        }
        if let Some(segment) = r#ref.segment {
            found.insert(segment);
        }
    }
    found
}

fn _real(values: &[Value]) -> Vec<Value> {
    values.iter().filter(|one| !one.flags).copied().collect()
}

/// The cell this op purely loads, and the value it lands in.
///
/// Purely: a LOAD, one read, no write, one value defined, nothing read as data, and
/// an address something can name, either a symbolic cell or a whole SSA
/// pointer. An anonymous memory effect is neither and cannot supply data.
/// `and cx,[x]` fails the last test -- it uses cx as data as well as
/// defining it, so the bytes it leaves in cx are not the cell's. Constants
/// are not SSA uses: after folding, `20 + [base]` can have only address uses
/// yet is still not a load.
pub fn loaded_into(op: &Op) -> Option<(MemRef, Value)> {
    if op.kind != Kind::Load
        || op.floating.is_some()
        || op.loads.len() != 1
        || !op.stores.is_empty()
        || op.barrier()
        || op.loads[0].addr.is_none() && !op.loads[0].pointer
    {
        return None;
    }
    let defines = _real(&op.defines);
    if defines.len() != 1 {
        return None;
    }
    let addressing = _addressing(op);
    let preserved = _preserved(op);
    let reading = _real(&op.uses)
        .into_iter()
        .any(|one| !addressing.contains(&one) && !preserved.contains(&one));
    if reading {
        return None;
    }
    Some((op.loads[0].clone(), defines[0]))
}

/// The uses that are only the destination's own untouched halves.
///
/// A sixteen-bit load writes half of a thirty-two bit variable, so the
/// high half survives and MIR records a read of the old value. That read
/// is real, but it is not the operation consulting memory's contents,
/// which is the question loaded_into asks.
///
/// **The operation has to be a load.** A subtract from a cell reads the
/// old value the same way, and there the read is the whole point. The two
/// are told apart by the operands -- a load's only argument is the cell, so
/// any other use is a preserved half, while a subtract names its other input
/// among its arguments.
fn _preserved(op: &Op) -> BTreeSet<Value> {
    if op.kind != Kind::Load {
        return BTreeSet::new();
    }
    let named: BTreeSet<Value> = op
        .args
        .iter()
        .filter_map(|one| match one {
            Arg::Held(held) => Some(held.value),
            _ => None,
        })
        .collect();
    _real(&op.uses).into_iter().filter(|one| !named.contains(one)).collect()
}

/// The cell this op purely stores, and the value it wrote there.
///
/// Purely: the cell receives the value, not something computed from it.
/// `inc [x]` and `add [x],1` read one value and store another, and naming
/// the one they read made deedlines' zoom read `kxy0%` as the value it
/// held before the branch that changed it.
pub fn stored_from(op: &Op) -> Option<(MemRef, Holder)> {
    if !matches!(op.kind, Kind::Store | Kind::Arg)
        || op.floating.is_some()
        || op.stores.len() != 1
        || !op.loads.is_empty()
        || op.barrier()
        || op.stores[0].addr.is_none() && !op.stores[0].pointer
    {
        return None;
    }
    if !_real(&op.defines).is_empty() {
        return None;
    }
    let addressing = _addressing(op);
    let reading: Vec<Value> = _real(&op.uses).into_iter().filter(|one| !addressing.contains(one)).collect();
    if reading.is_empty() {
        // A constant written straight to the cell reads no value at all, so
        // the count above refused it and the bytes were unknowable for the
        // rest of the body. `DEF SEG = &HA000` is one store and twenty-three
        // reads, none of which could be served.
        let written: Vec<Holder> = op
            .args
            .iter()
            .filter_map(|one| match one {
                Arg::Const(constant) => Some(Holder::Const(constant.clone())),
                Arg::Symbol(symbol) => Some(Holder::Symbol(*symbol)),
                _ => None,
            })
            .collect();
        let width = match written.first() {
            Some(Holder::Const(constant)) => Some(constant.width),
            Some(Holder::Symbol(symbol)) => Some(symbol.width),
            _ => None,
        };
        if written.len() == 1 && op.args.len() == 1 && width == Some(op.stores[0].width) {
            return Some((op.stores[0].clone(), written[0].clone()));
        }
        return None;
    }
    if reading.len() != 1 {
        return None;
    }
    Some((op.stores[0].clone(), Holder::Value(reading[0])))
}

/// Whether a later store through `other` writes every byte of `ref`.
///
/// Not `same_bytes`, which asks whether the two name the same cell. A long
/// written as one `mov [x],eax` covers both halves BC stored separately.
fn _covered_by(r#ref: &MemRef, other: &MemRef, _dgroup: Option<&RegionLayout>) -> bool {
    let (r#ref, other) = (mir::symbolic_ref(r#ref), mir::symbolic_ref(other));
    let (Some(ref_addr), Some(other_addr)) = (r#ref.addr, other.addr) else {
        return false;
    };
    let mut aligned = other.clone().into_owned();
    aligned.addr = Some(other_addr.plus(ref_addr.disp - other_addr.disp));
    aligned.width = r#ref.width;
    if !mir::same_bytes(&r#ref, &aligned) {
        return false;
    }
    other_addr.disp <= ref_addr.disp
        && ref_addr.disp + i64::from(r#ref.width) <= other_addr.disp + i64::from(other.width)
}

/// The cell this op purely stores to, whatever it put there.
///
/// stored_from() answers a narrower question -- the cell *and the value* --
/// and needs exactly one value read to name the second. Which cell was
/// written is the whole question for the dead-store walk.
pub fn stored_cell(op: &Op) -> Option<&MemRef> {
    if op.floating.is_some()
        || op.stores.len() != 1
        || !op.loads.is_empty()
        || op.barrier()
        || op.stores[0].addr.is_none()
    {
        return None;
    }
    if !_real(&op.defines).is_empty() {
        return None;
    }
    Some(&op.stores[0])
}

/// The map across one op.
fn _after(
    op: &Op,
    mut holders: Holders,
    dgroup: Option<&RegionLayout>,
    _calls: &IndexMap<i64, String>,
    known: Option<&BTreeMap<Value, Interval>>,
) -> Holders {
    if effects::unmodeled_write(op) {
        return IndexMap::default();
    }
    if op.kind == Kind::Call {
        holders.retain(|one, _| _crosses_edges(one));
    }
    for r#ref in &op.stores {
        // Rust refuses a region endpoint Python's integers can express;
        // either way the store may overlap.
        holders.retain(|one, _| !overlapping(one, r#ref, known, known, dgroup).unwrap_or(true));
    }
    let found = stored_from(op).or_else(|| loaded_into(op).map(|(r#ref, value)| (r#ref, Holder::Value(value))));
    if let Some((r#ref, value)) = found {
        holders.insert(r#ref, value);
    }
    holders
}

/// Whether a cell survives a block boundary: stack slots do not.
///
/// A Space.STACK address is a depth measured from the top of the block that
/// pushed it, so `[sp-8]` in one block and `[sp-8]` in another are two
/// different addresses that compare equal. Carrying one across an edge is
/// the one way this analysis could be unsound, so it does not.
fn _crosses_edges(cell: &MemRef) -> bool {
    cell.addr.is_none_or(|addr| addr.space != Space::Stack)
}

/// Only what every predecessor agrees on, value and all.
fn _meet(maps: &[&Holders]) -> Holders {
    let Some(first) = maps.first() else {
        return IndexMap::default();
    };
    let mut out: Holders = first
        .iter()
        .filter(|(one, _)| _crosses_edges(one))
        .map(|(one, who)| (one.clone(), who.clone()))
        .collect();
    // Every key left crosses edges, so a stack slot in `other` never matches one.
    for other in &maps[1..] {
        out.retain(|one, who| other.get(one) == Some(who));
    }
    out
}

/// Which value each cell holds, at every block's entry and exit.
pub fn holders(body: &Rc<MirBody>, dgroup: Option<&RegionLayout>, calls: Option<&IndexMap<i64, String>>) -> Held {
    let calls = calls.cloned().unwrap_or_default();
    // Register facts only: fold has already written a cell's known value into the
    // body, and a memory-aware solve per gvn run was a third of its time.
    let known: BTreeMap<Value, Interval> = ranges::constants(body, None, None).into_iter().collect();
    let mut preds: IndexMap<i64, Vec<i64>> = body.blocks.iter().map(|block| (block.at, Vec::new())).collect();
    for block in &body.blocks {
        for succ in &block.succ {
            if let Some(parents) = preds.get_mut(succ) {
                parents.push(block.at);
            }
        }
    }

    let mut into: IndexMap<i64, Holders> = body.blocks.iter().map(|block| (block.at, IndexMap::default())).collect();
    let mut outof: IndexMap<i64, Holders> = body.blocks.iter().map(|block| (block.at, IndexMap::default())).collect();

    let mut changing = true;
    while changing {
        changing = false;
        for block in &body.blocks {
            let arriving = if block.at == body.entry {
                IndexMap::default()
            } else {
                _meet(&preds[&block.at].iter().map(|one| &outof[one]).collect::<Vec<_>>())
            };
            let mut leaving = arriving.clone();
            for op in &block.ops {
                leaving = _after(op, leaving, dgroup, &calls, Some(&known));
            }
            if arriving != into[&block.at] || leaving != outof[&block.at] {
                into.insert(block.at, arriving);
                outof.insert(block.at, leaving);
                changing = true;
            }
        }
    }

    Held { into, outof, known }
}

/// The value holding `ref`'s bytes just before the op at `at`.
///
/// A new use extends its lifetime; this says nothing about its allocation.
pub fn provider(
    body: &Rc<MirBody>,
    dgroup: Option<&RegionLayout>,
    at: i64,
    r#ref: &MemRef,
    calls: Option<&IndexMap<i64, String>>,
) -> Option<Holder> {
    let calls = calls.cloned().unwrap_or_default();
    let found = holders(body, dgroup, Some(&calls));
    for block in &body.blocks {
        if !block.ops.iter().any(|op| op.at == at) {
            continue;
        }
        let mut current = found.into[&block.at].clone();
        for op in &block.ops {
            if op.at == at {
                return current.iter().find(|(one, _)| mir::same_bytes(one, r#ref)).map(|(_, who)| who.clone());
            }
            current = _after(op, current, dgroup, &calls, Some(&found.known));
        }
    }
    None
}

/// A memory read whose bytes equal a known SSA value, or a constant.
#[derive(Clone, Debug, PartialEq)]
pub struct Forward<'a> {
    pub at: i64,
    // The value holding them. Which register that is, is the allocator's
    // answer; this analysis never asks where either value used to live.
    pub value: Holder,
    pub op: Option<&'a Op>,
}

/// Whether a cell is named outright rather than reached through a value.
///
/// A direct symbolic operand names its bytes.  So does canonical provenance:
/// an SSA pointer may choose the machine address at run time while its object
/// and possible byte lanes are already explicit in MIR.  An unresolved
/// pointer, index or selector remains unnamed and reaches a private cell no
/// more than an unknown call does.
fn _fixed(r#ref: &MemRef) -> bool {
    let canonical = r#ref.provenance.as_ref().is_some_and(|provenance| {
        !provenance.slices.is_empty()
            && provenance.slices.iter().all(|one| one.object.kind != crate::model::memory::MemoryKind::Unknown)
    });
    let direct = r#ref.addr.is_some_and(|addr| addr.direct()) && r#ref.base.is_none() && r#ref.segment.is_none();
    canonical || direct
}

/// Every cell a body stores to, numbered once for its dead-store solve.
///
/// A block's state is a set of these, not a map rebuilt per block: what is
/// overwritten only ever names a cell some op here stores, as LLVM's DSE
/// numbers its MemoryLocations. `buckets` indexes them all as `overlapping`
/// rules writes out; a write asks only the live cells of the buckets it reaches.
struct _Stored<'a> {
    cells: Vec<&'a MemRef>,
    number: IndexMap<&'a MemRef, usize>,
    index: CellMap<usize, (), OverlapBucket>,
    private: Bits,
    /// Per cell, lazily: the cells whose store writes all its bytes, and the cells naming its bytes.
    covering: RefCell<Vec<Option<Bits>>>,
    alike: RefCell<Vec<Option<Bits>>>,
}

impl<'a> _Stored<'a> {
    fn new(body: &'a MirBody, private: Option<&dyn Fn(&MemRef) -> bool>) -> Self {
        let mut number: IndexMap<&'a MemRef, usize> = IndexMap::default();
        for r#ref in body.blocks.iter().flat_map(|block| &block.ops).filter_map(stored_cell) {
            let next = number.len();
            number.entry(r#ref).or_insert(next);
        }
        let cells: Vec<&MemRef> = number.keys().copied().collect();
        let index = CellMap::new((0..cells.len()).map(|at| (at, ())).collect(), |at| (overlap_bucket(cells[*at]), None));
        let mut marked = Bits::new(cells.len());
        if let Some(private) = private {
            cells.iter().enumerate().filter(|(_, one)| private(one)).for_each(|(at, _)| marked.insert(at));
        }
        let size = cells.len();
        Self { cells, number, index, private: marked, covering: RefCell::new(vec![None; size]), alike: RefCell::new(vec![None; size]) }
    }

    fn none(&self) -> Bits {
        Bits::new(self.cells.len())
    }

    fn _related(&self, memo: &RefCell<Vec<Option<Bits>>>, at: usize, related: impl Fn(&MemRef, &MemRef) -> bool) -> Bits {
        if let Some(known) = &memo.borrow()[at] {
            return known.clone();
        }
        let mut found = self.none();
        self.cells.iter().enumerate().filter(|(_, other)| related(self.cells[at], other)).for_each(|(one, _)| found.insert(one));
        memo.borrow_mut()[at] = Some(found.clone());
        found
    }

    fn covering(&self, at: usize, dgroup: Option<&RegionLayout>) -> Bits {
        self._related(&self.covering, at, |one, other| _covered_by(one, other, dgroup))
    }

    fn alike(&self, at: usize) -> Bits {
        self._related(&self.alike, at, mir::same_bytes)
    }
}

/// One block, backward, from what its successors have already overwritten.
///
/// Returns the stores it found dead and what is overwritten on entry, so
/// the caller can carry the second to the predecessors.
#[allow(clippy::too_many_arguments)]
fn _dead_in(
    block: &MirBlock,
    overwritten: &Bits,
    stored: &_Stored,
    dgroup: Option<&RegionLayout>,
    private: Option<&dyn Fn(&MemRef) -> bool>,
    layout: Option<&RegionLayout>,
    sealed: bool,
    handles_errors: bool,
) -> (Vec<usize>, Bits) {
    let mut found: Vec<usize> = Vec::new();
    let mut overwritten = overwritten.clone();
    for op in block.ops.iter().rev() {
        // Nothing can read a private cell but by its name: not a call, and
        // not an address this cannot resolve.
        let shielded = private.is_some() && op.floating.is_none() && !op.barrier();
        if op.kind == Kind::Join {
            // `push eax / pop ax / pop dx`. A barrier for values, because
            // nothing here can name in SSA what it writes -- and nothing at
            // all for memory: ir.RESTORE_EFFECTS reports no load and no
            // store, since sp comes back where it started and nothing
            // outside the idiom reads the cells it passed through.
            continue;
        }
        let exception = effects::exposes_memory(op, handles_errors);
        if exception || effects::unmodeled_write(op) || effects::unmodeled_read(op) {
            // A float exception's handler runs outside the body, and in a
            // sealed one resumes nowhere inside: a private cell is as safe as
            // across a call, and the op's own cells are read as any op's.
            let caught =
                exception && sealed && private.is_some() && !op.barrier() && !effects::unmodeled_write(op);
            let kept = (shielded && op.kind == Kind::Call) || caught;
            if kept {
                overwritten.intersect_with(&stored.private);
            } else {
                overwritten = stored.none();
            }
            if !caught {
                continue;
            }
        }

        let wrote = stored_cell(op);
        if let Some(r#ref) = wrote {
            if r#ref.addr.is_some_and(|addr| addr.space != Space::Stack) {
                let at = stored.number[r#ref];
                if stored.covering(at, dgroup).intersects(&overwritten) {
                    found.push(op as *const Op as usize);
                }
                overwritten.insert(at);
                continue;
            }
        }

        // Anything this op reads, and anything it writes that this
        // cannot name, puts the cells it may touch back in doubt.
        for r#ref in &op.loads {
            if r#ref.addr.is_some_and(|addr| addr.space == Space::Stack) {
                continue; // a pop, for the same reason a push is skipped below
            }
            _clobber(&mut overwritten, stored, r#ref, shielded && (op.kind == Kind::Call || !_fixed(r#ref)), layout);
        }
        if wrote.is_none() {
            for r#ref in &op.stores {
                if r#ref.addr.is_some_and(|addr| addr.space == Space::Stack) {
                    // A push. `stored_from()` does not name it, and regions
                    // says a stack cell and a static may be the same byte,
                    // because BC runs with SS == DS. True only of a program
                    // whose stack has already grown down into its own data.
                    continue;
                }
                _clobber(&mut overwritten, stored, r#ref, shielded && (op.kind == Kind::Call || !_fixed(r#ref)), layout);
            }
        }
    }
    (found, overwritten)
}

#[cfg(test)]
thread_local! {
    /// Overlap tests dead stores asked, for the test that pins its index.
    pub(crate) static DEAD_OVERLAPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Forget the cells an access through `reference` may touch; an `unnamed` one cannot reach a private cell.
fn _clobber(overwritten: &mut Bits, stored: &_Stored, reference: &MemRef, unnamed: bool, layout: Option<&RegionLayout>) {
    let live = |at: &usize| overwritten.contains(*at) && !(unnamed && stored.private.contains(*at));
    let reached: Vec<usize> = match overlap_buckets(reference, &stored.index.parts) {
        None => stored.index.buckets.values().flat_map(IndexMap::keys).filter(|at| live(at)).copied().collect(),
        Some(buckets) => buckets
            .iter()
            .filter_map(|bucket| stored.index.buckets.get(bucket))
            .flat_map(IndexMap::keys)
            .filter(|at| live(at))
            .copied()
            .collect(),
    };
    for at in reached {
        #[cfg(test)]
        DEAD_OVERLAPS.with(|asked| asked.set(asked.get() + 1));
        if overlapping(stored.cells[at], reference, None, None, layout).unwrap_or(true) {
            overwritten.remove(at);
        }
    }
}

/// Stores whose bytes are overwritten before anything reads them.
///
/// Return operations, not addresses: inserted stores can share an address
/// with an unrelated live load.
///
/// Backward through each block: a store to a cell that a later store
/// overwrites, with nothing in between that could have read it, computed
/// nothing. Across edges a cell has to be overwritten on *every* successor
/// path, so what a block starts from is the intersection of what its
/// successors have. The fixed point starts from every stored cell and
/// shrinks; only the last round's verdicts stand.
///
/// A block with no successor, or one this cannot see, starts from nothing:
/// the caller may read the cell -- unless `private` says nothing outside
/// the body can, in which case it starts from every such cell stored here.
///
/// Stack slots are excluded outright rather than reasoned about.
pub fn dead_stores<'a>(
    body: &'a MirBody,
    dgroup: Option<&RegionLayout>,
    calls: &IndexMap<i64, String>,
    private: Option<&dyn Fn(&MemRef) -> bool>,
    bounds: Option<&IndexMap<(Space, i64), Vec<i64>>>,
    handles_errors: bool,
) -> Vec<&'a Op> {
    crate::support::debug::timed("analysis avail.dead_stores", || _dead_stores_solved(body, dgroup, calls, private, bounds, handles_errors))
}

fn _dead_stores_solved<'a>(
    body: &'a MirBody,
    dgroup: Option<&RegionLayout>,
    _calls: &IndexMap<i64, String>,
    private: Option<&dyn Fn(&MemRef) -> bool>,
    bounds: Option<&IndexMap<(Space, i64), Vec<i64>>>,
    handles_errors: bool,
) -> Vec<&'a Op> {
    // Python passes `dgroup` and `bounds` to regions as separate inputs;
    // Rust's `RegionLayout` carries both.
    let mut layout = dgroup.cloned().unwrap_or_default();
    if let Some(bounds) = bounds {
        layout.landmarks = bounds.iter().map(|(key, marks)| (*key, marks.clone())).collect();
    }
    let layout = Some(&layout);
    let stored = _Stored::new(body, private);
    let mut every = stored.none();
    for (at, one) in stored.cells.iter().enumerate() {
        if one.addr.is_some_and(|addr| addr.space != Space::Stack) {
            every.insert(at);
        }
    }
    let mut entry: IndexMap<i64, Bits> = body.blocks.iter().map(|block| (block.at, every.clone())).collect();
    let unread = if private.is_some() { stored.private.clone() } else { stored.none() };

    let mut sorted: Vec<&MirBlock> = body.blocks.iter().collect();
    sorted.sort_by(|one, other| other.at.cmp(&one.at));
    let mut found: BTreeSet<usize> = BTreeSet::new();
    // A block's answer depends only on what is overwritten after it.
    let mut last: HashMap<i64, (Bits, Vec<usize>, Bits)> = HashMap::default();
    let mut changing = true;
    while changing {
        changing = false;
        found = BTreeSet::new();
        for block in &sorted {
            let mut out: Option<Bits> = None;
            for successor in &block.succ {
                let Some(have) = entry.get(successor) else {
                    // an edge out of this body
                    out = Some(unread.clone());
                    break;
                };
                out = Some(match out {
                    None => have.clone(),
                    Some(mut out) => {
                        let held: Vec<usize> = out.iter().collect();
                        for one in held {
                            if !stored.alike(one).intersects(have) {
                                out.remove(one);
                            }
                        }
                        out
                    }
                });
            }
            // no successor at all: only the caller may read it
            let out = out.unwrap_or_else(|| unread.clone());
            let (mine, start) = match last.get(&block.at) {
                Some((seen, mine, start)) if *seen == out => (mine.clone(), start.clone()),
                _ => {
                    let (mine, start) = _dead_in(block, &out, &stored, dgroup, private, layout, body.sealed, handles_errors);
                    last.insert(block.at, (out, mine.clone(), start.clone()));
                    (mine, start)
                }
            };
            found.extend(mine);
            if start.len() != entry[&block.at].len() {
                changing = true;
            }
            entry.insert(block.at, start);
        }
    }
    body.blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| found.contains(&(*op as *const Op as usize)))
        .collect()
}

/// Reads in `want` that a known SSA value can serve instead of memory.
///
/// The caller replaces the operand, retaining any arithmetic and extending
/// the provider's lifetime. Operation identity distinguishes captures that
/// share a source address.
pub fn forwardable<'a>(
    body: &'a Rc<MirBody>,
    dgroup: Option<&RegionLayout>,
    calls: &IndexMap<i64, String>,
    want: &BTreeSet<i64>,
) -> Vec<Forward<'a>> {
    let held = holders(body, dgroup, Some(calls));
    let mut found: Vec<Forward<'a>> = Vec::new();
    let mut missing: Vec<(memoryssa::Site, &'a Op)> = Vec::new();

    for block in &body.blocks {
        let mut current = held.into[&block.at].clone();
        for (index, op) in block.ops.iter().enumerate() {
            if want.contains(&op.at) && !op.loads.is_empty() {
                let who = current.iter().find(|(cell, _)| mir::same_bytes(cell, &op.loads[0])).map(|(_, w)| w.clone());
                match who {
                    Some(who) => found.push(Forward { at: op.at, value: who, op: Some(op) }),
                    None => missing.push((memoryssa::Site { block: block.at, index }, op)),
                }
            }
            current = _after(op, current, dgroup, calls, Some(&held.known));
        }
    }
    if !missing.is_empty() {
        found.extend(_memory_providers(body, dgroup, &missing));
    }
    found
}

/// Recover dominating memory values lost by the forward lattice at loops.
fn _memory_providers<'a>(
    body: &'a MirBody,
    dgroup: Option<&RegionLayout>,
    missing: &[(memoryssa::Site, &'a Op)],
) -> Vec<Forward<'a>> {
    let graph = memoryssa::built(body);
    let accesses: BTreeMap<usize, &memoryssa::Access> =
        graph.accesses.iter().map(|access| (access.id, access)).collect();
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let loads: Vec<(memoryssa::Site, (MemRef, Value))> = graph
        .operations
        .iter()
        .filter_map(|(site, op)| loaded_into(op).map(|loaded| (*site, loaded)))
        .collect();
    let mut found: Vec<Forward<'a>> = Vec::new();

    let available = |source: memoryssa::Site, site: memoryssa::Site, cell: &MemRef| -> bool {
        dominators[&site.block].contains(&source.block)
            && (source.block != site.block || source.index < site.index)
            && (source.block == site.block || _crosses_edges(cell))
    };

    for (site, op) in missing {
        let (site, op) = (*site, *op);
        if op.loads.len() != 1 || op.barrier() || op.kind == Kind::Call {
            continue;
        }
        let clobbers = graph.clobbers(site, &op.loads[0], dgroup);
        let access = if clobbers.len() == 1 { Some(accesses[clobbers.first().expect("one")]) } else { None };
        if let Some(access) = access.filter(|access| access.kind == memoryssa::Kind::Def) {
            if let Some(access_site) = access.site {
                let stored = stored_from(graph.operations[&access_site]);
                if let Some((cell, value)) = stored {
                    if available(access_site, site, &cell) && graph.pointers.same_bytes(&cell, &op.loads[0]) {
                        found.push(Forward { at: op.at, value, op: Some(op) });
                        continue;
                    }
                }
            }
        }
        for (source, (cell, value)) in &loads {
            let value = Holder::Value(*value);
            if available(*source, site, cell)
                && graph.pointers.same_bytes(cell, &op.loads[0])
                && graph.unchanged(*source, site, cell, dgroup)
            {
                found.push(Forward { at: op.at, value, op: Some(op) });
                break;
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "avail_tests.rs"]
mod tests;
