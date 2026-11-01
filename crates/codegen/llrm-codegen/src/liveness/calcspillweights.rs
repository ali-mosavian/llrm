//! Spill weights and copy hints of live intervals: `VirtRegAuxInfo`, translated
//! from llvm/include/llvm/CodeGen/CalcSpillWeights.h and
//! llvm/lib/CodeGen/CalcSpillWeights.cpp, and `LiveIntervals::getSpillWeight`
//! (llvm/lib/CodeGen/LiveIntervals.cpp#L965), at d1106deb71cc.
//!
//! # How it works
//! A value's weight is what spilling it costs per unit of live range: each
//! instruction that reads or writes it adds the instruction's block frequency
//! (twice for a read and a write), a store that updates a value at a loop exit
//! counts three times, the sum is weakly raised if the value has copy hints,
//! halved when every definition can be rematerialised, and divided by
//! `size + 25 * InstrDist` so small intervals are mostly proportional to their
//! uses. A tiny interval not live across a call cannot be spilled to any
//! profit and is marked not spillable. The weight is stored in the interval;
//! the allocator reads it, it does not work it out again.
//!
//! ```text
//!   weight = sum(uses * block frequency) * hint boost * remat / (size + 400)
//! ```
//!
//! DEVIATE (target interface, no number): `TargetInstrInfo`, `MachineLoopInfo`,
//! `MachineBlockFrequencyInfo` and the register class's spill weight scale are
//! asked of `SpillWeightEnv`, which the body's analyses implement; sub-register
//! hints, statepoints, inline asm folding and `ProfileSummaryInfo` are not
//! translated (no such constructs).
//!
//! # Example
//! A value written once in a loop-exit block of frequency 8 and read once in
//! the entry block, live over 20 instructions: `(3 * 8 + 1) / (320 + 400)`.
//!
//! # Used by
//! The allocator's shadow core.

use std::collections::BTreeMap;

use llrm_lir::registers::RegId;

use super::liveintervals::LiveIntervals;
use super::liverange::LiveInterval;
use super::slotindex::{InstrKey, SlotIndex};
use crate::support::hash::HashMap;

/// What each instruction does to the register whose operands these are
/// (`readsWritesVirtualRegister`): reads, writes, in order of first appearance.
/// The operands of one instruction need not be next to each other: a join that
/// renames another register into this one appends the renamed operands.
pub(crate) fn by_instruction(operands: &[crate::liveness::liverangecalc::Operand]) -> Vec<(InstrKey, bool, bool)> {
    let mut out: Vec<(InstrKey, bool, bool)> = Vec::with_capacity(operands.len());
    // Kept only for a long list: a short one is searched, and the operands of
    // one instruction are mostly next to each other.
    let mut at: HashMap<InstrKey, usize> = HashMap::default();
    for op in operands {
        let slot = match out.last() {
            Some(last) if last.0 == op.instr => out.len() - 1,
            _ if out.len() <= 16 => match out.iter().position(|one| one.0 == op.instr) {
                Some(slot) => slot,
                None => {
                    out.push((op.instr, false, false));
                    out.len() - 1
                }
            },
            _ => {
                if at.is_empty() {
                    at.extend(out.iter().enumerate().map(|(slot, one)| (one.0, slot)));
                }
                *at.entry(op.instr).or_insert_with(|| {
                    out.push((op.instr, false, false));
                    out.len() - 1
                })
            }
        };
        out[slot].1 |= op.reads_reg;
        out[slot].2 |= op.is_def;
    }
    out
}

/// `normalizeSpillWeight` (CalcSpillWeights.h#L35): the weight of
/// `use_def_freq` over an interval of `size`.
pub fn normalize_spill_weight(
    use_def_freq: f32,
    _size: u32,
    _num_instr: u32,
) -> f32 {
    // The constant 25 instructions is added to avoid depending too much on
    // accidental SlotIndex gaps for small intervals. The effect is that small
    // intervals have a spill weight that is mostly proportional to the number
    // of uses, while large intervals get a spill weight that is closer to a use
    // density.
    use_def_freq / (_size as f32 + 25.0 * SlotIndex::INSTR_DIST as f32)
}

