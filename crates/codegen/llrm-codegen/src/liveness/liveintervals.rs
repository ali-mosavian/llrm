//! `LiveIntervals` and the function it is computed over: the live interval of
//! every virtual register of a body, and the instructions, operands and slot
//! indexes the allocator edits as it spills. Translated from
//! llvm/lib/CodeGen/LiveIntervals.cpp (`computeVirtRegInterval`,
//! `createAndComputeVirtRegInterval`, `getInterval`, `intervalIsInOneMBB`,
//! `removeInterval`, `pruneValue`, `extendToIndices`, `shrinkToUses`) and the
//! parts of `MachineFunction`/`MachineRegisterInfo` it reads, at d1106deb71cc.
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
//! DEVIATE (no number): `shrinkToUses` has no sub-ranges and sets no dead flags
//! on operands (an instruction here has none); it lists the instructions whose
//! value is dead and `all_defs_are_dead` tells which can go.
//!
//! # Example
//! `%1 = 5; %2 = %1 + 1; use %2`: `%1` is `[16r,32r)`, `%2` is `[32r,48r)`.
//!
//! # Used by
//! The allocator's shadow core.

use std::rc::Rc;
use std::sync::Arc;

mod handlemove;

use super::cfg::{Cfg, DomTree};
use super::eqclasses::{ConnectedVNInfoEqClasses, distribute_range};
use super::liverange::{LiveInterval, LiveRange, Segment, VNInfo, Vn};
use super::liverangecalc::{Ctx, LiveIntervalCalc, Operand};
use super::slotindex::{InstrKey, SlotIndex, SlotIndexes};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::{HashMap, HashSet};

