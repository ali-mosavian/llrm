//! An [`Object`] as a COFF object for i386 (`IMAGE_FILE_MACHINE_I386`). The container is
//! `llrm-coff`'s.

use llrm_coff::{Machine, Relocation};
use llrm_object::{Arch, Kind, Object, Unsupported};

const DIR32: u16 = 6;
const SECTION: u16 = 0xA;
const SECREL: u16 = 0xB;
const REL32: u16 = 0x14;

struct I386;

impl Machine for I386 {
    const ARCH: Arch = Arch::I386;
    const NUMBER: u16 = 0x14C;
    const SAFE_SEH: bool = true;

    fn relocation(kind: Kind) -> Result<Relocation, Unsupported> {
        let (kind, width, from, baked) = match kind {
            Kind::Abs { width: 4 } => (DIR32, 4, 0, 0),
            Kind::PcRel { width: 4, from } => (REL32, 4, from, 4),
            Kind::Branch { width: 4 } => (REL32, 4, 4, 4),
            Kind::SectionIndex => (SECTION, 2, 0, 0),
            Kind::SectionOffset { width: 4 } => (SECREL, 4, 0, 0),
            Kind::SegmentBase => return Err(Unsupported("a segment selector has no COFF relocation".into())),
            Kind::FarPointer => return Err(Unsupported("a 16:16 far pointer has no COFF relocation".into())),
            other => return Err(Unsupported(format!("COFF i386 has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width, from, baked })
    }
}

/// `object` as a COFF object file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_coff::write::<I386>(object)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use llrm_object::{Binding, Definition, Reloc, Role, Section, Symbol, Target};

    use super::*;

    fn le32(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    fn le16(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
    }

    /// (name, raw size, raw offset, relocation offset, relocation count, flags) of section `index`.
    fn header(bytes: &[u8], index: usize) -> (String, usize, usize, usize, usize, u32) {
        let at = 20 + 40 * index;
        let raw = &bytes[at..at + 8];
        let name = if raw[0] == b'/' {
            let offset: usize = std::str::from_utf8(&raw[1..]).unwrap().trim_end_matches('\0').parse().unwrap();
            strings(bytes, offset)
        } else {
            raw.iter().take_while(|&&one| one != 0).map(|&one| one as char).collect()
        };
        (name, le32(bytes, at + 16) as usize, le32(bytes, at + 20) as usize, le32(bytes, at + 24) as usize, le16(bytes, at + 32) as usize, le32(bytes, at + 36))
    }

    fn strings(bytes: &[u8], offset: usize) -> String {
        let table = le32(bytes, 8) as usize + 18 * le32(bytes, 12) as usize;
        bytes[table + offset..].iter().take_while(|&&one| one != 0).map(|&one| one as char).collect()
    }

    /// (name, value, section, type, class, aux) of symbol-table entry `index`.
    fn symbol_at(bytes: &[u8], index: usize) -> (String, u32, i16, u16, u8, u8) {
        let at = le32(bytes, 8) as usize + 18 * index;
        let name = if le32(bytes, at) == 0 { strings(bytes, le32(bytes, at + 4) as usize) } else { bytes[at..at + 8].iter().take_while(|&&one| one != 0).map(|&one| one as char).collect() };
        (name, le32(bytes, at + 8), le16(bytes, at + 12) as i16, le16(bytes, at + 14), bytes[at + 16], bytes[at + 17])
    }

    fn text(image: Vec<u8>, relocs: Vec<Reloc>) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: "A_TEXT".into(), role: Role::Text, near: true, align: 16, image, spans, relocs }
    }

    fn data(image: Vec<u8>, relocs: Vec<Reloc>) -> Section {
        Section { role: Role::Data, name: "_DATA".into(), align: 4, ..text(image, relocs) }
    }

    fn symbol(name: &str, binding: Binding, definition: Definition) -> Symbol {
        Symbol { name: name.into(), binding, definition, group: None }
    }

    fn object(sections: Vec<Section>, symbols: Vec<Symbol>) -> Object {
        Object { name: "a.c".into(), arch: Arch::I386, sections, symbols, omf_groups: Vec::new(), debug: None }
    }

    /// REL32 is relative to the field's end, so the field holds the addend, not ELF's addend - 4:
    /// writing -4 here would send every call four bytes short.
    #[test]
    fn a_call_is_rel32_with_its_addend_in_the_field() {
        let call = Reloc { at: 1, kind: Kind::Branch { width: 4 }, target: Target::Symbol(0), addend: 0 };
        let bytes = write(&object(vec![text(vec![0xE8, 0, 0, 0, 0, 0xC3], vec![call])], vec![symbol("_f", Binding::Public, Definition::Undefined)])).unwrap();
        let (name, size, raw, relocs, count, flags) = header(&bytes, 0);
        assert_eq!((name.as_str(), size, count), (".text", 6, 1));
        assert_eq!(&bytes[raw..raw + 6], [0xE8, 0, 0, 0, 0, 0xC3]);
        // The entry: offset, symbol index (`.file` and its aux, `@feat.00`, then the section's two), type.
        assert_eq!((le32(&bytes, relocs), le32(&bytes, relocs + 4), le16(&bytes, relocs + 8)), (1, 5, REL32));
        assert_eq!(symbol_at(&bytes, 5), ("_f".into(), 0, 0, 0, 2, 0));
        // 16-byte alignment, code, execute, read.
        assert_eq!(flags, 0x20 | 0x2000_0000 | 0x4000_0000 | 5 << 20);
    }

    /// A pc-relative field not ending the instruction (`cmp [sym], imm8` is 1 past) keeps the
    /// distance the linker will not take off: the field holds addend + 4 - from.
    #[test]
    fn a_pc_relative_field_short_of_the_instruction_end_compensates() {
        let reloc = Reloc { at: 2, kind: Kind::PcRel { width: 4, from: 5 }, target: Target::Symbol(0), addend: 0 };
        let bytes = write(&object(vec![text(vec![0x83, 0x3D, 0, 0, 0, 0, 1], vec![reloc])], vec![symbol("_v", Binding::Public, Definition::Undefined)])).unwrap();
        let raw = header(&bytes, 0).2;
        assert_eq!(le32(&bytes, raw + 2) as i32, -1);
    }

    /// A local symbol is a static in the table: DIR32 against it keeps the addend in the field and
    /// the symbol says where.
    #[test]
    fn a_local_symbol_is_a_static_and_dir32_keeps_the_addend_in_the_field() {
        let reloc = Reloc { at: 4, kind: Kind::Abs { width: 4 }, target: Target::Symbol(0), addend: 2 };
        let bytes = write(&object(vec![data(vec![0; 8], vec![reloc])], vec![symbol("loc", Binding::Local, Definition::Defined { section: 0, offset: 4 })])).unwrap();
        let (name, _, raw, relocs, ..) = header(&bytes, 0);
        assert_eq!(name, ".data");
        assert_eq!(le32(&bytes, raw + 4), 2);
        assert_eq!(le16(&bytes, relocs + 8), DIR32);
        assert_eq!(symbol_at(&bytes, le32(&bytes, relocs + 4) as usize), ("loc".into(), 4, 1, 0, 3, 0));
    }

    /// A name over eight bytes lives in the string table: the section's is `/offset`, the symbol's
    /// has four zero bytes first. Written inline it truncated both.
    #[test]
    fn long_names_live_in_the_string_table() {
        let mut second = text(vec![0xC3], vec![]);
        second.name = "second".into();
        let made = object(vec![text(vec![0xC3], vec![]), second], vec![symbol("_a_very_long_name", Binding::Public, Definition::Defined { section: 0, offset: 0 })]);
        let bytes = write(&made).unwrap();
        assert_eq!((header(&bytes, 0).0.as_str(), header(&bytes, 1).0.as_str()), (".text", ".text$second"));
        assert_eq!(symbol_at(&bytes, 7).0, "_a_very_long_name");
        assert_eq!(symbol_at(&bytes, 7).3, 0x20);
    }

    /// Bss stores nothing: a size and no raw data.
    #[test]
    fn bss_has_a_size_and_no_raw_data() {
        let bss = Section { role: Role::Bss, name: "_BSS".into(), image: vec![0; 64], spans: vec![], ..data(vec![], vec![]) };
        let bytes = write(&object(vec![bss], vec![])).unwrap();
        let (name, size, raw, _, _, flags) = header(&bytes, 0);
        assert_eq!((name.as_str(), size, raw, flags & 0x80), (".bss", 64, 0, 0x80));
    }

    /// Past 65535 relocations the header says 0xFFFF with LNK_NRELOC_OVFL, and the first entry's
    /// address is the real count plus one. A bare 16-bit count wrapped and lost relocations.
    #[test]
    fn more_than_65535_relocations_overflow_into_the_first_entry() {
        let total = 70_000;
        let relocs = (0..total).map(|at| Reloc { at: at * 4, kind: Kind::Abs { width: 4 }, target: Target::Symbol(0), addend: 0 }).collect();
        let bytes = write(&object(vec![data(vec![0; total * 4], relocs)], vec![symbol("_x", Binding::Public, Definition::Undefined)])).unwrap();
        let (_, _, _, at, count, flags) = header(&bytes, 0);
        assert_eq!((count, flags & 0x0100_0000), (0xFFFF, 0x0100_0000));
        assert_eq!(le32(&bytes, at) as usize, total + 1);
        assert_eq!(le32(&bytes, at + 10 + 10 * (total - 1)) as usize, (total - 1) * 4);
    }

    /// SECTION and SECREL are the debug sections' pair: 2 bytes and 4.
    #[test]
    fn section_and_secrel_are_two_and_four_byte_fields() {
        let relocs = vec![
            Reloc { at: 0, kind: Kind::SectionOffset { width: 4 }, target: Target::Symbol(0), addend: 0 },
            Reloc { at: 4, kind: Kind::SectionIndex, target: Target::Symbol(0), addend: 0 },
        ];
        let bytes = write(&object(vec![data(vec![0; 8], relocs)], vec![symbol("_x", Binding::Public, Definition::Undefined)])).unwrap();
        let at = header(&bytes, 0).3;
        assert_eq!((le16(&bytes, at + 8), le16(&bytes, at + 18)), (SECREL, SECTION));
    }

    /// What COFF cannot say is refused, never written near.
    #[test]
    fn what_coff_cannot_say_is_refused() {
        let at = |kind| {
            let reloc = Reloc { at: 0, kind, target: Target::Symbol(0), addend: 0 };
            write(&object(vec![text(vec![0; 4], vec![reloc])], vec![symbol("_f", Binding::Public, Definition::Undefined)]))
        };
        assert!(at(Kind::SegmentBase).is_err());
        assert!(at(Kind::FarPointer).is_err());
        assert!(at(Kind::Abs { width: 2 }).is_err());
        assert!(write(&object(vec![Section { near: false, ..data(vec![0], vec![]) }], vec![])).unwrap_err().0.contains("selector"));
        assert!(write(&object(vec![Section { role: Role::Stack, ..data(vec![0], vec![]) }], vec![])).is_err());
        assert!(write(&object(vec![Section { align: 3, ..data(vec![0], vec![]) }], vec![])).is_err());
        let mut other = object(vec![data(vec![0], vec![])], vec![]);
        other.arch = Arch::I8086;
        assert!(write(&other).is_err());
    }

    fn llvm(tool: &str) -> Option<std::path::PathBuf> {
        let mut dirs = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect::<Vec<_>>()).unwrap_or_default();
        dirs.push(Path::new("/usr/lib/llvm-20/bin").to_owned());
        dirs.iter().map(|dir| dir.join(tool)).find(|path| path.exists())
    }

    /// An i386 object says its handlers are all registered (`@feat.00` = 1): without it lld-link's
    /// default /safeseh refused the link, "is not compatible with SEH".
    #[test]
    fn the_object_declares_itself_safe_for_safeseh() {
        let bytes = write(&object(vec![text(vec![0xC3], vec![])], vec![])).unwrap();
        assert_eq!(symbol_at(&bytes, 2), ("@feat.00".into(), 1, -1, 0, 3, 0));
    }

    /// `_start` calls `_f` and stores `_f`'s address in `_slot`: lld-link links it and the call
    /// and the stored address both name `_f`; llvm-readobj and llvm-objdump read the object clean.
    #[test]
    fn lld_link_links_it_and_the_call_reaches_f() {
        let (Some(link), Some(readobj), Some(objdump)) = (llvm("lld-link"), llvm("llvm-readobj"), llvm("llvm-objdump")) else {
            eprintln!("skipped: needs lld-link, llvm-readobj and llvm-objdump");
            return;
        };
        let code = vec![0xE8, 0, 0, 0, 0, 0xC3, 0x90, 0x90, 0xC3];
        let call = Reloc { at: 1, kind: Kind::Branch { width: 4 }, target: Target::Symbol(1), addend: 0 };
        let slot = Reloc { at: 0, kind: Kind::Abs { width: 4 }, target: Target::Symbol(1), addend: 0 };
        let made = object(
            vec![text(code, vec![call]), data(vec![0; 4], vec![slot])],
            vec![symbol("_start", Binding::Public, Definition::Defined { section: 0, offset: 0 }), symbol("_f", Binding::Public, Definition::Defined { section: 0, offset: 8 }), symbol("_slot", Binding::Public, Definition::Defined { section: 1, offset: 0 })],
        );
        let scratch = tempfile::tempdir().unwrap();
        let (obj, exe) = (scratch.path().join("a.obj"), scratch.path().join("a.exe"));
        std::fs::write(&obj, write(&made).unwrap()).unwrap();
        let said = Command::new(&readobj).args(["--file-headers", "--sections", "--symbols", "--relocations"]).arg(&obj).output().unwrap();
        let text = format!("{}{}", String::from_utf8_lossy(&said.stdout), String::from_utf8_lossy(&said.stderr));
        assert!(said.status.success() && !text.contains("warning") && !text.contains("error"), "{text}");
        let linked = Command::new(&link).args(["/machine:x86", "/subsystem:console", "/entry:start", "/nodefaultlib", "/fixed", "/base:0x400000"]).arg(format!("/out:{}", exe.display())).arg(&obj).output().unwrap();
        assert!(linked.status.success(), "{}{}", String::from_utf8_lossy(&linked.stdout), String::from_utf8_lossy(&linked.stderr));
        let said = Command::new(&objdump).args(["-d", "--no-show-raw-insn"]).arg(&exe).output().unwrap();
        let listing = String::from_utf8_lossy(&said.stdout);
        // `.text` is at 0x401000: `_f` is 8 bytes in.
        assert!(listing.contains("calll\t0x401008") || listing.contains("call\t0x401008"), "{listing}");
        let said = Command::new(&objdump).args(["-s", "-j", ".data"]).arg(&exe).output().unwrap();
        assert!(String::from_utf8_lossy(&said.stdout).contains("08104000"), "{}", String::from_utf8_lossy(&said.stdout));
    }

}
