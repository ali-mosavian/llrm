//! `InterleaveGroup` and `InterleavedAccessInfo`: the loads and stores of a
//! loop that stride through memory in steps of two to four elements, grouped
//! so a vectorizer can make one wide access and shuffles of each group.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `VectorUtils.h` (`InterleaveGroup`,
//! `canReorderMemAccessesForInterleavedGroups`, `collectDependences`) and
//! `VectorUtils.cpp` (`collectConstStrideAccesses`, `analyzeInterleaving`,
//! `invalidateGroupsRequiringScalarEpilogue`). Every access with a constant
//! stride (`getPtrStride`) is visited bottom up in program order; an access
//! `B` of an allowed stride starts a group, and each earlier access `A` of the
//! same kind, stride, size and address space whose distance from `B` is a
//! whole number of elements joins it, unless a dependence would be moved over
//! by hoisting a load of the group or sinking a store of it. A group with gaps
//! must not wrap (`getPtrStride` with `ShouldCheckWrap`); a load group with a
//! gap at its end needs a scalar epilogue iteration.
//!
//! # Example
//! ```text
//! for (i = 0; i < n; i++) { p[2*i] = a; p[2*i + 1] = b; }   // one store group, factor 2
//! ```
//!
//! # DEVIATE rows
//! - IAI1 (owner llrm-arm64-vec, exit 2026-11-24): no
//!   `PredicatedScalarEvolution` (LoopAccessAnalysis D24): the predicates
//!   `collectConstStrideAccesses` would collect are never added, so an access
//!   whose stride needs an assumption has stride 0 and joins no group.
//! - IAI2 (same owner and exit): `MaxInterleaveGroupFactor` is its default, 8;
//!   the `-max-interleave-group-factor` option does not exist.

use std::collections::BTreeMap;

use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, InstId};
use llrm_mir::opcode::Opcode;

use crate::loopaccess::{
    SymbolicStrideMap, block_needs_predication, get_ptr_stride, load_store_pointer_operand, load_store_type,
};
use crate::loopaccessinfo::{LoopAccessInfo, loop_blocks_rpo};
use crate::scev::expr::Scev;
use crate::scev::{Cx, ScalarEvolution};

/// `MaxInterleaveGroupFactor`: the widest factor a group has.
const MAX_INTERLEAVE_GROUP_FACTOR: i64 = 8;

/// `InterleaveGroup<Instruction>`: the members of one group, by their index
/// from the group's smallest key.
#[derive(Clone, Debug)]
pub struct InterleaveGroup {
    factor: u32,
    reverse: bool,
    alignment: u64,
    members: BTreeMap<i32, InstId>,
    smallest_key: i32,
    largest_key: i32,
    insert_pos: InstId,
}

impl InterleaveGroup {
    /// `InterleaveGroup(Instr, Stride, Alignment)`: a group of `stride`'s
    /// magnitude whose first member is `inst`.
    fn new(
        inst: InstId,
        stride: i32,
        alignment: u64,
    ) -> Self {
        let factor = stride.unsigned_abs();
        assert!(factor > 1, "Invalid interleave factor");
        Self {
            factor,
            reverse: stride < 0,
            alignment,
            members: BTreeMap::from([(0, inst)]),
            smallest_key: 0,
            largest_key: 0,
            insert_pos: inst,
        }
    }

    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    pub fn factor(&self) -> u32 {
        self.factor
    }

    pub fn align(&self) -> u64 {
        self.alignment
    }

    pub fn num_members(&self) -> u32 {
        self.members.len() as u32
    }

    /// `insertMember`: `inst` at `index` from the group's first member, unless
    /// the index is taken or would make the group wider than its factor.
    fn insert_member(
        &mut self,
        inst: InstId,
        index: i32,
        new_align: u64,
    ) -> bool {
        // Make sure the key fits in an int32_t.
        let Some(key) = index.checked_add(self.smallest_key) else { return false };

        // Skip if there is already a member with the same index.
        if self.members.contains_key(&key) {
            return false;
        }

        if key > self.largest_key {
            // The largest index is always less than the interleave factor.
            if index >= self.factor as i32 {
                return false;
            }
            self.largest_key = key;
        } else if key < self.smallest_key {
            // Make sure the largest index fits in an int32_t.
            let Some(largest_index) = self.largest_key.checked_sub(key) else { return false };

            // The largest index is always less than the interleave factor.
            if i64::from(largest_index) >= i64::from(self.factor) {
                return false;
            }
            self.smallest_key = key;
        }

        // It's always safe to select the minimum alignment.
        self.alignment = self.alignment.min(new_align);
        self.members.insert(key, inst);
        true
    }

