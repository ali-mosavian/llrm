//! x86 legality, registers, ABI, selection, timing, and encoding hooks.

mod allocation;
mod basic_abi;
mod basic_abi_expand;
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
mod selection;
mod verify;

pub use allocation::{X86AllocationError, allocate_registers};
pub use basic_abi::{BasicAbiError, ExpandedBasicFunction, expand_basic_runtime};
pub use basic_abi_expand::{BasicAbiExpansionError, expand_allocated_basic_abi};
pub use call_clobbers::{CallClobberError, materialize_far_call_clobbers};
pub use encoding::{EncodeError, EncodedInstruction, encode, encode_with_fixups, encoded_size};
pub use fixup::X86FixupKind;
pub use frame::{BasicFramePlan, BasicFramePlanError, BasicRuntime, plan_basic_frame};
pub use frame_indices::{FrameIndexMaterializationError, materialize_frame_indices};
pub use instructions::{ComparisonKind, ConditionCode, OperandSize, X86Opcode};
pub use jump_layout::{JumpLayoutVerificationStage, X86JumpLayoutError, relax_and_encode_jumps};
pub use mc::{McLowerError, UnresolvedOperand, lower_instruction};
pub use mc_encode::{McEncodeVerificationStage, X86McEncodeError, encode_mc_module};
pub use mc_module::{
    DefinedSymbolKind, McModuleIdKind, X86McModuleLowerError, lower_allocated_module,
};
pub use omf::{X86OmfError, lower_to_omf};
pub use registers::{X86Register, X86RegisterClass};
pub use selection::{FunctionProperty, SelectionError, select_module};
pub use verify::verify_machine;
