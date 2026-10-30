//! The driver of the translated greedy allocator: it builds the inputs
//! `RAGreedy` reads (intervals, spill weights, the matrix) from the caller's
//! body and runs it. It is the allocator `backend::allocate` runs on every
//! function (`Mode::Inline`, spills written by `InlineSpiller`); the other
//! modes only count (`Record`, `Simple`). The name is from when it ran
//! beside the old allocator.
//!
//! # How it works
//! `run` numbers the body, computes its live intervals and spill weights, puts
//! the pinned values in the matrix where their register fixes them, and drives
//! `RAGreedy::allocate_phys_regs`. The registers a value may take are the
//! caller's allocation order mapped to whole registers (this allocator's
//! `_whole`: a value holds all the byte lanes of its root); a call destroys
//! the registers its instruction names, seen as `IK_RegMask`.
//!
//! DEVIATE (no number yet): a value holds its whole root register (the file's
//! sub-register widths are not modelled in the candidates); fixed ranges other
//! than pins and calls (segment loads, string instructions) are not fixed
//! intervals here; rematerializable definitions are not recognised.
//!
//! # Example
//! `LLRM_GREEDY_TRACE=1 llrm-cc -O2 -c f.c` prints one line per allocation
//! with the counts.
//!
//! # Used by
//! `backend::allocate::greedy_rewritten`.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegSet;
use llrm_lir::registers::{RegId, Regs};
use llrm_support::hash::HashMap;

use super::greedy::{GreedyEnv, RAGreedy, Stats};
use super::inlinespiller::{InlineSpiller, Tables};
use super::matrix::{LiveRegMatrix, PhysInterference};
use super::spill::SimpleSpiller;
use super::virtregmap::RegisterInfo;
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::Frame;
use crate::liveness::calcspillweights::SpillWeightEnv;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::LiveInterval;
use crate::liveness::slotindex::SlotIndex;
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::IndexMap;

/// What the caller gives the shadow allocator.
pub struct ShadowInputs<'a> {
    pub body: &'a LirBody,
    pub registers: Regs,
    /// The registers a value may be placed in, most preferred first.
    pub order: &'a dyn Fn(u32) -> Vec<RegId>,
    pub pins: &'a IndexMap<u32, RegId>,
    pub unspillable: &'a BTreeSet<u32>,
    /// The register every unprefixed access reads, which an instruction that
    /// needs the data group reads at once (`target::needs_data_group`).
    pub data: Option<RegId>,
    /// The sets an operand confines a value to: a register made by a split
    /// takes the class its own instructions give (`recomputeRegClass`),
    /// else its parent's.
    pub sets: Option<&'a crate::backend::regclass::Sets>,
    /// `Profile::stable_loads`: rematerialize loads of cells nothing writes.
    pub stable_loads: bool,
    /// The callee-saved registers, whole (`getLastCalleeSavedAlias`): none
    /// turns the first-use cost of one off.
    pub saved: &'a [RegId],
}

/// What the shadow allocator decided.
#[derive(Clone, Debug, Default)]
pub struct ShadowReport {
    pub values: usize,
    pub stats: Stats,
    pub reloads: usize,
    pub stores: usize,
    pub folded: usize,
    pub signature: u64,
}

/// The body's analyses the allocator asks of.
struct BodyEnv {
    orders: Rc<Tables>,
    /// The register each pinned value is fixed to, as a root: the physical
    /// hint of a copy from it (`copyHint`).
    pins: IndexMap<u32, RegId>,
    freq: Vec<f32>,
    exiting: Vec<bool>,
    saved: Vec<RegId>,
}

impl SpillWeightEnv for BodyEnv {
    fn block_freq_relative_to_entry(
        &self,
        block: usize,
    ) -> f32 {
        self.freq[block]
    }

    fn is_loop_exiting(
        &self,
        block: usize,
    ) -> bool {
        self.exiting[block]
    }

    fn copy_of(
        &self,
        instr: &Insn,
    ) -> Option<(u32, u32)> {
        instr.copy_of()
    }

    fn pinned(
        &self,
        reg: u32,
    ) -> Option<RegId> {
        self.pins.get(&reg).copied()
    }

    fn is_rematerializable(
        &self,
        instr: &Insn,
    ) -> bool {
        super::inlinespiller::remat_shape(instr) && self.orders.remat.borrow().contains(&instr.defines[0])
    }

    fn reg_mask_slots(&self) -> Vec<SlotIndex> {
        Vec::new()
    }

    fn spill_weight_scale(
        &self,
        _reg: u32,
    ) -> f32 {
        1.0
    }

    fn is_allocatable(
        &self,
        _reg: RegId,
    ) -> bool {
        true
    }
}

impl GreedyEnv for BodyEnv {
    fn has_class(
        &self,
        virt: u32,
    ) -> bool {
        self.orders.orders.borrow().contains_key(&virt)
    }

    fn allocation_order(
        &self,
        virt: u32,
    ) -> Vec<RegId> {
        self.orders.orders.borrow().get(&virt).cloned().unwrap_or_default()
    }

    fn num_allocatable_regs(
        &self,
        virt: u32,
    ) -> usize {
        self.orders.orders.borrow().get(&virt).map_or(0, Vec::len)
    }

    fn copy_cost(
        &self,
        _virt: u32,
    ) -> u32 {
        1
    }

    fn same_reg_class(
        &self,
        a: u32,
        b: u32,
    ) -> bool {
        let orders = self.orders.orders.borrow();
        orders.get(&a) == orders.get(&b)
    }

    fn has_tied_def(
        &self,
        _virt: u32,
    ) -> bool {
        false
    }

    fn callee_saved_alias(
        &self,
        phys: RegId,
    ) -> Option<RegId> {
        self.saved.contains(&phys).then_some(phys)
    }

    fn csr_first_use_cost(&self) -> u32 {
        // push + pop (`X86RegisterInfo::getCSRFirstUseCost`).
        if self.saved.is_empty() { 0 } else { 2 }
    }
}

/// What `SplitKit` asks of the function: its loops, its terminators and the
/// copies it inserts.
struct BodySplitEnv {
    tables: Rc<Tables>,
    forest: Rc<llrm_mir::forest::Forest>,
    /// The address of each block, by number.
    ats: Vec<i64>,
    /// Where each block address stands.
    number: crate::support::hash::HashMap<i64, usize>,
    registers: Regs,
    sets: Option<Rc<crate::backend::regclass::Sets>>,
    /// The values fixed to a register, and the registers a split made of them:
    /// their class is that register.
    pinned: std::cell::RefCell<crate::support::hash::HashSet<u32>>,
}

impl BodySplitEnv {
    /// How wide `from` is named by the instructions that read it: for a value
    /// no table holds (one a spill made).
    fn named_width(
        lis: &LiveIntervals,
        from: u32,
    ) -> Option<u32> {
        lis.operands(from)
            .iter()
            .flat_map(|op| crate::backend::widths::_named_widths(lis.instr(op.instr)).collect::<Vec<_>>())
            .filter(|(value, _)| *value == from)
            .map(|(_, width)| width)
            .max()
    }

    fn new(
        body: &LirBody,
        tables: Rc<Tables>,
        registers: Regs,
        sets: Option<Rc<crate::backend::regclass::Sets>>,
        pinned: crate::support::hash::HashSet<u32>,
    ) -> Self {
        let ats: Vec<i64> = body.blocks.iter().map(|block| block.at).collect();
        let number = ats.iter().enumerate().map(|(n, &at)| (at, n)).collect();
        Self {
            tables,
            forest: crate::analysis::loops::forest_of(&body.blocks, Some(body.entry)),
            ats,
            number,
            registers,
            sets,
            pinned: std::cell::RefCell::new(pinned),
        }
    }
}

/// `MachineRegisterInfo::recomputeRegClass` for the registers an edit made: the
/// order each takes from the instructions that name it, which can be wider
/// than its parent's. A pinned value, one with a class of its own kind (the
/// x87 stack) and one an address pair confines (its side is its parent's)
/// keep theirs.
fn reclass_orders(
    tables: &Tables,
    pinned: &crate::support::hash::HashSet<u32>,
    sets: Option<&crate::backend::regclass::Sets>,
    registers: Regs,
    lis: &LiveIntervals,
    regs: &[u32],
) {
    use crate::backend::regclass::{Confinement, confinement_of};
    let Some(sets) = sets else { return };
    for &reg in regs {
        let Some(current) = tables.orders.borrow().get(&reg).cloned() else { continue };
        if pinned.contains(&reg) || !current.iter().all(|one| sets.available.contains(one)) {
            continue;
        }
        let mut seen = crate::support::hash::HashSet::default();
        let insns: Vec<Arc<Insn>> = lis
            .operands(reg)
            .iter()
            .filter(|op| seen.insert(op.instr))
            .map(|op| Arc::clone(lis.instr(op.instr)))
            .collect();
        let order: Vec<RegId> = match confinement_of(registers, insns.iter().map(|one| &**one), reg, sets) {
            Confinement::Unknown => continue,
            Confinement::Free => sets.available.clone(),
            Confinement::Only(allowed) => {
                let wanted: BTreeSet<RegId> = allowed.iter().map(|one| crate::model::ir::root(*one)).collect();
                sets.available.iter().copied().filter(|one| wanted.contains(one)).collect()
            }
        };
        if !order.is_empty() {
            tables.orders.borrow_mut().insert(reg, order);
        }
    }
}

impl super::splitkit::SplitEnv for BodySplitEnv {
    fn is_terminator(
        &self,
        insn: &Insn,
    ) -> bool {
        use crate::model::ir::Operation;
        insn.what
            .as_ref()
            .is_some_and(|what| matches!(
                what.op,
                Operation::Jump | Operation::Branch | Operation::Return
            ))
    }

    /// `MRI.cloneVirtualRegister` (`LiveRangeEdit::createFrom`): the clone has
    /// the parent's width, which decides whether a call destroys its high half
    /// (`CallMasks`). The parent may be a value a spill made, in no table: its
    /// instructions say how wide it is.
    fn note_split_from(
        &self,
        lis: &LiveIntervals,
        new: u32,
        from: u32,
    ) {
        self.tables.derive_family(new, from);
        let mut widths = self.tables.widths.borrow_mut();
        let width = widths.get(&from).copied().or_else(|| Self::named_width(lis, from));
        if let Some(width) = width {
            widths.insert(new, width);
        }
        let bytes = self.tables.float_bytes.borrow().get(&from).copied();
        if let Some(bytes) = bytes {
            self.tables.float_bytes.borrow_mut().insert(new, bytes);
        }
        let order = self.tables.orders.borrow().get(&from).cloned();
        if let Some(order) = order {
            self.tables.orders.borrow_mut().entry(new).or_insert(order);
        }
        let fixed = self.pinned.borrow().contains(&from);
        if fixed {
            self.pinned.borrow_mut().insert(new);
        }
    }

    /// `LiveRangeEdit::calculateRegClassAndHint`'s `recomputeRegClass`: a
    /// register the split made takes the class its own instructions give,
    /// which can be wider than its parent's.
    fn reclass(
        &self,
        lis: &LiveIntervals,
        regs: &[u32],
    ) {
        reclass_orders(&self.tables, &self.pinned.borrow(), self.sets.as_deref(), self.registers, lis, regs);
    }

