//! Target-independent IR transformations.

mod algebraic;
mod branches;
pub(crate) mod cfg;
mod cse;
mod dead;
mod dead_store;
mod fold;
pub(crate) mod indvars;
pub(crate) mod loop_utils;
mod pass;
mod rewrite;
pub(crate) mod rotate;
pub(crate) mod strength;
mod unreachable;

pub use algebraic::{AlgebraicError, AlgebraicSimplify};
pub use branches::{BranchSimplifyError, SimplifyBranches};
pub use cse::CommonSubexpressionElimination;
pub use dead::DeadCodeElimination;
pub use dead_store::DeadStoreElimination;
pub use fold::{ConstantFold, FoldError};
pub use pass::{
    AnalysisInvalidation, FunctionIdentity, FunctionPass, FunctionPassManager, FunctionPassReport,
    PassError, PassExecution, PassFailure, PassInstrumentation, PassOutcome, PreservedAnalyses,
};
pub use unreachable::UnreachableBlockElimination;