/// `LiveIntervals::getSpillWeight` (LiveIntervals.cpp#L965).
pub fn get_spill_weight(
    is_def: bool,
    is_use: bool,
    block_freq_relative_to_entry: f32,
    opt_for_size: bool,
) -> f32 {
    let weight = f32::from(is_def) + f32::from(is_use);
    // When optimizing for size we only consider the codesize impact of spilling
    // the register, not the runtime impact.
    if opt_for_size {
        return weight;
    }
    weight * block_freq_relative_to_entry
}

/// A hint from a copy: another virtual register, or a physical register.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Hint {
    Virt(u32),
    Phys(RegId),
}

/// What `VirtRegAuxInfo` asks of the target and the body's analyses.
pub trait SpillWeightEnv {
    /// `MBFI.getBlockFreqRelativeToEntryBlock`.
    fn block_freq_relative_to_entry(
        &self,
        block: usize,
    ) -> f32;
    /// `Loop->isLoopExiting(MBB)` of the innermost loop of `block`.
    fn is_loop_exiting(
        &self,
        block: usize,
    ) -> bool;
    /// `TII.isCopyInstr`: the (destination, source) values of a plain copy
    /// between two values.
    fn copy_of(
        &self,
        instr: &crate::model::lir::Insn,
    ) -> Option<(u32, u32)>;
    /// The register a value is pinned to, where it is (a physical register
    /// hint).
    fn pinned(
        &self,
        reg: u32,
    ) -> Option<RegId>;
    /// `TII.isReMaterializable(MI)`.
    fn is_rematerializable(
        &self,
        instr: &crate::model::lir::Insn,
    ) -> bool;
    /// `LIS.getRegMaskSlots()`: where a call destroys registers.
    fn reg_mask_slots(&self) -> Vec<SlotIndex>;
    /// What one reference to `reg` in `instr` adds to its weight, as a
    /// multiple of LLVM's one: a target whose instruction takes the operand
    /// from memory as cheaply (`fadd m`) says less, one that pays a copy to
    /// read it from a register says more.
    fn reference_weight(
        &self,
        _instr: &crate::model::lir::Insn,
        _reg: u32,
    ) -> f32 {
        1.0
    }
    /// `TRI.getSpillWeightScaleFactor(RC)` of the value's register class.
    fn spill_weight_scale(
        &self,
        reg: u32,
    ) -> f32;
    /// `MRI.isAllocatable(HintReg)`.
    fn is_allocatable(
        &self,
        reg: RegId,
    ) -> bool;
}

/// `VirtRegAuxInfo` (CalcSpillWeights.h#L47): the spill weight and allocation
/// hints of virtual registers.
pub struct VirtRegAuxInfo<'a> {
    pub env: &'a dyn SpillWeightEnv,
    /// `VRM.getOriginal`: the register a split interval came from.
    pub original: &'a HashMap<u32, u32>,
    pub opt_for_size: bool,
    /// `MRI.getRegAllocationHint`, filled in by the weight calculation.
    pub hints: BTreeMap<u32, Vec<Hint>>,
}

impl<'a> VirtRegAuxInfo<'a> {
    pub fn new(
        env: &'a dyn SpillWeightEnv,
        original: &'a HashMap<u32, u32>,
    ) -> Self {
        Self { env, original, opt_for_size: false, hints: BTreeMap::new() }
    }

    fn original_of(
        &self,
        reg: u32,
    ) -> u32 {
        self.original.get(&reg).copied().unwrap_or(reg)
    }

    /// `calculateSpillWeightsAndHints` (CalcSpillWeights.cpp#L45): for every
    /// virtual register that has an operand.
    pub fn calculate_spill_weights_and_hints(
        &mut self,
        lis: &mut LiveIntervals,
    ) {
        let mut regs: Vec<u32> = lis.values().collect();
        regs.sort_unstable();
        for reg in regs {
            let mut li = lis.interval(reg).clone();
            self.calculate_spill_weight_and_hint(&mut li, lis);
            lis.set_weight(reg, li.weight());
        }
    }