    fn is_rematerializable(
        &self,
        reg: u32,
    ) -> bool {
        self.tables.remat.borrow().contains(&reg)
    }

    fn loop_of(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.forest.innermost(self.ats[block])
    }

    fn loop_depth(
        &self,
        l: usize,
    ) -> u32 {
        self.forest.loops[l].depth
    }

    fn loop_header(
        &self,
        l: usize,
    ) -> usize {
        self.number[&self.forest.loops[l].header]
    }

    fn is_loop_latch(
        &self,
        block: usize,
    ) -> bool {
        let at = self.ats[block];
        self.forest.innermost(at).is_some_and(|l| self.forest.loops[l].latches.contains(&at))
    }

    fn is_copy_like(
        &self,
        insn: &Insn,
    ) -> bool {
        insn.copy_of().is_some()
    }

    fn is_pure_def(
        &self,
        insn: &Insn,
    ) -> bool {
        super::inlinespiller::remat_shape(insn) && insn.uses.is_empty()
    }

    /// `def` (a literal, address or load that reads no
    /// register) as a definition of `to`: `rematerializeAt`.
    fn rematerialize(
        &self,
        def: &Insn,
        to: u32,
        parent: u32,
        beside: Option<&Insn>,
    ) -> Option<Arc<Insn>> {
        use crate::model::ir::{Held, Loc, Operation, Semantics};
        if !super::inlinespiller::remat_shape(def) || !def.uses.is_empty() {
            return None;
        }
        let what = def.what.clone()?;
        // `SplitEditor::defFromParent` asks `canRematerializeAt(..,
        // cheapAsAMove = true)`: a literal or an address is made again,
        // a load (`MOV32rm` of a stack argument) or an `lea` is not
        // `isAsCheapAsAMove`, and the split copies the register
        // instead.
        let _ = parent;
        let load = matches!(what.op, Operation::Move | Operation::FloatLoad)
            && matches!(what.sources.first(), Some(Loc::Mem(_)));
        if load || what.op == Operation::Address {
            return None;
        }
        let Loc::Held(defined) = what.dests[0] else { return None };
        self.tables.widths.borrow_mut().insert(to, defined.width);
        self.tables.remat.borrow_mut().insert(to);
        let (at, covers) = beside.map_or((0, None), |b| {
            let start = b.covers.map_or(b.at, |c| c.0);
            (b.at, Some((start, start)))
        });
        Some(Arc::new(Insn {
            at,
            covers,
            defines: vec![to],
            what: Some(Semantics { dests: vec![Loc::Held(Held { value: to, width: defined.width })], ..what }),
            spill_reload: true,
            rematerialized: true,
            ..def.clone()
        }))
    }

    /// A move of `from` into `to`, as wide as the widest access of `from`
    /// (a value a spill made is in no table: its instructions say).
    fn build_copy(
        &self,
        lis: &LiveIntervals,
        to: u32,
        from: u32,
        beside: Option<&Insn>,
    ) -> Arc<Insn> {
        use crate::model::ir::{Held, Loc, Operation, Semantics};
        let known = self.tables.widths.borrow().get(&from).copied();
        let width = known.or_else(|| Self::named_width(lis, from)).unwrap_or(4);
        self.tables.widths.borrow_mut().insert(to, width);
        self.tables.derive_family(to, from);
        let bytes = self.tables.float_bytes.borrow().get(&from).copied();
        if let Some(bytes) = bytes {
            self.tables.float_bytes.borrow_mut().insert(to, bytes);
        }
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: to, width })],
            sources: vec![Loc::Held(Held { value: from, width })],
            ..Semantics::new(Operation::Move)
        };
        // Beside another instruction, claiming none of its bytes.
        let (at, covers) = beside.map_or((0, None), |b| {
            let start = b.covers.map_or(b.at, |c| c.0);
            (b.at, Some((start, start)))
        });
        Arc::new(Insn::new(at, covers, Some(what), vec![to], vec![from]))
    }
}

/// The successors of each block, by position.
fn successors(body: &LirBody) -> Vec<Vec<usize>> {
    let position: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(n, block)| (block.at, n)).collect();
    body.blocks.iter().map(|block| block.succ.iter().filter_map(|to| position.get(to).copied()).collect()).collect()
}

/// The calls: where registers are destroyed.
struct CallMasks {
    /// The reg slot of each instruction that destroys registers, ascending
    /// (`LIS->getRegMaskSlots()`), and what a value live across it loses:
    /// a narrow value, a wide one (the high halves too), and the registers it
    /// reads a value ending at it would lose.
    calls: Vec<(SlotIndex, RegSet, RegSet, RegSet)>,
    /// Whether each of `calls` destroys any register (the rest only read one).
    destroying: Vec<bool>,
    /// Everything any call takes from a narrow and from a wide value (and
    /// every read): a range that has lost it all meets no call that adds.
    ceiling: [RegSet; 2],
    /// Each block's slots, start and end, and the run of `calls` in it
    /// (`getRegMaskSlotsInBlock`), in slot order.
    blocks: Vec<(SlotIndex, SlotIndex, std::ops::Range<usize>)>,
    /// The widest each value is named (bytes): a value wider than 2 loses the
    /// high half of the roots in a call's third list.
    widths: Rc<Tables>,
    /// The blocks' successors (by position), where a call can lose a high half
    /// only if a wide read can follow it: set by `with_flow`.
    flow: Option<Vec<Vec<usize>>>,
    /// Each block's blocks reachable by one edge or more, as bits.
    reach: std::cell::OnceCell<Vec<Vec<u64>>>,
    /// The value a split is tried for (`PhysInterference::set_lane`).
    lane: std::cell::Cell<Option<u32>>,
    /// The registers some call keeps the low half of only.
    lane_registers: RegSet,
}

