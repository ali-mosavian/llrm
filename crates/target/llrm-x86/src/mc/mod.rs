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

use llrm_debug::{DebugCaps, DebugObjectTarget, Format as DebugFormat, Info};
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
        self.write_described(format, object, None)
    }

    fn asm_info(
        &self,
        format: &str,
    ) -> Option<AsmInfo> {
        Some(match format {
            "omf" => AsmInfo { hosted: false, ..AsmInfo::default() },
            "elf" => AsmInfo { pref_align: true, hosted: true, unwind: UnwindKind::EhFrame },
            "macho" => AsmInfo { pref_align: false, hosted: true, unwind: UnwindKind::Compact },
            "coff" => AsmInfo { pref_align: false, hosted: true, unwind: UnwindKind::Win64 },
            _ => return None,
        })
    }
}

impl DebugObjectTarget for X86Objects {
    fn write_described(
        &self,
        format: &str,
        object: &Object,
        debug: Option<&Info>,
    ) -> Result<Vec<u8>, Unsupported> {
        // The debug writer is the format's pick, not the container's: DWARF for
        // ELF and Mach-O, C13 for COFF.
        let expanded;
        let object = match (format, debug) {
            ("elf" | "macho", Some(info)) => {
                expanded = llrm_dwarf::expanded(object, info)?;
                &expanded
            }
            ("coff", Some(info)) => {
                expanded = llrm_codeview::expanded(object, info, codeview_machine(object.bits))?;
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
        let macho_unwound;
        let object = if format == "macho" && object.bits == 64 && !object.frames.is_empty() {
            macho_unwound = unwind::macho_expanded(object)?;
            &macho_unwound
        } else {
            object
        };
        match (format, object.bits) {
            ("omf", _) => {
                llrm_omf::write::write(object, debug, &omf::X86Omf).map_err(|one| Unsupported(one.to_string()))
            }
            ("elf", 32) => elf32::write(object),
            ("elf", 64) => elf64::write(object),
            ("macho", 64) => llrm_macho::write(object, &macho::X8664MachO),
            ("coff", 32) => coff32::write(object),
            ("coff", 64) => coff64::write(&win64unwind::expanded(object)?),
            _ => Err(Unsupported(format!("no {format} writer for a {}-bit object yet", object.bits))),
        }
    }

    fn debug_caps(
        &self,
        format: &str,
        asked: DebugFormat,
    ) -> DebugCaps {
        match (asked, format) {
            (DebugFormat::Dwarf { .. }, _) => llrm_dwarf::CAPS,
            (DebugFormat::TurboDebugger, _) => llrm_omf::td::CAPS,
            // CodeView 4 names one place for a scope, C13 has ranges and no
            // frame address.
            (_, "omf") => DebugCaps { location_ranges: false, cfa_locations: false },
            (_, "coff") => DebugCaps { location_ranges: true, cfa_locations: false },
            _ => llrm_dwarf::CAPS,
        }
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

    /// OMF was written outside the table, so a caller needed a second dispatch
    /// for it: `write_object("omf", ..)` said "no omf writer".
    #[test]
    fn the_table_writes_omf_as_the_omf_writer_does() {
        let object = Object {
            name: "t.c".to_owned(),
            bits: 16,
            sections: Vec::new(),
            symbols: Vec::new(),
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        };
        let direct = llrm_omf::write::write(&object, None, &omf::X86Omf).unwrap();
        assert_eq!(X86Objects.write_object("omf", &object).unwrap(), direct);
    }

    /// The capabilities lived on `Format` and `AsmInfo` apart from the writers
    /// that have them: DWARF ranges and CFA, Turbo Debugger neither, CodeView
    /// 4 (OMF) neither, C13 (COFF) ranges only.
    #[test]
    fn the_target_says_what_each_debug_writer_can() {
        let caps = |format, asked| {
            let one = X86Objects.debug_caps(format, asked);
            (one.location_ranges, one.cfa_locations)
        };
        assert_eq!(caps("omf", DebugFormat::Default), (false, false));
        assert_eq!(caps("omf", DebugFormat::CodeView), (false, false));
        assert_eq!(caps("omf", DebugFormat::TurboDebugger), (false, false));
        assert_eq!(caps("omf", DebugFormat::Dwarf { version: 4 }), (true, true));
        assert_eq!(
            (caps("elf", DebugFormat::Default), caps("macho", DebugFormat::Default)),
            ((true, true), (true, true))
        );
        assert_eq!(
            (caps("coff", DebugFormat::Default), caps("coff", DebugFormat::CodeView)),
            ((true, false), (true, false))
        );
    }

    #[test]
    fn codeview_names_the_machine_by_width() {
        assert_eq!((codeview_machine(32), codeview_machine(64), codeview_machine(16)), (Some(0x03), Some(0xD0), None));
    }
}
