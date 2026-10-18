//! An [`Object`] as OMF records: LNAMES, SEGDEF, GRPDEF, EXTDEF, PUBDEF, LEDATA
//! and FIXUPP, and LINNUM and a CodeView marker where it carries debug
//! information.

use std::borrow::Cow;
use std::fmt;
use std::rc::Rc;

use llrm_object::{Definition, Kind, Object, Role, Section, Target};

use crate::omf;

/// A relocated field's location code.
const OFFSET: u8 = 1;
const BASE: u8 = 2;
const POINTER: u8 = 3;
const OFFSET32: u8 = 9;

/// LEDATA payload per record. A fixup's offset into its record has ten bits.
const CHUNK: usize = 1000;
/// relocatable, word aligned, public, 16-bit
const ACBP: u8 = 0x48;
/// relocatable, byte aligned, public: a DWARF section, whose modules the linker
/// joins end to end with no padding between them
const BYTE: u8 = 0x28;
/// relocatable, paragraph aligned, public, 16-bit
const PARAGRAPH: u8 = 0x68;
/// relocatable, paragraph aligned, stack, 16-bit
const STACK_SEGMENT: u8 = 0x74;
/// relocatable, dword aligned, public, 16-bit
const DWORD: u8 = 0xA8;
const SEGMENT_TARGET: u8 = 0;
const GROUP_TARGET: u8 = 1;
const EXTERNAL_TARGET: u8 = 2;
const GROUP_FRAME: u8 = 1;
const TARGET_FRAME: u8 = 5;
/// COMDAT, and the symbol-relative line numbers (LINSYM) that go with it, each
/// with its 32-bit twin.
const COMDAT: u8 = 0xC2;
const LINSYM: u8 = 0xC4;
/// A COMDAT's attributes: allocated explicitly in the segment named, no
/// duplicate allowed.
const COMDAT_EXPLICIT: u8 = 0;
/// The COMDAT flag bit: this data continues the one before it.
const COMDAT_CONTINUATION: u8 = 1;

/// The sections written as COMDATs: each is named by its symbol at offset 0,
/// which a fixup reaches as an external (the way MSC's /Gy objects do it), the
/// linker finding the COMDAT that defines it.
struct Comdats {
    /// Section index to the symbol that names it.
    main: std::collections::BTreeMap<usize, usize>,
    /// Section index to the name the COMDAT is written under: the symbol's, or
    /// for a local one (a linker finds a local COMDAT through no external)
    /// that name made the module's own.
    names: std::collections::BTreeMap<usize, String>,
    /// Section index to the index of its EXTDEF, where a fixup refers to it.
    extdef: std::collections::BTreeMap<usize, usize>,
}

