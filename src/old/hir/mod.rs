//! Typed, resolved, non-SSA source representation.
//!
//! Frontends construct this representation directly. Compatibility encoders
//! belong at the edge of the pipeline; HIR itself contains no wire-format or
//! frontend parser state.

mod calls;
mod globals;
mod lower;
mod model;
mod text;
mod verify;

pub use calls::{
    AbiOrderError, CallOperandError, CallPlanError, CallSignature, CallableParameterError,
};
pub use globals::GlobalPlanError;
pub use lower::{
    InvalidProperty as LowerInvalidProperty, LowerError, UnsupportedFeature, UnsupportedOperand,
    lower_module as lower_to_ir, lower_module_with_array_order as lower_to_ir_with_array_order,
};
pub use model::*;
pub use text::{TextError, parse as parse_text, write as write_text};
pub use verify::{verify, verify_module};
