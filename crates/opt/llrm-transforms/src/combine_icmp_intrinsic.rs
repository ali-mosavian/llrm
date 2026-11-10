//! `icmp` of an intrinsic and a constant.
//!
//! Translation of the pin's (d1106deb71cc, InstCombineCompares.cpp)
//! [`foldICmpIntrinsicWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4329),
//! its equality half
//! [`foldICmpEqIntrinsicWithConstant`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L3929)
//! and the intrinsic arm of
//! [`foldICmpInstWithConstantAllowPoison`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineCompares.cpp#L4102).
//!
//! MIRROR: the arms and their order. `abs`, `bswap`, `bitreverse`, `ctlz`,
//! `cttz`, `ctpop`, `fshl`, `fshr`, `umax`, `uadd.sat`, `ssub.sat` and
//! `usub.sat` against a constant, equality first, then the relational arms of
//! `ctpop`, `ctlz`, `cttz`, `ssub.sat` and `abs`.
//!
//! OWED: `foldCtpopPow2Test` (known bits),
//! `foldICmpUSubSatOrUAddSatWithConstant`, `foldICmpOfCmpIntrinsicWithConstant`
//! (`scmp`/`ucmp`) and `foldICmpIntrinsicWithIntrinsic`.
//!
//! # Used by
//! `fold_icmp_inst_with_constant`.
//!
//! # Example
//! ```text
//! %c = call i64 @llvm.ctpop.i64(i64 %x)
//! %r = icmp eq i64 %c, 0                    ==>   %r = icmp eq i64 %x, 0
//! ```

use llrm_mir::context::{mask, signed};
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate};
use llrm_mir::valuetracking::pattern;

use crate::combine::{Combiner, Visit};
use crate::combine_icmp_const::new_icmp;
use crate::combine_shared::{has_one_use, query};

/// The intrinsics this file folds.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Abs,
    Bswap,
    Bitreverse,
    Ctlz,
    Cttz,
    Ctpop,
    Fshl,
    Fshr,
    Umax,
    UaddSat,
    SsubSat,
    UsubSat,
}

const KINDS: [(&str, Kind); 12] = [
    ("llvm.abs", Kind::Abs),
    ("llvm.bswap", Kind::Bswap),
    ("llvm.bitreverse", Kind::Bitreverse),
    ("llvm.ctlz", Kind::Ctlz),
    ("llvm.cttz", Kind::Cttz),
    ("llvm.ctpop", Kind::Ctpop),
    ("llvm.fshl", Kind::Fshl),
    ("llvm.fshr", Kind::Fshr),
    ("llvm.umax", Kind::Umax),
    ("llvm.uadd.sat", Kind::UaddSat),
    ("llvm.ssub.sat", Kind::SsubSat),
    ("llvm.usub.sat", Kind::UsubSat),
];

/// `dyn_cast<IntrinsicInst>(v)` of one of `KINDS`: which, and its arguments.
fn intrinsic_of(
    c: &Combiner,
    v: Operand,
) -> Option<(Kind, Vec<Operand>)> {
    let q = query(c, None);
    KINDS.into_iter().find_map(|(base, kind)| {
        let (_, args) = pattern::intrinsic_call(&q, v, base)?;
        Some((kind, args.to_vec()))
    })
}

/// `APInt::getLowBitsSet(width, n)`.
fn low_bits(
    width: u32,
    n: u32,
) -> u128 {
    mask(n.min(width))
}

/// `APInt::getHighBitsSet(width, n)`.
fn high_bits(
    width: u32,
    n: u32,
) -> u128 {
    mask(width) & !mask(width - n.min(width))
}

/// `APInt::rotr`.
fn rotr(
    value: u128,
    amount: u128,
    width: u32,
) -> u128 {
    let by = (amount % u128::from(width)) as u32;
    if by == 0 { value } else { ((value >> by) | (value << (width - by))) & mask(width) }
}

/// `foldICmpIntrinsicWithConstant`, entered from `foldICmpInstWithConstant`,
/// and the intrinsic arm of `foldICmpInstWithConstantAllowPoison`.
pub(crate) fn fold_icmp_intrinsic_with_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    value: u128,
    width: u32,
) -> Option<Visit> {
    let (kind, args) = intrinsic_of(c, op0)?;
    let equality = matches!(pred, IntPredicate::Eq | IntPredicate::Ne);
    if equality && let Some(visit) = fold_icmp_eq_intrinsic_with_constant(c, inst, pred, op0, kind, &args, value, width)
    {
        return Some(visit);
    }
    if !equality && let Some(visit) = fold_icmp_relational(c, inst, pred, op0, kind, &args, value, width) {
        return Some(visit);
    }
    // foldICmpInstWithConstantAllowPoison: (rot X, ?) == 0/-1 --> X == 0/-1.
    if matches!(kind, Kind::Fshl | Kind::Fshr)
        && equality
        && args[0] == args[1]
        && (value == 0 || value == mask(width))
    {
        let constant = c.int(c.type_of(op0), value);
        return Some(new_icmp(c, inst, pred, args[0], constant));
    }
    None
}

