//! `JoinVals`: what becomes of each value number of two intervals about to be
//! joined. Translated from RegisterCoalescer.cpp (`JoinVals::analyzeValue`,
//! `computeAssignment`, `mapValues`, `resolveConflicts`, `isPrunedValue`,
//! `pruneValues`, `eraseInstrs`, `followCopyChain`, `valuesIdentical`) at
//! d1106deb71cc.
//!
//! # How it works
//! The two sides are the destination (`LHS`, index 0) and the source (`RHS`,
//! index 1) of a `CoalescerPair`. `map_values` gives every value number a
//! resolution and a place in `new_vn_info`, the values of the joined range:
//!
//! - `Keep`: no overlap with a value of the other side.
//! - `Erase`: the value is defined by a copy that becomes an identity, or by a
//!   copy of a value the other side already holds; it is merged into the other
//!   side's value and the copy goes.
//! - `Merge`: defined by the same instruction as a value of the other side.
//! - `Replace`: a PHI-def, or a def that overlaps a value of the other side
//!   that nothing reads there; it takes the other value's place and prunes it.
//! - `Impossible`: the join is refused.
//!
//! `prune_values` cuts the other side's live range where a `Replace` value
//! redefines it, collecting the end points to extend again after the join;
//! `erase_instrs` removes the copies of `Erase` values.
//!
//! DEVIATE C1 (see the module header): a value is one lane, so `ValidLanes`
//! and `WriteLanes` are true for every analyzed value, `Redef` never holds and
//! `CR_Unresolved` never arises; `resolve_conflicts` asserts it.
//!
//! # Example
//! `v2 := mov v1` with `v1` dying there: `v2`'s value is defined by a
//! coalescable copy that kills `v1`'s value: `Erase`, merged into `v1`'s.
//!
//! # Used by
//! `register_coalescer`.

use std::rc::Rc;

use super::pair::{CoalescerPair, full_copy};
use crate::liveness::liveintervals::LiveIntervals;
use crate::liveness::liverange::{LiveRange, Vn};
use crate::liveness::slotindex::{InstrKey, SlotIndex};
use crate::support::hash::HashSet;

/// `JoinVals::ConflictResolution`, without `CR_Unresolved` (DEVIATE C1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resolution {
    Keep,
    Erase,
    Merge,
    Replace,
    Impossible,
}

/// `JoinVals::Val`.
#[derive(Clone)]
struct Val {
    resolution: Resolution,
    analyzed: bool,
    other_vni: Option<Vn>,
    pruned: bool,
    pruned_computed: bool,
    identical: bool,
}

impl Val {
    fn new() -> Self {
        Self {
            resolution: Resolution::Keep,
            analyzed: false,
            other_vni: None,
            pruned: false,
            pruned_computed: false,
            identical: false,
        }
    }
}

/// One side of the join: the live range and what is known of its values.
pub struct Side {
    pub reg: u32,
    pub lr: LiveRange,
    vals: Vec<Val>,
    /// Value number assignments: value numbers of `lr` to entries of
    /// `new_vn_info`; what `LiveRange::join` takes.
    pub assignments: Vec<i32>,
}

impl Side {
    /// Whether `vni` is this side's parameter on entry to the function.
    fn is_entry_input(
        &self,
        lis: &LiveIntervals,
        vni: &Vn,
    ) -> bool {
        vni.is_phi_def() && lis.is_input(self.reg) && vni.def() == lis.indexes.get_mbb_start_idx(lis.cfg.entry)
    }

    fn new(
        reg: u32,
        lr: LiveRange,
    ) -> Self {
        let count = lr.get_num_val_nums() as usize;
        Self { reg, lr, vals: vec![Val::new(); count], assignments: vec![-1; count] }
    }
}

/// The two `JoinVals` of a join: `sides[0]` is `LHS`, `sides[1]` is `RHS`.
pub struct JoinVals<'a> {
    pub sides: [Side; 2],
    /// Values that will be present in the final live range.
    pub new_vn_info: Vec<Option<Vn>>,
    cp: &'a CoalescerPair,
    lis: &'a mut LiveIntervals,
}

impl<'a> JoinVals<'a> {
    pub fn new(
        lis: &'a mut LiveIntervals,
        cp: &'a CoalescerPair,
        lhs: LiveRange,
        rhs: LiveRange,
    ) -> Self {
        let sides = [Side::new(cp.dst_reg(), lhs), Side::new(cp.src_reg(), rhs)];
        Self { sides, new_vn_info: Vec::new(), cp, lis }
    }

