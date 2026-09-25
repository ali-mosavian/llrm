//! Port of `qbopt/frontend/qb`: QB HIR to a BASIC-envelope OMF object.

pub mod abi;
pub mod compile;
pub mod driver;
pub mod inline_x87;
pub mod main;
pub mod qbstages;
pub mod stage_text;
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
mod test_runtime_model;
