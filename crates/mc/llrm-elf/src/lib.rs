//! An [`Object`] as an ELF relocatable file, of either class. What a machine
//! adds is its ELF machine number, its relocation types, and whether its
//! relocations carry an addend (RELA) or leave it in the field (REL): see
//! [`Machine`]. `llrm-x86`'s `mc::elf32` and `mc::elf64` and `llrm-arm64`'s
//! `mc::elf` are the machines.

use llrm_object::{Binding, Definition, Object, Reloc, Role, Section, Target, Unsupported, assembler::AsmBackend};

const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHT_NOBITS: u32 = 8;
const SHT_INIT_ARRAY: u32 = 14;
const SHT_REL: u32 = 9;
const SHF_WRITE: u64 = 1;
const SHF_ALLOC: u64 = 2;
const SHF_EXECINSTR: u64 = 4;
const SHF_INFO_LINK: u64 = 0x40;
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;
const STB_WEAK: u8 = 2;
const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;
const STT_SECTION: u8 = 3;
const STT_FILE: u8 = 4;
const SHN_ABS: u16 = 0xFFF1;

/// A relocation type and the bytes of the field it patches.
pub struct Relocation {
    pub kind: u32,
    pub width: usize,
}

/// What a machine says of its ELF.
pub trait Machine {
    /// The width of a near offset, in bits, of the objects this machine takes.
    const BITS: u8;
    /// ELFCLASS64 rather than ELFCLASS32.
    const WIDE: bool;
    /// `e_machine`.
    const NUMBER: u16;
    /// Relocations carry their addend (RELA); else it is in the field (REL).
    const ADDEND: bool;
    /// `sh_type` of `.eh_frame`: `SHT_X86_64_UNWIND` on x86-64, else PROGBITS.
    const UNWIND_TYPE: u32 = SHT_PROGBITS;

    fn relocation(reloc: &Reloc) -> Result<Relocation, Unsupported>;

    /// The target's assembler backend, which fills the fields the file keeps
    /// (`MCAsmBackend::applyFixup`).
    fn backend() -> &'static dyn AsmBackend;
}

fn unsupported(text: impl Into<String>) -> Unsupported {
    Unsupported(text.into())
}

/// A string table: NUL-terminated names, the first byte NUL.
struct Strings(Vec<u8>);

impl Strings {
    fn new() -> Self {
        Strings(vec![0])
    }

    fn add(
        &mut self,
        text: &str,
    ) -> u32 {
        let at = self.0.len() as u32;
        self.0.extend(text.as_bytes());
        self.0.push(0);
        at
    }
}

struct Header {
    name: u32,
    kind: u32,
    flags: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    align: u64,
    entry: u64,
}

fn put16(
    out: &mut Vec<u8>,
    value: u16,
) {
    out.extend(value.to_le_bytes());
}

fn put32(
    out: &mut Vec<u8>,
    value: u32,
) {
    out.extend(value.to_le_bytes());
}

/// An address-sized field: 4 bytes in a 32-bit file, 8 in a 64-bit one.
fn put_word<M: Machine>(
    out: &mut Vec<u8>,
    value: u64,
) {
    if M::WIDE {
        out.extend(value.to_le_bytes());
    } else {
        put32(out, value as u32);
    }
}

/// The ELF name, type and flags of `section`.
fn spelling<M: Machine>(section: &Section) -> Result<(&'static str, u32, u64), Unsupported> {
    if !section.near {
        return Err(unsupported(format!(
            "{}: a segment addressed by its own selector has no ELF section",
            section.name
        )));
    }
    Ok(match section.role {
        Role::Text => (".text", SHT_PROGBITS, SHF_ALLOC | SHF_EXECINSTR),
        Role::ROData => (".rodata", SHT_PROGBITS, SHF_ALLOC),
        Role::Data => (".data", SHT_PROGBITS, SHF_ALLOC | SHF_WRITE),
        Role::Bss => (".bss", SHT_NOBITS, SHF_ALLOC | SHF_WRITE),
        Role::Stack => return Err(unsupported(format!("{}: an OMF stack segment has no ELF section", section.name))),
        // Named as the DWARF writer names it.
        Role::InitFunctions => (".init_array", SHT_INIT_ARRAY, SHF_ALLOC | SHF_WRITE),
        Role::Debug(_) => (".debug", SHT_PROGBITS, 0),
        Role::Unwind => (".eh_frame", M::UNWIND_TYPE, SHF_ALLOC),
    })
}

fn align(
    bytes: &mut Vec<u8>,
    to: usize,
) {
    while bytes.len() % to != 0 {
        bytes.push(0);
    }
}