thread_local! {
    static CALLS_VISITED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// (searches for the regmask slots of a range, runs read for a block) on
    /// this thread.
    static RANGE_SEARCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BLOCK_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// The pins a query for a register walked, on this thread.
    static PINS_EXAMINED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread searched the regmask slots for a block's range,
/// and how many times it read the block's run: for a test that the interference
/// cache and the region splitter do the second.
pub fn regmask_lookups() -> (usize, usize) {
    (RANGE_SEARCHES.with(std::cell::Cell::get), BLOCK_RUNS.with(std::cell::Cell::get))
}

/// The calls `reg_mask_clobbered` has looked at on this thread, for a test that
/// a range with a hole does not look at the calls in it.
pub fn calls_visited() -> usize {
    CALLS_VISITED.with(std::cell::Cell::get)
}

impl CallMasks {
    /// The masks of `calls`, each (slot, destroyed, high halves destroyed,
    /// read): what a narrow and a wide value live across lose is found once.
    fn of(
        calls: Vec<(SlotIndex, RegSet, RegSet, RegSet)>,
        blocks: Vec<(SlotIndex, SlotIndex, std::ops::Range<usize>)>,
        widths: Rc<Tables>,
    ) -> Self {
        let destroying =
            calls.iter().map(|(_, destroyed, high, _)| !destroyed.is_empty() || !high.is_empty()).collect();
        let calls: Vec<(SlotIndex, RegSet, RegSet, RegSet)> = calls
            .into_iter()
            .map(|(slot, destroyed, high, read)| {
                let narrow = destroyed.union(&read);
                (slot, narrow, narrow.union(&high), read)
            })
            .collect();
        let mut lane_registers = RegSet::default();
        for (_, narrow, wide_lost, _) in &calls {
            for root in wide_lost.iter().filter(|root| !narrow.contains(root)) {
                lane_registers.insert(root);
            }
        }
        let ceiling = [false, true].map(|wide| {
            let mut all = RegSet::default();
            for &(_, narrow, lost, read) in &calls {
                all.union_with(&if wide { lost } else { narrow });
                all.union_with(&read);
            }
            all
        });
        Self {
            calls,
            destroying,
            ceiling,
            blocks,
            widths,
            flow: None,
            reach: Default::default(),
            lane: Default::default(),
            lane_registers,
        }
    }

    /// The same, told the blocks' successors.
    fn with_flow(
        mut self,
        succ: Vec<Vec<usize>>,
    ) -> Self {
        self.flow = Some(succ);
        self
    }

    /// Whether block `to` can be reached from block `from` by one edge or more.
    fn reaches(
        &self,
        from: usize,
        to: usize,
    ) -> bool {
        let reach = self
            .reach
            .get_or_init(
                || {
                    let succ = self.flow.as_deref().unwrap_or_default();
                    let words = succ.len().div_ceil(64);
                    (0..succ.len())
                        .map(|start| {
                            let mut seen = vec![0u64; words];
                            let mut work: Vec<usize> = succ[start].clone();
                            while let Some(block) = work.pop() {
                                if seen[block / 64] >> (block % 64) & 1 == 0 {
                                    seen[block / 64] |= 1 << (block % 64);
                                    work.extend(succ[block].iter().copied());
                                }
                            }
                            seen
                        })
                        .collect()
                },
            );
        reach.get(from).is_some_and(|row| row[to / 64] >> (to % 64) & 1 == 1)
    }

    /// Whether a read of more than 2 bytes of a value of `reg`'s family can
    /// follow the call at `slot` (a later one in its block, one in a block it
    /// reaches, or an earlier one round a loop). A value in no family, or a
    /// body without flow, is wide.
    fn wide_read_follows(
        &self,
        reg: u32,
        slot: &SlotIndex,
    ) -> bool {
        let (Some(_), Some(root)) = (&self.flow, self.widths.family.borrow().get(&reg).copied()) else { return true };
        let reads = self.widths.wide_reads.borrow();
        let Some(list) = reads.get(&root) else { return true };
        let Some(call_block) = self.blocks.partition_point(|(begin, ..)| begin <= slot).checked_sub(1) else {
            return true;
        };
        list.iter().any(|(read, block)| {
            if *block == call_block {
                read > slot || self.reaches(call_block, call_block)
            } else {
                self.reaches(call_block, *block)
            }
        })
    }

    /// `LiveIntervals::intervalIsInOneMBB`: the run of calls in the one block
    /// `li` starts and ends in, or none where it spans blocks or starts or
    /// ends at a block boundary.
    fn in_one_block(
        &self,
        li: &LiveInterval,
    ) -> Option<std::ops::Range<usize>> {
        let (start, stop) = (li.begin_index(), li.end_index());
        if start.is_block() || stop.is_block() || self.blocks.is_empty() {
            return None;
        }
        let at = self.blocks.partition_point(|(begin, ..)| *begin <= start).checked_sub(1)?;
        let (_, end, run) = &self.blocks[at];
        (stop.get_prev_slot() < *end).then(|| run.clone())
    }
}

/// LLVM's physical register range for a value an instruction pins to a
/// register (`LIS->getRegUnit`): the value is
/// not allocated; every other range meets it as a fixed range, except where
/// the overlap starts at a copy between the two (`LiveRange::overlaps(Other,
/// CP, Indexes)`, `LiveRegMatrix::checkRegUnitInterference`).
/// `LLRM_PINS_AS_VREGS` allocates pinned values as virtual registers again (the
/// old way, to be deleted).
struct FixedPins {
    tri: RegisterInfo,
    pins: Vec<FixedPin>,
    /// The pinned ranges of each register unit, merged (`LIS->getRegUnit`):
    /// what the interference cache and the region splitter see of them.
    units: crate::support::hash::HashMap<u32, crate::liveness::liverange::LiveRange>,
    /// The copies of the body by the slot they define at: (destination,
    /// source).
    copies: crate::support::hash::HashMap<SlotIndex, (u32, u32)>,
    /// The pins that share a unit with each register asked about, found once.
    overlapping: std::cell::RefCell<crate::support::hash::HashMap<RegId, Rc<[usize]>>>,
}

struct FixedPin {
    reg: u32,
    root: RegId,
    segments: Vec<(SlotIndex, SlotIndex)>,
}

impl FixedPins {
    fn interferes(
        &self,
        li: &LiveInterval,
        phys: RegId,
    ) -> bool {
        let near = {
            let mut known = self.overlapping.borrow_mut();
            Rc::clone(known.entry(phys).or_insert_with(|| {
                (0..self.pins.len()).filter(|&at| self.tri.overlaps(self.pins[at].root, phys)).collect()
            }))
        };
        let (Some(first), Some(last)) = (li.segments.first(), li.segments.last()) else { return false };
        // The merged range of each unit of the register (the pin's
        // `LiveRegMatrix::checkRegUnitInterference`) says at once that no pin
        // meets the range; only a meeting is then told from a copy that makes
        // it, pin by pin.
        let meets = self
            .tri
            .regunits(phys)
            .any(
                |unit| self.units
                    .get(&unit)
                    .is_some_and(
                        |range| li.segments.iter().any(|segment| range.overlaps_interval(&segment.start, &segment.end)),
                    ),
            );
        if !meets {
            return false;
        }
        near.iter().map(|&at| &self.pins[at]).any(|pin| {
            #[cfg(test)]
            PINS_EXAMINED.with(|count| count.set(count.get() + 1));
            // A pin that ends before the range starts, or starts after it
            // ends, meets none of its segments.
            let (Some(from), Some(to)) = (pin.segments.first(), pin.segments.last()) else { return false };
            if to.1 <= first.start || last.end <= from.0 {
                return false;
            }
            let (mut i, mut j) = (0, 0);
            while i < li.segments.len() && j < pin.segments.len() {
                let (a, b) = (&li.segments[i], &pin.segments[j]);
                if a.start < b.1 && b.0 < a.end {
                    let def = std::cmp::max(a.start.clone(), b.0.clone());
                    let excused = !def.is_block()
                        && self
                            .copies
                            .get(&def)
                            .is_some_and(|&(d, s)| (d, s) == (li.reg(), pin.reg) || (d, s) == (pin.reg, li.reg()));
                    if !excused {
                        return true;
                    }
                }
                if a.end <= b.1 {
                    i += 1;
                } else {
                    j += 1;
                }
            }
            false
        })
    }
}

/// The calls and the pinned ranges as the matrix's fixed constraints.
struct Fixed {
    calls: CallMasks,
    pins: Rc<std::cell::OnceCell<FixedPins>>,
}

impl PhysInterference for Fixed {
    fn reg_mask_clobbered(
        &self,
        li: &LiveInterval,
    ) -> Option<RegSet> {
        self.calls.reg_mask_clobbered(li)
    }

    fn reg_unit_interferes(
        &self,
        li: &LiveInterval,
        phys: RegId,
    ) -> bool {
        self.pins.get().is_some_and(|pins| pins.interferes(li, phys))
    }

    fn fixed_range(
        &self,
        unit: u32,
    ) -> Option<&crate::liveness::liverange::LiveRange> {
        self.pins.get().and_then(|pins| pins.units.get(&unit))
    }

    fn reg_mask_range(
        &self,
        start: SlotIndex,
        stop: SlotIndex,
    ) -> std::ops::Range<usize> {
        self.calls.reg_mask_range(start, stop)
    }

    fn reg_mask_range_in_block(
        &self,
        mbb: usize,
        start: SlotIndex,
        stop: SlotIndex,
    ) -> std::ops::Range<usize> {
        self.calls.reg_mask_range_in_block(mbb, start, stop)
    }

    fn reg_mask_destroys(
        &self,
        i: usize,
    ) -> bool {
        self.calls.reg_mask_destroys(i)
    }

    fn reg_mask_slot(
        &self,
        i: usize,
    ) -> SlotIndex {
        self.calls.reg_mask_slot(i)
    }

    fn reg_mask_clobbers(
        &self,
        i: usize,
        phys: RegId,
    ) -> bool {
        self.calls.reg_mask_clobbers(i, phys)
    }

    fn lane_of(
        &self,
        phys: RegId,
    ) -> Option<u32> {
        self.calls.lane_of(phys)
    }

    fn set_lane(
        &self,
        virt: Option<u32>,
    ) {
        self.calls.set_lane(virt);
    }
}

impl PhysInterference for CallMasks {
    /// `LiveIntervals::checkRegMaskInterference`: the slots are ascending, so
    /// the calls the range is live across are found by one merge with its
    /// segments.
    fn reg_mask_clobbered(
        &self,
        li: &LiveInterval,
    ) -> Option<RegSet> {
        if self.calls.is_empty() || li.empty() {
            return None;
        }
        // A range in one block meets only that block's calls
        // (`intervalIsInOneMBB`, LiveIntervals.cpp#L1007).
        let calls = match self.in_one_block(li) {
            Some(run) => &self.calls[run],
            None => &self.calls[..],
        };
        if calls.is_empty() {
            return None;
        }
        let mut found: Option<RegSet> = None;
        // Every allocated value has a width (the table, or the clone it was
        // made from, `note_split_from`); a missing one is the safe side, wide.
        let width = self.widths.widths.borrow().get(&li.reg()).copied();
        let wide = width.is_none_or(|width| width > 2);
        // Per segment, as `checkRegMaskInterference` walks `SlotI`: the calls
        // at or before its start are crossed by a search (a hole looks at
        // none), those inside it are lost across, and one at its end takes
        // what it reads. Once a range has lost all any call takes, no call
        // adds.
        let ceiling = &self.ceiling[usize::from(wide)];
        let mut at = calls.partition_point(|(slot, ..)| *slot < li.begin_index());
        for s in &li.segments {
            at += calls[at..].partition_point(|(slot, ..)| *slot <= s.start);
            while let Some((slot, narrow, wide_lost, _)) = calls.get(at).filter(|(slot, ..)| *slot < s.end) {
                #[cfg(test)]
                CALLS_VISITED.with(|visited| visited.set(visited.get() + 1));
                debug_assert!(s.start < *slot);
                // DEVIATE C-LANE: a call destroys the high half of the pin's
                // whole register; here a value loses it only where a wide
                // read can follow.
                let lost = wide && (narrow == wide_lost || self.wide_read_follows(li.reg(), slot));
                let found = found.get_or_insert_with(RegSet::default);
                found.union_with(if lost { wide_lost } else { narrow });
                if found == ceiling {
                    return Some(*found);
                }
                at += 1;
            }
            match calls.get(at) {
                None => break,
                // An instruction that reads a register it names not (the data
                // segment) also takes it from a range that ends at it.
                Some((slot, _, _, read)) if *slot == s.end && !read.is_empty() => {
                    #[cfg(test)]
                    CALLS_VISITED.with(|visited| visited.set(visited.get() + 1));
                    let found = found.get_or_insert_with(RegSet::default);
                    found.union_with(read);
                    if found == ceiling {
                        return Some(*found);
                    }
                }
                Some(_) => {}
            }
        }
        found
    }

    fn reg_unit_interferes(
        &self,
        _li: &LiveInterval,
        _phys: RegId,
    ) -> bool {
        false
    }

    fn reg_mask_range(
        &self,
        start: SlotIndex,
        stop: SlotIndex,
    ) -> std::ops::Range<usize> {
        #[cfg(test)]
        RANGE_SEARCHES.with(|searches| searches.set(searches.get() + 1));
        self.calls.partition_point(|(slot, ..)| *slot < start)..self.calls.partition_point(|(slot, ..)| *slot < stop)
    }

    /// `LiveIntervals::getRegMaskSlotsInBlock` (LiveIntervals.h#L400): the run
    /// the block was given when the calls were collected.
    fn reg_mask_range_in_block(
        &self,
        mbb: usize,
        _start: SlotIndex,
        _stop: SlotIndex,
    ) -> std::ops::Range<usize> {
        #[cfg(test)]
        BLOCK_RUNS.with(|runs| runs.set(runs.get() + 1));
        self.blocks[mbb].2.clone()
    }

    fn reg_mask_slot(
        &self,
        i: usize,
    ) -> SlotIndex {
        self.calls[i].0
    }

    fn reg_mask_destroys(
        &self,
        i: usize,
    ) -> bool {
        self.destroying[i]
    }

    /// DEVIATE D-IC2: a call's registers are the roots it destroys plus the
    /// high halves a wide value loses; the cache asks per register, not per
    /// value, so both count. The fact: `reg_mask_clobbered` decides the high
    /// halves by the width of the value asked, which a register has none of.
    fn reg_mask_clobbers(
        &self,
        i: usize,
        phys: RegId,
    ) -> bool {
        let root = crate::model::ir::root(phys);
        let (slot, destroyed, high, _) = &self.calls[i];
        if destroyed.contains(&root) {
            return true;
        }
        // DEVIATE C-LANE: the high half of a register a call keeps the low
        // half of is lost to a value only where a wide read can follow.
        high.contains(&root) && self.lane.get().is_none_or(|virt| self.wide_read_follows(virt, slot))
    }

    fn lane_of(
        &self,
        phys: RegId,
    ) -> Option<u32> {
        let root = crate::model::ir::root(phys);
        self.flow.as_ref()?;
        if self.lane_registers.contains(&root) { self.lane.get() } else { None }
    }

    fn set_lane(
        &self,
        virt: Option<u32>,
    ) {
        self.lane.set(virt);
    }
}

/// What `decide` decided: the register of each value that has one, and the
/// values to spill.
pub struct Decided {
    pub report: ShadowReport,
    pub placed: Vec<(u32, RegId)>,
    pub spilled: Vec<u32>,
    /// With an inline spiller: the function as edited, which `placed` is
    /// about, and whether a spill could not be written.
    pub edited: Option<LirBody>,
    pub spill_failed: bool,
}

/// How a spilled range is dealt with.
pub enum Mode<'a> {
    /// Only noted; the caller's emitter writes it and allocates again.
    Record,
    /// Placeholder reloads and stores, to count them.
    Simple,
    /// Written into the function being allocated (`InlineSpiller`).
    Inline { frame: &'a mut Frame, classes: &'a RegisterClasses },
}

/// Runs the translated greedy allocator over `inputs`; None when the body has
/// phis (they are eliminated before allocation).
pub fn run(inputs: &ShadowInputs) -> Option<ShadowReport> {
    decide(inputs, false).map(|decided| decided.report)
}

/// `run`, and the decisions. With `recording` a spilled range is only noted and
/// leaves the problem; otherwise a spiller makes its reload and store ranges.
pub fn decide(
    inputs: &ShadowInputs,
    recording: bool,
) -> Option<Decided> {
    decide_with(inputs, if recording { Mode::Record } else { Mode::Simple })
}

/// `decide`, with the way spills are written chosen.
pub fn decide_with(
    inputs: &ShadowInputs,
    mode: Mode<'_>,
) -> Option<Decided> {
    decide_with_split(inputs, mode, !llrm_support::env_flag!("LLRM_NO_SPLIT"))
}