    /// `getMember`: the member at `index`, if the group has one there.
    pub fn member(
        &self,
        index: u32,
    ) -> Option<InstId> {
        self.members.get(&(self.smallest_key + index as i32)).copied()
    }

    /// `members()`: the members in index order.
    pub fn members(&self) -> impl Iterator<Item = InstId> + '_ {
        (0..self.factor).filter_map(|index| self.member(index))
    }

    /// `getIndex`: the index of the member `inst`.
    pub fn index(
        &self,
        inst: InstId,
    ) -> u32 {
        let (key, _) = self.members.iter().find(|(_, member)| **member == inst).expect("a member of the group");
        (key - self.smallest_key) as u32
    }

    /// `getInsertPos`: where the wide access goes (the first load or the
    /// last store of the group in program order).
    pub fn insert_pos(&self) -> InstId {
        self.insert_pos
    }

    /// `requiresScalarEpilogue`: a load group without its last member.
    pub fn requires_scalar_epilogue(&self) -> bool {
        if self.member(self.factor - 1).is_some() {
            return false;
        }
        // We have a group with gaps. It therefore can't be a reversed access,
        // because such groups get invalidated (TODO).
        assert!(!self.reverse, "Group should have been invalidated");
        true
    }

    /// `isFull`: every index has a member.
    pub fn is_full(&self) -> bool {
        self.num_members() == self.factor
    }
}

/// An index into the groups of an [`InterleavedAccessInfo`].
type GroupId = usize;

/// `StrideDescriptor`: what the analysis knows of one access.
#[derive(Clone, Copy)]
struct StrideDescriptor {
    stride: i64,
    scev: Scev,
    size: u64,
    alignment: u64,
}

/// `InterleavedAccessInfo`.
#[derive(Default)]
pub struct InterleavedAccessInfo {
    /// The groups; a released group is `None`.
    groups: Vec<Option<InterleaveGroup>>,
    group_of: IdMap<InstId, GroupId>,
    requires_scalar_epilogue: bool,
}

impl InterleavedAccessInfo {
    /// `InterleavedAccessInfo(PSE, L, DT, LI, LAI, OptForSize)` then
    /// `analyzeInterleaving(EnableMaskedInterleavedGroup)`.
    pub fn analyze(
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
        lai: &LoopAccessInfo,
        enable_predicated_interleaved_mem_accesses: bool,
    ) -> Self {
        let mut info = Self::default();
        info.analyze_interleaving(se, cx, lp, lai, enable_predicated_interleaved_mem_accesses);
        info
    }

    /// `isInterleaved`.
    pub fn is_interleaved(
        &self,
        inst: InstId,
    ) -> bool {
        self.group_of.contains_key(&inst)
    }

    /// `getInterleaveGroup`.
    pub fn interleave_group(
        &self,
        inst: InstId,
    ) -> Option<&InterleaveGroup> {
        self.group_of.get(&inst).and_then(|&id| self.groups[id].as_ref())
    }

    /// `getInterleaveGroups`: the groups, in creation order.
    pub fn interleave_groups(&self) -> impl Iterator<Item = &InterleaveGroup> {
        self.groups.iter().flatten()
    }

    /// `requiresScalarEpilogue`.
    pub fn requires_scalar_epilogue(&self) -> bool {
        self.requires_scalar_epilogue
    }

    /// `invalidateGroups`: no group is left.
    pub fn invalidate_groups(&mut self) -> bool {
        let any = self.groups.iter().any(Option::is_some);
        self.groups.clear();
        self.group_of.clear();
        self.requires_scalar_epilogue = false;
        any
    }

    /// `invalidateGroupsRequiringScalarEpilogue`: release every group that
    /// needs the epilogue (the loop may not have one).
    pub fn invalidate_groups_requiring_scalar_epilogue(&mut self) {
        if !self.requires_scalar_epilogue {
            return;
        }
        let needing: Vec<GroupId> = (0..self.groups.len())
            .filter(|&id| self.groups[id].as_ref().is_some_and(InterleaveGroup::requires_scalar_epilogue))
            .collect();
        for id in needing {
            self.release_group(id);
        }
        self.requires_scalar_epilogue = false;
    }

    /// `createInterleaveGroup`.
    fn create_group(
        &mut self,
        inst: InstId,
        stride: i64,
        alignment: u64,
    ) -> GroupId {
        let id = self.groups.len();
        self.groups.push(Some(InterleaveGroup::new(inst, stride as i32, alignment)));
        self.group_of.insert(inst, id);
        id
    }

