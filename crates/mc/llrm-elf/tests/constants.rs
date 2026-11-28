use llrm_elf::{Machine, Relocation, write};
use llrm_object::{
    Binding, Definition, Fix, FixupKind, Object, Reloc, Role, Section, Symbol, Target, Unsupported, Visibility,
    assembler::AsmBackend,
};

struct Plain;

struct PlainBackend;

impl AsmBackend for PlainBackend {
    fn kind_info(
        &self,
        kind: llrm_object::FixupKind,
    ) -> llrm_object::FixupKindInfo {
        kind.info()
    }

    fn write_nop_data(
        &self,
        _: &mut Vec<u8>,
        _: usize,
    ) -> bool {
        false
    }
}

impl Machine for Plain {
    const BITS: u8 = 64;
    const WIDE: bool = true;
    const NUMBER: u16 = 62;
    const ADDEND: bool = true;

    fn backend() -> &'static dyn AsmBackend {
        &PlainBackend
    }

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported> {
        let _ = reloc;
        Ok(Relocation { kind: 2, width: 4 })
    }
}

fn section(
    name: &str,
    role: Role,
    size: usize,
) -> Section {
    Section {
        name: name.into(),
        role,
        image: vec![0; size],
        relocs: Vec::new(),
        align: size.max(1),
        near: true,
        spans: vec![[0, size]],
    }
}

/// (name, sh_flags, sh_entsize) of each section header of a little-endian ELF64
/// file.
fn headers(bytes: &[u8]) -> Vec<(String, u64, u64)> {
    let word =
        |at: usize, width: usize| bytes[at..at + width].iter().rev().fold(0u64, |sum, &b| sum << 8 | u64::from(b));
    let (table, size, number, names) = (word(0x28, 8) as usize, word(0x3A, 2) as usize, word(0x3C, 2), word(0x3E, 2));
    let header = |index: usize| table + index * size;
    let strings = word(header(names as usize) + 0x18, 8) as usize;
    (1..number as usize)
        .map(|index| {
            let at = header(index);
            let name = strings + word(at, 4) as usize;
            let end = bytes[name..].iter().position(|&b| b == 0).unwrap();
            (String::from_utf8_lossy(&bytes[name..name + end]).into_owned(), word(at + 8, 8), word(at + 0x38, 8))
        })
        .collect()
}

/// A float constant pool was written as `.data`: writable. The pin's section
/// is `.rodata.cstN`, allocated and mergeable (SHF_ALLOC | SHF_MERGE) with
/// entry size N.
#[test]
fn a_constant_section_is_read_only_and_mergeable_with_its_entry_size() {
    let object = Object {
        name: "c.s".into(),
        bits: 64,
        sections: vec![
            section(".text", Role::Text, 4),
            section(".rodata.cst8", Role::ROData, 16),
            section(".rodata.cst4", Role::ROData, 8),
        ],
        symbols: Vec::new(),
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        comdats: Vec::new(),
        frames: Vec::new(),
    };
    let found = headers(&write::<Plain>(&object).unwrap());
    let of =
        |name: &str| found.iter().find(|one| one.0 == name).cloned().unwrap_or_else(|| panic!("no {name}: {found:?}"));
    assert_eq!(of(".rodata.cst8"), (".rodata.cst8".to_owned(), 0x12, 8));
    assert_eq!(of(".rodata.cst4"), (".rodata.cst4".to_owned(), 0x12, 4));
}

/// A relocation against a mergeable section named the section's symbol and an
/// offset: the linker took the entry from `S + A` of a field's pc bias and
/// stopped with "access beyond end of merged section (-4)" (floats -O2 at
/// m64 would not link). The relocation names the entry's own private label, a
/// local symbol there, as `.LCPI0_0` is.
#[test]
fn a_relocation_to_a_constant_entry_names_the_entrys_own_symbol() {
    let mut text = section(".text", Role::Text, 8);
    text.relocs = vec![Reloc::new(2, Fix::plain(FixupKind::DATA_4), Target::Symbol(0), 0)];
    let object = Object {
        name: "c.s".into(),
        bits: 64,
        sections: vec![text, section(".rodata.cst8", Role::ROData, 16)],
        symbols: vec![Symbol {
            name: "$K0".into(),
            binding: Binding::Local,
            definition: Definition::Defined { section: 1, offset: 8 },
            size: 0,
            group: None,
            visibility: Visibility::Default,
        }],
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        comdats: Vec::new(),
        frames: Vec::new(),
    };
    let bytes = write::<Plain>(&object).unwrap();
    let word =
        |at: usize, width: usize| bytes[at..at + width].iter().rev().fold(0u64, |sum, &b| sum << 8 | u64::from(b));
    let (table, size, number) = (word(0x28, 8) as usize, word(0x3A, 2) as usize, word(0x3C, 2) as usize);
    // The `.rela.text` section: type SHT_RELA (4).
    let rela = (0..number).map(|index| table + index * size).find(|&at| word(at + 4, 4) == 4).unwrap();
    let entry = word(rela + 0x18, 8) as usize;
    let info = word(entry + 8, 8);
    // Symbols: 0 null, 1 file, 2 and 3 the sections, 4 the entry's label.
    assert_eq!(info >> 32, 4, "the relocation names a section symbol, not the entry's label");
    assert_eq!(word(entry + 16, 8), 0, "an entry's offset is its symbol's, not the addend");
}
