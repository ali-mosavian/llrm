//! `f128` on AArch64: a legal type in `FPR128` with no arithmetic, so the
//! operations that are not loads, stores and moves are calls of compiler-rt's
//! `tf` routines with the operands in `q` registers (AAPCS64).
//!
//! # How it works
//! `AArch64TargetLowering` marks `FADD`, `FSUB`, `FMUL` and `FDIV` of `f128`
//! `LibCall` (the legalizer calls [`super::pipeline::Aarch64Lowering`]'s
//! `make_lib_call`), and `FP_EXTEND`, `FP_TO_*INT`, `*INT_TO_FP`, `SETCC`,
//! `BR_CC` and `SELECT_CC` over it `Custom`. Each one here is the pin's
//! `LowerF128Call` / `LowerFP_TO_INT` / `LowerINT_TO_FP` /
//! `softenSetCCOperands`: the routine named by `RTLIB`, its result an integer
//! that a compare reads against zero.
//!
//! `SELECT` is the pin's `F128CSEL` block split (`emit.rs`), and `FNEG`, `FABS`
//! and `FCOPYSIGN` its sign-byte rewrite through a stack slot (`ExpandFNEG`, in
//! the shared legalizer).
//!
//! DEVIATE SD534: `f128` `FSQRT`, `FMA`, `FRINT`, `FSIN`, `FCOS` and the other
//! libm routines are not translated: the node is refused as a libcall the table
//! does not name. Exit: B3.
//!
//! # See also
//! `AArch64ISelLowering.cpp`: `LowerF128Call`, `LowerFP_EXTEND`,
//! `LowerFP_ROUND`, `LowerINT_TO_FP`, `LowerFP_TO_INT`;
//! `TargetLowering::softenSetCCOperands`.

use llrm_codegen::backend::selectiondag::isd::CondCode;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{SDValue, SelectionDAG};
use llrm_sdag::libcalls::{self, Cmp3, Libcall};

use super::lowering::cond_of;

fn is_quad(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.value_type(value) == EVT::Simple(MVT::F128)
}

fn scalar(vt: EVT) -> Option<MVT> {
    match vt {
        EVT::Simple(mvt) => Some(mvt),
        _ => None,
    }
}

