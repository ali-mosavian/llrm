//! LLVM's block frequencies, translated at d1106deb71cc: `BranchProbability`,
//! `BlockFrequency`, `ScaledNumber<uint64_t>` and `BlockFrequencyInfoImpl`,
//! one generic translation over a [`FlowGraph`] that MIR's functions and
//! LIR's bodies both give.
//!
//! # Used by
//! Nothing yet; greedy's spill weights (#2276) and the inliner's IR
//! frequencies are to read it.

pub mod branchprobability;
pub mod frequency;
pub mod imp;
pub mod scalednumber;

pub use branchprobability::BranchProbability;
pub use frequency::BlockFrequency;
pub use imp::{BlockFrequencyInfo, EdgeProbabilities};
pub use scalednumber::Scaled64;

pub use crate::cycleinfo::FlowGraph;

#[cfg(test)]
mod tests;
