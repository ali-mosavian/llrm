//! Port of `qbopt/analysis/effects.py`: conservative memory effects not
//! described by explicit MIR write ranges.

use crate::model::mir::{Kind, Op};

/// Whether an operation may write beyond its explicit MIR store ranges.
pub fn unmodeled_write(op: &Op) -> bool {
    (op.barrier() || op.kind == Kind::Call) && !op.memory_complete
}
