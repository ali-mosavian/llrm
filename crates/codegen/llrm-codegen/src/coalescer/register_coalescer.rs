//! `RegisterCoalescer::joinCopy` and `joinVirtRegs`: join the intervals of one
//! copy's values, or refuse. Translated from RegisterCoalescer.cpp at
//! d1106deb71cc.
//!
//! # How it works
//! `join_copy` takes one copy of the function `LiveIntervals` holds:
//!
//! 1. `CoalescerPair::set_registers`; not a move the coalescer joins, refuse.
//! 2. A copy whose result is dead is removed, and what that leaves dead with it
//!    (`eliminateDeadDefs`).
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
//! DEVIATE C10 (no number): `reMaterializeDef` in a build for size makes a copy
//! of a constant again only where the source has no other reader, so its own
//! move goes. Fact: elsewhere it replaced a 2-byte copy with a 3 to 5 byte
//! move and cost 40 of 813 sources +1,383 bytes (`regparm3_matrix_*` up to
//! +89) for -0.8% executed instructions. Exit: the allocator gives the constant
//! back as a copy where its register is wanted.
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

use std::rc::Rc;
use std::sync::Arc;

use super::joinvals::JoinVals;
use super::pair::{CoalescerEnv, CoalescerPair, full_copy};
use crate::backend::twoaddr::commutative;
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::{LiveInterval, Segment, Vn};
use crate::liveness::slotindex::InstrKey;
use crate::model::ir::Loc;
use crate::support::hash::{HashMap, HashSet};

/// Whether `one` does nothing but define its values: a copy or a definition
/// the spiller could make again (`isTriviallyReMaterializable`).
fn effect_free(one: &crate::model::lir::Insn) -> bool {
    one.copy_of().is_some() || crate::greedy::inlinespiller::remat_shape(one)
}

/// `RegisterCoalescer::eliminateDeadDefs` (`LiveRangeEdit::eliminateDeadDefs`
/// over `DeadDefs`): the instructions of `dead` whose values nothing reads go,
/// the registers they read are cut back, and what that leaves dead goes in
/// turn.
fn eliminate_dead_defs(
    lis: &mut LiveIntervals,
    state: &mut State,
    mut dead: Vec<InstrKey>,
) {
    while !dead.is_empty() {
        // Only what has no effect of its own goes: a call that delivers a
        // value nothing reads still calls (`LiveRangeEdit::isDead` is
        // `wouldBeTriviallyDead`).
        let round: Vec<InstrKey> = std::mem::take(&mut dead)
            .into_iter()
            .filter(|&key| effect_free(lis.instr(key)) && lis.all_defs_are_dead(key))
            .collect();
        state.erased.extend(round.iter().copied());
        dead = lis.erase_dead_defs(&round);
    }
}

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
        eliminate_dead_defs(lis, state, vec![copy]);
        lis.remove_interval(cp.dst_reg());
        return JoinResult::Joined;
    }
    // Eliminate undefs.
    if eliminate_undef_copy(lis, state, copy) {
        // An IMPLICIT_DEF is left alone, and neither is coalescable.
        return JoinResult::Rejected;
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
        // Coalescing failed. Try rematerializing the definition of the source
        // if it is cheap.
        if re_materialize_def(lis, env, state, copy) {
            return JoinResult::Joined;
        }
        // If we can eliminate the copy without merging the live segments, do
        // so now.
        if adjust_copies_back_from(lis, state, &cp, copy) {
            return JoinResult::Joined;
        }
        // If the source is defined by a commutable two-address instruction
        // whose other operand is the destination, commute it so the copy
        // becomes the identity.
        if remove_copy_by_commuting_def(lis, env, state, &cp, copy) {
            return JoinResult::Joined;
        }
        // Try and see if we can partially eliminate the copy by moving the
        // copy to its predecessor.
        if remove_partial_redundancy(lis, env, state, &cp, copy) {
            return JoinResult::Joined;
        }
        return result;
    }
    // Coalescing to a virtual register that is of a sub-register class of the
    // other. Make sure the resulting register is set to the right register
    // class.
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/RegisterCoalescer.cpp#L2244-L2247
    env.narrow(cp.dst_reg(), cp.src_reg());
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
    lis.rename_input(src, dst);
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

