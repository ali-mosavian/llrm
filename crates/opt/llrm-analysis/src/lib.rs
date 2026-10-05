//! Facts about the MIR, each adapted from the llrm-core analysis that owns
//! it there, with that analysis's tests. One analysis owns each fact; the
//! passes in llrm-transforms ask it.
//!
//! llrm-core's `analysis/flags.rs` has no port: it is liveness of x86 flags
//! over BC instructions, and MIR carries a carry or overflow as a value.
//! Its `analysis/manager.rs` is llrm-mir's `passes::Analyses`, LLVM's
//! analysis manager; `manager` makes the memory analyses its entries.

pub mod alias;
pub mod assumptions;
pub mod avail;
pub mod branchprob;
pub mod cellmap;
pub mod cfg;
pub mod constant_cycles;
pub mod difference;
pub mod consts;
pub mod effects;
pub mod floatbounds;
pub mod floatfacts;
pub mod frameescape;
pub mod graph;
pub mod guards;
pub mod globalsaa;
pub mod induction;
pub mod interprocedural;
pub mod liveness;
pub mod manager;
pub mod memory;
pub mod memoryssa;
pub mod noreturn;
pub mod parameter_ranges;
pub mod observers;
pub mod occurrence;
pub mod peelsize;
pub mod pointerfacts;
pub mod ranges;
pub mod regions;
pub mod ssa;
#[cfg(test)]
mod corpus_tests;
#[cfg(any(test, feature = "testing"))]
pub mod generated;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
