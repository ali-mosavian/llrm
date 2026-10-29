//! The 6502's action table: what the DAG legalizers do with each operation.
//!
//! `i8` and `i16` are register types (`Imag8`, `Imag16`: the pair is what a pointer lives in). Byte
//! and word add, subtract and logic are instructions; the carry forms (`UADDO`, `UADDO_CARRY`, `USUBO`,
//! `USUBO_CARRY`, the carry an `i1` in the `C` register) are too, so the type legalizer expands an `i32` into two words joined by the carry, an
//! `i64` into four. There is no multiply, divide or shift by a variable: those are library calls.
//!
//! A comparison is `MOSISD::CMP a, b`: the `i8` flags of `a - b`. `SETCC`, `CMOV` and `BRCOND` read a
//! condition code (a constant, [`cond`]) and those flags. Only the flags the 6502 branches on are
//! codes: Z (`EQ`, `NE`), C (`CC`, `CS`) and N (`MI`, `PL`). An unsigned greater-than swaps the
//! operands; a signed compare flips both sign bits and compares unsigned, except against zero,
//! where N is the sign.
//!
//! A word compare is `CMP` too (C and N come out of the high byte's subtract-with-borrow); equality
//! is `CMPZ`, whose Z covers both bytes.

use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt, NodeData, SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::{TargetLowering, TargetLoweringInfo};

use super::generated::ids::isd as ids;

/// The 6502's own nodes, numbered by `own_ids.txt`.
pub mod mosisd {
    use super::{ISD, ids};

    pub const CMP: ISD = ISD::Target(ids::CMP);
    pub const CMPZ: ISD = ISD::Target(ids::CMPZ);
    pub const BRCOND: ISD = ISD::Target(ids::BRCOND);
    pub const SETCC: ISD = ISD::Target(ids::SETCC);
    pub const CMOV: ISD = ISD::Target(ids::CMOV);
}

/// Condition codes; a code and its opposite differ in bit 0.
pub mod cond {
    pub const EQ: u128 = 0;
    pub const NE: u128 = 1;
    pub const CC: u128 = 2;
    pub const CS: u128 = 3;
    pub const MI: u128 = 4;
    pub const PL: u128 = 5;
}

pub fn target_lowering() -> TargetLowering {
    use ISD::*;
    let mut tl = TargetLowering::empty(TargetLoweringInfo::new(&[MVT::I1, MVT::I8, MVT::I16], false));
    tl.init_actions();
    for vt in [MVT::I1, MVT::I8, MVT::I16, MVT::I32, MVT::I64] {
        for ext in [LoadExt::SExtLoad, LoadExt::ZExtLoad, LoadExt::ExtLoad] {
            tl.set_load_ext_action(ext, vt, MVT::I1, Action::Promote);
        }
    }
    for (vt, mem) in [(MVT::I64, MVT::I32), (MVT::I64, MVT::I16), (MVT::I64, MVT::I8), (MVT::I32, MVT::I16)]
        .into_iter()
        .chain([(MVT::I32, MVT::I8), (MVT::I16, MVT::I8)])
    {
        tl.set_trunc_store_action(vt, mem, Action::Expand);
    }
    let mut set = |ops: &[ISD], action: Action| {
        for &op in ops {
            for vt in [MVT::I8, MVT::I16] {
                tl.set_operation_action(op, vt, action);
            }
        }
    };
    set(&[Add, Sub, And, Or, Xor, UAddO, USubO, UAddOCarry, USubOCarry], Action::Legal);
    set(&[Mul, SDiv, UDiv, SRem, URem, Shl, Srl, Sra], Action::LibCall);
    set(&[UDivRem, SDivRem, MulHS, MulHU, UMulLoHi, SMulLoHi], Action::Expand);
    set(&[CtTz, CtLz, CtPop, BSwap], Action::Expand);
    tl.set_operation_action(BrCond, MVT::Other, Action::Custom);
    for op in [SetCC, Select] {
        for vt in [MVT::I8, MVT::I16] {
            tl.set_operation_action(op, vt, Action::Custom);
        }
    }
    tl
}

/// The custom lowerings.
pub struct MosLowering;

