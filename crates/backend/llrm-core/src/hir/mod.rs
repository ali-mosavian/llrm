//! The common HIR (`llrm-hir`), its lowering to MIR, and its interpreter.

pub mod execute;
pub mod symbols;

pub use llrm_hir::{codec, debug, escape, facts, model, verify};

/// `llrm_hir::mir`, with `emit` for real mode's layout: BASIC's lowering and the tests of
/// code16-only code. A target-aware caller passes its own (`emit_for`); this is to go.
pub mod mir {
    pub use llrm_hir::mir::*;

    pub fn emit(program: &llrm_hir::model::Program) -> Vec<Emitted> {
        llrm_hir::mir::emit(program, &llrm_x86_code16::layout())
    }
}

pub use codec::{decode, encode};
pub use model::{
    AddressKind, ArrayElement, ArrayOrder, Block, CallAbi, CallDistance, Constant, DataLinkage, DataObject,
    DataRelocation, DescriptorField, DescriptorPlace, Dialect, FloatEvaluation, FloatMode, Function,
    FunctionLinkage, IndirectPlace, Instruction, Module, Op, Place, PlaceRef, ProcedureAbi, Program,
    ProjectedPlace, RuntimeProfile, StackCleanup, FloatReturn, Storage, TargetProfile, Terminator, TerminatorKind, Type,
    TypeKind, Value, ValueRef,
};
pub use verify::{InvalidHIR, verify};
