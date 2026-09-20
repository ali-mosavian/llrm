//! Typed, resolved, non-SSA source representation.
//!
//! Frontends construct this representation directly. Compatibility encoders
//! belong at the edge of the pipeline; HIR itself contains no wire-format or
//! frontend parser state.

mod model;
mod text;
mod verify;

pub use model::*;
pub use text::{TextError, parse as parse_text, write as write_text};
pub use verify::verify;
