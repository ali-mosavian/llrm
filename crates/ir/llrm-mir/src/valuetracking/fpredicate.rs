//! `fcmpImpliesClass`, `fcmpToClassTest`: which classes the operand of a
//! floating-point compare has where the compare is true or false.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `GenericFloatingPointPredicateUtils.h`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/GenericFloatingPointPredicateUtils.h))
//! with the `Function` instantiation of `FloatingPointPredicateUtils.cpp`:
//! the order of the cases, the exact class tests and the bounds for a
//! positive or negative normal or subnormal. `fcmp_implies_class` returns the
//! tested value (`None` for `Invalid`), the classes it has if the compare is
//! true and the classes if it is false. A compare that is an exact class test
//! has `if_true == !if_false`.
//!
//! - DEVIATE 1: `queryDenormalMode` answers IEEE for every function, as
//!   [`fp`](super::fp) does (DEVIATE 3 there), so a compare with zero is an
//!   exact class test.
//! - DEVIATE 2: the right operand of the `Value` form is a floating constant
//!   (`m_APFloatAllowPoison`) of `float` or `double`; an `x86_fp80` one is not
//!   matched.
//!
//! # Example
//! `fcmp ogt float %x, 0.0` is true exactly for a positive subnormal, normal or
//! infinity.
//!
//! # Used by
//! [`fp`](super::fp) (`computeKnownFPClassFromCond`).

use super::pattern::{self, IntrinsicId};
use super::query::Query;
use crate::apfloat::{ApFloat, IEEE_DOUBLE, IEEE_SINGLE};
use crate::context::ConstantKind;
use crate::fpclass::{
    DenormalMode, DenormalModeKind, FC_ALL_FLAGS, FC_FINITE, FC_INF, FC_NAN, FC_NEG_INF, FC_NEG_NORMAL,
    FC_NEG_SUBNORMAL, FC_NEG_ZERO, FC_NEGATIVE, FC_NONE, FC_POS_INF, FC_POS_NORMAL, FC_POS_SUBNORMAL, FC_POS_ZERO,
    FC_POSITIVE, FC_SUBNORMAL, FC_ZERO, FpClassTest, inverse_fabs,
};
use crate::module::Operand;
use crate::opcode::FloatPredicate;
use crate::types::{FloatKind, Type};

/// `{ TestedValue, ClassesIfTrue, ClassesIfFalse }`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Implied {
    pub value: Option<Operand>,
    pub if_true: FpClassTest,
    pub if_false: FpClassTest,
}

/// `exactClass`.
fn exact_class(
    value: Operand,
    mask: FpClassTest,
) -> Implied {
    Implied { value: Some(value), if_true: mask, if_false: !mask }
}

const INVALID: Implied = Implied { value: None, if_true: FC_ALL_FLAGS, if_false: FC_ALL_FLAGS };

/// `queryDenormalMode` (DEVIATE 1).
fn denormal_mode() -> DenormalMode {
    DenormalMode::get_ieee()
}

/// `lookThroughFAbs`: `m_FAbs(m_Value(Src))`.
fn look_through_fabs(
    q: &Query,
    lhs: Operand,
) -> Option<Operand> {
    pattern::intrinsic(q, lhs, IntrinsicId::Fabs).map(|(_, call)| pattern::call_arguments(call)[0])
}

/// `matchConstantFloat`: `m_APFloatAllowPoison` (DEVIATE 2).
pub fn match_constant_float(
    q: &Query,
    v: Operand,
) -> Option<ApFloat> {
    let Operand::Constant(id) = v else { return None };
    let constant = q.context().get(id);
    let Type::Float(kind) = *q.context().types.get(constant.ty) else { return None };
    let sem = match kind {
        FloatKind::Float => IEEE_SINGLE,
        FloatKind::Double => IEEE_DOUBLE,
        FloatKind::X86Fp80 => return None,
    };
    match constant.kind {
        ConstantKind::Float(bits) => Some(ApFloat::from_bits(sem, u128::from(bits))),
        ConstantKind::Null | ConstantKind::Zero => Some(ApFloat::from_bits(sem, 0)),
        _ => None,
    }
}

