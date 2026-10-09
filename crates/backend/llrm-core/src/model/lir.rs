//! What each lowered operation requires of a register.
//!
//! Direct port of `qbopt.model.lir`.  This is deliberately not the newer
//! generic Machine IR: LIR owns source-byte spans, and carries from the MIR
//! the answers it was selected with, such as what a call may touch.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::model::ir::{Addr, Held, Operation, Semantics, Space};

use crate::support::hash::IndexMap;

use crate::support::pyrepr::{self, Repr};

/// Every byte a 16-bit frame can address, below and above BP.
pub const WHOLE_FRAME: (Addr, u32) = (Addr::new(Space::Frame, -(1 << 15)), 1 << 16);

/// The frame's bytes no range in `reach` covers.
pub fn outside(reach: &BTreeSet<(i64, i64)>) -> Vec<(Addr, u32)> {
    let (start, size) = WHOLE_FRAME;
    let (low, high) = (start.disp, start.disp + i64::from(size));
    let mut out = Vec::new();
    let mut at = low;
    for &(start, end) in reach {
        if start > at {
            out.push((Addr::new(Space::Frame, at), (start - at) as u32));
        }
        at = at.max(end);
    }
    if at < high {
        out.push((Addr::new(Space::Frame, at), (high - at) as u32));
    }
    out
}

/// What a call may read and write: its effects, as `llrm_mir::memory::of`
/// answers, and the frame bytes it cannot reach, those of the allocas
/// `llrm_analysis::frameescape` finds no pointer to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallMemory {
    pub effects: llrm_mir::memory::Effects,
    pub private: Vec<(Addr, u32)>,
    /// The registers the callee's convention (the description's) disturbs: a function that keeps one for its caller saves it
    /// before such a call, as it does before writing it. Empty where the callee's contract is what the call went by.
    pub disturbs: BTreeSet<iced_x86::Register>,
}

impl CallMemory {
    /// Whether it may write memory.
    #[must_use]
    pub fn writes(&self) -> bool {
        self.effects.writes
    }

    /// Whether no byte of the frame is reachable from it.
    #[must_use]
    pub fn spares_the_frame(&self) -> bool {
        self.private.contains(&WHOLE_FRAME)
    }

    /// Whether the frame bytes at `disp`, `width` long, are private to the
    /// caller; with no `disp`, whether the whole frame is.
    #[must_use]
    pub fn spares(&self, disp: Option<i64>, width: u32) -> bool {
        let (whole, size) = WHOLE_FRAME;
        let (low, high) = match disp {
            Some(disp) => (disp, disp + i64::from(width)),
            None => (whole.disp, whole.disp + i64::from(size)),
        };
        self.private.iter().any(|(start, size)| start.space == Space::Frame && start.disp <= low && high <= start.disp + i64::from(*size))
    }
}

/// What `-g` says of the values an instruction makes, and where in the source's variables it stands. Equal whatever it holds: two
/// instructions the code does not tell apart are not told apart by what a debugger is told of them, or `-g` would change the code
/// wherever a pass compares instructions.
#[derive(Clone, Debug, Default)]
pub struct DebugTags {
    /// The values (by the number their register had in SSA) this instruction defines, whichever register it ends up writing.
    pub defines: Vec<u32>,
    /// The notes (`LirBody::notes`) that stand before it.
    pub before: Vec<u32>,
}

impl PartialEq for DebugTags {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for DebugTags {}

/// One machine instruction, as the thing that emits it needs it.
///
/// Direct port of `qbopt.model.lir:Insn`.  `call` is a shared owned reference
/// so cloned LIR instructions retain it, as Python's frozen dataclass copies do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Insn {
    pub at: i64,
    pub covers: Option<(i64, i64)>,
    pub what: Option<Semantics>,
    pub defines: Vec<u32>,
    pub uses: Vec<u32>,
    pub clobbers: BTreeSet<iced_x86::Register>,
    pub clobbers_high: BTreeSet<iced_x86::Register>,
    pub spread: Vec<(i64, i64)>,
    /// What a call may touch, as the MIR's answers say.
    pub call: Option<Arc<CallMemory>>,
    pub group: Option<i64>,
    pub requires: Vec<(Held, iced_x86::Register)>,
    pub delivers: Vec<(Held, iced_x86::Register)>,
    pub widths: Vec<(u32, u32)>,
    pub symbol: Option<bool>,
    pub spill_reload: bool,
    pub spill_store: bool,
    pub frame_adjust: bool,
    pub rematerialized: bool,
    /// Its memory access must happen exactly as written.
    pub volatile: bool,
    /// A return that reads only its `requires` and the epilogue's registers.
    pub reads_complete: bool,
    /// The source line of the MIR instruction it was selected from (`!dbg`).
    pub line: Option<u32>,
    pub debug: DebugTags,
    /// What it does to the registers (`liveness::effect`), worked out once for everything that asks.
    pub effect: Derived<(u32, Option<crate::backend::liveness::Effect>)>,
}

/// A fact worked out from an instruction's fields, kept on it for every pass that asks. Cloning gives an empty one: a clone is
/// made to be changed (`Insn { what: x, ..(**one).clone() }`, a hundred places), and the old instruction's answer is not the
/// new one's. Instructions in a body are shared and not changed, so what is asked of one is asked once.
pub struct Derived<T>(std::sync::OnceLock<T>);