    /// `calculateSpillWeightAndHint` (CalcSpillWeights.cpp#L235).
    pub fn calculate_spill_weight_and_hint(
        &mut self,
        li: &mut LiveInterval,
        lis: &mut LiveIntervals,
    ) {
        let weight = self.weight_calc_helper(li, lis);
        // Check if unspillable.
        if weight < 0.0 {
            return;
        }
        li.set_weight(weight);
    }

    /// `copyHint` (CalcSpillWeights.cpp#L57): the preferred allocation register
    /// for `reg`, given a copy.
    fn copy_hint(
        &self,
        copy: (u32, u32),
        reg: u32,
    ) -> Option<Hint> {
        let h_reg = if copy.0 == reg { copy.1 } else { copy.0 };
        if let Some(phys) = self.env.pinned(h_reg) {
            return Some(Hint::Phys(phys));
        }
        Some(Hint::Virt(h_reg))
    }

    /// `isRematerializable` (CalcSpillWeights.cpp#L84): every value of `li` is
    /// defined by an instruction that can be rematerialised.
    pub fn is_rematerializable(
        &self,
        li: &LiveInterval,
        lis: &mut LiveIntervals,
    ) -> bool {
        let reg = li.reg();
        let original = self.original_of(reg);
        let mut vni_defs: HashMap<u32, InstrKey> = HashMap::default();
        for vni in &li.valnos {
            let mut vni = vni.clone();
            let orig_vni = vni.clone();
            if vni.is_unused() {
                continue;
            }
            if vni.is_phi_def() {
                return false;
            }
            let mut instr = lis.indexes.get_instruction_from_index(&vni.def()).expect("Dead valno in interval");
            // Trace copies introduced by live range splitting. The inline
            // spiller can rematerialize through these copies, so the spill
            // weight must reflect this.
            let mut cur = reg;
            while let Some((dest, source)) = self.env.copy_of(lis.instr(instr)) {
                // The copy destination must match the interval register.
                if dest != cur {
                    return false;
                }
                // Get the source register.
                cur = source;
                // If the original (pre-splitting) registers match this
                // copy came from a split.
                if self.original_of(cur) != original {
                    return false;
                }
                // Follow the copy live-in value.
                let src_li = lis.interval(cur).clone();
                let src_q = src_li.query(&vni.def());
                // DEVIATE (no number): `%1 = COPY undef %1`, a copy whose
                // source is not live at it, is a value no
                // register carries. The pin's coalescer erases such copies
                // before it joins them (`eliminateUndefCopy`, ported in the
                // coalescer); a copy that reaches here by another path is
                // not rematerializable.
                let Some(source_value) = src_q.value_in() else { return false };
                vni = source_value;
                if vni.is_phi_def() {
                    return false;
                }
                instr = lis.indexes.get_instruction_from_index(&vni.def()).expect("Dead valno in interval");
            }
            if !self.env.is_rematerializable(lis.instr(instr)) {
                return false;
            }
            vni_defs.insert(orig_vni.id(), instr);
        }
        // If MI has register uses, it will only be rematerializable if its uses
        // are also live at the indices it will be rematerialized at.
        let operands: Vec<_> = lis.operands(li.reg()).to_vec();
        for mo in operands {
            if !mo.reads_reg {
                continue;
            }
            let use_idx = lis.indexes.get_instruction_index(mo.instr);
            let def = vni_defs[&li.get_vn_info_at(&use_idx).expect("Use with no def").id()];
            if !self.all_uses_available_at(def, &use_idx, lis) {
                return false;
            }
        }
        true
    }

    /// `allUsesAvailableAt` (CalcSpillWeights.cpp#L156): whether every register
    /// the instruction `instr` reads has, at `use_idx`, the value it had at the
    /// instruction.
    pub fn all_uses_available_at(
        &self,
        instr: InstrKey,
        use_idx: &SlotIndex,
        lis: &mut LiveIntervals,
    ) -> bool {
        let orig_idx = lis.indexes.get_instruction_index(instr).get_reg_slot(true);
        let use_idx = std::cmp::max(*use_idx, use_idx.get_reg_slot(true));
        let uses: Vec<u32> = lis.instr(instr).uses.clone();
        for reg in uses {
            let li = lis.interval(reg).clone();
            let Some(ovni) = li.get_vn_info_at(&orig_idx) else { continue };
            // Don't allow rematerialization immediately after the original def.
            // It would be incorrect if OrigMI redefines the register.
            // See PR14098.
            if SlotIndex::is_same_instr(&orig_idx, &use_idx) {
                return false;
            }
            let at_use = li.get_vn_info_at(&use_idx);
            if !at_use.as_ref().is_some_and(|v| std::rc::Rc::ptr_eq(v, &ovni)) {
                return false;
            }
        }
        true
    }

