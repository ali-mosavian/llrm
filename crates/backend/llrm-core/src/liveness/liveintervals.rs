//! `LiveIntervals` and the function it is computed over: the live interval of
//! every virtual register of a body, and the instructions, operands and slot
//! indexes the allocator edits as it spills. Translated from
//! llvm/lib/CodeGen/LiveIntervals.cpp (`computeVirtRegInterval`,
//! `createAndComputeVirtRegInterval`, `getInterval`, `intervalIsInOneMBB`,
//! `removeInterval`) and the parts of `MachineFunction`/`MachineRegisterInfo`
//! it reads, at d1106deb71cc.
//!
//! # How it works
//! `new` numbers the instructions (`SlotIndexes`), builds the CFG and dominator
//! tree and lists, for each value, the operands that define or read it (the
//! body's `MachineRegisterInfo`). `interval(reg)` builds the interval of one
//! value with `LiveIntervalCalc::calculate` the first time it is asked.
//! Instructions are inserted, rewritten and removed through this type
//! (`insert_instr_before`, `rewrite_instr`, `remove_instr`), which keeps the
//! slot indexes and operand lists current; the intervals are the caller's to
//! recompute, as `LiveRangeEdit` does.
//!
//! DEVIATE (no number): the operand lists are made by one pass over the
//! instructions rather than kept as use-def chains; a body with phis is not
//! translated (the allocator's input has none: PHI elimination is before it);
//! an interval is an `Rc` so the allocator can hold one while the map changes
//! (the `LiveInterval*` of C++).
//!
//! # Example
//! `%1 = 5; %2 = %1 + 1; use %2`: `%1` is `[16r,32r)`, `%2` is `[32r,48r)`.
//!
//! # Used by
//! The allocator's shadow core.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use super::cfg::{Cfg, DomTree};
use super::liverange::LiveInterval;
use super::liverangecalc::{Ctx, LiveIntervalCalc, Operand};
use super::slotindex::{InstrKey, SlotIndex, SlotIndexes};
use crate::model::lir::{Insn, LirBody};

/// `LiveIntervals` (LiveIntervals.h).
pub struct LiveIntervals {
    pub indexes: SlotIndexes,
    pub cfg: Cfg,
    pub dom: DomTree,
    /// The instructions of each block, in order (`MachineBasicBlock`).
    blocks: Vec<Vec<Arc<Insn>>>,
    operands: HashMap<u32, Vec<Operand>>,
    /// Every instruction by its key (`MachineInstr*`).
    insns: HashMap<InstrKey, Arc<Insn>>,
    /// The values defined on entry (parameters), which no instruction defines.
    inputs: std::collections::BTreeSet<u32>,
    intervals: HashMap<u32, Rc<LiveInterval>>,
    calc: LiveIntervalCalc,
    next_vreg: u32,
}

impl LiveIntervals {
    pub fn new(body: &LirBody) -> Self {
        assert!(body.blocks.iter().all(|block| block.phis.is_empty()), "phis are eliminated before allocation");
        let indexes = SlotIndexes::new(body);
        let cfg = Cfg::of(body);
        let dom = DomTree::of(&cfg);
        let mut found = Self {
            indexes,
            cfg,
            dom,
            blocks: Vec::with_capacity(body.blocks.len()),
            operands: HashMap::new(),
            insns: HashMap::new(),
            inputs: body.inputs.clone(),
            intervals: HashMap::new(),
            calc: LiveIntervalCalc::new(),
            next_vreg: 0,
        };
        for block in &body.blocks {
            let insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            for one in &insns {
                found.add_operands(one);
                found.insns.insert(one.id(), Arc::clone(one));
            }
            found.blocks.push(insns);
        }
        found.next_vreg =
            found.operands.keys().copied().chain(body.inputs.iter().copied()).max().map_or(0, |largest| largest + 1);
        found
    }

    fn add_operands(
        &mut self,
        one: &Insn,
    ) {
        let instr = one.id();
        for &value in &one.defines {
            self.operands
                .entry(value)
                .or_default()
                .push(
                    Operand {
                        instr,
                        is_def: true,
                        reads_reg: false,
                        early_clobber: false,
                    },
                );
        }
        for &value in &one.uses {
            self.operands
                .entry(value)
                .or_default()
                .push(
                    Operand {
                        instr,
                        is_def: false,
                        reads_reg: true,
                        early_clobber: false,
                    },
                );
        }
    }