/// `foldICmpEqIntrinsicWithConstant`.
fn fold_icmp_eq_intrinsic_with_constant(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    kind: Kind,
    args: &[Operand],
    value: u128,
    width: u32,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    let min_signed = 1u128 << (width - 1);
    match kind {
        // abs(A) == 0 -> A == 0; abs(A) == INT_MIN -> A == INT_MIN
        Kind::Abs if value == 0 || value == min_signed => {
            let constant = c.int(ty, value);
            Some(new_icmp(c, inst, pred, args[0], constant))
        }
        // bswap(A) == C -> A == bswap(C)
        Kind::Bswap if width % 8 == 0 => {
            let constant = c.int(ty, value.swap_bytes() >> (128 - width));
            Some(new_icmp(c, inst, pred, args[0], constant))
        }
        // bitreverse(A) == C -> A == bitreverse(C)
        Kind::Bitreverse => {
            let constant = c.int(ty, value.reverse_bits() >> (128 - width));
            Some(new_icmp(c, inst, pred, args[0], constant))
        }
        Kind::Ctlz | Kind::Cttz => {
            // ctz(A) == bitwidth(A) -> A == 0
            if value == u128::from(width) {
                let zero = c.int(ty, 0);
                return Some(new_icmp(c, inst, pred, args[0], zero));
            }
            // ctz(A) == C -> A & Mask1 == Mask2, only with one use.
            let num = value.min(u128::from(width)) as u32;
            if num != width && has_one_use(c, op0) {
                let trailing = kind == Kind::Cttz;
                let (mask1, mask2) = if trailing {
                    (low_bits(width, num + 1), 1u128 << num)
                } else {
                    (high_bits(width, num + 1), 1u128 << (width - num - 1))
                };
                let mask1 = c.int(ty, mask1);
                let masked = c.create_binop(inst, BinaryOp::And, args[0], mask1, Flags::default());
                let mask2 = c.int(ty, mask2);
                return Some(new_icmp(c, inst, pred, masked, mask2));
            }
            None
        }
        // popcount(A) == 0 -> A == 0; popcount(A) == bitwidth(A) -> A == -1
        Kind::Ctpop if value == 0 || value == u128::from(width) => {
            let constant = c.int(ty, if value == 0 { 0 } else { u128::MAX });
            Some(new_icmp(c, inst, pred, args[0], constant))
        }
        // rot(X, RotAmtC) == C -> X == rot'(C, RotAmtC)
        Kind::Fshl | Kind::Fshr if args[0] == args[1] => {
            let (amount, _) = query(c, None).int_constant(args[2])?;
            let rotated =
                if kind == Kind::Fshl { rotr(value, amount, width) } else { rotr(value, u128::from(width) - amount % u128::from(width), width) };
            let constant = c.int(ty, rotated);
            Some(new_icmp(c, inst, pred, args[0], constant))
        }
        // uadd.sat(a, b) == 0 and umax(a, b) == 0 -> (a | b) == 0
        Kind::Umax | Kind::UaddSat if value == 0 && has_one_use(c, op0) => {
            let or = c.create_binop(inst, BinaryOp::Or, args[0], args[1], Flags::default());
            let zero = c.int(ty, 0);
            Some(new_icmp(c, inst, pred, or, zero))
        }
        // ssub.sat(a, b) == 0 -> a == b, not for i1
        Kind::SsubSat if value == 0 && width > 1 => Some(new_icmp(c, inst, pred, args[0], args[1])),
        // usub.sat(a, b) == 0 -> a <= b
        Kind::UsubSat if value == 0 => {
            let new_pred = if pred == IntPredicate::Eq { IntPredicate::Ule } else { IntPredicate::Ugt };
            Some(new_icmp(c, inst, new_pred, args[0], args[1]))
        }
        _ => None,
    }
}

