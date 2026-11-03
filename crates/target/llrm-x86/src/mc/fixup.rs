//! x86's fixup kinds (`X86FixupKinds.h`), numbered from `FIRST_TARGET`.

use llrm_debug::DebugTarget;
use llrm_object::{Fix, FixupKind, FixupKindInfo};

/// A call or jump's target, relative to the field's end: the formats that tell
/// a branch from a data reference (Mach-O's `BRANCH`) read it.
pub const BRANCH_2: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 1);
pub const BRANCH_4: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 2);
/// x86-64: the place's offset from the target's global offset table entry
/// (`R_X86_64_REX_GOTPCRELX`).
pub const GOT_PCREL_4: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 3);
/// x86-64 static code: a symbol's address as a sign-extended `disp32`
/// beside a base or an index (`R_X86_64_32S`).
pub const ABS_SIGNED_4: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 6);

/// OMF: the selector of the target's segment (OMF's BASE).
pub const SEGMENT_BASE: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 4);
/// OMF: a 16:16 pointer, whose offset is the field's first two
/// bytes (OMF's POINTER).
pub const FAR_POINTER: FixupKind = FixupKind(FixupKind::FIRST_TARGET.0 + 5);

/// What a debug writer needs of the machine: its width and the kinds that name
/// a segment (OMF's, which 32-bit objects also carry).
pub fn debug_target(bits: u8) -> DebugTarget {
    DebugTarget {
        segment_base: Some(SEGMENT_BASE),
        far_pointer: Some(FAR_POINTER),
        entry_values: bits == 64,
        // System V: the ABI of the objects that carry DWARF.
        call_arguments: if bits == 64 {
            ["rdi", "rsi", "rdx", "rcx", "r8", "r9"].map(str::to_owned).to_vec()
        } else {
            Vec::new()
        },
        call_preserved: if bits == 64 {
            ["rbx", "rbp", "r12", "r13", "r14", "r15"].map(str::to_owned).to_vec()
        } else {
            Vec::new()
        },
        ..DebugTarget::of(bits)
    }
}

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

/// A symbol's address as a sign-extended 4-byte field.
pub const fn abs_signed_4() -> Fix {
    Fix::plain(ABS_SIGNED_4)
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
        ABS_SIGNED_4 => ("reloc_signed_4byte", 32),
        SEGMENT_BASE => ("segment_base", 16),
        FAR_POINTER => ("far_pointer", 32),
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