/// `decide_with`, with live range splitting (`trySplit`) chosen.
/// The blocks an x87 value is live into and out of, as (in, out) by block.
fn float_live_blocks(
    lis: &mut LiveIntervals,
    floats: &crate::support::hash::HashSet<u32>,
    count: usize,
) -> (Vec<bool>, Vec<bool>) {
    let mut live_in = vec![false; count];
    let mut live_out = vec![false; count];
    if floats.is_empty() {
        return (live_in, live_out);
    }
    let starts: Vec<SlotIndex> = (0..count).map(|at| lis.indexes.get_mbb_start_idx(at)).collect();
    let ends: Vec<SlotIndex> = (0..count).map(|at| lis.indexes.get_mbb_end_idx(at)).collect();
    for &reg in floats {
        for segment in &lis.interval(reg).segments {
            let mut at = starts.partition_point(|start| *start <= segment.start).saturating_sub(1);
            while at < count && starts[at] < segment.end {
                live_in[at] |= segment.start <= starts[at];
                live_out[at] |= segment.end >= ends[at];
                at += 1;
            }
        }
    }
    (live_in, live_out)
}

/// The intervals the allocator starts from: the coalescer's, edited to `body`,
/// else new ones.
///
/// `RAGreedy::runOnMachineFunction` then renumbers before allocating, "to get
/// accurate and consistent results from getApproxInstrDistance": the erased
/// copies leave gaps that inflate range sizes, and the dequeue priority is the
/// size.
/// https://github.com/llvm/llvm-project/blob/main/llvm/lib/CodeGen/RegAllocGreedy.cpp#L3074
fn intervals_to_allocate(body: &LirBody) -> LiveIntervals {
    let mut lis =
        crate::liveness::liveintervals::take_handed_over_edited_to(body).unwrap_or_else(|| LiveIntervals::new(body));
    lis.indexes.pack_indexes();
    lis
}

