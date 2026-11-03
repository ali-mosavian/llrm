use llrm_elf::{Machine, Relocation, write};
use llrm_object::{
    Binding, Definition, Fix, FixupKind, Object, Reloc, Role, Section, Symbol, Target, Unsupported,
    assembler::AsmBackend,
};

struct Backend;

impl AsmBackend for Backend {
    fn kind_info(
        &self,
        kind: FixupKind,
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

/// ELF32 with RELA, as m68k is: big- and little-endian twins.
struct Big;
struct Little;

macro_rules! machine {
    ($name:ident, $big:expr) => {
        impl Machine for $name {
            const BITS: u8 = 32;
            const WIDE: bool = false;
            const NUMBER: u16 = 4;
            const ADDEND: bool = true;
            const BIG_ENDIAN: bool = $big;

            fn backend() -> &'static dyn AsmBackend {
                &Backend
            }

            fn relocation(_: &Reloc) -> Result<Relocation, Unsupported> {
                Ok(Relocation { kind: 1, width: 4 })
            }
        }
    };
}
machine!(Big, true);
machine!(Little, false);

fn object() -> Object {
    Object {
        name: "x.s".into(),
        bits: 32,
        sections: vec![Section {
            name: ".text".into(),
            role: Role::Text,
            image: vec![0; 8],
            relocs: vec![Reloc::new(2, Fix::plain(FixupKind::DATA_4), Target::Symbol(0), 0x10203)],
            align: 4,
            near: true,
            spans: vec![[0, 8]],
        }],
        symbols: vec![Symbol {
            name: "f".into(),
            binding: Binding::Public,
            definition: Definition::Undefined,
            size: 0,
            group: None,
        }],
        omf_groups: Vec::new(),
        omf_comdat: Vec::new(),
        frames: Vec::new(),
    }
}

fn u32_at(
    bytes: &[u8],
    at: usize,
    big: bool,
) -> u32 {
    let word: [u8; 4] = bytes[at..at + 4].try_into().unwrap();
    if big { u32::from_be_bytes(word) } else { u32::from_le_bytes(word) }
}

/// The section header `index` of an ELF32 file: (type, offset, size, entsize).
fn section(
    bytes: &[u8],
    index: usize,
    big: bool,
) -> (u32, usize, usize, usize) {
    let table = u32_at(bytes, 32, big) as usize + index * 40;
    let at = |field: usize| u32_at(bytes, table + field, big) as usize;
    (at(4) as u32, at(16), at(20), at(36))
}

fn rela(
    bytes: &[u8],
    big: bool,
) -> (usize, usize, usize) {
    let count =
        if big { u16::from_be_bytes([bytes[48], bytes[49]]) } else { u16::from_le_bytes([bytes[48], bytes[49]]) };
    let (_, offset, size, entsize) = (0..usize::from(count))
        .map(|index| section(bytes, index, big))
        .find(|one| one.0 == 4)
        .expect("a .rela section");
    (offset, size, entsize)
}

/// The header says which byte order the file is in, and every field of it
/// follows: `e_machine`, the section table, the symbol and relocation words.
#[test]
fn a_big_endian_machine_writes_a_big_endian_file() {
    let bytes = write::<Big>(&object()).unwrap();
    assert_eq!(bytes[5], 2, "EI_DATA is ELFDATA2MSB");
    assert_eq!(&bytes[18..20], &4u16.to_be_bytes(), "e_machine");
    let (offset, size, entsize) = rela(&bytes, true);
    assert_eq!((size, entsize), (12, 12), "one Elf32_Rela");
    assert_eq!(u32_at(&bytes, offset, true), 2, "r_offset");
    assert_eq!(u32_at(&bytes, offset + 4, true) & 0xff, 1, "r_info type");
    assert_eq!(u32_at(&bytes, offset + 8, true), 0x10203, "r_addend");
}

/// An ELF32 RELA entry is 12 bytes. The writer put an 8-byte addend in every
/// RELA entry, 16 bytes for a 32-bit machine, which no reader could walk.
#[test]
fn a_32_bit_rela_entry_is_twelve_bytes_in_either_order() {
    let bytes = write::<Little>(&object()).unwrap();
    assert_eq!(bytes[5], 1, "EI_DATA is ELFDATA2LSB");
    let (offset, size, entsize) = rela(&bytes, false);
    assert_eq!((size, entsize), (12, 12));
    assert_eq!(u32_at(&bytes, offset + 8, false), 0x10203, "r_addend");
}

/// An undefined weak symbol was written STB_GLOBAL, so a linker would refuse
/// the reference the program is written to survive.
#[test]
fn an_undefined_weak_symbol_is_weak() {
    let mut weak = object();
    weak.symbols[0].binding = Binding::Weak;
    let bytes = write::<Little>(&weak).unwrap();
    let count = usize::from(u16::from_le_bytes([bytes[48], bytes[49]]));
    let (_, offset, size, entsize) =
        (0..count).map(|index| section(&bytes, index, false)).find(|one| one.0 == 2).expect("a symbol table");
    let last = offset + size - entsize;
    assert_eq!(bytes[last + 12] >> 4, 2, "st_info binding of the undefined symbol is STB_WEAK");
}

struct Flagged;

impl Machine for Flagged {
    const BITS: u8 = 32;
    const WIDE: bool = false;
    const NUMBER: u16 = 6502;
    const ADDEND: bool = true;
    const FLAGS: u32 = 3;

    fn backend() -> &'static dyn AsmBackend {
        &Backend
    }

    fn relocation(_: &Reloc) -> Result<Relocation, Unsupported> {
        Ok(Relocation { kind: 3, width: 2 })
    }
}

/// `e_flags` was always 0, so a machine that names its CPU there (llvm-mos:
/// 0x3, mos6502 + mos6502bcd) wrote an object its linker takes for another.
#[test]
fn a_machine_sets_e_flags() {
    let bytes = write::<Flagged>(&object()).unwrap();
    assert_eq!(u32::from_le_bytes(bytes[36..40].try_into().unwrap()), 3);
}