    /// `weightCalcHelper` (CalcSpillWeights.cpp#L289): the weight of `li`, or a
    /// negative number if it cannot be spilled.
    pub fn weight_calc_helper(
        &mut self,
        li: &mut LiveInterval,
        lis: &mut LiveIntervals,
    ) -> f32 {
        let mut total_weight = 0.0f32;
        let mut num_instr = 0u32; // Number of instructions using LI
        // Don't recompute spill weight for an unspillable register.
        if li.is_spillable() {
            // li comes from a split of OrigInt. If OrigInt was marked
            // as not spillable, make sure the new interval is marked
            // as not spillable as well.
            let original = self.original_of(li.reg());
            if original != li.reg() && !lis.interval(original).is_spillable() {
                li.mark_not_spillable();
            }
        }
        let is_spillable = li.is_spillable();
        let mut is_exiting = false;
        let mut mbb: Option<usize> = None;
        let mut hint: HashMap<Hint, f32> = HashMap::default();
        let mut hint_order: Vec<Hint> = Vec::new();
        // The instructions that name the register, once each, in body order.
        for (instr, reads, writes) in by_instruction(lis.operands(li.reg())) {
            num_instr += 1;
            let insn = lis.instr(instr).clone();
            let copy = self.env.copy_of(&insn);
            let identity_copy = copy.is_some_and(|(dest, source)| dest == source);
            if identity_copy {
                continue;
            }
            let mut weight = 1.0f32;
            if is_spillable {
                // Get loop info for mi.
                let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(instr));
                if mbb != Some(block) {
                    mbb = Some(block);
                    is_exiting = self.env.is_loop_exiting(block);
                }
                // Calculate instr weight.
                weight =
                    get_spill_weight(writes, reads, self.env.block_freq_relative_to_entry(block), self.opt_for_size)
                        * self.env.reference_weight(&insn, li.reg());
                // Give extra weight to what looks like a loop induction
                // variable update.
                if writes && is_exiting && is_live_out_of_mbb(li, block, lis) {
                    weight *= 3.0;
                }
                total_weight += weight;
            }
            // Get allocation hints from copies.
            let Some(copy) = copy else { continue };
            if let Some(hint_reg) = self.copy_hint(copy, li.reg()) {
                let allocatable = match hint_reg {
                    Hint::Virt(_) => true,
                    Hint::Phys(reg) => self.env.is_allocatable(reg),
                };
                if allocatable {
                    if !hint.contains_key(&hint_reg) {
                        hint_order.push(hint_reg);
                    }
                    *hint.entry(hint_reg).or_insert(0.0) += weight;
                }
            }
        }
        // Pass all the sorted copy hints to mri.
        if !hint.is_empty() {
            let mut reg_hints: Vec<(Hint, f32)> = hint_order.iter().map(|h| (*h, hint[h])).collect();
            // CopyHint::operator<: any physreg first, then the heavier, then by
            // register.
            reg_hints.sort_by(|a, b| {
                let (pa, pb) = (matches!(a.0, Hint::Phys(_)), matches!(b.0, Hint::Phys(_)));
                pb.cmp(&pa)
                    .then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                    .then_with(|| hint_key(&a.0).cmp(&hint_key(&b.0)))
            });
            self.hints.insert(li.reg(), reg_hints.into_iter().map(|(h, _)| h).collect());
            // Weakly boost the spill weight of hinted registers.
            total_weight *= 1.01;
        }
        // If the live interval was already unspillable, leave it that way.
        if !is_spillable {
            return -1.0;
        }
        // Mark li as unspillable if all live ranges are tiny and the interval
        // is not live at any reg mask. If the interval is live at a reg mask
        // spilling may be required.
        if li.is_zero_length(&lis.indexes) && !li.is_live_at_indexes(&self.env.reg_mask_slots()) {
            li.mark_not_spillable();
            return -1.0;
        }
        // If all of the definitions of the interval are re-materializable,
        // it is a preferred candidate for spilling.
        if self.is_rematerializable(li, lis) {
            total_weight *= 0.5;
        }
        // Finally, we scale the weight by the scale factor of register class.
        total_weight *= self.env.spill_weight_scale(li.reg());
        normalize_spill_weight(total_weight, li.get_size(), num_instr)
    }
}