impl<T> Derived<T> {
    pub fn get_or_init(&self, work: impl FnOnce() -> T) -> &T {
        self.0.get_or_init(work)
    }
}

impl<T> Default for Derived<T> {
    fn default() -> Self {
        Self(std::sync::OnceLock::new())
    }
}

impl<T> Clone for Derived<T> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

/// Not a part of what an instruction is: two that say the same are equal whatever has been asked of them.
impl<T> PartialEq for Derived<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for Derived<T> {}

impl<T> std::fmt::Debug for Derived<T> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Derived")
    }
}

impl LirBody {
    /// The values (by the number their register had in SSA) that `-g`'s notes name.
    #[must_use]
    pub fn named_values(&self) -> BTreeSet<u32> {
        self.notes.iter().filter_map(|note| if let NoteValue::Value(value) = note.value { Some(value) } else { None }).collect()
    }

    /// `self`, a pass's result from `old`, with the notes of the instructions the pass removed or replaced standing before the next
    /// instruction of the block that is still there, or the last: a note says where in the source a variable takes a value, and
    /// that place has not moved because the instruction it stood before is gone.
    #[must_use]
    pub fn with_notes_kept(&self, old: &LirBody) -> LirBody {
        if self.notes.is_empty() {
            return self.clone();
        }
        let mut changed = false;
        let blocks = self
            .blocks
            .iter()
            .map(|block| {
                let Some(before) = old.blocks.iter().find(|one| one.at == block.at) else { return block.clone() };
                let present: BTreeSet<u32> = block.insns.iter().flat_map(|one| one.debug.before.iter().copied()).collect();
                let alive: BTreeSet<*const Insn> = block.insns.iter().map(Arc::as_ptr).collect();
                // Where each lost note goes: the index in the new block of the first old instruction after it that survives.
                let mut moved: Vec<(usize, u32)> = Vec::new();
                for (index, one) in before.insns.iter().enumerate() {
                    let lost: Vec<u32> = one.debug.before.iter().copied().filter(|note| !present.contains(note)).collect();
                    if lost.is_empty() {
                        continue;
                    }
                    let survivor = before.insns[index..].iter().find(|later| alive.contains(&Arc::as_ptr(later))).and_then(|later| block.insns.iter().position(|now| Arc::ptr_eq(now, later)));
                    let at = survivor.or_else(|| block.insns.len().checked_sub(1));
                    if let Some(at) = at {
                        moved.extend(lost.into_iter().map(|note| (at, note)));
                    }
                }
                if moved.is_empty() {
                    return block.clone();
                }
                changed = true;
                let insns = block
                    .insns
                    .iter()
                    .enumerate()
                    .map(|(index, one)| {
                        let mut notes: Vec<u32> = moved.iter().filter(|(at, _)| *at == index).map(|&(_, note)| note).collect();
                        if notes.is_empty() {
                            return Arc::clone(one);
                        }
                        notes.extend(one.debug.before.iter().copied());
                        notes.sort_unstable();
                        Arc::new(Insn { debug: DebugTags { before: notes, ..one.debug.clone() }, ..(**one).clone() })
                    })
                    .collect();
                block.with_insns(insns)
            })
            .collect();
        if changed { self.with_blocks(blocks) } else { self.clone() }
    }

    /// `self`, a pass's result from `old`, with the values the instructions the pass replaced defined told of the last instruction of
    /// the block, made from the same source instruction, that writes the same place: the value is complete there.
    #[must_use]
    pub fn with_defs_kept(&self, old: &LirBody) -> LirBody {
        if self.notes.is_empty() {
            return self.clone();
        }
        let mut changed = false;
        let blocks = self
            .blocks
            .iter()
            .map(|block| {
                let Some(before) = old.blocks.iter().find(|one| one.at == block.at) else { return block.clone() };
                let alive: BTreeSet<*const Insn> = block.insns.iter().map(Arc::as_ptr).collect();
                let present: BTreeSet<u32> = block.insns.iter().flat_map(|one| one.debug.defines.iter().copied()).collect();
                let mut moved: Vec<(usize, Vec<u32>)> = Vec::new();
                for one in &before.insns {
                    if one.arrival() || alive.contains(&Arc::as_ptr(one)) {
                        continue;
                    }
                    let lost: Vec<u32> = one.debug.defines.iter().copied().filter(|tag| !present.contains(tag)).collect();
                    let dest = one.what.as_ref().and_then(|what| what.dests.first());
                    if lost.is_empty() || dest.is_none() {
                        continue;
                    }
                    let at = block.insns.iter().rposition(|now| now.at == one.at && now.what.as_ref().and_then(|what| what.dests.first()) == dest);
                    if let Some(at) = at {
                        moved.push((at, lost));
                    }
                }
                if moved.is_empty() {
                    return block.clone();
                }
                changed = true;
                let insns = block
                    .insns
                    .iter()
                    .enumerate()
                    .map(|(index, one)| {
                        let tags: Vec<u32> = moved.iter().filter(|(at, _)| *at == index).flat_map(|(_, tags)| tags.iter().copied()).collect();
                        if tags.is_empty() {
                            return Arc::clone(one);
                        }
                        let mut defines = one.debug.defines.clone();
                        defines.extend(tags);
                        defines.sort_unstable();
                        defines.dedup();
                        Arc::new(Insn { debug: DebugTags { defines, ..one.debug.clone() }, ..(**one).clone() })
                    })
                    .collect();
                block.with_insns(insns)
            })
            .collect();
        if changed { self.with_blocks(blocks) } else { self.clone() }
    }

