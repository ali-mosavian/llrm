//! `InterleavedAccessInfo`: the loads and stores of an innermost loop that
//! together cover consecutive elements of one array, grouped so a vectorizer
//! can replace each group by one wide access and shuffles.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `VectorUtils.cpp`
//! `InterleavedAccessInfo::analyzeInterleaving` and `InterleaveGroup`. The
//! accesses with a constant stride of 2 to 8 elements are collected in program
//! order, then visited bottom-up: each starts a group, and the accesses above
//! it that have its stride and size, and sit a whole number of elements away,
//! join it at their distance, unless a dependence from the access to a store
//! (or from a store to it) would be crossed by the code motion (loads move up
//! to the group's first load, stores down to its last store). Groups with gaps
//! whose first or last member may wrap are dropped; a load group missing its
//! last member needs a scalar epilogue iteration.
//!
//! # Example
//! `s += a[2*i] * a[2*i+1]` is one load group of factor 2: member 0 at
//! `a[2*i]`, member 1 at `a[2*i+1]`. `a[3*i] = x; a[3*i+1] = y` is a store
//! group of factor 3 with a gap at 2, which is dropped unless predicated
//! accesses are enabled.
//!
//! # DEVIATE rows
//! - IAI1 (no predicated SCEV, as LAA 24): `PredicatedScalarEvolution` never
//!   collects a predicate, so strides are taken with the loop's own SCEV and
//!   `OptForSize` (which only decides whether predicates are collected) is not
//!   a parameter. Exit: predicated SCEV.
//! - IAI2 (no symbolic strides, as LAA 25): `replaceSymbolicStrideSCEV` is the
//!   identity. Exit: `collectStridedAccess`.
//! - IAI3 the dependences are the checker's own, indexed by the position of the
//!   instruction in its memory-instruction list; a missing list (more than
//!   `MaxDependences`) is `areDependencesValid() == false`, as the pin.
//!
//! # Used by
//! The loop vectorizer's interleaved-access recipes.

use std::collections::BTreeMap;

