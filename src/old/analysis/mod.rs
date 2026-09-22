//! Side-effect-free analyses over portable IR and source-neutral MIR.

mod cfg;
mod def_use;
mod dominators;
mod loops;
mod postorder;

pub use cfg::{CfgError, ControlFlowGraph};
pub use def_use::{DefUse, DefUseError, Definition, Use};
pub use dominators::Dominators;
pub use loops::{LoopInfo, NaturalLoop};
pub use postorder::PostOrder;