    fn remove_operands(
        &mut self,
        one: &Insn,
    ) {
        let instr = one.id();
        for value in one.defines.iter().chain(&one.uses) {
            if let Some(list) = self.operands.get_mut(value) {
                list.retain(|operand| operand.instr != instr);
                if list.is_empty() {
                    self.operands.remove(value);
                }
            }
        }
    }

    /// The operands that name `reg`, in no particular order
    /// (`reg_nodbg_operands`).
    pub fn operands(
        &self,
        reg: u32,
    ) -> &[Operand] {
        self.operands.get(&reg).map_or(&[], Vec::as_slice)
    }

    /// `MRI->reg_nodbg_empty(Reg)`: nothing names the register.
    pub fn reg_nodbg_empty(
        &self,
        reg: u32,
    ) -> bool {
        !self.operands.contains_key(&reg)
    }

    /// The instruction with this key.
    pub fn instr(
        &self,
        key: InstrKey,
    ) -> &Arc<Insn> {
        &self.insns[&key]
    }

    /// The instructions of a block, in order.
    pub fn block_insns(
        &self,
        block: usize,
    ) -> &[Arc<Insn>] {
        &self.blocks[block]
    }

    /// The virtual registers that have an operand, ascending.
    pub fn virt_regs(&self) -> Vec<u32> {
        let mut regs: Vec<u32> = self.operands.keys().copied().collect();
        regs.sort_unstable();
        regs
    }

