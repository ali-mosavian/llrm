//! Target-independent IR transformations.

mod branches;
mod dead;
mod fold;
mod pass;
mod rewrite;

pub use branches::{BranchSimplifyError, SimplifyBranches};
pub use dead::DeadCodeElimination;
pub use fold::{ConstantFold, FoldError};
pub use pass::{
    AnalysisInvalidation, FunctionIdentity, FunctionPass, FunctionPassManager, FunctionPassReport,
    PassError, PassExecution, PassFailure, PassInstrumentation, PassOutcome, PreservedAnalyses,
};
