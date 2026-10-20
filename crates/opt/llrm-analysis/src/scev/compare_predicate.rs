//! `SCEVComparePredicate` and `SCEVUnionPredicate`, and `isKnownMultipleOf`,
//! the one place the union grows.
//!
//! # How it works
//! A [`ComparePredicate`] says `lhs pred rhs` is to be checked at run time. A
//! [`UnionPredicate`] is a list of them, none implied by another:
//! `SCEVComparePredicate::implies` is `lhs == rhs`-style equality of the two
//! sides under `==`, and nothing else implies anything.
//! [`ScalarEvolution::is_known_multiple_of`] decides `S % M == 0` at compile
//! time where it can, and otherwise records `S % M == 0` as the predicate that
//! would make it so.
//!
//! # Deviations from LLVM
//! - D1: only compare predicates exist (no wrap predicates, no
//!   `PredicatedScalarEvolution`): the callers add no other kind.
//!
//! # Example
//! `(4 * %n)<nuw>` is a multiple of 4 outright: a product with `nuw` is one
//! where any operand is.

use llrm_mir::opcode::IntPredicate;

use super::expr::{Kind, Scev, Use};
use super::{Cx, ScalarEvolution};

/// `SCEVComparePredicate` (D1): `lhs pred rhs` is to be checked at run time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparePredicate {
    pub pred: IntPredicate,
    pub lhs: Scev,
    pub rhs: Scev,
}

impl ComparePredicate {
    /// `SCEVComparePredicate::implies`.
    pub fn implies(
        &self,
        other: &Self,
    ) -> bool {
        if self.pred != IntPredicate::Eq {
            return false;
        }
        other.lhs == self.lhs && other.rhs == self.rhs
    }
}

/// `SCEVUnionPredicate` (D1): the predicates, none implied by another.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnionPredicate {
    pub preds: Vec<ComparePredicate>,
}

impl UnionPredicate {
    /// `SCEVUnionPredicate::implies`.
    pub fn implies(
        &self,
        other: &ComparePredicate,
    ) -> bool {
        self.preds.iter().any(|one| one.implies(other))
    }

    /// `SCEVUnionPredicate::add`.
    pub fn add(
        &mut self,
        other: ComparePredicate,
    ) {
        // Implication checks are quadratic in the number of predicates. Stop
        // doing them if there are many predicates.
        let check_implies = self.preds.len() < 16;
        if check_implies && self.implies(&other) {
            return;
        }
        // Build a new vector containing the current predicates, except the
        // ones that are implied by the new predicate.
        let mut pruned: Vec<ComparePredicate> =
            self.preds.iter().copied().filter(|one| !(check_implies && other.implies(one))).collect();
        pruned.push(other);
        self.preds = pruned;
    }

    /// `SCEVUnionPredicate(Preds, SE)`.
    pub fn of(preds: &[ComparePredicate]) -> Self {
        let mut union = Self::default();
        for &one in preds {
            union.add(one);
        }
        union
    }

    /// `isAlwaysTrue`.
    pub fn is_always_true(&self) -> bool {
        self.preds.is_empty()
    }
}

impl ScalarEvolution {
    /// `isKnownMultipleOf`: whether `s` is a multiple of `m`. Where only a
    /// run-time check would tell, and `predicates` is given, the predicate
    /// `s % m == 0` is added to it (unless implied) and the answer is true.
    pub fn is_known_multiple_of(
        &mut self,
        cx: &Cx,
        s: Scev,
        m: u64,
        mut predicates: Option<&mut Vec<ComparePredicate>>,
    ) -> bool {
        self.known_multiple_of(cx, s, m, &mut predicates)
    }

    fn known_multiple_of(
        &mut self,
        cx: &Cx,
        s: Scev,
        m: u64,
        predicates: &mut Option<&mut Vec<ComparePredicate>>,
    ) -> bool {
        if m == 0 {
            return false;
        }
        if m == 1 {
            return true;
        }

        // For a constant, check that "S % M == 0".
        if let Some(constant) = self.as_constant(s) {
            return constant.zext_value() % u128::from(m) == 0;
        }

        // Basic tests have failed. Check "S % M == 0" at compile time and
        // record run-time assumptions.
        let ty = self.type_of(s);
        let modulus = self.get_constant_of_type(cx, ty, m as i128);
        let s_mod_m = self.get_urem_expr(cx, Use::of(s), Use::of(modulus));
        let zero = self.get_zero(cx, ty);

        // Check whether "S % M == 0" is known at compile time.
        if self.is_known_predicate_scev(cx, IntPredicate::Eq, s_mod_m, zero) {
            return true;
        }

        // Check whether "S % M != 0" is known at compile time.
        if self.is_known_predicate_scev(cx, IntPredicate::Ne, s_mod_m, zero) {
            return false;
        }

        if predicates.is_none() {
            return false;
        }

        // Look through Add and AddRec expressions with nuw to improve the
        // precision of added predicates. S is a multiple of M if S starts with
        // a multiple of M and at every iteration step S only adds multiples of
        // M.
        let node = self.node(s).clone();
        let operands: Vec<Use> = self.operands_of(s).to_vec();
        let nuw = node.flags & super::expr::flags::NUW != 0;
        if matches!(node.kind, Kind::Add(_) | Kind::AddRec(..))
            && nuw
            && operands.iter().all(|operand| self.known_multiple_of(cx, operand.scev, m, predicates))
        {
            return true;
        }

        // Similarly, look through Mul with nuw, where any operand being a
        // known-multiple is sufficient.
        if matches!(node.kind, Kind::Mul(_))
            && nuw
            && operands.iter().any(|operand| self.known_multiple_of(cx, operand.scev, m, predicates))
        {
            return true;
        }

        // Similarly, look through MinMax, with no wrapping arithmetic to
        // consider.
        if matches!(
            node.kind,
            Kind::SMax(_) | Kind::UMax(_) | Kind::SMin(_) | Kind::UMin(_)
        )
            && operands.iter().all(|operand| self.known_multiple_of(cx, operand.scev, m, predicates))
        {
            return true;
        }

        let predicate = ComparePredicate { pred: IntPredicate::Eq, lhs: s_mod_m, rhs: zero };

        // Detect redundant predicates.
        let list = predicates.as_deref_mut().expect("predicates");
        if list.iter().any(|one| one.implies(&predicate)) {
            return true;
        }

        // Only record non-redundant predicates.
        list.push(predicate);
        true
    }
}