    pub fn resolution(
        &self,
        me: usize,
        val_no: usize,
    ) -> Resolution {
        self.sides[me].vals[val_no].resolution
    }

    /// `followCopyChain`: the ultimate value `vni` was copied from, and the
    /// value it is a value of.
    fn follow_copy_chain(
        &mut self,
        me: usize,
        mut vni: Vn,
    ) -> (Option<Vn>, u32) {
        let mut track_reg = self.sides[me].reg;
        while !vni.is_phi_def() {
            let def = vni.def();
            let key = self.lis.defining_instr(&def, track_reg);
            let Some((_, src)) = full_copy(self.lis.instr(key)) else { return (Some(vni), track_reg) };
            let li = self.lis.interval_rc(src.value);
            // No subrange involved.
            let Some(value_in) = li.query(&def).value_in() else {
                // Reaching an undefined value is legitimate.
                return (None, src.value);
            };
            vni = value_in;
            track_reg = src.value;
        }
        (Some(vni), track_reg)
    }

    /// `valuesIdentical`: whether the two values are the same value, both
    /// copies of one value of a third register.
    fn values_identical(
        &mut self,
        me: usize,
        value0: &Vn,
        value1: &Vn,
    ) -> bool {
        let other = 1 - me;
        let (orig0, reg0) = self.follow_copy_chain(me, Rc::clone(value0));
        if reg0 == self.sides[other].reg && orig0.as_ref().is_some_and(|found| Rc::ptr_eq(found, value1)) {
            return true;
        }
        let (orig1, reg1) = self.follow_copy_chain(other, Rc::clone(value1));
        // If both values are undefined, and the source registers are the same
        // register, the values are identical. Filter out cases where only one
        // value is defined.
        let (Some(orig0), Some(orig1)) = (orig0, orig1) else {
            return false;
        };
        // The values are equal if they are defined at the same place and use
        // the same register.
        orig0.def() == orig1.def() && reg0 == reg1
    }

