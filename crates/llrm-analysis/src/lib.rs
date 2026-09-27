//! Facts about the MIR, each adapted from the llrm-core analysis that owns
//! it there, with that analysis's tests. One analysis owns each fact; the
//! passes in llrm-transforms ask it.
//!
//! llrm-core's `analysis/flags.rs` has no port: it is liveness of x86 flags
//! over BC instructions, and MIR carries a carry or overflow as a value.

pub mod cfg;
pub mod effects;
pub mod frameescape;
pub mod interprocedural;
pub mod liveness;
pub mod noreturn;
pub mod occurrence;
pub mod pointerfacts;
pub mod ssa;
#[cfg(test)]
mod corpus_tests;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
