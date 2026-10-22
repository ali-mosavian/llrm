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

use std::sync::Arc;

use super::joinvals::JoinVals;
use super::pair::{CoalescerEnv, CoalescerPair, full_copy};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::Segment;
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
    let mut dead = lis.shrink_to_uses(src.value).1;
    while let Some(key) = dead.pop() {
        if lis.all_defs_are_dead(key) {
            state.erased.insert(key);
            dead.extend(lis.erase_dead_def(key));
        }
    }
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