    /// `self` with a copy that defines a value `-g` names told so: the moves that phi elimination makes for a phi's result are
    /// the only instructions that make it.
    #[must_use]
    pub fn with_phi_copies_noted(&self) -> Self {
        let named = self.named_values();
        if named.is_empty() {
            return self.clone();
        }
        let blocks = self
            .blocks
            .iter()
            .map(|block| {
                let insns = block
                    .insns
                    .iter()
                    .map(|one| match one.defines.as_slice() {
                        [value] if named.contains(value) && one.debug.defines.is_empty() && one.what.as_ref().is_some_and(|what| what.op == crate::model::ir::Operation::Move) && one.uses.len() == 1 => {
                            Arc::new(Insn { debug: DebugTags { defines: vec![*value], ..one.debug.clone() }, ..(**one).clone() })
                        }
                        _ => Arc::clone(one),
                    })
                    .collect();
                block.with_insns(insns)
            })
            .collect();
        self.with_blocks(blocks)
    }
}

impl Insn {
    /// The instruction at a function's entry that says which registers its arguments arrive in: it is no
    /// code, and anything placed at the entry goes after it, or it clobbers an argument.
    #[must_use]
    pub fn arrival(&self) -> bool {
        self.call.is_none() && !self.delivers.is_empty() && self.what.as_ref().is_some_and(|what| what.op == crate::model::ir::Operation::Nothing)
    }
    /// Constructs Python's five-required-field `Insn` form with every later
    /// field at its dataclass default.
    #[must_use]
    pub fn new(
        at: i64,
        covers: Option<(i64, i64)>,
        what: Option<Semantics>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Self {
        Self {
            at,
            covers,
            what,
            defines,
            uses,
            clobbers: BTreeSet::new(),
            clobbers_high: BTreeSet::new(),
            spread: Vec::new(),
            call: None,
            group: None,
            requires: Vec::new(),
            delivers: Vec::new(),
            widths: Vec::new(),
            symbol: None,
            spill_reload: false,
            spill_store: false,
            frame_adjust: false,
            rematerialized: false,
            volatile: false,
            reads_complete: false,
            line: None,
            debug: DebugTags::default(),
            effect: Derived::default(),
        }
    }

    /// Whether it was inserted beside a source instruction: it covers no bytes.
    #[must_use]
    pub fn inserted(&self) -> bool {
        self.covers.is_some_and(|(start, end)| start == end)
    }

    /// Whether its access must happen exactly as written.
    #[must_use]
    pub fn volatile(&self) -> bool {
        self.volatile
    }

    /// Whether it reads no register beyond its `requires` and the epilogue's.
    #[must_use]
    pub fn reads_complete(&self) -> bool {
        self.reads_complete
    }

    /// Whether nothing may move across it or merge with it.
    #[must_use]
    pub fn barrier(&self) -> bool {
        self.volatile
    }

    /// Whether it may write memory its operands do not name: a call or
    /// barrier with no `call` to list what it touches.
    #[must_use]
    pub fn unmodeled_write(&self) -> bool {
        self.call.is_none() && self.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Call | Operation::Barrier))
    }

    /// Whether this instruction puts bytes out, which its machine form
    /// answers and its kind does not: select emits a named NOTHING (`nop`,
    /// `fwait`) and only an unnamed one as no bytes.
    ///
    /// A phi's copy placed after allocation rides on a NOTHING op. Asked by
    /// kind, deedlines' `IF ... THEN rc% = -1` read as an empty block, the jump
    /// over it went, and the copy ran on both paths.
    #[must_use]
    pub fn emits(&self) -> bool {
        match &self.what {
            Some(what) => what.op != Operation::Nothing || what.name.as_deref().is_some_and(|name| !name.is_empty()),
            // A call selected with its effects listed, not an inserted copy.
            None => self.call.is_some() && !self.inserted(),
        }
    }

    /// LLVM's `isMetaInstruction`: emits nothing and names no value or
    /// register, so it only marks where source bytes land. Code must not
    /// depend on one: no heuristic counts it.
    #[must_use]
    pub fn is_meta(&self) -> bool {
        !self.emits()
            && self.defines.is_empty()
            && self.uses.is_empty()
            && self.clobbers.is_empty()
            && self.clobbers_high.is_empty()
            && self.requires.is_empty()
            && self.delivers.is_empty()
    }

    /// The source ranges this instruction owns.
    #[must_use]
    pub fn owned(&self) -> Vec<(i64, i64)> {
        if self.spread.is_empty() { self.covers.into_iter().filter(|(lo, hi)| lo < hi).collect() } else { self.spread.clone() }
    }

}

/// One value that is two definitions above this block, by id.
///
/// Direct port of `qbopt.model.lir:Phi`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Phi {
    pub result: u32,
    pub incoming: Vec<(i64, u32)>,
}

impl Repr for Phi {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Phi",
            &[("result", self.result.repr()), ("incoming", pyrepr::tuple(&self.incoming))],
        )
    }
}

/// One LIR CFG block.
///
/// Direct port of `qbopt.model.lir:LirBlock`.
#[derive(Debug, Eq, PartialEq)]
pub struct LirBlock {
    pub at: i64,
    pub insns: Insns,
    pub succ: Vec<i64>,
    pub phis: Vec<Phi>,
    /// Laid out after the hot code: every path from it ends in `unreachable`, isel finds.
    pub cold: bool,
}