pub fn decide_with_split(
    inputs: &ShadowInputs,
    mode: Mode<'_>,
    split: bool,
) -> Option<Decided> {
    let body = inputs.body;
    if body.blocks.iter().any(|block| !block.phis.is_empty()) || body.blocks.is_empty() {
        return None;
    }
    if llrm_support::env_var("LLRM_MIR_PRINT").is_some_and(|n| n == &*body.name) {
        match crate::backend::mirprint::mir(body, inputs.pins) {
            Ok(text) => eprintln!("{text}"),
            Err(why) => eprintln!("MIR-PRINT-REFUSED {}: {why}", body.name),
        }
    }
    if llrm_support::env_var("LLRM_INPUT_DUMP").is_some_and(|n| n == &*body.name) {
        for (b, block) in body.blocks.iter().enumerate() {
            for (at, one) in block.insns.iter().enumerate() {
                eprintln!(
                    "IN b{b}[{at}] #{} d{:?} u{:?} g{:?} {:?}",
                    one.id(),
                    one.defines,
                    one.uses,
                    one.group,
                    one.what.as_ref().map(|w| (&w.name, &w.dests, &w.sources))
                );
            }
        }
    }
    let mut lis = intervals_to_allocate(body);
    // The environment: orders, frequencies, loop exits.
    let mbfi = crate::liveness::blockfrequency::MachineBlockFrequencyInfo::of(body);
    let freq: Vec<f32> =
        (0..body.blocks.len()).map(|block| mbfi.get_block_freq_relative_to_entry_block(block) as f32).collect();
    if llrm_support::env_flag!("LLRM_GREEDY_DECISIONS") {
        eprintln!("block frequencies {:?}", freq);
    }
    let loops = crate::analysis::loops::loops(&body.blocks, Some(body.entry));
    let exiting: Vec<bool> = body
        .blocks
        .iter()
        .map(|block| {
            // The innermost loop holding the block (loops come innermost
            // first).
            loops
                .iter()
                .find(|one| one.body.contains(&block.at))
                .is_some_and(|one| block.succ.iter().any(|to| !one.body.contains(to)))
        })
        .collect();
    let tables = Rc::new(Tables::default());
    // Under RFP the x87 values take FP0..FP6 (LLVM's RFP
    // classes); a stack position an instruction names is the stackifier's.
    let floats = crate::backend::floatassign::_floating_values(body);
    // A value named only as a byte is of the GR8 class, whose order names the
    // high bytes too (X86RegisterInfo.td); the others hold their root.
    let widest = crate::backend::widths::_widest(body);
    let bytes = inputs.registers.byte_order();
    for reg in lis.virt_regs() {
        let mut order: Vec<RegId> = if floats.contains(&reg) {
            inputs.registers.rfp_registers().to_vec()
        } else {
            (inputs.order)(reg).into_iter().map(crate::model::ir::root).collect()
        };
        order.dedup();
        if widest.get(&reg) == Some(&1) && !inputs.pins.contains_key(&reg) && !bytes.is_empty() {
            let roots = order.clone();
            let lanes: Vec<RegId> =
                bytes.iter().copied().filter(|byte| roots.contains(&crate::model::ir::root(*byte))).collect();
            if !lanes.is_empty() {
                order = lanes;
            }
        }
        tables.orders.borrow_mut().insert(reg, order);
    }
    // DEVIATE C-LANE (the pin's regmask is per whole GR32 register): where a
    // call destroys the high halves of registers (`clobbers_high`), a value no
    // instruction reads wider than 2 bytes is 2 bytes wide: its high half is
    // never read, so a call does not lose it and its spill is a word. Sound
    // because the width is the widest read of the value anywhere in the body,
    // so a read after the call, around a loop, or through a copy of the value
    // keeps it wide.
    let lanes = body.blocks.iter().flat_map(|block| block.insns.iter()).any(|one| !one.clobbers_high.is_empty());
    let read_widths = if lanes { crate::backend::widths::_widest_read(body) } else { Default::default() };
    for (value, width) in crate::backend::widths::_widest(body) {
        let width = if lanes && width > 2 && read_widths.get(&value).copied().unwrap_or(0) <= 2 { 2 } else { width };
        tables.widths.borrow_mut().insert(value, width);
    }
    if lanes {
        // Where the wide reads are, for `CallMasks::wide_read_follows`.
        let mut family = tables.family.borrow_mut();
        let mut wide_reads = tables.wide_reads.borrow_mut();
        for (at, block) in body.blocks.iter().enumerate() {
            for one in &block.insns {
                let slot = lis.indexes.get_instruction_index(one.id()).get_reg_slot(false);
                for (value, _) in crate::backend::widths::_read_widths(one).into_iter().filter(|(_, w)| *w > 2) {
                    wide_reads.entry(value).or_default().push((slot.clone(), at));
                }
            }
        }
        for value in read_widths.keys().chain(lis.virt_regs().iter()) {
            family.insert(*value, *value);
            wide_reads.entry(*value).or_default();
        }
    }
    tables.float_bytes.borrow_mut().extend(super::inlinespiller::float_precisions(body));
    let invariant = if !inputs.stable_loads {
        super::remat::argument_loads(body)
    } else {
        crate::backend::postings::following(body, |postings| {
            let defined: std::collections::BTreeSet<u32> = body
                .blocks
                .iter()
                .flat_map(|block| block.insns.iter())
                .flat_map(|one| one.defines.iter().copied())
                .collect();
            crate::backend::bodyfacts::_stable_loads_by(body, &defined, &Default::default(), postings)
                .keys()
                .copied()
                .collect::<std::collections::BTreeSet<u32>>()
        })
    };
    // A load the instruction calls invariant needs no proof (D-I exit).
    let invariant: std::collections::BTreeSet<u32> =
        invariant.into_iter().chain(super::remat::flagged_loads(body)).collect();
    tables.remat.borrow_mut().extend(super::remat::values(body, &invariant));
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        if let [value] = one.defines.as_slice()
            && tables.remat.borrow().contains(value)
        {
            tables.remat_roots.borrow_mut().entry(*value).or_insert_with(|| Arc::clone(one));
        }
    }
    let pins = inputs
        .pins
        .iter()
        .filter(|(reg, _)| !floats.contains(reg))
        .map(|(&reg, &phys)| (reg, crate::model::ir::root(phys)))
        .collect();
    let env = BodyEnv {
        orders: Rc::clone(&tables),
        pins,
        freq,
        exiting,
        saved: inputs.saved.iter().map(|one| crate::model::ir::root(*one)).collect(),
    };
    // The calls.
    let mut calls = Vec::new();
    let mut blocks = Vec::with_capacity(body.blocks.len());
    let (float_in, float_out) = float_live_blocks(&mut lis, &floats, body.blocks.len());
    for (at, block) in body.blocks.iter().enumerate() {
        let first = calls.len();
        let last = block.insns.len().saturating_sub(1);
        for (n, one) in block.insns.iter().enumerate() {
            // DEVIATE D-DS1 (exit: a spill or split tells the masks): the
            // masks are made once, before allocation. A split or a spill of an
            // x87 value that lives through a block stores or reloads it
            // (`fstp`, `fld`) at the block's ends, where no instruction of the
            // body names it; those need the data group (`needs_data_group`),
            // and `datagroup` then loads `ds` from `ss` there. A selector
            // placed in `ds` over such an end is lost.
            let at_edge = (n == 0 && float_in[at]) || (n == last && float_out[at]);
            let read: RegSet = inputs
                .data
                .filter(|_| at_edge || crate::backend::target::needs_data_group(one))
                .map(crate::model::ir::root)
                .into_iter()
                .collect();
            // The stack is empty at a call and wherever an instruction names a
            // position (`X86FloatingPoint`): FP0..FP6 are lost there.
            let empties = !floats.is_empty()
                && (one.call.is_some() || one.clobbers.iter().any(|r| inputs.registers.positional(*r)));
            if !one.clobbers.is_empty() || !one.clobbers_high.is_empty() || !read.is_empty() || empties {
                let slot = lis.indexes.get_instruction_index(one.id()).get_reg_slot(false);
                let roots =
                    |list: &BTreeSet<RegId>| -> RegSet { list.iter().map(|r| crate::model::ir::root(*r)).collect() };
                let mut destroyed = roots(&one.clobbers);
                if empties {
                    destroyed.extend(inputs.registers.rfp_registers().iter().copied());
                }
                calls.push((slot, destroyed, roots(&one.clobbers_high), read));
            }
        }
        let (start, end) = (lis.indexes.get_mbb_start_idx(at), lis.indexes.get_mbb_end_idx(at));
        debug_assert!(
            calls[first..].iter().all(|(slot, ..)| start <= *slot && *slot < end),
            "block {at}'s calls lie in its slots"
        );
        blocks.push((start, end, first..calls.len()));
    }
    let fixed_pins = !llrm_support::env_flag!("LLRM_PINS_AS_VREGS");
    let pins_cell: Rc<std::cell::OnceCell<FixedPins>> = Rc::default();
    let matrix = LiveRegMatrix::new(
        RegisterInfo::new(inputs.registers),
        Box::new(Fixed {
            calls: {
                let masks = CallMasks::of(calls, blocks, Rc::clone(&tables));
                if lanes { masks.with_flow(successors(body)) } else { masks }
            },
            pins: Rc::clone(&pins_cell),
        }),
    );
    let inline = matches!(mode, Mode::Inline { .. });
    // A split edits the function, and a caller that only records what to spill
    // writes it on the function as it was.
    let split = split && !matches!(mode, Mode::Record);
    let spiller: Box<dyn super::greedy::Spiller + '_> = match mode {
        Mode::Record => Box::new(super::spill::RecordingSpiller::default()),
        Mode::Simple => Box::new(SimpleSpiller::default()),
        Mode::Inline { frame, classes } => {
            let (made, sets, pins, registers) = (
                Rc::clone(&tables),
                inputs.sets.map(|sets| Rc::new(sets.clone())),
                inputs.pins.keys().copied().collect::<crate::support::hash::HashSet<u32>>(),
                inputs.registers,
            );
            Box::new(InlineSpiller::new(body, frame, classes, inputs.order, Rc::clone(&tables)).with_reclass(Box::new(
                move |lis, regs| reclass_orders(&made, &pins, sets.as_deref(), registers, lis, regs),
            )))
        }
    };
    let mut greedy = RAGreedy::new(lis, matrix, &env, spiller);
    if split {
        let split_env = Box::new(BodySplitEnv::new(
            body,
            Rc::clone(&tables),
            inputs.registers,
            inputs.sets.map(|sets| Rc::new(sets.clone())),
            inputs.pins.keys().copied().collect(),
        ));
        greedy.split = Some(super::regionsplit::SplitSupport::new(split_env, mbfi, &greedy.lis.cfg.succs));
    }
    greedy.calculate_spill_weights_and_hints();
    let fixed_regs: Vec<(u32, RegId)> = if fixed_pins {
        let mut copies = crate::support::hash::HashMap::default();
        for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
            if let Some((d, s)) =
                one.what.as_ref().and_then(|w| match (w.name.as_deref(), &w.dests[..], &w.sources[..]) {
                    (Some("mov"), [llrm_lir::Loc::Held(d)], [llrm_lir::Loc::Held(s)]) => Some((d.value, s.value)),
                    _ => None,
                })
            {
                copies.insert(greedy.lis.indexes.get_instruction_index(one.id()).get_reg_slot(false), (d, s));
            }
        }
        let wanted: Vec<(u32, RegId)> = inputs
            .pins
            .iter()
            .filter(|(reg, _)| !floats.contains(reg) && greedy.lis.has_interval(**reg))
            .map(|(&reg, &phys)| (reg, phys))
            .collect();
        let pins: Vec<FixedPin> = wanted
            .into_iter()
            .map(|(reg, phys)| FixedPin {
                reg,
                root: crate::model::ir::root(phys),
                segments: greedy
                    .lis
                    .interval_rc(reg)
                    .segments
                    .iter()
                    .map(|s| (s.start.clone(), s.end.clone()))
                    .collect(),
            })
            .collect();
        let regs = pins.iter().map(|pin| (pin.reg, pin.root)).collect();
        let tri = RegisterInfo::new(inputs.registers);
        let mut spans: crate::support::hash::HashMap<u32, Vec<(SlotIndex, SlotIndex)>> =
            crate::support::hash::HashMap::default();
        for pin in &pins {
            for unit in tri.regunits(pin.root) {
                spans.entry(unit).or_default().extend(pin.segments.iter().cloned());
            }
        }
        // Two pinned values meeting in one register, other than at a copy
        // between them, cannot both be there: no allocation exists.
        let mut conflict = false;
        let units = spans
            .into_iter()
            .map(|(unit, mut list)| {
                list.sort();
                let mut merged: Vec<(SlotIndex, SlotIndex)> = Vec::new();
                for (start, end) in list {
                    match merged.last_mut() {
                        Some(last) if start <= last.1 => {
                            conflict |= start < last.1 && !copies.contains_key(&start);
                            last.1 = std::cmp::max(last.1.clone(), end)
                        }
                        _ => merged.push((start, end)),
                    }
                }
                let range = crate::liveness::liverange::LiveRange {
                    segments: merged
                        .into_iter()
                        .map(|(start, end)| crate::liveness::liverange::Segment::new(start, end, None))
                        .collect(),
                    valnos: Vec::new(),
                };
                (unit, range)
            })
            .collect();
        if conflict {
            return None;
        }
        let _ = pins_cell.set(FixedPins { tri, pins, units, copies, overlapping: Default::default() });
        regs
    } else {
        Vec::new()
    };
    let values = greedy.lis.virt_regs().len();
    // Values that may not be spilled, and the pinned ones in the registers that
    // fix them.
    for &reg in inputs.unspillable {
        if greedy.lis.has_interval(reg) {
            let mut li = (*greedy.lis.interval_rc(reg)).clone();
            li.mark_not_spillable();
            greedy.lis.insert_interval(li);
        }
    }
    // A value an instruction requires in one register may be given that one
    // only: its allocation order is the register (`getRegClass` of a
    // one-register class), and it competes like any other range: it can be
    // evicted or spilled.
    for (&reg, &phys) in inputs.pins {
        if !greedy.lis.reg_nodbg_empty(reg) && !floats.contains(&reg) {
            tables.orders.borrow_mut().insert(reg, vec![crate::model::ir::root(phys)]);
            if !fixed_regs.iter().any(|(fixed, _)| *fixed == reg) {
                greedy.pinned.insert(reg);
            }
        }
    }
    for (reg, root) in &fixed_regs {
        greedy.fixed.insert(*reg);
        greedy.matrix.vrm.assign_virt2phys(*reg, *root);
    }
    if llrm_support::env_flag!("LLRM_GREEDY_DECISIONS") {
        eprintln!("== {}", body.name);
    }
    greedy.allocate_phys_regs();
    if inline {
        greedy.hoist_spills(body);
        greedy.stacked_spills();
    }
    let mut signature = 0u64;
    for reg in greedy.lis.virt_regs() {
        if let Some(phys) = greedy.matrix.vrm.get_phys(reg) {
            signature = signature.wrapping_mul(1_000_003).wrapping_add(u64::from(reg) * 131 + phys.index() as u64);
        }
    }
    let (reloads, stores, folded) = greedy.spill_counts();
    if llrm_support::env_var("LLRM_MIR_PRINT").is_some_and(|n| n == &*body.name) {
        eprintln!("SPILL-COUNTS {} stores={stores} reloads={reloads} folded={folded}", body.name);
    }
    let mut placed: Vec<(u32, RegId)> = Vec::new();
    for reg in greedy.lis.virt_regs() {
        if let Some(phys) = greedy.matrix.vrm.get_phys(reg) {
            placed.push((reg, phys));
        }
    }
    if llrm_support::env_flag!("LLRM_PLACED_DEBUG") {
        for reg in greedy.lis.values().collect::<Vec<_>>() {
            if greedy.matrix.vrm.get_phys(reg).is_none() {
                eprintln!(
                    "v{reg} named, no phys; interval {} stage {:?} original {}",
                    greedy.lis.has_interval(reg),
                    greedy.extra.get_stage(reg),
                    greedy.matrix.vrm.get_original(reg)
                );
            }
        }
    }
    let spilled = greedy.recorded_spills();
    let report = ShadowReport { values, stats: greedy.stats.clone(), reloads, stores, folded, signature };
    let edited = inline.then(|| {
        let blocks =
            body.blocks.iter().zip(greedy.lis.blocks()).map(|(block, insns)| block.with_insns(insns.clone())).collect();
        body.with_blocks(blocks)
    });
    if llrm_support::env_flag!("LLRM_CHECK_LIVEINTERVALS") {
        if let Some(edited) = &edited {
            let held = greedy.lis.virt_regs().into_iter().filter(|r| greedy.lis.has_interval(*r)).count();
            let compared = greedy.lis.assert_intervals_equal_afresh(edited);
            assert!(
                compared == held,
                "LLRM_CHECK_LIVEINTERVALS compared {compared} of {held} intervals of {}",
                body.name
            );
            if llrm_support::env_flag!("LLRM_GREEDY_TRACE") {
                eprintln!("check-liveintervals {}: {compared} intervals equal afresh", body.name);
            }
        }
    }
    Some(Decided { report, placed, spilled, edited, spill_failed: greedy.spill_failed() })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::liveness::liverange::Segment;
    use crate::liveness::slotindex::SlotIndexes;
    use crate::model::lir::{Block, LirBlock};

    /// The interference cache searched the regmask slots twice (two binary
    /// searches over `SlotIndex`es, 364 instructions) for every block it
    /// computed: 3.9 M of d_faces -O2's 1,548 M. A block's run is read as
    /// `getRegMaskSlotsInBlock` does.
    #[test]
    fn test_a_block_s_regmask_slots_are_read_not_searched() {
        let (searched, read) = regmask_lookups();
        crate::backend::regalloc_fuzz_tests::allocate_generated_bodies(40);
        let (searched_now, read_now) = regmask_lookups();
        assert!(read_now > read, "premise: some block's interference was computed");
        assert_eq!(searched_now - searched, 0, "searches for a block's regmask slots");
    }

    /// Under RFP an x87 value takes one of FP0..FP6 or is
    /// spilled: nine live at once over seven registers, two at least go to
    /// memory. A value that lived in an x87 register over an instruction that
    /// names a stack position (the stack must be empty there) would be lost.
    #[test]
    fn test_x87_values_take_fp0_to_fp6_and_the_rest_spill() {
        use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
        let float = |value: u32| Loc::Held(Held { value, width: 10 });
        let cell = |disp: i64| Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, disp)), 8));
        let what = |op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>| Semantics {
            name: Some(name.into()),
            dests,
            sources,
            ..Semantics::new(op)
        };
        let mut insns: Vec<Arc<Insn>> = (1..=9)
            .map(|v| {
                let load = what(Operation::FloatLoad, "fld", vec![float(v)], vec![cell(-8 * i64::from(v))]);
                Arc::new(Insn::new(i64::from(v), None, Some(load), vec![v], vec![]))
            })
            .collect();
        let mut empties = Insn::new(20, None, None, vec![], vec![]);
        empties.clobbers = [RegId::ST0].into_iter().collect();
        insns.push(Arc::new(empties));
        for v in 1..=9 {
            let store = what(Operation::FloatStore, "fstp", vec![cell(-100 - 8 * i64::from(v))], vec![float(v)]);
            insns.push(Arc::new(Insn::new(30 + i64::from(v), None, Some(store), vec![], vec![v])));
        }
        let body = LirBody::new(
            "x87",
            0,
            vec![Block::from(LirBlock::new(0, insns))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let order = |_: u32| vec![RegId::EAX];
        let inputs = ShadowInputs {
            body: &body,
            registers: Regs(&llrm_x86_m16::REGISTER_INFO),
            order: &order,
            pins: &IndexMap::default(),
            unspillable: &BTreeSet::new(),
            data: None,
            sets: None,
            stable_loads: true,
            saved: &[],
        };
        let decided = decide_with(&inputs, Mode::Simple).expect("greedy allocated the body");
        let rfp = Regs(&llrm_x86_m16::REGISTER_INFO).rfp_registers().to_vec();
        assert!(decided.placed.iter().all(|(_, reg)| rfp.contains(reg)), "{:?}", decided.placed);
        // All nine live across the empty-stack point: none keeps a register
        // (the registers placed are the reloads' and the stores' short ranges).
        assert!(decided.placed.iter().all(|(value, _)| *value > 9), "{:?}", decided.placed);
        assert_eq!(decided.report.stats.spilled_ranges, 9, "{:?}", decided.report.stats);
    }

    /// Instructions added since the slot indexes were made are numbered at half
    /// the spacing, and the allocator did not renumber as `RAGreedy` does: a
    /// range across them was shorter than the same range elsewhere, so the
    /// dequeue order (the size) followed where the edits were
    /// (_bench_x_dct under the scheduler after the coalescer: 253 insns,
    /// 227 packed).
    #[test]
    fn test_the_allocator_starts_from_packed_slot_indexes() {
        let make = |from: i64, n: i64| -> Vec<Arc<Insn>> {
            (from..from + n).map(|k| Arc::new(Insn::new(k, None, None, vec![], vec![]))).collect()
        };
        let old = make(0, 2);
        let added = make(10, 5);
        let body_of = |list: Vec<Arc<Insn>>| {
            LirBody::new("pack", 0, vec![Block::from(LirBlock::new(0, list))], IndexMap::default(), IndexMap::default())
        };
        let before = body_of(old.clone());
        let mut grown = vec![Arc::clone(&old[0])];
        grown.extend(added.iter().cloned());
        grown.push(Arc::clone(&old[1]));
        let after = body_of(grown);
        crate::liveness::liveintervals::hand_over(LiveIntervals::new(&before), &before);
        let lis = intervals_to_allocate(&after);
        let at: Vec<u32> =
            after.blocks[0].insns.iter().map(|one| lis.indexes.get_instruction_index(one.id()).get_index()).collect();
        let steps: Vec<u32> = at.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert!(steps.iter().all(|step| *step == SlotIndex::INSTR_DIST), "{steps:?}");
    }

    /// Greedy's spill weights read `MachineBlockFrequencyInfo`
    /// (`liveness::blockfrequency`), not the old `analysis::frequency`,
    /// which saturated an f32 at u64::MAX (D-H, #2276).
    #[test]
    fn test_greedy_asks_no_old_frequency() {
        let insns: Vec<Arc<Insn>> = vec![
            Arc::new(Insn::new(0, None, None, vec![1], vec![])),
            Arc::new(Insn::new(1, None, None, vec![], vec![1])),
        ];
        let body = LirBody::new(
            "shadow",
            0,
            vec![Block::from(LirBlock::new(0, insns))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let order = |_: u32| vec![RegId::EAX, RegId::EBX];
        let inputs = ShadowInputs {
            body: &body,
            registers: Regs(&llrm_x86_m16::REGISTER_INFO),
            order: &order,
            pins: &IndexMap::default(),
            unspillable: &BTreeSet::new(),
            data: None,
            sets: None,
            stable_loads: true,
            saved: &[],
        };
        let before = crate::analysis::frequency::OF_CALLS.with(std::cell::Cell::get);
        assert!(decide_with(&inputs, Mode::Simple).is_some(), "premise: greedy allocated the body");
        assert_eq!(crate::analysis::frequency::OF_CALLS.with(std::cell::Cell::get) - before, 0);
    }

    fn widths(pairs: &[(u32, u32)]) -> Rc<Tables> {
        let tables = Tables::default();
        tables.widths.borrow_mut().extend(pairs.iter().copied());
        Rc::new(tables)
    }

    /// A 16-bit callee that pushes and pops `si` keeps its low half and
    /// destroys the high half (`clobbers_high`): a 32-bit value held in `esi`
    /// across the call came back with a wrong high half (parity/loop at -O2:
    /// printed -81, wanted 130991). A 16-bit value in `si` is unharmed.
    #[test]
    fn test_a_wide_value_does_not_live_in_a_register_whose_high_half_a_call_destroys() {
        let insns: Vec<Arc<Insn>> = (0..10).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "shadow",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at = |n: usize| indexes.get_instruction_index(insns[n].id()).get_reg_slot(false);
        let esi = crate::model::ir::root(RegId::ESI);
        let masks = CallMasks::of(
            vec![(at(5), RegSet::new(), [esi].into_iter().collect(), RegSet::new())],
            Vec::new(),
            widths(&[(1, 4), (2, 2)]),
        );
        let across = |reg: u32| {
            let mut li = LiveInterval::new(reg, 1.0);
            let v = li.get_next_value(at(2));
            li.add_segment(Segment::new(at(2), at(8), Some(v)));
            li
        };
        assert_eq!(masks.reg_mask_clobbered(&across(1)), Some([esi].into_iter().collect()));
        assert!(masks.reg_mask_clobbered(&across(2)).is_none_or(|roots| roots.is_empty()));
    }

    /// A value in no width table is held as wide as the widest: the complement
    /// of a split has no entry until `note_split_from` gives it its parent's,
    /// and read as narrow it was placed in `esi` across a call that destroys
    /// the high half (crc.bas -O2 m16 printed 14630 for -873187034).
    #[test]
    fn test_a_value_in_no_width_table_is_held_against_a_call_that_destroys_a_high_half() {
        let insns: Vec<Arc<Insn>> = (0..10).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "shadow",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at = |n: usize| indexes.get_instruction_index(insns[n].id()).get_reg_slot(false);
        let esi = crate::model::ir::root(RegId::ESI);
        let masks = CallMasks::of(
            vec![(at(5), RegSet::new(), [esi].into_iter().collect(), RegSet::new())],
            Vec::new(),
            widths(&[(1, 4)]),
        );
        let mut li = LiveInterval::new(9, 1.0);
        let v = li.get_next_value(at(2));
        li.add_segment(Segment::new(at(2), at(8), Some(v)));
        assert_eq!(masks.reg_mask_clobbered(&li), Some([esi].into_iter().collect()));
    }

    /// An instruction that reads the data segment register without naming it
    /// takes it from a range that ends at it: a far array's selector placed in
    /// `ds` was loaded with the stack's, and matmul.bas stored into the stack
    /// (`mov ds, ss`) where its array was (a wrong checksum, a crash).
    #[test]
    fn test_a_range_that_ends_at_an_instruction_reading_the_data_segment_is_not_placed_in_it() {
        let insns: Vec<Arc<Insn>> = (0..10).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "shadow",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at = |n: usize| indexes.get_instruction_index(insns[n].id()).get_reg_slot(false);
        let ds = crate::model::ir::root(RegId::DS);
        let masks = CallMasks::of(
            vec![(at(5), RegSet::new(), RegSet::new(), [ds].into_iter().collect())],
            Vec::new(),
            widths(&[]),
        );
        let live = |to: usize| {
            let mut li = LiveInterval::new(1, 1.0);
            let v = li.get_next_value(at(2));
            li.add_segment(Segment::new(at(2), at(to), Some(v)));
            li
        };
        assert_eq!(masks.reg_mask_clobbered(&live(5)), Some([ds].into_iter().collect()), "ends at the instruction");
        assert_eq!(masks.reg_mask_clobbered(&live(8)), Some([ds].into_iter().collect()), "goes on past it");
        assert!(masks.reg_mask_clobbered(&live(4)).is_none_or(|roots| roots.is_empty()), "ends before it");
    }

    /// A selector placed in `ds` over a block an x87 value lives through was
    /// clobbered: a split or spill stores the value (`fstp`) at the block's
    /// edge, an instruction that needs the data group, and `datagroup` put
    /// `mov ds, ss` there. nbody.bas at -O2 (m16) wrote an array through the
    /// stack segment and printed 4849201 for 4774160.
    #[test]
    fn test_a_selector_is_not_placed_in_ds_across_a_block_an_x87_value_lives_through() {
        use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Semantics, Space};
        let what = |op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>| Semantics {
            name: Some(name.into()),
            dests,
            sources,
            ..Semantics::new(op)
        };
        let insn = |at: i64, what: Semantics, defines: &[u32], uses: &[u32]| {
            Arc::new(Insn::new(at, None, Some(what), defines.to_vec(), uses.to_vec()))
        };
        let float = Loc::Held(Held { value: 1, width: 10 });
        let selector = Loc::Held(Held { value: 2, width: 2 });
        let cell = |disp: i64| Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, disp)), 8));
        let load = || insn(1, what(Operation::FloatLoad, "fld", vec![float.clone()], vec![cell(-8)]), &[1], &[]);
        let store = || insn(2, what(Operation::FloatStore, "fstp", vec![cell(-16)], vec![float.clone()]), &[], &[1]);
        let make = || {
            insn(
                3,
                what(
                    Operation::Move,
                    "mov",
                    vec![selector.clone()],
                    vec![Loc::Imm(Imm { value: 1, width: 2, address: None })],
                ),
                &[2],
                &[],
            )
        };
        let read = || insn(4, what(Operation::Move, "mov", vec![cell(-24)], vec![selector.clone()]), &[], &[2]);
        let jump = |at: i64| insn(at, what(Operation::Jump, "jmp", vec![], vec![]), &[], &[]);
        let body = |blocks: Vec<Vec<Arc<Insn>>>| {
            let count = blocks.len();
            let blocks: Vec<Block> = blocks
                .into_iter()
                .enumerate()
                .map(|(n, insns)| {
                    let mut b = LirBlock::new(n as i64, insns);
                    if n + 1 < count {
                        b.succ = vec![n as i64 + 1].into();
                    }
                    Block::from(b)
                })
                .collect();
            LirBody::new("selector", 0, blocks, IndexMap::default(), IndexMap::default())
        };
        let order = |value: u32| if value == 2 { vec![RegId::DS, RegId::ES] } else { vec![RegId::EAX] };
        let placed_in_ds = |body: &LirBody| {
            let inputs = ShadowInputs {
                body,
                registers: Regs(&llrm_x86_m16::REGISTER_INFO),
                order: &order,
                pins: &IndexMap::default(),
                unspillable: &BTreeSet::new(),
                data: Some(RegId::DS),
                sets: None,
                stable_loads: true,
                saved: &[],
            };
            let decided = decide_with(&inputs, Mode::Simple).expect("allocated");
            decided
                .placed
                .iter()
                .any(|(value, reg)| *value == 2 && crate::model::ir::root(*reg) == crate::model::ir::root(RegId::DS))
        };
        // The x87 value dies before the middle block: the selector may take
        // `ds`.
        let apart = body(vec![vec![make(), jump(5)], vec![jump(6)], vec![read(), load(), store()]]);
        assert!(placed_in_ds(&apart), "premise: the selector is placed in ds");
        // The x87 value lives through the middle block.
        let through = body(vec![vec![load(), make(), jump(5)], vec![jump(6)], vec![read(), store()]]);
        assert!(!placed_in_ds(&through), "the selector was placed in ds over a block the x87 value lives through");
    }

    /// A local range may take the register of a cheaper local one when that one
    /// can move to a free register (`canReassign`, on in the pin: the subtarget
    /// answers true). Without it the heavier range took the first register it
    /// found and the other range, with a single register to go to, was spilled.
    #[test]
    fn test_a_local_range_is_evicted_when_it_can_move_to_a_free_register() {
        let none = || Arc::new(Insn::new(0, None, None, vec![], vec![]));
        let insn = |defines: &[u32], uses: &[u32]| Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()));
        // 1 and 2 are long and lightly used (1 more than 2); 3 is short and
        // heavy.
        let mut insns = vec![insn(&[1], &[]), insn(&[2], &[]), insn(&[3], &[])];
        insns.extend((0..4).map(|_| insn(&[], &[3])));
        insns.push(insn(&[], &[1]));
        insns.extend((0..11).map(|_| none()));
        insns.extend([insn(&[], &[1]), insn(&[], &[1]), insn(&[], &[2])]);
        let body = LirBody::new(
            "reassign",
            0,
            vec![Block::from(LirBlock::new(0, insns))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let order = |value: u32| match value {
            1 => vec![RegId::EAX],
            2 => vec![RegId::EBX, RegId::ECX],
            3 => vec![RegId::EAX, RegId::EBX],
            _ => vec![RegId::EAX],
        };
        let inputs = ShadowInputs {
            body: &body,
            registers: Regs(&llrm_x86_m16::REGISTER_INFO),
            order: &order,
            pins: &IndexMap::default(),
            unspillable: &BTreeSet::new(),
            data: None,
            sets: None,
            stable_loads: true,
            saved: &[],
        };
        let decided = decide_with(&inputs, Mode::Simple).expect("allocated");
        assert_eq!(decided.report.stats.spilled_ranges, 0, "{:?}", decided.report.stats);
    }

    /// A range live at the two ends of a function and not between them was
    /// checked against every call between: a step per call in the hole
    /// (`reg_mask_clobbered`: 8.4 M of d_faces' register allocation). The calls
    /// in the hole are skipped by a search.
    #[test]
    fn test_a_hole_in_a_range_is_not_walked_call_by_call() {
        let insns: Vec<Arc<Insn>> = (0..2000).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "shadow",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at = |n: usize| indexes.get_instruction_index(insns[n].id()).get_reg_slot(false);
        let esi = crate::model::ir::root(RegId::ESI);
        let calls = (10..1990).map(|n| (at(n), RegSet::new(), [esi].into_iter().collect(), RegSet::new())).collect();
        let masks = CallMasks::of(calls, Vec::new(), widths(&[(1, 4)]));
        let mut li = LiveInterval::new(1, 1.0);
        let v = li.get_next_value(at(2));
        li.add_segment(Segment::new(at(2), at(4), Some(v.clone())));
        li.add_segment(Segment::new(at(1995), at(1998), Some(v)));
        let before = calls_visited();
        assert!(masks.reg_mask_clobbered(&li).is_none_or(|roots| roots.is_empty()));
        let visited = calls_visited() - before;
        assert!(visited <= 10, "{visited} calls looked at for a range with no call in it");
    }

    /// `checkRegMaskInterference` looks at the calls inside the segments and
    /// one at an end, no more: the walk stepped through calls between the
    /// segments too, one union of three masks each (d_faces: 7.66 M against
    /// LLVM's 0.78 M for the same 964 ranges).
    #[test]
    fn test_only_the_calls_inside_a_range_are_looked_at() {
        let insns: Vec<Arc<Insn>> = (0..100).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "shadow",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at = |n: usize| indexes.get_instruction_index(insns[n].id()).get_reg_slot(false);
        let esi = crate::model::ir::root(RegId::ESI);
        // The call at 70 takes edi too, which no call in the range does: the
        // range never has all the calls take, and meets each one it spans.
        let edi = crate::model::ir::root(RegId::EDI);
        let calls = [5, 12, 15, 30, 40, 55, 70]
            .map(|n| {
                let taken = if n == 70 { vec![esi, edi] } else { vec![esi] };
                (at(n), taken.into_iter().collect::<RegSet>(), RegSet::new(), RegSet::new())
            })
            .into_iter()
            .collect();
        let masks = CallMasks::of(calls, Vec::new(), widths(&[(1, 4)]));
        let mut li = LiveInterval::new(1, 1.0);
        let v = li.get_next_value(at(10));
        li.add_segment(Segment::new(at(10), at(20), Some(v.clone())));
        li.add_segment(Segment::new(at(50), at(60), Some(v)));
        let before = calls_visited();
        assert_eq!(masks.reg_mask_clobbered(&li), Some([esi].into_iter().collect()));
        // 12, 15 and 55.
        assert_eq!(calls_visited() - before, 3);
    }

    /// A range live across calls that all take the same registers lost them at
    /// the first: it was unioned with the other 98 of each query (d_faces:
    /// 98 calls a query, 2.8 M for 1,067 queries; LLVM's 2.2 unions a query).
    #[test]
    fn test_a_range_that_has_lost_all_any_call_takes_looks_at_no_more_calls() {
        let insns: Vec<Arc<Insn>> = (0..100).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "shadow",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        let at = |n: usize| indexes.get_instruction_index(insns[n].id()).get_reg_slot(false);
        let esi = crate::model::ir::root(RegId::ESI);
        let calls =
            (10..90).map(|n| (at(n), [esi].into_iter().collect::<RegSet>(), RegSet::new(), RegSet::new())).collect();
        let masks = CallMasks::of(calls, Vec::new(), widths(&[(1, 4)]));
        let mut li = LiveInterval::new(1, 1.0);
        let v = li.get_next_value(at(2));
        li.add_segment(Segment::new(at(2), at(95), Some(v)));
        let before = calls_visited();
        assert_eq!(masks.reg_mask_clobbered(&li), Some([esi].into_iter().collect()));
        assert_eq!(calls_visited() - before, 1, "80 calls, all taking esi");
    }

    /// A value used before and after a stretch where two others need both
    /// registers was spilled everywhere and reloaded at each of its four uses
    /// (`trySplit` was a stub): split around the stretch it is reloaded once.
    #[test]
    fn test_a_value_that_does_not_fit_in_the_middle_is_split_not_spilled_everywhere() {
        let insn = |defines: &[u32], uses: &[u32]| Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()));
        let blocks: Vec<Block> = [
            (0, vec![insn(&[1], &[]), insn(&[], &[1]), insn(&[], &[1])]),
            (1, vec![insn(&[2], &[]), insn(&[3], &[]), insn(&[], &[2, 3])]),
            (2, vec![insn(&[], &[1]), insn(&[], &[1])]),
        ]
        .into_iter()
        .map(|(n, insns)| {
            let mut b = LirBlock::new(n, insns);
            if n < 2 {
                b.succ = vec![n + 1].into();
            }
            Block::from(b)
        })
        .collect();
        let body = LirBody::new("split", 0, blocks, IndexMap::default(), IndexMap::default());
        let order = |_: u32| vec![RegId::EAX, RegId::EBX];
        let inputs = ShadowInputs {
            body: &body,
            registers: Regs(&llrm_x86_m16::REGISTER_INFO),
            order: &order,
            pins: &IndexMap::default(),
            unspillable: &BTreeSet::new(),
            data: None,
            sets: None,
            stable_loads: true,
            saved: &[],
        };
        let plain = decide_with_split(&inputs, Mode::Simple, false).expect("allocated");
        let split = decide_with_split(&inputs, Mode::Simple, true).expect("allocated");
        assert_eq!(plain.report.stats.failed, 0, "premise: it allocates without splitting");
        assert_eq!(split.report.stats.failed, 0);
        assert!(plain.report.reloads >= 4, "premise: {} reloads without a split", plain.report.reloads);
        assert!(
            split.report.reloads < plain.report.reloads,
            "{} reloads with a split, {} without",
            split.report.reloads,
            plain.report.reloads
        );
    }

    /// Splitting is on by default (`LLRM_NO_SPLIT=1` turns it off): the
    /// default allocation of the body that only splitting can improve is the
    /// split one.
    #[test]
    fn test_live_range_splitting_is_on_by_default() {
        let insn = |defines: &[u32], uses: &[u32]| Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()));
        let blocks: Vec<Block> = [
            (0, vec![insn(&[1], &[]), insn(&[], &[1]), insn(&[], &[1])]),
            (1, vec![insn(&[2], &[]), insn(&[3], &[]), insn(&[], &[2, 3])]),
            (2, vec![insn(&[], &[1]), insn(&[], &[1])]),
        ]
        .into_iter()
        .map(|(n, insns)| {
            let mut b = LirBlock::new(n, insns);
            if n < 2 {
                b.succ = vec![n + 1].into();
            }
            Block::from(b)
        })
        .collect();
        let body = LirBody::new("on", 0, blocks, IndexMap::default(), IndexMap::default());
        let order = |_: u32| vec![RegId::EAX, RegId::EBX];
        let inputs = ShadowInputs {
            body: &body,
            registers: Regs(&llrm_x86_m16::REGISTER_INFO),
            order: &order,
            pins: &IndexMap::default(),
            unspillable: &BTreeSet::new(),
            data: None,
            sets: None,
            stable_loads: true,
            saved: &[],
        };
        let plain = decide_with_split(&inputs, Mode::Simple, false).expect("allocated");
        let split = decide_with_split(&inputs, Mode::Simple, true).expect("allocated");
        assert!(split.report.reloads < plain.report.reloads, "premise: the split body reloads less");
        let default = decide_with(&inputs, Mode::Simple).expect("allocated");
        assert_eq!(default.report.reloads, split.report.reloads, "the default is the split one");
    }

    /// A register made by a split is a clone of its parent (`createFrom`): as
    /// wide, so a call that destroys the high half of `esi` (m16) is held
    /// against it. The complement of a split has no copy defining it and no
    /// table entry; it was placed in `edi` across a call and bintree.c -O2
    /// -m16 printed 165553 for 5736113.
    #[test]
    fn test_a_register_made_by_a_split_is_as_wide_as_its_parent() {
        use crate::greedy::splitkit::SplitEnv as _;
        use crate::model::ir::{Held, Loc, Operation, Semantics};
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: 1, width: 4 })],
            sources: vec![Loc::Held(Held { value: 2, width: 4 })],
            ..Semantics::new(Operation::Move)
        };
        let insns = vec![Arc::new(Insn::new(0, None, Some(what), vec![1], vec![2]))];
        let body = LirBody::new(
            "wide",
            0,
            vec![Block::from(LirBlock::new(0, insns))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let lis = LiveIntervals::new(&body);
        // The parent (value 1) is in no table: a spill made it.
        let tables = Rc::new(Tables::default());
        let env =
            BodySplitEnv::new(&body, Rc::clone(&tables), Regs(&llrm_x86_m16::REGISTER_INFO), None, Default::default());
        env.note_split_from(&lis, 7, 1);
        assert_eq!(tables.widths.borrow().get(&7), Some(&4), "the clone is as wide as its parent");
    }

    /// A split's copy is as wide as the parent's class (`createFrom`'s
    /// register class), read from the table, not from a walk over every
    /// operand of the parent: d_faces -O2 spent 3 M instructions on 266 copies
    /// that way, a scan of the parent's operands for each.
    #[test]
    fn test_a_split_copy_takes_the_width_the_table_holds_for_its_parent() {
        use crate::greedy::splitkit::SplitEnv as _;
        use crate::model::ir::{Held, Loc, Operation, Semantics};
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: 1, width: 2 })],
            sources: vec![Loc::Held(Held { value: 2, width: 2 })],
            ..Semantics::new(Operation::Move)
        };
        let insns = vec![Arc::new(Insn::new(0, None, Some(what), vec![1], vec![2]))];
        let body = LirBody::new(
            "table",
            0,
            vec![Block::from(LirBlock::new(0, insns))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let lis = LiveIntervals::new(&body);
        let tables = Rc::new(Tables::default());
        tables.widths.borrow_mut().insert(1, 4);
        let env =
            BodySplitEnv::new(&body, Rc::clone(&tables), Regs(&llrm_x86_m16::REGISTER_INFO), None, Default::default());
        let copy = env.build_copy(&lis, 7, 1, None);
        let width = copy.what.as_ref().map(|what| what.dests.clone());
        assert_eq!(width, Some(vec![Loc::Held(Held { value: 7, width: 4 })]), "the table says 4, the operands say 2");
    }

    /// A phi value defined by a literal on one edge and an add on the other
    /// is not rematerializable as a register, but its literal definition is:
    /// a split copied it instead, and fib -O3 -m32 kept `xor ebx, ebx` in the
    /// loop header to store ebx on the exit edge where LLVM stores `0`.
    #[test]
    fn test_a_split_remakes_a_literal_definition_of_a_register_defined_twice() {
        use crate::greedy::splitkit::SplitEnv as _;
        use crate::model::ir::{Held, Imm, Loc, Mem, Operation, Semantics};
        let held = |value| Loc::Held(Held { value, width: 4 });
        let literal = Semantics {
            name: Some("mov".into()),
            dests: vec![held(1)],
            sources: vec![Loc::Imm(Imm { value: 0, width: 4, address: None })],
            ..Semantics::new(Operation::Move)
        };
        let add = Semantics {
            name: Some("add".into()),
            dests: vec![held(1)],
            sources: vec![held(1), held(2)],
            ..Semantics::new(Operation::Binary)
        };
        let zero = Insn::new(0, None, Some(literal), vec![1], vec![]);
        let insns = vec![Arc::new(zero.clone()), Arc::new(Insn::new(1, None, Some(add), vec![1], vec![1, 2]))];
        let body = LirBody::new(
            "phi",
            0,
            vec![Block::from(LirBlock::new(0, insns))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let tables = Rc::new(Tables::default());
        let env =
            BodySplitEnv::new(&body, Rc::clone(&tables), Regs(&llrm_x86_m16::REGISTER_INFO), None, Default::default());
        let made = env.rematerialize(&zero, 7, 1, None).expect("the literal is made again");
        assert_eq!(made.defines, [7]);
        assert!(matches!(made.what.as_ref().unwrap().sources[..], [Loc::Imm(Imm { value: 0, .. })]));
        // A load still needs the register's facts (D-I): nothing says the cell
        // is unchanged at the split.
        let load = Semantics {
            name: Some("mov".into()),
            dests: vec![held(1)],
            sources: vec![Loc::Mem(Mem {
                addr: None,
                width: 4,
                through: RegId::EBP,
                offset: -4,
                disp_width: 1,
                base: None,
                stack_argument: false,
                selector: None,
                index: None,
                scale: 1,
                index_through: RegId::EBP,
                exact: false,
            })],
            ..Semantics::new(Operation::Move)
        };
        let load = Insn::new(0, None, Some(load), vec![1], vec![]);
        assert!(env.rematerialize(&load, 8, 1, None).is_none());
    }

    /// A reload made for a stored word got its parent's order, {BX}
    /// (ls.c `_ls_animate@3`: `[abs+516 + %53 + v16] := mov %51`), and with the
    /// base holding {BX} too the two had one register between them: the
    /// function fell back to the rounds. Its own instructions only store
    /// it, so the order widens to every register; the base, which an
    /// address confines, stays among BX, SI and DI.
    #[test]
    fn test_a_spilled_values_stored_word_takes_the_class_its_instructions_give() {
        use crate::model::ir::{Held, Imm, Loc, Mem, Operation, Semantics};
        let (base, word) = (1u32, 2u32);
        let held = |value| Loc::Held(Held { value, width: 2 });
        let what = |op, name: &str, dests, sources| Semantics {
            name: Some(name.into()),
            dests,
            sources,
            ..Semantics::new(op)
        };
        let imm = |value| Loc::Imm(Imm { value, width: 2, address: None });
        let make = |at, defines: Vec<u32>, uses: Vec<u32>, w| Arc::new(Insn::new(at, None, Some(w), defines, uses));
        let cell = Mem { base: Some(Held { value: base, width: 2 }), ..Mem::new(None, 2) };
        let insns = vec![
            make(1, vec![base], vec![], what(Operation::Move, "mov", vec![held(base)], vec![imm(2)])),
            make(2, vec![word], vec![], what(Operation::Move, "mov", vec![held(word)], vec![imm(3)])),
            make(3, vec![], vec![base, word], what(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![held(word)])),
            make(4, vec![], vec![], what(Operation::Return, "ret", vec![], vec![])),
        ];
        let body = LirBody::new(
            "f",
            0,
            vec![crate::model::lir::Block::from(crate::model::lir::LirBlock::new(0, insns))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let lis = LiveIntervals::new(&body);
        let classes = RegisterClasses::m16();
        let sets = crate::backend::regclass::Sets::of(&classes);
        let tables = Tables::default();
        tables.orders.borrow_mut().insert(word, vec![RegId::EBX]);
        tables.orders.borrow_mut().insert(base, vec![RegId::EBX]);
        reclass_orders(&tables, &Default::default(), Some(&sets), classes.registers, &lis, &[word, base]);
        assert_eq!(tables.orders.borrow()[&word], sets.available, "the stored word may take any register");
        assert_eq!(
            tables.orders.borrow()[&base],
            vec![RegId::EBX, RegId::ESI, RegId::EDI],
            "an address base stays among the address registers"
        );
    }

    fn fixed_pin_slots() -> Vec<SlotIndex> {
        let insns: Vec<Arc<Insn>> = (0..40).map(|_| Arc::new(Insn::new(0, None, None, vec![], vec![]))).collect();
        let body = LirBody::new(
            "pins",
            0,
            vec![Block::from(LirBlock::new(0, insns.clone()))],
            IndexMap::default(),
            IndexMap::default(),
        );
        let indexes = SlotIndexes::new(&body);
        insns.iter().map(|one| indexes.get_instruction_index(one.id()).get_reg_slot(false)).collect()
    }

    fn span(
        reg: u32,
        from: usize,
        to: usize,
        at: &[SlotIndex],
    ) -> LiveInterval {
        let mut li = LiveInterval::new(reg, 1.0);
        let v = li.get_next_value(at[from]);
        li.add_segment(Segment::new(at[from], at[to], Some(v)));
        li
    }

    /// `FixedPins` as `decide_with_split` builds them: the merged range of each
    /// unit comes from the pins.
    fn fixed_pins(
        tri: RegisterInfo,
        pins: Vec<FixedPin>,
        copies: crate::support::hash::HashMap<SlotIndex, (u32, u32)>,
    ) -> FixedPins {
        let mut units: crate::support::hash::HashMap<u32, crate::liveness::liverange::LiveRange> = Default::default();
        for pin in &pins {
            for unit in tri.regunits(pin.root) {
                let range = units
                    .entry(unit)
                    .or_insert_with(
                        || crate::liveness::liverange::LiveRange {
                            segments: Vec::new(),
                            valnos: Vec::new(),
                        },
                    );
                for (start, end) in &pin.segments {
                    range.segments.push(crate::liveness::liverange::Segment::new(start.clone(), end.clone(), None));
                }
                range.segments.sort_by(|a, b| a.start.cmp(&b.start));
            }
        }
        FixedPins { tri, pins, units, copies, overlapping: Default::default() }
    }

    /// A value copied into the register a pinned value holds may sit in it:
    /// the overlap starts at the copy (`LiveRange::overlaps(Other, CP)`). The
    /// pinned value was a vreg that every other range had to avoid, so `k`
    /// was moved out of ecx and back with a `mov ecx, edx` per trip (x_cordic).
    /// Any other range over the pin, and the pin's register's aliases only,
    /// still meet it.
    #[test]
    fn a_range_meets_a_fixed_pin_except_at_the_copy_that_makes_it() {
        let at = fixed_pin_slots();
        let tri = RegisterInfo::new(Regs(&llrm_x86_m16::REGISTER_INFO));
        // Value 2 is pinned to ecx over [10, 20) and is made by `mov v2, v1` at
        // 10.
        let pins = fixed_pins(
            tri,
            vec![FixedPin { reg: 2, root: RegId::ECX, segments: vec![(at[10].clone(), at[20].clone())] }],
            [(at[10].clone(), (2, 1))].into_iter().collect(),
        );
        assert!(!pins.interferes(&span(1, 5, 30, &at), RegId::ECX), "the copy's source shares ecx");
        assert!(pins.interferes(&span(3, 5, 30, &at), RegId::ECX), "another range over the pin");
        assert!(!pins.interferes(&span(3, 5, 30, &at), RegId::EDX), "a register the pin does not hold");
        assert!(pins.interferes(&span(1, 12, 30, &at), RegId::ECX), "a range that starts inside, not at the copy");
    }

    /// A query for a register walked every pin of the function, 2.9 K
    /// instructions each on d_faces -O2 (10.6 M of 15 M in `checkInterference`,
    /// the pin's whole `LiveRegMatrix` costs 2.9 M): only the pins that share a
    /// unit with the register are walked.
    #[test]
    fn a_query_walks_only_the_pins_that_share_a_unit_with_the_register() {
        let at = fixed_pin_slots();
        let tri = RegisterInfo::new(Regs(&llrm_x86_m16::REGISTER_INFO));
        let pin = |reg, root| FixedPin { reg, root, segments: vec![(at[10].clone(), at[20].clone())] };
        let pins = fixed_pins(
            tri,
            vec![pin(2, RegId::ECX), pin(3, RegId::EDX), pin(4, RegId::ESI), pin(5, RegId::EDI)],
            Default::default(),
        );
        PINS_EXAMINED.with(|count| count.set(0));
        assert!(pins.interferes(&span(1, 12, 30, &at), RegId::ECX));
        assert!(pins.interferes(&span(1, 12, 30, &at), RegId::ECX));
        assert!(!pins.interferes(&span(1, 12, 30, &at), RegId::EAX));
        assert_eq!(PINS_EXAMINED.with(std::cell::Cell::get), 2, "the ecx pin, twice; none for eax");
    }

    /// A query walked every pin sharing a unit with the register even when the
    /// range lay between them all: 200 pins of ecx examined per question, 21 M
    /// instructions of `FixedPins::interferes` in jpeg_fixed -O2. The merged
    /// range of the unit rejects it first.
    #[test]
    fn a_range_between_the_pins_of_a_register_examines_none_of_them() {
        let at = fixed_pin_slots();
        let tri = RegisterInfo::new(Regs(&llrm_x86_m16::REGISTER_INFO));
        let pins = fixed_pins(
            tri,
            (0..8)
                .map(|k| FixedPin {
                    reg: 10 + k,
                    root: RegId::ECX,
                    segments: vec![(at[10 + 2 * k as usize].clone(), at[11 + 2 * k as usize].clone())],
                })
                .collect(),
            Default::default(),
        );
        PINS_EXAMINED.with(|count| count.set(0));
        assert!(!pins.interferes(&span(1, 3, 5, &at), RegId::ECX), "before every pin");
        assert!(!pins.interferes(&span(1, 11, 12, &at), RegId::ECX), "between two");
        assert_eq!(PINS_EXAMINED.with(std::cell::Cell::get), 0, "no pin walked for a range that meets none");
        assert!(pins.interferes(&span(1, 12, 13, &at), RegId::ECX), "over one");
    }
}
