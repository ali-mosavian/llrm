//! Machine IR and target-independent register allocation.

mod allocation;
mod allocation_greedy;
mod allocation_constraints;
mod allocation_intervals;
mod allocation_rewrite;
mod allocation_spill;
mod interference;
mod liveness;
mod model;
mod semantics;
mod text;
mod verify;

pub use allocation::{AllocationError, RegisterAssignment, allocate};
pub use allocation_greedy::{GreedyAllocation, GreedyAllocationError, allocate_greedy};
pub use allocation_constraints::{ConstraintError, ConstraintTarget, split_fixed_occurrences};
pub use allocation_intervals::{
    DEF, GRACE, PER_INSN, PER_LEVEL, USE, LiveInterval, LiveSegment, MachineIntervalError,
    MachineIntervalIndexes, index_intervals, interval_weights, live_intervals,
    weighted_live_intervals,
};
pub use allocation_rewrite::{AllocationRewriteError, apply_assignment};
pub use allocation_spill::{
    SpillMaterialization, SpillMaterializationError, SpillTarget, materialize_spills,
};
pub use interference::{InterferenceError, InterferenceGraph, compute_interference};
pub use liveness::{
    BlockLiveness, MachineLiveness, MachineLivenessError, compute_liveness,
};
pub use model::*;
pub use semantics::*;
pub use text::{FORMAT_VERSION, TextError, parse_text, write_text};
pub use verify::verify;