/// A block's instructions, shared: a copy of the block is the same instructions, and two blocks are the same instructions when
/// they are the same allocation (`same_as`), which is how the facts of a block are kept for every body that has it.
#[derive(Clone, Debug, Default)]
pub struct Insns(Arc<[Arc<Insn>]>);

impl Insns {
    /// `change` made to a copy of the instructions, which then are these: the way a test or a one-off edit changes a block.
    pub fn edit<R>(&mut self, change: impl FnOnce(&mut Vec<Arc<Insn>>) -> R) -> R {
        let mut insns = self.0.to_vec();
        let out = change(&mut insns);
        *self = insns.into();
        out
    }

    pub fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for Insns {
    type Target = [Arc<Insn>];
    fn deref(&self) -> &[Arc<Insn>] {
        &self.0
    }
}

impl From<Vec<Arc<Insn>>> for Insns {
    fn from(insns: Vec<Arc<Insn>>) -> Self {
        Self(insns.into())
    }
}

impl PartialEq for Insns {
    fn eq(&self, other: &Self) -> bool {
        self.same_as(other) || *self.0 == *other.0
    }
}

impl Eq for Insns {}

impl PartialEq<Vec<Arc<Insn>>> for Insns {
    fn eq(&self, other: &Vec<Arc<Insn>>) -> bool {
        *self.0 == **other
    }
}

impl<const N: usize> PartialEq<[Arc<Insn>; N]> for Insns {
    fn eq(&self, other: &[Arc<Insn>; N]) -> bool {
        *self.0 == *other
    }
}

impl FromIterator<Arc<Insn>> for Insns {
    fn from_iter<I: IntoIterator<Item = Arc<Insn>>>(insns: I) -> Self {
        Self(insns.into_iter().collect())
    }
}

impl<'a> IntoIterator for &'a Insns {
    type Item = &'a Arc<Insn>;
    type IntoIter = std::slice::Iter<'a, Arc<Insn>>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
thread_local! {
    /// The blocks this thread has cloned: what a pass that copies the body per change shows (#560, #913).
    pub static BLOCK_CLONES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl Clone for LirBlock {
    fn clone(&self) -> Self {
        #[cfg(test)]
        BLOCK_CLONES.with(|count| count.set(count.get() + 1));
        Self { at: self.at, insns: self.insns.clone(), succ: self.succ.clone(), phis: self.phis.clone(), cold: self.cold }
    }
}

impl LirBlock {
    #[must_use]
    pub fn new(at: i64, insns: Vec<Arc<Insn>>) -> Self {
        Self {
            at,
            insns: insns.into(),
            succ: Vec::new(),
            phis: Vec::new(),
            cold: false,
        }
    }

    /// Python's `replace(block, insns=insns)`: the old insns are never copied.
    #[must_use]
    pub fn with_insns(&self, insns: Vec<Arc<Insn>>) -> Self {
        Self { at: self.at, insns: insns.into(), succ: self.succ.clone(), phis: self.phis.clone(), cold: self.cold }
    }

    /// Python `LirBlock.arrives`.
    #[must_use]
    pub fn arrives(&self) -> Vec<u32> {
        self.phis.iter().map(|one| one.result).collect()
    }
}

/// `-g`: a source variable or parameter, and where it lives: a frame slot
/// until a frame rewrite moves it with the operands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugVariable {
    pub name: String,
    pub r#type: llrm_mir::MetadataId,
    pub place: DebugPlace,
    /// Passed in, not declared in the body.
    pub parameter: bool,
    /// Of a parameter's home: the argument it was passed as.
    pub argument: Option<i64>,
    /// The register that argument arrives in, where one does: it holds the value until the function stores it
    /// into the home.
    pub arrives: Option<iced_x86::Register>,
}

/// Where a `-g` variable is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebugPlace {
    /// A frame cell, or a place in data.
    At(Addr),
    /// The register a parameter arrives in, which holds it until the body starts.
    Register(iced_x86::Register),
    /// A parameter the optimiser removed: there is none to show.
    Gone,
    /// A variable the code keeps in no one place: where it is over the code is found from the notes (`LirBody::notes`) that
    /// name it, the variable's metadata node being the number.
    Tracked(u32),
}

/// What `-g` says of a variable at a point in the code: the value it has from there on, or that it has none.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugNote {
    /// The variable's metadata node.
    pub variable: u32,
    /// The bytes of the variable the value is, from the byte (`DW_OP_piece`); none where it is all of it.
    pub piece: Option<(u32, u32)>,
    pub value: NoteValue,
}

/// What a note says a variable (or a piece of it) is.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NoteValue {
    /// Nothing: its value was deleted.
    Nothing,
    /// The value, by the number its register had in SSA.
    Value(u32),
    /// A constant.
    Constant(i64),
}

/// How often each block runs per call, by `at`.
#[derive(Clone, Debug, Default)]
pub struct BlockFrequencies(pub IndexMap<i64, f64>);

impl PartialEq for BlockFrequencies {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|(at, runs)| other.0.get(at).is_some_and(|more| more.to_bits() == runs.to_bits()))
    }
}

impl Eq for BlockFrequencies {}

