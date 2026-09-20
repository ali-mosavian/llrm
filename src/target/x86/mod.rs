//! x86 legality, registers, ABI, selection, timing, and encoding hooks.

mod allocation;
mod basic_abi;
mod call_clobbers;
mod encoding;
mod frame;
mod frame_indices;
mod instructions;
mod mc;
mod registers;
mod selection;
mod verify;

pub use allocation::{X86AllocationError, allocate_registers};
pub use basic_abi::{BasicAbiError, ExpandedBasicFunction, expand_basic_runtime};
pub use call_clobbers::{CallClobberError, materialize_far_call_clobbers};
pub use encoding::{EncodeError, encode, encoded_size};
pub use frame::{BasicFramePlan, BasicFramePlanError, BasicRuntime, plan_basic_frame};
pub use frame_indices::{FrameIndexMaterializationError, materialize_frame_indices};
pub use instructions::{ComparisonKind, ConditionCode, OperandSize, X86Opcode};
pub use mc::{McLowerError, UnresolvedOperand, lower_instruction};
pub use registers::{X86Register, X86RegisterClass};
pub use selection::{FunctionProperty, SelectionError, select_module};
pub use verify::verify_machine;
