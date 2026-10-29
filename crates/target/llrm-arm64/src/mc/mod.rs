//! The machine code of arm64: an instruction as data (`inst`), its encoding
//! (`encode`), its text (`text`) and the immediates the encodings code
//! (`immediate`). Translated from LLVM's `AArch64` MC layer.

pub mod backend;
pub mod elf;
pub mod encode;
pub mod fixup;
pub mod immediate;
pub mod inst;
pub mod macho;
mod text;
pub mod unwind;
pub mod visit;

#[cfg(test)]
pub(crate) mod tests;

use llrm_object::{AsmInfo, Object, ObjectTarget, Unsupported, UnwindKind};

/// arm64's object writers.
pub struct Arm64Objects;

impl ObjectTarget for Arm64Objects {
    fn write_object(
        &self,
        format: &str,
        object: &Object,
    ) -> Result<Vec<u8>, Unsupported> {
        let expanded;
        let object = match (format, &object.debug) {
            ("elf" | "macho", Some(info)) => {
                expanded = llrm_dwarf::expanded(object, info)?;
                &expanded
            }
            _ => object,
        };
        match format {
            "macho" => llrm_macho::write(object, &macho::Arm64MachO),
            "elf" if !object.frames.is_empty() => {
                // `.eh_frame` goes in whether or not `-g` is on, for the frames
                // the backend declared.
                elf::write(&llrm_dwarf::eh_frame::expanded(object, &unwind::eh_target())?)
            }
            "elf" => elf::write(object),
            _ => Err(Unsupported(format!("no {format} writer for arm64 yet"))),
        }
    }

    fn asm_info(
        &self,
        format: &str,
    ) -> Option<AsmInfo> {
        matches!(format, "macho" | "elf").then_some(AsmInfo {
            pref_align: false,
            hosted: true,
            location_ranges: true,
            cfa_locations: true,
            unwind: if format == "macho" { UnwindKind::Compact } else { UnwindKind::EhFrame },
        })
    }
}
