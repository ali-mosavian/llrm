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

use std::collections::{BTreeSet, HashMap};

use llrm_lir::registers::{RegId, Regs};

use super::greedy::{GreedyEnv, LiveRangeStage, RAGreedy, Stats};
use super::matrix::{LiveRegMatrix, PhysInterference};
use super::spill::SimpleSpiller;
use super::virtregmap::RegisterInfo;
use crate::analysis::frequency::Frequency;
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
    orders: HashMap<u32, Vec<RegId>>,
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
        self.orders.get(&virt).cloned().unwrap_or_default()
    }

    fn num_allocatable_regs(
        &self,
        virt: u32,
    ) -> usize {
        self.orders.get(&virt).map_or(0, Vec::len)
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
        self.orders.get(&a) == self.orders.get(&b)
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
    calls: Vec<(SlotIndex, Vec<RegId>)>,
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
        let mut at = self.calls.partition_point(|(slot, _)| *slot < li.begin_index());
        let mut segment = 0usize;
        while at < self.calls.len() && segment < li.segments.len() {
            let (slot, destroyed) = &self.calls[at];
            if li.segments[segment].end <= *slot {
                segment = li.advance_to(segment, slot);
                if segment == li.segments.len() {
                    break;
                }
            }
            let s = &li.segments[segment];
            // Live across: defined before the call, read after it.
            if s.start < *slot && s.end > *slot {
                let roots = found.get_or_insert_with(Vec::new);
                for r in destroyed {
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
    let body = inputs.body;
    if body.blocks.iter().any(|block| !block.phis.is_empty()) || body.blocks.is_empty() {
        return None;
    }
    let lis = LiveIntervals::new(body);
    // The environment: orders, frequencies, loop exits.
    let busy = Frequency::of(body);
    let freq: Vec<f32> = body.blocks.iter().map(|block| busy.block(block.at) as f32).collect();
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
    let mut orders: HashMap<u32, Vec<RegId>> = HashMap::new();
    for reg in lis.virt_regs() {
        let mut order: Vec<RegId> = (inputs.order)(reg).into_iter().map(crate::model::ir::root).collect();
        order.dedup();
        orders.insert(reg, order);
    }
    let env = BodyEnv { orders, freq, exiting };
    // The calls.
    let mut calls = Vec::new();
    for block in &body.blocks {
        for one in block.insns.iter() {
            if !one.clobbers.is_empty() {
                let slot = lis.indexes.get_instruction_index(one.id()).get_reg_slot(false);
                calls.push((
                    slot,
                    one.clobbers
                        .iter()
                        .map(|r| crate::model::ir::root(*r))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                ));
            }
        }
    }
    let matrix = LiveRegMatrix::new(RegisterInfo::new(inputs.registers), Box::new(CallMasks { calls }));
    let spiller: Box<dyn super::greedy::Spiller> = if recording {
        Box::new(super::spill::RecordingSpiller::default())
    } else {
        Box::new(SimpleSpiller::default())
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
    for (&reg, &phys) in inputs.pins {
        if greedy.lis.has_interval(reg) || !greedy.lis.reg_nodbg_empty(reg) {
            let li = greedy.lis.interval_rc(reg);
            greedy.matrix.assign(&li, crate::model::ir::root(phys));
            greedy.extra.set_stage(reg, LiveRangeStage::Done);
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
    let spilled = greedy.recorded_spills();
    let report = ShadowReport { values, stats: greedy.stats.clone(), reloads, stores, folded, signature };
    Some(Decided { report, placed, spilled })
}
