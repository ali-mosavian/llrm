//! `convertOrOfShiftsToFunnelShift` and `matchFunnelShift`: an `or` of two
//! opposite shifts, or of a pair of concatenations, is a funnel shift.
//!
//! Translation of
//! [`InstCombinerImpl::convertOrOfShiftsToFunnelShift`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstCombineAndOrXor.cpp#L2992)
//! and `matchFunnelShift` (:3189) at d1106deb71cc.
//!
//! MIRROR: the shift pair (constant amounts summing to the width, `Width - x`
//! with `x` known below the width, and for a rotate the masked-negation forms),
//! and the `zext` concatenation pair with its dominance and poison-flag
//! conditions.
//!
//! DEVIATE 1: no vectors; only integer constants (`undef` and `poison` lanes of
//! a constant amount are not matched).
//!
//! # Used by
//! `visitOr` ([`crate::combine_or`]), and by `SimplifyDemandedBits`
//! ([`crate::combine_demanded`]) through
//! [`Combiner::convert_or_of_shifts_to_funnel_shift`].
//!
//! # Example
//! ```text
//! %a = shl i32 %x, 5                      %r = call i32 @llvm.fshl.i32(i32 %x, i32 %y, i32 5)
//! %b = lshr i32 %y, 27           ==>
//! %r = or i32 %a, %b
//! ```

use llrm_mir::context::mask;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::valuetracking::{Query, known, pattern};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::{binop, drop_poison_generating_flags, has_one_use, query, width};

/// Which funnel shift: `fshl` or `fshr`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Funnel {
    Left,
    Right,
}

impl Funnel {
    /// The intrinsic's name.
    pub fn name(self) -> &'static str {
        match self {
            Funnel::Left => "fshl",
            Funnel::Right => "fshr",
        }
    }
}

/// `m_ZExt(v)`: the operand.
fn zext_of(
    q: &Query,
    v: Operand,
) -> Option<Operand> {
    pattern::cast(q, v, CastOp::ZExt).map(|(_, _, x)| x)
}

/// `m_And(m_Value(X), m_SpecificInt(mask))`: `X`.
fn masked(
    q: &Query,
    v: Operand,
    low: u128,
) -> Option<Operand> {
    let (x, m, _) = binop(q, v, BinaryOp::And)?;
    pattern::apint(q, m).filter(|&(bits, _)| bits == low).map(|_| x)
}

/// `m_Neg(m_Specific(x))`: `v` is `0 - x`.
fn negated(
    q: &Query,
    v: Operand,
    x: Operand,
) -> bool {
    pattern::neg(q, v).is_some_and(|(_, inner, _)| inner == x)
}

