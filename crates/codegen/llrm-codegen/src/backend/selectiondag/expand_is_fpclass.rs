//! `TargetLowering::expandIS_FPCLASS`, from
//! [`TargetLowering.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L10462)
//! at d1106deb71cc.
//!
//! MIRROR: the order of the arms and each condition.
//!
//! DEVIATE: the denormal mode is IEEE (MIR has no `denormal-fp-math`), so the
//! `fcmp` against zero tests only the zeros alone; `isFAbsFree` is false (the
//! default, which AArch64 and X86 keep); scalars only (the DAG has no `f16`,
//! `bf16` or `ppcf128`, and `llvm.is.fpclass` of a vector is refused earlier).
//!
//! # How it works
//! A class test is a float compare where one reads it the same
//! (`isnan` is `x != x`, `isinf` is `fabs(x) == inf`, `iszero` is `x == 0`);
//! otherwise it is integer compares of the bits: the exponent and mantissa as
//! an unsigned number against the bits of infinity, the sign bit apart.
//!
//! # Used by
//! `dag_combiner` (the first combine, which has the target's actions: the pin
//! expands in `SelectionDAGBuilder`) and `legalize_dag` (`ISD::IS_FPCLASS`).
//!
//! # Example
//! `is.fpclass(x, fcSubnormal)` on an `f32`:
//!
//! ```text
//! %bits = bitcast f32 x to i32          ; %abs = and %bits, 0x7fffffff
//! %m1   = sub %abs, 1                   ; %r = setult %m1, 0x7fffff
//! ```

use llrm_mir::apfloat::{ApFloat, IEEE_DOUBLE, IEEE_QUAD, IEEE_SINGLE, Sem, X87_DOUBLE_EXTENDED};
use llrm_mir::fpclass::{
    FC_ALL_FLAGS, FC_FINITE, FC_INF, FC_NAN, FC_NEG_FINITE, FC_NEG_INF, FC_NEG_NORMAL, FC_NEG_SUBNORMAL, FC_NONE,
    FC_NORMAL, FC_POS_FINITE, FC_POS_INF, FC_POS_NORMAL, FC_POS_SUBNORMAL, FC_POS_ZERO, FC_QNAN, FC_SNAN, FC_SUBNORMAL,
    FC_ZERO, FpClassTest, invert_fp_class_test_if_simpler,
};

use super::TargetLowering;
use super::actions::Action;
use super::dag::{SDValue, SelectionDAG};
use super::fold::mask;
use super::isd::{CondCode, ISD};
use super::mvt::{EVT, MVT};

/// The semantics of a scalar float type.
fn semantics(vt: EVT) -> Sem {
    match vt {
        EVT::Simple(MVT::F32) => IEEE_SINGLE,
        EVT::Simple(MVT::F64) => IEEE_DOUBLE,
        EVT::Simple(MVT::F80) => X87_DOUBLE_EXTENDED,
        EVT::Simple(MVT::F128) => IEEE_QUAD,
        other => unreachable!("a class test of {other:?}"),
    }
}

/// `isFCmpEqualZero`: whether the test is the ordered (`Some(true)`) or the
/// unordered (`Some(false)`) compare with zero.
fn is_fcmp_equal_zero(test: u32) -> Option<bool> {
    let ordered_mask = test & !FC_NAN.0;
    let nan_test = test & FC_NAN.0;
    let ordered = nan_test == 0;
    let unordered = nan_test == FC_NAN.0;
    // Skip cases that are testing for only a qnan or snan.
    if !ordered && !unordered {
        return None;
    }
    // The input denormal mode is IEEE: a subnormal is not a zero.
    (ordered_mask == FC_ZERO.0).then_some(ordered)
}

/// The ISD `isCondCodeLegalOrCustom` of a scalar float type.
fn cond_code_ok(
    tli: &TargetLowering,
    code: CondCode,
    vt: EVT,
) -> bool {
    let EVT::Simple(mvt) = vt else { return false };
    matches!(
        tli.cond_code_action(code, mvt),
        Action::Legal | Action::Custom
    )
}

