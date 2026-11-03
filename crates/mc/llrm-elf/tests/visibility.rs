use llrm_elf::{Machine, Relocation, write};
use llrm_object::{
    Binding, Definition, Object, Reloc, Role, Section, Symbol, Unsupported, Visibility, assembler::AsmBackend,
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
        Err(Unsupported(format!("{:?}", reloc.kind)))
    }
}

fn symbol(
    name: &str,
    visibility: Visibility,
) -> Symbol {
    Symbol {
        name: name.into(),
        binding: Binding::Public,
        definition: Definition::Defined { section: 0, offset: 0 },
        size: 0,
        group: None,
        visibility,
    }
}

/// `st_other` of the last `count` symbols of a little-endian ELF64 file.
fn others(
    bytes: &[u8],
    count: usize,
) -> Vec<u8> {
    let word =
        |at: usize, width: usize| bytes[at..at + width].iter().rev().fold(0usize, |sum, &b| sum << 8 | b as usize);
    let (headers, size, number) = (word(0x28, 8), word(0x3A, 2), word(0x3C, 2));
    let symtab = (0..number).map(|index| headers + index * size).find(|&at| word(at + 4, 4) == 2).unwrap();
    let (offset, length) = (word(symtab + 0x18, 8), word(symtab + 0x20, 8));
    let entries = length / 24;
    (entries - count..entries).map(|index| bytes[offset + index * 24 + 5]).collect()
}

/// A hidden or protected symbol was written with `st_other` 0 (default
/// visibility), so a linker exported and preempted what the source hid.
#[test]
fn a_symbols_visibility_is_its_st_other() {
    let object = Object {
        name: "v.s".into(),
        bits: 64,
        sections: vec![Section {
            name: ".text".into(),
            role: Role::Text,
            image: vec![0; 4],
            relocs: Vec::new(),
            align: 4,
            near: true,
            spans: vec![[0, 4]],
        }],
        symbols: vec![
            symbol("a", Visibility::Default),
            symbol("b", Visibility::Hidden),
            symbol("c", Visibility::Protected),
        ],
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        frames: Vec::new(),
    };
    assert_eq!(others(&write::<Plain>(&object).unwrap(), 3), [0, 2, 3]);
}
