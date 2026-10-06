//! An [`Object`] as an ELF32 relocatable file for i386: PROGBITS and NOBITS sections, a symbol
//! table, and REL relocations (the addend is in the field), as GNU ld and lld take them.

use llrm_object::{Arch, Binding, Definition, Kind, Object, Role, Section, Target, Unsupported};

const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_NOBITS: u32 = 8;
const SHT_REL: u32 = 9;
const SHF_WRITE: u32 = 1;
const SHF_ALLOC: u32 = 2;
const SHF_EXECINSTR: u32 = 4;
const SHF_INFO_LINK: u32 = 0x40;
const STB_LOCAL: u8 = 0;
const STB_GLOBAL: u8 = 1;
const STT_OBJECT: u8 = 1;
const STT_FUNC: u8 = 2;
const STT_SECTION: u8 = 3;
const STT_FILE: u8 = 4;
const SHN_ABS: u16 = 0xFFF1;
const R_386_32: u8 = 1;
const R_386_PC32: u8 = 2;
const R_386_16: u8 = 20;
const R_386_PC16: u8 = 21;
const R_386_8: u8 = 22;
const R_386_PC8: u8 = 23;

fn unsupported(text: impl Into<String>) -> Unsupported {
    Unsupported(text.into())
}

/// A string table: NUL-terminated names, the first byte NUL.
struct Strings(Vec<u8>);

impl Strings {
    fn new() -> Self {
        Strings(vec![0])
    }

    fn add(&mut self, text: &str) -> u32 {
        let at = self.0.len() as u32;
        self.0.extend(text.as_bytes());
        self.0.push(0);
        at
    }
}

struct Header {
    name: u32,
    kind: u32,
    flags: u32,
    offset: u32,
    size: u32,
    link: u32,
    info: u32,
    align: u32,
    entry: u32,
}

/// The ELF name and flags of `section`, and whether it holds bytes.
fn spelling(section: &Section) -> Result<(&'static str, u32, u32), Unsupported> {
    if !section.near {
        return Err(unsupported(format!("{}: a segment addressed by its own selector has no ELF section", section.name)));
    }
    Ok(match section.role {
        Role::Text => (".text", SHT_PROGBITS, SHF_ALLOC | SHF_EXECINSTR),
        Role::ROData => (".rodata", SHT_PROGBITS, SHF_ALLOC),
        Role::Data => (".data", SHT_PROGBITS, SHF_ALLOC | SHF_WRITE),
        Role::Bss => (".bss", SHT_NOBITS, SHF_ALLOC | SHF_WRITE),
        Role::Stack => return Err(unsupported(format!("{}: an OMF stack segment has no ELF section", section.name))),
        Role::Debug(_) => return Err(unsupported(format!("{}: CodeView debug information is OMF's", section.name))),
    })
}

/// R_386_* of a field, and the bytes of it.
fn relocation(kind: Kind) -> Result<(u8, usize, usize), Unsupported> {
    Ok(match kind {
        Kind::Abs { width: 4 } => (R_386_32, 4, 0),
        Kind::Abs { width: 2 } => (R_386_16, 2, 0),
        Kind::Abs { width: 1 } => (R_386_8, 1, 0),
        Kind::PcRel { width: 4, from } => (R_386_PC32, 4, from),
        Kind::PcRel { width: 2, from } => (R_386_PC16, 2, from),
        Kind::PcRel { width: 1, from } => (R_386_PC8, 1, from),
        Kind::SegmentBase => return Err(unsupported("a segment selector has no ELF relocation")),
        Kind::FarPointer => return Err(unsupported("a 16:16 far pointer has no ELF relocation")),
        other => return Err(unsupported(format!("ELF i386 has no relocation for {other:?}"))),
    })
}

fn align(bytes: &mut Vec<u8>, to: usize) {
    while bytes.len() % to != 0 {
        bytes.push(0);
    }
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_le_bytes());
}