/// `LowerINT_TO_FP` / `LowerFP_TO_INT` / `LowerFP_EXTEND` / `LowerFP_ROUND`
/// of an `f128`. `None`: not an `f128` node. `Some(None)`: one with no routine
/// here, left `Custom`.
pub fn convert(
    dag: &mut SelectionDAG,
    node: SDValue,
    call: &mut dyn FnMut(&mut SelectionDAG, &'static str, &[SDValue], EVT) -> Option<SDValue>,
) -> Option<Option<SDValue>> {
    let quad = EVT::Simple(MVT::F128);
    let op = dag.opcode(node);
    if !matches!(
        op,
        ISD::SIntToFp | ISD::UIntToFp | ISD::FpToSInt | ISD::FpToUInt | ISD::FpExtend | ISD::FpRound
    ) {
        return None;
    }
    let (to, from) = (dag.value_type(node), dag.value_type(dag.operand(node, 0)));
    let name = match op {
        ISD::SIntToFp | ISD::UIntToFp if to == quad => {
            let from = scalar(from)?;
            let kind = if op == ISD::SIntToFp {
                Libcall::SIntToFp(from, MVT::F128)
            } else {
                Libcall::UIntToFp(from, MVT::F128)
            };
            return Some(libcalls::name(kind).and_then(|name| call(dag, name, &[dag.operand(node, 0)], quad)));
        }
        ISD::FpToSInt | ISD::FpToUInt if from == quad => {
            // a result narrower than a word is the word's low bits
            let word = if to.size_in_bits() > 32 { MVT::I64 } else { MVT::I32 };
            let kind = if op == ISD::FpToSInt {
                Libcall::FpToSInt(MVT::F128, word)
            } else {
                Libcall::FpToUInt(MVT::F128, word)
            };
            let Some(name) = libcalls::name(kind) else { return Some(None) };
            let Some(result) = call(dag, name, &[dag.operand(node, 0)], EVT::Simple(word)) else { return Some(None) };
            return Some(Some(if EVT::Simple(word) == to {
                result
            } else {
                dag.get_node(ISD::Truncate, to, &[result])
            }));
        }
        ISD::FpExtend if to == quad => Libcall::FpExt(scalar(from)?, MVT::F128),
        ISD::FpRound if from == quad => Libcall::FpRound(MVT::F128, scalar(to)?),
        _ => return None,
    };
    Some(libcalls::name(name).and_then(|name| call(dag, name, &[dag.operand(node, 0)], to)))
}

/// What `softenSetCCOperands` makes of an `f128` compare: an integer compare
/// of a routine's result with zero, or (a compare no routine answers alone)
/// the finished boolean.
pub enum Softened {
    Compare { lhs: SDValue, rhs: SDValue, code: CondCode },
    Boolean(SDValue),
}

/// `softenSetCCOperands` for `f128`.
pub fn soften_compare(
    dag: &mut SelectionDAG,
    lhs: SDValue,
    rhs: SDValue,
    code: CondCode,
    result_vt: EVT,
    call: &mut dyn FnMut(&mut SelectionDAG, &'static str, &[SDValue], EVT) -> Option<SDValue>,
) -> Option<Softened> {
    use CondCode::*;
    let word = EVT::I32;
    let mut one = |dag: &mut SelectionDAG, cmp: Cmp3, code: CondCode| -> Option<(SDValue, SDValue, CondCode)> {
        let name = libcalls::name(Libcall::Fcmp3(cmp, MVT::F128))?;
        let result = call(dag, name, &[lhs, rhs], word)?;
        Some((result, dag.get_constant(0, word), code))
    };
    let (cmp, code) = match code {
        SetOEq | SetEq => (Cmp3::Oeq, SetEq),
        SetUNe | SetNe => (Cmp3::Une, SetNe),
        SetOGe | SetGe => (Cmp3::Oge, SetGe),
        SetOLt | SetLt => (Cmp3::Olt, SetLt),
        SetOLe | SetLe => (Cmp3::Ole, SetLe),
        SetOGt | SetGt => (Cmp3::Ogt, SetGt),
        SetO => (Cmp3::Uo, SetEq),
        SetUo => (Cmp3::Uo, SetNe),
        // unordered or less: the inverse of an ordered compare
        SetULt => (Cmp3::Oge, SetLt),
        SetULe => (Cmp3::Ogt, SetLe),
        SetUGt => (Cmp3::Ole, SetGt),
        SetUGe => (Cmp3::Olt, SetGe),
        SetONe => {
            let (a, az, ac) = one(dag, Cmp3::Olt, SetLt)?;
            let lt = dag.get_setcc(result_vt, a, az, ac);
            let (b, bz, bc) = one(dag, Cmp3::Ogt, SetGt)?;
            let gt = dag.get_setcc(result_vt, b, bz, bc);
            return Some(Softened::Boolean(dag.get_node(ISD::Or, result_vt, &[lt, gt])));
        }
        SetUEq => {
            let (a, az, ac) = one(dag, Cmp3::Uo, SetNe)?;
            let unordered = dag.get_setcc(result_vt, a, az, ac);
            let (b, bz, bc) = one(dag, Cmp3::Oeq, SetEq)?;
            let equal = dag.get_setcc(result_vt, b, bz, bc);
            return Some(Softened::Boolean(dag.get_node(ISD::Or, result_vt, &[unordered, equal])));
        }
        _ => return None,
    };
    let (lhs, rhs, code) = one(dag, cmp, code)?;
    Some(Softened::Compare { lhs, rhs, code })
}

/// `LowerSETCC`, `LowerBR_CC` and `LowerSELECT_CC` over an `f128` compare: the
/// compare is softened and the node made again over the integer result.
pub fn compare(
    dag: &mut SelectionDAG,
    node: SDValue,
    call: &mut dyn FnMut(&mut SelectionDAG, &'static str, &[SDValue], EVT) -> Option<SDValue>,
) -> Option<Option<SDValue>> {
    let op = dag.opcode(node);
    let (lhs, rhs, code) = match op {
        ISD::SetCC => (dag.operand(node, 0), dag.operand(node, 1), cond_of(dag, dag.operand(node, 2))),
        ISD::BrCC => (dag.operand(node, 2), dag.operand(node, 3), cond_of(dag, dag.operand(node, 1))),
        ISD::SelectCC => (dag.operand(node, 0), dag.operand(node, 1), cond_of(dag, dag.operand(node, 4))),
        _ => return None,
    };
    if !is_quad(dag, lhs) {
        return None;
    }
    let vt = dag.value_type(node);
    let result_vt = if op == ISD::SetCC { vt } else { EVT::I32 };
    let Some(softened) = soften_compare(dag, lhs, rhs, code, result_vt, call) else { return Some(None) };
    let (new_lhs, new_rhs, new_code) = match softened {
        Softened::Compare { lhs, rhs, code } => (lhs, rhs, code),
        Softened::Boolean(boolean) => {
            if op == ISD::SetCC {
                return Some(Some(boolean));
            }
            let zero = dag.get_constant(0, dag.value_type(boolean));
            (boolean, zero, CondCode::SetNe)
        }
    };
    let cc = dag.get_cond_code(new_code);
    Some(Some(match op {
        ISD::SetCC => dag.get_setcc(vt, new_lhs, new_rhs, new_code),
        ISD::BrCC => {
            let (chain, dest) = (dag.operand(node, 0), dag.operand(node, 4));
            dag.get_node(ISD::BrCC, EVT::OTHER, &[chain, cc, new_lhs, new_rhs, dest])
        }
        _ => {
            let (tval, fval) = (dag.operand(node, 2), dag.operand(node, 3));
            dag.get_node(ISD::SelectCC, vt, &[new_lhs, new_rhs, tval, fval, cc])
        }
    }))
}
