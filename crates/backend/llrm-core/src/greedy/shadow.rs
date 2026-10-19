//! The translated greedy allocator run beside the allocator in
//! `backend::allocate`, on the same input, to compare decisions and cost. It
//! allocates nothing and rewrites nothing the caller sees: it reports what the
//! LLVM-shaped allocator would have decided (how many ranges it assigned,
//! evicted, waited, spilled) and the caller measures the cost.
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
//! `LLRM_SHADOW_GREEDY=1 llrm-c -O2 f.c` prints one line per allocation with
//! the counts.
//!
//! # Used by
//! `backend::allocate` (behind `LLRM_SHADOW_GREEDY`).

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegSet;
use llrm_lir::registers::{RegId, Regs};

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
        crate::backend::coalesce::_copy(instr)
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
}

impl BodySplitEnv {
    fn new(
        body: &LirBody,
        tables: Rc<Tables>,
    ) -> Self {
        let ats: Vec<i64> = body.blocks.iter().map(|block| block.at).collect();
        let number = ats.iter().enumerate().map(|(n, &at)| (at, n)).collect();
        Self { tables, forest: crate::analysis::loops::forest_of(&body.blocks, Some(body.entry)), ats, number }
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
        let named = lis
            .operands(from)
            .iter()
            .flat_map(|op| crate::backend::allocate::_named_widths(lis.instr(op.instr)).collect::<Vec<_>>())
            .filter(|(value, _)| *value == from)
            .map(|(_, width)| width)
            .max();
        let mut widths = self.tables.widths.borrow_mut();
        if let Some(width) = widths.get(&from).copied().or(named) {
            widths.insert(new, width);
        }
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
        crate::backend::coalesce::_copy(insn).is_some()
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
        let named = lis
            .operands(from)
            .iter()
            .flat_map(|op| crate::backend::allocate::_named_widths(lis.instr(op.instr)).collect::<Vec<_>>())
            .filter(|(value, _)| *value == from)
            .map(|(_, width)| width)
            .max();
        let width = named.or_else(|| self.tables.widths.borrow().get(&from).copied()).unwrap_or(4);
        self.tables.widths.borrow_mut().insert(to, width);
        let what = Semantics {
            name: Some("mov".to_owned()),
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

/// The calls: where registers are destroyed.
struct CallMasks {
    /// The reg slot of each instruction that destroys registers, ascending
    /// (`LIS->getRegMaskSlots()`), and what a value live across it loses:
    /// a narrow value, a wide one (the high halves too), and the registers it
    /// reads a value ending at it would lose.
    calls: Vec<(SlotIndex, RegSet, RegSet, RegSet)>,
    /// Each block's slots, start and end, and the run of `calls` in it
    /// (`getRegMaskSlotsInBlock`), in slot order.
    blocks: Vec<(SlotIndex, SlotIndex, std::ops::Range<usize>)>,
    /// The widest each value is named (bytes): a value wider than 2 loses the
    /// high half of the roots in a call's third list.
    widths: Rc<Tables>,
}

thread_local! {
    static CALLS_VISITED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
        let calls = calls
            .into_iter()
            .map(|(slot, destroyed, high, read)| {
                let narrow = destroyed.union(&read);
                (slot, narrow, narrow.union(&high), read)
            })
            .collect();
        Self { calls, blocks, widths }
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
        // what it reads.
        let mut at = calls.partition_point(|(slot, ..)| *slot < li.begin_index());
        for s in &li.segments {
            at += calls[at..].partition_point(|(slot, ..)| *slot <= s.start);
            while let Some((slot, narrow, wide_lost, _)) = calls.get(at).filter(|(slot, ..)| *slot < s.end) {
                #[cfg(test)]
                CALLS_VISITED.with(|visited| visited.set(visited.get() + 1));
                debug_assert!(s.start < *slot);
                found.get_or_insert_with(RegSet::default).union_with(if wide { wide_lost } else { narrow });
                at += 1;
            }
            match calls.get(at) {
                None => break,
                // An instruction that reads a register it names not (the data
                // segment) also takes it from a range that ends at it.
                Some((slot, _, _, read)) if *slot == s.end && !read.is_empty() => {
                    #[cfg(test)]
                    CALLS_VISITED.with(|visited| visited.set(visited.get() + 1));
                    found.get_or_insert_with(RegSet::default).union_with(read);
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
        self.calls.partition_point(|(slot, ..)| *slot < start)..self.calls.partition_point(|(slot, ..)| *slot < stop)
    }

    fn reg_mask_slot(
        &self,
        i: usize,
    ) -> SlotIndex {
        self.calls[i].0
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
        let (_, destroyed, high, _) = &self.calls[i];
        destroyed.contains(&root) || high.contains(&root)
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
pub fn decide_with_split(
    inputs: &ShadowInputs,
    mode: Mode<'_>,
    split: bool,
) -> Option<Decided> {
    let body = inputs.body;
    if body.blocks.iter().any(|block| !block.phis.is_empty()) || body.blocks.is_empty() {
        return None;
    }
    if std::env::var("LLRM_INPUT_DUMP").is_ok_and(|n| n == body.name) {
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
    let lis = LiveIntervals::new(body);
    // The environment: orders, frequencies, loop exits.
    let mbfi = crate::liveness::blockfrequency::MachineBlockFrequencyInfo::of(body);
    let freq: Vec<f32> =
        (0..body.blocks.len()).map(|block| mbfi.get_block_freq_relative_to_entry_block(block) as f32).collect();
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
    // Under `LLRM_GREEDY_RFP` the x87 values take FP0..FP6 (LLVM's RFP
    // classes); a stack position an instruction names is the stackifier's.
    let floats = if crate::backend::allocate::rfp_on() {
        crate::backend::floatassign::_floating_values(body)
    } else {
        Default::default()
    };
    for reg in lis.virt_regs() {
        let mut order: Vec<RegId> = if floats.contains(&reg) {
            crate::backend::allocate::RFP.to_vec()
        } else {
            (inputs.order)(reg).into_iter().map(crate::model::ir::root).collect()
        };
        order.dedup();
        tables.orders.borrow_mut().insert(reg, order);
    }
    for (value, width) in crate::backend::allocate::_widest(body) {
        tables.widths.borrow_mut().insert(value, width);
    }
    tables.remat.borrow_mut().extend(crate::backend::spiller::remade_values(body));
    let pins = inputs
        .pins
        .iter()
        .filter(|(reg, _)| !floats.contains(reg))
        .map(|(&reg, &phys)| (reg, crate::model::ir::root(phys)))
        .collect();
    let env = BodyEnv { orders: Rc::clone(&tables), pins, freq, exiting };
    // The calls.
    let mut calls = Vec::new();
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for (at, block) in body.blocks.iter().enumerate() {
        let first = calls.len();
        for one in block.insns.iter() {
            let read: RegSet = inputs
                .data
                .filter(|_| crate::backend::target::needs_data_group(one))
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
                    destroyed.extend(crate::backend::allocate::RFP);
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
    let matrix = LiveRegMatrix::new(
        RegisterInfo::new(inputs.registers),
        Box::new(CallMasks::of(calls, blocks, Rc::clone(&tables))),
    );
    let inline = matches!(mode, Mode::Inline { .. });
    // A split edits the function, and a caller that only records what to spill
    // writes it on the function as it was.
    let split = split && !matches!(mode, Mode::Record);
    let spiller: Box<dyn super::greedy::Spiller + '_> = match mode {
        Mode::Record => Box::new(super::spill::RecordingSpiller::default()),
        Mode::Simple => Box::new(SimpleSpiller::default()),
        Mode::Inline { frame, classes } => {
            Box::new(InlineSpiller::new(body, frame, classes, inputs.order, Rc::clone(&tables)))
        }
    };
    let mut greedy = RAGreedy::new(lis, matrix, &env, spiller);
    if split {
        let split_env = Box::new(BodySplitEnv::new(body, Rc::clone(&tables)));
        greedy.split = Some(super::regionsplit::SplitSupport::new(split_env, mbfi, &greedy.lis.cfg.succs));
    }
    greedy.calculate_spill_weights_and_hints();
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
            greedy.pinned.insert(reg);
        }
    }
    greedy.allocate_phys_regs();
    let mut signature = 0u64;
    for reg in greedy.lis.virt_regs() {
        if let Some(phys) = greedy.matrix.vrm.get_phys(reg) {
            signature = signature.wrapping_mul(1_000_003).wrapping_add(u64::from(reg) * 131 + phys.index() as u64);
        }
    }
    let (reloads, stores, folded) = greedy.spill_counts();
    let mut placed: Vec<(u32, RegId)> = Vec::new();
    for reg in greedy.lis.virt_regs() {
        if let Some(phys) = greedy.matrix.vrm.get_phys(reg) {
            placed.push((reg, phys));
        }
    }
    if std::env::var("LLRM_PLACED_DEBUG").is_ok() {
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

    /// Under `LLRM_GREEDY_RFP` an x87 value takes one of FP0..FP6 or is
    /// spilled: nine live at once over seven registers, two at least go to
    /// memory. A value that lived in an x87 register over an instruction that
    /// names a stack position (the stack must be empty there) would be lost.
    #[test]
    fn test_x87_values_take_fp0_to_fp6_and_the_rest_spill() {
        use crate::model::ir::{Addr, Held, Loc, Mem, Operation, Semantics, Space};
        let _rfp = crate::backend::allocate::RfpOn::set();
        let float = |value: u32| Loc::Held(Held { value, width: 10 });
        let cell = |disp: i64| Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, disp)), 8));
        let what = |op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>| Semantics {
            name: Some(name.to_owned()),
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
        };
        let decided = decide_with(&inputs, Mode::Simple).expect("greedy allocated the body");
        let rfp = crate::backend::allocate::RFP;
        assert!(decided.placed.iter().all(|(_, reg)| rfp.contains(reg)), "{:?}", decided.placed);
        // All nine live across the empty-stack point: none keeps a register
        // (the registers placed are the reloads' and the stores' short ranges).
        assert!(decided.placed.iter().all(|(value, _)| *value > 9), "{:?}", decided.placed);
        assert_eq!(decided.report.stats.spilled_ranges, 9, "{:?}", decided.report.stats);
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
        let calls = [5, 12, 15, 30, 40, 55, 70]
            .map(|n| (at(n), [esi].into_iter().collect::<RegSet>(), RegSet::new(), RegSet::new()))
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
            name: Some("mov".to_owned()),
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
        let env = BodySplitEnv::new(&body, Rc::clone(&tables));
        env.note_split_from(&lis, 7, 1);
        assert_eq!(tables.widths.borrow().get(&7), Some(&4), "the clone is as wide as its parent");
    }
}
