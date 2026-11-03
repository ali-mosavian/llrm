//! arm64's fixup kinds (`AArch64FixupKinds.h`), numbered from `FIRST_TARGET`.

use llrm_object::{Fix, FixupKind, FixupKindInfo};

/// The word offset in the low 26 bits of a `b` or `bl`.
pub const BRANCH26: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 1);
/// The 4 KiB page of the target, relative to the place's page, in an `adrp`.
pub const PAGE21: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 2);
/// The target's offset in its page, in the 12-bit immediate of an `add`.
pub const ADD_IMM12: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 3);
/// The same in a load or store of `1 << n` bytes: `LDST_IMM12_SCALE1 + n`.
pub const LDST_IMM12_SCALE1: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 4);
pub const LDST_IMM12_SCALE2: FixupKind = FixupKind(LDST_IMM12_SCALE1.0 + 1);
pub const LDST_IMM12_SCALE4: FixupKind = FixupKind(LDST_IMM12_SCALE1.0 + 2);
pub const LDST_IMM12_SCALE8: FixupKind = FixupKind(LDST_IMM12_SCALE1.0 + 3);
pub const LDST_IMM12_SCALE16: FixupKind = FixupKind(LDST_IMM12_SCALE1.0 + 4);

/// The word offset in bits 23:5 of `b.cond`, `cbz` and `cbnz`.
pub const BRANCH19: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 9);
/// The word offset in bits 18:5 of `tbz` and `tbnz`.
pub const BRANCH14: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 10);
/// The byte offset of an `adr`, its low two bits in 30:29 and the rest in 23:5.
pub const ADR21: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 11);

pub const BRANCH26_FIX: Fix = Fix::relative(BRANCH26, 0);
pub const PAGE21_FIX: Fix = Fix::relative(PAGE21, 0);
/// `PAGE21` of the target's GOT entry.
pub const GOT_PAGE21: Fix = Fix { spec: Fix::S_GOT, ..PAGE21_FIX };
/// The offset of the target's GOT entry in its page, in an 8-byte load.
pub const GOT_PAGE_OFFSET12: Fix = Fix { spec: Fix::S_GOT, ..Fix::plain(LDST_IMM12_SCALE8) };

/// The target's offset in its page, in the 12-bit immediate of an `add`
/// (`access` is `None`) or of a load or store of `1 << access` bytes.
pub fn page_offset12(access: Option<u8>) -> Fix {
    Fix::plain(access.map_or(ADD_IMM12, |log| FixupKind(LDST_IMM12_SCALE1.0 + u16::from(log))))
}

/// Whether `kind` is `page_offset12`'s.
pub fn is_page_offset12(kind: FixupKind) -> bool {
    kind == ADD_IMM12 || (LDST_IMM12_SCALE1.0..=LDST_IMM12_SCALE16.0).contains(&kind.0)
}

/// What `kind` says of its field; the generic kinds are the object's.
pub fn kind_info(kind: FixupKind) -> FixupKindInfo {
    let (name, bits) = match kind {
        BRANCH26 | BRANCH19 | BRANCH14 | ADR21 | PAGE21 | ADD_IMM12 => ("arm64", 32),
        kind if is_page_offset12(kind) => ("arm64_ldst_imm12", 32),
        generic => return generic.info(),
    };
    FixupKindInfo { name, offset: 0, bits }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page-offset kinds are one family: `add` and the five load/store
    /// scales, and no other kind.
    #[test]
    fn the_page_offset_kinds_are_one_family() {
        assert!(
            is_page_offset12(page_offset12(None).kind)
                && (0..=4).all(|n| is_page_offset12(page_offset12(Some(n)).kind))
        );
        assert!(!is_page_offset12(PAGE21) && !is_page_offset12(BRANCH26));
        assert_eq!(kind_info(GOT_PAGE_OFFSET12.kind).bits, 32);
    }
}
