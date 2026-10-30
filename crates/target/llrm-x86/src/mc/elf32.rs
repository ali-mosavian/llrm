//! An [`Object`] as an ELF32 relocatable file for i386, REL relocations (the
//! addend is in the field), as GNU ld and lld take them. The container is
//! `llrm-elf`'s.

use llrm_elf::{Machine, Relocation};
use llrm_object::{FixupKind, Object, Reloc, Unsupported};

use super::fixup;

const R_386_32: u32 = 1;
const R_386_PC32: u32 = 2;
const R_386_PLT32: u32 = 4;
const R_386_16: u32 = 20;
const R_386_PC16: u32 = 21;
const R_386_8: u32 = 22;
const R_386_PC8: u32 = 23;

struct I386;

impl Machine for I386 {
    const BITS: u8 = 32;
    const WIDE: bool = false;
    const NUMBER: u16 = 3;
    const ADDEND: bool = false;

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        let from = reloc.pc_bias;
        let (kind, width, from) = match (reloc.kind, reloc.pc_rel) {
            (FixupKind::DATA_4, false) => (R_386_32, 4, 0),
            (FixupKind::DATA_2, false) => (R_386_16, 2, 0),
            (FixupKind::DATA_1, false) => (R_386_8, 1, 0),
            (FixupKind::DATA_4, true) => (R_386_PC32, 4, from),
            (fixup::BRANCH_4, true) => (R_386_PLT32, 4, 4),
            (fixup::BRANCH_2, true) => (R_386_PC16, 2, 2),
            (FixupKind::DATA_2, true) => (R_386_PC16, 2, from),
            (FixupKind::DATA_1, true) => (R_386_PC8, 1, from),
            (fixup::SEGMENT_BASE, _) => return Err(Unsupported("a segment selector has no ELF relocation".into())),
            (fixup::FAR_POINTER, _) => return Err(Unsupported("a 16:16 far pointer has no ELF relocation".into())),
            other => return Err(Unsupported(format!("ELF i386 has no relocation for {other:?}"))),
        };
        Ok(Relocation { kind, width, from })
    }
}

/// `object` as an ELF32 relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    llrm_elf::write::<I386>(object)
}

#[cfg(test)]
mod tests {
    use llrm_debug::DebugObjectTarget;
    use llrm_object::{Binding, Definition, Fix, Reloc, Role, Section, Symbol, Target};

    use super::*;

    fn word(
        bytes: &[u8],
        at: usize,
    ) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    /// (name, type, offset, size, link, info) of each section header.
    fn sections(bytes: &[u8]) -> Vec<(String, u32, usize, usize, u32, u32)> {
        let table = word(bytes, 32) as usize;
        let count = u16::from_le_bytes([bytes[48], bytes[49]]) as usize;
        let strings = {
            let header = table + 40 * u16::from_le_bytes([bytes[50], bytes[51]]) as usize;
            word(bytes, header + 16) as usize
        };
        (0..count)
            .map(|index| {
                let at = table + 40 * index;
                let name = bytes[strings + word(bytes, at) as usize..]
                    .iter()
                    .take_while(|&&one| one != 0)
                    .map(|&one| one as char)
                    .collect();
                (
                    name,
                    word(bytes, at + 4),
                    word(bytes, at + 16) as usize,
                    word(bytes, at + 20) as usize,
                    word(bytes, at + 24),
                    word(bytes, at + 28),
                )
            })
            .collect()
    }

