//! `combineAddOrSubToADCOrSBB`: an add or subtract of a value and a compare
//! result (`cmp; setcc; movzx; add`) is `cmp; adc`/`sbb`, and `0 - setcc` of a
//! carry condition is `sbb r, r`. QuickBASIC's `-1`/`0` booleans and
//! `x == 0 ? -1 : 0` were `test; sete; movzx; neg`; the pin has `cmp x, 1;
//! sbb r, r`.
//!
//! DEVIATE SD219: the `ADOX` form (no x86 target of ours has ADX), the
//! or/xor entry (`combineOrXorWithSETCC`) and the `ADD(ADC/SBB)`/`SUB(…, SBB)`
//! folds of `combineAdd`/`combineSub` are not translated. Exit: with the rest
//! of `combineSBB`/`combineADC`. Owner w10. Review 2026-12-10.
//!
//! # See also
//! LLVM's `combineAddOrSubToADCOrSBB`
//! ([L53776](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L53776)).

use super::dag::{SDValue, SelectionDAG};
use super::fold;
use super::isd::{CondCode, ISD};
use super::mvt::EVT;
use super::x86_lowering::{cond, cond_code, lower_and_to_bt, x86isd};

fn all_ones(
    dag: &mut SelectionDAG,
    vt: EVT,
) -> SDValue {
    dag.get_constant(fold::mask(vt.size_in_bits()), vt)
}

/// `X86ISD::SUB a, b` and its flags result.
fn sub_flags(
    dag: &mut SelectionDAG,
    a: SDValue,
    b: SDValue,
) -> SDValue {
    let vt = dag.value_type(a);
    let sub = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[a, b]);
    SDValue { node: sub.node, res: 1 }
}

fn carry_mask(
    dag: &mut SelectionDAG,
    vt: EVT,
    flags: SDValue,
) -> SDValue {
    let code = cond_code(dag, cond::B);
    dag.get_node(x86isd::SETCC_CARRY, vt, &[code, flags])
}

fn adc_or_sbb(
    dag: &mut SelectionDAG,
    opcode: ISD,
    vt: EVT,
    x: SDValue,
    y: SDValue,
    flags: SDValue,
) -> SDValue {
    dag.get_node_multi(opcode, &[vt, EVT::I32], &[x, y, flags])
}

