//! Opaque physical-register identities shared by object provenance and CodeGen.

use std::fmt;

/// A target-defined physical register number.
///
/// The support layer assigns no architectural meaning to the number. Targets
/// own that mapping; object provenance and Machine IR only need one shared,
/// typed identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalRegister(u32);

impl PhysicalRegister {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for PhysicalRegister {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
