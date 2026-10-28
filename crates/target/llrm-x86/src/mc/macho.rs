//! x86-64 for the Mach-O writer, as LLVM's `X86MachObjectWriter`.
//!
//! # Runs
//! Once per Mach-O object of x86-64, from `objbuild::serialized`.
//!
//! # Example
//! `llrm_macho::write(&object, &X8664MachO)`.

use llrm_macho::{Described, MachTargetWriter};
use llrm_object::{FixupKind, Reloc, Unsupported};

use super::fixup;

const CPU_TYPE_X86_64: u32 = 0x0100_0007;
const CPU_SUBTYPE_X86_64_ALL: u32 = 3;
const X86_64_RELOC_UNSIGNED: u32 = 0;
const X86_64_RELOC_SIGNED: u32 = 1;
const X86_64_RELOC_BRANCH: u32 = 2;
const X86_64_RELOC_SIGNED_1: u32 = 6;
const X86_64_RELOC_SIGNED_2: u32 = 7;
const X86_64_RELOC_SIGNED_4: u32 = 8;

fn unsupported(text: impl Into<String>) -> Unsupported {
    Unsupported(text.into())
}

pub struct X8664MachO;

impl MachTargetWriter for X8664MachO {
    fn cpu(&self) -> (u32, u32) {
        (CPU_TYPE_X86_64, CPU_SUBTYPE_X86_64_ALL)
    }

    fn lists_locals(&self) -> bool {
        false
    }

    fn relocation(
        &self,
        reloc: &Reloc,
    ) -> Result<Described, Unsupported> {
        let from = reloc.pc_bias;
        let (kind, length, pcrel, beyond) = match (reloc.kind, reloc.pc_rel) {
            (FixupKind::DATA_8, false) => (X86_64_RELOC_UNSIGNED, 3, false, 0),
            (FixupKind::DATA_4, false) => (X86_64_RELOC_UNSIGNED, 2, false, 0),
            (fixup::BRANCH_4, true) => (X86_64_RELOC_BRANCH, 2, true, 0),
            (FixupKind::DATA_4, true) if from == 4 => (X86_64_RELOC_SIGNED, 2, true, 0),
            (FixupKind::DATA_4, true) if from == 5 => (X86_64_RELOC_SIGNED_1, 2, true, 1),
            (FixupKind::DATA_4, true) if from == 6 => (X86_64_RELOC_SIGNED_2, 2, true, 2),
            (FixupKind::DATA_4, true) if from == 8 => (X86_64_RELOC_SIGNED_4, 2, true, 4),
            (fixup::SEGMENT_BASE, _) => return Err(unsupported("a segment selector has no Mach-O relocation")),
            (fixup::FAR_POINTER, _) => return Err(unsupported("a 16:16 far pointer has no Mach-O relocation")),
            other => return Err(unsupported(format!("Mach-O x86-64 has no relocation for {other:?}"))),
        };
        Ok(Described { kind, length, pcrel, beyond, addend_in_field: true })
    }
}
