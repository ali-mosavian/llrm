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
    section: &Section,
    at: usize,
) -> Result<Resolved, Error> {
    let reloc = section.relocs.iter().find(|one| one.at == at).expect("the caller names a fixup");
    let (loc, relative) = location(reloc.kind)?;
    let in_group = |section: usize| object.omf_groups.iter().position(|group| group.members.contains(&section));
    let (method, datum, group) = match reloc.target {
        Target::OmfGroup(group) => (GROUP_TARGET, group as i64 + 1, Some(group)),
        Target::Section(section) => (SEGMENT_TARGET, section as i64 + 1, in_group(section)),
        Target::Symbol(symbol) => match (&object.symbols[symbol], object.symbols[symbol].definition) {
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
    let subrecord = match group {
        Some(group) if matches!(loc, OFFSET | OFFSET32) && !relative => {
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

/// Section `index`'s LEDATA and FIXUPP records.
fn ledata(
    object: &Object,
    extern_index: &[usize],
    index: usize,
) -> Result<Vec<Rc<omf::Record>>, Error> {
    let bits = object.arch.bits();
    let section = &object.sections[index];
    let mut at_sorted: Vec<usize> = section.relocs.iter().map(|one| one.at).collect();
    at_sorted.sort_unstable();
    let mut image = section.image.clone();
    let mut subrecords = std::collections::BTreeMap::new();
    for &at in &at_sorted {
        let made = resolved(object, extern_index, section, at)?;
        let kind = section.relocs.iter().find(|one| one.at == at).expect("sorted from them").kind;
        if kind == Kind::SegmentBase && made.value != 0 {
            return Err(unencodable(format!("{}: a segment base with an addend", section.name)));
        }
        pack_field(&mut image, at, packed(kind), made.value);
        subrecords.insert(at, (made.subrecord, kind));
    }
    let (mut out, mut placed) = (Vec::new(), 0);
    for &[mut start, end] in &section.spans {
        while start < end {
            let mut stop = end.min(start + CHUNK);
            for &at in &at_sorted {
                if at < stop && stop < at + subrecords[&at].1.width() {
                    stop = at;
                }
            }
            let payload = &image[start..stop];
            out.push(if bits == 32 {
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
    let debug = object.debug.is_some() && !turbo;
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
        let size = section.image.len();
        // A USE32 segment is dword aligned at least: its offsets are 32-bit.
        let alignment = if selector(section) {
            PARAGRAPH
        } else {
            alignment_for(if bits == 32 { section.align.max(4) } else { section.align })
        };
        let alignment = if section.role == Role::Stack { STACK_SEGMENT } else { alignment };
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
    let mut data = Vec::new();
    for index in 0..object.sections.len() {
        data.extend(ledata(object, &extern_index, index)?);
    }

    let mut records = vec![Rc::new(omf::Record::new(omf::THEADR, string(&object.name)))];
    records.extend(td.iter().flat_map(|one| one.before.iter().cloned()));
    records.push(Rc::new(omf::Record::new(omf::LNAMES, lnames.iter().flat_map(|one| string(one)).collect())));
    if debug {
        // CodeView 4's marker: LINK /CO reads the debug information after it.
        records.push(Rc::new(omf::Record::new(omf::COMENT, vec![0x00, 0xA1, 0x01, b'C', b'V'])));
    }
    records.extend(segdefs);
    records.extend(grpdefs);
    let declared: Vec<&llrm_object::Symbol> =
        object.symbols.iter().filter(|symbol| symbol.definition == Definition::Undefined).collect();
    if !declared.is_empty() {
        let body = declared.iter().flat_map(|symbol| [string(&symbol.name), vec![0]].concat()).collect();
        records.push(Rc::new(omf::Record::new(omf::EXTDEF, body)));
    }
    for index in 0..object.sections.len() {
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
        records.extend(linnum(index + 1, lines)?);
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
        Symbol { name: name.into(), binding: Binding::Public, definition, group: None }
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
        Object { name: "t.c".into(), arch, sections, symbols, omf_groups: Vec::new(), debug: None }
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
