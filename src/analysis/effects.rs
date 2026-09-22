//! Port of `qbopt/analysis/effects.py`: conservative memory effects not
//! described by explicit MIR write ranges.

use crate::model::mir::{Kind, Op};

const _TRAPS: [Kind; 5] = [Kind::Fcheck, Kind::Div, Kind::Rem, Kind::Divmod, Kind::FixedDiv];

/// Whether an operation may write beyond its explicit MIR store ranges.
pub fn unmodeled_write(op: &Op) -> bool {
    (op.barrier() || op.kind == Kind::Call) && !op.memory_complete
}

/// Whether an operation may read beyond its explicit MIR load ranges.
pub fn unmodeled_read(op: &Op) -> bool {
    (op.barrier() || op.kind == Kind::Call) && !op.memory_complete
}

/// Whether a trap here can reach an ON ERROR handler that reads memory.
pub fn exposes_memory(op: &Op, handles_errors: bool) -> bool {
    handles_errors && (op.floating.is_some() || _TRAPS.contains(&op.kind))
}