fn hint_key(hint: &Hint) -> (u8, u32) {
    match hint {
        Hint::Virt(reg) => (1, *reg),
        Hint::Phys(reg) => (0, reg.index() as u32),
    }
}

/// `LiveIntervals::isLiveOutOfMBB`: whether `li` is live out of the block.
fn is_live_out_of_mbb(
    li: &LiveInterval,
    block: usize,
    lis: &LiveIntervals,
) -> bool {
    li.live_at(&lis.indexes.get_mbb_last_idx(block))
}

#[cfg(test)]
mod tests {

    /// The operands of `add v1, v1`'s register were listed as use, another
    /// instruction's, def: read as two groups, the def was dropped as a repeat
    /// and the induction variable of x_fir lost its x3 for an update at the
    /// loop exit (weight 0.0041 against the pin's 0.025, the index spilled
    /// before each use, #4269's AM-IO-2).
    #[test]
    fn test_the_operands_of_one_instruction_need_not_be_adjacent() {
        use crate::liveness::liverangecalc::Operand;
        let operand = |instr, is_def, reads_reg| Operand { instr, is_def, reads_reg, early_clobber: false };
        let got = by_instruction(&[operand(7, false, true), operand(9, false, true), operand(7, true, false)]);
        assert_eq!(got, vec![(7, true, true), (9, true, false)]);
    }
    use std::sync::Arc;

    use super::*;
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    struct Env {
        freq: Vec<f32>,
        /// What a reference weighs (`reference_weight`): 1 as in LLVM.
        reference: f32,
    }

    impl SpillWeightEnv for Env {
        fn block_freq_relative_to_entry(
            &self,
            block: usize,
        ) -> f32 {
            self.freq[block]
        }

        fn is_loop_exiting(
            &self,
            _block: usize,
        ) -> bool {
            false
        }

        fn copy_of(
            &self,
            instr: &Insn,
        ) -> Option<(u32, u32)> {
            instr.copy_of()
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

        fn reference_weight(
            &self,
            _instr: &Insn,
            _reg: u32,
        ) -> f32 {
            self.reference
        }

        fn is_allocatable(
            &self,
            _reg: RegId,
        ) -> bool {
            true
        }
    }