impl Hooks for MosLowering {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SetCC => lower_setcc(dag, node),
            ISD::Select => Some(lower_select(dag, node)),
            ISD::BrCond => Some(lower_brcond(dag, node)),
            _ => None,
        }
    }
}

fn cc_operand(
    dag: &mut SelectionDAG,
    code: u128,
) -> SDValue {
    dag.get_constant(code, EVT::I8)
}

fn cond_of(
    dag: &SelectionDAG,
    setcc: SDValue,
) -> CondCode {
    match dag.node(setcc).data {
        NodeData::CondCode(cc) => cc,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

/// `x ^ sign bit`: so a signed compare is an unsigned one. A constant folds.
fn flip_sign(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> SDValue {
    let vt = dag.value_type(value);
    let sign = 1u128 << (vt.size_in_bits() - 1);
    match dag.constant(value) {
        Some(bits) => dag.get_constant(bits ^ sign, vt),
        None => {
            let sign = dag.get_constant(sign, vt);
            dag.get_node(ISD::Xor, vt, &[value, sign])
        }
    }
}

/// The code a comparison of `op0` with `op1` branches on, and the operands to compare (possibly
/// swapped or sign-flipped).
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    op0: SDValue,
    op1: SDValue,
) -> (u128, SDValue, SDValue) {
    use CondCode::*;
    let against_zero = dag.is_null_constant(op1);
    match cc {
        SetEq => (cond::EQ, op0, op1),
        SetNe => (cond::NE, op0, op1),
        SetULt => (cond::CC, op0, op1),
        SetUGe => (cond::CS, op0, op1),
        SetUGt => (cond::CC, op1, op0),
        SetULe => (cond::CS, op1, op0),
        SetLt if against_zero => (cond::MI, op0, op1),
        SetGe if against_zero => (cond::PL, op0, op1),
        SetLt | SetGe => {
            let (a, b) = (flip_sign(dag, op0), flip_sign(dag, op1));
            (if cc == SetLt { cond::CC } else { cond::CS }, a, b)
        }
        SetGt | SetLe => {
            // a > b is b < a, a <= b is b >= a
            let (a, b) = (flip_sign(dag, op1), flip_sign(dag, op0));
            (if cc == SetGt { cond::CC } else { cond::CS }, a, b)
        }
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// `LowerSETCC`, integer part: `SETCC cc, (CMP a, b)`.
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (op0, op1) = (dag.operand(node, 0), dag.operand(node, 1));
    if !matches!(dag.value_type(op0), EVT::I8 | EVT::I16) {
        return None;
    }
    let cc = cond_of(dag, dag.operand(node, 2));
    let (code, a, b) = translate_cc(dag, cc, op0, op1);
    // Equality on a word needs Z over both bytes.
    let compare = if dag.value_type(a) == EVT::I16 && matches!(code, cond::EQ | cond::NE) { mosisd::CMPZ } else { mosisd::CMP };
    let flags = dag.get_node(compare, EVT::I8, &[a, b]);
    let code = cc_operand(dag, code);
    Some(dag.get_node(mosisd::SETCC, EVT::I8, &[code, flags]))
}

/// The code and flags a condition value branches on: a lowered compare, or the value tested
/// against zero (`NE`).
fn condition(
    dag: &mut SelectionDAG,
    value: SDValue,
) -> (SDValue, SDValue) {
    let value = if dag.opcode(value) == ISD::SetCC { lower_setcc(dag, value).unwrap_or(value) } else { value };
    if dag.opcode(value) == mosisd::SETCC {
        return (dag.operand(value, 0), dag.operand(value, 1));
    }
    let byte = if dag.value_type(value) == EVT::I1 { dag.get_node(ISD::ZeroExtend, EVT::I8, &[value]) } else { value };
    let zero = dag.get_constant(0, EVT::I8);
    let flags = dag.get_node(mosisd::CMP, EVT::I8, &[byte, zero]);
    (cc_operand(dag, cond::NE), flags)
}

/// `LowerSELECT`: `CMOV false, true, cc, flags`.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (test, on_true, on_false) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    let vt = dag.value_type(node);
    dag.get_node(mosisd::CMOV, vt, &[on_false, on_true, code, flags])
}

/// `LowerBRCOND`: `BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (chain, test, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (code, flags) = condition(dag, test);
    dag.get_node(mosisd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
}
