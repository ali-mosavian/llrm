//! Ports of `qbopt/analysis`.

pub mod alias;
pub mod avail;
pub(crate) mod cellmap;
pub(crate) mod constant_cycles;
pub(crate) mod consts;
pub(crate) mod manager;
pub mod effects;
pub mod flags;
pub(crate) mod floatbounds;
pub(crate) mod floatfacts;
pub mod frameescape;
pub(crate) mod induction;
pub(crate) mod interprocedural;
pub mod intervals;
pub mod liveness;
pub(crate) mod loops;
pub mod memoryssa;
pub(crate) mod noreturn;
pub mod observers;
pub(crate) mod peelsize;
pub(crate) mod occurrence;
pub mod pointerfacts;
pub(crate) mod ranges;
pub(crate) mod regions;
pub(crate) mod ssa;