/// One lowered procedure.  Blocks remain in emitted order.
///
/// Direct port of `qbopt.model.lir:LirBody`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LirBody {
    pub name: String,
    pub entry: i64,
    pub blocks: Vec<LirBlock>,
    pub origin: IndexMap<u32, iced_x86::Register>,
    pub pins: IndexMap<u32, iced_x86::Register>,
    pub inputs: BTreeSet<u32>,
    pub loop_trip_counts: Vec<(i64, i64)>,
    pub ordered: bool,
    pub noreturn: bool,
    /// Blocks keep the order they arrive in. A BASIC body with an error
    /// handler says so: RESUME NEXT finds the statement after the faulting
    /// address, which holds only while each statement's code is contiguous.
    pub source_order: bool,
    /// Only an instruction that names them writes the incoming argument
    /// slots: the IR this came from cannot address them.
    pub sealed_arguments: bool,
    /// `-g`'s parameters, in order, then variables.
    pub variables: Vec<DebugVariable>,
    /// Estimated branch probabilities, by edge, as isel found them; an
    /// edge made since has none.
    pub odds: BlockOdds,
    /// How often each block runs per call, worked out once where the loops are plain (instruction selection) and kept
    /// through every rewrite: a block made since is derived from its predecessors' (`Frequency::of`), one removed drops out.
    /// The estimate is a fact about the program, not about the shape of whatever blocks the allocator has put on its edges.
    pub frequencies: Option<Arc<BlockFrequencies>>,
    /// It calls a routine that returns twice (`setjmp`): no frame slot is shared.
    pub returns_twice: bool,
    /// (load, write) by `at`: the optimizer proved the write leaves the load's cell as it was (`!llrm.spares`).
    pub spares: Arc<BTreeSet<(i64, i64)>>,
    /// How many floating values the target holds on its register stack at once.
    pub float_stack: usize,
    /// A phi's value, by number, and the fixed cell the program also holds it in wherever it is live (`!llrm.home`).
    pub homes: Arc<std::collections::BTreeMap<u32, crate::model::ir::Mem>>,
    /// 16 or 32: the mode the target's code runs in, which decides how an instruction encodes and what it touches.
    pub bits: u32,
    /// Every frame cell names the slot it lies in (`Addr::slot_home`): set once instruction selection has tagged them, and
    /// then a rule of the verifier.
    pub slotted: bool,
    /// `-g`'s variables are found from the canonical frame address, so they need no frame register: the debug format says
    /// where a cell is by its distance from the caller's frame (`FrameBase::Cfa`), whichever register the code addresses it by.
    pub cfa_variables: bool,
    /// What `-g` says of its variables at points in the code, by the numbers `DebugTags::before` holds.
    pub notes: Arc<Vec<DebugNote>>,
    /// The values (by number) that arrive in a cell above the frame, with the cell's displacement and size: arguments the
    /// caller pushed.
    pub arguments_in_cells: Arc<Vec<(u32, i64, u32)>>,
    /// The facts worked out of this body and the bodies made from it (`analysis::facts`).
    pub facts: crate::analysis::facts::Kept,
}

/// Fixed point, in 2^31sts, so a body stays `Eq`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockOdds {
    pub taken: IndexMap<(i64, i64), u32>,
}

impl BlockOdds {
    pub const CERTAIN: f64 = 2147483648.0;

    /// `from`'s edge to `to`'s probability, if isel estimated it.
    pub fn probability(&self, from: i64, to: i64) -> Option<f64> {
        self.taken.get(&(from, to)).map(|one| f64::from(*one) / Self::CERTAIN)
    }

    /// `from`'s edge to `to` among its successors `succ`: as isel stated it,
    /// or an even part of what its stated edges leave the unstated ones.
    pub fn chance(&self, from: i64, succ: &[i64], to: i64) -> f64 {
        let succ: BTreeSet<i64> = succ.iter().copied().collect();
        if !succ.contains(&to) {
            return 0.0;
        }
        if succ.len() == 1 {
            return 1.0;
        }
        if let Some(stated) = self.probability(from, to) {
            return stated;
        }
        let known: f64 = succ.iter().filter_map(|one| self.probability(from, *one)).sum();
        let unknown = succ.iter().filter(|one| self.probability(from, **one).is_none()).count();
        (1.0 - known).max(0.0) / unknown as f64
    }

    /// `from`'s edge to `old`, among its successors `succ`, replaced by edges
    /// to `into`, each at `old`'s chance times its share. Every edge `from`
    /// had is stated first, implicit ones included, so each reads the same
    /// whatever its successors become; the old edge stays recorded so that
    /// undoing a split finds it.
    pub fn rerouted(&mut self, from: i64, succ: &[i64], old: i64, into: &[(i64, f64)]) {
        let before: Vec<(i64, f64)> = succ.iter().map(|to| (*to, self.chance(from, succ, *to))).collect();
        let through = self.chance(from, succ, old);
        let fixed = |probability: f64| (probability * Self::CERTAIN).round().min(f64::from(u32::MAX)) as u32;
        for (to, probability) in &before {
            self.taken.insert((from, *to), fixed(*probability));
        }
        // Only a successor's edge adds to what an edge brings: an entry kept
        // for an edge `from` no longer has is no part of it.
        let had = |to: i64| before.iter().find(|(one, _)| *one == to && to != old).map_or(0.0, |(_, probability)| *probability);
        for (to, share) in into {
            self.taken.insert((from, *to), fixed(had(*to) + through * share));
        }
    }
}