    fn named<'a>(
        found: &'a [(String, u32, usize, usize, u32, u32)],
        name: &str,
    ) -> &'a (String, u32, usize, usize, u32, u32) {
        found.iter().find(|one| one.0 == name).unwrap_or_else(|| panic!("no section {name}: {found:?}"))
    }

    fn text(
        image: Vec<u8>,
        relocs: Vec<Reloc>,
    ) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: "A_TEXT".into(), role: Role::Text, near: true, align: 1, image, spans, relocs }
    }

    fn symbol(
        name: &str,
        binding: Binding,
        definition: Definition,
    ) -> Symbol {
        Symbol { name: name.into(), binding, definition, size: 0, group: None }
    }

    fn object(
        sections: Vec<Section>,
        symbols: Vec<Symbol>,
    ) -> Object {
        Object {
            name: "a.c".into(),
            bits: 32,
            sections,
            symbols,
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// A function's `st_size` was written 0, so a tool that sums symbol sizes
    /// read no code at all; it is the size the object model carries.
    #[test]
    fn a_function_symbol_carries_its_size() {
        let mut function = symbol("f", Binding::Public, Definition::Defined { section: 0, offset: 16 });
        function.size = 3;
        let bytes = write(&object(vec![text(vec![0x90; 19], vec![])], vec![function])).unwrap();
        let found = sections(&bytes);
        let table = named(&found, ".symtab");
        // Entries are 16 bytes: the null symbol, the file, one section, then
        // `f`: name, value, size.
        let at = table.2 + 16 * 3;
        assert_eq!((word(&bytes, at + 4), word(&bytes, at + 8)), (16, 3));
    }

    /// `call f`: REL puts S + A - P in the field, so the field is -4 where the
    /// call's addend is 0, and the entry is R_386_PC32 against `f`, the
    /// first global.
    #[test]
    fn a_call_is_r_386_pc32_with_minus_four_in_the_field() {
        let call = Reloc::new(1, Fix::pc_rel(4, 4), Target::Symbol(0), 0);
        let made = object(
            vec![text(vec![0xE8, 0, 0, 0, 0, 0xC3], vec![call])],
            vec![symbol("f", Binding::Public, Definition::Undefined)],
        );
        let bytes = write(&made).unwrap();
        let found = sections(&bytes);
        let code = named(&found, ".text");
        assert_eq!(&bytes[code.2..code.2 + 6], [0xE8, 0xFC, 0xFF, 0xFF, 0xFF, 0xC3]);
        let rel = named(&found, ".rel.text");
        assert_eq!((rel.1, rel.5), (9, 1));
        assert_eq!((word(&bytes, rel.2), word(&bytes, rel.2 + 4)), (1, 3 << 8 | R_386_PC32));
    }

    /// A local symbol has no ELF symbol: a reference to it is its section's
    /// symbol and its offset.
    #[test]
    fn a_local_symbol_is_its_section_and_its_offset() {
        let data = Section {
            name: "_DATA".into(),
            role: Role::Data,
            near: true,
            align: 4,
            image: vec![0; 8],
            spans: vec![[0, 8]],
            relocs: vec![Reloc::new(4, Fix::abs(4), Target::Symbol(0), 2)],
        };
        let made =
            object(vec![data], vec![symbol("loc", Binding::Local, Definition::Defined { section: 0, offset: 4 })]);
        let bytes = write(&made).unwrap();
        let found = sections(&bytes);
        let data = named(&found, ".data");
        assert_eq!(word(&bytes, data.2 + 4), 6);
        let rel = named(&found, ".rel.data");
        // symbol 2 is the first section's.
        assert_eq!(word(&bytes, rel.2 + 4), 2 << 8 | R_386_32);
    }

    /// Bss stores nothing and a section with no relocations has no REL section.
    #[test]
    fn bss_is_nobits_and_an_unrelocated_section_has_no_rel() {
        let bss = Section {
            name: "_BSS".into(),
            role: Role::Bss,
            near: true,
            align: 4,
            image: vec![0; 64],
            spans: vec![],
            relocs: vec![],
        };
        let bytes = write(&object(vec![bss], vec![])).unwrap();
        let found = sections(&bytes);
        assert_eq!((named(&found, ".bss").1, named(&found, ".bss").3), (8, 64));
        assert!(found.iter().all(|one| !one.0.starts_with(".rel")));
    }

    /// What ELF cannot say is refused, never written near.
    #[test]
    fn what_elf_cannot_say_is_refused() {
        let at = |fix| {
            let reloc = Reloc::new(0, fix, Target::Symbol(0), 0);
            write(&object(
                vec![text(vec![0; 4], vec![reloc])],
                vec![symbol("f", Binding::Public, Definition::Undefined)],
            ))
        };
        assert!(at(Fix::plain(fixup::SEGMENT_BASE)).is_err());
        assert!(at(Fix::plain(fixup::FAR_POINTER)).is_err());
        let far = Section { near: false, role: Role::Data, ..text(vec![0], vec![]) };
        assert!(write(&object(vec![far], vec![])).unwrap_err().0.contains("selector"));
        let stack = Section { role: Role::Stack, ..text(vec![0], vec![]) };
        assert!(write(&object(vec![stack], vec![])).is_err());
        let mut real = object(vec![text(vec![0], vec![])], vec![]);
        real.bits = 16;
        assert!(write(&real).is_err());
    }

    /// An object that asks for CodeView or Turbo Debugger information is
    /// refused by name: the information of another format is never written
    /// in its place.
    #[test]
    fn a_debug_format_this_object_cannot_carry_is_refused() {
        for (format, name) in
            [(llrm_debug::Format::CodeView, "CodeView"), (llrm_debug::Format::TurboDebugger, "Turbo Debugger")]
        {
            let made = object(vec![text(vec![0], vec![])], vec![]);
            let debug = Some(llrm_debug::Info {
                options: llrm_debug::DebugOptions { format, ..Default::default() },
                ..Default::default()
            });
            let why = crate::mc::X86Objects.write_described("elf", &made, debug.as_ref()).unwrap_err().0;
            assert!(why.contains("cannot carry") && why.contains(name), "{why}");
        }
    }
}