impl Comdats {
    fn of(
        object: &Object,
        undefined: usize,
    ) -> Result<Self, Error> {
        let mut main = std::collections::BTreeMap::new();
        for &section in &object.omf_comdat {
            let named = object
                .symbols
                .iter()
                .position(
                    |symbol| matches!(
                        symbol.definition,
                        Definition::Defined { section: at, offset: 0 } if at == section
                    ),
                );
            let Some(named) = named else {
                return Err(unencodable(format!(
                    "{}: a COMDAT section with no symbol at its start",
                    object.sections[section].name
                )));
            };
            main.insert(section, named);
        }
        // The module's key: its name and everything it holds (not the debug
        // sections: `-g` changes no name), so two modules of one name,
        // as two `m.nib` of different directories, do not meet in a local
        // function's name.
        let mut own = 0xcbf2_9ce4_8422_2325_u64;
        let mut feed = |bytes: &[u8]| {
            for &byte in bytes {
                own = (own ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
            // A separator, so that two halves cannot trade bytes.
            own = (own ^ 0xff).wrapping_mul(0x0000_0100_0000_01b3);
        };
        feed(object.name.as_bytes());
        for section in object.sections.iter().filter(|section| section.role != Role::Debug) {
            feed(section.name.as_bytes());
            feed(&section.image);
            for reloc in &section.relocs {
                feed(&reloc.at.to_le_bytes());
                feed(format!("{:?}{}", reloc.target, reloc.addend).as_bytes());
            }
        }
        for symbol in &object.symbols {
            feed(symbol.name.as_bytes());
            feed(format!("{:?}", symbol.definition).as_bytes());
        }
        let names = main
            .iter()
            .map(|(&section, &symbol)| {
                let one = &object.symbols[symbol];
                let name = if one.binding == llrm_object::Binding::Public {
                    one.name.clone()
                } else {
                    format!("{}${own:016X}", one.name)
                };
                (section, name)
            })
            .collect();
        let reached = |target: &Target| match *target {
            Target::Section(section) => Some(section),
            Target::Symbol(symbol) => match object.symbols[symbol].definition {
                Definition::Defined { section, .. } => Some(section),
                Definition::Undefined => None,
            },
            Target::OmfGroup(_) => None,
        };
        let referenced: std::collections::BTreeSet<usize> = object
            .sections
            .iter()
            .flat_map(|section| section.relocs.iter())
            .filter_map(|reloc| reached(&reloc.target))
            .filter(|section| main.contains_key(section))
            .collect();
        let extdef = referenced.into_iter().enumerate().map(|(at, section)| (section, undefined + at)).collect();
        Ok(Comdats { main, names, extdef })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// The object says something OMF cannot, or something that does not fit a
    /// record.
    Unencodable(String),
    Value(omf::ValueError),
}

impl fmt::Display for Error {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Error::Unencodable(text) => formatter.write_str(text),
            Error::Value(one) => formatter.write_str(&one.0),
        }
    }
}

impl std::error::Error for Error {}

impl From<omf::ValueError> for Error {
    fn from(one: omf::ValueError) -> Self {
        Error::Value(one)
    }
}

fn unencodable(text: impl Into<String>) -> Error {
    Error::Unencodable(text.into())
}

/// The segment alignment code a segment whose widest item asks for `to` bytes
/// needs.
fn alignment_for(to: usize) -> u8 {
    match to {
        0..=2 => ACBP,
        3..=4 => DWORD,
        _ => PARAGRAPH,
    }
}

fn class(section: &Section) -> &'static str {
    match (section.role, section.near) {
        (Role::Text, _) => "CODE",
        (Role::ROData, _) => "CONST",
        (Role::Data, true) => "DATA",
        (Role::Data, false) => "FAR_DATA",
        (Role::Bss, true) => "BSS",
        (Role::Bss, false) => "FAR_BSS",
        (Role::Stack, _) => "STACK",
        (Role::Debug, _) if section.name == crate::codeview::SYMBOLS => "DEBSYM",
        (Role::Debug, _) if section.name.starts_with(".debug_") => "DWARF",
        (Role::Debug, _) => "DEBTYP",
    }
}

/// The location code of a field, and whether it is self-relative.
fn location(kind: Kind) -> Result<(u8, bool), Error> {
    Ok(match kind {
        Kind::Abs { width: 2 } => (OFFSET, false),
        Kind::Abs { width: 4 } => (OFFSET32, false),
        Kind::SegmentBase => (BASE, false),
        Kind::FarPointer => (POINTER, false),
        Kind::PcRel { width, from } if width == from && matches!(width, 2 | 4) => {
            (if width == 2 { OFFSET } else { OFFSET32 }, true)
        }
        Kind::Branch { width: 2 } => (OFFSET, true),
        Kind::Branch { width: 4 } => (OFFSET32, true),
        other => return Err(unencodable(format!("OMF has no fixup for {other:?}"))),
    })
}

/// Bytes of a field that hold a value: a far pointer's offset is its first two.
fn packed(kind: Kind) -> usize {
    match kind {
        Kind::Abs { width: 4 } | Kind::PcRel { width: 4, .. } | Kind::Branch { width: 4 } => 4,
        _ => 2,
    }
}

