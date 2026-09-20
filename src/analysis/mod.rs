//! Side-effect-free analyses over portable IR.

mod def_use;

pub use def_use::{DefUse, DefUseError, Definition, Use};
