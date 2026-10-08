//! An [`Object`] as a COFF object file, the format link.exe, lld-link and wlink read. What a
//! machine adds is its COFF machine number and its relocation types: see [`Machine`].
//! `llrm-coff32` and `llrm-coff64` are the machines.

/// C13 gives each local the ranges its place holds over.
pub const LOCATION_RANGES: bool = true;

pub mod codeview;

use llrm_object::{Arch, Binding, Definition, Kind, Object, Role, Section, Target, Unsupported};

const SCN_CNT_CODE: u32 = 0x20;
const SCN_CNT_INITIALIZED_DATA: u32 = 0x40;
const SCN_CNT_UNINITIALIZED_DATA: u32 = 0x80;
const SCN_LNK_NRELOC_OVFL: u32 = 0x0100_0000;
const SCN_MEM_DISCARDABLE: u32 = 0x0200_0000;
const SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const SCN_MEM_READ: u32 = 0x4000_0000;
const SCN_MEM_WRITE: u32 = 0x8000_0000;
const SYM_CLASS_EXTERNAL: u8 = 2;
const SYM_CLASS_STATIC: u8 = 3;
const SYM_CLASS_FILE: u8 = 103;
const SYM_TYPE_FUNCTION: u16 = 0x20;
const SYM_ABSOLUTE: i16 = -1;
const SYM_DEBUG: i16 = -2;
const SYMBOL: usize = 18;

/// A relocation type, the bytes of the field it patches, and the distance from the field's start
/// to the place the linker takes a pc-relative value relative to (`baked`): `REL32` is the
/// field's end, `REL32_n` is `n` bytes past it.
pub struct Relocation {
    pub kind: u16,
    pub width: usize,
    pub from: usize,
    pub baked: usize,
}

/// What a machine says of its COFF.
pub trait Machine {
    const ARCH: Arch;
    /// `IMAGE_FILE_MACHINE_*`.
    const NUMBER: u16;
    /// `@feat.00` bit 0: no exception handler is left unregistered. A 32-bit link with /safeseh
    /// (its default) refuses an object that does not say so; x86-64 has no such table.
    const SAFE_SEH: bool = false;

    fn relocation(kind: Kind) -> Result<Relocation, Unsupported>;
}

fn unsupported(text: impl Into<String>) -> Unsupported {
    Unsupported(text.into())
}

/// The string table: its length, then NUL-terminated names. An offset counts the length.
struct Strings(Vec<u8>);

impl Strings {
    fn add(&mut self, text: &str) -> u32 {
        let at = (self.0.len() + 4) as u32;
        self.0.extend(text.as_bytes());
        self.0.push(0);
        at
    }
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_le_bytes());
}

/// An 8-byte name field: the name itself, or `/offset` into the string table, or for a symbol
/// four zero bytes and the offset.
fn section_name(name: &str, strings: &mut Strings) -> [u8; 8] {
    let mut field = [0u8; 8];
    if name.len() <= 8 {
        field[..name.len()].copy_from_slice(name.as_bytes());
    } else {
        let text = format!("/{}", strings.add(name));
        field[..text.len()].copy_from_slice(text.as_bytes());
    }
    field
}

fn symbol_name(name: &str, strings: &mut Strings) -> [u8; 8] {
    let mut field = [0u8; 8];
    if name.len() <= 8 {
        field[..name.len()].copy_from_slice(name.as_bytes());
    } else {
        field[4..].copy_from_slice(&strings.add(name).to_le_bytes());
    }
    field
}

struct Entry {
    name: [u8; 8],
    value: u32,
    section: i16,
    kind: u16,
    class: u8,
    aux: u8,
}

fn symbol(out: &mut Vec<u8>, entry: Entry) {
    out.extend(entry.name);
    put32(out, entry.value);
    put16(out, entry.section as u16);
    put16(out, entry.kind);
    out.push(entry.class);
    out.push(entry.aux);
}

