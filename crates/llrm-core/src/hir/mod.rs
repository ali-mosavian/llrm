//! The common HIR (`llrm-hir`), its lowering to MIR, and its interpreter.

pub mod callmemory;
pub mod dump;
pub mod execute;
pub mod lower;

pub use llrm_hir::{codec, escape, mir, model, verify};

pub use codec::{decode, encode};
pub use dump::mir_text;
pub use lower::{Lowered, lower};
pub use model::{
    AddressKind, ArrayElement, ArrayOrder, Block, CallAbi, CallDistance, Constant, DataLinkage, DataObject,
    DataRelocation, DescriptorField, DescriptorPlace, Dialect, FloatEvaluation, FloatMode, Function,
    FunctionLinkage, IndirectPlace, Instruction, Module, Op, Place, PlaceRef, ProcedureAbi, Program,
    ProjectedPlace, RuntimeProfile, StackCleanup, FloatReturn, Storage, TargetProfile, Terminator, TerminatorKind, Type,
    TypeKind, Value, ValueRef,
};
pub use verify::{InvalidHIR, verify};

#[cfg(test)]
mod test_hir;
