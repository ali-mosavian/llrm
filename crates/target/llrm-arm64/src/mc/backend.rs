//! arm64's `MCAsmBackend`.

use llrm_object::assembler::AsmBackend;
use llrm_object::{FixupKind, FixupKindInfo, Unsupported};

use super::fixup;

pub struct Arm64Backend;

impl AsmBackend for Arm64Backend {
    fn kind_info(
        &self,
        kind: FixupKind,
    ) -> FixupKindInfo {
        fixup::kind_info(kind)
    }

    /// `AArch64AsmBackend::applyFixup`: the offset in the instruction's own
    /// field, ORed into the word at `at`. The kinds a label of the function
    /// leaves (`b`, `b.cond`, `tbz`, `adr`) are filled here; any other kind is
    /// a data field.
    fn apply_fixup(
        &self,
        image: &mut [u8],
        at: usize,
        kind: FixupKind,
        pc_rel: bool,
        value: i64,
        section: &str,
    ) -> Result<(), Unsupported> {
        let internal = match kind {
            fixup::BRANCH26 => super::encode::FixupKind::Branch26,
            fixup::BRANCH19 => super::encode::FixupKind::Branch19,
            fixup::BRANCH14 => super::encode::FixupKind::Branch14,
            fixup::ADR21 => super::encode::FixupKind::Adr21,
            _ => {
                let width = usize::from(self.kind_info(kind).bits).div_ceil(8);
                return llrm_object::apply_fixup(image, at, width, pc_rel, value, section);
            }
        };
        let field = image.get_mut(at..at + 4).ok_or_else(|| Unsupported(format!("{section}: no word at {at}")))?;
        let word = u32::from_le_bytes([field[0], field[1], field[2], field[3]]);
        let word = super::encode::resolve(word, internal, value)
            .map_err(|error| Unsupported(format!("{section}: {error}")))?;
        field.copy_from_slice(&word.to_le_bytes());
        Ok(())
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

    /// A label's offset in the instruction's field, as `llvm-mc -triple
    /// aarch64-linux-gnu` writes it (`b l1` at 0 with `l1` at 40 is
    /// `0x1400000a`; `adr x5, f` at 48 is `0x10fffe85`).
    #[test]
    fn a_label_offset_is_filled_as_the_pin_fills_it() {
        let cases: [(FixupKind, u32, i64, u32); 9] = [
            (fixup::BRANCH26, 0x1400_0000, 40, 0x1400_000a),
            (fixup::BRANCH26, 0x9400_0000, 36, 0x9400_0009),
            (fixup::BRANCH26, 0x1400_0000, -44, 0x17ff_fff5),
            (fixup::BRANCH19, 0x5400_0000, 32, 0x5400_0100),
            (fixup::BRANCH19, 0xb400_0001, 28, 0xb400_00e1),
            (fixup::BRANCH19, 0x5400_0001, -52, 0x54ff_fe61),
            (fixup::BRANCH14, 0x3618_0002, 24, 0x3618_00c2),
            (fixup::BRANCH14, 0x3708_0006, -56, 0x370f_fe46),
            (fixup::ADR21, 0x1000_0004, 20, 0x1000_00a4),
        ];
        for (kind, word, offset, want) in cases {
            let mut image = [0; 4];
            image.copy_from_slice(&word.to_le_bytes());
            Arm64Backend.apply_fixup(&mut image, 0, kind, true, offset, ".text").unwrap();
            assert_eq!(u32::from_le_bytes(image), want, "{kind:?} {offset}");
        }
        let mut image = 0x1000_0005u32.to_le_bytes();
        Arm64Backend.apply_fixup(&mut image, 0, fixup::ADR21, true, -48, ".text").unwrap();
        assert_eq!(u32::from_le_bytes(image), 0x10ff_fe85);
        assert!(Arm64Backend.apply_fixup(&mut image, 0, fixup::BRANCH26, true, 2, ".text").is_err());
    }

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
