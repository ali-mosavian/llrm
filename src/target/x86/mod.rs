//! x86 legality, registers, ABI, selection, timing, and encoding hooks.

mod allocation;
mod basic_abi;
mod basic_abi_expand;
mod c_abi;
mod call_clobbers;
mod encoding;
mod fixup;
mod frame;
mod frame_indices;
mod instructions;
mod jump_layout;
mod mc;
mod mc_encode;
mod mc_module;
mod omf;
mod registers;
mod segmented_memory;
mod selection;
mod verify;
mod word_merge;

pub use allocation::{
    X86AllocationError, X86AllocationResult, allocate_registers, allocate_registers_with_spills,
    split_fixed_register_occurrences,
};
pub use basic_abi::{
    BasicAbiError, ExpandedBasicFunction, expand_basic_runtime, refresh_basic_runtime_frame,
};
pub use basic_abi_expand::{BasicAbiExpansionError, expand_allocated_basic_abi};
pub use c_abi::{
    CAbiExpansionError, CFramePlan, CFramePlanError, expand_allocated_c_abi, plan_c_frame,
};
pub use call_clobbers::{
    CallClobberError, materialize_c_call_clobbers, materialize_far_call_clobbers,
};
pub use encoding::{EncodeError, EncodedInstruction, encode, encode_with_fixups, encoded_size};
pub use fixup::X86FixupKind;
pub use frame::{BasicFramePlan, BasicFramePlanError, BasicRuntime, X86FrameLayout, plan_basic_frame};
pub use frame_indices::{
    FrameIndexMaterializationError, materialize_frame_indices,
    materialize_frame_indices_with_layout,
};
pub use instructions::{ComparisonKind, ConditionCode, OperandSize, X86Opcode};
pub use jump_layout::{JumpLayoutVerificationStage, X86JumpLayoutError, relax_and_encode_jumps};
pub use mc::{McLowerError, UnresolvedOperand, lower_instruction};
pub use mc_encode::{McEncodeVerificationStage, X86McEncodeError, encode_mc_module};
pub use mc_module::{
    DefinedSymbolKind, LoweredMcModule, McModuleIdKind, X86McModuleLowerError,
    lower_allocated_module, lower_allocated_module_with_lineage,
};
pub use omf::{X86OmfError, lower_to_omf, lower_to_omf_with_dgroup};
pub use registers::{X86Register, X86RegisterClass};
pub use segmented_memory::{SegmentedMemoryExpansionError, expand_allocated_segmented_memory};
pub use selection::{FunctionProperty, SelectionError, select_module};
pub use verify::verify_machine;
pub use word_merge::{expand_allocated_word_merges, WordMergeExpansionError};
