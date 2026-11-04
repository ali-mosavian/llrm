//! InstCombine's min/max intrinsics: the narrowing folds of
//! `InstCombinerImpl::visitCallInst` for `umin`, `umax`, `smin` and `smax`
//! ([InstCombineCalls.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCalls.cpp#L2261)).
//!
//! # How it works
//! A min or max of two operands extended from one type, or of one extended
//! operand and a constant that fits the narrow type, is the extension of the
//! narrow min or max. The unsigned intrinsics try `zext` first, then `sext`
//! (the pin falls through from its unsigned case into its signed one); the
//! signed ones only `sext`.
//!
//! # Example
//! ```text
//! umin.i33(zext i16 %x to i33, 9)  ==>  zext (umin.i16(%x, 9)) to i33
//! ```
//!
//! MIRROR: both folds, their one-use tests and the lossless-truncation test
//! of the constant, and the `i1` fold (`and` for `umin` and `smax`, `or` for
//! `umax` and `smin`).
//!
//! MIRROR (second slice, in the pin's order): `umin(x, 1)` is `zext (x != 0)`;
//! `umax(nuw_shl(x, C), x + 1)` and its `nuw_mul` form; the canonical clamp
//! `smax(smin(X, MinC), MaxC)`; `smin(smax(X, -1), 1)` is `scmp(X, 0)`;
//! `min/max (neg nsw X), (neg nsw Y)`; `min/max X, -X` is `abs`; a constant
//! right-hand side the range of the left decides.
//!
//! MIRROR (`scmp`/`ucmp`): `foldCmpIntrinsicOfExtended` (both extends of one
//! type, or one and a constant that narrows losslessly), `scmp(X, 0)` of a
//! value in [-1, 1], and `scmp(nsw (sub X, Y), 0)`.
//!
//! DEVIATE 1: the rest of the min/max case is not translated:
//! `foldMinimumOverTrailingOrLeadingZeroCount`, `xor` of a power of two,
//! `moveNotAfterMinMax`, `moveAddAfterMinMax`, `minmax (X & NegPow2C, ...)`,
//! `foldClampRangeOfTwo`, `matchSAddSubSat`, the `reassociate*` and
//! `factorizeMinMaxTree` folds and `foldIntrinsicUsingDistributiveLaws`. Exit:
//! the next slices of `visitCallInst`.
//!
//! # Used by
//! `visit_call` (`combine_memintrinsics`).

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::valuetracking::pattern;
use llrm_mir::valuetracking::range::{
    SelectOperand, compute_constant_range, compute_constant_range_including_known_bits, is_known_negation,
};

use crate::combine::{Combiner, Visit};
use crate::combine_shared::query;

fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `getLosslessUnsignedTrunc` / `getLosslessSignedTrunc`: `value`, of `bits`,
/// in `small` bits when extending it again gives `value` back.
pub(crate) fn lossless_trunc(
    value: u128,
    bits: u32,
    small: u32,
    ext: CastOp,
) -> Option<u128> {
    let narrow = value & mask(small);
    let back = match ext {
        CastOp::SExt if narrow >> (small - 1) & 1 == 1 => narrow | (mask(bits) & !mask(small)),
        _ => narrow,
    };
    (back == value & mask(bits)).then_some(narrow)
}

