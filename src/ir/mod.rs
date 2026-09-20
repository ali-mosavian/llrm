//! Portable typed SSA intermediate representation.

mod interpreter;
mod lower;
mod model;
mod text;
mod verify;

pub use interpreter::{
    DEFAULT_STEP_LIMIT, InterpretError, Interpreter, RuntimeInteger, interpret,
    interpret_with_step_limit,
};
pub use lower::{
    InvalidProperty as LowerInvalidProperty, LowerError, UnsupportedFeature, UnsupportedOperand,
    lower_module,
};
pub use model::*;
pub use text::{parse as parse_text, write as write_text, TextError};
pub use verify::verify;