/// `reMaterializeDef`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L1298>):
/// the copy of a value an immediate move defines becomes that move into the
/// copy's own value, and the source loses a read. Whether it did.
fn re_materialize_def(
    lis: &mut LiveIntervals,
    env: &dyn CoalescerEnv,
    state: &mut State,
    copy: InstrKey,
) -> bool {
    let copy_insn = Arc::clone(lis.instr(copy));
    // DEVIATE C11: a member of a parallel copy stays one. The move that takes
    // its place is no member, so it writes before the others read: seed 91 of
    // the generated bodies stored 0x45 where the program stored 0. Exit:
    // parallel copies sequenced before the coalescer (D-F).
    if copy_insn.group.is_some() {
        return false;
    }
    let Some((dst, src)) = full_copy(&copy_insn) else { return false };
    let at = lis.indexes.get_instruction_index(copy);
    let Some(value) = lis.interval(src.value).query(&at).value_in().filter(|vn| !vn.is_phi_def() && !vn.is_unused())
    else {
        return false;
    };
    let def = lis.defining_instr(&value.def(), src.value);
    let def_insn = Arc::clone(lis.instr(def));
    if !env.rematerializable(&def_insn) {
        return false;
    }
    // `LiveRangeEdit::allUsesAvailableAt`: each register the definition reads
    // holds the value it read, at the copy.
    let def_use = lis.indexes.get_instruction_index(def).get_reg_slot(true);
    let copy_use = at.get_reg_slot(true);
    if !def_insn.uses.iter().all(|&used| {
        let li = lis.interval(used);
        li.get_vn_info_at(&def_use)
            .is_none_or(|read| li.get_vn_info_at(&copy_use).is_some_and(|now| Rc::ptr_eq(&read, &now)))
    }) {
        return false;
    }
    // DEVIATE C10: in a build for size the copy (2 bytes) becomes the move
    // (3 to 5) and pays for itself only where the source's own move goes.
    if env.tuned_for_size() && lis.operands(src.value).len() > 2 {
        return false;
    }
    let block = lis.indexes.get_mbb_from_index(&at);
    // The copy's value is defined at the copy's index; the new instruction
    // takes that index over.
    let made = def_insn.renamed(&|v| if v == src.value { dst.value } else { v });
    let made_id = made.id();
    lis.rewrite_instr(block, copy, made);
    state.erased.insert(copy);
    // The source interval can become smaller because we removed a use, and
    // its definition may now be dead.
    let dead = lis.shrink_to_uses(src.value).1;
    eliminate_dead_defs(lis, state, dead);
    // The move owns the definition's bytes only where the definition went; else
    // two instructions claim them.
    if !state.erased.contains(&def) {
        let one = Arc::clone(lis.instr(made_id));
        if let Some((from, to)) = one.covers.filter(|(from, to)| from != to) {
            let _ = to;
            let mut anchor = (*one).clone();
            anchor.covers = Some((from, from));
            let anchor = Arc::new(anchor);
            let new = anchor.id();
            lis.rewrite_instr(block, made_id, anchor);
            state.moved.insert(made_id, new);
        }
    }
    true
}