/// The COFF name, flags and alignment of `section`; a role's first section is `.text`, `.data`...
/// and a further one is `.text$name`, which the linker merges into the first.
fn spelling(section: &Section, taken: bool) -> Result<(String, u32), Unsupported> {
    if !section.near {
        return Err(unsupported(format!("{}: a segment addressed by its own selector has no COFF section", section.name)));
    }
    let (base, flags) = match section.role {
        Role::Text => (".text", SCN_CNT_CODE | SCN_MEM_EXECUTE | SCN_MEM_READ),
        Role::ROData => (".rdata", SCN_CNT_INITIALIZED_DATA | SCN_MEM_READ),
        Role::Data => (".data", SCN_CNT_INITIALIZED_DATA | SCN_MEM_READ | SCN_MEM_WRITE),
        Role::Bss => (".bss", SCN_CNT_UNINITIALIZED_DATA | SCN_MEM_READ | SCN_MEM_WRITE),
        Role::Stack => return Err(unsupported(format!("{}: an OMF stack segment has no COFF section", section.name))),
        Role::Debug => (section.name.as_str(), SCN_CNT_INITIALIZED_DATA | SCN_MEM_READ | SCN_MEM_DISCARDABLE),
    };
    if !section.align.is_power_of_two() || section.align > 8192 {
        return Err(unsupported(format!("{}: COFF aligns a section to a power of two up to 8192, not {}", section.name, section.align)));
    }
    let align = (section.align.trailing_zeros() + 1) << 20;
    let name = if taken && section.role != Role::Debug { format!("{base}${}", section.name) } else { base.to_owned() };
    Ok((name, flags | align))
}

fn pad(bytes: &mut Vec<u8>, to: usize) {
    while bytes.len() % to != 0 {
        bytes.push(0);
    }
}

