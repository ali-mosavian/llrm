//! What the x87 allocator raises, and where its stack cannot reach.

use std::fmt;

use crate::backend::frame as frames;
use crate::backend::lower::Unlowered;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::Insn;

/// The Python exceptions this module and `floatalloc` raise.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Raised {
    Unlowered(Unlowered),
    Refused(frames::Refused),
    /// `ValueError`.
    Value(String),
}

impl From<Unlowered> for Raised {
    fn from(error: Unlowered) -> Self {
        Self::Unlowered(error)
    }
}

impl From<frames::Refused> for Raised {
    fn from(error: frames::Refused) -> Self {
        Self::Refused(error)
    }
}

impl fmt::Display for Raised {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlowered(error) => error.fmt(formatter),
            Self::Refused(error) => error.fmt(formatter),
            Self::Value(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Raised {}

/// Whether the x87 stack cannot be assumed to survive this instruction.
#[must_use]
pub fn boundary(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    matches!(what.op, Operation::Call | Operation::Barrier)
        || what.sources.iter().chain(&what.dests).any(|arg| matches!(arg, Loc::St(_)))
}

