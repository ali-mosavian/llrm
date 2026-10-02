//! Machine-independent floating formats and observable evaluation rules.
//!
//! Direct port of `qbopt/model/floating.py`: `Format`, `Precision`,
//! `Rounding`, `Exceptions`, and `Semantics`.  This is a source-neutral
//! value model: it records an operation's observable conversion rules, not
//! an instruction encoding, register assignment, or target floating unit.

use std::fmt;

/// Python `qbopt.model.floating:Format`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Format {
    Binary32,
    Binary64,
    Extended80,
    Signed16,
    Signed32,
    Signed64,
    Unsigned64,
}

impl Format {
    /// The exact `StrEnum` spelling from Python.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Binary32 => "binary32",
            Self::Binary64 => "binary64",
            Self::Extended80 => "extended80",
            Self::Signed16 => "signed16",
            Self::Signed32 => "signed32",
            Self::Signed64 => "signed64",
            Self::Unsigned64 => "unsigned64",
        }
    }

}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Python `qbopt.model.floating:Precision`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Precision {
    Exact,
    Destination,
    Dynamic,
    /// A store that rounds to its destination in memory, while a later read
    /// of the cell may observe the stored value at its source's precision.
    Excess,
}

impl Precision {
    /// The exact `StrEnum` spelling from Python.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Destination => "destination",
            Self::Dynamic => "dynamic",
            Self::Excess => "excess",
        }
    }
}

impl fmt::Display for Precision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Python `qbopt.model.floating:Rounding`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rounding {
    None,
    Dynamic,
    /// Whatever the environment says; C's cast to an integer.
    TowardZero,
}

impl Rounding {
    /// The exact `StrEnum` spelling from Python.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Dynamic => "dynamic",
            Self::TowardZero => "toward_zero",
        }
    }
}

impl fmt::Display for Rounding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Python `qbopt.model.floating:Exceptions`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Exceptions {
    Strict,
    Deferred,
}

impl Exceptions {
    /// The exact `StrEnum` spelling from Python.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Deferred => "deferred",
        }
    }
}

impl fmt::Display for Exceptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A storage conversion is distinct from arithmetic evaluation precision.
///
/// Dynamic properties read the floating environment. Strict exceptions
/// remain observable even when the numeric conversion itself is exact.
/// An absent rule on an operation means unknown semantics, never permission
/// to assume nearest rounding, no traps, or an unrounded store.
///
/// Direct port of `qbopt.model.floating:Semantics`. Field declaration order
/// and equality follow Python's frozen dataclass exactly.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Semantics {
    pub inputs: Box<[Format]>,
    pub result: Format,
    pub precision: Precision,
    pub rounding: Rounding,
    pub exceptions: Exceptions,
}

impl Semantics {

}
