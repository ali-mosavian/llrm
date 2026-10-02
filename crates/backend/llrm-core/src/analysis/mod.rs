//! Ports of `qbopt/analysis`.

pub use llrm_bcmachine::analysis::flags;

pub mod alias;
pub mod avail;
pub mod cellmap;
pub mod consts;
pub mod effects;
pub mod floatfacts;
pub mod frequency;
pub mod induction;
pub mod intervals;
pub mod loops;
pub mod ranges;
pub mod regions;