impl LirBody {
    /// The edges from a block with more than one successor into a block with more than one predecessor: code
    /// that must run on one of them has no block of its own to go in until one is made.
    #[must_use]
    pub fn critical_edges(&self) -> BTreeSet<(i64, i64)> {
        let mut preds: IndexMap<i64, usize> = IndexMap::default();
        for block in &self.blocks {
            for to in &block.succ {
                *preds.entry(*to).or_default() += 1;
            }
        }
        self.blocks
            .iter()
            .filter(|block| block.succ.len() > 1)
            .flat_map(|block| block.succ.iter().filter(|to| preds.get(*to).is_some_and(|count| *count > 1)).map(move |to| (block.at, *to)))
            .collect()
    }

    /// Constructs Python's five-required-field `LirBody` form.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        entry: i64,
        blocks: Vec<LirBlock>,
        origin: IndexMap<u32, iced_x86::Register>,
        pins: IndexMap<u32, iced_x86::Register>,
    ) -> Self {
        Self {
            name: name.into(),
            entry,
            blocks,
            origin,
            pins,
            inputs: BTreeSet::new(),
            loop_trip_counts: Vec::new(),
            ordered: false,
            noreturn: false,
            source_order: false,
            sealed_arguments: false,
            variables: Vec::new(),
            odds: BlockOdds::default(),
            frequencies: None,
            returns_twice: false,
            spares: Arc::default(),
            float_stack: 0,
            homes: Arc::default(),
            bits: crate::frontends::bc::declen::BITNESS,
            slotted: false,
            cfa_variables: false,
            notes: Arc::default(),
            arguments_in_cells: Arc::default(),
            facts: Default::default(),
        }
    }

    /// Python's `replace(body, blocks=blocks)`: the old blocks are never copied.
    #[must_use]
    pub fn with_blocks(&self, blocks: Vec<LirBlock>) -> Self {
        let frequencies = self.frequencies.as_ref().map(|kept| {
            // A block removed leaves the table with it.
            let present: BTreeSet<i64> = blocks.iter().map(|one| one.at).collect();
            if kept.0.keys().all(|at| present.contains(at)) {
                Arc::clone(kept)
            } else {
                Arc::new(BlockFrequencies(kept.0.iter().filter(|(at, _)| present.contains(at)).map(|(at, runs)| (*at, *runs)).collect()))
            }
        });
        Self {
            name: self.name.clone(),
            entry: self.entry,
            blocks,
            origin: self.origin.clone(),
            pins: self.pins.clone(),
            inputs: self.inputs.clone(),
            loop_trip_counts: self.loop_trip_counts.clone(),
            ordered: self.ordered,
            noreturn: self.noreturn,
            source_order: self.source_order,
            sealed_arguments: self.sealed_arguments,
            variables: self.variables.clone(),
            odds: self.odds.clone(),
            frequencies,
            returns_twice: self.returns_twice,
            spares: Arc::clone(&self.spares),
            float_stack: self.float_stack,
            homes: Arc::clone(&self.homes),
            bits: self.bits,
            slotted: self.slotted,
            cfa_variables: self.cfa_variables,
            notes: Arc::clone(&self.notes),
            arguments_in_cells: Arc::clone(&self.arguments_in_cells),
            facts: self.facts.clone(),
        }
    }

    /// Python `LirBody.insns`.
    #[must_use]
    pub fn insns(&self) -> Vec<Arc<Insn>> {
        self.blocks
            .iter()
            .flat_map(|block| block.insns.iter().cloned())
            .collect()
    }

    /// Every source byte an instruction owns, once per owner, in order. A
    /// machine phase may move bytes between instructions but neither lose
    /// nor duplicate one.
    #[must_use]
    pub fn owned_bytes(&self) -> Vec<i64> {
        let mut bytes: Vec<i64> = self
            .blocks
            .iter()
            .flat_map(|block| &block.insns)
            .flat_map(|one| one.owned())
            .flat_map(|(lo, hi)| lo..hi)
            .collect();
        bytes.sort_unstable();
        bytes
    }
}

/// What an instruction that only owns source bytes does: nothing, and
/// selects to no bytes. Named `nop`, select emits a real one.
#[must_use]
pub fn inert() -> Semantics {
    Semantics { name: Some(String::new()), ..Semantics::new(Operation::Nothing) }
}

/// Keep virtual dataflow and source-byte ownership for an elided machine op.
///
/// Direct port of `qbopt.model.lir:anchor`.
#[must_use]
pub fn anchor(one: Arc<Insn>) -> Arc<Insn> {
    let mut anchored = (*one).clone();
    anchored.what = Some(inert());
    anchored.clobbers.clear();
    anchored.clobbers_high.clear();
    anchored.group = None;
    anchored.requires.clear();
    anchored.delivers.clear();
    anchored.symbol = Some(false);
    anchored.spill_reload = false;
    anchored.spill_store = false;
    anchored.frame_adjust = false;
    anchored.rematerialized = false;
    Arc::new(anchored)
}

/// Only the source bytes `one` owns: no machine work and no dataflow.
#[must_use]
pub fn bytes_only(one: &Arc<Insn>) -> Arc<Insn> {
    let inert = one.what.as_ref().is_some_and(|what| what.op == Operation::Nothing && what.name.as_deref().unwrap_or("").is_empty());
    if inert && one.defines.is_empty() && one.uses.is_empty() {
        return Arc::clone(one);
    }
    let mut kept = (*anchor(Arc::clone(one))).clone();
    kept.defines.clear();
    kept.uses.clear();
    Arc::new(kept)
}

