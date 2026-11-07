//! What the shared mid-end asks of `WebAssemblyTargetLowering`: the
//! [`Lowering`] hooks CodeGenPrepare reads, answered from the DAG's action
//! table (`dagisel::lowering`), one copy of what each operation does on each
//! type. WebAssembly overrides none of the hooks the trait lists besides the
//! register classes, so the rest are the base class's.
//!
//! DEVIATE WL1: only the steps in [`STEPS`] run; the pin runs them all. Each
//! is added with its own comparison against the pin. Exit: all of them on.

use std::sync::OnceLock;

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};
use llrm_codegen::backend::selectiondag::{Action, LoadExt as DagLoadExt, TargetLowering};
use llrm_mir::lowering::{CgpStep, Isd, LoadExt, Lowering};

use crate::dagisel::hooks::Cpu;
use crate::dagisel::lowering::target_lowering;
use crate::features::Features;

/// The steps of CodeGenPrepare that run on wasm so far.
const STEPS: &[CgpStep] = &[CgpStep::SwitchType];

/// The action table of wasm32, built once.
fn table() -> &'static TargetLowering {
    static TABLE: OnceLock<TargetLowering> = OnceLock::new();
    TABLE.get_or_init(|| target_lowering(Cpu::Wasm32, Features::default()))
}

/// The answers of `WebAssemblyTargetLowering`.
pub struct WasmIrLowering;

impl Lowering for WasmIrLowering {
    fn runs_cgp_step(
        &self,
        step: CgpStep,
    ) -> bool {
        STEPS.contains(&step)
    }

    // The register classes: i32 and i64 (WebAssemblyISelLowering.cpp,
    // addRegisterClass).
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

    // `TargetLoweringBase::isCtpopFast`: `isOperationLegal(CTPOP, VT)`.
    fn is_ctpop_fast(
        &self,
        bits: u32,
    ) -> bool {
        MVT::integer(bits).is_some_and(|vt| table().is_operation_legal(ISD::CtPop, EVT::Simple(vt)))
    }

    // The base class (TargetLoweringBase.cpp#L1813): the register type, i32
    // for a narrower integer.
    fn preferred_switch_condition_bits(
        &self,
        bits: u32,
    ) -> u32 {
        bits.max(32)
    }
}
