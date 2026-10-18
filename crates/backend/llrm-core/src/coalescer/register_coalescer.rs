//! `RegisterCoalescer::joinCopy` and `joinVirtRegs`: join the intervals of one
//! copy's values, or refuse. Translated from RegisterCoalescer.cpp at
//! d1106deb71cc.
//!
//! # How it works
//! `join_copy` takes one copy of the function `LiveIntervals` holds:
//!
//! 1. `CoalescerPair::set_registers`; not a move the coalescer joins, refuse.
//! 2. A copy whose result is dead is removed (DEVIATE C5).
//! 3. A copy of a value to itself (a join made it so) merges the two value
//!    numbers at the copy and goes.
//! 4. The larger interval stays: the pair is flipped when the source has more
//!    segments than the destination.
//! 5. `join_virt_regs`: map the values of both sides, prune where a `Replace`
//!    value redefines the other side, erase the copies that became identities,
//!    shrink the registers those copies read, join the ranges and extend them
//!    to the end points the pruning left.
//! 6. Every instruction that names the absorbed value names the surviving one
//!    (`updateRegDefsUses`), and the absorbed interval goes.
//!
//! DEVIATE C5 (no number): a dead copy is removed with its source shrunk and
//! the destination's interval made again, where C++ calls
//! `LiveRangeEdit::eliminateDeadDefs`. Exit: that function translated.
//!
//! DEVIATE C6 (no number): `updateRegDefsUses` sets no `undef` flags: an
//! instruction here has none, and a read of no live value is a bug of the
//! caller that `shrink_to_uses` skips.
//!
//! # Example
//! See `coalescer::tests`: `v1 := load; v2 := mov v1; use v2` becomes
//! `v1 := load; use v1` with one interval, `[load, use)`.
//!
//! # Used by
//! Nothing yet.

use std::sync::Arc;

use super::joinvals::JoinVals;
use super::pair::{CoalescerEnv, CoalescerPair};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::slotindex::InstrKey;
use crate::support::hash::{HashMap, HashSet};

/// `RegisterCoalescer::JoinResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JoinResult {
    Joined,
    Rejected,
    /// May be possible to coalesce later.
    Deferred,
}

/// `LargeIntervalSizeThreshold` and `LargeIntervalFreqThreshold`
/// (RegisterCoalescer.cpp#L111-L117): an interval with this many values has
/// been joined this many times is not worth another try.
const LARGE_INTERVAL_SIZE_THRESHOLD: usize = 100;
const LARGE_INTERVAL_FREQ_THRESHOLD: u32 = 256;

/// What the coalescer remembers between copies.
#[derive(Default)]
pub struct State {
    /// `LargeLIVisitCounter`.
    large_visits: HashMap<u32, u32>,
    /// `ErasedInstrs`.
    pub erased: HashSet<InstrKey>,
    /// Instructions renamed to a new instruction, by their old key. A
    /// `MachineInstr*` survives its operands being rewritten; a key here is
    /// the instruction's identity, so the lists of copies follow the moves.
    moved: HashMap<InstrKey, InstrKey>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// The key the instruction first known as `key` has now.
    pub fn current(
        &self,
        mut key: InstrKey,
    ) -> InstrKey {
        while let Some(next) = self.moved.get(&key) {
            key = *next;
        }
        key
    }

    /// `isHighCostLiveInterval`.
    fn is_high_cost(
        &mut self,
        reg: u32,
        valnos: usize,
    ) -> bool {
        if valnos < LARGE_INTERVAL_SIZE_THRESHOLD {
            return false;
        }
        let counter = self.large_visits.entry(reg).or_insert(0);
        if *counter < LARGE_INTERVAL_FREQ_THRESHOLD {
            *counter += 1;
            return false;
        }
        true
    }
}

