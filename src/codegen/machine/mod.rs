//! Machine IR and target-independent register allocation.

mod allocation;
mod allocation_rewrite;
mod interference;
mod liveness;
mod model;
mod text;
mod verify;

pub use allocation::{AllocationError, RegisterAssignment, allocate};
pub use allocation_rewrite::{AllocationRewriteError, apply_assignment};
pub use interference::{InterferenceError, InterferenceGraph, compute_interference};
pub use liveness::{
    BlockLiveness, MachineLiveness, MachineLivenessError, compute_liveness,
};
pub use model::*;
pub use text::{FORMAT_VERSION, TextError, parse_text, write_text};
pub use verify::verify;
