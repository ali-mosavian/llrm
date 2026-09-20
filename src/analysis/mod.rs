//! Side-effect-free analyses over portable IR.

mod cfg;
mod def_use;

pub use cfg::{CfgError, ControlFlowGraph};
pub use def_use::{DefUse, DefUseError, Definition, Use};