impl Combiner<'_> {
    /// `matchShiftAmount(L, R, Width)`: the amount of the funnel shift of
    /// `or (shl _, L), (lshr _, R)`, if the pair is one.
    fn match_shift_amount(
        &self,
        inst: InstId,
        shifted: (Operand, Operand),
        (l, r): (Operand, Operand),
        bits: u32,
    ) -> Option<Operand> {
        let q = query(self, Some(inst));
        let width_value = u128::from(bits);
        // Constant amounts that sum to the bit width.
        if let (Some((li, _)), Some((ri, _))) = (pattern::apint(&q, l), pattern::apint(&q, r))
            && li < width_value
            && ri < width_value
            && li + ri == width_value
        {
            return Some(self.int(self.type_of(l), li));
        }
        // (shl ShVal, X) | (lshr ShVal, (Width - x)) iff X < Width.
        if has_one_use(self, r)
            && let Some((first, second, _)) = binop(&q, r, BinaryOp::Sub)
            && second == l
            && pattern::apint(&q, first).is_some_and(|(bits_, _)| bits_ == width_value)
        {
            let known_l = known::compute_known_bits(&q, l, 0);
            return (known_l.max_value() < width_value).then_some(l);
        }
        // The cases left are rotations only.
        if shifted.0 != shifted.1 || !bits.is_power_of_two() {
            return None;
        }
        let low = width_value - 1;
        // (shl ShVal, (X & (Width - 1))) | (lshr ShVal, ((-X) & (Width - 1)))
        if let Some(x) = masked(&q, l, low)
            && masked(&q, r, low).is_some_and(|inner| negated(&q, inner, x))
        {
            return Some(x);
        }
        // (shl ShVal, (X+1) & (Width-1)) | (lshr ShVal, ((X & (Width-1)) ^
        // (Width-1)))
        if let Some(plus_one) = masked(&q, l, low)
            && let Some((x, one, _)) = binop(&q, plus_one, BinaryOp::Add)
            && pattern::one(&q, one)
            && let Some((inner, m, _)) = binop(&q, r, BinaryOp::Xor)
            && pattern::apint(&q, m).is_some_and(|(bits_, _)| bits_ == low)
            && masked(&q, inner, low) == Some(x)
        {
            return Some(plus_one);
        }
        // (shl ShVal, X) | (lshr ShVal, ((-X) & (Width - 1)))
        if masked(&q, r, low).is_some_and(|inner| negated(&q, inner, l)) {
            return Some(l);
        }
        // The shift amount may be extended after masking.
        if let Some(inner) = zext_of(&q, l)
            && let Some(x) = masked(&q, inner, low)
            && masked(&q, r, low).is_some_and(|neg| {
                pattern::neg(&q, neg)
                    .is_some_and(|(_, z, _)| zext_of(&q, z).and_then(|y| masked(&q, y, low)) == Some(x))
            })
        {
            return Some(l);
        }
        if let Some(inner) = zext_of(&q, l)
            && let Some(x) = masked(&q, inner, low)
            && zext_of(&q, r).and_then(|y| masked(&q, y, low)).is_some_and(|neg| negated(&q, neg, x))
        {
            return Some(l);
        }
        None
    }

    /// `convertOrOfShiftsToFunnelShift(Or)`: the funnel shift `or` is and its
    /// three arguments.
    pub fn convert_or_of_shifts_to_funnel_shift(
        &mut self,
        or: InstId,
    ) -> Option<(Funnel, [Operand; 3])> {
        let instruction = self.function.instruction(or);
        if instruction.opcode != Opcode::Binary(BinaryOp::Or) {
            return None;
        }
        let [op0, op1] = instruction.operands[..] else { return None };
        let bits = width(self, op0);
        let (Some(or0), Some(or1)) = (self.definition_of(op0), self.definition_of(op1)) else { return None };
        let (opcode0, opcode1) =
            (self.function.instruction(or0).opcode.clone(), self.function.instruction(or1).opcode.clone());

        // First, an or'd pair of opposite shifts.
        if matches!(opcode0, Opcode::Binary(_)) && matches!(opcode1, Opcode::Binary(_)) {
            let logical = |c: &Self, v: Operand| -> Option<(BinaryOp, Operand, Operand)> {
                if !has_one_use(c, v) {
                    return None;
                }
                let q = query(c, None);
                [BinaryOp::Shl, BinaryOp::LShr].into_iter().find_map(|op| binop(&q, v, op).map(|(x, y, _)| (op, x, y)))
            };
            let (op_0, val_0, amount_0) = logical(self, op0)?;
            let (op_1, val_1, amount_1) = logical(self, op1)?;
            if op_0 == op_1 {
                return None;
            }
            // Canonicalize to or(shl(ShVal0, ShAmt0), lshr(ShVal1, ShAmt1)).
            let ((val0, amount0), (val1, amount1)) = if op_0 == BinaryOp::LShr {
                ((val_1, amount_1), (val_0, amount_0))
            } else {
                ((val_0, amount_0), (val_1, amount_1))
            };
            let shifted = (val0, val1);
            if let Some(amount) = self.match_shift_amount(or, shifted, (amount0, amount1), bits) {
                return Some((Funnel::Left, [val0, val1, amount]));
            }
            let amount = self.match_shift_amount(or, shifted, (amount1, amount0), bits)?;
            return Some((Funnel::Right, [val0, val1, amount]));
        }

        // Two ors concatenating variables in opposite order.
        if matches!(opcode0, Opcode::Cast(CastOp::ZExt)) || matches!(opcode1, Opcode::Cast(CastOp::ZExt)) {
            let (or0, or1) = if matches!(opcode1, Opcode::Cast(CastOp::ZExt)) { (or0, or1) } else { (or1, or0) };
            let (shl_value, zext_value) = (self.value_of_inst(or0)?, self.value_of_inst(or1)?);
            let q = query(self, Some(or));
            if !has_one_use(self, shl_value) {
                return None;
            }
            let (zext_high, high_amount, _) = binop(&q, shl_value, BinaryOp::Shl)?;
            let (high_amount, _) = pattern::apint(&q, high_amount)?;
            let low = zext_of(&q, zext_value)?;
            let high = zext_of(&q, zext_high)?;
            let high_size = width(self, high);
            let low_size = width(self, low);
            // High must not overlap Low, and its top bits not be shifted out.
            if high_amount < u128::from(low_size) || high_amount > u128::from(bits - high_size) {
                return None;
            }
            let Operand::Value(zext_high_value) = zext_high else { return None };
            let users: Vec<InstId> = self.function.users(zext_high_value).iter().map(|one| one.user).collect();
            for user in users {
                let Some(user_value) = self.value_of_inst(user) else { continue };
                let q = query(self, Some(or));
                let Some((x, y, _)) = binop(&q, user_value, BinaryOp::Or) else { continue };
                let (x, y) = if zext_of(&q, y).is_some() { (x, y) } else { (y, x) };
                let Some((shifted, low_amount, _)) = binop(&q, x, BinaryOp::Shl) else { continue };
                let Some((low_amount, _)) = pattern::apint(&q, low_amount) else { continue };
                if shifted != zext_value
                    || y != zext_high
                    || !self.env.analyses.dominator_tree().instruction_dominates(self.function, user, or)
                    || low_amount + high_amount != u128::from(bits)
                {
                    continue;
                }
                // The result cannot reuse what may be poison: drop the flags.
                drop_poison_generating_flags(self, user);
                if let Some(shl) = self.definition_of(x) {
                    drop_poison_generating_flags(self, shl);
                }
                let amount = self.int(self.type_of(op0), high_amount & mask(bits));
                return Some((Funnel::Left, [user_value, user_value, amount]));
            }
        }
        None
    }

    /// The value `inst` defines.
    fn value_of_inst(
        &self,
        inst: InstId,
    ) -> Option<Operand> {
        self.function.instruction(inst).result.map(Operand::Value)
    }
}

/// `matchFunnelShift(Or, IC)`: `or` as a call of the funnel shift it is.
pub(crate) fn match_funnel_shift(
    c: &mut Combiner,
    inst: InstId,
    _: Operand,
    _: Operand,
) -> Option<Visit> {
    let (funnel, args) = c.convert_or_of_shifts_to_funnel_shift(inst)?;
    let ty = c.type_of(args[0]);
    let call = c.create_intrinsic(inst, funnel.name(), ty, &args);
    Some(Visit::Replace(call))
}

#[cfg(test)]
#[path = "combine_funnel_tests.rs"]
mod tests;
