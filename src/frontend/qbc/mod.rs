//! Port of `qbopt/frontend/qb`: QB HIR to a BASIC-envelope OMF object.
//!
//! `qbc`, not `qb`, only because `src/frontend/qb` holds the frozen parser
//! fork; the cutover moves this to `src/frontends/qb`.

pub mod abi;
pub mod compile;
pub mod driver;
pub mod inline_x87;
pub mod main;
pub mod qbstages;
pub mod stage_text;

#[cfg(test)]
mod test_hir;

#[cfg(test)]
mod test_hir_part1;

#[cfg(test)]
mod test_hir_part2;

#[cfg(test)]
mod test_hir_part3;