/// `adjustCopiesBackFrom`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L628>):
/// the copy `B1 = A3` whose source `A3` is itself a copy `A3 = B0` of the
/// destination's register, with `B` not live between the two, is the same
/// value as `B0`: extend `B0` to the copy, merge the value numbers and delete
/// the copy. Whether it did.
///
/// DEVIATE C12 (no number): a member of a parallel copy stays one, on either
/// side, as in `re_materialize_def` (C11): a member reads before any member
/// writes, so the value the second copy reads is not the one the first wrote.
/// Exit: parallel copies sequenced before the coalescer (D-F).
fn adjust_copies_back_from(
    lis: &mut LiveIntervals,
    state: &mut State,
    cp: &CoalescerPair,
    copy: InstrKey,
) -> bool {
    if lis.instr(copy).group.is_some() {
        return false;
    }
    let (a_reg, b_reg) = if cp.is_flipped() { (cp.dst_reg(), cp.src_reg()) } else { (cp.src_reg(), cp.dst_reg()) };
    let copy_idx = lis.indexes.get_instruction_index(copy).get_reg_slot(false);
    let int_a = lis.interval_rc(a_reg);
    let mut int_b = (*lis.interval_rc(b_reg)).clone();
    // BValNo is a value number in B that is defined by a copy from A: `B1`.
    let bs = int_b.find_segment_containing(&copy_idx);
    let Some(b_val) = int_b.segments.get(bs).and_then(|segment| segment.valno.clone()) else { return false };
    // Either this value has an unknown definition point or it is defined at
    // CopyIdx; if unknown, it cannot be processed.
    if b_val.def() != copy_idx {
        return false;
    }
    // AValNo is the value number in A that defines the copy: `A3`.
    let copy_use_idx = copy_idx.get_reg_slot(true);
    let a_s = int_a.find_segment_containing(&copy_use_idx);
    let Some(a_segment) = int_a.segments.get(a_s) else { return false };
    let Some(a_val) = a_segment.valno.clone() else { return false };
    // AValNo must be defined as a copy from B (no partial copies).
    let Some(a_copy) = lis.indexes.get_instruction_from_index(&a_val.def()) else { return false };
    let a_copy_insn = lis.instr(a_copy);
    if a_copy_insn.group.is_some() || !cp.is_coalescable(a_copy_insn) {
        return false;
    }
    // The segment in B that this value number starts with.
    let val_s = int_b.find_segment_containing(&a_val.def().get_prev_slot());
    let Some(val_segment) = int_b.segments.get(val_s) else { return false };
    // The end of that live segment is inside the same block as the copy.
    let Some(val_s_end_inst) = lis.indexes.get_instruction_from_index(&val_segment.end.get_prev_slot()) else {
        return false;
    };
    if lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(val_s_end_inst))
        != lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(copy))
    {
        return false;
    }
    // ValS ends in the same block that the copy's live range starts: with no
    // intervening live segments between them in B, they merge.
    if val_s + 1 != bs {
        return false;
    }
    let (filler_start, filler_end) = (val_segment.end.clone(), int_b.segments[bs].start.clone());
    let val_s_val = val_segment.valno.clone().expect("a segment has a value");
    // The copy is about to go, so the value is defined where B0 ended.
    b_val.set_def(filler_start.clone());
    // A new live range [ValS.end, BS.begin) of either value number, then the
    // two value numbers merge: "B1" into "B0".
    int_b.add_segment(Segment::new(filler_start, filler_end, Some(b_val.clone())));
    if !std::rc::Rc::ptr_eq(&b_val, &val_s_val) {
        int_b.merge_value_number_into(b_val, val_s_val);
    }
    // If the copy was killing its source before the merge, A's range is
    // trimmed once the copy is gone.
    let recompute_a = a_segment.end == copy_idx;
    lis.insert_interval(int_b);
    let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(copy));
    state.erased.insert(copy);
    lis.remove_instr(block, copy);
    if recompute_a {
        lis.shrink_to_uses(a_reg);
    }
    true
}

/// `hasOtherReachingDefs`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L770>).
fn has_other_reaching_defs(
    lis: &LiveIntervals,
    int_a: &LiveInterval,
    int_b: &LiveInterval,
    a_val: &Vn,
    b_val: &Vn,
) -> bool {
    // If AValNo has PHI kills, conservatively assume that IntB defs can reach
    // the PHI values.
    if has_phi_kill(lis, int_a, a_val) {
        return true;
    }
    for a_seg in int_a.segments.iter().filter(|seg| seg.valno.as_ref().is_some_and(|v| Rc::ptr_eq(v, a_val))) {
        let mut bi = int_b.segments.partition_point(|seg| seg.start <= a_seg.start);
        bi = bi.saturating_sub(1);
        while let Some(b_seg) = int_b.segments.get(bi) {
            if a_seg.end < b_seg.start {
                break;
            }
            bi += 1;
            if b_seg.valno.as_ref().is_some_and(|v| Rc::ptr_eq(v, b_val)) {
                continue;
            }
            if b_seg.start <= a_seg.start && b_seg.end > a_seg.start {
                return true;
            }
            if b_seg.start > a_seg.start && b_seg.start < a_seg.end {
                return true;
            }
        }
    }
    false
}

