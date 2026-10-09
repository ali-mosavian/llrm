//! Opaque physical-register identities shared by object provenance and CodeGen.

use std::fmt;

/// A target-defined physical register number.
///
/// The support layer assigns no architectural meaning to the number. Targets
/// own that mapping; object provenance and Machine IR only need one shared,
/// typed identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalRegister(u32);

impl PhysicalRegister {}

impl fmt::Display for PhysicalRegister {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
