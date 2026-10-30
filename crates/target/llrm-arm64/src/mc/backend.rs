//! arm64's `MCAsmBackend`.

use llrm_object::assembler::AsmBackend;
use llrm_object::{FixupKind, FixupKindInfo};

use super::fixup;

pub struct Arm64Backend;

impl AsmBackend for Arm64Backend {
    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo {
        fixup::kind_info(kind)
    }

    /// `AArch64AsmBackend::writeNopData`: whole `nop` words (0xD503201F); no
    /// other length.
    fn write_nop_data(
        &self,
        out: &mut Vec<u8>,
        count: usize,
    ) -> bool {
        if count % 4 != 0 {
            return false;
        }
        for _ in 0..count / 4 {
            out.extend(0xD503_201Fu32.to_le_bytes());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pad of 8 is two nop words; a pad of 6 has no form and is refused,
    /// never padded short.
    #[test]
    fn a_pad_is_whole_nop_words() {
        let mut out = Vec::new();
        assert!(Arm64Backend.write_nop_data(&mut out, 8));
        assert_eq!(out, [0x1F, 0x20, 0x03, 0xD5, 0x1F, 0x20, 0x03, 0xD5]);
        assert!(!Arm64Backend.write_nop_data(&mut Vec::new(), 6));
    }
}
