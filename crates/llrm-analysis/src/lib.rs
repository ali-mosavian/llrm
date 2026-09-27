//! Facts about the MIR, each adapted from the llrm-core analysis that owns
//! it there, with that analysis's tests. One analysis owns each fact; the
//! passes in llrm-transforms ask it.

pub mod cfg;
pub mod memory;
#[cfg(test)]
pub mod testing;
