//! Portable typed SSA intermediate representation.

mod interpreter;
mod model;
mod text;
mod type_verify;
mod verify;

pub use interpreter::{
    DEFAULT_STEP_LIMIT, InterpretError, Interpreter, RuntimeInteger, interpret,
    interpret_with_step_limit,
};
pub use model::*;
pub use text::{TextError, parse as parse_text, write as write_text};
pub use verify::verify;
