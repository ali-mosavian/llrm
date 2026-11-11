//! What the shared mid-end asks of the 6502's `TargetLowering`: the
//! [`Lowering`] hooks CodeGenPrepare reads, answered from the DAG's action
//! table (`dagisel::lowering`), one copy of what each operation does on each
//! type. i8 and i16 are the register types; there is no i32.
//!
//! DEVIATE 6502-CGP: only the steps in [`STEPS`] run; the pin runs them all
//! (`Mos6502Machine` had no `lowering()`, so every step behind it was dead).
//! Each is added in its own change, with the pin's answer for the hooks it asks
//! and an identity A/B on the corpus. Owner 6502. Exit: 2026-11-20, all on.

use std::sync::OnceLock;

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt as DagLoadExt, TargetLowering};
use llrm_mir::lowering::{CgpStep, Isd, LoadExt, Lowering};

use crate::dagisel::lowering::target_lowering;

/// The steps of CodeGenPrepare that run on the 6502 so far.
pub const STEPS: &[CgpStep] = &[];

/// The action table, built once.
fn table() -> &'static TargetLowering {
    static TABLE: OnceLock<TargetLowering> = OnceLock::new();
    TABLE.get_or_init(target_lowering)
}

/// The answers of the 6502's `TargetLowering`.
pub struct MosIrLowering;

impl Lowering for MosIrLowering {
    fn runs_cgp_step(
        &self,
        step: CgpStep,
    ) -> bool {
        STEPS.contains(&step)
    }

    fn is_type_legal(
        &self,
        bits: u32,
    ) -> bool {
        MVT::integer(bits).is_some_and(|vt| table().types.is_type_legal(vt.into()))
    }

    fn is_operation_legal_or_custom(
        &self,
        op: Isd,
        bits: u32,
    ) -> bool {
        let Some(vt) = MVT::integer(bits) else { return false };
        let node = match op {
            Isd::Add => ISD::Add,
            Isd::Sub => ISD::Sub,
            Isd::Mul => ISD::Mul,
            Isd::UDiv => ISD::UDiv,
            Isd::SDiv => ISD::SDiv,
            Isd::URem => ISD::URem,
            Isd::SRem => ISD::SRem,
            Isd::Shl => ISD::Shl,
            Isd::Srl => ISD::Srl,
            Isd::Sra => ISD::Sra,
            Isd::And => ISD::And,
            Isd::Or => ISD::Or,
            Isd::Xor => ISD::Xor,
            Isd::Select => ISD::Select,
            Isd::SetCc => ISD::SetCC,
            Isd::Truncate => ISD::Truncate,
            Isd::ZeroExtend => ISD::ZeroExtend,
            Isd::SignExtend => ISD::SignExtend,
        };
        table().is_operation_legal_or_custom(node, EVT::Simple(vt))
    }

    fn is_load_ext_legal(
        &self,
        ext: LoadExt,
        value_bits: u32,
        memory_bits: u32,
    ) -> bool {
        let (Some(value), Some(memory)) = (MVT::integer(value_bits), MVT::integer(memory_bits)) else { return false };
        let ext = match ext {
            LoadExt::Any => DagLoadExt::ExtLoad,
            LoadExt::Zero => DagLoadExt::ZExtLoad,
            LoadExt::Sign => DagLoadExt::SExtLoad,
        };
        table().load_ext_action(ext, value, memory) == Action::Legal
    }
}