/// `LiveIntervals::hasPHIKill`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/LiveIntervals.cpp#L928>).
fn has_phi_kill(
    lis: &LiveIntervals,
    li: &LiveInterval,
    vni: &Vn,
) -> bool {
    for phi in li.valnos.iter().filter(|phi| !phi.is_unused() && phi.is_phi_def()) {
        let block = lis.indexes.get_mbb_from_index(&phi.def());
        // Conservatively return true instead of scanning huge predecessor
        // lists.
        if lis.cfg.preds[block].len() > 100 {
            return true;
        }
        for &pred in &lis.cfg.preds[block] {
            if li.get_vn_info_before(&lis.indexes.get_mbb_end_idx(pred)).is_some_and(|v| Rc::ptr_eq(&v, vni)) {
                return true;
            }
        }
    }
    false
}

/// `removeCopyByCommutingDef`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L823>):
/// for `B1 = A3` where `A3 = op A2, B0` is a commutable two-address
/// instruction (`A3 = op A3, B0` after the copy that ties it) and `B0` dies
/// there, the instruction becomes `B2 = op B0, A2`, `A3`'s readers read `B2`
/// and the copy is the identity: it goes. Whether it did.
///
/// DEVIATE C12: a member of a parallel copy, as the copy or as a reader, stays
/// as it is.
fn remove_copy_by_commuting_def(
    lis: &mut LiveIntervals,
    env: &dyn CoalescerEnv,
    state: &mut State,
    cp: &CoalescerPair,
    copy: InstrKey,
) -> bool {
    if lis.instr(copy).group.is_some() {
        return false;
    }
    let (a_reg, b_reg) = if cp.is_flipped() { (cp.dst_reg(), cp.src_reg()) } else { (cp.src_reg(), cp.dst_reg()) };
    let int_a = lis.interval_rc(a_reg);
    let int_b = lis.interval_rc(b_reg);
    // BValNo is a value number in B that is defined by a copy from A: `B1`.
    let copy_idx = lis.indexes.get_instruction_index(copy).get_reg_slot(false);
    let Some(b_val) = int_b.get_vn_info_at(&copy_idx) else { return false };
    if b_val.def() != copy_idx {
        return false;
    }
    // AValNo is the value number in A that defines the copy: `A3`.
    let Some(a_val) = int_a.get_vn_info_at(&copy_idx.get_reg_slot(true)) else { return false };
    if a_val.is_unused() || a_val.is_phi_def() {
        return false;
    }
    let Some(def_key) = lis.indexes.get_instruction_from_index(&a_val.def()) else { return false };
    let def_insn = Arc::clone(lis.instr(def_key));
    // A commutable instruction that writes the register of its first source
    // (`isRegTiedToUseOperand`), the other source being B.
    let Some(what) = def_insn.what.as_ref().filter(|what| commutative(what)) else { return false };
    if def_insn.group.is_some() || !def_insn.requires.is_empty() || !def_insn.delivers.is_empty() {
        return false;
    }
    let ([Loc::Held(into)], [Loc::Held(first), Loc::Held(other)]) = (what.dests.as_slice(), what.sources.as_slice())
    else {
        return false;
    };
    if into.value != a_reg || first.value != a_reg || other.value != b_reg {
        return false;
    }
    if !(into.width == first.width && first.width == other.width) {
        return false;
    }
    // B dies at the instruction: the commuted one writes it.
    if !int_b.query(&a_val.def()).is_kill() {
        return false;
    }
    // Make sure there are no other definitions of IntB that would reach the
    // uses which the new definition can reach.
    if has_other_reaching_defs(lis, &int_a, &int_b, &a_val, &b_val) {
        return false;
    }
    // Make sure all reads of AValNo can be rewritten to the new register.
    let mut readers: Vec<InstrKey> = Vec::new();
    for operand in lis.operands(a_reg) {
        if !operand.reads_reg {
            continue;
        }
        let use_idx = lis.indexes.get_instruction_index(operand.instr);
        let inside = int_a.segments.get(int_a.find_segment_containing(&use_idx));
        if !inside.is_some_and(|seg| seg.valno.as_ref().is_some_and(|v| Rc::ptr_eq(v, &a_val))) {
            continue;
        }
        // Partial defs and tied uses can't be rewritten independently: a
        // reader that also defines `a_reg` is a tied use.
        let reader = lis.instr(operand.instr);
        if operand.is_def || reader.group.is_some() || reader.defines.contains(&a_reg) {
            return false;
        }
        readers.push(operand.instr);
    }
    // The registers must be able to share a class: `constrainRegClass`.
    if !env.can_share(a_reg, b_reg) {
        return false;
    }

    // Commute the instruction: its definition and tied source are B's.
    let swap = |value: u32| {
        if value == a_reg {
            b_reg
        } else if value == b_reg {
            a_reg
        } else {
            value
        }
    };
    let def_block = lis.indexes.get_mbb_from_index(&a_val.def());
    let commuted = def_insn.renamed(&swap);
    state.moved.insert(def_key, commuted.id());
    lis.rewrite_instr(def_block, def_key, commuted);

    // Update the readers of AValNo to read B. A copy that becomes `B = B` and
    // defines a value of its own merges that value into BValNo.
    let mut int_a = (*int_a).clone();
    let mut int_b = (*int_b).clone();
    let mut b_val = b_val;
    readers.sort_unstable();
    readers.dedup();
    for key in readers {
        let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(key));
        let renamed = Arc::clone(lis.instr(key)).renamed(&|value| if value == a_reg { b_reg } else { value });
        let (new, is_noop) = (
            renamed.id(),
            key != copy && full_copy(&renamed).is_some_and(|(dst, src)| dst.value == b_reg && src.value == b_reg),
        );
        lis.rewrite_instr(block, key, renamed);
        state.moved.insert(key, new);
        if is_noop {
            let def_idx = lis.indexes.get_instruction_index(new).get_reg_slot(false);
            if let Some(dvni) = int_b.get_vn_info_at(&def_idx) {
                b_val = int_b.merge_value_number_into(dvni, b_val);
                state.erased.insert(new);
                lis.remove_instr(block, new);
            }
        }
    }

    // Extend BValNo by merging in IntA live segments of AValNo. Val# definition
    // is updated.
    b_val.set_def(a_val.def());
    let mut shrink_b = false;
    let ends: Vec<_> = int_a
        .segments
        .iter()
        .filter(|seg| seg.valno.as_ref().is_some_and(|v| Rc::ptr_eq(v, &a_val)))
        .map(|seg| (seg.start.clone(), seg.end.clone()))
        .collect();
    for (start, end) in ends {
        let merged = int_b.add_segment(Segment::new(start, end, Some(b_val.clone())));
        shrink_b |= int_b.segments[merged].end.is_dead();
    }
    LiveIntervals::remove_vreg_def_at(&mut int_a, &a_val.def());
    lis.insert_interval(int_a);
    lis.insert_interval(int_b);
    env.narrow(b_reg, a_reg);

    // The copy is the identity now.
    let copy = state.moved.get(&copy).copied().unwrap_or(copy);
    let block = lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(copy));
    state.erased.insert(copy);
    lis.remove_instr(block, copy);
    if shrink_b {
        lis.shrink_to_uses(b_reg);
    }
    true
}