/// `object` as an ELF relocatable file of machine `M`.
pub fn write<M: Machine>(object: &Object) -> Result<Vec<u8>, Unsupported> {
    if object.bits != M::BITS {
        return Err(unsupported(format!(
            "a {}-bit object is not the {}-bit this ELF writer is for",
            object.bits,
            M::BITS
        )));
    }
    if !object.omf_groups.is_empty() {
        return Err(unsupported("a group of segments is OMF's"));
    }

    // Section indices: 0 is the null section, the object's follow in order.
    let mut names = Strings::new();
    let spelled: Vec<(&str, u32, u64)> = object.sections.iter().map(spelling::<M>).collect::<Result<_, _>>()?;
    // A role's first section is `.text`, `.data`...; a further one is told
    // apart by its own name.
    let mut section_names: Vec<String> = Vec::new();
    for (index, section) in object.sections.iter().enumerate() {
        let taken =
            !matches!(section.role, Role::Debug(_)) && spelled[..index].iter().any(|one| one.0 == spelled[index].0);
        section_names.push(if matches!(section.role, Role::Debug(_)) {
            section.name.clone()
        } else if taken {
            format!("{}.{}", spelled[index].0, section.name)
        } else {
            spelled[index].0.to_owned()
        });
    }

    // Symbols: null, the source's file, one section symbol per section, then
    // the globals.
    let mut strings = Strings::new();
    let symbol_size = if M::WIDE { 24 } else { 16 };
    let mut symtab: Vec<u8> = vec![0; symbol_size];
    let symbol = |out: &mut Vec<u8>, name: u32, value: u64, size: u64, info: u8, section: u16| {
        put32(out, name);
        if M::WIDE {
            out.push(info);
            out.push(0);
            put16(out, section);
            out.extend(value.to_le_bytes());
            out.extend(size.to_le_bytes());
        } else {
            put32(out, value as u32);
            put32(out, size as u32);
            out.push(info);
            out.push(0);
            put16(out, section);
        }
    };
    let file = strings.add(&object.name);
    symbol(&mut symtab, file, 0, 0, STB_LOCAL << 4 | STT_FILE, SHN_ABS);
    for index in 0..object.sections.len() {
        symbol(&mut symtab, 0, 0, 0, STB_LOCAL << 4 | STT_SECTION, index as u16 + 1);
    }
    // The locals come before the globals (`sh_info` is the first global), as
    // `ELFObjectWriter` lists them; relocations name a local through its
    // section's symbol.
    let mut locals = 0;
    for one in &object.symbols {
        if let (Definition::Defined { section, offset }, Binding::Local) = (one.definition, one.binding) {
            // A label inside a function has no size: it is private, as `.L`
            // ones are.
            if one.is_private() || (object.sections[section].role == Role::Text && one.size == 0) {
                continue;
            }
            let kind = if object.sections[section].role == Role::Text { STT_FUNC } else { STT_OBJECT };
            let name = strings.add(&one.name);
            symbol(&mut symtab, name, offset as u64, one.size as u64, STB_LOCAL << 4 | kind, section as u16 + 1);
            locals += 1;
        }
    }
    let first_global = 2 + object.sections.len() + locals;
    // The ELF symbol of each object symbol that is global.
    let mut elf_symbol: Vec<Option<u32>> = vec![None; object.symbols.len()];
    let mut next = first_global as u32;
    for (index, one) in object.symbols.iter().enumerate() {
        if matches!(
            (one.definition, one.binding),
            (Definition::Defined { .. }, Binding::Local)
        ) {
            continue;
        }
        let name = strings.add(&one.name);
        match one.definition {
            Definition::Defined { section, offset } => {
                let kind = if object.sections[section].role == Role::Text { STT_FUNC } else { STT_OBJECT };
                let binding = if one.binding == Binding::Weak { STB_WEAK } else { STB_GLOBAL };
                symbol(&mut symtab, name, offset as u64, one.size as u64, binding << 4 | kind, section as u16 + 1);
            }
            Definition::Undefined => symbol(&mut symtab, name, 0, 0, STB_GLOBAL << 4, 0),
        }
        elf_symbol[index] = Some(next);
        next += 1;
    }

    // Each section's bytes (with REL's addends in the fields), and its
    // relocation entries.
    let mut images: Vec<Vec<u8>> = Vec::new();
    let mut relocs: Vec<Vec<u8>> = Vec::new();
    for section in &object.sections {
        let mut image = section.image.clone();
        let mut entries = Vec::new();
        for one in &section.relocs {
            let Relocation { kind, width } = M::relocation(one)?;
            let (symbol, own) = match one.target {
                Target::OmfGroup(_) => return Err(unsupported("a reference to a group is OMF's")),
                // The section's own symbol.
                Target::Section(index) => (2 + index as u32, 0),
                Target::Symbol(index) => match (elf_symbol[index], object.symbols[index].definition) {
                    (Some(symbol), _) => (symbol, 0),
                    // A local symbol is its section's symbol and its offset.
                    (None, Definition::Defined { section, offset }) => (2 + section as u32, offset as i64),
                    (None, Definition::Undefined) => unreachable!("an undefined symbol is global"),
                },
            };
            // S + A - P: the addend is the model's, less the pc bias.
            let addend = one.addend + own;
            // Checked for RELA too: the addend must fit the field the linker
            // would have patched.
            let mut field = [0u8; 8];
            M::backend().apply_fixup(&mut field, 0, one.kind, one.pc_rel, addend, &section.name)?;
            put_word::<M>(&mut entries, one.at as u64);
            if M::WIDE {
                entries.extend((u64::from(symbol) << 32 | u64::from(kind)).to_le_bytes());
            } else {
                put32(&mut entries, symbol << 8 | kind);
            }
            if M::ADDEND {
                entries.extend(addend.to_le_bytes());
            } else {
                image[one.at..one.at + width].copy_from_slice(&field[..width]);
            }
        }
        images.push(image);
        relocs.push(entries);
    }

    // The file: header, section contents, then the section header table.
    let header_size = if M::WIDE { 64 } else { 52 };
    let word = if M::WIDE { 8 } else { 4 };
    let mut out = vec![0u8; header_size];
    let mut headers =
        vec![Header { name: 0, kind: 0, flags: 0, offset: 0, size: 0, link: 0, info: 0, align: 0, entry: 0 }];
    for (index, section) in object.sections.iter().enumerate() {
        let (_, kind, flags) = spelled[index];
        let alignment = section.align.max(4).next_power_of_two();
        align(&mut out, alignment);
        let offset = out.len() as u64;
        // Every type but NOBITS has its bytes in the file (`.eh_frame` is
        // `SHT_X86_64_UNWIND`, `.init_array` `SHT_INIT_ARRAY`).
        if kind != SHT_NOBITS {
            out.extend(&images[index]);
        }
        headers.push(Header {
            name: names.add(&section_names[index]),
            kind,
            flags,
            offset,
            size: images[index].len() as u64,
            link: 0,
            info: 0,
            align: alignment as u64,
            entry: 0,
        });
    }
    let symtab_index = 1 + object.sections.len() + relocs.iter().filter(|one| !one.is_empty()).count() + 1;
    let (rel_kind, rel_name, rel_entry) =
        if M::ADDEND { (SHT_RELA, ".rela", 3 * word) } else { (SHT_REL, ".rel", 2 * word) };
    for (index, entries) in relocs.iter().enumerate().filter(|(_, one)| !one.is_empty()) {
        align(&mut out, word);
        let offset = out.len() as u64;
        out.extend(entries);
        let name = names.add(&format!("{rel_name}{}", section_names[index]));
        headers.push(Header {
            name,
            kind: rel_kind,
            flags: SHF_INFO_LINK,
            offset,
            size: entries.len() as u64,
            link: symtab_index as u32,
            info: index as u32 + 1,
            align: word as u64,
            entry: rel_entry as u64,
        });
    }
    // An empty .note.GNU-stack: the object does not need an executable stack.
    headers.push(Header {
        name: names.add(".note.GNU-stack"),
        kind: SHT_PROGBITS,
        flags: 0,
        offset: out.len() as u64,
        size: 0,
        link: 0,
        info: 0,
        align: 1,
        entry: 0,
    });
    align(&mut out, word);
    let offset = out.len() as u64;
    out.extend(&symtab);
    assert_eq!(headers.len(), symtab_index, "the relocations name the symbol table by this index");
    headers.push(Header {
        name: names.add(".symtab"),
        kind: SHT_SYMTAB,
        flags: 0,
        offset,
        size: symtab.len() as u64,
        link: headers.len() as u32 + 1,
        info: first_global as u32,
        align: word as u64,
        entry: symbol_size as u64,
    });
    let offset = out.len() as u64;
    out.extend(&strings.0);
    headers.push(Header {
        name: names.add(".strtab"),
        kind: SHT_STRTAB,
        flags: 0,
        offset,
        size: strings.0.len() as u64,
        link: 0,
        info: 0,
        align: 1,
        entry: 0,
    });
    let name = names.add(".shstrtab");
    let offset = out.len() as u64;
    out.extend(&names.0);
    headers.push(Header {
        name,
        kind: SHT_STRTAB,
        flags: 0,
        offset,
        size: names.0.len() as u64,
        link: 0,
        info: 0,
        align: 1,
        entry: 0,
    });
    align(&mut out, word);
    let table = out.len() as u64;
    for one in &headers {
        put32(&mut out, one.name);
        put32(&mut out, one.kind);
        put_word::<M>(&mut out, one.flags);
        put_word::<M>(&mut out, 0);
        put_word::<M>(&mut out, one.offset);
        put_word::<M>(&mut out, one.size);
        put32(&mut out, one.link);
        put32(&mut out, one.info);
        put_word::<M>(&mut out, one.align);
        put_word::<M>(&mut out, one.entry);
    }

    out[..16].copy_from_slice(&[0x7F, b'E', b'L', b'F', if M::WIDE { 2 } else { 1 }, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let mut header = Vec::new();
    put16(&mut header, 1); // ET_REL
    put16(&mut header, M::NUMBER);
    put32(&mut header, 1);
    put_word::<M>(&mut header, 0); // entry
    put_word::<M>(&mut header, 0); // program headers
    put_word::<M>(&mut header, table);
    put32(&mut header, 0); // flags
    put16(&mut header, header_size as u16);
    put16(&mut header, 0);
    put16(&mut header, 0);
    put16(&mut header, if M::WIDE { 64 } else { 40 });
    put16(&mut header, headers.len() as u16);
    put16(&mut header, headers.len() as u16 - 1);
    out[16..header_size].copy_from_slice(&header);
    Ok(out)
}