impl Combiner<'_> {
    /// The operand `v` is an `ext` of, when it is one.
    fn extended_from(
        &self,
        v: Operand,
        ext: CastOp,
    ) -> Option<Operand> {
        let def = self.definition_of(v)?;
        let inst = self.function.instruction(def);
        (inst.opcode == Opcode::Cast(ext)).then(|| inst.operands[0])
    }

    /// The `umin`/`umax`/`smin`/`smax` case of `visitCallInst`, in the pin's
    /// order (`umin` falls through to `umax`, which falls through to the
    /// signed case).
    pub(crate) fn narrow_min_max(
        &mut self,
        call: InstId,
        signed: bool,
        max: bool,
    ) -> Visit {
        let operands = self.function.instruction(call).operands.clone();
        let [i0, i1, ..] = operands[..] else { return Visit::None };
        let ty = self.function.instruction(call).ty;
        let Some(bits) = self.context.types.int_bits(ty) else { return Visit::None };
        let base = min_max_name(signed, max);
        // umin(x, 1) == zext(x != 0)
        if !signed && !max && pattern::one(&query(self, None), i1) {
            let zero = self.int(self.type_of(i0), 0);
            let cmp = self.create_icmp(call, IntPredicate::Ne, i0, zero);
            return Visit::Replace(self.create_cast(call, CastOp::ZExt, cmp, ty, Flags::default()));
        }
        let exts: &[CastOp] = if signed { &[CastOp::SExt] } else { &[CastOp::ZExt, CastOp::SExt] };
        for &ext in exts {
            if let (Some(x), Some(y)) = (self.extended_from(i0, ext), self.extended_from(i1, ext))
                && self.type_of(x) == self.type_of(y)
                && (self.has_one_use(i0) || self.has_one_use(i1))
            {
                let narrow = self.create_intrinsic(call, base, self.type_of(x), &[x, y]);
                return Visit::Replace(self.create_cast(call, ext, narrow, ty, Flags::default()));
            }
            if let Some(x) = self.extended_from(i0, ext)
                && self.has_one_use(i0)
                && let Some((value, _)) = pattern::apint(&query(self, None), i1)
            {
                let small_ty = self.type_of(x);
                let small = self.context.types.int_bits(small_ty).expect("an integer");
                if let Some(narrow_c) = lossless_trunc(value, bits, small, ext) {
                    let constant = self.int(small_ty, narrow_c);
                    let narrow = self.create_intrinsic(call, base, small_ty, &[x, constant]);
                    return Visit::Replace(self.create_cast(call, ext, narrow, ty, Flags::default()));
                }
            }
            // umax(nuw_shl(x, C), x + 1), the one place between the zero and
            // the sign extension the pin tries.
            if ext == CastOp::ZExt && !signed && max {
                for (a, b) in [(i0, i1), (i1, i0)] {
                    if let Some(found) = self.fold_max_mul_shift(call, a, b) {
                        return found;
                    }
                }
            }
        }
        if let Some(found) = self.canonical_clamp(call, signed, max, i0, i1) {
            return found;
        }
        // umin(i1 X, i1 Y) -> and i1 X, Y
        // smax(i1 X, i1 Y) -> and i1 X, Y
        // umax(i1 X, i1 Y) -> or i1 X, Y
        // smin(i1 X, i1 Y) -> or i1 X, Y
        if bits == 1 {
            let op = if max == signed { BinaryOp::And } else { BinaryOp::Or };
            return self.replace_with_binop(call, op, i0, i1, Flags::default());
        }
        // smin(smax(X, -1), 1) -> scmp(X, 0)
        if signed && !max {
            let q = query(self, None);
            if let Some((_, [x, minus_one])) =
                pattern::intrinsic_call(&q, i0, "llvm.smax").map(|(i, a)| (i, [a[0], a[1]]))
                && pattern::all_ones(&q, minus_one)
                && pattern::one(&q, i1)
                && self.has_one_use(i0)
            {
                let zero = self.int(self.type_of(x), 0);
                let name = format!(
                    "llvm.scmp.i{}.i{}",
                    bits,
                    self.context.types.int_bits(self.type_of(x)).expect("an integer")
                );
                return Visit::Replace(self.create_intrinsic_named(call, &name, ty, &[x, zero]));
            }
        }
        if signed && let Some(found) = self.negated_operands(call, max, i0, i1) {
            return found;
        }
        if let Some(found) = self.negation_is_abs(call, signed, max, i0, i1) {
            return found;
        }
        self.constant_rhs_by_range(call, signed, max, i0, i1)
    }

    /// `foldMaxMulShift(A, B)`: if C is not 0, `umax(nuw_shl(x, C), x + 1)` is
    /// `x == 0 ? 1 : nuw_shl(x, C)`; if C is not 0 or 1, the same for
    /// `nuw_mul`.
    fn fold_max_mul_shift(
        &mut self,
        call: InstId,
        a: Operand,
        b: Operand,
    ) -> Option<Visit> {
        let q = query(self, None);
        let (x, c) = if let Some((_, inst, x, c)) = pattern::binary(&q, a, BinaryOp::Shl)
            && inst.flags.contains(Flags::NUW)
        {
            (x, pattern::apint(&q, c)?)
        } else {
            let (_, inst, x, c) = pattern::binary(&q, a, BinaryOp::Mul)?;
            let c = pattern::apint(&q, c)?;
            (inst.flags.contains(Flags::NUW) && c.0 != 1).then_some((x, c))?
        };
        if c.0 == 0 {
            return None;
        }
        let (_, _, y, one) = pattern::binary(&q, b, BinaryOp::Add)?;
        if y != x || !pattern::one(&q, one) || !self.has_one_use(b) {
            return None;
        }
        let ty = self.type_of(x);
        let zero = self.int(ty, 0);
        let cmp = self.create_icmp(call, IntPredicate::Eq, x, zero);
        let one = self.int(ty, 1);
        Some(Visit::Replace(self.create_select(call, cmp, one, a)))
    }

    /// `smax(smin(X, MinC), MaxC)` is `smin(smax(X, MaxC), MinC)` if
    /// `MinC s> MaxC`, and the same unsigned.
    fn canonical_clamp(
        &mut self,
        call: InstId,
        signed: bool,
        max: bool,
        i0: Operand,
        i1: Operand,
    ) -> Option<Visit> {
        if !max {
            return None;
        }
        let q = query(self, None);
        let inner = if signed { "llvm.smin" } else { "llvm.umin" };
        let (_, args) = pattern::intrinsic_call(&q, i0, inner)?;
        let (x, min_c) = (args[0], pattern::apint(&q, args[1])?);
        let max_c = pattern::apint(&q, i1)?;
        if !self.has_one_use(i0) || !greater(min_c, max_c, signed) {
            return None;
        }
        let ty = self.type_of(x);
        let (max_name, min_name) = if signed { ("smax", "smin") } else { ("umax", "umin") };
        let max_c = self.int(ty, max_c.0);
        let min_c = self.int(ty, min_c.0);
        let new_max = self.create_intrinsic(call, max_name, ty, &[x, max_c]);
        Some(Visit::Replace(self.create_intrinsic(call, min_name, ty, &[new_max, min_c])))
    }

    /// `smax (neg nsw X), (neg nsw Y)` is `neg nsw (smin X, Y)`, and the
    /// mirror for `smin`.
    fn negated_operands(
        &mut self,
        call: InstId,
        max: bool,
        i0: Operand,
        i1: Operand,
    ) -> Option<Visit> {
        let q = query(self, None);
        let (_, x, f0) = pattern::neg(&q, i0)?;
        let (_, y, f1) = pattern::neg(&q, i1)?;
        if !f0.contains(Flags::NSW) || !f1.contains(Flags::NSW) || !(self.has_one_use(i0) || self.has_one_use(i1)) {
            return None;
        }
        let ty = self.type_of(x);
        let inverse = self.create_intrinsic(call, min_max_name(true, !max), ty, &[x, y]);
        let zero = self.int(ty, 0);
        Some(Visit::Replace(self.create_binop(call, BinaryOp::Sub, zero, inverse, Flags::NSW)))
    }

    /// `smax(X, -X)` is `abs(X)`, `smin(X, -X)` is `-abs(X)`, `umax(X, -X)`
    /// is `-abs(X)`, `umin(X, -X)` is `abs(X)`.
    fn negation_is_abs(
        &mut self,
        call: InstId,
        signed: bool,
        max: bool,
        i0: Operand,
        i1: Operand,
    ) -> Option<Visit> {
        let q = query(self, None);
        let (a, b) = (SelectOperand::Value(i0), SelectOperand::Value(i1));
        if !is_known_negation(&q, a, b, false, true) {
            return None;
        }
        // Either operand can be abs()'s input; eliminating the only use of a
        // value is better for the folds after.
        let (i0, i1) = if self.has_one_use(i0) && !self.has_one_use(i1) { (i1, i0) } else { (i0, i1) };
        let q = query(self, None);
        let int_min_is_poison = is_known_negation(&q, SelectOperand::Value(i0), SelectOperand::Value(i1), true, true);
        let flag = self.int(self.context.types.int(1), u128::from(int_min_is_poison));
        let ty = self.type_of(i0);
        let abs = self.create_intrinsic(call, "abs", ty, &[i0, flag]);
        // There is no `nabs` intrinsic: negate by the operation.
        let result = if (signed && !max) || (!signed && max) {
            let zero = self.int(ty, 0);
            let flags = if int_min_is_poison { Flags::NSW } else { Flags::default() };
            self.create_binop(call, BinaryOp::Sub, zero, abs, flags)
        } else {
            abs
        };
        Some(Visit::Replace(result))
    }

    /// A constant right-hand side the range of the left decides: the left
    /// when it is already on the right side of the constant, else the
    /// constant.
    fn constant_rhs_by_range(
        &mut self,
        call: InstId,
        signed: bool,
        max: bool,
        i0: Operand,
        i1: Operand,
    ) -> Visit {
        let q = query(self, Some(call));
        let Some((value, width)) = pattern::apint(&q, i1) else { return Visit::None };
        let pred = match (signed, max) {
            (false, false) => IntPredicate::Ule,
            (false, true) => IntPredicate::Uge,
            (true, false) => IntPredicate::Sle,
            (true, true) => IntPredicate::Sge,
        };
        let left = compute_constant_range_including_known_bits(&q, i0, signed);
        if left.is_full_set() {
            return Visit::None;
        }
        let rhs = ConstantRange::single(width, value);
        if left.icmp(pred, &rhs) {
            return Visit::Replace(i0);
        }
        if left.icmp(swapped(pred), &rhs) {
            return Visit::Replace(i1);
        }
        Visit::None
    }
}

