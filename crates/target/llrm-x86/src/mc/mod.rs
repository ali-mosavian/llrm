//! What an x86 object writer asks of its target: the Mach-O writer's
//! description of x86-64.

pub mod coff32;
pub mod coff64;
pub mod elf32;
pub mod elf64;
pub mod macho;

use llrm_object::{Arch, AsmInfo, Object, ObjectTarget, Unsupported};

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
                expanded = llrm_codeview::expanded(object)?;
                &expanded
            }
            _ => object,
        };
        match (format, object.arch) {
            ("elf", Arch::I386) => elf32::write(object),
            ("elf", Arch::X8664) => elf64::write(object),
            ("macho", Arch::X8664) => llrm_macho::write(object, &macho::X8664MachO),
            ("coff", Arch::I386) => coff32::write(object),
            ("coff", Arch::X8664) => coff64::write(object),
            _ => Err(Unsupported(format!("no {format} writer for {:?} yet", object.arch))),
        }
    }

    fn asm_info(
        &self,
        format: &str,
    ) -> Option<AsmInfo> {
        Some(match format {
            "omf" => AsmInfo { pref_align: false, hosted: false, location_ranges: false, cfa_locations: false },
            "elf" | "macho" => {
                AsmInfo { pref_align: format == "elf", hosted: true, location_ranges: true, cfa_locations: true }
            }
            "coff" => AsmInfo { pref_align: false, hosted: true, location_ranges: true, cfa_locations: false },
            _ => return None,
        })
    }
}
