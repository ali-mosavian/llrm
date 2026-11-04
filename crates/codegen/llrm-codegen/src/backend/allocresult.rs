//! What an allocation returns: the error kinds and the assignment.
//!
//! # How it works
//! [`Unplaced`] (a value no register can hold) and [`Spilled`] (a spill nothing
//! writes) are the two ways an allocation fails; [`Error`] adds a refused body
//! and carries them out of `RegAlloc`. [`Assignment`] says where each value
//! lives, what it cost and whether that is the best.
//!
//! # Used by
//! `backend::allocate`, the passes that run after it and the tests that
//! allocate a body.
//!
//! # Example
//! A body that names `eax` for three values live at once gives
//! `Error::Unplaced("no register for v3")`; one that fits gives an `Assignment`
//! with `spilled` empty.

use std::collections::BTreeSet;
use std::fmt;

use llrm_lir::registers::RegId;

use crate::backend::frame::Refused;
use crate::model::passes::Exception;
use crate::support::hash::IndexMap;

/// A value has no register, or no register it may take is free of values that
/// cannot be spilled: the body asks for more registers at one point than the
/// machine has, or names a register no value can be in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unplaced(pub String);

/// A value the allocator chose to spill, and nothing writes the spill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spilled(pub String);

impl fmt::Display for Unplaced {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for Spilled {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unplaced {}
impl std::error::Error for Spilled {}

/// Every exception that may leave the register-allocation modules.
///
/// Python raises these through one another's frames; Rust says which one
/// it was so `except Unplaced` can catch exactly that one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Unplaced(Unplaced),
    Spilled(Spilled),
    Refused(Refused),
    /// `ValueError`, or a call into a module not yet ported.
    Value(String),
}

impl fmt::Display for Error {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Unplaced(one) => one.fmt(formatter),
            Self::Spilled(one) => one.fmt(formatter),
            Self::Refused(one) => one.fmt(formatter),
            Self::Value(one) => formatter.write_str(one),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Python's class name and message, for a caller that catches by class.
    pub fn raised(&self) -> Exception {
        let (module, kind) = match self {
            Self::Unplaced(_) => ("qbopt.backend.allocate", "Unplaced"),
            Self::Spilled(_) => ("qbopt.backend.allocate", "Spilled"),
            Self::Refused(_) => ("qbopt.backend.frame", "Refused"),
            Self::Value(_) => ("builtins", "ValueError"),
        };
        Exception::defined_in(module, kind, self.to_string())
    }
}

impl From<Unplaced> for Error {
    fn from(one: Unplaced) -> Self {
        Self::Unplaced(one)
    }
}

impl From<Spilled> for Error {
    fn from(one: Spilled) -> Self {
        Self::Spilled(one)
    }
}

impl From<Refused> for Error {
    fn from(one: Refused) -> Self {
        Self::Refused(one)
    }
}

/// Where each value lives, what it cost, and whether that is the best.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub r#where: IndexMap<u32, RegId>,
    pub spilled: BTreeSet<u32>,
    pub cost: f64,
    pub optimal: bool,
    pub why: String,
}