/// `removePartialRedundancy`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L1115>):
/// for the copy `B = A` in `BB2`, where `A` is a phi of `BB2` and one
/// predecessor `BB0` holds the reverse copy `A = B` with `B` unchanged after
/// it, `B = A` is redundant on the way from `BB0`. It moves to the other
/// predecessor, which has no other successor (so it is colder than `BB2`),
/// or goes outright when every predecessor has the reverse copy. The special
/// case is a loop of one block: the copy is hoisted to its preheader. Whether
/// the copy was removed.
fn remove_partial_redundancy(
    lis: &mut LiveIntervals,
    env: &dyn CoalescerEnv,
    state: &mut State,
    cp: &CoalescerPair,
    copy: InstrKey,
) -> bool {
    // DEVIATE C12: a member of a parallel copy stays one.
    if lis.instr(copy).group.is_some() {
        return false;
    }
    let copy_insn = Arc::clone(lis.instr(copy));
    let copy_idx = lis.indexes.get_instruction_index(copy);
    let block = lis.indexes.get_mbb_from_index(&copy_idx);
    if lis.cfg.preds[block].len() != 2 {
        return false;
    }
    let (a_reg, b_reg) = if cp.is_flipped() { (cp.dst_reg(), cp.src_reg()) } else { (cp.src_reg(), cp.dst_reg()) };
    let int_a = lis.interval_rc(a_reg);
    let mut int_b = (*lis.interval_rc(b_reg)).clone();
    // A is defined by a phi at the entry of the block.
    let early = copy_idx.get_reg_slot(true);
    let Some(a_val) = int_a.get_vn_info_at(&early) else { return false };
    if !a_val.is_phi_def() {
        return false;
    }
    // No B is referenced before the copy in the block.
    if int_b.overlaps_interval(&lis.indexes.get_mbb_start_idx(block), &early) {
        return false;
    }
    // The block has two predecessors: one contains `A = B`, so no copy is
    // inserted for it; the other has the copy moved from the block.
    let mut found_reverse_copy = false;
    let mut copy_left: Option<usize> = None;
    for pred in lis.cfg.preds[block].clone() {
        let pred_end = lis.indexes.get_mbb_end_idx(pred);
        let Some(p_val) = int_a.get_vn_info_before(&pred_end) else { return false };
        let reverse = lis.indexes.get_instruction_from_index(&p_val.def()).map(|def| Arc::clone(lis.instr(def)));
        // Is the definition a reverse copy `A = B`, in `pred`?
        let is_reverse = reverse.as_ref().is_some_and(|def| {
            full_copy(def).is_some_and(|(dst, src)| dst.value == a_reg && src.value == b_reg)
                && lis
                    .indexes
                    .get_instruction_from_index(&p_val.def())
                    .is_some_and(|key| lis.indexes.get_mbb_from_index(&lis.indexes.get_instruction_index(key)) == pred)
        });
        if !is_reverse {
            copy_left = Some(pred);
            continue;
        }
        // If B has another def after the reverse copy and before the end of
        // `pred`, the copy of `B = A` stays at the end of `pred` if it goes
        // from the block.
        let b_changed =
            int_b.valnos.iter().any(|vni| !vni.is_unused() && p_val.def() < vni.def() && vni.def() < pred_end);
        if b_changed {
            copy_left = Some(pred);
            continue;
        }
        found_reverse_copy = true;
    }
    // If no reverse copy is found in the predecessors, nothing to do.
    if !found_reverse_copy {
        return false;
    }
    // When the copy moves, its new block has a single successor, so the
    // block is hotter than it.
    if let Some(left) = copy_left {
        if lis.cfg.succs[left].len() > 1 {
            return false;
        }
        // Where the new copy goes: before the first terminator, which must not
        // reference B (a new definition of B comes before it).
        let at = lis.block_insns(left).iter().find(|insn| env.is_terminator(insn)).map(|insn| insn.id());
        if let Some(at) = at {
            let at_idx = lis.indexes.get_instruction_index(at).get_reg_slot(true);
            if int_b.overlaps_interval(&at_idx, &lis.indexes.get_mbb_end_idx(left)) {
                return false;
            }
        }
        // A new instruction beside the source one: it owns no bytes.
        let mut made = (*copy_insn).clone();
        let owned = copy_insn.covers.map_or(copy_insn.at, |(from, _)| from);
        made.covers = Some((owned, owned));
        let made = Arc::new(made);
        let made_id = made.id();
        let index = lis.insert_instr_before(left, at, made);
        int_b.create_dead_def(index.get_reg_slot(false));
        state.erased.remove(&made_id);
    }
    // Remove the copy. This is fine before the ranges are updated: updating
    // them looks only at slot indices.
    state.erased.insert(copy);
    lis.remove_instr(block, copy);
    // Update the liveness: prune B's value from the copy on, and extend B to
    // where its range ended.
    let reg_slot = copy_idx.get_reg_slot(false);
    let mut end_points = Vec::new();
    let b_val = int_b.query(&copy_idx.get_reg_slot(true)).value_out_or_dead();
    lis.prune_value(&mut int_b.range, &reg_slot, Some(&mut end_points));
    if let Some(b_val) = b_val {
        b_val.mark_unused();
    }
    lis.extend_to_indices(&mut int_b.range, &end_points);
    lis.insert_interval(int_b);
    // Dead defs that were extended are truncated.
    lis.shrink_to_uses(b_reg);
    // Finally, the range of A.
    lis.shrink_to_uses(a_reg);
    true
}

