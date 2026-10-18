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

use llrm_lir::registers::{RegId, Regs};

use super::greedy::{GreedyEnv, RAGreedy, Stats};
use super::matrix::{LiveRegMatrix, PhysInterference};
use super::inlinespiller::{InlineSpiller, Tables};
use super::spill::SimpleSpiller;
use super::virtregmap::RegisterInfo;
use crate::analysis::frequency::Frequency;
use crate::liveness::calcspillweights::SpillWeightEnv;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::LiveInterval;
use crate::liveness::slotindex::SlotIndex;
use crate::backend::classes::RegisterClasses;
use crate::backend::frame::Frame;
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
        _reg: u32,
    ) -> Option<RegId> {
        None
    }

    fn is_rematerializable(
        &self,
        _instr: &Insn,
    ) -> bool {
        false
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

/// The calls: where registers are destroyed.
struct CallMasks {
    /// The reg slot of each instruction that destroys registers, ascending
    /// (`LIS->getRegMaskSlots()`), and the roots it destroys.
    calls: Vec<(SlotIndex, Vec<RegId>, Vec<RegId>, Vec<RegId>)>,
    /// The widest each value is named (bytes): a value wider than 2 loses the
    /// high half of the roots in a call's third list.
    widths: Rc<Tables>,
}

impl PhysInterference for CallMasks {
    /// `LiveIntervals::checkRegMaskInterference`: the slots are ascending, so
    /// the calls the range is live across are found by one merge with its
    /// segments.
    fn reg_mask_clobbered(
        &self,
        li: &LiveInterval,
    ) -> Option<Vec<RegId>> {
        if self.calls.is_empty() || li.empty() {
            return None;
        }
        let mut found: Option<Vec<RegId>> = None;
        let wide = self.widths.widths.borrow().get(&li.reg()).copied().unwrap_or(0) > 2;
        let mut at = self.calls.partition_point(|(slot, ..)| *slot < li.begin_index());
        let mut segment = 0usize;
        while at < self.calls.len() && segment < li.segments.len() {
            let (slot, destroyed, high, read) = &self.calls[at];
            if li.segments[segment].end < *slot {
                segment = li.advance_to(segment, slot);
                if segment == li.segments.len() {
                    break;
                }
            }
            let s = &li.segments[segment];
            // Live across: defined before the call, read after it.
            // An instruction that reads a register it names not (the data
            // segment) also takes it from a range that ends at it.
            let across = s.start < *slot && s.end > *slot;
            if across || (s.start < *slot && s.end == *slot && !read.is_empty()) {
                let roots = found.get_or_insert_with(Vec::new);
                let lost = destroyed.iter().chain(high.iter().filter(|_| wide)).filter(|_| across);
                for r in lost.chain(read) {
                    if !roots.contains(r) {
                        roots.push(*r);
                    }
                }
            }
            at += 1;
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
    let body = inputs.body;
    if body.blocks.iter().any(|block| !block.phis.is_empty()) || body.blocks.is_empty() {
        return None;
    }
    if std::env::var("LLRM_INPUT_DUMP").is_ok_and(|n| n == body.name) {
        for (b, block) in body.blocks.iter().enumerate() {
            for (at, one) in block.insns.iter().enumerate() {
                eprintln!("IN b{b}[{at}] #{} d{:?} u{:?} g{:?} {:?}", one.id(), one.defines, one.uses, one.group, one.what.as_ref().map(|w| (&w.name, &w.dests, &w.sources)));
            }
        }
    }
    let lis = LiveIntervals::new(body);
    // The environment: orders, frequencies, loop exits.
    let busy = Frequency::of(body);
    // `MachineBlockFrequencyInfo` keeps a frequency in 64 bits and saturates:
    // a deep loop nest reaches the top, never infinity, which the spill weights
    // read as a range that cannot be spilled.
    let freq: Vec<f32> = body.blocks.iter().map(|block| (busy.block(block.at) as f64).min(u64::MAX as f64) as f32).collect();
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
    for reg in lis.virt_regs() {
        let mut order: Vec<RegId> = (inputs.order)(reg).into_iter().map(crate::model::ir::root).collect();
        order.dedup();
        tables.orders.borrow_mut().insert(reg, order);
    }
    for (value, width) in crate::backend::allocate::_widest(body) {
        tables.widths.borrow_mut().insert(value, width);
    }
    let env = BodyEnv { orders: Rc::clone(&tables), freq, exiting };
    // The calls.
    let mut calls = Vec::new();
    for block in &body.blocks {
        for one in block.insns.iter() {
            let read: Vec<RegId> = inputs
                .data
                .filter(|_| crate::backend::target::needs_data_group(one))
                .map(crate::model::ir::root)
                .into_iter()
                .collect();
            if !one.clobbers.is_empty() || !one.clobbers_high.is_empty() || !read.is_empty() {
                let slot = lis.indexes.get_instruction_index(one.id()).get_reg_slot(false);
                let roots = |list: &BTreeSet<RegId>| -> Vec<RegId> {
                    list.iter().map(|r| crate::model::ir::root(*r)).collect::<BTreeSet<_>>().into_iter().collect()
                };
                calls.push((slot, roots(&one.clobbers), roots(&one.clobbers_high), read));
            }
        }
    }
    let matrix = LiveRegMatrix::new(RegisterInfo::new(inputs.registers), Box::new(CallMasks { calls, widths: Rc::clone(&tables) }));
    let inline = matches!(mode, Mode::Inline { .. });
    let spiller: Box<dyn super::greedy::Spiller + '_> = match mode {
        Mode::Record => Box::new(super::spill::RecordingSpiller::default()),
        Mode::Simple => Box::new(SimpleSpiller::default()),
        Mode::Inline { frame, classes } => {
            Box::new(InlineSpiller::new(body, frame, classes, inputs.order, Rc::clone(&tables)))
        }
    };
    let mut greedy = RAGreedy::new(lis, matrix, &env, spiller);
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
    // A value an instruction requires in one register may be given that one only:
    // its allocation order is the register (`getRegClass` of a one-register
    // class), and it competes like any other range: it can be evicted or spilled.
    for (&reg, &phys) in inputs.pins {
        if !greedy.lis.reg_nodbg_empty(reg) {
            tables.orders.borrow_mut().insert(reg, vec![crate::model::ir::root(phys)]);
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
                eprintln!("v{reg} named, no phys; interval {} stage {:?} original {}", greedy.lis.has_interval(reg), greedy.extra.get_stage(reg), greedy.matrix.vrm.get_original(reg));
            }
        }
    }
    let spilled = greedy.recorded_spills();
    let report = ShadowReport { values, stats: greedy.stats.clone(), reloads, stores, folded, signature };
    let edited = inline.then(|| {
        let blocks = body
            .blocks
            .iter()
            .zip(greedy.lis.blocks())
            .map(|(block, insns)| block.with_insns(insns.clone()))
            .collect();
        body.with_blocks(blocks)
    });
    if llrm_support::env_flag!("LLRM_CHECK_LIVEINTERVALS") {
        if let Some(edited) = &edited {
            let held = greedy.lis.interval_count();
            let compared = greedy.lis.assert_intervals_equal_afresh(edited);
            assert!(compared == held, "LLRM_CHECK_LIVEINTERVALS compared {compared} of {held} intervals of {}", body.name);
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
        let masks = CallMasks { calls: vec![(at(5), vec![], vec![esi], vec![])], widths: widths(&[(1, 4), (2, 2)]) };
        let across = |reg: u32| {
            let mut li = LiveInterval::new(reg, 1.0);
            let v = li.get_next_value(at(2));
            li.add_segment(Segment::new(at(2), at(8), Some(v)));
            li
        };
        assert_eq!(masks.reg_mask_clobbered(&across(1)), Some(vec![esi]));
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
        let masks = CallMasks { calls: vec![(at(5), vec![], vec![], vec![ds])], widths: widths(&[]) };
        let live = |to: usize| {
            let mut li = LiveInterval::new(1, 1.0);
            let v = li.get_next_value(at(2));
            li.add_segment(Segment::new(at(2), at(to), Some(v)));
            li
        };
        assert_eq!(masks.reg_mask_clobbered(&live(5)), Some(vec![ds]), "ends at the instruction");
        assert_eq!(masks.reg_mask_clobbered(&live(8)), Some(vec![ds]), "goes on past it");
        assert!(masks.reg_mask_clobbered(&live(4)).is_none_or(|roots| roots.is_empty()), "ends before it");
    }
}
