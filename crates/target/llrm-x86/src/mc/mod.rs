//! What an x86 object writer asks of its target: the Mach-O writer's
//! description of x86-64.

pub mod backend;
pub mod coff32;
pub mod coff64;
pub mod compact_unwind;
pub mod elf32;
pub mod elf64;
pub mod fixup;
pub mod macho;
pub mod omf;
pub mod unwind;
pub mod win64unwind;

use llrm_object::{AsmInfo, Object, ObjectTarget, Unsupported, UnwindKind};

/// CodeView's machine number of x86 at `bits`.
fn codeview_machine(bits: u8) -> Option<u16> {
    match bits {
        32 => Some(0x03),
        64 => Some(0xD0),
        _ => None,
    }
}

/// x86's object writers: the ELF, COFF and Mach-O tables of i386 and x86-64.
pub struct X86Objects;

impl ObjectTarget for X86Objects {
    fn write_object(
        &self,
        format: &str,
        object: &Object,
    ) -> Result<Vec<u8>, Unsupported> {
        // The debug writer is the format's pick, not the container's: DWARF for
        // ELF and Mach-O, C13 for COFF.
        let expanded;
        let object = match (format, &object.debug) {
            ("elf" | "macho", Some(_)) => {
                expanded = llrm_dwarf::expanded(object, object.debug.as_ref().expect("matched"))?;
                &expanded
            }
            ("coff", Some(_)) => {
                expanded = llrm_codeview::expanded(object, codeview_machine(object.bits))?;
                &expanded
            }
            _ => object,
        };
        // `.eh_frame` goes in whether or not `-g` is on, for the frames the
        // backend declared.
        let unwound;
        let object = if format == "elf" && matches!(object.bits, 32 | 64) && !object.frames.is_empty() {
            let target = if object.bits == 32 { unwind::eh_target32() } else { unwind::eh_target() };
            unwound = llrm_dwarf::eh_frame::expanded(object, &target)?;
            &unwound
        } else {
            object
        };
        match (format, object.bits) {
            ("elf", 32) => elf32::write(object),
            ("elf", 64) => elf64::write(object),
            ("macho", 64) => llrm_macho::write(object, &macho::X8664MachO),
            ("coff", 32) => coff32::write(object),
            ("coff", 64) => coff64::write(&win64unwind::expanded(object)?),
            _ => Err(Unsupported(format!("no {format} writer for a {}-bit object yet", object.bits))),
        }
    }

    fn asm_info(
        &self,
        format: &str,
    ) -> Option<AsmInfo> {
        Some(match format {
            "omf" => AsmInfo { hosted: false, location_ranges: false, cfa_locations: false, ..AsmInfo::default() },
            "elf" => AsmInfo {
                pref_align: true,
                hosted: true,
                location_ranges: true,
                cfa_locations: true,
                unwind: UnwindKind::EhFrame,
            },
            "macho" => AsmInfo {
                pref_align: false,
                hosted: true,
                location_ranges: true,
                cfa_locations: true,
                unwind: UnwindKind::Compact,
            },
            "coff" => AsmInfo {
                pref_align: false,
                hosted: true,
                location_ranges: true,
                cfa_locations: false,
                unwind: UnwindKind::Win64,
            },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Arch` told the CodeView machine apart; the width does now, and a width
    /// x86 has no C13 machine for has none.
    /// Each format names the unwind information it carries; OMF has none.
    #[test]
    fn each_format_names_its_unwind_information() {
        let kind = |format| X86Objects.asm_info(format).unwrap().unwind;
        assert_eq!(
            [kind("omf"), kind("elf"), kind("macho"), kind("coff")],
            [UnwindKind::None, UnwindKind::EhFrame, UnwindKind::Compact, UnwindKind::Win64]
        );
    }

    #[test]
    fn codeview_names_the_machine_by_width() {
        assert_eq!((codeview_machine(32), codeview_machine(64), codeview_machine(16)), (Some(0x03), Some(0xD0), None));
    }
}
