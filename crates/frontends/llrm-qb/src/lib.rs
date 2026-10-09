//! Port of `qbopt/frontend/qb`: QB HIR to a BASIC-envelope OMF object.

pub use llrm_core::abi::qb as abi;
pub mod cli;
pub mod compile;
pub mod driver;
pub mod qbstages;
mod zero_fill;

#[cfg(test)]
mod test_hir;

#[cfg(test)]
mod test_hir_part1;

#[cfg(test)]
mod test_hir_part2;

#[cfg(test)]
mod test_hir_part3;

#[cfg(test)]
mod test_quickr;

#[cfg(test)]
mod test_frames;

#[cfg(test)]
mod test_stack_check;

#[cfg(test)]
mod test_intrinsics;

#[cfg(test)]
mod test_inline_runtime;

#[cfg(test)]
mod test_debug;

#[cfg(test)]
mod test_runtime_model;

#[cfg(test)]
mod callconv_tests;
#[cfg(test)]
mod test_pipeline;
