//! Port of `qbopt/analysis/effects.py`: conservative memory effects not
//! described by explicit MIR write ranges.

use crate::model::mir::{Kind, Op};