/// `fcmpToClassTest(Pred, F, LHS, RHS, LookThroughSrc)`: the value and the
/// class test that is the compare, or `(None, fcAllFlags)`.
pub fn fcmp_to_class_test(
    q: &Query,
    pred: FloatPredicate,
    lhs: Operand,
    rhs: Operand,
    look_through_src: bool,
) -> (Option<Operand>, FpClassTest) {
    let Some(constant) = match_constant_float(q, rhs) else { return (None, FC_ALL_FLAGS) };
    fcmp_to_class_test_of(q, pred, lhs, &constant, look_through_src)
}

/// `fcmpToClassTest(Pred, F, LHS, ConstRHS, LookThroughSrc)`.
pub fn fcmp_to_class_test_of(
    q: &Query,
    pred: FloatPredicate,
    lhs: Operand,
    constant: &ApFloat,
    look_through_src: bool,
) -> (Option<Operand>, FpClassTest) {
    let implied = fcmp_implies_class_of(q, pred, lhs, constant, look_through_src);
    match implied.value {
        Some(src) if implied.if_true == !implied.if_false => (Some(src), implied.if_true),
        _ => (None, FC_ALL_FLAGS),
    }
}

/// `fcmpImpliesClass(Pred, F, LHS, RHSClass, LookThroughSrc)`.
pub fn fcmp_implies_class(
    q: &Query,
    pred: FloatPredicate,
    lhs: Operand,
    rhs_class: FpClassTest,
    look_through_src: bool,
) -> Implied {
    assert!(rhs_class != FC_NONE);
    let mut src = lhs;
    if pred == FloatPredicate::True {
        return exact_class(src, FC_ALL_FLAGS);
    }
    if pred == FloatPredicate::False {
        return exact_class(src, FC_NONE);
    }
    let orig_class = rhs_class;
    let mut rhs_class = rhs_class;
    let is_negative_rhs = (rhs_class & FC_NEGATIVE) == rhs_class;
    let is_positive_rhs = (rhs_class & FC_POSITIVE) == rhs_class;
    let is_nan = (rhs_class & !FC_NAN) == FC_NONE;
    if is_nan {
        // fcmp o__ x, nan -> false
        // fcmp u__ x, nan -> true
        return exact_class(src, if pred.is_ordered() { FC_NONE } else { FC_ALL_FLAGS });
    }
    // fcmp ord x, zero|normal|subnormal|inf -> ~fcNan
    if pred == FloatPredicate::Ord {
        return exact_class(src, !FC_NAN);
    }
    // fcmp uno x, zero|normal|subnormal|inf -> fcNan
    if pred == FloatPredicate::Uno {
        return exact_class(src, FC_NAN);
    }
    let mut is_fabs = false;
    if look_through_src && let Some(inner) = look_through_fabs(q, lhs) {
        src = inner;
        is_fabs = true;
    }
    if is_fabs {
        rhs_class = inverse_fabs(rhs_class);
    }

    let is_zero = (orig_class & FC_ZERO) == orig_class;
    if is_zero {
        assert!(pred != FloatPredicate::Ord && pred != FloatPredicate::Uno);
        // Compares with fcNone are only exactly equal to fcZero if input
        // denormals are not flushed.
        // TODO (as in the pin): handle DAZ by expanding masks to cover
        // subnormal cases.
        if denormal_mode().input != DenormalModeKind::Ieee {
            return INVALID;
        }
        let exact = |mask: FpClassTest| exact_class(src, if is_fabs { inverse_fabs(mask) } else { mask });
        return match pred {
            // Match x == 0.0
            FloatPredicate::Oeq => exact_class(src, FC_ZERO),
            // Match isnan(x) || (x == 0.0)
            FloatPredicate::Ueq => exact_class(src, FC_ZERO | FC_NAN),
            // Match (x != 0.0)
            FloatPredicate::Une => exact_class(src, !FC_ZERO),
            // Match !isnan(x) && x != 0.0
            FloatPredicate::One => exact_class(src, !FC_NAN & !FC_ZERO),
            FloatPredicate::Ogt => exact(FC_POS_SUBNORMAL | FC_POS_NORMAL | FC_POS_INF),
            FloatPredicate::Ugt => exact(FC_POS_SUBNORMAL | FC_POS_NORMAL | FC_POS_INF | FC_NAN),
            FloatPredicate::Oge => exact(FC_POSITIVE | FC_NEG_ZERO),
            FloatPredicate::Uge => exact(FC_POSITIVE | FC_NEG_ZERO | FC_NAN),
            FloatPredicate::Olt => exact(FC_NEG_SUBNORMAL | FC_NEG_NORMAL | FC_NEG_INF),
            FloatPredicate::Ult => exact(FC_NEG_SUBNORMAL | FC_NEG_NORMAL | FC_NEG_INF | FC_NAN),
            FloatPredicate::Ole => exact(FC_NEGATIVE | FC_POS_ZERO),
            FloatPredicate::Ule => exact(FC_NEGATIVE | FC_POS_ZERO | FC_NAN),
            FloatPredicate::Ord | FloatPredicate::Uno | FloatPredicate::True | FloatPredicate::False => {
                unreachable!("all compare types are handled")
            }
        };
    }

    let is_denormal_rhs = (orig_class & FC_SUBNORMAL) == orig_class;
    let is_inf = (orig_class & FC_INF) == orig_class;
    if is_inf {
        let mask = match pred {
            // Match __builtin_isinf patterns.
            FloatPredicate::Oeq | FloatPredicate::Une => {
                if is_negative_rhs {
                    if is_fabs { FC_NONE } else { FC_NEG_INF }
                } else if is_fabs {
                    FC_POS_INF | FC_NEG_INF
                } else {
                    FC_POS_INF
                }
            }
            FloatPredicate::One | FloatPredicate::Ueq => {
                if is_negative_rhs {
                    if is_fabs { !FC_NAN } else { !FC_NEG_INF & !FC_NAN }
                } else {
                    let mask = !FC_POS_INF & !FC_NAN;
                    if is_fabs { mask & !FC_NEG_INF } else { mask }
                }
            }
            FloatPredicate::Olt | FloatPredicate::Uge => {
                if is_negative_rhs {
                    // No value is ordered and less than negative infinity.
                    // All values are unordered with or at least negative
                    // infinity.
                    FC_NONE
                } else if is_fabs {
                    FC_FINITE
                } else {
                    FC_FINITE | FC_NEG_INF
                }
            }
            FloatPredicate::Oge | FloatPredicate::Ult => {
                if is_negative_rhs {
                    !FC_NAN
                } else if is_fabs {
                    FC_POS_INF | FC_NEG_INF
                } else {
                    FC_POS_INF
                }
            }
            FloatPredicate::Ogt | FloatPredicate::Ule => {
                if is_negative_rhs {
                    if is_fabs { !FC_NAN } else { !(FC_NEG_INF | FC_NAN) }
                } else {
                    // No value is ordered and greater than infinity.
                    FC_NONE
                }
            }
            FloatPredicate::Ole | FloatPredicate::Ugt => {
                if is_negative_rhs {
                    if is_fabs { FC_NONE } else { FC_NEG_INF }
                } else {
                    !FC_NAN
                }
            }
            _ => unreachable!("all compare types are handled"),
        };
        // Invert the comparison for the unordered cases.
        return exact_class(src, if pred.is_unordered() { !mask } else { mask });
    }

    let implied = |if_true: FpClassTest, if_false: FpClassTest| Implied { value: Some(src), if_true, if_false };
    match pred {
        FloatPredicate::Oeq => return implied(rhs_class, FC_ALL_FLAGS),
        FloatPredicate::Ueq => return implied(rhs_class | FC_NAN, !FC_NAN),
        FloatPredicate::One => return implied(!FC_NAN, rhs_class | FC_NAN),
        FloatPredicate::Une => return implied(FC_ALL_FLAGS, rhs_class),
        _ => {}
    }

    if is_negative_rhs {
        // TODO (as in the pin): handle fneg(fabs).
        if is_fabs {
            // fabs(x) o> -k -> fcmp ord x, x
            // fabs(x) u> -k -> true
            // fabs(x) o< -k -> false
            // fabs(x) u< -k -> fcmp uno x, x
            return match pred {
                FloatPredicate::Ogt | FloatPredicate::Oge => implied(!FC_NAN, FC_NAN),
                FloatPredicate::Ugt | FloatPredicate::Uge => implied(FC_ALL_FLAGS, FC_NONE),
                FloatPredicate::Olt | FloatPredicate::Ole => implied(FC_NONE, FC_ALL_FLAGS),
                FloatPredicate::Ult | FloatPredicate::Ule => implied(FC_NAN, !FC_NAN),
                _ => INVALID,
            };
        }
        let mut classes_le = FC_NEG_INF | FC_NEG_NORMAL;
        let mut classes_ge = FC_POSITIVE | FC_NEG_ZERO | FC_NEG_SUBNORMAL;
        if is_denormal_rhs {
            classes_le |= FC_NEG_SUBNORMAL;
        } else {
            classes_ge |= FC_NEG_NORMAL;
        }
        if let Some(result) = ordered_bounds(&implied, pred, classes_ge, classes_le, rhs_class) {
            return result;
        }
    } else if is_positive_rhs {
        let mut classes_ge = FC_POS_NORMAL | FC_POS_INF;
        let mut classes_le = FC_NEGATIVE | FC_POS_ZERO | FC_POS_SUBNORMAL;
        if is_denormal_rhs {
            classes_ge |= FC_POS_SUBNORMAL;
        } else {
            classes_le |= FC_POS_NORMAL;
        }
        if is_fabs {
            classes_ge = inverse_fabs(classes_ge);
            classes_le = inverse_fabs(classes_le);
        }
        if let Some(result) = ordered_bounds(&implied, pred, classes_ge, classes_le, rhs_class) {
            return result;
        }
    }
    INVALID
}