/// The relational half of `foldICmpIntrinsicWithConstant`.
fn fold_icmp_relational(
    c: &mut Combiner,
    inst: InstId,
    pred: IntPredicate,
    op0: Operand,
    kind: Kind,
    args: &[Operand],
    value: u128,
    width: u32,
) -> Option<Visit> {
    let ty = c.type_of(op0);
    let bits = u128::from(width);
    match kind {
        Kind::Ctpop => {
            // (ctpop X > BitWidth - 1) --> X == -1
            if value == bits - 1 && pred == IntPredicate::Ugt {
                let ones = c.all_ones(ty);
                return Some(new_icmp(c, inst, IntPredicate::Eq, args[0], ones));
            }
            // (ctpop X < BitWidth) --> X != -1
            if value == bits && pred == IntPredicate::Ult {
                let ones = c.all_ones(ty);
                return Some(new_icmp(c, inst, IntPredicate::Ne, args[0], ones));
            }
            None
        }
        Kind::Ctlz => {
            // ctlz(0bXXXXXXXX) > 3 -> 0bXXXXXXXX < 0b00010000
            if pred == IntPredicate::Ugt && value < bits {
                let limit = c.int(ty, 1u128 << (width - value as u32 - 1));
                return Some(new_icmp(c, inst, IntPredicate::Ult, args[0], limit));
            }
            // ctlz(0bXXXXXXXX) < 3 -> 0bXXXXXXXX > 0b00011111
            if pred == IntPredicate::Ult && value >= 1 && value <= bits {
                let limit = c.int(ty, low_bits(width, width - value as u32));
                return Some(new_icmp(c, inst, IntPredicate::Ugt, args[0], limit));
            }
            None
        }
        Kind::Cttz => {
            // Limit to one use to ensure we don't increase instruction count.
            if !has_one_use(c, op0) {
                return None;
            }
            let zero = c.int(ty, 0);
            // cttz(0bXXXXXXXX) > 3 -> 0bXXXXXXXX & 0b00001111 == 0
            if pred == IntPredicate::Ugt && value < bits {
                let mask = c.int(ty, low_bits(width, value as u32 + 1));
                let masked = c.create_binop(inst, BinaryOp::And, args[0], mask, Flags::default());
                return Some(new_icmp(c, inst, IntPredicate::Eq, masked, zero));
            }
            // cttz(0bXXXXXXXX) < 3 -> 0bXXXXXXXX & 0b00000111 != 0
            if pred == IntPredicate::Ult && value >= 1 && value <= bits {
                let mask = c.int(ty, low_bits(width, value as u32));
                let masked = c.create_binop(inst, BinaryOp::And, args[0], mask, Flags::default());
                return Some(new_icmp(c, inst, IntPredicate::Ne, masked, zero));
            }
            None
        }
        // ssub.sat(a, b) spred 0 -> a spred b; not for i1.
        Kind::SsubSat => {
            let is_signed = matches!(
                pred,
                IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
            );
            if !is_signed || width <= 1 {
                return None;
            }
            if value == 0 {
                return Some(new_icmp(c, inst, pred, args[0], args[1]));
            }
            // X s<= 0 is canonicalized to X s< 1
            if pred == IntPredicate::Slt && value == 1 {
                return Some(new_icmp(c, inst, IntPredicate::Sle, args[0], args[1]));
            }
            // X s>= 0 is canonicalized to X s> -1
            if pred == IntPredicate::Sgt && value == mask(width) {
                return Some(new_icmp(c, inst, IntPredicate::Sge, args[0], args[1]));
            }
            None
        }
        Kind::Abs => {
            if !has_one_use(c, op0) {
                return None;
            }
            let signed_value = signed(value, width);
            // If C >= 0: abs(X) u> C --> X + C u> 2 * C
            if pred == IntPredicate::Ugt && signed_value >= 0 {
                let constant = c.int(ty, value);
                let sum = c.create_add(inst, args[0], constant);
                let twice = c.int(ty, value.wrapping_mul(2));
                return Some(new_icmp(c, inst, IntPredicate::Ugt, sum, twice));
            }
            // If C >= 1: abs(X) u< C --> X + (C - 1) u<= 2 * (C - 1)
            if pred == IntPredicate::Ult && signed_value >= 1 {
                let less = value.wrapping_sub(1);
                let constant = c.int(ty, less);
                let sum = c.create_add(inst, args[0], constant);
                let twice = c.int(ty, less.wrapping_mul(2));
                return Some(new_icmp(c, inst, IntPredicate::Ule, sum, twice));
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "combine_icmp_intrinsic_tests.rs"]
mod tests;