/// `object` as a COFF object file of machine `M`, its debug information, if any, as C13.
pub fn write<M: Machine>(object: &Object) -> Result<Vec<u8>, Unsupported> {
    let expanded;
    let object = match &object.debug {
        Some(info) => {
            let encoded = codeview::encode(object, info)?;
            let section = |name: &str, one: codeview::Section| Section { name: name.to_owned(), role: Role::Debug, near: true, align: 1, spans: vec![[0, one.image.len()]], image: one.image, relocs: one.relocs };
            let mut sections = object.sections.clone();
            sections.push(section(".debug$S", encoded.symbols));
            sections.push(section(".debug$T", encoded.types));
            expanded = Object { sections, debug: None, ..object.clone() };
            &expanded
        }
        None => object,
    };
    if object.arch != M::ARCH {
        return Err(unsupported(format!("{:?} is not the {:?} this COFF writer is for", object.arch, M::ARCH)));
    }
    if !object.omf_groups.is_empty() {
        return Err(unsupported("a group of segments is OMF's"));
    }
    if object.sections.len() > i16::MAX as usize {
        return Err(unsupported("COFF numbers its sections in 15 bits"));
    }

    let mut strings = Strings(Vec::new());
    let mut spelled: Vec<(String, u32)> = Vec::new();
    for (index, section) in object.sections.iter().enumerate() {
        let taken = object.sections[..index].iter().any(|other| other.role == section.role);
        spelled.push(spelling(section, taken)?);
    }

    // Symbols: `.file`, one symbol per section, then the object's, each local one as a static.
    let mut table: Vec<u8> = Vec::new();
    let mut count = 0u32;
    let file = object.name.as_bytes();
    let aux = file.len().div_ceil(SYMBOL).max(1);
    symbol(&mut table, Entry { name: *b".file\0\0\0", value: 0, section: SYM_DEBUG, kind: 0, class: SYM_CLASS_FILE, aux: aux as u8 });
    let mut name = file.to_vec();
    name.resize(aux * SYMBOL, 0);
    table.extend(name);
    count += 1 + aux as u32;
    if M::SAFE_SEH {
        symbol(&mut table, Entry { name: *b"@feat.00", value: 1, section: SYM_ABSOLUTE, kind: 0, class: SYM_CLASS_STATIC, aux: 0 });
        count += 1;
    }
    count += 2 * object.sections.len() as u32;
    // Each object symbol's index in the table.
    let coff_symbol: Vec<u32> = (count..count + object.symbols.len() as u32).collect();
    count += object.symbols.len() as u32;

    // Sections' bytes and relocation entries.
    let mut images: Vec<Vec<u8>> = Vec::new();
    let mut relocs: Vec<Vec<u8>> = Vec::new();
    let mut reloc_counts: Vec<usize> = Vec::new();
    for section in &object.sections {
        let mut image = section.image.clone();
        let mut entries = Vec::new();
        for one in &section.relocs {
            let Relocation { kind, width, from, baked } = M::relocation(one.kind)?;
            let Target::Symbol(target) = one.target else { return Err(unsupported("a reference to a group is OMF's")) };
            // The linker adds S + field (- P - baked, if relative); the model wants S + addend - (at + from).
            let field = if one.kind.relative() { one.addend - (from as i64 - baked as i64) } else { one.addend };
            let fits = match width {
                8 => true,
                4 => i32::try_from(field).is_ok() || (!one.kind.relative() && u32::try_from(field).is_ok()),
                _ => i16::try_from(field).is_ok() || (!one.kind.relative() && u16::try_from(field).is_ok()),
            };
            if !fits {
                return Err(unsupported(format!("{}: a value of {field} does not fit its {width}-byte field", section.name)));
            }
            put32(&mut entries, one.at as u32);
            put32(&mut entries, coff_symbol[target]);
            put16(&mut entries, kind);
            image[one.at..one.at + width].copy_from_slice(&field.to_le_bytes()[..width]);
        }
        reloc_counts.push(section.relocs.len());
        images.push(image);
        relocs.push(entries);
    }

    for (index, section) in object.sections.iter().enumerate() {
        let name = symbol_name(&spelled[index].0, &mut strings);
        symbol(&mut table, Entry { name, value: 0, section: index as i16 + 1, kind: 0, class: SYM_CLASS_STATIC, aux: 1 });
        put32(&mut table, section.image.len() as u32);
        put16(&mut table, reloc_counts[index].min(0xFFFF) as u16);
        put16(&mut table, 0);
        put32(&mut table, 0);
        put16(&mut table, 0);
        table.extend([0u8; 4]);
    }
    for one in &object.symbols {
        let name = symbol_name(&one.name, &mut strings);
        let class = if one.binding == Binding::Public { SYM_CLASS_EXTERNAL } else { SYM_CLASS_STATIC };
        match one.definition {
            Definition::Defined { section, offset } => {
                let kind = if object.sections[section].role == Role::Text { SYM_TYPE_FUNCTION } else { 0 };
                symbol(&mut table, Entry { name, value: offset as u32, section: section as i16 + 1, kind, class, aux: 0 });
            }
            Definition::Undefined => symbol(&mut table, Entry { name, value: 0, section: 0, kind: 0, class: SYM_CLASS_EXTERNAL, aux: 0 }),
        }
    }

    // The file: header, section headers, then the contents, relocations, symbols and strings.
    let mut body: Vec<u8> = Vec::new();
    let base = 20 + 40 * object.sections.len();
    let mut headers: Vec<u8> = Vec::new();
    for (index, section) in object.sections.iter().enumerate() {
        pad(&mut body, 4);
        let data = (section.role != Role::Bss && !images[index].is_empty()).then(|| (base + body.len()) as u32);
        if data.is_some() {
            body.extend(&images[index]);
        }
        pad(&mut body, 4);
        let overflow = reloc_counts[index] > 0xFFFF;
        let at = (reloc_counts[index] > 0).then(|| (base + body.len()) as u32);
        if overflow {
            put32(&mut body, reloc_counts[index] as u32 + 1);
            put32(&mut body, 0);
            put16(&mut body, 0);
        }
        body.extend(&relocs[index]);
        headers.extend(section_name(&spelled[index].0, &mut strings));
        put32(&mut headers, 0);
        put32(&mut headers, 0);
        put32(&mut headers, images[index].len() as u32);
        put32(&mut headers, data.unwrap_or(0));
        put32(&mut headers, at.unwrap_or(0));
        put32(&mut headers, 0);
        put16(&mut headers, reloc_counts[index].min(0xFFFF) as u16);
        put16(&mut headers, 0);
        put32(&mut headers, spelled[index].1 | if overflow { SCN_LNK_NRELOC_OVFL } else { 0 });
    }
    pad(&mut body, 4);
    let symbols_at = base + body.len();
    body.extend(&table);
    put32(&mut body, strings.0.len() as u32 + 4);
    body.extend(&strings.0);

    let mut out = Vec::with_capacity(base + body.len());
    put16(&mut out, M::NUMBER);
    put16(&mut out, object.sections.len() as u16);
    put32(&mut out, 0); // a reproducible object has no timestamp
    put32(&mut out, symbols_at as u32);
    put32(&mut out, count);
    put16(&mut out, 0);
    put16(&mut out, 0);
    out.extend(headers);
    out.extend(body);
    Ok(out)
}