/// `insns` without the ones `drop` picks, their bytes given to a survivor.
///
/// Direct port of `qbopt.model.lir:without`.  The retained operation is the
/// rewritten occurrence, while ownership decisions are deliberately made
/// against the original occurrence, matching Python's two-phase loop.
#[must_use]
pub fn without<F, R>(insns: &[Arc<Insn>], drop: F, rewrite: Option<R>) -> Vec<Arc<Insn>>
where
    F: Fn(&Arc<Insn>) -> bool,
    R: Fn(&Arc<Insn>) -> Arc<Insn>,
{
    let mut out = Vec::new();
    // Whether `out` starts with a dropped instruction's anchor.
    let mut first_dropped = false;
    for one in insns {
        let kept = rewrite
            .as_ref()
            .map_or_else(|| Arc::clone(one), |rewrite| rewrite(one));
        if !drop(&kept) {
            out.push(kept);
            continue;
        }
        // Bytes no survivor can take stay on an anchor, never on the code.
        let anchored = bytes_only(&kept);
        let Some((start, end)) = one.covers else {
            continue;
        };
        if start == end {
            continue;
        }
        if one.spread.len() > 1 {
            first_dropped |= out.is_empty();
            out.push(anchored);
            continue;
        }
        let where_ = out.iter().rposition(|previous| {
            previous
                .covers
                .is_some_and(|(previous_start, previous_end)| previous_start != previous_end)
        });
        let Some(where_) = where_ else {
            first_dropped |= out.is_empty();
            out.push(anchored);
            continue;
        };
        if out[where_]
            .covers
            .is_none_or(|(_, previous_end)| previous_end != start)
        {
            out.push(anchored);
            continue;
        }
        let mut replacement = (*out[where_]).clone();
        replacement.covers = Some((replacement.covers.expect("checked above").0, end));
        out[where_] = Arc::new(replacement);
    }
    if out.len() > 1 {
        let following = out.iter().enumerate().skip(1).find_map(|(index, one)| {
            one.covers
                .is_some_and(|(start, end)| start < end)
                .then_some(index)
        });
        let first = Arc::clone(&out[0]);
        let second = following.map(|index| Arc::clone(&out[index]));
        if first_dropped
            && first.covers.is_some()
            && second.is_some()
            && first.spread.len() <= 1
            && second.as_ref().is_some_and(|second| second.spread.len() <= 1)
            && first.covers.map(|covers| covers.1) == second.as_ref().and_then(|second| second.covers).map(|covers| covers.0)
        {
            let following = following.expect("second is not None");
            let mut replacement = (*out[following]).clone();
            replacement.covers = Some((first.covers.expect("checked").0, out[following].covers.expect("checked").1));
            out[following] = Arc::new(replacement);
            out.remove(0);
        }
    }
    out
}

#[cfg(test)]
mod tests {

    /// A copy of a body copied every block's instruction list (a block clone was 2.5% of compiling d_faces), and no block could say it
    /// was the one a fact had been made of. A copy shares them; a rewritten block is the only one that is not the same.
    #[test]
    fn test_a_copy_of_a_body_shares_its_blocks_instructions_and_a_rewrite_shares_the_rest() {
        let one = Arc::new(Insn::new(1, None, None, Vec::new(), Vec::new()));
        let body = super::LirBody::new("f", 1, vec![super::LirBlock::new(1, vec![Arc::clone(&one)]), super::LirBlock::new(2, vec![one])], Default::default(), Default::default());
        let copy = body.clone();
        assert!(body.blocks.iter().zip(&copy.blocks).all(|(a, b)| a.insns.same_as(&b.insns)), "a copy made its own instruction lists");
        let rewritten = body.with_blocks(vec![body.blocks[0].with_insns(Vec::new()), body.blocks[1].clone()]);
        assert!(!rewritten.blocks[0].insns.same_as(&body.blocks[0].insns) && rewritten.blocks[1].insns.same_as(&body.blocks[1].insns));
        assert!(body.blocks[0].insns == body.blocks[0].insns.to_vec(), "same instructions compare equal as lists");
    }
    use std::sync::Arc;

    use super::{Insn, anchor, without};

    use crate::model::ir::{Held, Operation, Semantics};

