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

use llrm_debug::{DebugObjectTarget, Info};
use llrm_object::{AsmInfo, Object, ObjectTarget, Unsupported, UnwindKind};

/// arm64's object writers.
pub struct Arm64Objects;

impl ObjectTarget for Arm64Objects {
    fn write_object(
        &self,
        format: &str,
        object: &Object,
    ) -> Result<Vec<u8>, Unsupported> {
        self.write_described(format, object, None)
    }

    fn asm_info(
        &self,
        format: &str,
    ) -> Option<AsmInfo> {
        matches!(format, "macho" | "elf").then_some(AsmInfo {
            pref_align: false,
            hosted: true,
            unwind: if format == "macho" { UnwindKind::Compact } else { UnwindKind::EhFrame },
            dwarf_relocs_across_sections: format != "macho",
        })
    }
}

impl DebugObjectTarget for Arm64Objects {
    fn write_described(
        &self,
        format: &str,
        object: &Object,
        debug: Option<&Info>,
    ) -> Result<Vec<u8>, Unsupported> {
        let expanded;
        let object = match (format, debug) {
            ("elf" | "macho", Some(info)) => {
                expanded = llrm_dwarf::expanded_for(
                    object,
                    info,
                    self.asm_info(format).is_some_and(|one| one.dwarf_relocs_across_sections),
                )?;
                &expanded
            }
            _ => object,
        };
        match format {
            "macho" if !object.frames.is_empty() => llrm_macho::write(&with_unwind(object)?, &macho::Arm64MachO),
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

    fn debug_caps(
        &self,
        _: &str,
        _: llrm_debug::Format,
    ) -> llrm_debug::DebugCaps {
        llrm_dwarf::CAPS
    }
}

/// `object` with `__eh_frame` and `__compact_unwind` for the frames the backend
/// declared: ld64 reads an entry of 32 bytes for each function (its address,
/// its length, the encoding, a personality and a language-specific area).
fn with_unwind(object: &Object) -> Result<Object, Unsupported> {
    use llrm_object::{Fix, Reloc, Role, Section, Target};
    let (mut expanded, fdes) = llrm_dwarf::eh_frame::expanded_as(object, &unwind::eh_target(), "__eh_frame")?;
    let (mut image, mut relocs) = (Vec::new(), Vec::new());
    for (frame, fde) in object.frames.iter().zip(fdes) {
        relocs.push(Reloc::new(image.len(), Fix::abs(8), Target::Section(frame.section), frame.offset as i64));
        image.extend(0u64.to_le_bytes());
        image.extend((frame.length as u32).to_le_bytes());
        image.extend(unwind::compact_encoding(&frame.rows, fde).to_le_bytes());
        image.extend([0u8; 16]);
    }
    expanded
        .sections
        .push(
            Section {
                name: "__compact_unwind".to_owned(),
                role: Role::Unwind,
                near: true,
                align: 8,
                spans: vec![[0, image.len()]],
                image,
                relocs,
            },
        );
    Ok(expanded)
}
