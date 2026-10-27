//! Predicates on expressions (S5): `isKnownPredicate` and what it is made of,
//! `SimplifyICmpOperands`, and the three reasonings `isKnownViaNonRecursive
//! Reasoning` adds to the constant ranges. The implication engine
//! (`isImpliedCond`, `applyLoopGuards`) and the loop guards are the rest of S5.

use std::collections::BTreeMap;

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::dense::IdSet;
use llrm_mir::hash::HashSet;
use llrm_mir::module::{BlockId, Operand, ValueDef};
use llrm_mir::opcode::{IntPredicate, Opcode};

use super::ap::Ap;
use super::expr::{Kind, Scev, Type, Use, flags};
use super::{Cx, ScalarEvolution};

/// `ICmpInst::isTrueWhenEqual`.
pub(super) fn is_true_when_equal(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Eq | IntPredicate::Uge | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Sle
    )
}

/// `ICmpInst::isFalseWhenEqual`.
pub(super) fn is_false_when_equal(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ne | IntPredicate::Ugt | IntPredicate::Ult | IntPredicate::Sgt | IntPredicate::Slt
    )
}

/// `CmpInst::isSigned`.
pub(super) fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

/// `CmpInst::isUnsigned`.
pub(super) fn is_unsigned(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
    )
}

/// `ICmpInst::isEquality`.
pub(super) fn is_equality(pred: IntPredicate) -> bool {
    matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
}

/// `CmpInst::isRelational`.
pub(super) fn is_relational(pred: IntPredicate) -> bool {
    !is_equality(pred)
}

/// `ICmpInst::compare(LHS, RHS, Pred)`.
fn compare(
    lhs: Ap,
    rhs: Ap,
    pred: IntPredicate,
) -> bool {
    match pred {
        IntPredicate::Eq => lhs == rhs,
        IntPredicate::Ne => lhs != rhs,
        IntPredicate::Ugt => rhs.ult(lhs),
        IntPredicate::Uge => rhs.ule(lhs),
        IntPredicate::Ult => lhs.ult(rhs),
        IntPredicate::Ule => lhs.ule(rhs),
        IntPredicate::Sgt => rhs.slt(lhs),
        IntPredicate::Sge => rhs.sle(lhs),
        IntPredicate::Slt => lhs.slt(rhs),
        IntPredicate::Sle => lhs.sle(rhs),
    }
}