    fn insn(
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, defines, uses))
    }

    fn body(blocks: Vec<Vec<Arc<Insn>>>) -> LirBody {
        let blocks = blocks
            .into_iter()
            .enumerate()
            .map(|(at, insns)| {
                let mut block = LirBlock::new(at as i64, insns);
                if at + 1 < 2 {
                    block.succ = vec![at as i64 + 1].into();
                }
                Block::from(block)
            })
            .collect();
        LirBody::new("weights", 0, blocks, IndexMap::default(), IndexMap::default())
    }

    /// `(uses * block frequency + defs) / (size + 25 * InstrDist)`, by hand: a
    /// value defined in the entry block and read twice in a block that runs
    /// four times.
    #[test]
    fn test_a_weight_is_the_block_frequencies_of_its_references_over_size_and_the_constant() {
        let body = body(vec![vec![insn(vec![1], vec![])], vec![insn(vec![], vec![1]), insn(vec![], vec![1])]]);
        let mut lis = LiveIntervals::new(&body);
        let env = Env { freq: vec![1.0, 4.0], reference: 1.0 };
        let original = HashMap::default();
        let mut aux = VirtRegAuxInfo::new(&env, &original);
        aux.calculate_spill_weights_and_hints(&mut lis);
        let li = lis.interval(1).clone();
        assert_eq!(li.get_size(), 48);
        assert!((li.weight() - 9.0 / 448.0).abs() < 1e-6, "{}", li.weight());
    }

    /// A target that reads an operand from memory as cheaply as from a register
    /// says a reference weighs less: the value is worth less to keep.
    #[test]
    fn test_a_target_can_weigh_a_reference_less_than_the_pin_does() {
        let body = body(vec![vec![insn(vec![1], vec![])], vec![insn(vec![], vec![1]), insn(vec![], vec![1])]]);
        let weight = |reference| {
            let mut lis = LiveIntervals::new(&body);
            let env = Env { freq: vec![1.0, 4.0], reference };
            let original = HashMap::default();
            VirtRegAuxInfo::new(&env, &original).calculate_spill_weights_and_hints(&mut lis);
            lis.interval(1).weight()
        };
        assert!((weight(0.5) - weight(1.0) / 2.0).abs() < 1e-6, "{} {}", weight(0.5), weight(1.0));
    }

    /// A value that lives for one instruction and is not live over a call is
    /// not worth spilling: it is marked not spillable.
    #[test]
    fn test_a_tiny_interval_is_not_spillable() {
        let body = body(vec![vec![insn(vec![1], vec![]), insn(vec![], vec![1])], vec![]]);
        let mut lis = LiveIntervals::new(&body);
        let env = Env { freq: vec![1.0, 1.0], reference: 1.0 };
        let original = HashMap::default();
        let mut aux = VirtRegAuxInfo::new(&env, &original);
        aux.calculate_spill_weights_and_hints(&mut lis);
        assert!(!lis.interval(1).is_spillable());
    }

    /// Copy hints are collected for both ends of a copy, heavier first.
    /// A join left `%1 = COPY undef %1` (its source not live there): tracing
    /// the copies for a rematerializable definition found no value and
    /// panicked "Copy from non-existing value" on bench/crc at -O2 (m16 and
    /// m32).
    #[test]
    fn test_a_copy_of_an_undefined_value_is_not_rematerializable() {
        let mut copy = (*Arc::new(Insn::new(0, None, None, vec![1], vec![]))).clone();
        copy.what = Some(crate::model::ir::Semantics {
            name: Some("mov".into()),
            dests: vec![crate::model::ir::Loc::Held(crate::model::ir::Held { value: 1, width: 4 })],
            sources: vec![crate::model::ir::Loc::Held(crate::model::ir::Held { value: 1, width: 4 })],
            ..crate::model::ir::Semantics::new(crate::model::ir::Operation::Move)
        });
        let body = body(vec![vec![Arc::new(copy), insn(vec![], vec![1])], vec![]]);
        let mut lis = LiveIntervals::new(&body);
        let env = Env { freq: vec![1.0, 1.0], reference: 1.0 };
        let original = HashMap::default();
        let aux = VirtRegAuxInfo::new(&env, &original);
        let li = (*lis.interval_rc(1)).clone();
        assert!(!aux.is_rematerializable(&li, &mut lis));
    }

    #[test]
    fn test_a_copy_hints_each_end_to_the_other() {
        let mut copy = (*Arc::new(Insn::new(0, None, None, vec![2], vec![1]))).clone();
        copy.what = Some(crate::model::ir::Semantics {
            name: Some("mov".into()),
            dests: vec![crate::model::ir::Loc::Held(crate::model::ir::Held { value: 2, width: 4 })],
            sources: vec![crate::model::ir::Loc::Held(crate::model::ir::Held { value: 1, width: 4 })],
            ..crate::model::ir::Semantics::new(crate::model::ir::Operation::Move)
        });
        let body = body(vec![
            vec![insn(vec![1], vec![]), insn(vec![], vec![]), Arc::new(copy), insn(vec![], vec![2])],
            vec![],
        ]);
        let mut lis = LiveIntervals::new(&body);
        let env = Env { freq: vec![1.0, 1.0], reference: 1.0 };
        let original = HashMap::default();
        let mut aux = VirtRegAuxInfo::new(&env, &original);
        aux.calculate_spill_weights_and_hints(&mut lis);
        assert_eq!(aux.hints[&1], vec![Hint::Virt(2)]);
        assert_eq!(aux.hints[&2], vec![Hint::Virt(1)]);
    }
}