impl Combiner<'_> {
    /// The `scmp`/`ucmp` case of `visitCallInst`.
    pub(crate) fn visit_compare(
        &mut self,
        call: InstId,
        signed: bool,
    ) -> Visit {
        let operands = self.function.instruction(call).operands.clone();
        let [i0, i1, ..] = operands[..] else { return Visit::None };
        let ty = self.function.instruction(call).ty;
        if let Some(found) = self.compare_of_extended(call, signed, i0, i1) {
            return Visit::Replace(found);
        }
        if !signed {
            return Visit::None;
        }
        let q = query(self, Some(call));
        // scmp(X, 0) -> sext_or_trunc(X) if X is known to be one of -1, 0, 1.
        if pattern::zero(&q, i1) {
            let width = q.scalar_bits(i0);
            let range = compute_constant_range(&q, i0, true, 0);
            if ConstantRange::new(width, mask(width), 2).contains_range(&range) {
                let (from, to) = (width, self.context.types.int_bits(ty).expect("an integer"));
                let widened = match from.cmp(&to) {
                    std::cmp::Ordering::Less => self.create_cast(call, CastOp::SExt, i0, ty, Flags::default()),
                    std::cmp::Ordering::Greater => self.create_cast(call, CastOp::Trunc, i0, ty, Flags::default()),
                    std::cmp::Ordering::Equal => i0,
                };
                return Visit::Replace(widened);
            }
            // scmp(nsw (sub LHS, RHS), 0) -> scmp(LHS, RHS)
            if let Some((_, instruction, lhs, rhs)) = pattern::binary(&q, i0, BinaryOp::Sub)
                && instruction.flags.contains(Flags::NSW)
            {
                let name = self.compare_name(true, ty, lhs);
                return Visit::Replace(self.create_intrinsic_named(call, &name, ty, &[lhs, rhs]));
            }
        }
        Visit::None
    }

    /// `llvm.scmp.i<result>.i<operand>` (or `ucmp`).
    fn compare_name(
        &self,
        signed: bool,
        ty: llrm_mir::types::TypeId,
        operand: Operand,
    ) -> String {
        let result = self.context.types.int_bits(ty).expect("an integer");
        let argument = self.context.types.int_bits(self.type_of(operand)).expect("an integer");
        format!("llvm.{}.i{result}.i{argument}", if signed { "scmp" } else { "ucmp" })
    }

    /// `foldCmpIntrinsicOfExtended`: `scmp (sext X), (sext Y)` is `scmp X, Y`,
    /// `scmp (zext X), (zext Y)` is `ucmp X, Y`, `ucmp (ext X), (ext Y)` is
    /// `ucmp X, Y`; a constant operand is narrowed if that is lossless.
    fn compare_of_extended(
        &mut self,
        call: InstId,
        signed: bool,
        i0: Operand,
        i1: Operand,
    ) -> Option<Operand> {
        let ext_of = |c: &Self, v: Operand| {
            [CastOp::ZExt, CastOp::SExt].into_iter().find_map(|ext| c.extended_from(v, ext).map(|x| (ext, x)))
        };
        // scmp/ucmp are not commutative: the extend may be on either side.
        let (index, (ext, x)) = match ext_of(self, i0) {
            Some(found) => (0, found),
            None => (1, ext_of(self, i1)?),
        };
        let narrow = self.type_of(x);
        let other = [i0, i1][1 - index];
        let y = if let Some((other_ext, y)) = ext_of(self, other) {
            if other_ext != ext || self.type_of(y) != narrow {
                return None;
            }
            y
        } else {
            let q = query(self, None);
            let (value, width) = pattern::apint(&q, other)?;
            let small = self.context.types.int_bits(narrow).expect("an integer");
            self.int(narrow, lossless_trunc(value, width, small, ext)?)
        };
        // Both extends keep the unsigned order; only sext keeps the signed one.
        let signed = signed && ext == CastOp::SExt;
        let (x, y) = if index == 0 { (x, y) } else { (y, x) };
        let ty = self.function.instruction(call).ty;
        let name = self.compare_name(signed, ty, x);
        Some(self.create_intrinsic_named(call, &name, ty, &[x, y]))
    }
}

fn min_max_name(
    signed: bool,
    max: bool,
) -> &'static str {
    match (signed, max) {
        (false, false) => "umin",
        (false, true) => "umax",
        (true, false) => "smin",
        (true, true) => "smax",
    }
}

/// `ICmpInst::getSwappedPredicate`, of the non-strict predicates.
fn swapped(pred: IntPredicate) -> IntPredicate {
    match pred {
        IntPredicate::Ule => IntPredicate::Uge,
        IntPredicate::Uge => IntPredicate::Ule,
        IntPredicate::Sle => IntPredicate::Sge,
        _ => IntPredicate::Sle,
    }
}

/// `a > b` of two constants `(bits, width)`, signed or unsigned.
fn greater(
    a: (u128, u32),
    b: (u128, u32),
    signed: bool,
) -> bool {
    let value = |(bits, width): (u128, u32)| {
        if signed && width < 128 && bits >> (width - 1) & 1 == 1 { (bits | !mask(width)) as i128 } else { bits as i128 }
    };
    if signed { value(a) > value(b) } else { a.0 > b.0 }
}

#[cfg(test)]
#[path = "combine_minmax_tests.rs"]
mod tests;