/// `object` as an ELF32 relocatable file.
pub fn write(object: &Object) -> Result<Vec<u8>, Unsupported> {
    if object.arch != Arch::I386 {
        return Err(unsupported(format!("{:?} has no ELF32 machine", object.arch)));
    }
    if object.debug.is_some() {
        return Err(unsupported("CodeView debug information is OMF's"));
    }
    if !object.omf_groups.is_empty() {
        return Err(unsupported("a group of segments is OMF's"));
    }

    // Section indices: 0 is the null section, the object's follow in order.
    let mut names = Strings::new();
    let spelled: Vec<(&str, u32, u32)> = object.sections.iter().map(spelling).collect::<Result<_, _>>()?;
    // A role's first section is `.text`, `.data`...; a further one is told apart by its own name.
    let mut section_names: Vec<String> = Vec::new();
    for (index, section) in object.sections.iter().enumerate() {
        let taken = spelled[..index].iter().any(|one| one.0 == spelled[index].0);
        section_names.push(if taken { format!("{}.{}", spelled[index].0, section.name) } else { spelled[index].0.to_owned() });
    }

    // Symbols: null, the source's file, one section symbol per section, then the globals.
    let mut strings = Strings::new();
    let mut symtab: Vec<u8> = vec![0; 16];
    let symbol = |out: &mut Vec<u8>, name: u32, value: u32, size: u32, info: u8, section: u16| {
        put32(out, name);
        put32(out, value);
        put32(out, size);
        out.push(info);
        out.push(0);
        put16(out, section);
    };
    let file = strings.add(&object.name);
    symbol(&mut symtab, file, 0, 0, STB_LOCAL << 4 | STT_FILE, SHN_ABS);
    for index in 0..object.sections.len() {
        symbol(&mut symtab, 0, 0, 0, STB_LOCAL << 4 | STT_SECTION, index as u16 + 1);
    }
    let first_global = 2 + object.sections.len();
    // The ELF symbol of each object symbol that is global, in order.
    let mut elf_symbol: Vec<Option<u32>> = vec![None; object.symbols.len()];
    let mut next = first_global as u32;
    for (index, one) in object.symbols.iter().enumerate() {
        if matches!((one.definition, one.binding), (Definition::Defined { .. }, Binding::Local)) {
            continue;
        }
        let name = strings.add(&one.name);
        match (one.definition, one.binding) {
            (Definition::Defined { section, offset }, Binding::Public) => {
                let kind = if object.sections[section].role == Role::Text { STT_FUNC } else { STT_OBJECT };
                symbol(&mut symtab, name, offset as u32, 0, STB_GLOBAL << 4 | kind, section as u16 + 1);
            }
            (Definition::Defined { .. }, Binding::Local) => unreachable!("skipped above"),
            (Definition::Undefined, _) => symbol(&mut symtab, name, 0, 0, STB_GLOBAL << 4, 0),
        }
        elf_symbol[index] = Some(next);
        next += 1;
    }

    // Each section's bytes with REL's addends in the fields, and its relocation entries.
    let mut images: Vec<Vec<u8>> = Vec::new();
    let mut relocs: Vec<Vec<u8>> = Vec::new();
    for section in &object.sections {
        let mut image = section.image.clone();
        let mut entries = Vec::new();
        for one in &section.relocs {
            let (kind, width, from) = relocation(one.kind)?;
            let (symbol, own) = match one.target {
                Target::OmfGroup(_) => return Err(unsupported("a reference to a group is OMF's")),
                Target::Symbol(index) => match (elf_symbol[index], object.symbols[index].definition) {
                    (Some(symbol), _) => (symbol, 0),
                    // A local symbol is its section's symbol and its offset.
                    (None, Definition::Defined { section, offset }) => (2 + section as u32, offset as i64),
                    (None, Definition::Undefined) => unreachable!("an undefined symbol is global"),
                },
            };
            // S + A - P, where the model's value is S + addend - (at + from).
            let addend = one.addend + own - from as i64;
            let fits = match width {
                4 => i32::try_from(addend).is_ok() || u32::try_from(addend).is_ok(),
                2 => i16::try_from(addend).is_ok() || u16::try_from(addend).is_ok(),
                _ => i8::try_from(addend).is_ok() || u8::try_from(addend).is_ok(),
            };
            if !fits {
                return Err(unsupported(format!("{}: an addend of {addend} does not fit its {width}-byte field", section.name)));
            }
            image[one.at..one.at + width].copy_from_slice(&addend.to_le_bytes()[..width]);
            put32(&mut entries, one.at as u32);
            put32(&mut entries, symbol << 8 | u32::from(kind));
        }
        images.push(image);
        relocs.push(entries);
    }

    // The file: header, section contents, then the section header table.
    let mut out = vec![0u8; 52];
    let mut headers = vec![Header { name: 0, kind: 0, flags: 0, offset: 0, size: 0, link: 0, info: 0, align: 0, entry: 0 }];
    for (index, section) in object.sections.iter().enumerate() {
        let (_, kind, flags) = spelled[index];
        let alignment = section.align.max(4).next_power_of_two();
        align(&mut out, alignment);
        let offset = out.len() as u32;
        if kind == SHT_PROGBITS {
            out.extend(&images[index]);
        }
        headers.push(Header { name: names.add(&section_names[index]), kind, flags, offset, size: images[index].len() as u32, link: 0, info: 0, align: alignment as u32, entry: 0 });
    }
    let symtab_index = 1 + object.sections.len() + relocs.iter().filter(|one| !one.is_empty()).count() + 1;
    for (index, entries) in relocs.iter().enumerate().filter(|(_, one)| !one.is_empty()) {
        align(&mut out, 4);
        let offset = out.len() as u32;
        out.extend(entries);
        let name = names.add(&format!(".rel{}", section_names[index]));
        headers.push(Header { name, kind: SHT_REL, flags: SHF_INFO_LINK, offset, size: entries.len() as u32, link: symtab_index as u32, info: index as u32 + 1, align: 4, entry: 8 });
    }
    // An empty .note.GNU-stack: the object does not need an executable stack.
    headers.push(Header { name: names.add(".note.GNU-stack"), kind: SHT_PROGBITS, flags: 0, offset: out.len() as u32, size: 0, link: 0, info: 0, align: 1, entry: 0 });
    align(&mut out, 4);
    let offset = out.len() as u32;
    out.extend(&symtab);
    assert_eq!(headers.len(), symtab_index, "the relocations name the symbol table by this index");
    headers.push(Header { name: names.add(".symtab"), kind: SHT_SYMTAB, flags: 0, offset, size: symtab.len() as u32, link: headers.len() as u32 + 1, info: first_global as u32, align: 4, entry: 16 });
    let offset = out.len() as u32;
    out.extend(&strings.0);
    headers.push(Header { name: names.add(".strtab"), kind: SHT_STRTAB, flags: 0, offset, size: strings.0.len() as u32, link: 0, info: 0, align: 1, entry: 0 });
    let name = names.add(".shstrtab");
    let offset = out.len() as u32;
    out.extend(&names.0);
    headers.push(Header { name, kind: SHT_STRTAB, flags: 0, offset, size: names.0.len() as u32, link: 0, info: 0, align: 1, entry: 0 });
    align(&mut out, 4);
    let table = out.len() as u32;
    for one in &headers {
        for field in [one.name, one.kind, one.flags, 0, one.offset, one.size, one.link, one.info, one.align, one.entry] {
            put32(&mut out, field);
        }
    }

    out[..16].copy_from_slice(&[0x7F, b'E', b'L', b'F', 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let mut header = Vec::new();
    put16(&mut header, 1); // ET_REL
    put16(&mut header, 3); // EM_386
    put32(&mut header, 1);
    put32(&mut header, 0); // entry
    put32(&mut header, 0); // program headers
    put32(&mut header, table);
    put32(&mut header, 0); // flags
    put16(&mut header, 52);
    put16(&mut header, 0);
    put16(&mut header, 0);
    put16(&mut header, 40);
    put16(&mut header, headers.len() as u16);
    put16(&mut header, headers.len() as u16 - 1);
    out[16..52].copy_from_slice(&header);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use llrm_object::{Reloc, Symbol};

    use super::*;

    fn word(bytes: &[u8], at: usize) -> u32 {
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
                let name = bytes[strings + word(bytes, at) as usize..].iter().take_while(|&&one| one != 0).map(|&one| one as char).collect();
                (name, word(bytes, at + 4), word(bytes, at + 16) as usize, word(bytes, at + 20) as usize, word(bytes, at + 24), word(bytes, at + 28))
            })
            .collect()
    }

    fn named<'a>(found: &'a [(String, u32, usize, usize, u32, u32)], name: &str) -> &'a (String, u32, usize, usize, u32, u32) {
        found.iter().find(|one| one.0 == name).unwrap_or_else(|| panic!("no section {name}: {found:?}"))
    }

    fn text(image: Vec<u8>, relocs: Vec<Reloc>) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: "A_TEXT".into(), role: Role::Text, near: true, align: 1, image, spans, relocs, lines: Vec::new() }
    }

    fn symbol(name: &str, binding: Binding, definition: Definition) -> Symbol {
        Symbol { name: name.into(), binding, definition, group: None }
    }

    fn object(sections: Vec<Section>, symbols: Vec<Symbol>) -> Object {
        Object { name: "a.c".into(), arch: Arch::I386, sections, symbols, omf_groups: Vec::new(), debug: None }
    }

    /// `call f`: REL puts S + A - P in the field, so the field is -4 where the call's addend is 0,
    /// and the entry is R_386_PC32 against `f`, the first global.
    #[test]
    fn a_call_is_r_386_pc32_with_minus_four_in_the_field() {
        let call = Reloc { at: 1, kind: Kind::PcRel { width: 4, from: 4 }, target: Target::Symbol(0), addend: 0 };
        let made = object(vec![text(vec![0xE8, 0, 0, 0, 0, 0xC3], vec![call])], vec![symbol("f", Binding::Public, Definition::Undefined)]);
        let bytes = write(&made).unwrap();
        let found = sections(&bytes);
        let code = named(&found, ".text");
        assert_eq!(&bytes[code.2..code.2 + 6], [0xE8, 0xFC, 0xFF, 0xFF, 0xFF, 0xC3]);
        let rel = named(&found, ".rel.text");
        assert_eq!((rel.1, rel.5), (SHT_REL, 1));
        assert_eq!((word(&bytes, rel.2), word(&bytes, rel.2 + 4)), (1, 3 << 8 | u32::from(R_386_PC32)));
    }

    /// A local symbol has no ELF symbol: a reference to it is its section's symbol and its offset.
    #[test]
    fn a_local_symbol_is_its_section_and_its_offset() {
        let data = Section { name: "_DATA".into(), role: Role::Data, near: true, align: 4, image: vec![0; 8], spans: vec![[0, 8]], relocs: vec![Reloc { at: 4, kind: Kind::Abs { width: 4 }, target: Target::Symbol(0), addend: 2 }], lines: Vec::new() };
        let made = object(vec![data], vec![symbol("loc", Binding::Local, Definition::Defined { section: 0, offset: 4 })]);
        let bytes = write(&made).unwrap();
        let found = sections(&bytes);
        let data = named(&found, ".data");
        assert_eq!(word(&bytes, data.2 + 4), 6);
        let rel = named(&found, ".rel.data");
        // symbol 2 is the first section's.
        assert_eq!(word(&bytes, rel.2 + 4), 2 << 8 | u32::from(R_386_32));
    }

    /// Bss stores nothing and a section with no relocations has no REL section.
    #[test]
    fn bss_is_nobits_and_an_unrelocated_section_has_no_rel() {
        let bss = Section { name: "_BSS".into(), role: Role::Bss, near: true, align: 4, image: vec![0; 64], spans: vec![], relocs: vec![], lines: Vec::new() };
        let bytes = write(&object(vec![bss], vec![])).unwrap();
        let found = sections(&bytes);
        assert_eq!((named(&found, ".bss").1, named(&found, ".bss").3), (SHT_NOBITS, 64));
        assert!(found.iter().all(|one| !one.0.starts_with(".rel")));
    }

    /// What ELF cannot say is refused, never written near.
    #[test]
    fn what_elf_cannot_say_is_refused() {
        let at = |kind| {
            let reloc = Reloc { at: 0, kind, target: Target::Symbol(0), addend: 0 };
            write(&object(vec![text(vec![0; 4], vec![reloc])], vec![symbol("f", Binding::Public, Definition::Undefined)]))
        };
        assert!(at(Kind::SegmentBase).is_err());
        assert!(at(Kind::FarPointer).is_err());
        let far = Section { near: false, role: Role::Data, ..text(vec![0], vec![]) };
        assert!(write(&object(vec![far], vec![])).unwrap_err().0.contains("selector"));
        let stack = Section { role: Role::Stack, ..text(vec![0], vec![]) };
        assert!(write(&object(vec![stack], vec![])).is_err());
        let mut real = object(vec![text(vec![0], vec![])], vec![]);
        real.arch = Arch::I8086;
        assert!(write(&real).is_err());
    }
}
