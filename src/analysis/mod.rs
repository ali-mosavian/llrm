//! Ports of `qbopt/analysis`.

pub mod alias;
pub mod avail;
pub(crate) mod constant_cycles;
pub mod flags;
pub(crate) mod consts;
pub mod effects;
pub mod frameescape;
pub(crate) mod induction;
pub mod intervals;
pub mod liveness;
pub mod observers;
pub mod memoryssa;
pub(crate) mod loops;
pub(crate) mod occurrence;
pub mod pointerfacts;
pub(crate) mod ranges;
pub(crate) mod regions;
pub(crate) mod ssa;