/// `joinCopy`.
pub fn join_copy(
    lis: &mut LiveIntervals,
    env: &dyn CoalescerEnv,
    state: &mut State,
    current_erased: &mut HashSet<InstrKey>,
    copy: InstrKey,
) -> JoinResult {
    let copy_insn = Arc::clone(lis.instr(copy));
    let Some(mut cp) = CoalescerPair::set_registers(&copy_insn, env) else {
        return JoinResult::Rejected;
    };
    let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(copy));
    // Dead code elimination. This really should be handled by MachineDCE, but
    // sometimes dead copies slip through, and we can't generate invalid live
    // ranges.
    if lis.all_defs_are_dead(copy) {
        state.erased.insert(copy);
        lis.remove_instr(block, copy);
        lis.shrink_to_uses(cp.src_reg());
        lis.remove_interval(cp.dst_reg());
        return JoinResult::Joined;
    }
    // Coalesced copies are normally removed immediately, but a join can leave
    // an identity copy. When that happens, just join the values and remove the
    // copy.
    if cp.src_reg() == cp.dst_reg() {
        let mut li = (*lis.interval_rc(cp.src_reg())).clone();
        let copy_idx = lis.indexes.get_instruction_index(copy);
        let lrq = li.query(&copy_idx);
        if let Some(def_vni) = lrq.value_defined() {
            let read_vni = lrq.value_in().expect("No value before copy and no <undef> flag.");
            assert!(!std::rc::Rc::ptr_eq(&read_vni, &def_vni), "Cannot read and define the same value.");
            li.merge_value_number_into(def_vni, read_vni);
            lis.insert_interval(li);
        }
        state.erased.insert(copy);
        lis.remove_instr(block, copy);
        return JoinResult::Joined;
    }
    // When possible, let DstReg be the larger interval.
    if lis.interval(cp.src_reg()).size() > lis.interval(cp.dst_reg()).size() {
        cp.flip();
    }
    let result = join_virt_regs(lis, state, &cp);
    if result != JoinResult::Joined {
        return result;
    }
    // CopyMI has been erased by joinVirtRegs at this point. Remove it from
    // ErasedInstrs since the work list does not add a successful join back.
    // This keeps ErasedInstrs from growing needlessly.
    if state.erased.remove(&copy) {
        // But we may encounter the instruction again in this iteration.
        current_erased.insert(copy);
    }
    // Rewrite all SrcReg operands to DstReg.
    update_reg_defs_uses(lis, state, cp.src_reg(), cp.dst_reg());
    // SrcReg is guaranteed to be the register whose live interval that is
    // being merged.
    lis.remove_interval(cp.src_reg());
    JoinResult::Joined
}

/// `joinVirtRegs`.
fn join_virt_regs(
    lis: &mut LiveIntervals,
    state: &mut State,
    cp: &CoalescerPair,
) -> JoinResult {
    let mut rhs = (*lis.interval_rc(cp.src_reg())).clone();
    let mut lhs = (*lis.interval_rc(cp.dst_reg())).clone();
    if state.is_high_cost(cp.dst_reg(), lhs.valnos.len()) || state.is_high_cost(cp.src_reg(), rhs.valnos.len()) {
        return JoinResult::Rejected;
    }
    let mut vals = JoinVals::new(lis, cp, std::mem::take(&mut lhs.range), std::mem::take(&mut rhs.range));
    // First compute NewVNInfo and the simple value mappings. Conflicts found
    // here only reject this attempt; subsequent coalescing may still make the
    // same copy joinable, so keep it deferred.
    if !vals.map_values(0) || !vals.map_values(1) {
        return JoinResult::Deferred;
    }
    // Some conflicts can only be resolved after all values have been mapped.
    if !vals.resolve_conflicts(0) || !vals.resolve_conflicts(1) {
        return JoinResult::Deferred;
    }
    // The merging algorithm in LiveInterval::join() can't handle conflicting
    // value mappings, so we need to remove any live ranges that overlap a
    // CR_Replace resolution. Collect a set of end points that can be used to
    // restore the live range after joining.
    let mut end_points = Vec::new();
    vals.prune_values(0, &mut end_points);
    vals.prune_values(1, &mut end_points);
    // Erase COPY instructions. This may cause some external registers to
    // require trimming.
    let mut shrink_regs = Vec::new();
    vals.erase_instrs(0, &mut state.erased, &mut shrink_regs);
    vals.erase_instrs(1, &mut state.erased, &mut shrink_regs);
    let lhs_assignments = vals.sides[0].assignments.clone();
    let rhs_assignments = vals.sides[1].assignments.clone();
    let mut new_vn_info = std::mem::take(&mut vals.new_vn_info);
    let (mut lhs_range, mut rhs_range) = vals.into_ranges();
    while let Some(reg) = shrink_regs.pop() {
        lis.shrink_to_uses(reg);
    }
    // Join RHS into LHS.
    lhs_range.join(&mut rhs_range, &lhs_assignments, &rhs_assignments, &mut new_vn_info);
    if !end_points.is_empty() {
        // Recompute the parts of the live range we had to remove because of
        // CR_Replace conflicts.
        lis.extend_to_indices(&mut lhs_range, &end_points);
    }
    lhs.range = lhs_range;
    lis.insert_interval(lhs);
    JoinResult::Joined
}

/// `updateRegDefsUses`: every instruction that names `src` names `dst`.
fn update_reg_defs_uses(
    lis: &mut LiveIntervals,
    state: &mut State,
    src: u32,
    dst: u32,
) {
    let mut keys: Vec<InstrKey> = lis.operands(src).iter().map(|operand| operand.instr).collect();
    keys.sort_unstable();
    keys.dedup();
    for key in keys {
        let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(key));
        let renamed = Arc::clone(lis.instr(key)).renamed(&|value| if value == src { dst } else { value });
        let new = renamed.id();
        lis.rewrite_instr(block, key, renamed);
        state.moved.insert(key, new);
    }
}
