//! Target-independent IR transformations.

mod algebraic;
mod branches;
mod cse;
mod dead;
mod dead_store;
mod fold;
mod pass;
mod rewrite;
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