use llrm_mir::module::{BlockId, InstId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::{HashMap, HashSet};

use crate::loopaccess::{
    SymbolicStrideMap, address_space, block_needs_predication, get_ptr_stride, load_store_pointer_operand,
    load_store_type, replace_symbolic_stride_scev,
};
use crate::loopaccessinfo::{LoopAccessInfo, loop_blocks_rpo};
use crate::scev::expr::{Scev, Use};
use crate::scev::{Cx, ScalarEvolution};

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L37
/// `MaxInterleaveGroupFactor`.
const MAX_INTERLEAVE_GROUP_FACTOR: u32 = 8;

/// A group of accesses `InterleaveGroup<Instruction>`: members at indices
/// `0..factor` from the first in memory order, some of which may be missing.
#[derive(Clone, Debug)]
pub struct InterleaveGroup {
    factor: u32,
    reverse: bool,
    align: u64,
    /// The members by key; the key is the index relative to the leader, so
    /// negative for a member below it.
    members: BTreeMap<i32, InstId>,
    smallest_key: i32,
    largest_key: i32,
    /// The first load or the last store, in program order: where the wide
    /// access goes.
    insert_pos: InstId,
}

impl InterleaveGroup {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/VectorUtils.h#L521
    /// `InterleaveGroup(Instr, Stride, Alignment)`.
    pub fn new(
        inst: InstId,
        stride: i32,
        align: u64,
    ) -> Self {
        let factor = stride.unsigned_abs();
        assert!(factor > 1, "Invalid interleave factor");
        Self {
            factor,
            reverse: stride < 0,
            align,
            members: BTreeMap::from([(0, inst)]),
            smallest_key: 0,
            largest_key: 0,
            insert_pos: inst,
        }
    }

    /// `isReverse`.
    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    /// `getFactor`.
    pub fn factor(&self) -> u32 {
        self.factor
    }

    /// `getAlign`.
    pub fn align(&self) -> u64 {
        self.align
    }

    /// `getNumMembers`.
    pub fn num_members(&self) -> u32 {
        self.members.len() as u32
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/VectorUtils.h#L540
    /// `insertMember`: `inst` at `index` from the leader (negative if it is
    /// the new leader), with alignment `align`. False if it does not belong.
    pub fn insert_member(
        &mut self,
        inst: InstId,
        index: i32,
        align: u64,
    ) -> bool {
        // Make sure the key fits in an `i32`.
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
            // Make sure the largest index fits in an `i32`, and is less than
            // the interleave factor.
            let Some(largest_index) = self.largest_key.checked_sub(key) else { return false };
            if i64::from(largest_index) >= i64::from(self.factor) {
                return false;
            }
            self.smallest_key = key;
        }

        // It's always safe to select the minimum alignment.
        self.align = self.align.min(align);
        self.members.insert(key, inst);
        true
    }

    /// `getMember`: the member at `index`, if the group has one.
    pub fn member(
        &self,
        index: u32,
    ) -> Option<InstId> {
        self.members.get(&(self.smallest_key + index as i32)).copied()
    }

    /// `members`: the members in index order.
    pub fn members(&self) -> impl Iterator<Item = InstId> + '_ {
        (0..self.factor).filter_map(|index| self.member(index))
    }

    /// `getIndex`: the index of `inst`, from 0.
    pub fn index(
        &self,
        inst: InstId,
    ) -> u32 {
        let (key, _) =
            self.members.iter().find(|(_, one)| **one == inst).expect("InterleaveGroup contains no such member");
        (key - self.smallest_key) as u32
    }

    /// `getInsertPos`.
    pub fn insert_pos(&self) -> InstId {
        self.insert_pos
    }

    /// `setInsertPos`.
    pub fn set_insert_pos(
        &mut self,
        inst: InstId,
    ) {
        self.insert_pos = inst;
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/VectorUtils.h#L631
    /// `requiresScalarEpilogue`: a group without its last member reads past
    /// the end of the array on the last iteration.
    pub fn requires_scalar_epilogue(&self) -> bool {
        // If the last member of the group exists, then a scalar epilog is not
        // needed for this group.
        if self.member(self.factor - 1).is_some() {
            return false;
        }
        // A group with gaps can't be a reversed access: those are
        // invalidated.
        assert!(!self.reverse, "Group should have been invalidated");
        true
    }

    /// `isFull`: no gaps.
    pub fn is_full(&self) -> bool {
        self.num_members() == self.factor
    }
}

/// An index into [`InterleavedAccessInfo`]'s groups.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GroupId(usize);

/// `InterleavedAccessInfo::StrideDescriptor`.
#[derive(Clone, Copy)]
struct StrideDescriptor {
    /// The access's stride; negative for a reverse access.
    stride: i64,
    /// The scalar expression of this access.
    scev: Scev,
    /// The size of the memory object.
    size: u64,
    /// The alignment of this access.
    align: u64,
}

/// `InterleavedAccessInfo`.
#[derive(Default)]
pub struct InterleavedAccessInfo {
    /// True if the loop may contain non-reversed interleaved groups with
    /// out-of-bounds accesses: a scalar epilogue iteration keeps them in
    /// bounds.
    requires_scalar_epilogue: bool,
    groups: Vec<Option<InterleaveGroup>>,
    /// The live groups, in the order they were made.
    live: Vec<GroupId>,
    /// The group of each member.
    group_of: HashMap<InstId, GroupId>,
    /// The dependences among the memory accesses: a source to its sinks.
    dependences: HashMap<InstId, HashSet<InstId>>,
    /// Whether the loop's access analysis has dependences to ask.
    dependences_valid: bool,
}

impl InterleavedAccessInfo {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L1373
    /// `analyzeInterleaving` of the innermost loop with header `lp`, with the
    /// dependences of `laa` (none: nothing may be reordered).
    /// `enable_predicated_accesses` is `EnablePredicatedInterleavedMemAccesses`
    /// (the target's masked interleaved accesses).
    pub fn analyze(
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
        laa: Option<&LoopAccessInfo>,
        enable_predicated_accesses: bool,
    ) -> Self {
        let mut info = Self::default();
        let strides = SymbolicStrideMap::default();
        let accesses = info.collect_const_stride_accesses(se, cx, lp, &strides);
        if accesses.is_empty() {
            return info;
        }
        info.collect_dependences(laa);
        info.group_accesses(se, cx, lp, &accesses, enable_predicated_accesses);
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
    pub fn group_of(
        &self,
        inst: InstId,
    ) -> Option<&InterleaveGroup> {
        self.group_of.get(&inst).and_then(|id| self.groups[id.0].as_ref())
    }

    /// `getInterleaveGroups`, in the order they were made.
    pub fn groups(&self) -> impl Iterator<Item = &InterleaveGroup> + '_ {
        self.live.iter().filter_map(|id| self.groups[id.0].as_ref())
    }

    /// `requiresScalarEpilogue`.
    pub fn requires_scalar_epilogue(&self) -> bool {
        self.requires_scalar_epilogue
    }

    /// `hasGroups`.
    pub fn has_groups(&self) -> bool {
        !self.live.is_empty()
    }

    /// `invalidateGroups`: true if any group was dropped.
    pub fn invalidate_groups(&mut self) -> bool {
        if self.live.is_empty() {
            assert!(!self.requires_scalar_epilogue, "RequiresScalarEpilog should not be set without interleave groups");
            return false;
        }
        self.group_of.clear();
        self.groups.clear();
        self.live.clear();
        self.requires_scalar_epilogue = false;
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L1683
    /// `invalidateGroupsRequiringScalarEpilogue`: drops the groups with gaps
    /// when a scalar epilogue is not allowed (optsize) and the gap cannot be
    /// masked.
    pub fn invalidate_groups_requiring_scalar_epilogue(&mut self) {
        // If no group had triggered the requirement to create an epilogue
        // loop, there is nothing to do.
        if !self.requires_scalar_epilogue {
            return;
        }
        let doomed: Vec<GroupId> = self
            .live
            .iter()
            .copied()
            .filter(|id| self.groups[id.0].as_ref().is_some_and(InterleaveGroup::requires_scalar_epilogue))
            .collect();
        assert!(!doomed.is_empty(), "At least one group must be invalidated, as a scalar epilogue was required");
        for id in doomed {
            self.live.retain(|one| *one != id);
            self.release_without_removing_from_set(id);
        }
        self.requires_scalar_epilogue = false;
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/VectorUtils.h#L816
    /// `createInterleaveGroup`.
    fn create_group(
        &mut self,
        inst: InstId,
        stride: i64,
        align: u64,
    ) -> GroupId {
        let id = GroupId(self.groups.len());
        let previous = self.group_of.insert(inst, id);
        assert!(previous.is_none(), "Already in an interleaved access group");
        self.groups.push(Some(InterleaveGroup::new(inst, stride as i32, align)));
        self.live.push(id);
        id
    }

    /// `releaseGroup`.
    fn release(
        &mut self,
        id: GroupId,
    ) {
        self.live.retain(|one| *one != id);
        self.release_without_removing_from_set(id);
    }

    /// `releaseGroupWithoutRemovingFromSet`.
    fn release_without_removing_from_set(
        &mut self,
        id: GroupId,
    ) {
        if let Some(group) = self.groups[id.0].take() {
            for member in group.members() {
                self.group_of.remove(&member);
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L1288
    /// `isStrided`: a stride a group may have.
    fn is_strided(stride: i64) -> bool {
        let factor = stride.unsigned_abs();
        (2..=u64::from(MAX_INTERLEAVE_GROUP_FACTOR)).contains(&factor)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/VectorUtils.cpp#L1293
    /// `collectConstStrideAccesses`: the loop's accesses with their strides,
    /// in program order (the blocks in reverse postorder).
    fn collect_const_stride_accesses(
        &self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
        strides: &SymbolicStrideMap,
    ) -> Vec<(InstId, StrideDescriptor)> {
        let mut accesses = Vec::new();
        for bb in loop_blocks_rpo(cx, lp) {
            for inst in cx.function.iter_block(bb) {
                let Some(ptr) = load_store_pointer_operand(cx, inst) else { continue };
                let element = load_store_type(cx, inst).expect("a load or a store");

                // Currently, codegen doesn't support cases where the type
                // size doesn't match the alloc size. Skip them for now.
                let size = cx.layout.alloc_size(&cx.context.types, element);
                if size * 8 != cx.layout.size_bits(&cx.context.types, element) {
                    continue;
                }

                // Wrapping is not checked here: whether `ptr` is part of a
                // full group or a group with gaps is not known yet, and the
                // checks for all pointers would be overly conservative. For
                // full groups, wrapping is fine, as the access at null
                // happens without the transformation. See the wrap checks
                // after the groups are formed.
                let stride = get_ptr_stride(se, cx, element, ptr, lp, strides, false).unwrap_or(0);
                let scev = replace_symbolic_stride_scev(se, cx, strides, ptr);
                accesses
                    .push((inst, StrideDescriptor { stride, scev, size, align: load_store_align(cx, inst, element) }));
            }
        }
        accesses
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/VectorUtils.h#L877
    /// `collectDependences`: the checker's dependences, source to sinks.
    fn collect_dependences(
        &mut self,
        laa: Option<&LoopAccessInfo>,
    ) {
        let Some(checker) = laa.map(LoopAccessInfo::dep_checker) else { return };
        let Some(dependences) = checker.dependences() else { return };
        self.dependences_valid = true;
        let instructions = checker.memory_instructions();
        for dependence in dependences {
            self.dependences
                .entry(instructions[dependence.source as usize])
                .or_default()
                .insert(instructions[dependence.destination as usize]);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/VectorUtils.h#L830
    /// `canReorderMemAccessesForInterleavedGroups`: whether the access `a`,
    /// which precedes `b` in program order, may be moved past it. False when
    /// there is a dependence from `a` to `b`, or none can be ruled out.
    fn can_reorder(
        &self,
        a: &(InstId, StrideDescriptor),
        b: &(InstId, StrideDescriptor),
        writes: &dyn Fn(InstId) -> bool,
    ) -> bool {
        // Code motion for interleaved accesses can't violate WAR
        // dependences: reordering is legal if the source isn't a write.
        if !writes(a.0) {
            return true;
        }
        // At least one of the accesses must be strided.
        if !Self::is_strided(a.1.stride) && !Self::is_strided(b.1.stride) {
            return true;
        }
        // If dependence information is not available from the access
        // analysis, conservatively assume the instructions can't be
        // reordered.
        if !self.dependences_valid {
            return false;
        }
        // If there is a dependence from source to sink, assume they can't be
        // reordered.
        !self.dependences.get(&a.0).is_some_and(|sinks| sinks.contains(&b.0))
    }

    /// The group formation and the wrap checks of `analyzeInterleaving`.
    fn group_accesses(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        lp: BlockId,
        accesses: &[(InstId, StrideDescriptor)],
        enable_predicated_accesses: bool,
    ) {
        let writes = |inst: InstId| matches!(cx.function.instruction(inst).opcode, Opcode::Store { .. });
        let predicated = |inst: InstId| {
            block_needs_predication(cx, cx.function.parent(inst).expect("an instruction in a block"), lp)
        };
        // Groups can't be added to once they are complete.
        let mut store_groups: Vec<GroupId> = Vec::new();
        let mut load_groups: Vec<GroupId> = Vec::new();
        let mut completed_load_groups: HashSet<GroupId> = HashSet::default();

        // Search in bottom-up program order for pairs of accesses (A and B)
        // that can form interleaved load or store groups: access A precedes
        // access B. A group is initialized for B in the outer loop, then each
        // A is inserted into B's group if A and B have the same stride and
        // memory object size and A belongs in B's group according to its
        // distance from B, taking care that group formation breaks no
        // dependence.
        for (bi, b) in accesses.iter().enumerate().rev() {
            let (inst_b, des_b) = (b.0, b.1);

            // Initialize a group for B if it has an allowable stride. Even if
            // no group is made for B, the bottom-up algorithm continues, so
            // as not to break any of B's dependences.
            let mut group_b: Option<GroupId> = None;
            if Self::is_strided(des_b.stride) && (!predicated(inst_b) || enable_predicated_accesses) {
                group_b = self.group_of.get(&inst_b).copied();
                if group_b.is_none() {
                    let id = self.create_group(inst_b, des_b.stride, des_b.align);
                    if writes(inst_b) {
                        store_groups.push(id);
                    } else {
                        load_groups.push(id);
                    }
                    group_b = Some(id);
                }
            }

            for a in accesses[..bi].iter().rev() {
                let (inst_a, des_a) = (a.0, a.1);

                // The code motion implies that there can be no dependence
                // between the accesses of a group and other accesses between
                // its first and last member. This also means a group can't
                // have more than one member at a given offset. The accesses of
                // a group can have dependences with other accesses, but the
                // group's boundaries must not encompass them.
                let dependent_member = |this: &Self, group: GroupId| -> Option<InstId> {
                    let group = this.groups[group.0].as_ref().expect("a live group");
                    (0..group.factor()).filter_map(|index| group.member(index)).find(|member| {
                        let entry = accesses.iter().find(|one| one.0 == *member).expect("a member is an access");
                        !this.can_reorder(a, entry, &writes)
                    })
                };

                let group_a = self.group_of.get(&inst_a).copied();
                // If A is a load, dependences are tolerable. If A and B belong
                // to the same (store) group, they are independent, even if no
                // dependence was recorded. If both groups are none, nothing is
                // to be done here.
                if writes(inst_a) && group_a != group_b {
                    let dependent = if group_b.is_some_and(|id| load_groups.contains(&id)) {
                        // If B's group is a load group, A is compared against
                        // all its members: if any load within it has a
                        // dependence on A, the group is marked complete and
                        // A's store group (if any) is released. The former
                        // prevents hoisting a load of B above store A, the
                        // latter sinking store A below load B.
                        dependent_member(self, group_b.expect("a load group"))
                    } else if !self.can_reorder(a, b, &writes) {
                        Some(inst_b)
                    } else {
                        None
                    };

                    if dependent.is_some() {
                        // A has a store dependence on B (or on some load
                        // within B's group) and is part of a store group:
                        // release it to prevent illegal sinking of A below B.
                        // A is then free to form another group with
                        // instructions that precede it.
                        if let Some(id) = group_a.filter(|id| store_groups.contains(id)) {
                            store_groups.retain(|one| *one != id);
                            self.release(id);
                        }
                        // If B is a load in an interleave group, no earlier
                        // loads can be added to it, or the dependent access
                        // would move across store A: mark it complete.
                        if let Some(id) = group_b.filter(|id| load_groups.contains(id)) {
                            completed_load_groups.insert(id);
                        }
                    }
                }
                if group_b.is_some_and(|id| completed_load_groups.contains(&id)) {
                    // Skip trying to add A to B; keep looking for other
                    // conflicting A's in groups to be released.
                    continue;
                }

                // At this point, the illegal code motion is checked for. If
                // either A or B isn't strided, nothing is left to do.
                if !Self::is_strided(des_a.stride) || !Self::is_strided(des_b.stride) {
                    continue;
                }

                // Ignore A if it's already in a group or isn't the same kind
                // of memory operation as B.
                if self.is_interleaved(inst_a) || writes(inst_a) != writes(inst_b) {
                    continue;
                }

                // Rules 1 and 2: ignore A if its stride or size differs from
                // B's.
                if des_a.stride != des_b.stride || des_a.size != des_b.size {
                    continue;
                }

                // Ignore A if the memory objects of A and B are not in the
                // same address space.
                let space = |inst: InstId| load_store_pointer_operand(cx, inst).and_then(|ptr| address_space(cx, ptr));
                if space(inst_a) != space(inst_b) {
                    continue;
                }

                // The distance from A to B.
                let diff = se.get_minus_scev(cx, Use::of(des_a.scev), Use::of(des_b.scev), 0, 0);
                let Some(constant) = se.as_constant(diff) else { continue };
                let Ok(distance_to_b) = i64::try_from(constant.sext_value()) else { continue };

                // Rule 3: ignore A if its distance to B is not a multiple of
                // the size.
                if distance_to_b % des_b.size as i64 != 0 {
                    continue;
                }

                // All members of a predicated interleave group must have the
                // same predicate, and currently must reside in the same block.
                let (block_a, block_b) = (cx.function.parent(inst_a), cx.function.parent(inst_b));
                if (predicated(inst_a) || predicated(inst_b)) && (!enable_predicated_accesses || block_a != block_b) {
                    continue;
                }

                // The index of A is the index of B plus A's distance to B in
                // multiples of the size.
                let group = group_b.expect("a strided B has a group");
                let index_a = i64::from(self.groups[group.0].as_ref().expect("a live group").index(inst_b))
                    + distance_to_b / des_b.size as i64;
                let Ok(index_a) = i32::try_from(index_a) else { continue };

                // Try to insert A into B's group.
                let target = self.groups[group.0].as_mut().expect("a live group");
                if target.insert_member(inst_a, index_a, des_a.align) {
                    self.group_of.insert(inst_a, group);
                    // Set the first load in program order as the insert
                    // position.
                    if !writes(inst_a) {
                        self.groups[group.0].as_mut().expect("a live group").set_insert_pos(inst_a);
                    }
                }
            } // Iteration over A accesses.
        } // Iteration over B accesses.

        // Whether the first or the last member of the group may wrap the
        // address space: the wrap check collectConstStrideAccesses left out.
        let may_wrap = |this: &Self, se: &mut ScalarEvolution, group: GroupId, index: u32| -> bool {
            let member =
                this.groups[group.0].as_ref().expect("a live group").member(index).expect("a group member exists");
            let ptr = load_store_pointer_operand(cx, member).expect("a load or a store");
            let element = load_store_type(cx, member).expect("a load or a store");
            get_ptr_stride(se, cx, element, ptr, lp, &SymbolicStrideMap::default(), true).unwrap_or(0) == 0
        };

        // Remove the load groups with gaps whose accesses may wrap around.
        // FORNOW the pin assumes nothing (`Assume=false`).
        for id in load_groups {
            let Some(group) = self.groups[id.0].as_ref() else { continue };
            // Case 1: a full group. The checks are skipped: if the wide load
            // would wrap around the address space, an access at null would
            // happen without the transformation.
            if group.is_full() {
                continue;
            }
            let (factor, reverse, has_last) =
                (group.factor(), group.is_reverse(), group.member(group.factor() - 1).is_some());
            // Case 2: if the first and last members don't wrap, no pointer in
            // the group does. Member 0 always exists; if the last one does
            // not, peeling (a scalar epilogue) makes up for it, for a
            // non-reversed access.
            if may_wrap(self, se, id, 0) {
                self.release(id);
                continue;
            }
            if has_last {
                if may_wrap(self, se, id, factor - 1) {
                    self.release(id);
                }
            } else {
                // Case 3: a non-reversed load group with gaps needs at least
                // one scalar epilogue iteration, so that nothing is read
                // out-of-bounds speculatively.
                if reverse {
                    self.release(id);
                    continue;
                }
                self.requires_scalar_epilogue = true;
            }
        }

        for id in store_groups {
            let Some(group) = self.groups[id.0].as_ref() else { continue };
            // Case 1: a full group: as for loads.
            if group.is_full() {
                continue;
            }
            // A store group with gaps is a masked wide store: dropped unless
            // the target has masked interleaved accesses.
            if !enable_predicated_accesses {
                self.release(id);
                continue;
            }
            // Case 2: the first and the last member. Case 3 (a scalar
            // epilogue) does not apply: a store with gaps is masked, not
            // speculative.
            let last = (1..group.factor()).rev().find(|index| group.member(*index).is_some());
            if may_wrap(self, se, id, 0) {
                self.release(id);
                continue;
            }
            if let Some(index) = last {
                if may_wrap(self, se, id, index) {
                    self.release(id);
                }
            }
        }
    }
}

/// `getLoadStoreAlignment`: the access's alignment, the type's ABI alignment
/// where the instruction states none.
fn load_store_align(
    cx: &Cx,
    inst: InstId,
    element: llrm_mir::types::TypeId,
) -> u64 {
    match cx.function.instruction(inst).opcode {
        Opcode::Load { align, .. } | Opcode::Store { align, .. } => {
            align.unwrap_or_else(|| cx.layout.align(&cx.context.types, element))
        }
        _ => unreachable!("a load or a store"),
    }
}

#[cfg(test)]
#[path = "interleavedaccess_tests.rs"]
mod tests;
