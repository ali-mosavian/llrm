//! The 6502's action table: what the DAG legalizers do with each operation.
//!
//! `i8` and `i16` are register types (`Imag8`, `Imag16`: the pair is what a pointer lives in). Byte
//! and word add, subtract and logic are instructions; the carry forms (`UADDO`, `UADDO_CARRY`, `USUBO`,
//! `USUBO_CARRY`, the carry an `i1` in the `C` register) are too, so the type legalizer expands an `i32` into two words joined by the carry, an
//! `i64` into four. There is no multiply, divide or shift by a variable: those are library calls.
//!
//! DEVIATE: `setcc`, `select` and `brcond` are not lowered yet (they need the flag register and
//! the branch forms). Exit: the loop slice.

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::MVT;
use llrm_codegen::backend::selectiondag::{Action, LoadExt, TargetLowering, TargetLoweringInfo};

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
    tl
}
