//! Port of `qbopt/backend/lower.py`.
//!
//! Ported so far: `Unlowered`.

use std::fmt;

/// An operand nothing here can turn into a machine location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unlowered(pub String);

impl fmt::Display for Unlowered {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unlowered {}
