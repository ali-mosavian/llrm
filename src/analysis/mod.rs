//! Ports of `qbopt/analysis`.

pub mod alias;
pub(crate) mod constant_cycles;
pub mod flags;
pub(crate) mod consts;
pub mod effects;
pub mod frameescape;
pub(crate) mod induction;
pub(crate) mod interprocedural;
pub mod intervals;
pub mod liveness;
pub(crate) mod loops;
pub(crate) mod noreturn;
pub(crate) mod occurrence;
pub(crate) mod ranges;
pub(crate) mod regions;
pub(crate) mod ssa;