    /// `analyzeValue`: the resolution of value `val_no` of side `me`, leaving
    /// the hard cases (none here) for later. Recursion goes up the dominator
    /// tree, so it ends.
    fn analyze_value(
        &mut self,
        me: usize,
        val_no: usize,
    ) -> Resolution {
        let other = 1 - me;
        assert!(!self.sides[me].vals[val_no].analyzed, "Value has already been analyzed!");
        self.sides[me].vals[val_no].analyzed = true;
        let vni = Rc::clone(&self.sides[me].lr.valnos[val_no]);
        if vni.is_unused() {
            return Resolution::Keep;
        }
        // Get the instruction defining this value.
        let def_mi = if vni.is_phi_def() {
            // Conservatively assume that all lanes in a PHI are valid.
            None
        } else {
            let key = self.lis.defining_instr(&vni.def(), self.sides[me].reg);
            Some(Rc::new(self.lis.instr(key).clone()))
        };
        // Find the value in Other that overlaps VNI->def, if any.
        let other_lrq = self.sides[other].lr.query(&vni.def());
        // It is possible that both values are defined by the same instruction,
        // or the values are PHIs defined in the same block. When that happens,
        // the two values should be merged into one, but not into any preceding
        // value. The first value defined or visited gets CR_Keep, the other
        // gets CR_Merge.
        if let Some(other_vni) = other_lrq.value_defined() {
            assert!(SlotIndex::is_same_instr(&vni.def(), &other_vni.def()), "Broken LRQ");
            // One value stays, the other is merged. Keep the earlier one, or
            // the first one we see.
            if other_vni.def() < vni.def() {
                self.compute_assignment(other, other_vni.id() as usize);
            } else if vni.def() < other_vni.def() && other_lrq.value_in().is_some() {
                // This is an early-clobber def overlapping a live-in value in
                // the other register. Not mergeable.
                self.sides[me].vals[val_no].other_vni = other_lrq.value_in();
                return Resolution::Impossible;
            }
            self.sides[me].vals[val_no].other_vni = Some(Rc::clone(&other_vni));
            // Keep this value, check for conflicts when analyzing OtherVNI.
            // Avoid revisiting OtherVNI->id in computeAssignment() before it
            // is assigned.
            let other_id = other_vni.id() as usize;
            if !self.sides[other].vals[other_id].analyzed || self.sides[other].assignments[other_id] == -1 {
                return Resolution::Keep;
            }
            // Both sides have been analyzed now. Allow overlapping PHI values.
            // Any real interference would show up in a predecessor, the PHI
            // itself can't introduce any conflicts. A parameter's value on
            // entry is no PHI: the entry block has no predecessor to show
            // it, and two parameters are two values (DEVIATE C9).
            if vni.is_phi_def() && !self.sides[me].is_entry_input(self.lis, &vni) {
                return Resolution::Merge;
            }
            // Overlapping lanes can't be resolved: one lane, valid on both.
            return Resolution::Impossible;
        }
        // No simultaneous def. Is Other live at the def?
        let Some(other_vni) = other_lrq.value_in() else {
            // No overlap, no conflict.
            return Resolution::Keep;
        };
        self.sides[me].vals[val_no].other_vni = Some(Rc::clone(&other_vni));
        assert!(!SlotIndex::is_same_instr(&vni.def(), &other_vni.def()), "Broken LRQ");
        // We have overlapping values, or possibly a kill of Other.
        // Recursively compute assignments up the dominator tree.
        self.compute_assignment(other, other_vni.id() as usize);
        // Allow overlapping PHI values.
        let Some(def_mi) = def_mi else {
            return Resolution::Replace;
        };
        // Include the non-conflict where DefMI is a coalescable copy that kills
        // OtherVNI. We still want the copy erased and value numbers merged.
        if self.cp.is_coalescable(&def_mi) {
            return Resolution::Erase;
        }
        // This may not be a real conflict if DefMI simply kills Other and
        // defines VNI.
        if other_lrq.is_kill() && other_lrq.end_point() <= vni.def() {
            return Resolution::Keep;
        }
        // Handle the case where VNI and OtherVNI can be proven to be
        // identical: `%other = COPY %ext; %this = COPY %ext` erases the second.
        if full_copy(&def_mi).is_some() && self.values_identical(me, &vni, &other_vni) {
            self.sides[me].vals[val_no].identical = true;
            return Resolution::Erase;
        }
        // The remaining checks apply to the lanes, of which there is one. The
        // value written is valid in OtherVNI, so `(WriteLanes &
        // OtherV.ValidLanes).none()` does not hold, and what is left is a def
        // that clobbers a live value: when it kills Other it is an early
        // clobber def (illegal to merge), else it clobbers all of the one lane.
        Resolution::Impossible
    }

    /// `computeAssignment`: the place of value `val_no` of side `me` among the
    /// values of the joined range.
    fn compute_assignment(
        &mut self,
        me: usize,
        val_no: usize,
    ) {
        if self.sides[me].vals[val_no].analyzed {
            // Recursion should always move up the dominator tree, so ValNo is
            // not supposed to reappear before it has been assigned.
            assert!(self.sides[me].assignments[val_no] != -1, "Bad recursion?");
            return;
        }
        let resolution = self.analyze_value(me, val_no);
        self.sides[me].vals[val_no].resolution = resolution;
        let other = 1 - me;
        match resolution {
            Resolution::Erase | Resolution::Merge => {
                // Merge this ValNo into OtherVNI.
                let other_vni =
                    self.sides[me].vals[val_no].other_vni.clone().expect("OtherVNI not assigned, can't merge.");
                let other_id = other_vni.id() as usize;
                assert!(self.sides[other].vals[other_id].analyzed, "Missing recursion");
                self.sides[me].assignments[val_no] = self.sides[other].assignments[other_id];
            }
            Resolution::Replace => {
                // The other value is going to be pruned if this join is
                // successful.
                let other_vni =
                    self.sides[me].vals[val_no].other_vni.clone().expect("OtherVNI not assigned, can't prune");
                self.sides[other].vals[other_vni.id() as usize].pruned = true;
                self.assign_new(me, val_no);
            }
            // This value number needs to go in the final joined live range.
            Resolution::Keep | Resolution::Impossible => self.assign_new(me, val_no),
        }
    }

    fn assign_new(
        &mut self,
        me: usize,
        val_no: usize,
    ) {
        self.sides[me].assignments[val_no] = self.new_vn_info.len() as i32;
        self.new_vn_info.push(Some(Rc::clone(&self.sides[me].lr.valnos[val_no])));
    }

