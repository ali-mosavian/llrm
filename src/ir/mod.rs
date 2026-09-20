//! Portable typed SSA intermediate representation.

mod model;
mod text;
mod verify;

pub use model::*;
pub use text::{parse as parse_text, write as write_text, TextError};
pub use verify::verify;