/// `value` into the field of `width` bytes at `at`, wrapped to it.
fn pack_field(
    buffer: &mut [u8],
    at: usize,
    width: usize,
    value: i64,
) {
    if width == 4 {
        buffer[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
    } else {
        buffer[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
    }
}

/// Everything of a fixup after its location: fix data, frame datum, target
/// datum.
struct Resolved {
    subrecord: Vec<u8>,
    /// What the field holds once the target's own offset is added in.
    value: i64,
}

fn resolved(
    object: &Object,
    extern_index: &[usize],
    comdats: &Comdats,
    section: &Section,
    at: usize,
) -> Result<Resolved, Error> {
    let reloc = section.relocs.iter().find(|one| one.at == at).expect("the caller names a fixup");
    let (loc, relative) = location(reloc.kind)?;
    let in_group = |section: usize| object.omf_groups.iter().position(|group| group.members.contains(&section));
    let (method, datum, group) = match reloc.target {
        Target::OmfGroup(group) => (GROUP_TARGET, group as i64 + 1, Some(group)),
        Target::Section(section) if comdats.extdef.contains_key(&section) => {
            (EXTERNAL_TARGET, comdats.extdef[&section] as i64 + 1, None)
        }
        Target::Section(section) => (SEGMENT_TARGET, section as i64 + 1, in_group(section)),
        Target::Symbol(symbol) => match (&object.symbols[symbol], object.symbols[symbol].definition) {
            (_, Definition::Defined { section, .. }) if comdats.extdef.contains_key(&section) => {
                (EXTERNAL_TARGET, comdats.extdef[&section] as i64 + 1, None)
            }
            (_, Definition::Defined { section, .. }) => (SEGMENT_TARGET, section as i64 + 1, in_group(section)),
            (named, Definition::Undefined) => (EXTERNAL_TARGET, extern_index[symbol] as i64 + 1, named.group),
        },
    };
    let local = match reloc.target {
        Target::Symbol(symbol) => match object.symbols[symbol].definition {
            Definition::Defined { offset, .. } => Some(offset as i64),
            Definition::Undefined => None,
        },
        Target::OmfGroup(_) | Target::Section(_) => None,
    };
    let own = if matches!(loc, OFFSET | POINTER | OFFSET32) && !relative { local.unwrap_or(0) } else { 0 };
    // A debug record's address is the place a debugger reads: the segment index
    // of the group the data is addressed through, and the offset in it. A
    // linker that writes the index of the fixup's frame (jwlink's
    // `FindGroupIdx`) finds none for a frame that is the segment itself
    // where the segment is not the group's first, and writes 0.
    let through_group =
        matches!(loc, OFFSET | OFFSET32) && !relative || section.role == Role::Debug && matches!(loc, POINTER | BASE);
    let subrecord = match group {
        Some(group) if through_group => {
            [vec![GROUP_FRAME << 4 | 4 | method], omf::as_index(group as i64 + 1)?, omf::as_index(datum)?].concat()
        }
        _ => [vec![TARGET_FRAME << 4 | 4 | method], omf::as_index(datum)?].concat(),
    };
    Ok(Resolved { subrecord, value: reloc.addend + own })
}

fn names(indices: &[i64]) -> Result<Vec<u8>, omf::ValueError> {
    Ok(indices.iter().map(|one| omf::as_index(*one)).collect::<Result<Vec<_>, _>>()?.concat())
}

/// A counted string, as the records spell names.
fn string(text: &str) -> Vec<u8> {
    let encoded: Vec<u8> = text
        .chars()
        .map(|one| u8::try_from(u32::from(one)).unwrap_or_else(|_| panic!("UnicodeEncodeError: 'latin-1' codec")))
        .collect();
    let length = u8::try_from(encoded.len()).unwrap_or_else(|_| panic!("ValueError: bytes must be in range(0, 256)"));
    [vec![length], encoded].concat()
}

/// Segment `index`'s LINNUM records: no base group, then (line, offset) pairs.
fn linnum(
    index: usize,
    lines: &[(u32, usize)],
) -> Result<Vec<Rc<omf::Record>>, Error> {
    let mut head = vec![0];
    head.extend(omf::as_index(index as i64)?);
    lines
        .chunks(CHUNK / 4)
        .map(|chunk| {
            let mut body = head.clone();
            for &(line, at) in chunk {
                let (Ok(line), Ok(at)) = (u16::try_from(line), u16::try_from(at)) else {
                    return Err(unencodable(format!("line {line} at {at:#x} does not fit LINNUM")));
                };
                body.extend(line.to_le_bytes());
                body.extend(at.to_le_bytes());
            }
            Ok(Rc::new(omf::Record::new(omf::LINNUM, body)))
        })
        .collect()
}

/// A COMDAT record: `payload` is the bytes of the function from `offset` on; a
/// record after the first of a function continues it.
fn comdat_record(
    bits: u32,
    name: i64,
    flags: u8,
    group: i64,
    segment: i64,
    offset: usize,
    payload: &[u8],
) -> Result<Rc<omf::Record>, Error> {
    // Alignment 0: the segment's own. Type index 0: none.
    let mut body = vec![flags, COMDAT_EXPLICIT, 0];
    if bits == 32 {
        body.extend((offset as u32).to_le_bytes());
    } else {
        body.extend(u16::try_from(offset).map_err(|_| unencodable("a COMDAT past 64K"))?.to_le_bytes());
    }
    body.extend(omf::as_index(0)?);
    body.extend(omf::as_index(group)?);
    body.extend(omf::as_index(segment)?);
    body.extend(omf::as_index(name)?);
    body.extend_from_slice(payload);
    Ok(Rc::new(omf::Record::new(if bits == 32 { COMDAT + 1 } else { COMDAT }, body)))
}

/// Where a COMDAT's data is named: the LNAMES index of its symbol.
struct Named {
    name: i64,
}

/// A COMDAT's line numbers: LINSYM, which names the COMDAT's symbol where
/// LINNUM names a segment.
fn linsym(
    bits: u32,
    named: &Named,
    lines: &[(u32, usize)],
) -> Result<Vec<Rc<omf::Record>>, Error> {
    let width = if bits == 32 { 4 } else { 2 };
    lines
        .chunks(CHUNK / (2 + width))
        .enumerate()
        .map(|(number, chunk)| {
            // Flags: a record after the first continues it.
            let mut body = vec![u8::from(number > 0)];
            body.extend(omf::as_index(named.name)?);
            for &(line, at) in chunk {
                let Ok(line) = u16::try_from(line) else {
                    return Err(unencodable(format!("line {line} at {at:#x} does not fit LINSYM")));
                };
                body.extend(line.to_le_bytes());
                if bits == 32 {
                    body.extend((at as u32).to_le_bytes());
                } else {
                    body.extend(
                        u16::try_from(at)
                            .map_err(|_| unencodable(format!("line {line} at {at:#x} does not fit LINSYM")))?
                            .to_le_bytes(),
                    );
                }
            }
            Ok(Rc::new(omf::Record::new(if bits == 32 { LINSYM + 1 } else { LINSYM }, body)))
        })
        .collect()
}

/// Section `index`'s LEDATA and FIXUPP records, or its COMDAT and FIXUPP
/// records where `named`.
fn ledata(
    object: &Object,
    extern_index: &[usize],
    comdats: &Comdats,
    named: Option<&Named>,
    index: usize,
) -> Result<Vec<Rc<omf::Record>>, Error> {
    let bits = object.arch.bits();
    let section = &object.sections[index];
    let mut at_sorted: Vec<usize> = section.relocs.iter().map(|one| one.at).collect();
    at_sorted.sort_unstable();
    let mut image = section.image.clone();
    let mut subrecords = std::collections::BTreeMap::new();
    for &at in &at_sorted {
        let made = resolved(object, extern_index, comdats, section, at)?;
        let kind = section.relocs.iter().find(|one| one.at == at).expect("sorted from them").kind;
        if kind == Kind::SegmentBase && made.value != 0 {
            return Err(unencodable(format!("{}: a segment base with an addend", section.name)));
        }
        pack_field(&mut image, at, packed(kind), made.value);
        subrecords.insert(at, (made.subrecord, kind));
    }
    let (mut out, mut placed) = (Vec::new(), 0);
    let group =
        object.omf_groups.iter().position(|group| group.members.contains(&index)).map_or(0, |one| one as i64 + 1);
    for &[mut start, end] in &section.spans {
        while start < end {
            let mut stop = end.min(start + CHUNK);
            for &at in &at_sorted {
                if at < stop && stop < at + subrecords[&at].1.width() {
                    stop = at;
                }
            }
            let payload = &image[start..stop];
            out.push(if let Some(named) = named {
                let flags = if out.is_empty() { 0 } else { COMDAT_CONTINUATION };
                comdat_record(bits, named.name, flags, group, index as i64 + 1, start, payload)?
            } else if bits == 32 {
                omf::ledata_record32(index as i64 + 1, start as i64, payload)?
            } else {
                omf::ledata_record(index as i64 + 1, start as i64, payload)?
            });
            let located: Vec<Vec<u8>> = at_sorted
                .iter()
                .filter(|&&at| start <= at && at < stop)
                .map(|&at| {
                    let (subrecord, kind) = &subrecords[&at];
                    let (loc, relative) = location(*kind).expect("resolved above");
                    let offset = at - start;
                    let lead = 0x80 | if relative { 0 } else { 0x40 } | (loc as usize) << 2 | offset >> 8;
                    [&[lead as u8, (offset & 0xFF) as u8][..], subrecord].concat()
                })
                .collect();
            if !located.is_empty() {
                out.push(if bits == 32 { omf::fixupp_record32(&located) } else { omf::fixupp_record(&located) });
            }
            placed += located.len();
            start = stop;
        }
    }
    if placed != section.relocs.len() || subrecords.len() != section.relocs.len() {
        return Err(unencodable(format!("{}: a fixup outside the data, or two in one field", section.name)));
    }
    Ok(out)
}

/// `object` as an OMF object file.
pub fn write(object: &Object) -> Result<Vec<u8>, Error> {
    let turbo = matches!(
        &object.debug,
        Some(info) if info.format == llrm_object::debug::Format::TurboDebugger
    );
    let dwarf = matches!(
        &object.debug,
        Some(info) if matches!(info.format, llrm_object::debug::Format::Dwarf { .. })
    );
    let debug = object.debug.is_some() && !turbo && !dwarf;
    // Turbo Debugger's records go among the object's own; CodeView's two
    // segments are this writer's, made from the object's debug information.
    let td = match &object.debug {
        Some(info) if turbo => Some(crate::td::records(object, info)?),
        _ => None,
    };
    let (object, lines): (Cow<Object>, Vec<Vec<(u32, usize)>>) = match &object.debug {
        None => (Cow::Borrowed(object), Vec::new()),
        Some(info) if turbo => {
            (Cow::Owned(Object { debug: None, ..object.clone() }), crate::codeview::lines(object, info)?)
        }
        // DWARF's sections are the DWARF writer's: the same ones an ELF object
        // holds, here segments of the class Open Watcom's -hd writes, which the
        // linker joins and relocates.
        Some(info) if dwarf => {
            let mut lines = crate::codeview::lines(object, info)?;
            let expanded = llrm_dwarf::expanded(object, info).map_err(|error| unencodable(error.0))?;

            lines.resize(expanded.sections.len(), Vec::new());
            (Cow::Owned(expanded), lines)
        }
        Some(info) => {
            let mut lines = crate::codeview::lines(object, info)?;
            let described = crate::codeview::sections(object, info)?;
            let mut expanded = Object { debug: None, ..object.clone() };
            expanded.sections.extend(described);
            lines.resize(expanded.sections.len(), Vec::new());
            (Cow::Owned(expanded), lines)
        }
    };
    let object = &*object;
    let bits = object.arch.bits();
    if !matches!(bits, 16 | 32) {
        return Err(unencodable(format!("OMF has no {bits}-bit records")));
    }
    let mut lnames: Vec<String> = vec![String::new()];
    let mut lname = |text: &str| -> i64 {
        lnames.push(text.to_owned());
        lnames.len() as i64
    };
    let selector = |section: &Section| !section.near && matches!(section.role, Role::Data | Role::Bss | Role::ROData);
    let mut segdefs = Vec::new();
    for section in &object.sections {
        let (klass, name) = (lname(class(section)), lname(&section.name));
        // A COMDAT's bytes are the linker's to place: its segment here is the
        // empty one it is placed in.
        let size = if object.omf_comdat.contains(&segdefs.len()) { 0 } else { section.image.len() };
        // A USE32 segment is dword aligned at least: its offsets are 32-bit.
        let alignment = if selector(section) {
            PARAGRAPH
        } else {
            alignment_for(if bits == 32 { section.align.max(4) } else { section.align })
        };
        let alignment = if section.role == Role::Stack { STACK_SEGMENT } else { alignment };
        let alignment = if class(section) == "DWARF" { BYTE } else { alignment };
        let (acbp, record) = if bits == 32 {
            (alignment | 1, omf::SEGDEF + 1)
        } else {
            (alignment | if size == 0x10000 { 2 } else { 0 }, omf::SEGDEF)
        };
        let mut body = vec![acbp];
        if bits == 32 {
            body.extend((size as u32).to_le_bytes());
        } else {
            body.extend(((size & 0xFFFF) as u16).to_le_bytes());
        }
        body.extend(names(&[name, klass, 1])?);
        segdefs.push(Rc::new(omf::Record::new(record, body)));
    }
    // LNAMES names DGROUP where no group is written, as it always has.
    let mut grpdefs = Vec::new();
    for group in &object.omf_groups {
        let mut body = names(&[lname(&group.name)])?;
        for &member in &group.members {
            body.push(0xFF);
            body.extend(omf::as_index(member as i64 + 1)?);
        }
        grpdefs.push(Rc::new(omf::Record::new(omf::GRPDEF, body)));
    }
    if object.omf_groups.is_empty() {
        lname("DGROUP");
    }
    let extern_index: Vec<usize> = object
        .symbols
        .iter()
        .scan(0, |next, symbol| {
            let index = *next;
            if symbol.definition == Definition::Undefined {
                *next += 1;
            }
            Some(index)
        })
        .collect();
    let undefined = object.symbols.iter().filter(|symbol| symbol.definition == Definition::Undefined).count();
    let comdats = Comdats::of(object, undefined)?;
    let mut named_comdats: std::collections::BTreeMap<usize, Named> = std::collections::BTreeMap::new();
    for (&section, name) in &comdats.names {
        named_comdats.insert(section, Named { name: lname(name) });
    }
    let mut data = Vec::new();
    for index in 0..object.sections.len() {
        data.extend(ledata(object, &extern_index, &comdats, named_comdats.get(&index), index)?);
    }

    let mut records = vec![Rc::new(omf::Record::new(omf::THEADR, string(&object.name)))];
    records.extend(td.iter().flat_map(|one| one.before.iter().cloned()));
    records.push(Rc::new(omf::Record::new(omf::LNAMES, lnames.iter().flat_map(|one| string(one)).collect())));
    if dwarf && bits == 32 {
        // The linker's directive "flat addresses": the debug sections'
        // addresses are the image's (Open Watcom's -hd objects carry
        // it).
        records.push(Rc::new(omf::Record::new(omf::COMENT, vec![0x80, 0xFE, b'F'])));
    }
    if debug {
        // CodeView 4's marker: LINK /CO reads the debug information after it.
        records.push(Rc::new(omf::Record::new(omf::COMENT, vec![0x00, 0xA1, 0x01, b'C', b'V'])));
    }
    records.extend(segdefs);
    records.extend(grpdefs);
    let declared: Vec<&llrm_object::Symbol> =
        object.symbols.iter().filter(|symbol| symbol.definition == Definition::Undefined).collect();
    // The COMDATs a fixup reaches are declared as externals, in the order their
    // indices were given.
    let mut names: Vec<&str> = declared.iter().map(|symbol| symbol.name.as_str()).collect();
    names.extend(comdats.extdef.keys().map(|section| comdats.names[section].as_str()));
    if !names.is_empty() {
        let body = names.iter().flat_map(|name| [string(name), vec![0]].concat()).collect();
        records.push(Rc::new(omf::Record::new(omf::EXTDEF, body)));
    }
    for index in 0..object.sections.len() {
        // A COMDAT's symbol is defined by the COMDAT itself.
        if comdats.main.contains_key(&index) {
            if let Some(inside) = object.symbols.iter().find(|symbol| {
                symbol.binding == llrm_object::Binding::Public
                    && matches!(
                        symbol.definition,
                        Definition::Defined { section, offset } if section == index && offset != 0
                    )
            }) {
                return Err(unencodable(format!("{}: a public symbol inside a COMDAT", inside.name)));
            }
            continue;
        }
        let defined: Vec<(usize, &str, usize)> = object
            .symbols
            .iter()
            .enumerate()
            .filter_map(|(at, symbol)| match symbol.definition {
                Definition::Defined { section, offset }
                    if section == index && symbol.binding == llrm_object::Binding::Public =>
                {
                    Some((at, symbol.name.as_str(), offset))
                }
                _ => None,
            })
            .collect();
        if defined.is_empty() {
            continue;
        }
        let group = object.omf_groups.iter().position(|group| group.members.contains(&index));
        let mut head = omf::as_index(group.map_or(0, |one| one as i64 + 1))?;
        head.extend(omf::as_index(index as i64 + 1)?);
        // One PUBDEF a symbol where Turbo Debugger's type record follows it.
        let mut pubdefs = vec![head.clone()];
        for &(symbol, name, at) in &defined {
            if td.as_ref().is_some_and(|one| one.publics.contains_key(&symbol))
                && pubdefs.last().is_some_and(|last| last.len() > head.len())
            {
                pubdefs.push(head.clone());
            }
            let last = pubdefs.last_mut().expect("a record");
            last.extend(string(name));
            if bits == 32 {
                last.extend((at as u32).to_le_bytes());
            } else {
                let at = u16::try_from(at)
                    .unwrap_or_else(|_| panic!("struct.error: 'H' format requires 0 <= number <= 65535"));
                last.extend(at.to_le_bytes());
            }
            last.push(0);
            if let Some(typed) = td.as_ref().and_then(|one| one.publics.get(&symbol)) {
                records.push(Rc::new(omf::Record::new(
                    if bits == 32 { omf::PUBDEF + 1 } else { omf::PUBDEF },
                    pubdefs.pop().expect("a record"),
                )));
                records.extend(typed.iter().cloned());
                pubdefs.push(head.clone());
            }
        }
        for body in pubdefs.into_iter().filter(|one| one.len() > head.len()) {
            records.push(Rc::new(omf::Record::new(if bits == 32 { omf::PUBDEF + 1 } else { omf::PUBDEF }, body)));
        }
    }
    if let Some(one) = &td {
        records.extend(one.module.iter().cloned());
    }
    records.extend(data);
    records.extend(td.iter().map(|one| Rc::clone(&one.source)));
    for (index, lines) in lines.iter().enumerate() {
        match named_comdats.get(&index) {
            Some(named) => records.extend(linsym(bits, named, lines)?),
            None => records.extend(linnum(index + 1, lines)?),
        }
    }
    records.push(Rc::new(omf::Record::new(omf::MODEND, vec![0])));
    Ok(records.iter().flat_map(|record| record.emit()).collect())
}

#[cfg(test)]
mod tests {
    use llrm_object::{Arch, Binding, Reloc, Symbol};

    use super::*;

    fn symbol(
        name: &str,
        definition: Definition,
    ) -> Symbol {
        Symbol { name: name.into(), binding: Binding::Public, definition, size: 0, group: None }
    }

    fn section(
        name: &str,
        image: Vec<u8>,
        relocs: Vec<Reloc>,
    ) -> Section {
        let spans = vec![[0, image.len()]];
        Section { name: name.into(), role: Role::Text, near: true, align: 1, image, spans, relocs }
    }

    fn object(
        arch: Arch,
        sections: Vec<Section>,
        symbols: Vec<Symbol>,
    ) -> Object {
        Object {
            name: "t.c".into(),
            arch,
            sections,
            symbols,
            omf_groups: Vec::new(),
            omf_comdat: Vec::new(),
            debug: None,
        }
    }

    /// A COMDAT object of one local function `helper` whose bytes are `code`,
    /// as an object named `t.c`.
    fn with_helper(code: Vec<u8>) -> Object {
        let mut made = object(
            Arch::I8086,
            vec![section("T", code, Vec::new())],
            vec![Symbol {
                name: "helper".into(),
                binding: Binding::Local,
                definition: Definition::Defined { section: 0, offset: 0 },
                size: 0, group: None,
            }],
        );
        made.omf_comdat = vec![0];
        made
    }

    /// The names of the module's COMDATs, as LNAMES holds them.
    fn comdat_names(made: &Object) -> Vec<String> {
        let records = crate::omf::parse(&write(made).unwrap()).unwrap();
        let lnames = records.iter().find(|one| one.r#type == omf::LNAMES).expect("LNAMES");
        let (mut names, mut at) = (Vec::new(), 0);
        while at < lnames.body.len() {
            let size = usize::from(lnames.body[at]);
            names.push(String::from_utf8_lossy(&lnames.body[at + 1..at + 1 + size]).into_owned());
            at += 1 + size;
        }
        names.into_iter().filter(|name| name.starts_with("helper")).collect()
    }

    /// A local function of one name in two modules of one name (`m.nib` of two
    /// directories) was the same COMDAT name in both, which jwlink refused
    /// as a redefinition (and a linker that kept the first would bind both to
    /// one body): a local COMDAT's name carries a key of what its module
    /// holds.
    #[test]
    fn a_local_comdat_of_the_same_name_in_two_modules_of_one_name_has_two_names() {
        let (first, second) =
            (comdat_names(&with_helper(vec![0x90, 0xC3])), comdat_names(&with_helper(vec![0x40, 0xC3])));
        assert_eq!((first.len(), second.len()), (1, 1), "{first:?} {second:?}");
        assert_ne!(first, second);
        assert!(first[0].starts_with("helper$"), "{first:?}");
        assert_eq!(first, comdat_names(&with_helper(vec![0x90, 0xC3])), "the same module has the same name each time");
    }

    /// A public function keeps its name, which other modules call it by, and
    /// the COMDAT alone defines it.
    #[test]
    fn a_public_comdat_keeps_its_name_and_is_defined_by_the_comdat_alone() {
        let mut made = with_helper(vec![0x90, 0xC3]);
        made.symbols[0].binding = Binding::Public;
        let records = crate::omf::parse(&write(&made).unwrap()).unwrap();
        assert_eq!(comdat_names(&made), ["helper"]);
        assert_eq!(records.iter().filter(|one| one.r#type == 0xC2).count(), 1);
        assert_eq!(records.iter().filter(|one| one.r#type == omf::PUBDEF).count(), 0);
    }

    /// NDMAX's 60-dimensional HARY expansion exceeded one LEDATA and was
    /// refused: a record may not end inside a fixup's field.
    #[test]
    fn a_fixup_is_never_split_between_ledata_records() {
        let size = CHUNK * 3;
        let starts = [CHUNK - 1, 2 * CHUNK - 2];
        let relocs = starts
            .iter()
            .map(|&at| Reloc { at, kind: Kind::FarPointer, target: Target::Symbol(0), addend: 0 })
            .collect();
        let made =
            object(Arch::I8086, vec![section("T", vec![0; size], relocs)], vec![symbol("f", Definition::Undefined)]);
        let bytes = write(&made).unwrap();
        let records = crate::omf::parse(&bytes).unwrap();
        let cuts: Vec<usize> = records
            .iter()
            .filter(|one| one.r#type == omf::LEDATA)
            .scan(0, |end, one| {
                *end += one.body.len() - 3;
                Some(*end)
            })
            .collect();
        assert_eq!(cuts.last(), Some(&size));
        assert!(!cuts.iter().any(|cut| starts.iter().any(|low| low < cut && *cut < low + 4)), "{cuts:?}");
    }

    /// A call to an undefined symbol is a self-relative fixup, whose field
    /// holds the addend.
    #[test]
    fn a_call_is_a_self_relative_fixup_against_an_extern() {
        let call = Reloc { at: 1, kind: Kind::PcRel { width: 4, from: 4 }, target: Target::Symbol(0), addend: -2 };
        let made = object(
            Arch::I386,
            vec![section("T", vec![0xE8, 0, 0, 0, 0], vec![call])],
            vec![symbol("f", Definition::Undefined)],
        );
        let records = crate::omf::parse(&write(&made).unwrap()).unwrap();
        let ledata = records.iter().find(|one| one.r#type == omf::LEDATA + 1).unwrap();
        assert_eq!(&ledata.body[ledata.body.len() - 5..], [0xE8, 0xFE, 0xFF, 0xFF, 0xFF]);
        let fixupp = records.iter().find(|one| one.r#type == omf::FIXUPP + 1).unwrap();
        // self-relative, offset32, at 1.
        assert_eq!(fixupp.body[0] & 0x40, 0);
        assert_eq!((fixupp.body[0] >> 2) & 0xF, 9);
    }

    /// jwlink wrote segment 0 for the S_LDATA16 of every BASIC module variable
    /// once the runtime was linked in: the debug fixup's frame was the data
    /// segment, which is not at its group's base there (`_NULL` comes first),
    /// so no group matched it. A data symbol's address in the debug section
    /// is addressed through its group.
    #[test]
    fn a_debug_address_in_a_group_is_addressed_through_the_group() {
        let mut data = section("D", vec![0; 4], vec![]);
        data.role = Role::Data;
        let far = Reloc { at: 2, kind: Kind::FarPointer, target: Target::Section(0), addend: 0 };
        let mut debug = section("$$SYMBOLS", vec![0; 8], vec![far]);
        debug.role = Role::Debug;
        let mut made = object(Arch::I8086, vec![data, debug], vec![]);
        made.omf_groups = vec![llrm_object::OmfGroup { name: "DGROUP".into(), members: vec![0] }];
        let records = crate::omf::parse(&write(&made).unwrap()).unwrap();
        let fixupp = records.iter().filter(|one| one.r#type == omf::FIXUPP).last().unwrap();
        // locat (2 bytes), then fix data: frame method in bits 4-6.
        assert_eq!((fixupp.body[2] >> 4) & 7, GROUP_FRAME, "{:?}", fixupp.body);
    }

    /// A 64-bit object was written with 16-bit records.
    #[test]
    fn a_64_bit_object_is_refused() {
        let made = object(Arch::X8664, vec![section("T", vec![0xC3], vec![])], vec![]);
        assert!(matches!(write(&made), Err(Error::Unencodable(text)) if text.contains("64-bit")));
    }

    /// A pc-relative field OMF has no fixup for was written near-ish; it is
    /// refused.
    #[test]
    fn a_pc_relative_field_omf_cannot_say_is_refused() {
        let jump = Reloc { at: 1, kind: Kind::PcRel { width: 4, from: 8 }, target: Target::Symbol(0), addend: 0 };
        let made =
            object(Arch::I386, vec![section("T", vec![0; 8], vec![jump])], vec![symbol("f", Definition::Undefined)]);
        assert!(matches!(write(&made), Err(Error::Unencodable(_))));
    }
}