    /// `releaseGroup`: the group goes and its members are in none.
    fn release_group(
        &mut self,
        id: GroupId,
    ) {
        if let Some(group) = self.groups[id].take() {
            for member in group.members() {
                self.group_of.remove(&member);
            }
        }
    }

    /// `isStrided`.
    fn is_strided(stride: i64) -> bool {
        let factor = stride.abs();
        (2..=MAX_INTERLEAVE_GROUP_FACTOR).contains(&factor)
    }

    /// `collectConstStrideAccesses`: every load and store of the loop with its
    /// stride, in reverse postorder (program order).
    fn collect_const_stride_accesses(
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
        strides: &SymbolicStrideMap,
    ) -> Vec<(InstId, StrideDescriptor)> {
        let mut accesses = Vec::new();
        // Since it's desired that the load/store instructions be maintained in
        // "program order" for the interleaved access analysis, we have to
        // visit the blocks in the loop in reverse postorder.
        for bb in loop_blocks_rpo(cx, lp) {
            for inst in cx.function.iter_block(bb) {
                let Some(ptr) = load_store_pointer_operand(cx, inst) else { continue };
                let Some(element_ty) = load_store_type(cx, inst) else { continue };

                // Currently, codegen doesn't support cases where the type size
                // doesn't match the alloc size. Skip them for now.
                let size = cx.layout.alloc_size(&cx.context.types, element_ty);
                if size * 8 != cx.layout.size_bits(&cx.context.types, element_ty) {
                    continue;
                }

                // We don't check wrapping here because we don't know yet if
                // Ptr will be part of a full group or a group with gaps. The
                // wrapping checks are deferred until after the groups are
                // formed.
                let stride = get_ptr_stride(se, cx, element_ty, ptr, lp, strides, false).unwrap_or(0);
                let scev = se.get_scev(cx, ptr);
                let alignment = load_store_alignment(cx, inst, element_ty);
                accesses.push((inst, StrideDescriptor { stride, scev, size, alignment }));
            }
        }
        accesses
    }

