//! `SCEVPatternMatch.h`: the matchers, as functions returning what they bind.
//! A pattern with sub-patterns is the function and the caller's tests of what
//! it returns.

use llrm_mir::module::BlockId;

use super::ap::Ap;
use super::expr::{Kind, Scev, Type};
use super::{Cx, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolutionExpressions.h#L354
    /// `SCEVAddRecExpr::isAffine`: `{start,+,step}`.
    pub fn is_affine(
        &self,
        ar: Scev,
    ) -> bool {
        self.operands(ar).len() == 2
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolutionExpressions.h#L363
    /// `SCEVAddRecExpr::isQuadratic`: `{a,+,b,+,c}`.
    pub fn is_quadratic(
        &self,
        ar: Scev,
    ) -> bool {
        self.operands(ar).len() == 3
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolutionExpressions.h#L221
    /// `SCEVNAryExpr::hasNoSelfWrap`.
    pub fn has_no_self_wrap(
        &self,
        ar: Scev,
    ) -> bool {
        self.get_no_wrap_flags(ar, super::expr::flags::NW) != 0
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L440
    /// `SCEV::isNonConstantNegative`: a product whose constant factor is
    /// negative, such as `-42 * %v`.
    pub fn is_non_constant_negative(
        &self,
        s: Scev,
    ) -> bool {
        let Kind::Mul(ops) = &self.node(s).kind else { return false };
        // If there is a constant factor, it will be first.
        let Some(factor) = ops.first().and_then(|op| self.as_constant(op.scev)) else { return false };
        // Return true if the value is negative, this matches things like
        // (-42 * V).
        factor.is_negative()
    }

    /// `m_scev_APInt`.
    pub fn match_apint(
        &self,
        s: Scev,
    ) -> Option<Ap> {
        self.as_constant(s)
    }

    /// `m_scev_Zero`.
    pub fn match_zero(
        &self,
        s: Scev,
    ) -> bool {
        self.as_constant(s).is_some_and(|value| value.is_zero())
    }

    /// `m_scev_One`.
    pub fn match_one(
        &self,
        s: Scev,
    ) -> bool {
        self.as_constant(s).is_some_and(|value| value.is_one())
    }

    /// `m_scev_AllOnes`.
    pub fn match_all_ones(
        &self,
        s: Scev,
    ) -> bool {
        self.as_constant(s).is_some_and(|value| value.is_all_ones())
    }

    /// `m_scev_Unary<SCEVTy>`: the operand of a cast of kind `tag`.
    pub fn match_unary(
        &self,
        s: Scev,
        tag: Type,
    ) -> Option<Scev> {
        match &self.node(s).kind {
            Kind::Truncate(op) if tag == Type::Truncate => Some(op.scev),
            Kind::ZeroExtend(op) if tag == Type::ZeroExtend => Some(op.scev),
            Kind::SignExtend(op) if tag == Type::SignExtend => Some(op.scev),
            Kind::PtrToAddr(op) if tag == Type::PtrToAddr => Some(op.scev),
            _ => None,
        }
    }

    /// `m_scev_Binary<SCEVTy, .., WrapFlags>`: the two operands of a
    /// two-operand expression of kind `tag` that has `wrap`. The caller tries
    /// both orders where the pattern is commutable.
    pub fn match_binary(
        &self,
        s: Scev,
        tag: Type,
        wrap: u8,
    ) -> Option<(Scev, Scev)> {
        if self.is_nary(s) && self.get_no_wrap_flags(s, wrap) != wrap {
            return None;
        }
        if self.scev_type(s) != tag {
            return None;
        }
        let ops = self.operands_of(s);
        (ops.len() == 2).then(|| (ops[0].scev, ops[1].scev))
    }

    /// `isa<SCEVNAryExpr>`.
    pub fn is_nary(
        &self,
        s: Scev,
    ) -> bool {
        matches!(
            self.node(s).kind,
            Kind::Add(_)
                | Kind::Mul(_)
                | Kind::AddRec(..)
                | Kind::UMax(_)
                | Kind::SMax(_)
                | Kind::UMin(_)
                | Kind::SMin(_)
                | Kind::SequentialUMin(_)
        )
    }

    /// `m_scev_AffineAddRec`: start, step and loop of a two-operand add
    /// recurrence.
    pub fn match_affine_add_rec(
        &self,
        s: Scev,
    ) -> Option<(Scev, Scev, BlockId)> {
        let (start, step) = self.match_binary(s, Type::AddRec, 0)?;
        Some((start, step, self.addrec_header(s)?))
    }

    /// `m_scev_URem`: the dividend and divisor of an expression of the form
    /// `getURemExpr` makes.
    pub fn match_urem(
        &mut self,
        cx: &Cx,
        expr: Scev,
    ) -> Option<(Scev, Scev)> {
        if self.type_of(expr).is_pointer() {
            return None;
        }

        // Try to match 'zext (trunc A to iB) to iY', which is used for URem
        // with constant power-of-2 second operands. Make sure the size of the
        // operand A matches the size of the whole expressions.
        if let Some(trunc) = self.match_unary(expr, Type::ZeroExtend)
            && let Some(a) = self.match_unary(trunc, Type::Truncate)
        {
            let trunc_ty = self.type_of(trunc);
            // Bail out if the type of the LHS is larger than the type of the
            // expression for now.
            let expr_bits = self.get_type_size_in_bits(cx, self.type_of(expr));
            if self.get_type_size_in_bits(cx, self.type_of(a)) > expr_bits {
                return None;
            }
            let mut lhs = a;
            if self.type_of(lhs) != self.type_of(expr) {
                lhs = self.get_zero_extend_expr(cx, super::expr::Use::of(lhs), self.type_of(expr), 0);
            }
            let rhs = self.get_constant(Ap::one(expr_bits).shl(self.get_type_size_in_bits(cx, trunc_ty)));
            return Some((lhs, rhs));
        }

        let (mul, a) = {
            let (x, y) = self.match_binary(expr, Type::Add, 0)?;
            if self.scev_type(x) == Type::Mul { (x, y) } else { return None }
        };

        // URem is represented as `A - ((A udiv B) * B)`. Only construct the
        // complex SCEV expression, if the multiply of the expression to check
        // has a UDiv operand.
        let mul_ops = self.operands_of(mul);
        if !mul_ops.iter().any(|op| self.scev_type(op.scev) == Type::UDiv) {
            return None;
        }

        let match_with_divisor = |se: &mut Self, b: Scev| -> Option<(Scev, Scev)> {
            // (SomeExpr + (-(SomeExpr / B) * B)).
            let urem = se.get_urem_expr(cx, super::expr::Use::of(a), super::expr::Use::of(b));
            (expr == urem).then_some((a, b))
        };

        // (SomeExpr + (-1 * (SomeExpr / B) * B)).
        if mul_ops.len() == 3 && self.as_constant(mul_ops[0].scev).is_some() {
            return match_with_divisor(self, mul_ops[1].scev).or_else(|| match_with_divisor(self, mul_ops[2].scev));
        }

        // (SomeExpr + ((-SomeExpr / B) * B)) or (SomeExpr + ((SomeExpr / B) *
        // -B)).
        if mul_ops.len() == 2 {
            if let Some(found) = match_with_divisor(self, mul_ops[1].scev) {
                return Some(found);
            }
            if let Some(found) = match_with_divisor(self, mul_ops[0].scev) {
                return Some(found);
            }
            let negated = self.get_negative_scev(cx, mul_ops[1].scev, 0);
            if let Some(found) = match_with_divisor(self, negated) {
                return Some(found);
            }
            let negated = self.get_negative_scev(cx, mul_ops[0].scev, 0);
            return match_with_divisor(self, negated);
        }
        None
    }
}