/// `ScalarEvolution::MonotonicPredicateType`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonotonicPredicateType {
    MonotonicallyIncreasing,
    MonotonicallyDecreasing,
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10961
    /// `HasSameValue`: structural equivalence is usually sufficient for testing
    /// whether two expressions are equal, however for the purposes of looking
    /// for a condition guarding a loop, it can be useful to be a little more
    /// general, since a front-end may have replicated the controlling
    /// expression.
    pub fn has_same_value(
        &self,
        cx: &Cx,
        a: Scev,
        b: Scev,
    ) -> bool {
        // Quick check to see if they are the same SCEV.
        if a == b {
            return true;
        }

        // Otherwise, if they're both SCEVUnknown, it's possible that they hold
        // two different instructions with the same value. Check for this case.
        if let (Kind::Unknown(Operand::Value(a)), Kind::Unknown(Operand::Value(b))) =
            (&self.node(a).kind, &self.node(b).kind)
            && let (ValueDef::Instruction(a), ValueDef::Instruction(b)) =
                (cx.function.value(*a).def, cx.function.value(*b).def)
        {
            let (a, b) = (cx.function.instruction(a), cx.function.instruction(b));
            // Not all instructions that are "identical" compute the same
            // value. For instance, two distinct alloca instructions allocating
            // the same type are identical and do not read memory; but compute
            // distinct values.
            return a.opcode == b.opcode
                && a.ty == b.ty
                && a.operands == b.operands
                && a.flags == b.flags
                && matches!(a.opcode, Opcode::Binary(_) | Opcode::GetElementPtr { .. });
        }

        // Otherwise assume they may have a different value.
        false
    }

    /// `MatchBinarySub`: `S` as `LHS - RHS`, with `RHS` the multiplied operand.
    pub(super) fn match_binary_sub(
        &self,
        s: Scev,
    ) -> Option<(Scev, Scev)> {
        let (op0, op1) = self.match_binary(s, Type::Add, 0)?;
        if let Some((all_ones, rhs)) = self.match_binary(op0, Type::Mul, 0)
            && self.match_all_ones(all_ones)
        {
            return Some((op1, rhs));
        }
        if let Some((all_ones, rhs)) = self.match_binary(op1, Type::Mul, 0)
            && self.match_all_ones(all_ones)
        {
            return Some((op0, rhs));
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11000
    /// `ScalarEvolution::SimplifyICmpOperands`: canonicalize the comparison,
    /// returning whether it changed.
    pub fn simplify_icmp_operands(
        &mut self,
        cx: &Cx,
        pred: &mut IntPredicate,
        lhs: &mut Scev,
        rhs: &mut Scev,
        depth: u32,
    ) -> bool {
        let mut changed = false;
        // If we hit the max recursion limit bail out.
        if depth >= 3 {
            return false;
        }

        // Simplifies ICMP to trivial true or false by turning it into '0 == 0'
        // or '0 != 0'.
        macro_rules! trivial_case {
            ($se:expr, $pred:expr, $lhs:expr, $rhs:expr, $trivially_true:expr) => {{
                let false_value = $se.get_constant(Ap::zero(1));
                *$lhs = false_value;
                *$rhs = false_value;
                *$pred = if $trivially_true { IntPredicate::Eq } else { IntPredicate::Ne };
                return true;
            }};
        }

        // (vscale patterns: DEVIATE 2.)

        // Canonicalize a constant to the right side.
        if let Some(lhsc) = self.as_constant(*lhs) {
            // Check for both operands constant.
            if let Some(rhsc) = self.as_constant(*rhs) {
                if !compare(lhsc, rhsc, *pred) {
                    trivial_case!(self, pred, lhs, rhs, false);
                }
                trivial_case!(self, pred, lhs, rhs, true);
            }
            // Otherwise swap the operands to put the constant on the right.
            std::mem::swap(lhs, rhs);
            *pred = pred.swapped();
            changed = true;
        }

        // (K + A) pred (K + B) --> A pred B
        // For equality, no flags are needed.
        // For signed, both adds must be NSW. For unsigned, both must be NUW.
        if let Some((c, new_lhs)) = self.match_binary(*lhs, Type::Add, 0)
            && self.as_constant(c).is_some()
            && let Some((c2, new_rhs)) = self.match_binary(*rhs, Type::Add, 0)
            && c2 == c
        {
            let (l_add, r_add) = (*lhs, *rhs);
            if is_equality(*pred)
                || (is_signed(*pred)
                    && self.get_no_wrap_flags(l_add, flags::NSW) != 0
                    && self.get_no_wrap_flags(r_add, flags::NSW) != 0)
                || (is_unsigned(*pred)
                    && self.get_no_wrap_flags(l_add, flags::NUW) != 0
                    && self.get_no_wrap_flags(r_add, flags::NUW) != 0)
            {
                *lhs = new_lhs;
                *rhs = new_rhs;
                changed = true;
            }
        }

        // (C * A) pred (C * B) --> A pred B
        // For equality predicates, both muls must be NUW or both must be NSW
        // (either suffices to make multiplication by C injective; C == 0 is
        // impossible because SCEV folds 0 * X to 0).
        // For signed ordering, C must be positive and both muls must be NSW.
        // For unsigned ordering, both muls must be NUW.
        if let Some((c, new_lhs)) = self.match_binary(*lhs, Type::Mul, 0)
            && let Some(c_ap) = self.as_constant(c)
            && let Some((c2, new_rhs)) = self.match_binary(*rhs, Type::Mul, 0)
            && c2 == c
        {
            let (l_mul, r_mul) = (*lhs, *rhs);
            let both_nuw =
                self.get_no_wrap_flags(l_mul, flags::NUW) != 0 && self.get_no_wrap_flags(r_mul, flags::NUW) != 0;
            let both_nsw =
                self.get_no_wrap_flags(l_mul, flags::NSW) != 0 && self.get_no_wrap_flags(r_mul, flags::NSW) != 0;
            if (is_equality(*pred) && (both_nuw || both_nsw))
                || (is_signed(*pred) && both_nsw && c_ap.is_strictly_positive())
                || (is_unsigned(*pred) && both_nuw)
            {
                *lhs = new_lhs;
                *rhs = new_rhs;
                changed = true;
            }
        }

        // If we're comparing an addrec with a value which is loop-invariant in
        // the addrec's loop, put the addrec on the left. Also make a dominance
        // check, as both operands could be addrecs loop-invariant in each
        // other's loop.
        if let Some(l) = self.addrec_header(*rhs)
            && self.is_loop_invariant(cx, *lhs, Some(l))
            && self.properly_dominates(cx, *lhs, l)
        {
            std::mem::swap(lhs, rhs);
            *pred = pred.swapped();
            changed = true;
        }

        // If there's a constant operand, canonicalize comparisons with boundary
        // cases, and canonicalize *-or-equal comparisons to regular
        // comparisons.
        if let Some(ra) = self.as_constant(*rhs) {
            let mut simplified_by_constant_range = false;

            if !is_equality(*pred) {
                let exact_cr = ConstantRange::make_exact_icmp_region(*pred, ra.width, ra.bits);
                if exact_cr.is_full_set() {
                    trivial_case!(self, pred, lhs, rhs, true);
                }
                if exact_cr.is_empty_set() {
                    trivial_case!(self, pred, lhs, rhs, false);
                }

                if let Some((new_pred, new_rhs)) = exact_cr.get_equivalent_icmp()
                    && is_equality(new_pred)
                {
                    // We were able to convert an inequality to an equality.
                    *pred = new_pred;
                    *rhs = self.get_constant(Ap::new(ra.width, new_rhs));
                    changed = true;
                    simplified_by_constant_range = true;
                }
            }

            if !simplified_by_constant_range {
                let one = Ap::one(ra.width);
                match *pred {
                    IntPredicate::Eq | IntPredicate::Ne => {
                        // Fold ((-1) * %a) + %b == 0 (equivalent to %b-%a == 0)
                        // into %a == %b.
                        if ra.is_zero()
                            && let Some((new_lhs, new_rhs)) = self.match_binary_sub(*lhs)
                        {
                            *lhs = new_lhs;
                            *rhs = new_rhs;
                            changed = true;
                        }
                    }

                    // The "Should have been caught earlier!" messages refer to
                    // the fact that the ExactCR.isFullSet() or
                    // ExactCR.isEmptySet() check above should have fired on the
                    // corresponding cases, and canonicalized the check to
                    // trivial case.
                    IntPredicate::Uge => {
                        debug_assert!(!ra.is_zero(), "Should have been caught earlier!");
                        *pred = IntPredicate::Ugt;
                        *rhs = self.get_constant(ra.sub(one));
                        changed = true;
                    }
                    IntPredicate::Ule => {
                        debug_assert!(!ra.is_all_ones(), "Should have been caught earlier!");
                        *pred = IntPredicate::Ult;
                        *rhs = self.get_constant(ra.add(one));
                        changed = true;
                    }
                    IntPredicate::Sge => {
                        debug_assert!(!ra.is_signed_min(), "Should have been caught earlier!");
                        *pred = IntPredicate::Sgt;
                        *rhs = self.get_constant(ra.sub(one));
                        changed = true;
                    }
                    IntPredicate::Sle => {
                        debug_assert!(ra != Ap::signed_max(ra.width), "Should have been caught earlier!");
                        *pred = IntPredicate::Slt;
                        *rhs = self.get_constant(ra.add(one));
                        changed = true;
                    }
                    _ => {}
                }
            }
        }

        // a /u b == 0 => a < b
        // a /u b != 0 => a >= b
        if is_equality(*pred)
            && self.match_zero(*rhs)
            && let Some((new_lhs, new_rhs)) = self.match_binary(*lhs, Type::UDiv, 0)
        {
            *lhs = new_lhs;
            *rhs = new_rhs;
            *pred = if *pred == IntPredicate::Eq { IntPredicate::Ult } else { IntPredicate::Uge };
            changed = true;
        }

        // Check for obvious equality.
        if self.has_same_value(cx, *lhs, *rhs) {
            if is_true_when_equal(*pred) {
                trivial_case!(self, pred, lhs, rhs, true);
            }
            if is_false_when_equal(*pred) {
                trivial_case!(self, pred, lhs, rhs, false);
            }
        }

        // If possible, canonicalize GE/LE comparisons to GT/LT comparisons, by
        // adding or subtracting 1 from one of the operands.
        let ty = self.type_of(*rhs);
        match *pred {
            IntPredicate::Sle => {
                if self.get_signed_range_max(cx, *rhs) != Ap::signed_max(self.get_type_size_in_bits(cx, ty)) {
                    let one = Use::of(self.get_constant_of_type(cx, ty, 1));
                    *rhs = self.get_add_expr(cx, vec![one, Use::of(*rhs)], super::Flags::expr(flags::NSW), 0).scev;
                    *pred = IntPredicate::Slt;
                    changed = true;
                } else if !self.get_signed_range_min(cx, *lhs).is_signed_min() {
                    let minus_one = Use::of(self.get_constant_of_type(cx, ty, -1));
                    *lhs =
                        self.get_add_expr(cx, vec![minus_one, Use::of(*lhs)], super::Flags::expr(flags::NSW), 0).scev;
                    *pred = IntPredicate::Slt;
                    changed = true;
                }
            }
            IntPredicate::Sge => {
                if !self.get_signed_range_min(cx, *rhs).is_signed_min() {
                    let minus_one = Use::of(self.get_constant_of_type(cx, ty, -1));
                    *rhs =
                        self.get_add_expr(cx, vec![minus_one, Use::of(*rhs)], super::Flags::expr(flags::NSW), 0).scev;
                    *pred = IntPredicate::Sgt;
                    changed = true;
                } else if self.get_signed_range_max(cx, *lhs) != Ap::signed_max(self.get_type_size_in_bits(cx, ty)) {
                    let one = Use::of(self.get_constant_of_type(cx, ty, 1));
                    *lhs = self.get_add_expr(cx, vec![one, Use::of(*lhs)], super::Flags::expr(flags::NSW), 0).scev;
                    *pred = IntPredicate::Sgt;
                    changed = true;
                }
            }
            IntPredicate::Ule => {
                if !self.get_unsigned_range_max(cx, *rhs).is_all_ones() {
                    let one = Use::of(self.get_constant_of_type(cx, ty, 1));
                    *rhs = self.get_add_expr(cx, vec![one, Use::of(*rhs)], super::Flags::expr(flags::NUW), 0).scev;
                    *pred = IntPredicate::Ult;
                    changed = true;
                } else if !self.get_unsigned_range_min(cx, *lhs).is_zero() {
                    let minus_one = Use::of(self.get_constant_of_type(cx, ty, -1));
                    *lhs = self.get_add_expr(cx, vec![minus_one, Use::of(*lhs)], super::Flags::default(), 0).scev;
                    *pred = IntPredicate::Ult;
                    changed = true;
                }
            }
            IntPredicate::Uge => {
                // If RHS is an op we can fold the -1, try that first.
                // Otherwise prefer LHS to preserve the nuw flag.
                let folds_minus_one = self.as_constant(*rhs).is_some()
                    || (matches!(self.node(*rhs).kind, Kind::Add(_) | Kind::AddRec(..))
                        && self.as_constant(self.operands(*rhs)[0].scev).is_some());
                if folds_minus_one && !self.get_unsigned_range_min(cx, *rhs).is_zero() {
                    let minus_one = Use::of(self.get_constant_of_type(cx, ty, -1));
                    *rhs = self.get_add_expr(cx, vec![minus_one, Use::of(*rhs)], super::Flags::default(), 0).scev;
                    *pred = IntPredicate::Ugt;
                    changed = true;
                } else if !self.get_unsigned_range_max(cx, *lhs).is_all_ones() {
                    let one = Use::of(self.get_constant_of_type(cx, ty, 1));
                    *lhs = self.get_add_expr(cx, vec![one, Use::of(*lhs)], super::Flags::expr(flags::NUW), 0).scev;
                    *pred = IntPredicate::Ugt;
                    changed = true;
                } else if !self.get_unsigned_range_min(cx, *rhs).is_zero() {
                    let minus_one = Use::of(self.get_constant_of_type(cx, ty, -1));
                    *rhs = self.get_add_expr(cx, vec![minus_one, Use::of(*rhs)], super::Flags::default(), 0).scev;
                    *pred = IntPredicate::Ugt;
                    changed = true;
                }
            }
            _ => {}
        }

        // TODO: More simplifications are possible here.

        // Recursively simplify until we either hit a recursion limit or nothing
        // changes.
        if changed {
            self.simplify_icmp_operands(cx, pred, lhs, rhs, depth + 1);
        }

        changed
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13184
    /// `isKnownPredicateExtendIdiom`: zext x u<= sext x, sext x s<= zext x.
    fn is_known_predicate_extend_idiom(
        &self,
        pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
    ) -> bool {
        match pred {
            IntPredicate::Sge | IntPredicate::Sle => {
                if pred == IntPredicate::Sge {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // If operand >=s 0 then ZExt == SExt. If operand <s 0 then SExt
                // <s ZExt.
                self.match_unary(lhs, Type::SignExtend)
                    .is_some_and(|op| self.match_unary(rhs, Type::ZeroExtend) == Some(op))
            }
            IntPredicate::Uge | IntPredicate::Ule => {
                if pred == IntPredicate::Uge {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // If operand >=u 0 then ZExt == SExt. If operand <u 0 then ZExt
                // <u SExt.
                self.match_unary(lhs, Type::ZeroExtend)
                    .is_some_and(|op| self.match_unary(rhs, Type::SignExtend) == Some(op))
            }
            _ => false,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11732
    /// `isKnownPredicateViaConstantRanges`.
    pub fn is_known_predicate_via_constant_ranges(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        if self.has_same_value(cx, lhs, rhs) {
            return is_true_when_equal(pred);
        }

        // The check at the top of the function catches the case where the
        // values are known to be equal.
        if pred == IntPredicate::Eq {
            return false;
        }

        if pred == IntPredicate::Ne {
            if self.check_range(cx, pred, lhs, rhs, true) || self.check_range(cx, pred, lhs, rhs, false) {
                return true;
            }
            let diff = self.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), 0, 0);
            return !matches!(self.node(diff).kind, Kind::CouldNotCompute) && self.is_known_non_zero(cx, diff);
        }

        self.check_range(cx, pred, lhs, rhs, is_signed(pred))
    }

    /// `CheckRange` of `isKnownPredicateViaConstantRanges`.
    fn check_range(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
        signed: bool,
    ) -> bool {
        let (range_lhs, range_rhs) = if signed {
            (self.get_signed_range(cx, lhs), self.get_signed_range(cx, rhs))
        } else {
            (self.get_unsigned_range(cx, lhs), self.get_unsigned_range(cx, rhs))
        };
        range_lhs.icmp(pred, &range_rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12419
    /// `splitBinaryAdd`: `expr` as `L + R` and its flags.
    pub fn split_binary_add(
        &self,
        expr: Scev,
    ) -> Option<(Scev, Scev, u8)> {
        let (l, r) = self.match_binary(expr, Type::Add, 0)?;
        Some((l, r, self.get_no_wrap_flags(expr, flags::MASK)))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11759
    /// `isKnownPredicateViaNoOverflow`.
    pub fn is_known_predicate_via_no_overflow(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
    ) -> bool {
        // Match X to (A + C1)<ExpectedFlags> and Y to (A + C2)<ExpectedFlags>,
        // where C1 and C2 are constant integers. If either X or Y are not add
        // expressions, consider them as X + 0 and Y + 0 respectively. C1 and
        // C2 are returned.
        let match_binary_add_to_const = |se: &mut Self, x: Scev, y: Scev, expected_flags: u8| -> Option<(Ap, Ap)> {
            let (x_const_op, x_non_const_op, x_flags_present) = match se.split_binary_add(x) {
                Some((l, r, f)) => (l, r, f),
                None => (se.get_zero(cx, se.type_of(x)), x, expected_flags),
            };
            let x_const = se.as_constant(x_const_op)?;

            let (y_const_op, y_non_const_op, y_flags_present) = match se.split_binary_add(y) {
                Some((l, r, f)) => (l, r, f),
                None => (se.get_zero(cx, se.type_of(y)), y, expected_flags),
            };

            if y_non_const_op != x_non_const_op {
                return None;
            }

            let y_const = se.as_constant(y_const_op)?;

            // When matching ADDs with NUW flags (and unsigned predicates),
            // only the second ADD (with the larger constant) requires NUW.
            if y_flags_present & expected_flags != expected_flags {
                return None;
            }
            if expected_flags != flags::NUW && x_flags_present & expected_flags != expected_flags {
                return None;
            }

            Some((x_const, y_const))
        };

        match pred {
            IntPredicate::Sge | IntPredicate::Sle => {
                if pred == IntPredicate::Sge {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // (X + C1)<nsw> s<= (X + C2)<nsw> if C1 s<= C2.
                if let Some((c1, c2)) = match_binary_add_to_const(self, lhs, rhs, flags::NSW)
                    && c1.sle(c2)
                {
                    return true;
                }
            }
            IntPredicate::Sgt | IntPredicate::Slt => {
                if pred == IntPredicate::Sgt {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // (X + C1)<nsw> s< (X + C2)<nsw> if C1 s< C2.
                if let Some((c1, c2)) = match_binary_add_to_const(self, lhs, rhs, flags::NSW)
                    && c1.slt(c2)
                {
                    return true;
                }
            }
            IntPredicate::Uge | IntPredicate::Ule => {
                if pred == IntPredicate::Uge {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // (X + C1) u<= (X + C2)<nuw> for C1 u<= C2.
                if let Some((c1, c2)) = match_binary_add_to_const(self, lhs, rhs, flags::NUW)
                    && c1.ule(c2)
                {
                    return true;
                }
            }
            IntPredicate::Ugt | IntPredicate::Ult => {
                if pred == IntPredicate::Ugt {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // (X + C1) u< (X + C2)<nuw> if C1 u< C2.
                if let Some((c1, c2)) = match_binary_add_to_const(self, lhs, rhs, flags::NUW)
                    && c1.ult(c2)
                {
                    return true;
                }
            }
            _ => {}
        }

        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11858
    /// `isKnownPredicateViaSplitting`: if L >= 0 then I `ult` L <=> I >= 0 &&
    /// I `slt` L.
    pub fn is_known_predicate_via_splitting(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        if pred != IntPredicate::Ult || self.proving_split_predicate {
            return false;
        }

        // Allowing arbitrary number of activations of
        // isKnownPredicateViaSplitting on the stack can result in exponential
        // time complexity.
        self.proving_split_predicate = true;

        // To prove L >= 0 we use isKnownNonNegative whereas to prove I >= 0 we
        // use isKnownPredicate. isKnownPredicate is more powerful, but also
        // more expensive; and using isKnownNonNegative(RHS) is
        // sufficient for most of the interesting cases seen in
        // practice. We can consider "upgrading" L >= 0 to use
        // isKnownPredicate later if needed.
        let zero = self.get_zero(cx, self.type_of(lhs));
        let known = self.is_known_non_negative(cx, rhs)
            && self.is_known_predicate_scev(cx, IntPredicate::Sge, lhs, zero)
            && self.is_known_predicate_scev(cx, IntPredicate::Slt, lhs, rhs);
        self.proving_split_predicate = false;
        known
    }

    /// `IsMinMaxConsistingOf<Kind>`: whether `maybe_min_max` is a `kind` of
    /// `candidate` and some other values.
    fn is_min_max_consisting_of(
        &self,
        kind: Type,
        maybe_min_max: Scev,
        candidate: Scev,
    ) -> bool {
        if self.scev_type(maybe_min_max) != kind {
            return false;
        }
        self.operands(maybe_min_max).iter().any(|op| op.bits == 0 && op.scev == candidate)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12906
    /// `IsKnownPredicateViaAddRecStart`: if both sides are affine addrecs for
    /// the same loop, with equal steps, and we know the recurrences don't wrap,
    /// then we only need to check the predicate on the starting values.
    fn is_known_predicate_via_add_rec_start(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        if !is_relational(pred) {
            return false;
        }

        let Some((l_start, step, l)) = self.match_affine_add_rec(lhs) else { return false };
        let Some((r_start, r_step, r_loop)) = self.match_affine_add_rec(rhs) else { return false };
        if r_step != step || r_loop != l {
            return false;
        }
        let nw = if is_signed(pred) { flags::NSW } else { flags::NUW };
        if self.get_no_wrap_flags(lhs, nw) == 0 || self.get_no_wrap_flags(rhs, nw) == 0 {
            return false;
        }

        self.is_known_predicate_scev(cx, pred, l_start, r_start)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11516
    /// `getMonotonicPredicateTypeImpl`.
    fn get_monotonic_predicate_type_impl(
        &mut self,
        cx: &Cx,
        lhs: Scev,
        pred: IntPredicate,
    ) -> Option<MonotonicPredicateType> {
        // A zero step value for LHS means the induction variable is
        // essentially a loop invariant value. We don't really depend on the
        // predicate actually flipping from false to true (for increasing
        // predicates, and the other way around for decreasing predicates), all
        // we care about is that *if* the predicate changes then it only changes
        // from false to true.
        //
        // A zero step value in itself is not very useful, but there may be
        // places where SCEV can prove X >= 0 but not prove X > 0, so it is
        // helpful to be as general as possible.

        // Only handle LE/LT/GE/GT predicates.
        if !is_relational(pred) {
            return None;
        }

        let is_greater = matches!(
            pred,
            IntPredicate::Uge | IntPredicate::Sge | IntPredicate::Ugt | IntPredicate::Sgt
        );

        // Check that AR does not wrap.
        if is_unsigned(pred) {
            if self.get_no_wrap_flags(lhs, flags::NUW) == 0 {
                return None;
            }
            return Some(if is_greater {
                MonotonicPredicateType::MonotonicallyIncreasing
            } else {
                MonotonicPredicateType::MonotonicallyDecreasing
            });
        }
        debug_assert!(is_signed(pred), "Relational predicate is either signed or unsigned!");
        if self.get_no_wrap_flags(lhs, flags::NSW) == 0 {
            return None;
        }

        let step = self.get_step_recurrence(cx, lhs);

        if self.is_known_non_negative(cx, step) {
            return Some(if is_greater {
                MonotonicPredicateType::MonotonicallyIncreasing
            } else {
                MonotonicPredicateType::MonotonicallyDecreasing
            });
        }

        if self.is_known_non_positive(cx, step) {
            return Some(if !is_greater {
                MonotonicPredicateType::MonotonicallyIncreasing
            } else {
                MonotonicPredicateType::MonotonicallyDecreasing
            });
        }

        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11496
    /// `getMonotonicPredicateType`.
    pub fn get_monotonic_predicate_type(
        &mut self,
        cx: &Cx,
        lhs: Scev,
        pred: IntPredicate,
    ) -> Option<MonotonicPredicateType> {
        self.get_monotonic_predicate_type_impl(cx, lhs, pred)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12935
    /// `IsKnownPredicateViaAddRecMonotonicity`: is LHS `Pred` RHS true because
    /// one of them is an AddRec that is known not to go below its own start
    /// value?
    fn is_known_predicate_via_add_rec_monotonicity(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
    ) -> bool {
        // Normalize to (AddRec Pred Start).
        if !matches!(self.node(lhs).kind, Kind::AddRec(..)) && matches!(self.node(rhs).kind, Kind::AddRec(..)) {
            pred = pred.swapped();
            std::mem::swap(&mut lhs, &mut rhs);
        }

        // The recurrence is equal to Start in the first iteration, so only the
        // non-strict predicate holds.
        if pred != IntPredicate::Uge && pred != IntPredicate::Sge {
            return false;
        }

        if !matches!(self.node(lhs).kind, Kind::AddRec(..)) || self.addrec_start(lhs) != rhs {
            return false;
        }

        self.get_monotonic_predicate_type(cx, lhs, pred) == Some(MonotonicPredicateType::MonotonicallyIncreasing)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12429
    /// `computeConstantDifference`: `More - Less` if it reduces to a constant.
    pub fn compute_constant_difference(
        &mut self,
        cx: &Cx,
        mut more: Scev,
        mut less: Scev,
    ) -> Option<Ap> {
        // We avoid subtracting expressions here because this function is
        // usually fairly deep in the call stack (i.e. is called many times).
        let bw = self.get_type_size_in_bits(cx, self.type_of(more));
        let mut diff = Ap::zero(bw);
        let mut diff_mul = Ap::one(bw);
        // Try various simplifications to reduce the difference to a constant.
        // Limit the number of allowed simplifications to keep compile-time low.
        for _ in 0..8 {
            if more == less {
                return Some(diff);
            }

            // Reduce addrecs with identical steps to their start value.
            if matches!(self.node(less).kind, Kind::AddRec(..)) && matches!(self.node(more).kind, Kind::AddRec(..)) {
                let (lar, mar) = (less, more);
                if self.addrec_header(lar) != self.addrec_header(mar) {
                    return None;
                }

                // We look at affine expressions only; not for correctness but
                // to keep getStepRecurrence cheap.
                if self.operands(lar).len() != 2 || self.operands(mar).len() != 2 {
                    return None;
                }

                if self.get_step_recurrence(cx, lar) != self.get_step_recurrence(cx, mar) {
                    return None;
                }

                less = self.addrec_start(lar);
                more = self.addrec_start(mar);
                continue;
            }

            // Try to match a common constant multiply.
            let match_const_mul = |se: &Self, s: Scev| -> Option<(Scev, Ap)> {
                let (c, op) = se.match_binary(s, Type::Mul, 0)?;
                Some((op, se.match_apint(c)?))
            };
            if let Some((more_op, more_c)) = match_const_mul(self, more)
                && let Some((less_op, less_c)) = match_const_mul(self, less)
                && more_c == less_c
            {
                more = more_op;
                less = less_op;
                diff_mul = diff_mul.mul(more_c);
                continue;
            }

            // Try to cancel out common factors in two add expressions.
            let mut multiplicity: BTreeMap<Scev, i32> = BTreeMap::new();
            let add = |se: &Self, s: Scev, mul: i32, diff: &mut Ap, multiplicity: &mut BTreeMap<Scev, i32>| {
                if let Some(c) = se.as_constant(s) {
                    if mul == 1 {
                        *diff = diff.add(c.mul(diff_mul));
                    } else {
                        debug_assert!(mul == -1);
                        *diff = diff.sub(c.mul(diff_mul));
                    }
                } else {
                    *multiplicity.entry(s).or_insert(0) += mul;
                }
            };
            for (s, mul) in [(more, 1), (less, -1)] {
                if matches!(self.node(s).kind, Kind::Add(_)) {
                    for op in self.operands(s).to_vec() {
                        add(self, op.scev, mul, &mut diff, &mut multiplicity);
                    }
                } else {
                    add(self, s, mul, &mut diff, &mut multiplicity);
                }
            }

            // Check whether all the non-constants cancel out, or reduce to new
            // More/Less values.
            let mut new_more: Option<Scev> = None;
            let mut new_less: Option<Scev> = None;
            for (s, mul) in &multiplicity {
                if *mul == 0 {
                    continue;
                }
                if *mul == 1 {
                    if new_more.is_some() {
                        return None;
                    }
                    new_more = Some(*s);
                } else if *mul == -1 {
                    if new_less.is_some() {
                        return None;
                    }
                    new_less = Some(*s);
                } else {
                    return None;
                }
            }

            // Values stayed the same, no point in trying further.
            if new_more == Some(more) || new_less == Some(less) {
                return None;
            }

            // Reduced to constant.
            if new_more.is_none() && new_less.is_none() {
                return Some(diff);
            }

            // Left with variable on only one side, bail out.
            let (Some(m), Some(l)) = (new_more, new_less) else { return None };
            more = m;
            less = l;
        }

        // Did not reduce to constant.
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L12960
    /// `IsKnownPredicateViaMinOrMax`: is LHS `Pred` RHS true on the virtue of
    /// LHS or RHS being a Min or Max expression?
    fn is_known_predicate_via_min_or_max(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
    ) -> bool {
        match pred {
            IntPredicate::Sge | IntPredicate::Sle => {
                if pred == IntPredicate::Sge {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // min(A, ...) <= A
                self.is_min_max_consisting_of(Type::SMin, lhs, rhs)
                    // A <= max(A, ...)
                    || self.is_min_max_consisting_of(Type::SMax, rhs, lhs)
            }
            IntPredicate::Uge | IntPredicate::Ule => {
                if pred == IntPredicate::Uge {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // min(A, ...) <= A
                // FIXME: what about umin_seq?
                self.is_min_max_consisting_of(Type::UMin, lhs, rhs)
                    // A <= max(A, ...)
                    || self.is_min_max_consisting_of(Type::UMax, rhs, lhs)
            }
            IntPredicate::Ugt | IntPredicate::Ult => {
                if pred == IntPredicate::Ugt {
                    std::mem::swap(&mut lhs, &mut rhs);
                }
                // umin(Ops) u<= each Op, so proving Op u< RHS for any Op proves
                // umin(Ops) u< RHS.
                //
                // Use computeConstantDifference instead of the more powerful
                // isKnownPredicate to keep this check cheap:
                // isKnownPredicateViaMinOrMax is called from
                // isKnownViaNonRecursiveReasoning, so recursing into the full
                // predicate prover would be expensive.
                if matches!(self.node(lhs).kind, Kind::UMin(_)) {
                    for op in self.operands(lhs).to_vec() {
                        let diff = self.compute_constant_difference(cx, rhs, op.scev);
                        // When Op and RHS share a common base differing by a
                        // constant offset D (RHS - Op = D), Op u< RHS holds iff
                        // D != 0 and RHS >= D (unsigned), i.e. the subtraction
                        // doesn't underflow.
                        if let Some(diff) = diff
                            && !diff.is_zero()
                            && self.get_unsigned_range_min(cx, rhs).bits >= diff.bits
                        {
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L13211
    /// `isKnownViaNonRecursiveReasoning`.
    pub fn is_known_via_non_recursive_reasoning(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        self.is_known_predicate_extend_idiom(pred, lhs, rhs)
            || self.is_known_predicate_via_constant_ranges(cx, pred, lhs, rhs)
            || self.is_known_predicate_via_min_or_max(cx, pred, lhs, rhs)
            || self.is_known_predicate_via_add_rec_start(cx, pred, lhs, rhs)
            || self.is_known_predicate_via_add_rec_monotonicity(cx, pred, lhs, rhs)
            || self.is_known_predicate_via_no_overflow(cx, pred, lhs, rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11444
    /// `isKnownPredicate`.
    pub fn is_known_predicate_scev(
        &mut self,
        cx: &Cx,
        mut pred: IntPredicate,
        mut lhs: Scev,
        mut rhs: Scev,
    ) -> bool {
        // Canonicalize the inputs first.
        let _ = self.simplify_icmp_operands(cx, &mut pred, &mut lhs, &mut rhs, 0);

        self.is_known_via_induction(cx, pred, lhs, rhs)
            || self.is_known_predicate_via_splitting(cx, pred, lhs, rhs)
            || self.is_known_via_non_recursive_reasoning(cx, pred, lhs, rhs)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11394
    /// `isKnownViaInduction`: the predicate holds on entry to the innermost
    /// loop the two expressions use and on its backedge, for their
    /// post-increment forms.
    fn is_known_via_induction(
        &mut self,
        cx: &Cx,
        pred: IntPredicate,
        lhs: Scev,
        rhs: Scev,
    ) -> bool {
        // First collect all loops.
        let mut loops_used = IdSet::new();
        self.get_used_loops(lhs, &mut loops_used);
        self.get_used_loops(rhs, &mut loops_used);
        let used: Vec<BlockId> = loops_used.iter().collect();
        // The loops form a linear order by domination: the innermost is the one
        // no other loop's header is properly dominated by.
        let Some(&first) = used.first() else { return false };
        let mdl = used
            .iter()
            .copied()
            .fold(
                first,
                |best, one| {
                    if cx.tree.properly_dominates(best, one) { one } else { best }
                },
            );

        // Get init and post increment value for LHS.
        let (lhs_init, lhs_post) = self.split_into_init_and_post_inc(cx, mdl, lhs);
        // If LHS contains unknown non-invariant SCEV then bail out.
        if lhs_init == self.get_could_not_compute() {
            return false;
        }
        // Get init and post increment value for RHS.
        let (rhs_init, rhs_post) = self.split_into_init_and_post_inc(cx, mdl, rhs);
        if rhs_init == self.get_could_not_compute() {
            return false;
        }
        // It is possible that init SCEV contains an invariant load but it does
        // not dominate MDL and is not available at MDL loop entry, so we should
        // check it here.
        if !self.is_available_at_loop_entry(cx, lhs_init, mdl) || !self.is_available_at_loop_entry(cx, rhs_init, mdl) {
            return false;
        }

        // It seems backedge guard check is faster than entry one so in some
        // cases it can speed up whole estimation by short circuit.
        self.is_loop_backedge_guarded_by_cond_pred(cx, mdl, pred, lhs_post, rhs_post)
            && self.is_loop_entry_guarded_by_cond_pred(cx, mdl, pred, lhs_init, rhs_init)
    }

    /// `getUsedLoops`: the loops of the add recurrences in `s`.
    pub fn get_used_loops(
        &self,
        s: Scev,
        loops_used: &mut IdSet<BlockId>,
    ) {
        let mut visited = HashSet::default();
        let mut worklist = vec![s];
        visited.insert(s);
        while let Some(s) = worklist.pop() {
            if let Some(header) = self.addrec_header(s) {
                loops_used.insert(header);
            }
            if matches!(self.node(s).kind, Kind::Constant(_) | Kind::Unknown(_)) {
                continue;
            }
            for op in self.operands_of(s) {
                if visited.insert(op.scev) {
                    worklist.push(op.scev);
                }
            }
        }
    }
}