    /// The values that have an operand (unordered).
    pub fn values(&self) -> impl Iterator<Item = u32> + '_ {
        self.operands.keys().copied()
    }

    /// `createVirtualRegister`: a new register number.
    pub fn create_virtual_register(&mut self) -> u32 {
        let reg = self.next_vreg;
        self.next_vreg += 1;
        reg
    }

    /// Makes `reg` a number `create_virtual_register` will not give out again:
    /// a register another component numbered (the spiller's fresh values).
    pub fn reserve_virtual_register(
        &mut self,
        reg: u32,
    ) {
        self.next_vreg = self.next_vreg.max(reg + 1);
    }

    /// The number the next `create_virtual_register` gives.
    pub fn next_virtual_register(&self) -> u32 {
        self.next_vreg
    }

    /// The function as edited: the instructions of every block, in order.
    pub fn blocks(&self) -> &[Vec<Arc<Insn>>] {
        &self.blocks
    }

    /// Replaces the weight of an interval already computed (the spill weights
    /// are computed after the intervals).
    pub fn set_weight(
        &mut self,
        reg: u32,
        weight: f32,
    ) {
        if let Some(li) = self.intervals.get_mut(&reg) {
            Rc::make_mut(li).set_weight(weight);
        }
    }

    /// `computeVirtRegInterval` (LiveIntervals.cpp#L190): a new interval for
    /// `reg` (weight 0) from its operands.
    pub fn compute_virt_reg_interval(
        &mut self,
        reg: u32,
    ) -> LiveInterval {
        let mut li = LiveInterval::new(reg, 0.0);
        self.calc.calc.reset(self.cfg.len());
        let ctx = Ctx { cfg: &self.cfg, indexes: &self.indexes, dom: &self.dom };
        // A parameter is defined at the entry of the function, as a PHI-def is
        // at its block's start (DEVIATE: LLVM has a COPY from a
        // physical register).
        if self.inputs.contains(&reg) {
            li.create_dead_def(self.indexes.get_mbb_start_idx(self.cfg.entry));
        }
        let empty = Vec::new();
        let operands = self.operands.get(&reg).unwrap_or(&empty);
        self.calc.calculate(&mut li, operands, &ctx);
        li
    }

    /// `getInterval`/`createAndComputeVirtRegInterval`: the interval of `reg`,
    /// computed the first time.
    pub fn interval(
        &mut self,
        reg: u32,
    ) -> &LiveInterval {
        self.interval_rc(reg);
        &self.intervals[&reg]
    }

    /// The interval of `reg` as a shared pointer.
    pub fn interval_rc(
        &mut self,
        reg: u32,
    ) -> Rc<LiveInterval> {
        if !self.intervals.contains_key(&reg) {
            let li = self.compute_virt_reg_interval(reg);
            self.intervals.insert(reg, Rc::new(li));
        }
        Rc::clone(&self.intervals[&reg])
    }

    /// `hasInterval`.
    pub fn has_interval(
        &self,
        reg: u32,
    ) -> bool {
        self.intervals.contains_key(&reg)
    }

    /// `removeInterval`.
    pub fn remove_interval(
        &mut self,
        reg: u32,
    ) {
        self.intervals.remove(&reg);
    }

    /// Replaces the interval of its register (a live range that was edited).
    pub fn insert_interval(
        &mut self,
        li: LiveInterval,
    ) -> Rc<LiveInterval> {
        let reg = li.reg();
        let li = Rc::new(li);
        self.intervals.insert(reg, Rc::clone(&li));
        li
    }

    /// `intervalIsInOneMBB` (LiveIntervals.cpp): the interval lies inside one
    /// block, defined and killed at instructions, not at block boundaries.
    pub fn interval_is_in_one_mbb(
        &self,
        li: &LiveInterval,
    ) -> Option<usize> {
        if li.empty() {
            return None;
        }
        let start = li.begin_index();
        if start.is_block() {
            return None;
        }
        let stop = li.end_index();
        if stop.is_block() {
            return None;
        }
        let (a, b) = (self.indexes.get_mbb_from_index(&start), self.indexes.get_mbb_from_index(&stop));
        (a == b).then_some(a)
    }

    fn position_of(
        &self,
        block: usize,
        key: InstrKey,
    ) -> usize {
        self.blocks[block].iter().position(|one| one.id() == key).expect("an instruction of the block")
    }

    /// `insertMachineInstrInMaps` for an instruction put immediately before
    /// `before` (or at the end of the block): the instruction is in the block,
    /// the indexes and the operand lists.
    pub fn insert_instr_before(
        &mut self,
        block: usize,
        before: Option<InstrKey>,
        insn: Arc<Insn>,
    ) -> SlotIndex {
        let pos = before.map_or(self.blocks[block].len(), |key| self.position_of(block, key));
        let prev = if pos == 0 {
            self.indexes.get_mbb_start_idx(block)
        } else {
            self.indexes.get_index_before(block, &self.blocks[block], pos)
        };
        let next = match before {
            Some(key) => self.indexes.get_instruction_index(key),
            None => self.indexes.get_mbb_end_idx(block),
        };
        let index = self.indexes.insert_machine_instr_in_maps(insn.id(), block, &prev, &next, false);
        self.add_operands(&insn);
        self.insns.insert(insn.id(), Arc::clone(&insn));
        self.blocks[block].insert(pos, insn);
        index
    }

    /// An instruction put immediately after `after`.
    pub fn insert_instr_after(
        &mut self,
        block: usize,
        after: InstrKey,
        insn: Arc<Insn>,
    ) -> SlotIndex {
        let pos = self.position_of(block, after) + 1;
        let prev = self.indexes.get_instruction_index(after);
        let next = self.indexes.get_index_after(block, &self.blocks[block], pos - 1);
        let index = self.indexes.insert_machine_instr_in_maps(insn.id(), block, &prev, &next, false);
        self.add_operands(&insn);
        self.insns.insert(insn.id(), Arc::clone(&insn));
        self.blocks[block].insert(pos, insn);
        index
    }

    /// `replaceMachineInstrInMaps`: `new` takes the place and the index of
    /// `old`; the operand lists follow (an instruction that names other
    /// registers than the one it replaces).
    pub fn rewrite_instr(
        &mut self,
        block: usize,
        old: InstrKey,
        new: Arc<Insn>,
    ) -> SlotIndex {
        let pos = self.position_of(block, old);
        let before = Arc::clone(&self.blocks[block][pos]);
        self.remove_operands(&before);
        self.insns.remove(&old);
        let index = self.indexes.replace_machine_instr_in_maps(old, new.id());
        self.add_operands(&new);
        self.insns.insert(new.id(), Arc::clone(&new));
        self.blocks[block][pos] = new;
        index
    }

    /// `RemoveMachineInstrFromMaps` and erasing the instruction.
    pub fn remove_instr(
        &mut self,
        block: usize,
        key: InstrKey,
    ) {
        let pos = self.position_of(block, key);
        let gone = self.blocks[block].remove(pos);
        self.remove_operands(&gone);
        self.insns.remove(&key);
        self.indexes.remove_machine_instr_from_maps(key);
    }

    /// A point of an interval as the function's own shape names it, so two
    /// numberings of one function compare: the instruction and the slot of it,
    /// or the block and the slot of its boundary.
    fn named_point(
        &self,
        index: &SlotIndex,
    ) -> (u8, u64, u32) {
        let slot = index.get_slot() as u32;
        if let Some(instr) = self.indexes.get_instruction_from_index(index) {
            return (0, instr, slot);
        }
        for block in 0..self.blocks.len() {
            if self.indexes.get_mbb_start_idx(block) == *index {
                return (1, block as u64, slot);
            }
            if self.indexes.get_mbb_end_idx(block) == *index {
                return (2, block as u64, slot);
            }
        }
        panic!("an interval ends at an index that is neither an instruction nor a block boundary");
    }

    /// `LLRM_CHECK_LIVEINTERVALS`: every interval this holds is what
    /// `computeVirtRegInterval` makes over `body` afresh (the function as
    /// edited), compared by instruction and slot, not by number. Returns how
    /// many intervals it compared; the caller asserts it is not zero, so the
    /// check cannot pass by comparing nothing.
    pub fn assert_intervals_equal_afresh(
        &self,
        body: &LirBody,
    ) -> usize {
        let shape = |lis: &Self, li: &LiveInterval| -> Vec<_> {
            li.segments.iter().map(|seg| (lis.named_point(&seg.start), lis.named_point(&seg.end))).collect()
        };
        // The slot indexes of every `SlotIndexes` of a thread live in one
        // arena, which a new one empties: what this holds is named first.
        let mut regs: Vec<u32> = self.intervals.keys().copied().collect();
        regs.sort_unstable();
        let held: Vec<(u32, Vec<_>)> = regs.iter().map(|reg| (*reg, shape(self, &self.intervals[reg]))).collect();
        let mut fresh = Self::new(body);
        let mut compared = 0;
        for (reg, was) in held {
            let again = fresh.compute_virt_reg_interval(reg);
            let now = shape(&fresh, &again);
            if was != now {
                // The instructions that name the register, for the report.
                for operand in fresh.operands(reg) {
                    let one = fresh.instr(operand.instr);
                    eprintln!(
                        "  v{reg} named by #{} d{:?} u{:?} reload {} store {} remat {}: {:?}",
                        operand.instr, one.defines, one.uses, one.spill_reload, one.spill_store, one.rematerialized,
                        one.what.as_ref().map(|w| (&w.name, &w.dests, &w.sources))
                    );
                }
            }
            assert_eq!(
                was, now,
                "LLRM_CHECK_LIVEINTERVALS: the edited interval of v{reg} differs from the one made afresh"
            );
            compared += 1;
        }
        compared
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lir::{Block, LirBlock};
    use crate::support::hash::IndexMap;

    fn body(insns: &[Arc<Insn>]) -> LirBody {
        LirBody::new(
            "edited",
            0,
            vec![Block::from(LirBlock::new(0, insns.to_vec()))],
            IndexMap::default(),
            IndexMap::default(),
        )
    }

    fn insn(
        defines: &[u32],
        uses: &[u32],
    ) -> Arc<Insn> {
        Arc::new(Insn::new(0, None, None, defines.to_vec(), uses.to_vec()))
    }

    /// An instruction put after the last use of `%1` that reads it, without
    /// `%1`'s interval made again: `LLRM_CHECK_LIVEINTERVALS` must say the held
    /// interval differs from the one made afresh (a check that passes whatever
    /// the edit is would be no check).
    #[test]
    #[should_panic(expected = "differs from the one made afresh")]
    fn test_the_afresh_check_fails_for_an_interval_an_edit_left_stale() {
        let insns = [insn(&[1], &[]), insn(&[], &[1]), insn(&[], &[])];
        let mut lis = LiveIntervals::new(&body(&insns));
        lis.interval(1);
        let late = insn(&[], &[1]);
        lis.insert_instr_after(0, insns[2].id(), Arc::clone(&late));
        let edited = body(&[Arc::clone(&insns[0]), Arc::clone(&insns[1]), Arc::clone(&insns[2]), late]);
        lis.assert_intervals_equal_afresh(&edited);
    }

    /// The same edit with the interval made again: equal, and one compared.
    #[test]
    fn test_the_afresh_check_passes_for_an_interval_made_again_after_the_edit() {
        let insns = [insn(&[1], &[]), insn(&[], &[1]), insn(&[], &[])];
        let mut lis = LiveIntervals::new(&body(&insns));
        lis.interval(1);
        let late = insn(&[], &[1]);
        lis.insert_instr_after(0, insns[2].id(), Arc::clone(&late));
        lis.remove_interval(1);
        lis.interval(1);
        let edited = body(&[Arc::clone(&insns[0]), Arc::clone(&insns[1]), Arc::clone(&insns[2]), late]);
        assert_eq!(lis.assert_intervals_equal_afresh(&edited), 1);
    }
}
