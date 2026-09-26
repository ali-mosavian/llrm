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
    pub const ALL: [Self; 7] = [
        Self::Binary32,
        Self::Binary64,
        Self::Extended80,
        Self::Signed16,
        Self::Signed32,
        Self::Signed64,
        Self::Unsigned64,
    ];

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
    pub const ALL: [Self; 4] = [Self::Exact, Self::Destination, Self::Dynamic, Self::Excess];

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
    pub const ALL: [Self; 3] = [Self::None, Self::Dynamic, Self::TowardZero];

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
    pub const ALL: [Self; 2] = [Self::Strict, Self::Deferred];

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
    /// Constructs Python's four-required-field form, whose exception rule is
    /// the dataclass default `Exceptions.STRICT`.
    pub fn new(
        inputs: impl Into<Box<[Format]>>,
        result: Format,
        precision: Precision,
        rounding: Rounding,
    ) -> Self {
        Self::with_exceptions(inputs, result, precision, rounding, Exceptions::Strict)
    }

    /// Constructs the five-field Python dataclass form with an explicit
    /// exception rule. All choices remain the closed, typed enums above.
    pub fn with_exceptions(
        inputs: impl Into<Box<[Format]>>,
        result: Format,
        precision: Precision,
        rounding: Rounding,
        exceptions: Exceptions,
    ) -> Self {
        Self {
            inputs: inputs.into(),
            result,
            precision,
            rounding,
            exceptions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Exceptions, Format, Precision, Rounding, Semantics};

    #[test]
    fn format_spellings_are_the_python_strenum_values() {
        assert_eq!(
            Format::ALL.map(Format::as_str),
            [
                "binary32",
                "binary64",
                "extended80",
                "signed16",
                "signed32",
                "signed64",
                "unsigned64",
            ]
        );
    }

    #[test]
    fn precision_spellings_are_the_python_strenum_values() {
        assert_eq!(
            Precision::ALL.map(Precision::as_str),
            ["exact", "destination", "dynamic", "excess"]
        );
    }

    #[test]
    fn rounding_spellings_are_the_python_strenum_values() {
        assert_eq!(
            Rounding::ALL.map(Rounding::as_str),
            ["none", "dynamic", "toward_zero"]
        );
    }

    #[test]
    fn exception_spellings_are_the_python_strenum_values() {
        assert_eq!(
            Exceptions::ALL.map(Exceptions::as_str),
            ["strict", "deferred"]
        );
    }

    #[test]
    fn semantics_defaults_exceptions_to_strict() {
        let rule = Semantics::new(
            [Format::Extended80, Format::Extended80],
            Format::Extended80,
            Precision::Dynamic,
            Rounding::Dynamic,
        );

        assert_eq!(
            rule.inputs.as_ref(),
            &[Format::Extended80, Format::Extended80]
        );
        assert_eq!(rule.result, Format::Extended80);
        assert_eq!(rule.precision, Precision::Dynamic);
        assert_eq!(rule.rounding, Rounding::Dynamic);
        assert_eq!(rule.exceptions, Exceptions::Strict);
    }

    #[test]
    fn semantics_explicit_exceptions_and_equality_match_frozen_dataclass() {
        let strict = Semantics::new(
            [Format::Extended80],
            Format::Binary32,
            Precision::Destination,
            Rounding::Dynamic,
        );
        let same = Semantics::with_exceptions(
            [Format::Extended80],
            Format::Binary32,
            Precision::Destination,
            Rounding::Dynamic,
            Exceptions::Strict,
        );
        let deferred = Semantics::with_exceptions(
            [Format::Extended80],
            Format::Binary32,
            Precision::Destination,
            Rounding::Dynamic,
            Exceptions::Deferred,
        );
        let different_inputs = Semantics::with_exceptions(
            [Format::Binary64],
            Format::Binary32,
            Precision::Destination,
            Rounding::Dynamic,
            Exceptions::Strict,
        );
        let different_result = Semantics::with_exceptions(
            [Format::Extended80],
            Format::Binary64,
            Precision::Destination,
            Rounding::Dynamic,
            Exceptions::Strict,
        );
        let different_precision = Semantics::with_exceptions(
            [Format::Extended80],
            Format::Binary32,
            Precision::Exact,
            Rounding::Dynamic,
            Exceptions::Strict,
        );
        let different_rounding = Semantics::with_exceptions(
            [Format::Extended80],
            Format::Binary32,
            Precision::Destination,
            Rounding::None,
            Exceptions::Strict,
        );

        assert_eq!(strict, same);
        assert_ne!(strict, different_inputs);
        assert_ne!(strict, different_result);
        assert_ne!(strict, different_precision);
        assert_ne!(strict, different_rounding);
        assert_ne!(strict, deferred);
    }

    #[test]
    fn single_store_rounding_boundary_rules_retain_the_numeric_policy() {
        // Model-level representation asserted by both parameterizations of
        // `tests/test_floating.py:test_single_store_is_a_rounding_boundary`.
        for exceptions in [Exceptions::Deferred, Exceptions::Strict] {
            let load = Semantics::with_exceptions(
                [Format::Binary32],
                Format::Extended80,
                Precision::Exact,
                Rounding::None,
                exceptions,
            );
            let multiply = Semantics::with_exceptions(
                [Format::Extended80, Format::Extended80],
                Format::Extended80,
                Precision::Dynamic,
                Rounding::Dynamic,
                exceptions,
            );
            let store = Semantics::with_exceptions(
                [Format::Extended80],
                Format::Binary32,
                Precision::Destination,
                Rounding::Dynamic,
                exceptions,
            );

            assert_eq!(load.inputs.as_ref(), &[Format::Binary32]);
            assert_eq!(load.result, Format::Extended80);
            assert_eq!(load.rounding, Rounding::None);
            assert_eq!(multiply.precision, Precision::Dynamic);
            assert_eq!(multiply.rounding, Rounding::Dynamic);
            assert_eq!(store.inputs.as_ref(), &[Format::Extended80]);
            assert_eq!(store.result, Format::Binary32);
            assert_eq!(store.rounding, Rounding::Dynamic);
            assert!(
                [&load, &multiply, &store]
                    .into_iter()
                    .all(|rule| rule.exceptions == exceptions)
            );
        }
    }
}
