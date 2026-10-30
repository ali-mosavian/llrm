//! Where the x87 stack cannot reach, and the error type the x87 allocator
//! raises.
//!
//! # How it works
//! `boundary` says whether the x87 stack cannot be assumed to survive an
//! instruction: a call, a barrier, an instruction with no semantics, or one
//! that names an x87 register by position (`target::positional_place`). The
//! allocator's passes treat each such instruction as a point where no value may
//! be live on the stack: [`crate::backend::floatassign`] ends a register
//! stretch there, [`crate::backend::floatalloc`] flushes the stack there.
//!
//! `Raised` is the error of those two passes: an operand that cannot be lowered
//! (`Unlowered`), a frame cell refused (`frame::Refused`), or a bad value.
//!
//! # Example
//! No honest example: `boundary` is a predicate and `Raised` an error type, so
//! neither rewrites LIR. The effect of a boundary is visible in the
//! `floatassign` example, where the call ends the register stretch of `a`.
//!
//! # Used by
//! `floatassign`, `floatalloc`.

use std::fmt;

use llrm_lir::registers::Regs;

use crate::backend::frame as frames;
use crate::model::ir::Operation;
use crate::model::lir::Insn;

/// An operand nothing here can turn into a machine location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unlowered(pub String);

impl fmt::Display for Unlowered {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unlowered {}

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
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Unlowered(error) => error.fmt(formatter),
            Self::Refused(error) => error.fmt(formatter),
            Self::Value(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Raised {}

/// Whether the x87 stack is cleared at this instruction (`handleCall`): a call,
/// or code nothing is known of. A barrier (a port access) leaves it as it is.
#[must_use]
pub fn clears_stack(
    regs: Regs,
    one: &Insn,
) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    what.op == Operation::Call
        || what.sources.iter().chain(&what.dests).any(|place| crate::backend::target::positional_place(regs, place))
}

/// Whether the x87 stack cannot be assumed to survive this instruction.
#[must_use]
pub fn boundary(
    regs: Regs,
    one: &Insn,
) -> bool {
    let Some(what) = &one.what else {
        return true;
    };
    matches!(what.op, Operation::Call | Operation::Barrier)
        || what.sources.iter().chain(&what.dests).any(|place| crate::backend::target::positional_place(regs, place))
}