    fn instruction(at: i64, covers: Option<(i64, i64)>) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            covers,
            Some(Semantics::new(Operation::Move)),
            Vec::new(),
            Vec::new(),
        ))
    }

    #[test]
    fn direct_lir_model_anchor_keeps_virtual_definitions_and_source_ownership() {
        // Port of tests/test_peephole.py::test_fusion_preserves_a_virtual_dataflow_anchor:
        // folding physical code must retain both virtual definitions.
        let mut source = (*instruction(0x10, Some((0x10, 0x12)))).clone();
        source.defines = vec![2];
        source.uses = vec![1];
        source.group = Some(7);
        source.requires = vec![(
            Held { value: 1, width: 2 },
            iced_x86::Register::DL,
        )];
        source.spill_reload = true;
        let source = Arc::new(source);
        let anchored = anchor(Arc::clone(&source));
        assert!(!Arc::ptr_eq(&anchored, &source));
        assert_eq!(anchored.defines, vec![2]);
        assert_eq!(anchored.uses, vec![1]);
        assert_eq!(anchored.covers, Some((0x10, 0x12)));
        assert_eq!(anchored.what.as_ref().unwrap().op, Operation::Nothing);
        assert!(anchored.clobbers.is_empty());
        assert!(anchored.requires.is_empty());
        assert_eq!(anchored.symbol, Some(false));
        assert!(!anchored.spill_reload);
    }

    #[test]
    fn direct_lir_model_without_gives_removed_bytes_to_the_previous_owner() {
        // Port of qbopt.model.lir:without: a removed identity copy may hand
        // its exact contiguous bytes backwards, never to a later entry.
        let prior = instruction(0x10, Some((0x10, 0x12)));
        let removed = instruction(0x12, Some((0x12, 0x14)));
        let kept = without(
            &[prior, removed],
            |one| one.at == 0x12,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].covers, Some((0x10, 0x14)));
    }

    #[test]
    fn direct_lir_model_without_transfers_the_first_owner_to_its_successor() {
        // Port of qbopt.model.lir:without's final first-instruction case:
        // only a block's first source span can safely move forward.
        let removed = instruction(0x10, Some((0x10, 0x12)));
        let following = instruction(0x12, Some((0x12, 0x14)));
        let kept = without(
            &[removed, following],
            |one| one.at == 0x10,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].covers, Some((0x10, 0x14)));
    }

    #[test]
    fn direct_lir_model_without_retains_noncontiguous_or_spread_source_owners() {
        // Port of qbopt.model.lir:without byte-ownership refusals: no valid
        // predecessor or multiple disjoint spans means the copy stays.
        let detached = instruction(0x20, Some((0x20, 0x22)));
        let next = instruction(0x30, Some((0x30, 0x32)));
        let kept = without(
            &[Arc::clone(&detached), next],
            |one| one.at == 0x20,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].covers, detached.covers);

        let prior = instruction(0x10, Some((0x10, 0x12)));
        let mut spread = (*instruction(0x12, Some((0x12, 0x14)))).clone();
        spread.spread = vec![(0x12, 0x14), (0x20, 0x22)];
        let spread = Arc::new(spread);
        let kept = without(
            &[prior, spread],
            |one| one.at == 0x12,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[1].covers, Some((0x12, 0x14)));
    }

    #[test]
    fn direct_lir_model_without_preserves_python_instruction_identity() {
        // Passes key sets on an instruction's identity, so a kept one is the
        // same object. A dropped one whose bytes no survivor can take becomes
        // an anchor: bytes never keep code alive.
        let kept = instruction(0x08, Some((0x08, 0x0a)));
        let recipient = instruction(0x10, Some((0x10, 0x12)));
        let donor = instruction(0x12, Some((0x12, 0x14)));
        let result = without(
            &[Arc::clone(&kept), Arc::clone(&recipient), donor],
            |one| one.at == 0x12,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert!(Arc::ptr_eq(&result[0], &kept));
        assert!(!Arc::ptr_eq(&result[1], &recipient));
        assert_eq!(result[1].covers, Some((0x10, 0x14)));

        let mut refused = (*instruction(0x20, Some((0x20, 0x22)))).clone();
        refused.spread = vec![(0x20, 0x22), (0x30, 0x32)];
        let refused = Arc::new(refused);
        let result = without(
            &[Arc::clone(&kept), Arc::clone(&refused)],
            |one| one.at == 0x20,
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
        assert!(Arc::ptr_eq(&result[0], &kept));
        assert!(result[1].is_meta() && result[1].spread == refused.spread, "{:?}", result[1]);
    }

    #[test]
    fn stage_dump_reprs_match_python() {
        // Expected strings printed by compile._lir_text's pieces in Python.
        use crate::model::ir::Loc;
        use crate::support::pyrepr::{self, Repr};
        use iced_x86::Register;

        let phi = |incoming: Vec<(i64, u32)>| super::Phi { result: 3, incoming }.repr();
        assert_eq!(phi(vec![(1, 2), (4, 5)]), "Phi(result=3, incoming=((1, 2), (4, 5)))");
        assert_eq!(phi(vec![(1, 2)]), "Phi(result=3, incoming=((1, 2),))");
        assert_eq!(phi(vec![]), "Phi(result=3, incoming=())");

        let mut one = Insn::new(
            16,
            Some((16, 18)),
            Some(Semantics {
                name: Some("call".to_owned()),
                sources: vec![Loc::Held(Held { value: 1, width: 2 })],
                target: Some(5),
                ..Semantics::new(Operation::Call)
            }),
            Vec::new(),
            Vec::new(),
        );
        one.requires = vec![
            (Held { value: 1, width: 2 }, Register::CX),
            (Held { value: 7, width: 4 }, Register::EBX),
        ];
        one.delivers = vec![(Held { value: 9, width: 1 }, Register::AL)];
        let line = |one: &Insn| {
            format!(
                "  {:4} {} req={} del={}",
                one.at,
                one.what.repr(),
                pyrepr::tuple(&one.requires),
                pyrepr::tuple(&one.delivers)
            )
        };
        assert_eq!(
            line(&one),
            "    16 Semantics(op=<Operation.CALL: 'call'>, name='call', dests=(), \
             sources=(Held(value=1, width=2),), target=5, indirect=False) \
             req=((Held(value=1, width=2), 22), (Held(value=7, width=4), 40)) \
             del=((Held(value=9, width=1), 1),)"
        );
        assert_eq!(line(&Insn::new(3, None, None, Vec::new(), Vec::new())), "     3 None req=() del=()");
        assert_eq!(pyrepr::tuple::<i64>(&[2]), "(2,)");
        assert_eq!(pyrepr::tuple::<i64>(&[2, 3]), "(2, 3)");
    }
}
