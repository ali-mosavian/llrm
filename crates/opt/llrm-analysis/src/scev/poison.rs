//! Which expressions are poison when a value is: `SCEVPoisonCollector`,
//! `impliesPoison`, `isGuaranteedNotToBePoison` and `isGuaranteedNotToCauseUB`
//! of ScalarEvolution.

use std::collections::BTreeSet;

use llrm_mir::hash::HashSet;
use llrm_mir::module::Operand;
use llrm_mir::valuetracking::guarantee;

use super::expr::{Kind, Scev, Type};
use super::{Cx, ScalarEvolution};

/// `scevUnconditionallyPropagatesPoisonFromOperands`.
fn unconditionally_propagates_poison_from_operands(kind: Type) -> bool {
    match kind {
        Type::Constant
        | Type::VScale
        | Type::Truncate
        | Type::ZeroExtend
        | Type::SignExtend
        | Type::PtrToAddr
        | Type::Add
        | Type::Mul
        | Type::UDiv
        | Type::AddRec
        | Type::UMax
        | Type::SMax
        | Type::UMin
        | Type::SMin
        // If any operand is poison, the whole expression is poison.
        | Type::Unknown => true,
        // FIXME: if the *first* operand is poison, the whole expression is
        // poison.
        // Pessimistically, say that it does not propagate poison.
        Type::SequentialUMin => false,
        Type::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
    }
}

impl ScalarEvolution {
    /// `SCEVPoisonCollector` run over `root` by `visitAll`: the unknowns that
    /// may be poison. The only way poison may be introduced in a SCEV
    /// expression is from a poison SCEVUnknown; all SCEV nodes propagate
    /// poison from inputs to outputs, with the notable exception of umin_seq,
    /// where only poison from the first operand is (unconditionally)
    /// propagated.
    fn collect_maybe_poison(
        &self,
        cx: &Cx,
        root: Scev,
        look_through_maybe_poison_blocking: bool,
    ) -> BTreeSet<Scev> {
        let q = cx.query(None);
        let mut maybe_poison = BTreeSet::new();
        let mut visited = HashSet::default();
        let mut worklist = Vec::new();
        let mut push = |s: Scev, maybe_poison: &mut BTreeSet<Scev>, worklist: &mut Vec<Scev>| {
            if !visited.insert(s) {
                return;
            }
            // follow
            if !look_through_maybe_poison_blocking
                && !unconditionally_propagates_poison_from_operands(self.scev_type(s))
            {
                return;
            }
            if let Kind::Unknown(value) = self.node(s).kind
                && !is_guaranteed_not_to_be_poison_value(&q, value)
            {
                maybe_poison.insert(s);
            }
            worklist.push(s);
        };
        push(root, &mut maybe_poison, &mut worklist);
        while let Some(s) = worklist.pop() {
            if matches!(self.node(s).kind, Kind::Constant(_) | Kind::Unknown(_)) {
                continue;
            }
            for op in self.operands_of(s) {
                push(op.scev, &mut maybe_poison, &mut worklist);
            }
        }
        maybe_poison
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4149
    /// `impliesPoison`: whether `s` is poison given that `assumed_poison` is.
    pub fn implies_poison(
        &self,
        cx: &Cx,
        assumed_poison: Scev,
        s: Scev,
    ) -> bool {
        // First collect all SCEVs that might result in AssumedPoison to be
        // poison. We need to look through potentially poison-blocking
        // operations here, because we want to find all SCEVs that *might*
        // result in poison, not only those that are *required* to.
        let pc1 = self.collect_maybe_poison(cx, assumed_poison, true);

        // AssumedPoison is never poison. As the assumption is false, the
        // implication is true. Don't bother walking the other SCEV in this
        // case.
        if pc1.is_empty() {
            return true;
        }

        // Collect all SCEVs in S that, if poison, *will* result in S being
        // poison as well. We cannot look through potentially poison-blocking
        // operations here, as their arguments only *may* make the result
        // poison.
        let pc2 = self.collect_maybe_poison(cx, s, false);

        // Make sure that no matter which SCEV in PC1.MaybePoison is actually
        // poison, it will also make S poison by being part of PC2.MaybePoison.
        pc1.is_subset(&pc2)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7425
    /// `ScalarEvolution::isGuaranteedNotToBePoison`.
    pub fn is_guaranteed_not_to_be_poison_scev(
        &self,
        cx: &Cx,
        op: Scev,
    ) -> bool {
        self.collect_maybe_poison(cx, op, true).is_empty()
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7431
    /// `ScalarEvolution::isGuaranteedNotToCauseUB`.
    pub fn is_guaranteed_not_to_cause_ub(
        &mut self,
        cx: &Cx,
        op: Scev,
    ) -> bool {
        // The UDiv may be UB if the divisor is poison or zero. Unless the
        // divisor is a non-zero constant, we have to assume the UDiv may be UB.
        let divisors: Vec<Scev> = {
            let mut found = Vec::new();
            let mut visited = HashSet::default();
            let mut worklist = vec![op];
            visited.insert(op);
            while let Some(s) = worklist.pop() {
                if let Kind::UDiv(_, rhs) = self.node(s).kind {
                    found.push(rhs.scev);
                }
                if matches!(self.node(s).kind, Kind::Constant(_) | Kind::Unknown(_)) {
                    continue;
                }
                for operand in self.operands_of(s) {
                    if visited.insert(operand.scev) {
                        worklist.push(operand.scev);
                    }
                }
            }
            found
        };
        !divisors
            .into_iter()
            .any(|op1| !self.is_known_non_zero(cx, op1) || !self.is_guaranteed_not_to_be_poison_scev(cx, op1))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11282
    /// `ScalarEvolution::isKnownNonZero`.
    pub fn is_known_non_zero(
        &mut self,
        cx: &Cx,
        s: Scev,
    ) -> bool {
        // Query push down for cases where the unsigned range is less than
        // sufficient.
        if let Kind::SignExtend(operand) = self.node(s).kind {
            return self.is_known_non_zero(cx, operand.scev);
        }
        self.get_unsigned_range(cx, s).get_unsigned_min() != 0
    }
}

/// `isGuaranteedNotToBePoison(V)` of ValueTracking.
fn is_guaranteed_not_to_be_poison_value(
    q: &llrm_mir::valuetracking::Query,
    value: Operand,
) -> bool {
    guarantee::is_guaranteed_not_to_be_poison(q, value, 0)
}