/// `combineAddOrSubToADCOrSBB(IsSub, DL, VT, X, Y, DAG, ZeroSecondOpOnly)`.
pub(super) fn add_or_sub_to_adc_or_sbb(
    dag: &mut SelectionDAG,
    is_sub: bool,
    vt: EVT,
    x: SDValue,
    y: SDValue,
    zero_second_only: bool,
) -> Option<SDValue> {
    if !vt.is_integer() || !matches!(vt.size_in_bits(), 8 | 16 | 32) {
        return None;
    }
    let (adc, sbb) = (x86isd::ADC, x86isd::SBB);
    let carry_op = if is_sub { sbb } else { adc };
    // Look through a one-use zext.
    let mut y = y;
    if dag.opcode(y) == ISD::ZeroExtend && dag.has_one_use(y.node) {
        y = dag.operand(y, 0);
    }
    let (code, flags) = if dag.opcode(y) == x86isd::SETCC && dag.has_one_use(y.node) {
        (dag.constant(dag.operand(y, 0))?, dag.operand(y, 1))
    } else if dag.opcode(y) == ISD::And && dag.constant(dag.operand(y, 1)) == Some(1) && dag.has_one_use(y.node) {
        let (flags, code) = lower_and_to_bt(dag, y, CondCode::SetNe)?;
        (code, flags)
    } else {
        return None;
    };
    let constant_x = dag.constant(x);
    let ones = fold::mask(vt.size_in_bits());
    if let (Some(c), false) = (constant_x, zero_second_only) {
        // -1 + SETAE --> -1 + !CF --> sbb r, r;  0 - SETB --> sbb r, r
        if (!is_sub && code == cond::AE && c == ones) || (is_sub && code == cond::B && c == 0) {
            return Some(carry_mask(dag, vt, flags));
        }
        if (!is_sub && code == cond::BE && c == ones) || (is_sub && code == cond::A && c == 0) {
            // Swap the operands of a SUB, and we have the same pattern as
            // above.
            if dag.opcode(flags) == x86isd::SUB
                && dag.has_one_use(flags.node)
                && flags.res == 1
                && dag.constant(dag.operand(flags, 1)).is_none()
            {
                let swapped = sub_flags(dag, dag.operand(flags, 1), dag.operand(flags, 0));
                return Some(carry_mask(dag, vt, swapped));
            }
        }
    }
    if code == cond::B {
        // X + SETB Z --> adc X, 0
        let zero = dag.get_constant(0, vt);
        return Some(adc_or_sbb(dag, carry_op, vt, x, zero, flags));
    }
    if zero_second_only {
        return None;
    }
    if code == cond::A {
        let is_sub_flags = dag.opcode(flags) == x86isd::SUB && flags.res == 1;
        // CMP X, Y: the carry flag is already X < Y.
        if dag.opcode(flags) == x86isd::CMP && dag.operand(flags, 0) == x {
            let zero = dag.get_constant(0, vt);
            return Some(adc_or_sbb(dag, carry_op, vt, x, zero, flags));
        }
        if is_sub_flags && dag.constant(dag.operand(flags, 1)).is_none() {
            if dag.has_one_use(flags.node) {
                let swapped = sub_flags(dag, dag.operand(flags, 1), dag.operand(flags, 0));
                let zero = dag.get_constant(0, vt);
                return Some(adc_or_sbb(dag, carry_op, vt, x, zero, swapped));
            }
            if is_sub && x == (SDValue { node: flags.node, res: 0 }) {
                let swapped = sub_flags(dag, dag.operand(flags, 1), dag.operand(flags, 0));
                return Some(adc_or_sbb(dag, sbb, vt, dag.operand(flags, 0), dag.operand(flags, 1), swapped));
            }
        }
    }
    if code == cond::AE {
        // X + SETAE --> sbb X, -1;  X - SETAE --> adc X, -1
        let minus_one = all_ones(dag, vt);
        let opcode = if is_sub { adc } else { sbb };
        return Some(adc_or_sbb(dag, opcode, vt, x, minus_one, flags));
    }
    if code == cond::BE
        && dag.opcode(flags) == x86isd::SUB
        && flags.res == 1
        && dag.has_one_use(flags.node)
        && dag.constant(dag.operand(flags, 1)).is_none()
    {
        let swapped = sub_flags(dag, dag.operand(flags, 1), dag.operand(flags, 0));
        let minus_one = all_ones(dag, vt);
        let opcode = if is_sub { adc } else { sbb };
        return Some(adc_or_sbb(dag, opcode, vt, x, minus_one, swapped));
    }
    if code != cond::E && code != cond::NE {
        return None;
    }
    if dag.opcode(flags) != x86isd::CMP
        || !dag.has_one_use(flags.node)
        || !dag.is_null_constant(dag.operand(flags, 1))
        || !dag.value_type(dag.operand(flags, 0)).is_integer()
    {
        return None;
    }
    let z = dag.operand(flags, 0);
    let z_vt = dag.value_type(z);
    if let Some(c) = constant_x {
        // `neg` sets the carry flag when Z != 0: 0 - (Z != 0), -1 + (Z == 0).
        if (is_sub && code == cond::NE && c == 0) || (!is_sub && code == cond::E && c == ones) {
            let zero = dag.get_constant(0, z_vt);
            let neg = sub_flags(dag, zero, z);
            return Some(carry_mask(dag, vt, neg));
        }
        // `cmp Z, 1` sets the carry flag when Z == 0: 0 - (Z == 0), -1 + (Z !=
        // 0).
        if (is_sub && code == cond::E && c == 0) || (!is_sub && code == cond::NE && c == ones) {
            let one = dag.get_constant(1, z_vt);
            let cmp = sub_flags(dag, z, one);
            return Some(carry_mask(dag, vt, cmp));
        }
    }
    let one = dag.get_constant(1, z_vt);
    let cmp = sub_flags(dag, z, one);
    if code == cond::NE {
        // X - (Z != 0) --> adc X, -1;  X + (Z != 0) --> sbb X, -1
        let minus_one = all_ones(dag, vt);
        let opcode = if is_sub { adc } else { sbb };
        return Some(adc_or_sbb(dag, opcode, vt, x, minus_one, cmp));
    }
    // X - (Z == 0) --> sbb X, 0;  X + (Z == 0) --> adc X, 0
    let zero = dag.get_constant(0, vt);
    Some(adc_or_sbb(dag, carry_op, vt, x, zero, cmp))
}

/// `combineAddOrSubToADCOrSBB(N, DL, DAG)`: either operand may be the
/// compare; a commuted subtract is negated.
pub(super) fn add_or_sub_node_to_adc_or_sbb(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let is_sub = dag.opcode(node) == ISD::Sub;
    let vt = dag.value_type(node);
    let (x, y) = (dag.operand(node, 0), dag.operand(node, 1));
    if let Some(result) = add_or_sub_to_adc_or_sbb(dag, is_sub, vt, x, y, false) {
        return Some(SDValue { node: result.node, res: 0 });
    }
    let result = add_or_sub_to_adc_or_sbb(dag, is_sub, vt, y, x, false)?;
    let result = SDValue { node: result.node, res: 0 };
    Some(if is_sub { dag.get_negative(result, vt) } else { result })
}