    /// `mapValues`: analyze the defs of side `me`. False if any conflict was
    /// impossible to resolve.
    pub fn map_values(
        &mut self,
        me: usize,
    ) -> bool {
        for i in 0..self.sides[me].lr.get_num_val_nums() as usize {
            self.compute_assignment(me, i);
            if self.sides[me].vals[i].resolution == Resolution::Impossible {
                return false;
            }
        }
        true
    }

    /// `resolveConflicts`: with one lane no conflict waits for all values to
    /// be mapped (DEVIATE C1).
    pub fn resolve_conflicts(
        &self,
        me: usize,
    ) -> bool {
        for val in &self.sides[me].vals {
            assert!(val.resolution != Resolution::Impossible, "Unresolvable conflict");
        }
        true
    }

    /// `isPrunedValue`: whether value `val_no` of side `me` is a copy of a
    /// value that will be pruned.
    fn is_pruned_value(
        &mut self,
        me: usize,
        val_no: usize,
    ) -> bool {
        let v = &self.sides[me].vals[val_no];
        if v.pruned || v.pruned_computed {
            return v.pruned;
        }
        if v.resolution != Resolution::Erase && v.resolution != Resolution::Merge {
            return v.pruned;
        }
        // Follow copies up the dominator tree and check if any intermediate
        // value has been pruned.
        let other_id = v.other_vni.as_ref().expect("a merged value has the value it merges into").id() as usize;
        self.sides[me].vals[val_no].pruned_computed = true;
        let pruned = self.is_pruned_value(1 - me, other_id);
        self.sides[me].vals[val_no].pruned = pruned;
        pruned
    }

    /// `pruneValues`: cut the other side's live range where a `Replace` value
    /// of side `me` redefines it, and the live range of a copy of a pruned
    /// value; the places to extend again after the join go in `end_points`.
    pub fn prune_values(
        &mut self,
        me: usize,
        end_points: &mut Vec<SlotIndex>,
    ) {
        let other = 1 - me;
        for i in 0..self.sides[me].lr.get_num_val_nums() as usize {
            let def = self.sides[me].lr.valnos[i].def();
            match self.sides[me].vals[i].resolution {
                Resolution::Keep => {}
                Resolution::Replace => {
                    // This value takes precedence over the value in Other.LR.
                    self.lis.prune_value(&mut self.sides[other].lr, &def, Some(end_points));
                    // This value will reach instructions below, but we need to
                    // make sure the live range also reaches the instruction at
                    // Def.
                    if !def.is_block() {
                        end_points.push(def);
                    }
                }
                Resolution::Erase | Resolution::Merge => {
                    if self.is_pruned_value(me, i) {
                        // This value is ultimately a copy of a pruned value in
                        // LR or Other.LR. We can no longer trust the value
                        // mapping computed by computeAssignment(), the value
                        // that was originally copied could have been replaced.
                        self.lis.prune_value(&mut self.sides[me].lr, &def, Some(end_points));
                    }
                }
                Resolution::Impossible => unreachable!("Unresolved conflicts"),
            }
        }
    }

    /// `eraseInstrs`: remove the instructions that were coalesced away. The
    /// values of other registers that the erased copies read are put in
    /// `shrink_regs`: their live range ended at the copy.
    pub fn erase_instrs(
        &mut self,
        me: usize,
        erased: &mut HashSet<InstrKey>,
        shrink_regs: &mut Vec<u32>,
    ) {
        for i in 0..self.sides[me].lr.get_num_val_nums() as usize {
            // Get the def location before anything invalidates it.
            let def = self.sides[me].lr.valnos[i].def();
            if self.sides[me].vals[i].resolution != Resolution::Erase {
                continue;
            }
            let key = self.lis.defining_instr(&def, self.sides[me].reg);
            if let Some((_, src)) = full_copy(self.lis.instr(key)) {
                if src.value != self.cp.src_reg() && src.value != self.cp.dst_reg() {
                    shrink_regs.push(src.value);
                }
            }
            erased.insert(key);
            let block = self.lis.indexes.get_mbb_from_index(&def);
            self.lis.remove_instr(block, key);
        }
    }

    /// The live ranges after the join's changes: `(LHS, RHS)`.
    pub fn into_ranges(self) -> (LiveRange, LiveRange) {
        let [lhs, rhs] = self.sides;
        (lhs.lr, rhs.lr)
    }

    pub fn lis(&mut self) -> &mut LiveIntervals {
        self.lis
    }
}
