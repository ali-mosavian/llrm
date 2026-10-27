//! arm64 for the Mach-O writer, as LLVM's `AArch64MachObjectWriter` writes it
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/MCTargetDesc/AArch64MachObjectWriter.cpp>).
//!
//! # Runs
//! Once per Mach-O object of arm64, from `driver::object_file`.
//!
//! # Example
//! `llrm_macho::write(&object, &Arm64MachO)`.

use llrm_macho::{Described, MachTargetWriter};
use llrm_object::{Fix, FixupKind, Reloc, Unsupported};

const CPU_TYPE_ARM64: u32 = 0x0100_000C;
const CPU_SUBTYPE_ARM64_ALL: u32 = 0;
const ARM64_RELOC_UNSIGNED: u32 = 0;
const ARM64_RELOC_BRANCH26: u32 = 2;
const ARM64_RELOC_PAGE21: u32 = 3;
const ARM64_RELOC_PAGEOFF12: u32 = 4;
const ARM64_RELOC_GOT_LOAD_PAGE21: u32 = 5;
const ARM64_RELOC_GOT_LOAD_PAGEOFF12: u32 = 6;

pub struct Arm64MachO;

impl MachTargetWriter for Arm64MachO {
    fn cpu(&self) -> (u32, u32) {
        (CPU_TYPE_ARM64, CPU_SUBTYPE_ARM64_ALL)
    }

    fn lists_locals(&self) -> bool {
        true
    }

    fn relocation(
        &self,
        reloc: &Reloc,
    ) -> Result<Described, Unsupported> {
        let got = reloc.spec == Fix::S_GOT;
        let (kind, length, pcrel, addend_in_field) = match (reloc.kind, reloc.pc_rel, got) {
            (FixupKind::DATA_8, false, _) => (ARM64_RELOC_UNSIGNED, 3, false, true),
            (FixupKind::DATA_4, false, _) => (ARM64_RELOC_UNSIGNED, 2, false, true),
            (FixupKind::BRANCH26, _, _) => (ARM64_RELOC_BRANCH26, 2, true, false),
            (FixupKind::PAGE21, _, false) => (ARM64_RELOC_PAGE21, 2, true, false),
            (kind, _, false) if kind.is_page_offset12() => (ARM64_RELOC_PAGEOFF12, 2, false, false),
            (FixupKind::PAGE21, _, true) => (ARM64_RELOC_GOT_LOAD_PAGE21, 2, true, false),
            (FixupKind::LDST_IMM12_SCALE8, _, true) => (ARM64_RELOC_GOT_LOAD_PAGEOFF12, 2, false, false),
            other => return Err(Unsupported(format!("Mach-O arm64 has no relocation for {other:?}"))),
        };
        Ok(Described { kind, length, pcrel, beyond: 0, addend_in_field })
    }
}
