//! Typed, resolved, non-SSA source representation.
//!
//! Frontends construct this representation directly. Compatibility encoders
//! belong at the edge of the pipeline; HIR itself contains no wire-format or
//! frontend parser state.

mod model;
mod verify;

pub use model::*;
pub use verify::verify;
