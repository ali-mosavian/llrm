//! Ports of `qbopt/analysis`.

pub use llrm_bcmachine::analysis::flags;

pub mod alias;
pub mod avail;
pub mod cellmap;
pub mod constant_cycles;
pub mod consts;
pub mod manager;
pub mod effects;
pub mod floatbounds;
pub mod floatfacts;
pub mod frameescape;
pub mod frequency;
pub mod induction;
pub mod interprocedural;
pub mod intervals;
pub mod liveness;
pub mod loops;
pub mod memoryssa;
pub mod noreturn;
pub mod observers;
pub mod peelsize;
pub mod occurrence;
pub mod pointerfacts;
pub mod ranges;
pub mod regions;
pub mod ssa;