/// The four `switch (Pred)` arms the negative and the positive right operand
/// share.
fn ordered_bounds(
    implied: &dyn Fn(FpClassTest, FpClassTest) -> Implied,
    pred: FloatPredicate,
    classes_ge: FpClassTest,
    classes_le: FpClassTest,
    rhs_class: FpClassTest,
) -> Option<Implied> {
    Some(match pred {
        FloatPredicate::Ogt | FloatPredicate::Oge => implied(classes_ge, !classes_ge | rhs_class),
        FloatPredicate::Ugt | FloatPredicate::Uge => implied(classes_ge | FC_NAN, !(classes_ge | FC_NAN) | rhs_class),
        FloatPredicate::Olt | FloatPredicate::Ole => implied(classes_le, !classes_le | rhs_class),
        FloatPredicate::Ult | FloatPredicate::Ule => implied(classes_le | FC_NAN, !(classes_le | FC_NAN) | rhs_class),
        _ => return None,
    })
}

/// `fcmpImpliesClass(Pred, F, LHS, ConstRHS, LookThroughSrc)`.
pub fn fcmp_implies_class_of(
    q: &Query,
    pred: FloatPredicate,
    lhs: Operand,
    constant: &ApFloat,
    look_through_src: bool,
) -> Implied {
    // We can refine checks against smallest normal / largest denormal to an
    // exact class test.
    if !constant.is_negative() && constant.is_smallest_normalized() {
        let mut src = lhs;
        let mut is_fabs = false;
        if look_through_src && let Some(inner) = look_through_fabs(q, lhs) {
            src = inner;
            is_fabs = true;
        }
        // Match pattern that's used in __builtin_isnormal.
        let mask = match pred {
            // fcmp olt x, smallest_normal ->
            //   fcNegInf|fcNegNormal|fcSubnormal|fcZero
            // fcmp olt fabs(x), smallest_normal -> fcSubnormal|fcZero
            FloatPredicate::Olt | FloatPredicate::Uge => {
                let mask = FC_ZERO | FC_SUBNORMAL;
                if is_fabs { mask } else { mask | FC_NEG_NORMAL | FC_NEG_INF }
            }
            // fcmp oge x, smallest_normal -> fcPosNormal | fcPosInf
            // fcmp oge fabs(x), smallest_normal -> fcInf | fcNormal
            FloatPredicate::Oge | FloatPredicate::Ult => {
                let mask = FC_POS_INF | FC_POS_NORMAL;
                if is_fabs { mask | FC_NEG_INF | FC_NEG_NORMAL } else { mask }
            }
            _ => return fcmp_implies_class(q, pred, lhs, constant.classify(), look_through_src),
        };
        // Invert the comparison for the unordered cases.
        return exact_class(src, if pred.is_unordered() { !mask } else { mask });
    }
    fcmp_implies_class(q, pred, lhs, constant.classify(), look_through_src)
}

/// `fcmpImpliesClass(Pred, F, LHS, RHS, LookThroughSrc)`.
pub fn fcmp_implies_class_value(
    q: &Query,
    pred: FloatPredicate,
    lhs: Operand,
    rhs: Operand,
    look_through_src: bool,
) -> Implied {
    let Some(constant) = match_constant_float(q, rhs) else { return INVALID };
    // TODO (as in the pin): call computeKnownFPClass for RHS to handle
    // non-constants.
    fcmp_implies_class_of(q, pred, lhs, &constant, look_through_src)
}
