//! x86 relocation kinds used by target instruction encodings.
//!
//! MC deliberately stores target fixup kinds as numeric identities.  This
//! module keeps the x86 meaning of those identities private to the target;
//! object writers decide how, or whether, a target kind maps to their format.

use crate::mc::FixupKind;

/// A relocation field in an x86 instruction encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum X86FixupKind {
    /// The offset:segment 16:16 field following an immediate far call/jump.
    FarPointer1616 = 1,
    /// A sixteen-bit absolute offset within the relocation target.
    Absolute16 = 2,
    /// A sixteen-bit displacement measured from the end of its field.
    ///
    /// This is the operand following the `E8` near-call opcode.  Keeping it
    /// separate from an absolute offset ensures object lowering cannot
    /// accidentally emit a segment-relative FIXUPP for a PC-relative call.
    PcRelative16 = 3,
    /// A sixteen-bit segment selector for the relocation target.
    Segment16 = 4,
}

impl X86FixupKind {
    /// Number of encoded bytes this relocation occupies.
    pub const fn width(self) -> u8 {
        match self {
            Self::FarPointer1616 => 4,
            Self::Absolute16 | Self::PcRelative16 | Self::Segment16 => 2,
        }
    }

    /// Whether the relocation is measured from the place being patched.
    pub const fn pc_relative(self) -> bool {
        match self {
            Self::FarPointer1616 | Self::Absolute16 | Self::Segment16 => false,
            Self::PcRelative16 => true,
        }
    }
}

impl From<X86FixupKind> for FixupKind {
    fn from(kind: X86FixupKind) -> Self {
        Self::new(kind as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn far_pointer_kind_has_a_stable_mc_identity() {
        let kind = X86FixupKind::FarPointer1616;
        assert_eq!(FixupKind::from(kind).get(), 1);
        assert_eq!(kind.width(), 4);
        assert!(!kind.pc_relative());

        let absolute = X86FixupKind::Absolute16;
        assert_eq!(FixupKind::from(absolute).get(), 2);
        assert_eq!(absolute.width(), 2);
        assert!(!absolute.pc_relative());

        let relative = X86FixupKind::PcRelative16;
        assert_eq!(FixupKind::from(relative).get(), 3);
        assert_eq!(relative.width(), 2);
        assert!(relative.pc_relative());

        let segment = X86FixupKind::Segment16;
        assert_eq!(FixupKind::from(segment).get(), 4);
        assert_eq!(segment.width(), 2);
        assert!(!segment.pc_relative());
    }
}
