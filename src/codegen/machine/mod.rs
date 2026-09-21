//! Machine IR and target-independent register allocation.

mod allocation;
mod allocation_constraints;
mod allocation_rewrite;
mod allocation_spill;
mod interference;
mod liveness;
mod model;
mod text;
mod verify;

pub use allocation::{AllocationError, RegisterAssignment, allocate};
pub use allocation_constraints::{ConstraintError, ConstraintTarget, split_fixed_occurrences};
pub use allocation_rewrite::{AllocationRewriteError, apply_assignment};
pub use allocation_spill::{
    SpillMaterialization, SpillMaterializationError, SpillTarget, materialize_spills,
};
pub use interference::{InterferenceError, InterferenceGraph, compute_interference};
pub use liveness::{
    BlockLiveness, MachineLiveness, MachineLivenessError, compute_liveness,
};
pub use model::*;
pub use text::{FORMAT_VERSION, TextError, parse_text, write_text};
pub use verify::verify;