    /// `analyzeInterleaving`.
    fn analyze_interleaving(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
        lai: &LoopAccessInfo,
        enable_predicated: bool,
    ) {
        let strides = SymbolicStrideMap::default();
        let accesses = Self::collect_const_stride_accesses(se, cx, lp, &strides);
        if accesses.is_empty() {
            return;
        }
        let at: IdMap<InstId, usize> = accesses.iter().enumerate().map(|(at, (inst, _))| (*inst, at)).collect();

        // `collectDependences`: the source of each dependence and its
        // destinations, if LoopAccessInfo has them.
        let checker = lai.dep_checker();
        let dependences: Option<IdMap<InstId, Vec<InstId>>> = checker
            .dependences()
            .map(
                |deps| {
                    let mut found: IdMap<InstId, Vec<InstId>> = IdMap::new();
                    for dep in deps {
                        found.get_or_insert_with(checker.source(dep), Vec::new).push(checker.destination(dep));
                    }
                    found
                },
            );
        let writes = |inst: InstId| matches!(cx.function.instruction(inst).opcode, Opcode::Store { .. });
        let reads = |inst: InstId| matches!(cx.function.instruction(inst).opcode, Opcode::Load { .. });
        // `canReorderMemAccessesForInterleavedGroups(A, B)`: A precedes B.
        let can_reorder = |a: usize, b: usize| -> bool {
            let ((src, src_des), (sink, sink_des)) = (&accesses[a], &accesses[b]);
            // Code motion for interleaved accesses can't violate WAR
            // dependences. Thus, reordering is legal if the source isn't a
            // write.
            if !writes(*src) {
                return true;
            }
            // At least one of the accesses must be strided.
            if !Self::is_strided(src_des.stride) && !Self::is_strided(sink_des.stride) {
                return true;
            }
            // If dependence information is not available from
            // LoopAccessInfo, conservatively assume the instructions can't be
            // reordered.
            let Some(dependences) = &dependences else { return false };
            // If we know there is a dependence from source to sink, assume
            // the instructions can't be reordered.
            !dependences.get(src).is_some_and(|sinks| sinks.contains(sink))
        };

        // Holds all interleaved store and load groups temporarily, in the
        // order they were made.
        let mut store_groups: Vec<GroupId> = Vec::new();
        let mut load_groups: Vec<GroupId> = Vec::new();
        // Groups added to this set cannot have new members added.
        let mut completed_load_groups: Vec<GroupId> = Vec::new();

        // Search in bottom-up program order for pairs of accesses (A and B)
        // that can form interleaved load or store groups.
        for bi in (0..accesses.len()).rev() {
            let (b, des_b) = accesses[bi];

            // Initialize a group for B if it has an allowable stride. Even if
            // we don't create a group for B, we continue with the bottom-up
            // algorithm to ensure we don't break any of B's dependences.
            let mut group_b: Option<GroupId> = None;
            if Self::is_strided(des_b.stride) && (!is_predicated(cx, lp, b) || enable_predicated) {
                group_b = self.group_of.get(&b).copied();
                if group_b.is_none() {
                    let id = self.create_group(b, des_b.stride, des_b.alignment);
                    if writes(b) {
                        store_groups.push(id);
                    } else {
                        load_groups.push(id);
                    }
                    group_b = Some(id);
                }
            }

            for ai in (0..bi).rev() {
                let (a, des_a) = accesses[ai];

                // Our code motion strategy implies that we can't have
                // dependences between accesses in an interleaved group and
                // other accesses located between the first and last member of
                // the group.
                let dependent_member = |this: &Self, group: GroupId, ai: usize| -> Option<InstId> {
                    let group = this.groups[group].as_ref()?;
                    (0..group.factor())
                        .filter_map(|index| group.member(index))
                        .find(|member| !can_reorder(ai, at[&member]))
                };

                let group_a = self.group_of.get(&a).copied();
                // If A is a load, dependencies are tolerable. If both A and B
                // belong to the same (store) group, they are independent,
                // even if dependencies have not been recorded. If both GroupA
                // and GroupB are null, there's nothing to do here.
                if writes(a) && group_a != group_b {
                    let mut dependent_inst: Option<InstId> = None;
                    // If GroupB is a load group, we have to compare AI
                    // against all members of GroupB because if any load
                    // within GroupB has a dependency on AI, we need to mark
                    // GroupB as complete and also release the store GroupA.
                    if group_b.is_some_and(|id| load_groups.contains(&id)) {
                        dependent_inst = dependent_member(self, group_b.expect("a group"), ai);
                    } else if !can_reorder(ai, bi) {
                        dependent_inst = Some(b);
                    }

                    if dependent_inst.is_some() {
                        // A has a store dependence on B (or on some load
                        // within GroupB) and is part of a store group.
                        // Release A's group to prevent illegal sinking of A
                        // below B.
                        if let Some(id) = group_a.filter(|id| store_groups.contains(id)) {
                            store_groups.retain(|&one| one != id);
                            self.release_group(id);
                        }
                        // If B is a load and part of an interleave group, no
                        // earlier loads can be added to B's interleave group,
                        // because this would mean the DependentInst would move
                        // across store A. Mark the interleave group as
                        // complete.
                        if let Some(id) = group_b.filter(|id| load_groups.contains(id)) {
                            completed_load_groups.push(id);
                        }
                    }
                }
                if group_b.is_some_and(|id| completed_load_groups.contains(&id)) {
                    // Skip trying to add A to B, continue to look for other
                    // conflicting A's in groups to be released.
                    continue;
                }

                // At this point, we've checked for illegal code motion. If
                // either A or B isn't strided, there's nothing left to do.
                if !Self::is_strided(des_a.stride) || !Self::is_strided(des_b.stride) {
                    continue;
                }

                // Ignore A if it's already in a group or isn't the same kind
                // of memory operation as B.
                if self.is_interleaved(a) || reads(a) != reads(b) || writes(a) != writes(b) {
                    continue;
                }

                // Check rules 1 and 2. Ignore A if its stride or size is
                // different from that of B.
                if des_a.stride != des_b.stride || des_a.size != des_b.size {
                    continue;
                }

                // Ignore A if the memory object of A and B don't belong to
                // the same address space.
                if address_space_of(cx, a) != address_space_of(cx, b) {
                    continue;
                }

                // Calculate the distance from A to B.
                let Some(dist) = se.compute_constant_difference(cx, des_a.scev, des_b.scev) else { continue };
                let Ok(distance_to_b) = i64::try_from(dist.sext_value()) else {
                    continue;
                };

                // Check rule 3. Ignore A if its distance to B is not a
                // multiple of the size.
                if distance_to_b % des_b.size as i64 != 0 {
                    continue;
                }

                // All members of a predicated interleave-group must have the
                // same predicate, and currently must reside in the same BB.
                let (block_a, block_b) =
                    (cx.function.parent(a).expect("a block"), cx.function.parent(b).expect("a block"));
                if (is_predicated_block(cx, lp, block_a) || is_predicated_block(cx, lp, block_b))
                    && (!enable_predicated || block_a != block_b)
                {
                    continue;
                }

                // The index of A is the index of B plus A's distance to B in
                // multiples of the size.
                let group_b_id = group_b.expect("B is strided, so it has a group");
                let index_a = self.groups[group_b_id].as_ref().expect("a live group").index(b) as i64
                    + distance_to_b / des_b.size as i64;
                let Ok(index_a) = i32::try_from(index_a) else { continue };

                // Try to insert A into B's group.
                if self.groups[group_b_id].as_mut().expect("a live group").insert_member(a, index_a, des_a.alignment) {
                    self.group_of.insert(a, group_b_id);

                    // Set the first load in program order as the insert
                    // position.
                    if reads(a) {
                        self.groups[group_b_id].as_mut().expect("a live group").insert_pos = a;
                    }
                }
            }
        }

        // Remove interleaved groups with gaps whose memory accesses may wrap
        // around. `collectConstStrideAccesses` does not check wrapping, so
        // the strides are asked again with `ShouldCheckWrap`. FORNOW we use
        // Assume = false.
        let may_wrap = |se: &mut ScalarEvolution, member: InstId| -> bool {
            let (Some(ptr), Some(ty)) = (load_store_pointer_operand(cx, member), load_store_type(cx, member)) else {
                return true;
            };
            get_ptr_stride(se, cx, ty, ptr, lp, &strides, true).unwrap_or(0) == 0
        };

        for id in load_groups {
            let Some(group) = self.groups[id].clone() else { continue };
            // Case 1: A full group. Can skip the checks; for full groups, if
            // the wide load would wrap around the address space we would do a
            // memory access at nullptr even without the transformation.
            if group.is_full() {
                continue;
            }

            // Case 2: If first and last members of the group don't wrap this
            // implies that all the pointers in the group don't wrap. So we
            // check only group member 0 (which is always guaranteed to exist),
            // and group member Factor - 1; if the latter doesn't exist we rely
            // on peeling (if it is a non-reversed access).
            if may_wrap(se, group.member(0).expect("member 0")) {
                self.release_group(id);
                continue;
            }
            if let Some(last) = group.member(group.factor() - 1) {
                if may_wrap(se, last) {
                    self.release_group(id);
                }
            } else {
                // Case 3: A non-reversed interleaved load group with gaps: we
                // need to execute at least one scalar epilogue iteration.
                // This will ensure we don't speculatively access memory
                // out-of-bounds.
                if group.is_reverse() {
                    self.release_group(id);
                    continue;
                }
                self.requires_scalar_epilogue = true;
            }
        }

        for id in store_groups {
            let Some(group) = self.groups[id].clone() else { continue };
            // Case 1: A full group. Can skip the checks.
            if group.is_full() {
                continue;
            }

            // Interleave-store-group with gaps is implemented using masked
            // wide store. Remove interleaved store groups with gaps if
            // masked-interleaved-accesses are not enabled by the target.
            if !enable_predicated {
                self.release_group(id);
                continue;
            }

            // Case 2: If first and last members of the group don't wrap this
            // implies that all the pointers in the group don't wrap.
            if may_wrap(se, group.member(0).expect("member 0")) {
                self.release_group(id);
                continue;
            }
            for index in (1..group.factor()).rev() {
                if let Some(member) = group.member(index) {
                    if may_wrap(se, member) {
                        self.release_group(id);
                    }
                    break;
                }
            }
        }
    }
}

/// `isPredicated(BB)`: the block of `inst` does not always run.
fn is_predicated(
    cx: &Cx,
    lp: BlockId,
    inst: InstId,
) -> bool {
    is_predicated_block(cx, lp, cx.function.parent(inst).expect("a block"))
}

fn is_predicated_block(
    cx: &Cx,
    lp: BlockId,
    block: BlockId,
) -> bool {
    block_needs_predication(cx, block, lp)
}

/// `getLoadStoreAddressSpace`.
fn address_space_of(
    cx: &Cx,
    inst: InstId,
) -> Option<u32> {
    crate::loopaccess::address_space(cx, load_store_pointer_operand(cx, inst)?)
}

/// `getLoadStoreAlignment`: the access's, else its type's.
fn load_store_alignment(
    cx: &Cx,
    inst: InstId,
    ty: llrm_mir::types::TypeId,
) -> u64 {
    match cx.function.instruction(inst).opcode {
        Opcode::Load { align, .. } | Opcode::Store { align, .. } => {
            align.unwrap_or_else(|| cx.layout.align(&cx.context.types, ty))
        }
        _ => 1,
    }
}

#[cfg(test)]
#[path = "interleavedaccess_tests.rs"]
mod tests;
