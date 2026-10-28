//! x86's fixup kinds (`X86FixupKinds.h`), numbered from `FIRST_TARGET`.

use llrm_object::{Fix, FixupKind, FixupKindInfo};

/// A call or jump's target, relative to the field's end: the formats that tell
/// a branch from a data reference (Mach-O's `BRANCH`) read it.
pub const BRANCH_2: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 1);
pub const BRANCH_4: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 2);
/// x86-64: the place's offset from the target's global offset table entry
/// (`R_X86_64_REX_GOTPCRELX`).
pub const GOT_PCREL_4: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 3);

/// A call or jump's target, `width` bytes relative to the field's end.
pub const fn branch(width: usize) -> Fix {
    Fix::relative(
        match width {
            2 => BRANCH_2,
            4 => BRANCH_4,
            _ => FixupKind::NONE,
        },
        width,
    )
}

/// The place's offset from the target's GOT entry, relative to the place `from`
/// bytes past the field's start.
pub const fn got_pc_rel(from: usize) -> Fix {
    Fix::relative(GOT_PCREL_4, from)
}

/// What `kind` says of its field; the generic kinds are the object's.
pub fn kind_info(kind: FixupKind) -> FixupKindInfo {
    let (name, bits) = match kind {
        BRANCH_2 => ("branch_2byte_pcrel", 16),
        BRANCH_4 => ("branch_4byte_pcrel", 32),
        GOT_PCREL_4 => ("reloc_riprel_4byte_gotpcrelx", 32),
        generic => return generic.info(),
    };
    FixupKindInfo { name, offset: 0, bits }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A branch kind missing from the table patched zero bytes, so OMF packed
    /// no displacement.
    #[test]
    fn a_branch_patches_its_width() {
        assert_eq!((kind_info(branch(2).kind).bits, kind_info(branch(4).kind).bits), (16, 32));
        assert_eq!(kind_info(got_pc_rel(4).kind).bits, 32);
        assert_eq!(kind_info(Fix::abs(8).kind).bits, 64);
    }
}