// How many `LiveIntervals` this thread has built, for a test that a phase that
// does not need the intervals builds none.
#[cfg(test)]
thread_local! {
    pub(crate) static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// The `LiveIntervals` a pass leaves for the next one (the pin's analysis,
// preserved across TwoAddress and the register coalescer), with the
// instructions of the body it was made for: the next pass takes it only for
// that body. One per thread, since the slot indexes of a thread share an arena.
thread_local! {
    static HANDED_OVER: std::cell::RefCell<Option<HandedOver>> = const { std::cell::RefCell::new(None) };
}

/// What a pass leaves for the next. A set still held when its thread ends is
/// leaked, not dropped: dropping a `LiveIntervals` reads the slot-index arena,
/// a thread local that may be gone by then (a coalescer run with no allocator
/// after it aborted the process: "thread local panicked on drop").
struct HandedOver {
    instructions: Vec<Vec<InstrKey>>,
    analysed: u64,
    lis: Option<LiveIntervals>,
}

impl Drop for HandedOver {
    fn drop(&mut self) {
        if let Some(lis) = self.lis.take() {
            std::mem::forget(lis);
        }
    }
}

fn instructions_of(body: &LirBody) -> Vec<Vec<InstrKey>> {
    body.blocks.iter().map(|block| block.insns.iter().map(|one| one.id()).collect()).collect()
}

/// `lis` is left for the next pass, which has `body`.
pub fn hand_over(
    lis: LiveIntervals,
    body: &LirBody,
) {
    HANDED_OVER.with(|slot| {
        *slot.borrow_mut() = Some(HandedOver {
            instructions: instructions_of(body),
            analysed: super::slotindex::analysed(),
            lis: Some(lis),
        });
    });
}

/// The `LiveIntervals` handed over for exactly this `body`, if there is one.
pub fn take_handed_over(body: &LirBody) -> Option<LiveIntervals> {
    HANDED_OVER.with(|slot| {
        let mut held = slot.borrow_mut().take()?;
        // Taken only if no other body was numbered since: its entries are the
        // arena's.
        let fits = held.instructions == instructions_of(body) && held.analysed == super::slotindex::analysed();
        let lis = held.lis.take();
        if fits { lis } else { None }
    })
}

/// The `LiveIntervals` a pass handed over, edited to `body`, where the passes
/// between only added, removed or rewrote instructions (the pin keeps one
/// analysis from the coalescer to the allocator): taken if no other body was
/// numbered since and the edit applies.
pub fn take_handed_over_edited_to(body: &LirBody) -> Option<LiveIntervals> {
    HANDED_OVER.with(|slot| {
        let mut held = slot.borrow_mut().take()?;
        if held.analysed != super::slotindex::analysed() {
            return None;
        }
        held.lis.take()?.edited_to(body)
    })
}

/// Where one instruction's operands of one value sit in the value's list:
/// nearly always one, which then costs no allocation.
enum Slots {
    One(u32),
    Many(Vec<u32>),
}

impl Slots {
    fn push(
        &mut self,
        at: u32,
    ) {
        match self {
            Slots::One(first) => *self = Slots::Many(vec![*first, at]),
            Slots::Many(list) => list.push(at),
        }
    }

    fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> {
        match self {
            Slots::One(one) => std::slice::from_mut(one).iter_mut(),
            Slots::Many(list) => list.iter_mut(),
        }
    }

    /// The positions, highest first.
    fn descending(self) -> Vec<u32> {
        match self {
            Slots::One(one) => vec![one],
            Slots::Many(mut list) => {
                list.sort_unstable_by(|a, b| b.cmp(a));
                list
            }
        }
    }
}

/// `LiveIntervals` (LiveIntervals.h).
pub struct LiveIntervals {
    pub indexes: SlotIndexes,
    pub cfg: Cfg,
    pub dom: DomTree,
    /// The instructions of each block, in order (`MachineBasicBlock`).
    blocks: Vec<Vec<Arc<Insn>>>,
    operands: HashMap<u32, Vec<Operand>>,
    /// Where each instruction's operands of a value sit in `operands`, so
    /// removing them is not a scan of the list (the use-def chain of C++
    /// unlinks in O(1)).
    slots: HashMap<(u32, InstrKey), Slots>,
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
        #[cfg(test)]
        BUILT.with(|one| one.set(one.get() + 1));
        assert!(body.blocks.iter().all(|block| block.phis.is_empty()), "phis are eliminated before allocation");
        let indexes = SlotIndexes::new(body);
        let cfg = Cfg::of(body);
        let dom = DomTree::of(&cfg);
        // The tables are sized once: an operand names one value, a value is
        // named a few times.
        let (insns, named) =
            body.blocks.iter().flat_map(|block| block.insns.iter()).fold((0, 0), |(many, named), one| {
                (many + 1, named + one.defines.len() + one.uses.len() + one.undefs.len())
            });
        let mut found = Self {
            indexes,
            cfg,
            dom,
            blocks: Vec::with_capacity(body.blocks.len()),
            operands: HashMap::with_capacity_and_hasher(named / 2, Default::default()),
            slots: HashMap::with_capacity_and_hasher(named, Default::default()),
            insns: HashMap::with_capacity_and_hasher(insns, Default::default()),
            inputs: body.inputs.clone(),
            intervals: HashMap::default(),
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
            self.push_operand(value, Operand { instr, is_def: true, reads_reg: false, early_clobber: false });
        }
        for &value in &one.uses {
            self.push_operand(value, Operand { instr, is_def: false, reads_reg: true, early_clobber: false });
        }
        // An operand flagged undef names the register and reads nothing
        // (`MachineOperand::isUndef`): it starts and extends no live range.
        for &value in &one.undefs {
            self.push_operand(value, Operand { instr, is_def: false, reads_reg: false, early_clobber: false });
        }
    }

    fn push_operand(
        &mut self,
        value: u32,
        operand: Operand,
    ) {
        let list = self.operands.entry(value).or_default();
        let at = list.len() as u32;
        match self.slots.entry((value, operand.instr)) {
            std::collections::hash_map::Entry::Occupied(mut one) => one.get_mut().push(at),
            std::collections::hash_map::Entry::Vacant(none) => {
                none.insert(Slots::One(at));
            }
        }
        list.push(operand);
    }

    fn remove_operands(
        &mut self,
        one: &Insn,
    ) {
        llrm_mir::counts::bump(llrm_mir::counts::Event::LirOperandListEdits);
        let instr = one.id();
        for &value in one.defines.iter().chain(&one.uses).chain(&one.undefs) {
            // A value named twice finds its slots gone the second time.
            let Some(mine) = self.slots.remove(&(value, instr)) else { continue };
            let list = self.operands.get_mut(&value).expect("a slot names a listed operand");
            // Highest first: the operand swapped in is then never one of ours.
            for at in mine.descending() {
                let at = at as usize;
                let last = list.len() - 1;
                list.swap_remove(at);
                if at != last {
                    let moved = self.slots.get_mut(&(value, list[at].instr)).expect("a listed operand has a slot");
                    *moved.iter_mut().find(|slot| **slot as usize == last).expect("the slot of the last operand") =
                        at as u32;
                }
            }
            if list.is_empty() {
                self.operands.remove(&value);
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

    /// Whether an instruction with this key is still in the function.
    pub fn has_instr(
        &self,
        key: InstrKey,
    ) -> bool {
        self.insns.contains_key(&key)
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

    /// This function edited to `body`, the same blocks with instructions added,
    /// removed or replaced: the instructions go in and out through the maps as
    /// the spiller's do, and the interval of every register one of them names
    /// is made again when asked for, where the rest stay as they are. None if
    /// the blocks, the inputs or the order of the instructions both bodies keep
    /// differ.
    pub fn edited_to(
        mut self,
        body: &LirBody,
    ) -> Option<Self> {
        if body.blocks.len() != self.blocks.len() || body.inputs != self.inputs {
            return None;
        }
        let mut touched: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for (n, block) in body.blocks.iter().enumerate() {
            let wanted: HashSet<InstrKey> = block.insns.iter().map(|one| one.id()).collect();
            let gone: Vec<InstrKey> =
                self.blocks[n].iter().map(|one| one.id()).filter(|key| !wanted.contains(key)).collect();
            for key in gone {
                let insn = Arc::clone(&self.insns[&key]);
                touched.extend(insn.defines.iter().chain(&insn.uses).copied());
                self.remove_instr(n, key);
            }
            let mut previous: Option<InstrKey> = None;
            for one in block.insns.iter() {
                if self.insns.contains_key(&one.id()) {
                    previous = Some(one.id());
                    continue;
                }
                touched.extend(one.defines.iter().chain(&one.uses).copied());
                match previous {
                    Some(after) => {
                        self.insert_instr_after(n, after, Arc::clone(one));
                    }
                    None => {
                        let first = self.blocks[n].first().map(|first| first.id());
                        self.insert_instr_before(n, first, Arc::clone(one));
                    }
                }
                previous = Some(one.id());
            }
            if !self.blocks[n].iter().map(|one| one.id()).eq(block.insns.iter().map(|one| one.id())) {
                return None;
            }
        }
        for reg in touched {
            self.intervals.remove(&reg);
            self.next_vreg = self.next_vreg.max(reg + 1);
        }
        Some(self)
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
        #[cfg(test)]
        COMPUTED.with(|one| one.borrow_mut().push(reg));
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

    /// `getInterval` of a register whose interval is held (`const`).
    pub fn get_interval(
        &self,
        reg: u32,
    ) -> &LiveInterval {
        &self.intervals[&reg]
    }

    /// `getInterval` as a shared pointer, of a register whose interval is held.
    pub fn get_interval_rc(
        &self,
        reg: u32,
    ) -> Rc<LiveInterval> {
        Rc::clone(&self.intervals[&reg])
    }

    /// `getInterval` to edit: a copy is made first if a queue still shares it.
    pub fn interval_mut(
        &mut self,
        reg: u32,
    ) -> &mut LiveInterval {
        Rc::make_mut(self.intervals.get_mut(&reg).expect("an interval is held"))
    }

    /// Takes the interval of `reg` out of the map, to edit it beside the maps.
    pub fn take_interval(
        &mut self,
        reg: u32,
    ) -> LiveInterval {
        let rc = self.intervals.remove(&reg).expect("an interval is held");
        Rc::try_unwrap(rc).unwrap_or_else(|shared| (*shared).clone())
    }

    /// `removeVRegDefAt`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveIntervals.cpp#L1848>),
    /// without sub-ranges.
    pub fn remove_vreg_def_at(
        li: &mut LiveInterval,
        pos: &SlotIndex,
    ) {
        if let Some(vni) = li.get_vn_info_at(pos) {
            assert!(vni.def().get_base_index() == pos.get_base_index());
            li.remove_val_no(&vni);
        }
    }

    /// `splitSeparateComponents`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveIntervals.cpp#L1866>)
    /// with `ConnectedVNInfoEqClasses::Distribute`: a register whose values
    /// are not connected becomes one register per component. Returns the new
    /// registers (component 0 stays); their intervals are held, their operands
    /// rewritten.
    pub fn split_separate_components(
        &mut self,
        reg: u32,
    ) -> Vec<u32> {
        let mut li = self.take_interval(reg);
        let mut con = ConnectedVNInfoEqClasses::new();
        let num_comp = con.classify(&li.range, &self.indexes, &self.cfg);
        if num_comp <= 1 {
            self.insert_interval(li);
            return Vec::new();
        }
        let new_regs: Vec<u32> = (1..num_comp).map(|_| self.create_virtual_register()).collect();

        // Rewrite instructions: each goes with the value it reads, else the
        // one it defines.
        let mut seen: Vec<(InstrKey, bool)> = Vec::new();
        for op in self.operands(reg) {
            match seen.iter_mut().find(|(k, _)| *k == op.instr) {
                Some(entry) => entry.1 |= op.reads_reg,
                None => seen.push((op.instr, op.reads_reg)),
            }
        }
        for (key, reads) in seen {
            let idx = self.indexes.get_instruction_index(key);
            let q = li.query(&idx);
            let vni = if reads { q.value_in() } else { q.value_defined() };
            // An undef use that isn't tied to any def has no value.
            let Some(vni) = vni else { continue };
            let class = con.get_eq_class(&vni);
            if class > 0 {
                let to = new_regs[class as usize - 1];
                let block = self.indexes.get_mbb_from_index(&idx);
                let old = Arc::clone(self.instr(key));
                let new = old.renamed(&|v| if v == reg { to } else { v });
                self.rewrite_instr(block, key, new);
            }
        }

        // Distribute the main live range.
        let mut splits: Vec<LiveRange> = (1..num_comp).map(|_| LiveRange::new()).collect();
        distribute_range(&mut li.range, &mut splits, &|id| con.class_of_id(id));
        for (&new, range) in new_regs.iter().zip(splits) {
            let mut made = LiveInterval::new(new, 0.0);
            made.range = range;
            self.insert_interval(made);
        }
        self.insert_interval(li);
        new_regs
    }

    /// How many intervals are held.
    pub fn interval_count(&self) -> usize {
        self.intervals.len()
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

    /// The position in `block` of the first instruction whose index is `at`
    /// or later (`strict`: later than `at`). The block is in slot order.
    pub fn partition_at(
        &self,
        block: usize,
        at: &SlotIndex,
        strict: bool,
    ) -> usize {
        self.blocks[block].partition_point(|one| {
            let here = self.indexes.get_instruction_index(one.id());
            if strict { here <= *at } else { here < *at }
        })
    }

    fn position_of(
        &self,
        block: usize,
        key: InstrKey,
    ) -> usize {
        // The block is in slot order, so the instruction is found by bisection
        // (LLVM's list gives the position for free); a parallel copy shares
        // one index, so the run of equal ones is searched.
        let at = self.indexes.get_instruction_index(key);
        let insns = &self.blocks[block];
        let first = insns.partition_point(|one| self.indexes.get_instruction_index(one.id()) < at);
        insns[first..]
            .iter()
            .take_while(|one| self.indexes.get_instruction_index(one.id()) == at)
            .position(|one| one.id() == key)
            .map(|run| first + run)
            .or_else(|| insns.iter().position(|one| one.id() == key))
            .expect("an instruction of the block")
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
        self.insert_instr_before_at(block, before, insn, false)
    }

    /// `insertMachineInstrInMaps(MI, Late)` for an instruction put immediately
    /// before `before` (or at the end of the block): with `late` its index is
    /// immediately before `before`'s, after any null entries.
    pub fn insert_instr_before_at(
        &mut self,
        block: usize,
        before: Option<InstrKey>,
        insn: Arc<Insn>,
        late: bool,
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
        let index = self.indexes.insert_machine_instr_in_maps(insn.id(), block, &prev, &next, late);
        llrm_mir::counts::bump(llrm_mir::counts::Event::LirOperandListEdits);
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
        llrm_mir::counts::bump(llrm_mir::counts::Event::LirOperandListEdits);
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
        llrm_mir::counts::bump(llrm_mir::counts::Event::LirRewrites);
        let pos = self.position_of(block, old);
        let before = Arc::clone(&self.blocks[block][pos]);
        self.remove_operands(&before);
        self.insns.remove(&old);
        let index = self.indexes.replace_machine_instr_in_maps(old, new.id());
        llrm_mir::counts::bump(llrm_mir::counts::Event::LirOperandListEdits);
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

    /// Erases `key`, which defines a value nothing reads, and cuts back the
    /// ranges of what it read to their other reads (`shrinkToUses`, as
    /// `LiveRangeEdit::eliminateDeadDefs` does): left alone they would end at
    /// the index of an instruction that is gone. Returns the instructions that
    /// now define a value nothing reads.
    pub fn erase_dead_def(
        &mut self,
        key: InstrKey,
    ) -> Vec<InstrKey> {
        self.erase_dead_defs(&[key])
    }

    /// `LiveRangeEdit::eliminateDeadDefs`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveRangeEdit.cpp#L367>)
    /// for one round: every instruction of `keys` goes, then each register they
    /// read is cut back once, however many of them read it (`ToShrink` is a
    /// set). Returns the instructions that now define a value nothing reads.
    ///
    /// DEVIATE (no number): LLVM shrinks one register, erases the dead defs
    /// that makes, and shrinks the next; here a round shrinks them all and
    /// the caller erases the new dead defs as the next round. `foldAsLoad`
    /// and `splitSeparateComponents` are the callers' (the spill's and the
    /// editor's).
    pub fn erase_dead_defs(
        &mut self,
        keys: &[InstrKey],
    ) -> Vec<InstrKey> {
        let mut to_shrink: Vec<u32> = Vec::new();
        for &key in keys {
            let at = self.indexes.get_instruction_index(key);
            let block = self.indexes.get_mbb_from_index(&at);
            let (defined, sources) = {
                let insn = self.instr(key);
                (insn.defines.clone(), insn.uses.clone())
            };
            for reg in defined {
                if self.has_interval(reg) {
                    Self::remove_vreg_def_at(self.interval_mut(reg), &at.get_reg_slot(false));
                }
            }
            self.remove_instr(block, key);
            for source in sources {
                if !to_shrink.contains(&source) {
                    to_shrink.push(source);
                }
            }
        }
        let mut dead = Vec::new();
        for source in to_shrink {
            if self.has_interval(source) {
                dead.extend(self.shrink_to_uses(source).1);
            }
        }
        dead
    }

    /// `body` with the instructions as edited through this (same blocks, same
    /// edges): what `assert_intervals_equal_afresh` compares with.
    pub fn body_like(
        &self,
        body: &LirBody,
    ) -> LirBody {
        let blocks =
            body.blocks.iter().enumerate().map(|(n, block)| block.with_insns(self.blocks[n].clone())).collect();
        LirBody { inputs: self.inputs.clone(), ..body.with_blocks(blocks) }
    }

    /// The instruction at `def` that defines `reg`: the one a value of `reg`
    /// starts at. At a bundle's index (a parallel copy shares one) there are
    /// several, and `getInstructionFromIndex` names the first.
    pub fn defining_instr(
        &self,
        def: &SlotIndex,
        reg: u32,
    ) -> InstrKey {
        // Nearly every index holds one instruction.
        if !self.indexes.is_bundled(def) {
            return self.indexes.get_instruction_from_index(def).expect("No instruction defining live value");
        }
        let all = self.indexes.get_instructions_at(def);
        all.iter().copied().find(|key| self.insns[key].defines.contains(&reg)).unwrap_or_else(|| all[0])
    }

    /// Whether `reg` is a parameter, defined on entry to the function.
    pub fn is_input(
        &self,
        reg: u32,
    ) -> bool {
        self.inputs.contains(&reg)
    }

    /// A parameter that joined another value is that value now (the rename
    /// `updateRegDefsUses` makes in every instruction, for the entry's def).
    pub fn rename_input(
        &mut self,
        from: u32,
        to: u32,
    ) {
        if self.inputs.remove(&from) {
            self.inputs.insert(to);
        }
    }

    /// `extendToIndices` (LiveIntervals.cpp#L695): extends `lr` to reach each
    /// of `indices`, as `LiveRangeCalc::extend` does for a use.
    pub fn extend_to_indices(
        &mut self,
        lr: &mut LiveRange,
        indices: &[SlotIndex],
    ) {
        self.calc.calc.reset(self.cfg.len());
        let ctx = Ctx { cfg: &self.cfg, indexes: &self.indexes, dom: &self.dom };
        for idx in indices {
            self.calc.calc.extend(lr, *idx, &ctx);
        }
    }

    /// `pruneValue` (LiveIntervals.cpp#L704): removes the part of `lr` that
    /// the value live at `kill` reaches from `kill` on, and puts the places
    /// where it ended in `end_points`.
    pub fn prune_value(
        &self,
        lr: &mut LiveRange,
        kill: &SlotIndex,
        mut end_points: Option<&mut Vec<SlotIndex>>,
    ) {
        let lrq = lr.query(kill);
        // LR may have liveness reachable from early clobber slot, which may be
        // only live-in instead of live-out of the instruction.
        let Some(vni) = lrq.value_out_or_dead().or_else(|| lrq.value_in()) else { return };
        let kill_mbb = self.indexes.get_mbb_from_index(kill);
        let mbb_end = self.indexes.get_mbb_end_idx(kill_mbb);
        // If VNI isn't live out from KillMBB, the value is trivially pruned.
        if lrq.end_point() < mbb_end {
            lr.remove_segment(kill, &lrq.end_point(), false);
            if let Some(points) = end_points.as_deref_mut() {
                points.push(lrq.end_point());
            }
            return;
        }
        // VNI is live out of KillMBB.
        lr.remove_segment(kill, &mbb_end, false);
        if let Some(points) = end_points.as_deref_mut() {
            points.push(mbb_end);
        }
        // Every block reachable from KillMBB without leaving VNI's live range.
        // KillMBB itself may be reachable, so a search starts at each
        // successor (`df_ext_begin` with one `Visited` set).
        let mut visited = vec![false; self.cfg.len()];
        for &succ in &self.cfg.succs[kill_mbb] {
            if std::mem::replace(&mut visited[succ], true) {
                continue;
            }
            let mut stack: Vec<(usize, usize)> = Vec::new();
            if self.prune_block(lr, &vni, succ, end_points.as_deref_mut()) {
                stack.push((succ, 0));
            }
            while let Some((block, next)) = stack.last_mut() {
                let Some(&child) = self.cfg.succs[*block].get(*next) else {
                    stack.pop();
                    continue;
                };
                *next += 1;
                if !std::mem::replace(&mut visited[child], true)
                    && self.prune_block(lr, &vni, child, end_points.as_deref_mut())
                {
                    stack.push((child, 0));
                }
            }
        }
    }

    /// One block of `pruneValue`'s search: whether the search goes on into its
    /// successors.
    fn prune_block(
        &self,
        lr: &mut LiveRange,
        vni: &Vn,
        block: usize,
        end_points: Option<&mut Vec<SlotIndex>>,
    ) -> bool {
        let (start, end) = *self.indexes.get_mbb_range(block);
        // Check if VNI is live in to MBB.
        let lrq = lr.query(&start);
        if !lrq.value_in().is_some_and(|found| Rc::ptr_eq(&found, vni)) {
            // This block isn't part of the VNI segment. Prune the search.
            return false;
        }
        // Prune the search if VNI is killed in MBB.
        if lrq.end_point() < end {
            lr.remove_segment(&start, &lrq.end_point(), false);
            if let Some(points) = end_points {
                points.push(lrq.end_point());
            }
            return false;
        }
        // VNI is live through MBB.
        lr.remove_segment(&start, &end, false);
        if let Some(points) = end_points {
            points.push(end);
        }
        true
    }

    /// `extendSegmentsToUses` (LiveIntervals.cpp#L443) for the main range:
    /// `segments` gets every block from the value's def to the places in
    /// `work`, going by the values `old` has at the ends of predecessors.
    fn extend_segments_to_uses(
        &self,
        segments: &mut LiveRange,
        work: &mut Vec<(SlotIndex, Vn)>,
        old: &LiveRange,
    ) {
        // Keep track of the PHIs that are in use.
        let mut used_phis: HashSet<*const VNInfo> = HashSet::default();
        // Blocks that have already been added to the work list as live-out.
        let mut live_out: HashSet<usize> = HashSet::default();
        while let Some((idx, vni)) = work.pop() {
            let block = self.indexes.get_mbb_from_index(&idx.get_prev_slot());
            let block_start = self.indexes.get_mbb_start_idx(block);
            // Extend the live range for VNI to be live at Idx.
            if let Some(ext) = segments.extend_in_block(&block_start, &idx) {
                assert!(Rc::ptr_eq(&ext, &vni), "Unexpected existing value number");
                // Is this a PHIDef we haven't seen before?
                if !vni.is_phi_def() || vni.def() != block_start || !used_phis.insert(Rc::as_ptr(&vni)) {
                    continue;
                }
                // The PHI is live, make sure the predecessors are live-out.
                for &pred in &self.cfg.preds[block] {
                    if !live_out.insert(pred) {
                        continue;
                    }
                    let stop = self.indexes.get_mbb_end_idx(pred);
                    // A predecessor is not required to have a live-out value
                    // for a PHI.
                    if let Some(pred_vni) = old.get_vn_info_before(&stop) {
                        work.push((stop, pred_vni));
                    }
                }
                continue;
            }
            // VNI is live-in to MBB.
            segments.add_segment(Segment::new(block_start, idx, Some(Rc::clone(&vni))));
            // Make sure VNI is live-out from the predecessors.
            for &pred in &self.cfg.preds[block] {
                if !live_out.insert(pred) {
                    continue;
                }
                let stop = self.indexes.get_mbb_end_idx(pred);
                let old_vni = old.get_vn_info_before(&stop).expect("Missing value out of predecessor for main range");
                assert!(Rc::ptr_eq(&old_vni, &vni), "Wrong value out of predecessor");
                work.push((stop, Rc::clone(&vni)));
            }
        }
    }

    /// `shrinkToUses` (LiveIntervals.cpp#L524): the interval of `reg` cut back
    /// to what its reads reach. Returns whether a value died (the interval may
    /// now fall into components) and the instructions that define a value
    /// nothing reads (`computeDeadValues`); `all_defs_are_dead` says which of
    /// them can go.
    pub fn shrink_to_uses(
        &mut self,
        reg: u32,
    ) -> (bool, Vec<InstrKey>) {
        #[cfg(test)]
        SHRUNK.with(|one| one.borrow_mut().push(reg));
        let mut li = (*self.interval_rc(reg)).clone();
        // Find all the values used, including PHI kills.
        let mut work: Vec<(SlotIndex, Vn)> = Vec::new();
        // Visit all instructions reading the register.
        for operand in self.operands(reg) {
            if !operand.reads_reg {
                continue;
            }
            let mut idx = self.indexes.get_instruction_index(operand.instr).get_reg_slot(false);
            let lrq = li.query(&idx);
            // A read of no live value is an operand that claims a read it does
            // not make.
            let Some(vni) = lrq.value_in() else { continue };
            // Special case: An early-clobber tied operand reads and writes the
            // register one slot early.
            if let Some(def) = lrq.value_defined() {
                idx = def.def();
            }
            work.push((idx, vni));
        }
        // Create new live ranges with only minimal live segments per def.
        let mut fresh = LiveRange::new();
        for vni in &li.valnos {
            if vni.is_unused() {
                continue;
            }
            let def = vni.def();
            fresh.add_segment(Segment::new(def, def.get_dead_slot(), Some(Rc::clone(vni))));
        }
        self.extend_segments_to_uses(&mut fresh, &mut work, &li);
        // Move the trimmed segments back.
        li.segments = fresh.segments;
        // Handle dead values.
        let (may_split, dead) = self.compute_dead_values(&mut li);
        self.intervals.insert(reg, Rc::new(li));
        (may_split, dead)
    }

    /// `computeDeadValues` (LiveIntervals.cpp#L582) without the dead flags
    /// (DEVIATE: an instruction here has no per-operand flags; the dead
    /// definitions are returned): a dead phi is removed, a dead def is listed.
    fn compute_dead_values(
        &self,
        li: &mut LiveInterval,
    ) -> (bool, Vec<InstrKey>) {
        let mut may_have_split_components = false;
        let mut dead = Vec::new();
        for vni in li.valnos.clone() {
            if vni.is_unused() {
                continue;
            }
            let def = vni.def();
            let at = li.find_segment_containing(&def);
            assert!(at != li.segments.len(), "Missing segment for VNI");
            if li.segments[at].end != def.get_dead_slot() {
                continue;
            }
            if vni.is_phi_def() {
                // This is a dead PHI. Remove it.
                vni.mark_unused();
                li.remove_segment_at(at, false);
            } else {
                // This is a dead def.
                let instr = self.defining_instr(&def, li.reg());
                dead.push(instr);
            }
            may_have_split_components = true;
        }
        (may_have_split_components, dead)
    }

    /// `MachineInstr::allDefsAreDead` for the instruction `key`: every value it
    /// defines is dead at it, by the intervals held.
    pub fn all_defs_are_dead(
        &mut self,
        key: InstrKey,
    ) -> bool {
        let defines = self.insns[&key].defines.clone();
        let idx = self.indexes.get_instruction_index(key).get_reg_slot(false);
        !defines.is_empty() && defines.into_iter().all(|reg| self.interval_rc(reg).query(&idx).is_dead_def())
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
        self.check_afresh(body, false)
    }

    /// `assert_intervals_equal_afresh` for intervals a join made: the points
    /// live are compared, segments that touch counted as one. A join keeps a
    /// PHI-def whose predecessors all carry the same value, which a fresh
    /// computation does not make (`LiveInterval::verify` does not ask for the
    /// minimal form).
    pub fn assert_intervals_cover_afresh(
        &self,
        body: &LirBody,
    ) -> usize {
        self.check_afresh(body, true)
    }

    fn check_afresh(
        &self,
        body: &LirBody,
        merge_touching: bool,
    ) -> usize {
        let shape = |lis: &Self, li: &LiveInterval| -> Vec<_> {
            let mut made: Vec<(_, _)> = Vec::new();
            for seg in &li.segments {
                let (start, end) = (lis.named_point(&seg.start), lis.named_point(&seg.end));
                match made.last_mut() {
                    Some((_, last_end)) if merge_touching && *last_end == start => *last_end = end,
                    _ => made.push((start, end)),
                }
            }
            made
        };
        // The slot indexes of every `SlotIndexes` of a thread live in one
        // arena, which a new one empties: what this holds is named first.
        // A register nothing names holds the range it had when it was split
        // (`isOriginalEndpoint` reads it): it is no longer computed from
        // anything, so it is not compared.
        let mut regs: Vec<u32> = self.intervals.keys().copied().filter(|r| !self.reg_nodbg_empty(*r)).collect();
        regs.sort_unstable();
        let held: Vec<(u32, Vec<_>)> = regs.iter().map(|reg| (*reg, shape(self, &self.intervals[reg]))).collect();
        let mut fresh = Self::new(body);
        let mut compared = 0;
        for (reg, was) in held {
            let again = fresh.compute_virt_reg_interval(reg);
            let now = shape(&fresh, &again);
            if was != now {
                if llrm_support::env_flag!("LLRM_AFRESH_DUMP") {
                    eprintln!(
                        "  held interval of v{reg}: {:?}",
                        self.intervals[&reg].segments.iter().map(|g| (g.start, g.end)).collect::<Vec<_>>()
                    );
                    for (b, insns) in self.blocks.iter().enumerate() {
                        if insns.iter().any(|one| one.uses.contains(&reg) || one.defines.contains(&reg)) {
                            eprintln!(
                                "  block {b} start {:?} end {:?}",
                                self.indexes.get_mbb_start_idx(b),
                                self.indexes.get_mbb_end_idx(b)
                            );
                            for (at, one) in insns.iter().enumerate() {
                                eprintln!(
                                    "    [{at}] #{} {:?} d{:?} u{:?} reload {} store {} {:?}",
                                    one.id(),
                                    self.indexes.get_instruction_index(one.id()),
                                    one.defines,
                                    one.uses,
                                    one.spill_reload,
                                    one.spill_store,
                                    one.what.as_ref().map(|w| (&w.name, &w.dests, &w.sources))
                                );
                            }
                        }
                    }
                }
                // The instructions that name the register, for the report.
                for operand in fresh.operands(reg) {
                    let one = fresh.instr(operand.instr);
                    eprintln!(
                        "  v{reg} named by #{} d{:?} u{:?} reload {} store {} remat {}: {:?}",
                        operand.instr,
                        one.defines,
                        one.uses,
                        one.spill_reload,
                        one.spill_store,
                        one.rematerialized,
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

    fn undef_reader(
        defines: &[u32],
        undefs: &[u32],
    ) -> Arc<Insn> {
        let mut one = Insn::new(0, None, None, defines.to_vec(), Vec::new());
        one.undefs = undefs.to_vec();
        Arc::new(one)
    }

    /// The allocator starts from the coalescer's intervals, edited to the body
    /// the passes between made: an instruction put in, one taken out, and the
    /// registers they name made again. Every interval then covers what a fresh
    /// build gives, and the untouched ones are the same objects.
    #[test]
    fn test_intervals_edited_to_the_next_body_are_those_a_fresh_build_covers() {
        let (def_a, def_b, use_both) = (insn(&[1], &[]), insn(&[2], &[]), insn(&[], &[1, 2]));
        let before = body(&[Arc::clone(&def_a), Arc::clone(&def_b), Arc::clone(&use_both)]);
        let mut lis = LiveIntervals::new(&before);
        let kept = lis.interval_rc(1);
        lis.interval(2);
        // A copy of v2 into v3 put in, the read of v2 now reads v3, and v1
        // stays.
        let copy = insn(&[3], &[2]);
        let reader = insn(&[], &[1, 3]);
        let after = body(&[Arc::clone(&def_a), Arc::clone(&def_b), Arc::clone(&copy), Arc::clone(&reader)]);
        let mut edited = lis.edited_to(&after).expect("the edit applies");
        for reg in [1, 2, 3] {
            edited.interval(reg);
        }
        assert_eq!(edited.assert_intervals_cover_afresh(&after), 3, "v1, v2 and v3 are compared");
        assert!(!Rc::ptr_eq(&kept, &edited.get_interval_rc(1)), "v1 is named by the new reader: made again");
        // A body whose shared instructions run in another order is refused.
        let swapped = body(&[Arc::clone(&def_b), Arc::clone(&def_a), Arc::clone(&copy), Arc::clone(&reader)]);
        assert!(LiveIntervals::new(&after).edited_to(&swapped).is_none());
    }

    /// An instruction taken out of the maps took its undef operands with it:
    /// the operand of a value read as undef stayed listed, and the
    /// coalescer asked the maps for an instruction that was gone
    /// ("Instruction not found in maps").
    #[test]
    fn test_removing_an_instruction_removes_its_undef_operands() {
        let (def, reader) = (insn(&[1], &[]), undef_reader(&[2], &[9]));
        let mut lis = LiveIntervals::new(&body(&[Arc::clone(&def), Arc::clone(&reader)]));
        assert_eq!(lis.operands(9).len(), 1);
        lis.remove_instr(0, reader.id());
        assert!(lis.operands(9).is_empty(), "an operand of a removed instruction is still listed");
    }

    /// A value an operand reads as undef (`ProcessImplicitDefs`) has no def and
    /// none is needed: no segment is made for it, where a plain read of a
    /// value nothing defines stops the calculation ("a unique value"),
    /// and the values around it keep theirs.
    #[test]
    fn test_a_value_read_as_undef_has_no_live_range_and_costs_its_reader_nothing() {
        let insns = [insn(&[1], &[]), undef_reader(&[2], &[9]), insn(&[], &[1, 2])];
        let mut lis = LiveIntervals::new(&body(&insns));
        assert!(lis.interval(9).empty(), "an undef read starts no range");
        assert!(!lis.interval(1).empty() && !lis.interval(2).empty());
        assert_eq!(lis.operands(9).len(), 1, "the operand is there for the register rewrite to find");
        assert!(!lis.operands(9)[0].reads_reg);
    }

    /// A join that renames a value renames the one an operand reads as undef
    /// too: left alone, the operand would name a value nothing places.
    #[test]
    fn test_renaming_a_value_renames_the_undef_reads_of_it() {
        let one = undef_reader(&[2], &[9]);
        let renamed = one.renamed(&|value| if value == 9 { 4 } else { value });
        assert_eq!(renamed.undefs, vec![4]);
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

    /// D-SE7's cost is a number: an instruction replaced in its block and the
    /// operand lists it edits are counted (`LLRM_DEBUG=counts`: d_faces -O2 has
    /// 2,387 rewrites and 6,006 operand-list edits), as the renames that
    /// rebuild an instruction are (`lir_rename_rebuilds`).
    #[test]
    fn test_a_rewrite_counts_itself_and_the_operand_lists_it_edits() {
        use llrm_mir::counts::{Event, get};
        let insns = [insn(&[1], &[]), insn(&[2], &[1]), insn(&[], &[2])];
        let mut lis = LiveIntervals::new(&body(&insns));
        let (rewrites, edits) = (get(Event::LirRewrites), get(Event::LirOperandListEdits));
        lis.rewrite_instr(0, insns[1].id(), insn(&[2], &[1]));
        assert_eq!(get(Event::LirRewrites) - rewrites, 1);
        assert_eq!(get(Event::LirOperandListEdits) - edits, 2, "the old operands out, the new in");
    }

    /// A dead copy that was the last read of `%1` goes, and `%1` ends at its
    /// last read left, not at the index of the copy: a split of QCport
    /// d_sprite.c -Os asked `enterIntvAfter` for that index and panicked.
    #[test]
    fn test_erasing_a_dead_def_cuts_back_the_range_of_what_it_read() {
        let insns = [insn(&[1], &[]), insn(&[], &[1]), insn(&[2], &[1]), insn(&[], &[])];
        let mut lis = LiveIntervals::new(&body(&insns));
        lis.interval(1);
        lis.interval(2);
        let end_before = lis.get_interval(1).end_index();
        lis.erase_dead_def(insns[2].id());
        let edited = body(&[Arc::clone(&insns[0]), Arc::clone(&insns[1]), Arc::clone(&insns[3])]);
        lis.remove_interval(2);
        assert!(lis.get_interval(1).end_index() < end_before, "premise: the range was long");
        assert_eq!(lis.assert_intervals_equal_afresh(&edited), 1);
    }

    /// Three dead copies that all read `%1` cut `%1` back once, not three times
    /// (`eliminateDeadDefs` shrinks each register of a round once): d_faces
    /// -O2 shrank one interval 22 times, each walk longer than the last, 6 M of
    /// the 17 M `SplitEditor::finish` cost against the pin's 0.35 M.
    #[test]
    fn test_dead_copies_reading_one_register_shrink_it_once() {
        let insns =
            [insn(&[1], &[]), insn(&[2], &[1]), insn(&[3], &[1]), insn(&[4], &[1]), insn(&[], &[1]), insn(&[], &[])];
        let mut lis = LiveIntervals::new(&body(&insns));
        for reg in 1..=4 {
            lis.interval(reg);
        }
        SHRUNK.with(|one| one.borrow_mut().clear());
        lis.erase_dead_defs(&[insns[1].id(), insns[2].id(), insns[3].id()]);
        assert_eq!(SHRUNK.with(|one| one.borrow().clone()), [1]);
    }

    fn blocks(shape: &[(&[Arc<Insn>], &[i64])]) -> LirBody {
        let made = shape
            .iter()
            .enumerate()
            .map(|(at, (insns, succ))| {
                Block::from(LirBlock { succ: succ.to_vec().into(), ..LirBlock::new(at as i64, insns.to_vec()) })
            })
            .collect();
        LirBody::new("edited", 0, made, IndexMap::default(), IndexMap::default())
    }

    fn reg_slot(
        lis: &LiveIntervals,
        insn: &Arc<Insn>,
    ) -> SlotIndex {
        lis.indexes.get_instruction_index(insn.id()).get_reg_slot(false)
    }

    /// `pruneValue` from the middle of a block that falls into a join: the
    /// value goes from the kill to the end of its block and from the join's
    /// start to its last read, and stays on the path that skips the block.
    #[test]
    fn test_prune_value_removes_what_the_value_reaches_after_the_kill() {
        let (def, nop, other, read) = (insn(&[1], &[]), insn(&[], &[]), insn(&[], &[]), insn(&[], &[1]));
        let body = blocks(&[
            (&[Arc::clone(&def)], &[1, 2]),
            (&[Arc::clone(&nop), Arc::clone(&other)], &[2]),
            (&[Arc::clone(&read)], &[]),
        ]);
        let mut lis = LiveIntervals::new(&body);
        let mut range = lis.interval(1).range.clone();
        let kill = reg_slot(&lis, &nop);
        let mut ends = Vec::new();
        lis.prune_value(&mut range, &kill, Some(&mut ends));
        assert_eq!(ends, vec![lis.indexes.get_mbb_end_idx(1), reg_slot(&lis, &read)]);
        assert!(
            range.live_at(&lis.indexes.get_mbb_end_idx(0).get_prev_slot()),
            "the path that skips the block keeps the value"
        );
        assert!(!range.live_at(&kill), "the kill is pruned");
        assert!(!range.live_at(&lis.indexes.get_mbb_start_idx(2)), "the join no longer holds the value");
    }

    /// `shrinkToUses` after the last read of a value in a loop exit is gone:
    /// the interval is what a fresh computation makes.
    #[test]
    fn test_shrink_to_uses_cuts_the_interval_back_to_the_reads_left() {
        let (def, inside, outside) = (insn(&[1], &[]), insn(&[], &[1]), insn(&[], &[1]));
        let was =
            blocks(&[(&[Arc::clone(&def)], &[1]), (&[Arc::clone(&inside)], &[1, 2]), (&[Arc::clone(&outside)], &[])]);
        let mut lis = LiveIntervals::new(&was);
        lis.interval(1);
        lis.remove_instr(2, outside.id());
        let now = blocks(&[(&[Arc::clone(&def)], &[1]), (&[Arc::clone(&inside)], &[1, 2]), (&[], &[])]);
        let (_, dead) = lis.shrink_to_uses(1);
        assert!(dead.is_empty());
        assert_eq!(lis.assert_intervals_equal_afresh(&now), 1);
    }

    /// A value whose reads are all gone is a dead def: `shrinkToUses` lists
    /// the instruction that defines it.
    #[test]
    fn test_shrink_to_uses_lists_the_definition_of_a_value_nothing_reads() {
        let (def, read) = (insn(&[1], &[]), insn(&[], &[1]));
        let mut lis = LiveIntervals::new(&body(&[Arc::clone(&def), Arc::clone(&read)]));
        lis.interval(1);
        lis.remove_instr(0, read.id());
        let (split, dead) = lis.shrink_to_uses(1);
        assert!(split);
        assert_eq!(dead, vec![def.id()]);
        assert!(lis.all_defs_are_dead(def.id()));
    }

    /// `extendToIndices` reaches a read added after the interval was made.
    #[test]
    fn test_extend_to_indices_reaches_a_read_added_later() {
        let (def, read, last) = (insn(&[1], &[]), insn(&[], &[1]), insn(&[], &[]));
        let mut lis = LiveIntervals::new(&body(&[Arc::clone(&def), Arc::clone(&read), Arc::clone(&last)]));
        let mut range = lis.interval(1).range.clone();
        let late = insn(&[], &[1]);
        lis.insert_instr_after(0, last.id(), Arc::clone(&late));
        let at = reg_slot(&lis, &late);
        lis.extend_to_indices(&mut range, &[at]);
        let mut held = (*lis.interval_rc(1)).clone();
        held.range = range;
        lis.insert_interval(held);
        let edited = body(&[def, read, last, late]);
        assert_eq!(lis.assert_intervals_equal_afresh(&edited), 1);
    }
}

// The registers `compute_virt_reg_interval` worked out, for a test that a spill
// works out only the registers it makes.
#[cfg(test)]
thread_local! {
    /// The registers `shrink_to_uses` was asked about, in order.
    static SHRUNK: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
thread_local! {
    pub(crate) static COMPUTED: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}