/// `eliminateUndefCopy`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/RegisterCoalescer.cpp#L1742>),
/// without sub-registers: `%1 = COPY undef %2` is deleted and its value number
/// removed from `%1`, whose reads of it are undef; where the value flows into
/// a phi-def it becomes an `implicit_def` instead. True where the copy is gone
/// or changed.
fn eliminate_undef_copy(
    lis: &mut LiveIntervals,
    state: &mut State,
    copy: InstrKey,
) -> bool {
    let insn = Arc::clone(lis.instr(copy));
    let Some((dst, src)) = full_copy(&insn) else { return false };
    let width = dst.width;
    let (dst, src) = (dst.value, src.value);
    let idx = lis.indexes.get_instruction_index(copy);
    // CopyMI is undef iff SrcReg is not live before the instruction.
    if lis.interval(src).live_at(&idx) {
        return false;
    }
    let block = lis.indexes.get_mbb_from_index(&idx);
    let reg_index = idx.get_reg_slot(false);
    let mut li = (*lis.interval_rc(dst)).clone();
    let end = li.get_segment_containing(&reg_index).expect("No segment for defining instruction").end.clone();
    let vni = li.get_vn_info_at(&end);
    // If the undef copy defines a live-out value (i.e. an input to a PHI
    // def), then replace it with an IMPLICIT_DEF.
    if vni.as_ref().is_some_and(|v| v.is_phi_def()) || (vni.is_none() && !li.live_at(&idx)) {
        let mut made = (*insn).clone();
        made.uses.retain(|v| *v != src);
        made.undefs.clear();
        made.what = Some(crate::model::ir::Semantics {
            op: crate::model::ir::Operation::Nothing,
            name: Some(crate::processimpdefs::IMPLICIT_DEF.into()),
            dests: vec![Loc::Held(crate::model::ir::Held { value: dst, width })],
            sources: Vec::new(),
            ..crate::model::ir::Semantics::new(crate::model::ir::Operation::Nothing)
        });
        lis.rewrite_instr(block, copy, Arc::new(made));
        return true;
    }
    // Remove value or merge with previous one in case of a subregister def.
    if let Some(prev) = li.get_vn_info_at(&idx) {
        let vni = li.get_vn_info_at(&reg_index).expect("the copy defines a value");
        li.merge_value_number_into(vni, prev);
    } else {
        LiveIntervals::remove_vreg_def_at(&mut li, &reg_index);
    }
    // Mark uses as undef.
    let readers: Vec<InstrKey> = lis.operands(dst).iter().filter(|op| op.reads_reg).map(|op| op.instr).collect();
    lis.insert_interval(li);
    for reader in readers {
        if reader == copy || !lis.has_instr(reader) {
            continue;
        }
        let at = lis.indexes.get_instruction_index(reader);
        if lis.interval(dst).live_at(&at) {
            continue;
        }
        let one = Arc::clone(lis.instr(reader));
        let mut made = (*one).clone();
        made.uses.retain(|v| *v != dst);
        if !made.undefs.contains(&dst) {
            made.undefs.push(dst);
        }
        let reader_block = lis.indexes.get_mbb_from_index(&at);
        lis.rewrite_instr(reader_block, reader, Arc::new(made));
    }
    state.erased.insert(copy);
    lis.remove_instr(block, copy);
    true
}
