//! What ScalarEvolution drops when an expression is forgotten, and the maps
//! from values to expressions: `forgetMemoizedResults`, `registerUser`,
//! `insertValueToMap`, `eraseValueFromMap` and `getExistingSCEV`.
//!
//! The caches LLVM keeps that no stage has yet filled (`HasRecMap`,
//! `ValuesAtScopes`, `BECountUsers`, the wrap-via-induction sets) are
//! cleared here when they land with their stage.

use llrm_mir::dense::IdSet;
use llrm_mir::hash::HashSet;
use llrm_mir::module::{BlockId, InstId, Operand};

use super::expr::Scev;
use super::{Cx, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L15605
    /// `registerUser`: `user` has each of its operands but the constants.
    pub(super) fn register_user(
        &mut self,
        user: Scev,
    ) {
        for op in self.operands_of(user) {
            // We do not expect that forgetting cached data for SCEVConstants
            // will ever open any prospects for sharpening or introduce any
            // correctness issues, so we don't bother storing their
            // dependencies.
            if self.as_constant(op.scev).is_none() {
                self.scev_users.entry(op.scev).or_default().insert(user);
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14685
    /// `forgetMemoizedResults(SCEVs)`: the cached results of the expressions,
    /// and of every expression that has one as an operand.
    pub fn forget_memoized_results(
        &mut self,
        scevs: &[Scev],
    ) {
        let mut to_forget: HashSet<Scev> = scevs.iter().copied().collect();
        let mut worklist: Vec<Scev> = to_forget.iter().copied().collect();

        while let Some(curr) = worklist.pop() {
            if let Some(users) = self.scev_users.get(&curr) {
                for &user in users {
                    if to_forget.insert(user) {
                        worklist.push(user);
                    }
                }
            }
        }

        for s in to_forget {
            self.forget_memoized_results_impl(s);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L14705
    /// `forgetMemoizedResultsImpl`.
    fn forget_memoized_results_impl(
        &mut self,
        s: Scev,
    ) {
        self.loop_dispositions.remove(&s);
        self.block_dispositions.remove(&s);
        self.unsigned_ranges.remove(&s);
        self.signed_ranges.remove(&s);
        self.constant_multiple_cache.remove(&s);
        self.not_cache.remove(&s);

        // `UnsignedWrapViaInductionTried.erase(AR)` and the signed one.
        self.unsigned_wrap_via_induction_tried.remove(&s);
        self.signed_wrap_via_induction_tried.remove(&s);

        if let Some(values) = self.expr_value_map.remove(&s) {
            for value in values {
                self.value_expr_map.remove(&value);
            }
        }

        if let Some(scope) = self.values_at_scopes.remove(&s) {
            for (l, value) in scope {
                let Some(value) = value else { continue };
                if self.as_constant(value.scev).is_none()
                    && let Some(users) = self.values_at_scopes_users.get_mut(&value.scev)
                    && let Some(at) = users.iter().position(|one| *one == (l, s))
                {
                    users.remove(at);
                }
            }
        }

        if let Some(users) = self.values_at_scopes_users.remove(&s) {
            for (l, v) in users {
                // The recorded value at scope is a use of S, which may carry
                // no-wrap flags that are not part of this key.
                if let Some(values) = self.values_at_scopes.get_mut(&v) {
                    values.retain(|ls| !(ls.0 == l && ls.1.is_some_and(|value| value.scev == s)));
                }
            }
        }

        if let Some(keys) = self.fold_cache_user.remove(&s) {
            for key in keys {
                self.fold_cache.remove(&key);
            }
        }
    }

    /// What the edits in `touched` made stale, dropped, and the rest kept: the
    /// work `SCEVCallbackVH` does in LLVM as values are deleted, and what
    /// `forgetValue` and `forgetLoop` do for those whose operands or blocks
    /// change. The edits leave the CFG as it was (`touched.cfg` is empty and no
    /// block was made or erased), so the loops are those of `cx`.
    pub fn forget_touched(
        &mut self,
        cx: &Cx,
        touched: &llrm_mir::touched::Touched,
    ) {
        self.forget_touched_keeping(cx, touched, false);
    }

    /// [`Self::forget_touched`], the expressions of values whose operands
    /// changed and the trip counts of the loops the edits are in kept where
    /// `keep_counts`: LLVM drops one only on a `forgetLoop`, a deleted value
    /// or an explicit `forgetValue`, none of which an operand replaced in
    /// place (the widening of a counter's compares) is.
    pub fn forget_touched_keeping(
        &mut self,
        cx: &Cx,
        touched: &llrm_mir::touched::Touched,
        keep_counts: bool,
    ) {
        let removed: Vec<Scev> = touched.values_removed.iter().filter_map(|v| self.value_expr_map.remove(&v)).collect();
        self.forget_memoized_results(&removed);
        for v in touched.operands_changed.iter().filter(|_| !keep_counts) {
            if !touched.values_removed.contains(&v) {
                self.forget_value(cx, Operand::Value(v));
            }
        }
        // The count of a loop reads its blocks, and its parents' count its
        // subloops.
        let mut forgotten = IdSet::<BlockId>::new();
        for block in touched.blocks.iter().filter(|_| !keep_counts) {
            let mut one = cx.loops.get_loop_for(block);
            while let Some(found) = one {
                let header = found.get_header();
                if !forgotten.insert(header) {
                    break;
                }
                self.forget_loop_users(cx, header);
                self.trips.forget(header);
                one = found.get_parent_loop();
            }
        }
    }

    /// `forgetMemoizedResults(LoopUsers[L])`: the expressions that use an add
    /// recurrence of the loop `header` heads. The trip counts call it when a
    /// loop gains information.
    pub fn forget_loop_users(
        &mut self,
        _cx: &Cx,
        header: BlockId,
    ) {
        let users = self.loop_users.get(&header).cloned().unwrap_or_default();
        self.forget_memoized_results(&users);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8690
    /// `visitAndClearUsers`: the SCEVs of the instructions on `worklist` and
    /// of all their users, collected in `to_forget` and dropped from the value
    /// map. (`ConstantEvolutionLoopExitValue` is not cached: DEVIATE 10.)
    pub fn visit_and_clear_users(
        &mut self,
        cx: &Cx,
        mut worklist: Vec<InstId>,
        visited: &mut IdSet<InstId>,
        to_forget: &mut Vec<Scev>,
    ) {
        while let Some(i) = worklist.pop() {
            let Some(result) = cx.function.instruction(i).result else { continue };
            if !self.is_value_scevable(cx, Operand::Value(result)) {
                continue;
            }
            if let Some(&s) = self.value_expr_map.get(&result) {
                to_forget.push(s);
                self.erase_value_from_map(Operand::Value(result));
            }
            // `PushDefUseChildren`
            for user in cx.function.users(result) {
                if visited.insert(user.user) {
                    worklist.push(user.user);
                }
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L8714
    /// `forgetValue`: the SCEV of `v` and of everything computed from it.
    pub fn forget_value(
        &mut self,
        cx: &Cx,
        v: Operand,
    ) {
        let Operand::Value(value) = v else { return };
        let llrm_mir::module::ValueDef::Instruction(i) = cx.function.value(value).def else { return };
        // Drop information about expressions based on loop-header PHIs.
        let mut visited: IdSet<InstId> = IdSet::new();
        visited.insert(i);
        let mut to_forget = Vec::new();
        self.visit_and_clear_users(cx, vec![i], &mut visited, &mut to_forget);
        self.forget_memoized_results(&to_forget);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4536
    /// `eraseValueFromMap`.
    pub fn erase_value_from_map(
        &mut self,
        v: Operand,
    ) {
        let Operand::Value(id) = v else { return };
        if let Some(s) = self.value_expr_map.remove(&id) {
            let values = self.expr_value_map.get_mut(&s).expect("a value in ExprValueMap");
            let at = values.iter().position(|one| *one == id);
            debug_assert!(at.is_some(), "Value not in ExprValueMap?");
            if let Some(at) = at {
                values.remove(at);
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4547
    /// `insertValueToMap`: a recursive query may have already computed the
    /// SCEV. It should be equivalent, but may not necessarily be exactly the
    /// same, e.g. due to lazily inferred nowrap flags.
    pub fn insert_value_to_map(
        &mut self,
        v: Operand,
        s: Scev,
    ) {
        let Operand::Value(id) = v else { return };
        if !self.value_expr_map.contains_key(&id) {
            self.value_expr_map.insert(id, s);
            self.expr_value_map.entry(s).or_default().push(id);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4568
    /// `getExistingSCEV`.
    pub fn get_existing_scev(
        &self,
        v: Operand,
    ) -> Option<Scev> {
        let Operand::Value(id) = v else { return None };
        self.value_expr_map.get(&id).copied()
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.cpp#L4526
    /// `getSCEVValues`: the values `s` was made for (`ExprValueMap`), in the
    /// order they were first mapped.
    pub fn get_scev_values(
        &self,
        s: Scev,
    ) -> Vec<Operand> {
        self.expr_value_map
            .get(&s)
            .map(|values| values.iter().map(|id| Operand::Value(*id)).collect())
            .unwrap_or_default()
    }
}