/// `expandIS_FPCLASS(ResultVT, Op, Test, Flags, DL, DAG)` with `NoFPExcept`
/// set (no `strictfp`).
pub fn expand_is_fp_class(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
    result_vt: EVT,
    op: SDValue,
    orig_test: FpClassTest,
) -> SDValue {
    let operand_vt = dag.value_type(op);
    let mut test = orig_test.0;

    // Degenerated cases.
    if test == FC_NONE.0 {
        return dag.get_bool_constant(false, result_vt);
    }
    if test == FC_ALL_FLAGS.0 {
        return dag.get_bool_constant(true, result_vt);
    }

    let sem = semantics(operand_vt);
    let is_f80 = operand_vt == EVT::Simple(MVT::F80);
    let fp_constant = |dag: &mut SelectionDAG, value: ApFloat| dag.get_constant_fp(value.to_bits(), operand_vt);

    // Some checks can be implemented using float comparisons, if floating point
    // exceptions are ignored.
    if tli.is_operation_legal_or_custom(ISD::SetCC, operand_vt) {
        let mut fp_test_mask = test;
        let mut is_inverted_fp = false;
        let inverted_fp_check = invert_fp_class_test_if_simpler(FpClassTest(fp_test_mask), true).0;
        if inverted_fp_check != 0 {
            fp_test_mask = inverted_fp_check;
            is_inverted_fp = true;
        }

        let ordered_cmp = if is_inverted_fp { CondCode::SetUNe } else { CondCode::SetOEq };
        let unordered_cmp = if is_inverted_fp { CondCode::SetONe } else { CondCode::SetUEq };

        // See if we can fold an | fcNan into an unordered compare.
        let mut ordered_fp_test_mask = fp_test_mask & !FC_NAN.0;
        // Can't fold the ordered check if we're only testing for snan or qnan
        // individually.
        if fp_test_mask & FC_NAN.0 != FC_NAN.0 {
            ordered_fp_test_mask = fp_test_mask;
        }
        let is_ordered = fp_test_mask == ordered_fp_test_mask;
        let compare = if is_ordered { ordered_cmp } else { unordered_cmp };

        if let Some(is_cmp0) = is_fcmp_equal_zero(fp_test_mask)
            && cond_code_ok(tli, if is_cmp0 { ordered_cmp } else { unordered_cmp }, operand_vt)
        {
            let zero = dag.get_constant_fp(0, operand_vt);
            return dag.get_setcc(result_vt, op, zero, if is_cmp0 { ordered_cmp } else { unordered_cmp });
        }

        if fp_test_mask == FC_NAN.0
            && cond_code_ok(tli, if is_inverted_fp { CondCode::SetO } else { CondCode::SetUo }, operand_vt)
        {
            return dag.get_setcc(result_vt, op, op, if is_inverted_fp { CondCode::SetO } else { CondCode::SetUo });
        }

        let is_ordered_inf = fp_test_mask == FC_INF.0;
        if (fp_test_mask == FC_INF.0 || fp_test_mask == (FC_INF.0 | FC_NAN.0))
            && cond_code_ok(tli, if is_ordered_inf { ordered_cmp } else { unordered_cmp }, operand_vt)
            && tli.is_operation_legal_or_custom(ISD::FAbs, operand_vt)
            && tli.operation_action(ISD::ConstantFP, operand_vt) == Action::Legal
        {
            // isinf(x) --> fabs(x) == inf
            let abs = dag.get_node(ISD::FAbs, operand_vt, &[op]);
            let inf = fp_constant(dag, ApFloat::get_inf(sem, false));
            return dag.get_setcc(result_vt, abs, inf, if is_ordered_inf { ordered_cmp } else { unordered_cmp });
        }

        if (ordered_fp_test_mask == FC_POS_INF.0 || ordered_fp_test_mask == FC_NEG_INF.0)
            && cond_code_ok(tli, compare, operand_vt)
        {
            // isposinf(x) --> x == inf
            // isneginf(x) --> x == -inf
            // isposinf(x) || nan --> x u== inf
            // isneginf(x) || nan --> x u== -inf
            let inf = fp_constant(dag, ApFloat::get_inf(sem, ordered_fp_test_mask == FC_NEG_INF.0));
            return dag.get_setcc(result_vt, op, inf, compare);
        }

        if ordered_fp_test_mask == (FC_SUBNORMAL.0 | FC_ZERO.0) && !is_ordered {
            let ordered_op = if is_inverted_fp { CondCode::SetUGe } else { CondCode::SetOLt };
            let unordered_op = if is_inverted_fp { CondCode::SetOGe } else { CondCode::SetULt };
            if cond_code_ok(tli, if is_ordered { ordered_op } else { unordered_op }, operand_vt) {
                // (issubnormal(x) || iszero(x)) --> fabs(x) < smallest_normal
                let abs = dag.get_node(ISD::FAbs, operand_vt, &[op]);
                let smallest_normal = fp_constant(dag, ApFloat::get_smallest_normalized(sem, false));
                return dag.get_setcc(
                    result_vt,
                    abs,
                    smallest_normal,
                    if is_ordered { ordered_op } else { unordered_op },
                );
            }
        }

        // `FPTestMask == fcNormal` needs `isFAbsFree`, which is false here.
    }

    // Some checks may be represented as inversion of simpler check, for example
    // "inf|normal|subnormal|zero" => !"nan".
    let mut is_inverted = false;
    let inverted_check = invert_fp_class_test_if_simpler(FpClassTest(test), false).0;
    if inverted_check != 0 {
        test = inverted_check;
        is_inverted = true;
    }

    // In the general case use integer operations.
    let bit_size = operand_vt.size_in_bits();
    let int_vt = EVT::integer(bit_size);
    let op_as_int = dag.get_node(ISD::BitCast, int_vt, &[op]);

    // Various masks.
    let sign_bit = 1u128 << (bit_size - 1);
    let value_mask = mask(bit_size) >> 1; // All bits but sign.
    let inf = ApFloat::get_inf(sem, false).to_bits(); // Exp and int bit.
    const EXPLICIT_INT_BIT_IN_F80: u32 = 63;
    let mut exp_mask = inf;
    if is_f80 {
        exp_mask &= !(1u128 << EXPLICIT_INT_BIT_IN_F80);
    }
    let all_one_mantissa = ApFloat::get_largest(sem, false).to_bits() & !inf;
    let qnan_bit_mask = 1u128 << (128 - all_one_mantissa.leading_zeros() - 1);
    let inversion_mask = mask(result_vt.size_in_bits());

    let value_mask_v = dag.get_constant(value_mask, int_vt);
    let sign_bit_v = dag.get_constant(sign_bit, int_vt);
    let exp_mask_v = dag.get_constant(exp_mask, int_vt);
    let zero_v = dag.get_constant(0, int_vt);
    let inf_v = dag.get_constant(inf, int_vt);
    let result_inversion_mask = dag.get_constant(inversion_mask, result_vt);

    let mut res: Option<SDValue> = None;
    let append_result = |dag: &mut SelectionDAG, partial: SDValue, res: &mut Option<SDValue>| {
        *res = Some(match *res {
            Some(previous) => dag.get_node(ISD::Or, result_vt, &[previous, partial]),
            None => partial,
        });
    };

    // The explicit integer bit in the f80 mantissa is set.
    let mut int_bit_is_set: Option<SDValue> = None;
    let mut get_int_bit_is_set = |dag: &mut SelectionDAG| -> SDValue {
        *int_bit_is_set.get_or_insert_with(|| {
            let int_bit_mask = dag.get_constant(1u128 << EXPLICIT_INT_BIT_IN_F80, int_vt);
            let int_bit = dag.get_node(ISD::And, int_vt, &[op_as_int, int_bit_mask]);
            dag.get_setcc(result_vt, int_bit, zero_v, CondCode::SetNe)
        })
    };

    // Split the value into sign bit and absolute value.
    let abs_v = dag.get_node(ISD::And, int_vt, &[op_as_int, value_mask_v]);
    let sign_zero = dag.get_constant(0, int_vt);
    let sign_v = dag.get_setcc(result_vt, op_as_int, sign_zero, CondCode::SetLt);

    // Tests that involve more than one class should be processed first.
    let mut partial: Option<SDValue> = None;
    if is_f80 {
        // Detect finite numbers of f80 by checking individual classes because
        // they have different settings of the explicit integer bit.
    } else if test & FC_FINITE.0 == FC_FINITE.0 {
        // finite(V) ==> (a << 1) < (inf << 1)
        let one = dag.get_constant(1, int_vt);
        let twice_op = dag.get_node(ISD::Shl, int_vt, &[op_as_int, one]);
        let twice_inf = dag.get_node(ISD::Shl, int_vt, &[exp_mask_v, one]);
        partial = Some(dag.get_setcc(result_vt, twice_op, twice_inf, CondCode::SetULt));
        test &= !FC_FINITE.0;
    } else if test & FC_FINITE.0 == FC_POS_FINITE.0 {
        // finite(V) && V > 0 ==> V < exp_mask
        partial = Some(dag.get_setcc(result_vt, op_as_int, exp_mask_v, CondCode::SetULt));
        test &= !FC_POS_FINITE.0;
    } else if test & FC_FINITE.0 == FC_NEG_FINITE.0 {
        // finite(V) && V < 0 ==> abs(V) < exp_mask && signbit == 1
        let below = dag.get_setcc(result_vt, abs_v, exp_mask_v, CondCode::SetLt);
        partial = Some(dag.get_node(ISD::And, result_vt, &[below, sign_v]));
        test &= !FC_NEG_FINITE.0;
    }
    if let Some(partial) = partial {
        append_result(dag, partial, &mut res);
    }

    let partial_check = test & (FC_ZERO.0 | FC_SUBNORMAL.0);
    if partial_check != 0 {
        // fcZero | fcSubnormal => test all exponent bits are 0
        if partial_check == (FC_ZERO.0 | FC_SUBNORMAL.0) {
            let exp_bits = dag.get_node(ISD::And, int_vt, &[op_as_int, exp_mask_v]);
            let exp_is_zero = dag.get_setcc(result_vt, exp_bits, zero_v, CondCode::SetEq);
            append_result(dag, exp_is_zero, &mut res);
            test &= !partial_check & FC_ALL_FLAGS.0;
        }
    }

    // Check for individual classes.
    let partial_check = test & FC_ZERO.0;
    if partial_check != 0 {
        let partial = if partial_check == FC_POS_ZERO.0 {
            dag.get_setcc(result_vt, op_as_int, zero_v, CondCode::SetEq)
        } else if partial_check == FC_ZERO.0 {
            dag.get_setcc(result_vt, abs_v, zero_v, CondCode::SetEq)
        } else {
            // fcNegZero
            dag.get_setcc(result_vt, op_as_int, sign_bit_v, CondCode::SetEq)
        };
        append_result(dag, partial, &mut res);
    }

    let partial_check = test & FC_SUBNORMAL.0;
    if partial_check != 0 {
        // issubnormal(V) ==> unsigned(abs(V) - 1) < (all mantissa bits set)
        // issubnormal(V) && V>0 ==> unsigned(V - 1) < (all mantissa bits set)
        let value = if partial_check == FC_POS_SUBNORMAL.0 { op_as_int } else { abs_v };
        let mantissa_v = dag.get_constant(all_one_mantissa, int_vt);
        let one = dag.get_constant(1, int_vt);
        let v_minus_one = dag.get_node(ISD::Sub, int_vt, &[value, one]);
        let mut partial = dag.get_setcc(result_vt, v_minus_one, mantissa_v, CondCode::SetULt);
        if partial_check == FC_NEG_SUBNORMAL.0 {
            partial = dag.get_node(ISD::And, result_vt, &[partial, sign_v]);
        }
        append_result(dag, partial, &mut res);
    }

    let partial_check = test & FC_INF.0;
    if partial_check != 0 {
        let partial = if partial_check == FC_POS_INF.0 {
            dag.get_setcc(result_vt, op_as_int, inf_v, CondCode::SetEq)
        } else if partial_check == FC_INF.0 {
            dag.get_setcc(result_vt, abs_v, inf_v, CondCode::SetEq)
        } else {
            // fcNegInf
            let neg_inf = ApFloat::get_inf(sem, true).to_bits();
            let neg_inf_v = dag.get_constant(neg_inf, int_vt);
            dag.get_setcc(result_vt, op_as_int, neg_inf_v, CondCode::SetEq)
        };
        append_result(dag, partial, &mut res);
    }

    let partial_check = test & FC_NAN.0;
    if partial_check != 0 {
        let inf_with_qnan_bit = inf | qnan_bit_mask;
        let inf_with_qnan_bit_v = dag.get_constant(inf_with_qnan_bit, int_vt);
        let partial = if partial_check == FC_NAN.0 {
            // isnan(V) ==> abs(V) > int(inf)
            let mut partial = dag.get_setcc(result_vt, abs_v, inf_v, CondCode::SetGt);
            if is_f80 {
                // Recognize unsupported values as NaNs for compatibility with
                // glibc. In them (exp(V)==0) == int_bit.
                let exp_bits = dag.get_node(ISD::And, int_vt, &[abs_v, exp_mask_v]);
                let exp_is_zero = dag.get_setcc(result_vt, exp_bits, zero_v, CondCode::SetEq);
                let int_bit = get_int_bit_is_set(dag);
                let is_pseudo = dag.get_setcc(result_vt, int_bit, exp_is_zero, CondCode::SetEq);
                partial = dag.get_node(ISD::Or, result_vt, &[partial, is_pseudo]);
            }
            partial
        } else if partial_check == FC_QNAN.0 {
            // isquiet(V) ==> abs(V) >= (unsigned(Inf) | quiet_bit)
            dag.get_setcc(result_vt, abs_v, inf_with_qnan_bit_v, CondCode::SetGe)
        } else {
            debug_assert_eq!(partial_check, FC_SNAN.0);
            // issignaling(V) ==> abs(V) > unsigned(Inf) &&
            //                    abs(V) < (unsigned(Inf) | quiet_bit)
            let is_nan = dag.get_setcc(result_vt, abs_v, inf_v, CondCode::SetGt);
            let is_not_qnan = dag.get_setcc(result_vt, abs_v, inf_with_qnan_bit_v, CondCode::SetLt);
            dag.get_node(ISD::And, result_vt, &[is_nan, is_not_qnan])
        };
        append_result(dag, partial, &mut res);
    }

    let partial_check = test & FC_NORMAL.0;
    if partial_check != 0 {
        // isnormal(V) ==> (0 < exp < max_exp) ==> (unsigned(exp-1) <
        // (max_exp-1))
        let exp_lsb = exp_mask & !((exp_mask << 1) & mask(bit_size));
        let exp_lsb_v = dag.get_constant(exp_lsb, int_vt);
        let exp_minus_1 = dag.get_node(ISD::Sub, int_vt, &[abs_v, exp_lsb_v]);
        let exp_limit = exp_mask - exp_lsb;
        let exp_limit_v = dag.get_constant(exp_limit, int_vt);
        let mut partial = dag.get_setcc(result_vt, exp_minus_1, exp_limit_v, CondCode::SetULt);
        if partial_check == FC_NEG_NORMAL.0 {
            partial = dag.get_node(ISD::And, result_vt, &[partial, sign_v]);
        } else if partial_check == FC_POS_NORMAL.0 {
            let pos_sign_v = dag.get_node(ISD::Xor, result_vt, &[sign_v, result_inversion_mask]);
            partial = dag.get_node(ISD::And, result_vt, &[partial, pos_sign_v]);
        }
        if is_f80 {
            let int_bit = get_int_bit_is_set(dag);
            partial = dag.get_node(ISD::And, result_vt, &[partial, int_bit]);
        }
        append_result(dag, partial, &mut res);
    }

    let Some(result) = res else {
        return dag.get_constant(u128::from(is_inverted), result_vt);
    };
    if is_inverted {
        return dag.get_node(ISD::Xor, result_vt, &[result, result_inversion_mask]);
    }
    result
}
