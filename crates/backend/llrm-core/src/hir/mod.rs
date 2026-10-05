//! The common HIR (`llrm-hir`), its lowering to MIR, and its interpreter.

pub mod execute;
pub mod symbols;

pub use llrm_hir::{codec, debug, escape, facts, mir, model, verify};

pub use codec::{decode, encode};
pub use model::{
    AddressKind, ArrayElement, ArrayOrder, Block, CallAbi, CallDistance, Constant, DataLinkage, DataObject,
    DataRelocation, DescriptorField, DescriptorPlace, Dialect, FloatEvaluation, FloatMode, Function,
    FunctionLinkage, IndirectPlace, Instruction, Module, Op, Place, PlaceRef, ProcedureAbi, Program,
    ProjectedPlace, RuntimeProfile, StackCleanup, FloatReturn, Storage, TargetProfile, Terminator, TerminatorKind, Type,
    TypeKind, Value, ValueRef,
};
pub use verify::{InvalidHIR, verify};
