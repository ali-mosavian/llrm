//! Side-effect-free analyses over portable IR and source-neutral MIR.

mod cfg;
pub(crate) mod constants;
mod def_use;
mod dominators;
pub(crate) mod induction;
mod loops;
pub(crate) mod occurrence;
mod postorder;
pub(crate) mod ssa;

pub use cfg::{CfgError, ControlFlowGraph};
pub use def_use::{DefUse, DefUseError, Definition, Use};
pub use dominators::Dominators;
pub use loops::{LoopInfo, NaturalLoop};
pub use postorder::PostOrder;
