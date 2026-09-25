//! The one OMF emitter, shared by the C and BC-object frontends.
//!
//! Port of `qbopt/backend/omfwrite.py`. The C adapter consumes a
//! `masm::Module`; the BC adapter, `written_bc`, consumes decoded segment,
//! symbol and relocation semantics plus freshly laid-out code. Both
//! construct a complete object here; neither invokes an assembler or
//! rewrites an input record stream. A reference to anything the C module
//! defines is a fixup against its segment with the addend in the code, as
//! jwasm writes it; anything else names its EXTDEF.

use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;
use std::sync::LazyLock;

use iced_x86::Register;

use crate::support::hash::IndexMap;

use crate::backend::layout;
use crate::backend::masm;
use crate::backend::select;
use crate::backend::target;
use crate::model::ir::{self, Loc, Operation, Semantics, Space};
use crate::model::lir::LirBody;
use crate::objectfile::module::{Addr, Module, SourceMap};
use crate::objectfile::omf;
use crate::support::pyrepr::{self, Repr};

/// OMF locations: offset16, segment base, ptr16:16, and the offset32 a
/// 32-bit address's displacement takes.
pub const OFFSET: i64 = 1;
pub const BASE: i64 = 2;
pub const POINTER: i64 = 3;
pub const OFFSET32: i64 = 9;
pub static WIDE: LazyLock<IndexMap<i64, usize>> =
    LazyLock::new(|| IndexMap::from_iter([(OFFSET, 2), (BASE, 2), (POINTER, 4), (OFFSET32, 4)]));
pub static CLASSES: LazyLock<IndexMap<&'static str, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([("_DATA", "DATA"), ("_BSS", "BSS"), ("CONST", "CONST")]));
/// DGROUP, the only group.
pub const GROUP: i64 = 1;
/// LEDATA payload per record. A fixup's offset into its record has ten bits.
pub const CHUNK: usize = 1000;
/// relocatable, word aligned, public, 16-bit
pub const ACBP: u8 = 0x48;
pub const SEGMENT_TARGET: u8 = 0;
pub const GROUP_TARGET: u8 = 1;
pub const EXTERNAL_TARGET: u8 = 2;
pub const GROUP_FRAME: u8 = 1;
pub const TARGET_FRAME: u8 = 5;

/// An instruction or reference this writer has no bytes for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unencodable(pub String);

impl fmt::Display for Unencodable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unencodable {}

/// A phi reached emission. Always a bug in phi elimination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Survived(pub String);

impl fmt::Display for Survived {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Survived {}

/// Every exception `written` and `written_bc` let escape, each with
/// Python's message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Unprintable(masm::Unprintable),
    Unencodable(Unencodable),
    Value(omf::ValueError),
    Survived(Survived),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unprintable(one) => one.fmt(formatter),
            Error::Unencodable(one) => one.fmt(formatter),
            Error::Value(one) => formatter.write_str(&one.0),
            Error::Survived(one) => one.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {}

impl From<masm::Unprintable> for Error {
    fn from(one: masm::Unprintable) -> Self {
        Error::Unprintable(one)
    }
}

impl From<Unencodable> for Error {
    fn from(one: Unencodable) -> Self {
        Error::Unencodable(one)
    }
}

impl From<omf::ValueError> for Error {
    fn from(one: omf::ValueError) -> Self {
        Error::Value(one)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixup {
    pub at: usize,
    pub loc: i64,
    pub name: String,
    pub relative: bool,
}

impl Fixup {
    pub fn new(at: usize, loc: i64, name: impl Into<String>) -> Self {
        Self { at, loc, name: name.into(), relative: false }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Piece {
    pub code: Vec<u8>,
    /// `at` relative to the piece
    pub fixups: Vec<Fixup>,
}

impl Piece {
    pub fn new(code: Vec<u8>) -> Self {
        Self { code, fixups: Vec::new() }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Jump {
    pub name: String,
    pub label: String,
    pub long: bool,
}

impl Jump {
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self { name: name.into(), label: label.into(), long: false }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Near {
    pub name: String,
}

/// `Encoded`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Encoded {
    Label(masm::Label),
    Piece(Piece),
    Jump(Jump),
    Near(Near),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub name: String,
    pub klass: String,
    pub grouped: bool,
    pub image: Vec<u8>,
    /// [start, end) holding data
    pub spans: Vec<[usize; 2]>,
    pub fixups: Vec<Fixup>,
}

impl Segment {
    pub fn new(name: &str, klass: &str, grouped: bool) -> Self {
        Self {
            name: name.to_owned(),
            klass: klass.to_owned(),
            grouped,
            image: Vec::new(),
            spans: Vec::new(),
            fixups: Vec::new(),
        }
    }

    pub fn put(&mut self, code: &[u8], fixups: &[Fixup]) {
        let at = self.image.len();
        self.fixups.extend(
            fixups.iter().map(|one| Fixup { at: at + one.at, loc: one.loc, name: one.name.clone(), relative: one.relative }),
        );
        self.image.extend_from_slice(code);
        match self.spans.last_mut() {
            Some(last) if last[1] == at => last[1] += code.len(),
            _ if !code.is_empty() => self.spans.push([at, at + code.len()]),
            _ => {}
        }
    }

    pub fn skip(&mut self, size: usize) {
        self.image.extend(std::iter::repeat_n(0, size));
    }
}

/// `struct.pack_into("<H", buffer, at, value)` of a value already masked.
fn pack_into(buffer: &mut [u8], at: usize, value: i64) {
    buffer[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

/// `value` into the `WIDE[loc]` bytes of a relocated field at `at`.
fn pack_field(buffer: &mut [u8], at: usize, loc: i64, value: i64) {
    if loc == OFFSET32 {
        buffer[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
    } else {
        pack_into(buffer, at, value & 0xFFFF);
    }
}

/// A relocated field's value.
fn field(buffer: &[u8], at: usize, loc: i64) -> i64 {
    if loc == OFFSET32 {
        i64::from(u32::from_le_bytes([buffer[at], buffer[at + 1], buffer[at + 2], buffer[at + 3]]))
    } else {
        i64::from(u16::from_le_bytes([buffer[at], buffer[at + 1]]))
    }
}

/// Write a complete fresh object for the BC-object frontend.
///
/// BC's OBJ is input syntax here: its declarations, data and relocations
/// are decoded, just as C source is decoded by the other frontend. The
/// output is never made by splicing LEDATA or FIXUPP records back into BC's
/// record stream; `_bc_object` serializes a new stream from those semantics.
///
/// `Ok(Err(reason))` is Python's `str` answer; `Err` is an exception.
#[allow(clippy::too_many_arguments)]
pub fn written_bc(
    found: &Module,
    bodies: &[LirBody],
    records: &[Rc<omf::Record>],
    assignment: &IndexMap<u32, Register>,
    tables: &[(i64, i64)],
    fields: &BTreeSet<i64>,
    reached: Option<&BTreeSet<i64>>,
    native_fpu: bool,
    ordered: bool,
    source: Option<&SourceMap>,
) -> Result<Result<Vec<u8>, String>, Error> {
    _require_no_phis(bodies)?;
    let ordered = ordered || (!bodies.is_empty() && bodies.iter().all(|body| body.ordered));
    let laid = layout::rebuild(
        found,
        bodies.iter().map(|one| (one.name.clone(), one.clone())).collect(),
        tables,
        fields,
        reached,
        native_fpu,
        Some(assignment).filter(|assignment| !assignment.is_empty()),
        ordered,
        &bodies.iter().filter(|body| body.ordered).map(|body| body.entry).collect(),
        source,
    );
    let laid = match laid {
        Ok(laid) => laid,
        Err(why) => return Ok(Err(why)),
    };

    let kept = bodies.iter().map(|body| body.entry).min().expect("min() arg is an empty sequence");
    let mut image = found.code[..kept as usize].to_vec();
    image.extend_from_slice(&laid.code);
    let mut relocations: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for &(new, old) in &laid.relocations {
        relocations.entry(old).or_default().push(kept + new);
    }
    let groups: Vec<String> = omf::groups(records).into_keys().collect();
    // A SEGMENT-space address outside DGROUP is an offset in its owning
    // segment, even when the instruction carries no explicit segment
    // override. Fresh OMF therefore frames those references at their target
    // segment.
    let target_segment = |address: &Addr| address.space == Space::Segment && !found.dgroup.contains(address.index);
    let group_framed = laid
        .symbols
        .iter()
        .any(|(_offset, address)| address.segment == Register::None && !target_segment(address));
    if group_framed && !groups.iter().any(|one| one == "DGROUP") {
        return Ok(Err("generated data references require an established DGROUP frame".to_owned()));
    }
    let mut added: Vec<(i64, omf::Fixup)> = Vec::new();
    for (offset, address) in &laid.symbols {
        let target = if address.space == Space::Segment { "segment" } else { "external" };
        let fixup = if address.segment != Register::None || target_segment(address) {
            omf::target_offset_fixup(found.seg, kept + offset, target, address.index, address.disp)?
        } else {
            let group = groups.iter().position(|one| one == "DGROUP").expect("checked above") as i64 + 1;
            omf::offset_fixup(found.seg, kept + offset, target, address.index, address.disp, group)?
        };
        added.push((kept + offset, fixup));
    }
    let mut moved = laid.covered.clone();
    moved.extend(laid.moved.iter().map(|(old, new)| (*old, *new)));
    let relocations: IndexMap<i64, Vec<i64>> = relocations
        .into_iter()
        .map(|(old, destinations)| {
            let mut unique: Vec<i64> = Vec::new();
            for one in destinations {
                if !unique.contains(&one) {
                    unique.push(one);
                }
            }
            (old, unique)
        })
        .collect();
    _bc_object(records, found.seg, kept, &image, &moved, &relocations, &laid.dropped, &added)
}

/// Reject SSA joins before anything attempts to encode instructions.
pub fn _require_no_phis(bodies: &[LirBody]) -> Result<(), Error> {
    let stuck: Vec<i64> =
        bodies.iter().flat_map(|body| &body.blocks).filter(|block| !block.phis.is_empty()).map(|block| block.at).collect();
    if !stuck.is_empty() {
        let at: Vec<String> = stuck.iter().map(|one| format!("{one:#06x}")).collect();
        return Err(Error::Survived(Survived(format!(
            "a phi survives at {}; nothing below can emit one",
            at.join(", ")
        ))));
    }
    Ok(())
}

pub fn _mapped(offset: i64, kept: i64, moved: &IndexMap<i64, i64>) -> Option<i64> {
    if offset < kept { Some(offset) } else { moved.get(&offset).copied() }
}

fn record(one: &omf::Record) -> Rc<omf::Record> {
    Rc::new(omf::Record::new(one.r#type, one.body.clone()))
}

/// Canonical OMF serialization of one decoded BC module.
///
/// Segment and symbol indices deliberately retain the frontend's numbering;
/// they are identities in decoded FIXUPP semantics, not positions borrowed
/// from the old output stream. Record boundaries and ordering are ours.
#[allow(clippy::too_many_arguments)]
pub fn _bc_object(
    records: &[Rc<omf::Record>],
    code_seg: i64,
    kept: i64,
    code: &[u8],
    moved: &IndexMap<i64, i64>,
    relocations: &IndexMap<i64, Vec<i64>>,
    dropped: &BTreeSet<i64>,
    added: &[(i64, omf::Fixup)],
) -> Result<Result<Vec<u8>, String>, Error> {
    let segments = omf::segments(records);
    if !(0 < code_seg && (code_seg as usize) < segments.len()) || segments[code_seg as usize].is_none() {
        return Ok(Err("the module has no code segment".to_owned()));
    }
    if records.iter().any(|one| one.r#type & 0xFE == omf::MODEND && omf::has_start_address(one)) {
        return Ok(Err("MODEND carries a start address, which this does not move yet".to_owned()));
    }
    let supported = [
        omf::THEADR,
        omf::COMENT,
        omf::MODEND,
        omf::EXTDEF,
        omf::PUBDEF,
        omf::LINNUM,
        omf::LNAMES,
        omf::SEGDEF,
        omf::GRPDEF,
        omf::FIXUPP,
        omf::LEDATA,
    ];
    if let Some(unknown) = records.iter().find(|one| !supported.contains(&(one.r#type & 0xFE))) {
        return Ok(Err(format!("fresh OMF emission does not model {}", unknown.name())));
    }

    let mut images: IndexMap<i64, Vec<u8>> = IndexMap::default();
    let mut spans: IndexMap<i64, Vec<(i64, i64)>> = IndexMap::default();
    for (index, segment) in segments.iter().enumerate() {
        let index = index as i64;
        let Some(segment) = segment.as_ref().filter(|_| index != 0) else { continue };
        images.insert(
            index,
            if index == code_seg { code.to_vec() } else { omf::segment_image(records, index, segment.1) },
        );
        let pieces: Vec<(i64, i64)> = if index == code_seg && !code.is_empty() {
            vec![(0, code.len() as i64)]
        } else {
            omf::ledata(records)
                .into_iter()
                .filter(|(_record, seg, _at, _payload)| *seg == index)
                .map(|(_record, _seg, at, payload)| (at, at + payload.len() as i64))
                .collect()
        };
        spans.insert(index, _merged(&pieces));
    }

    let mut placed: IndexMap<i64, Vec<(i64, omf::Fixup, i64)>> =
        images.keys().map(|index| (*index, Vec::new())).collect();
    for fixup in omf::fixups(records) {
        let Some(seg) = fixup.seg.filter(|seg| placed.contains_key(seg)) else {
            return Ok(Err("a fixup has no segment to attach to".to_owned()));
        };
        let destinations: Vec<i64> = if seg == code_seg {
            let landed = if fixup.offset >= kept { relocations.get(&fixup.offset).cloned() } else { Some(vec![fixup.offset]) };
            match landed {
                Some(landed) => landed,
                None => {
                    if dropped.contains(&fixup.offset) {
                        continue;
                    }
                    return Ok(Err(format!(
                        "the fixup at {:#x} has nowhere to go in the rebuilt segment",
                        fixup.offset
                    )));
                }
            }
        } else {
            vec![fixup.offset]
        };
        let mut disp = fixup.disp;
        if fixup.target == "segment" && fixup.index == code_seg && fixup.disp_pos.is_some() {
            let Some(mapped) = _mapped(disp, kept, moved) else {
                return Ok(Err(format!("a fixup names {disp:#x}, which is not an instruction the layout placed")));
            };
            disp = mapped;
        }
        for destination in destinations {
            placed[&seg].push((destination, fixup.clone(), disp));
        }
    }
    for (destination, fixup) in added {
        placed[&code_seg].push((*destination, fixup.clone(), fixup.disp));
    }

    let mut headers: Vec<Rc<omf::Record>> = Vec::new();
    let Some(first) = records.iter().find(|one| one.r#type & 0xFE == omf::THEADR) else {
        return Ok(Err("the module has no THEADR".to_owned()));
    };
    headers.push(record(first));
    headers.extend(records.iter().filter(|one| one.r#type & 0xFE == omf::COMENT).map(|one| record(one)));

    let lnames: Vec<u8> = omf::names(records)[1..].iter().flat_map(|one| _string(one)).collect();
    headers.push(Rc::new(omf::Record::new(omf::LNAMES, lnames)));

    let mut seg_index = 0;
    for one in records {
        if one.r#type & 0xFE != omf::SEGDEF {
            continue;
        }
        seg_index += 1;
        let mut body = one.body.clone();
        if seg_index == code_seg {
            let at = omf::segment_length_at(one);
            pack_into(&mut body, at, (code.len() & 0xFFFF) as i64);
            if code.len() == 0x10000 {
                body[0] |= 0x02;
            } else {
                body[0] &= !0x02;
            }
        }
        headers.push(Rc::new(omf::Record::new(one.r#type, body)));
    }
    headers.extend(records.iter().filter(|one| one.r#type & 0xFE == omf::GRPDEF).map(|one| record(one)));
    headers.extend(records.iter().filter(|one| one.r#type & 0xFE == omf::EXTDEF).map(|one| record(one)));

    for one in records {
        if !matches!(one.r#type & 0xFE, omf::PUBDEF | omf::LINNUM) {
            continue;
        }
        let mut changes: IndexMap<usize, Option<i64>> = IndexMap::default();
        for at in omf::code_offsets(one, code_seg) {
            if at + 2 > one.body.len() {
                return Ok(Err("a record names a code offset past its own end".to_owned()));
            }
            let value = i64::from(u16::from_le_bytes([one.body[at], one.body[at + 1]]));
            changes.insert(at, _mapped(value, kept, moved));
        }
        if changes.values().any(Option::is_none) {
            return Ok(Err("a symbol or line names code that the layout did not place".to_owned()));
        }
        let values: IndexMap<usize, i64> = changes.into_iter().filter_map(|(at, value)| Some((at, value?))).collect();
        let patched = omf::patched(one, &values);
        headers.push(Rc::new(omf::Record::new(patched.r#type, patched.body.clone())));
    }

    let mut data: Vec<Rc<omf::Record>> = Vec::new();
    for index in 1..segments.len() as i64 {
        if segments[index as usize].is_none() {
            continue;
        }
        let made = _fresh_segment(index, &images[&index], &spans[&index], &mut placed[&index])?;
        match made {
            Ok(made) => data.extend(made),
            Err(why) => return Ok(Err(why)),
        }
    }
    let Some(end) = records.iter().rev().find(|one| one.r#type & 0xFE == omf::MODEND) else {
        return Ok(Err("the module has no MODEND".to_owned()));
    };
    let mut fresh = headers;
    fresh.extend(data);
    fresh.push(record(end));
    Ok(Ok(fresh.iter().flat_map(|one| one.emit()).collect()))
}

pub fn _merged(spans: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut sorted = spans.to_vec();
    sorted.sort();
    let mut out: Vec<(i64, i64)> = Vec::new();
    for (lo, hi) in sorted {
        if lo == hi {
            continue;
        }
        match out.last_mut() {
            Some(last) if lo <= last.1 => *last = (last.0, last.1.max(hi)),
            _ => out.push((lo, hi)),
        }
    }
    out
}

/// Canonical LEDATA/FIXUPP records for one semantic segment.
pub fn _fresh_segment(
    index: i64,
    image: &[u8],
    spans: &[(i64, i64)],
    fixups: &mut Vec<(i64, omf::Fixup, i64)>,
) -> Result<Result<Vec<Rc<omf::Record>>, String>, Error> {
    let widths: IndexMap<i64, i64> =
        IndexMap::from_iter([(0, 1), (1, 2), (2, 2), (3, 4), (4, 1), (5, 2), (9, 4), (11, 6), (13, 4)]);
    if fixups.iter().any(|(_at, one, _disp)| !widths.contains_key(&one.loc)) {
        return Ok(Err("unsupported relocation field width".to_owned()));
    }
    let mut out: Vec<Rc<omf::Record>> = Vec::new();
    fixups.sort_by_key(|item| item.0);
    let mut placed = 0;
    for &(span_lo, span_hi) in spans {
        let mut start = span_lo;
        while start < span_hi {
            let mut stop = span_hi.min(start + CHUNK as i64);
            let crossing =
                fixups.iter().filter(|(at, one, _disp)| *at < stop && stop < at + widths[&one.loc]).map(|item| item.0).min();
            if let Some(crossing) = crossing {
                stop = crossing;
            }
            if stop <= start {
                return Ok(Err(format!("segment {index}: a relocation field cannot fit in LEDATA")));
            }
            out.push(omf::ledata_record(index, start, &image[start as usize..stop as usize])?);
            let mine: Vec<&(i64, omf::Fixup, i64)> =
                fixups.iter().filter(|(at, _one, _disp)| start <= *at && *at < stop).collect();
            if !mine.is_empty() {
                let subrecords = mine
                    .iter()
                    .map(|(at, one, disp)| _resolved_fixup(one, at - start, *disp))
                    .collect::<Result<Vec<_>, _>>()?;
                out.push(omf::fixupp_record(&subrecords));
            }
            placed += mine.len();
            start = stop;
        }
    }
    if placed != fixups.len() {
        return Ok(Err(format!("segment {index}: a fixup lies outside initialized data")));
    }
    Ok(Ok(out))
}

/// Encode a decoded fixup explicitly, with no dependency on THREAD state.
pub fn _resolved_fixup(one: &omf::Fixup, offset: i64, disp: i64) -> Result<Vec<u8>, Error> {
    if !(0..1024).contains(&offset) {
        return Err(Error::Value(omf::ValueError(format!("a fixup offset is ten bits; {offset:#x} does not fit"))));
    }
    let target_method = match one.target.as_str() {
        "segment" => 0,
        "group" => 1,
        "external" => 2,
        _ => return Err(Error::Value(omf::ValueError(format!("unsupported fixup target {}", one.target)))),
    };
    let (mut frame_method, frame_index) = match one.frame {
        Some(omf::Frame::Thread(thread)) => (Some(thread.method), Some(thread.index)),
        Some(omf::Frame::Int(frame)) => (one.frame_method, Some(frame)),
        None => (Some(5), None), // target's frame
    };
    if frame_method.is_none() {
        frame_method = Some(5);
    }
    let frame_method = frame_method.expect("set above");
    let lead = 0x80 | (if one.selfrel { 0 } else { 0x40 }) | one.loc << 2 | offset >> 8;
    let mut body: Vec<u8> = vec![lead as u8, (offset & 0xFF) as u8, (frame_method << 4 | target_method) as u8];
    if frame_method < 3 {
        let Some(frame_index) = frame_index else {
            return Err(Error::Value(omf::ValueError("an explicit fixup frame has no index".to_owned())));
        };
        body.extend(omf::as_index(frame_index)?);
    }
    body.extend(omf::as_index(one.index)?);
    if !(0..=0xFFFF).contains(&disp) {
        panic!("struct.error: 'H' format requires 0 <= number <= 65535");
    }
    body.extend((disp as u16).to_le_bytes());
    Ok(body)
}

/// `module` without the data objects nothing reaches.
///
/// A `Datum::Object` starts a unit that stays only if code, a public, or a
/// kept unit names one of its labels. Items before a segment's first Object
/// always stay.
pub fn live(module: &masm::Module) -> Result<masm::Module, Error> {
    if !module.data.iter().any(|(_, items)| items.iter().any(|item| matches!(item, masm::Datum::Object(_)))) {
        return Ok(module.clone());
    }
    let mut reached: BTreeSet<String> = module.publics.iter().cloned().collect();
    for (number, procedure) in module.procedures.iter().enumerate() {
        reached.insert(procedure.name.clone());
        for item in masm::listing(procedure, number)? {
            for one in _items(&item, &module.names, number)? {
                match one {
                    Encoded::Piece(Piece { fixups, .. }) => {
                        reached.extend(fixups.iter().map(|fixup| _target(&fixup.name).to_owned()));
                    }
                    Encoded::Near(Near { name }) => {
                        reached.insert(name);
                    }
                    Encoded::Label(_) | Encoded::Jump(_) => {}
                }
            }
        }
    }
    // (segment entry, droppable, items)
    let mut units: Vec<(usize, bool, Vec<masm::Datum>)> = Vec::new();
    for (entry, (_segment, items)) in module.data.iter().enumerate() {
        units.push((entry, false, Vec::new()));
        for item in items {
            if matches!(item, masm::Datum::Object(_)) {
                units.push((entry, true, Vec::new()));
            }
            units.last_mut().expect("a unit is open").2.push(item.clone());
        }
    }
    let labels: Vec<BTreeSet<&str>> = units
        .iter()
        .map(|(_, _, run)| {
            run.iter()
                .filter_map(|item| match item {
                    masm::Datum::Label(masm::Label { name }) | masm::Datum::Object(masm::Label { name }) => {
                        Some(name.as_str())
                    }
                    _ => None,
                })
                .collect()
        })
        .collect();
    let mut kept: Vec<bool> = units.iter().map(|(_, droppable, _)| !droppable).collect();
    let mut pending: Vec<usize> = (0..units.len()).filter(|&index| kept[index]).collect();
    while let Some(index) = pending.pop() {
        for item in &units[index].2 {
            if let masm::Datum::Pointer(masm::Pointer { name, .. }) | masm::Datum::SegmentWord(name) = item {
                reached.insert(_target(name).to_owned());
            }
        }
        for other in 0..units.len() {
            if !kept[other] && labels[other].iter().any(|name| reached.contains(*name)) {
                kept[other] = true;
                pending.push(other);
            }
        }
    }
    let mut data: Vec<(String, Vec<masm::Datum>)> =
        module.data.iter().map(|(segment, _)| (segment.clone(), Vec::new())).collect();
    for ((entry, _, run), keep) in units.into_iter().zip(kept) {
        if keep {
            data[entry].1.extend(run);
        }
    }
    Ok(masm::Module { data, ..module.clone() })
}

/// How an object lays out its procedures' code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeLayout {
    /// All of it in one segment, where a near call may reach any procedure.
    OneSegment,
    /// A segment of its own for each procedure, all of one name, so that the
    /// linker's `option eliminate` drops each one nothing calls. Every call
    /// between procedures must then be far.
    PerProcedure,
}

pub fn written(module: &masm::Module, source: &str) -> Result<Vec<u8>, Error> {
    written_as(module, source, CodeLayout::OneSegment)
}

pub fn written_as(module: &masm::Module, source: &str, layout: CodeLayout) -> Result<Vec<u8>, Error> {
    let module = &live(module)?;
    let groups: Vec<Vec<usize>> = match layout {
        CodeLayout::OneSegment => vec![(0..module.procedures.len()).collect()],
        CodeLayout::PerProcedure => (0..module.procedures.len()).map(|one| vec![one]).collect(),
    };
    let mut segments: Vec<Segment> = groups.iter().map(|_| Segment::new(&module.code, "CODE", false)).collect();
    let mut named: IndexMap<String, Segment> = IndexMap::from_iter([("_DATA".to_owned(), Segment::new("_DATA", "DATA", true))]);
    for (name, _items) in &module.data {
        if !named.contains_key(name) {
            let private = module.private.contains(name);
            let klass = CLASSES.get(name.as_str()).copied().unwrap_or(if private { "FAR_DATA" } else { "DATA" });
            named.insert(name.clone(), Segment::new(name, klass, !private));
        }
    }
    segments.extend(named.into_values());
    let mut symbols: IndexMap<String, (usize, usize)> = IndexMap::default();
    for (name, items) in &module.data {
        let index = segments
            .iter()
            .position(|one| &one.name == name)
            .unwrap_or_else(|| panic!("ValueError: {} is not in list", pyrepr::string(name)));
        _data(&mut segments[index], index, items, &mut symbols);
    }
    for (index, group) in groups.iter().enumerate() {
        _code(&mut segments[index], index, module, group, &mut symbols)?;
    }
    let externs: IndexMap<String, String> = module.externs.iter().cloned().collect();
    let records = _records(module, source, &mut segments, &symbols, &externs)?;
    Ok(records.iter().flat_map(|record| record.emit()).collect())
}

pub fn _data(segment: &mut Segment, index: usize, items: &[masm::Datum], symbols: &mut IndexMap<String, (usize, usize)>) {
    for item in items {
        match item {
            masm::Datum::Label(masm::Label { name }) | masm::Datum::Object(masm::Label { name }) => {
                symbols.insert(name.clone(), (index, segment.image.len()));
            }
            masm::Datum::Fill(masm::Fill { size, byte: None }) => segment.skip(*size as usize),
            masm::Datum::Fill(masm::Fill { size, byte: Some(byte) }) => segment.put(&vec![*byte; *size as usize], &[]),
            masm::Datum::Pointer(masm::Pointer { name, offset, far }) => {
                let loc = if *far { POINTER } else { OFFSET };
                segment.put(&vec![0; WIDE[&loc]], &[Fixup::new(0, loc, name.clone())]);
                let at = segment.image.len() - WIDE[&loc];
                pack_into(&mut segment.image, at, offset & 0xFFFF);
            }
            masm::Datum::Align(masm::Align { to }) => {
                segment.put(&vec![0; (-(segment.image.len() as i64)).rem_euclid(*to) as usize], &[]);
            }
            masm::Datum::Bytes(item) => segment.put(item, &[]),
            // No case matches it in Python's `_data`.
            masm::Datum::SegmentWord(_) => {}
        }
    }
}

/// The code of `module`'s procedures numbered `group`, into `segment`, the
/// object's segment `index`.
pub fn _code(
    segment: &mut Segment,
    index: usize,
    module: &masm::Module,
    group: &[usize],
    symbols: &mut IndexMap<String, (usize, usize)>,
) -> Result<(), Error> {
    let mut items: Vec<Encoded> = Vec::new();
    for &number in group {
        let procedure = &module.procedures[number];
        items.push(Encoded::Label(masm::Label { name: procedure.name.clone() }));
        for item in masm::listing(procedure, number)? {
            match _items(&item, &module.names, number) {
                Ok(encoded) => items.extend(encoded),
                Err(error) => return Err(Unencodable(format!("{}: {error}", procedure.name)).into()),
            }
        }
    }
    let labels = _relaxed(&mut items)?;
    let mut at = 0;
    for item in &items {
        match item {
            Encoded::Label(masm::Label { name }) => {
                symbols.insert(name.clone(), (index, at));
            }
            Encoded::Piece(Piece { code, fixups }) => segment.put(code, fixups),
            Encoded::Jump(Jump { name, label, long }) => segment.put(&_jump(name, labels[label], at, *long)?.code, &[]),
            Encoded::Near(Near { name }) if labels.contains_key(name) => {
                let distance = labels[name] - (at as i64 + 3);
                let distance = i16::try_from(distance)
                    .unwrap_or_else(|_| panic!("struct.error: 'h' format requires -32768 <= number <= 32767"));
                segment.put(&[&[0xE8][..], &distance.to_le_bytes()].concat(), &[]);
            }
            Encoded::Near(Near { name }) if module.procedures.iter().any(|one| &one.name == name) => {
                return Err(Unencodable(format!("a near call to {name} in another code segment")).into());
            }
            Encoded::Near(Near { name }) => {
                segment.put(&[0; 3], &[Fixup { relative: true, ..Fixup::new(1, OFFSET, name.clone()) }]);
                segment.image[at] = 0xE8;
            }
        }
        at = segment.image.len();
    }
    Ok(())
}

pub fn _items(
    item: &masm::Item,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
) -> Result<Vec<Encoded>, Unencodable> {
    Ok(match item {
        masm::Item::Label(label) => vec![Encoded::Label(label.clone())],
        masm::Item::Callee(masm::Callee { code, .. }) if !code.is_empty() => {
            code.iter().map(_part).collect::<Result<Vec<_>, _>>()?.into_iter().map(Encoded::Piece).collect()
        }
        masm::Item::Callee(masm::Callee { name, far: true, .. }) => vec![Encoded::Piece(Piece {
            code: vec![0x9A, 0, 0, 0, 0],
            fixups: vec![Fixup::new(1, POINTER, name.clone())],
        })],
        masm::Item::Callee(masm::Callee { name, .. }) => vec![Encoded::Near(Near { name: name.clone() })],
        masm::Item::Semantics(Semantics { op: Operation::Branch | Operation::Jump, name, target, .. }) => {
            let name = name.as_deref().filter(|one| !one.is_empty());
            let Some(target) = target else {
                return Err(Unencodable(format!("{} with no target", name.unwrap_or("jump"))));
            };
            vec![Encoded::Jump(Jump::new(name.unwrap_or("jmp"), masm::label(number, *target)))]
        }
        masm::Item::Semantics(what) => vec![Encoded::Piece(_encoded(what, names)?)],
    })
}

pub fn _part(part: &masm::InlinePart) -> Result<Piece, Unencodable> {
    match part {
        masm::InlinePart::Bytes(part) => Ok(Piece::new(part.clone())),
        masm::InlinePart::Fixup(kind, name, offset) if kind == "offset" => Ok(Piece {
            code: ((offset & 0xFFFF) as u16).to_le_bytes().to_vec(),
            fixups: vec![Fixup::new(0, OFFSET, name.clone())],
        }),
        masm::InlinePart::Fixup(kind, name, _) if kind == "segment" => {
            Ok(Piece { code: vec![0; 2], fixups: vec![Fixup::new(0, BASE, name.clone())] })
        }
        masm::InlinePart::Fixup(kind, name, offset) => Err(Unencodable(format!(
            "inline part ({}, {}, {offset})",
            pyrepr::string(kind),
            pyrepr::string(name)
        ))),
    }
}

pub fn _encoded(what: &Semantics, names: &IndexMap<(Space, i64), String>) -> Result<Piece, Unencodable> {
    let relocated = what.sources.iter().any(|one| matches!(one, Loc::Imm(ir::Imm { address: Some(_), .. })));
    let Some(made) = select::emit(what, 0, None, false, relocated, None) else {
        return Err(Unencodable(what.repr()));
    };
    let mut code = made.code.clone();
    let mut fixups: IndexMap<usize, Fixup> = IndexMap::default();
    for one in what.dests.iter().chain(&what.sources) {
        let (at, loc, addend, addr) = match one {
            Loc::Mem(ir::Mem { addr: Some(addr), through, index_through, .. })
                if matches!(addr.space, Space::Segment | Space::External) =>
            {
                let wide = [through, index_through].into_iter().any(|one| target::width_of(*one) == Some(4));
                (made.displacement_at, if wide { OFFSET32 } else { OFFSET }, addr.disp, addr)
            }
            Loc::Address(ir::Address { addr: Some(addr), .. }) if matches!(addr.space, Space::Segment | Space::External) => {
                (made.displacement_at, OFFSET, addr.disp, addr)
            }
            Loc::Imm(ir::Imm { address: Some(addr), .. }) if addr.space == Space::Group => {
                (made.immediate_at, BASE, 0, addr)
            }
            Loc::Imm(ir::Imm { address: Some(addr), value, .. }) => (made.immediate_at, OFFSET, addr.disp + value, addr),
            _ => continue,
        };
        let Some(at) = at else {
            return Err(Unencodable(format!("{}: no field for {}", what.repr(), one.repr())));
        };
        pack_field(&mut code, at, loc, addend);
        let name = names
            .get(&(addr.space, addr.index))
            .unwrap_or_else(|| panic!("KeyError: ({}, {})", addr.space.repr(), addr.index));
        fixups.insert(at, Fixup::new(at, loc, name.clone()));
    }
    Ok(Piece { code, fixups: fixups.into_values().collect() })
}

/// Every label's offset, with each jump short unless its target is out of reach.
///
/// Short first and lengthened to a fixed point, as jwasm does: lengthening
/// only moves targets further away, so it ends, and at the smallest layout.
pub fn _relaxed(items: &mut [Encoded]) -> Result<IndexMap<String, i64>, Unencodable> {
    loop {
        let (mut labels, mut at) = (IndexMap::default(), 0i64);
        for item in items.iter() {
            if let Encoded::Label(label) = item {
                labels.insert(label.name.clone(), at);
            }
            at += _length(item) as i64;
        }
        let mut changed = false;
        at = 0;
        for item in items.iter_mut() {
            // Measured before the jump may grow: `labels` is this pass's layout.
            let length = _length(item) as i64;
            if let Encoded::Jump(item) = item {
                if !item.long {
                    let Some(target) = labels.get(&item.label) else {
                        return Err(Unencodable(format!("a jump to {}, which is nowhere", item.label)));
                    };
                    if !(-128..=127).contains(&(target - (at + 2))) {
                        item.long = true;
                        changed = true;
                    }
                }
            }
            at += length;
        }
        if !changed {
            return Ok(labels);
        }
    }
}

pub fn _length(item: &Encoded) -> usize {
    match item {
        Encoded::Label(_) => 0,
        Encoded::Piece(Piece { code, .. }) => code.len(),
        Encoded::Jump(Jump { name, long, .. }) => {
            if !long {
                2
            } else if name == "jmp" {
                3
            } else {
                4
            }
        }
        Encoded::Near(_) => 3,
    }
}

pub fn _jump(name: &str, target: i64, at: usize, long: bool) -> Result<select::Emitted, Unencodable> {
    let made = if name == "jmp" {
        select::jump(target, at as u64, !long)
    } else {
        select::branch(name, target, at as u64, !long)
    };
    match made {
        Some(made) if made.code.len() == _length(&Encoded::Jump(Jump { long, ..Jump::new(name, "") })) => Ok(made),
        _ => Err(Unencodable(format!("{name} from {at:#x} to {target:#x}"))),
    }
}

pub fn _records(
    module: &masm::Module,
    source: &str,
    segments: &mut [Segment],
    symbols: &IndexMap<String, (usize, usize)>,
    externs: &IndexMap<String, String>,
) -> Result<Vec<Rc<omf::Record>>, Error> {
    let mut lnames: Vec<String> = vec![String::new()];

    let mut lname = |text: &str| -> i64 {
        lnames.push(text.to_owned());
        lnames.len() as i64
    };

    let mut segdefs = Vec::new();
    for segment in segments.iter() {
        let (klass, name) = (lname(&segment.klass), lname(&segment.name));
        let size = segment.image.len();
        let acbp = ACBP | if size == 0x10000 { 2 } else { 0 };
        let mut body = vec![acbp];
        body.extend(((size & 0xFFFF) as u16).to_le_bytes());
        body.extend(_names(&[name, klass, 1])?);
        segdefs.push(Rc::new(omf::Record::new(omf::SEGDEF, body)));
    }
    let grouped: Vec<i64> =
        segments.iter().enumerate().filter(|(_, segment)| segment.grouped).map(|(index, _)| index as i64 + 1).collect();
    let mut body = _names(&[lname("DGROUP")])?;
    for one in grouped {
        body.push(0xFF);
        body.extend(omf::as_index(one)?);
    }
    let grpdef = Rc::new(omf::Record::new(omf::GRPDEF, body));

    let used: BTreeSet<String> = segments
        .iter()
        .flat_map(|segment| segment.fixups.iter().map(|one| _target(&one.name).to_owned()))
        .filter(|name| !symbols.contains_key(name) && name != "DGROUP")
        .collect();
    let missing: Vec<&String> = used.iter().filter(|name| !externs.contains_key(*name)).collect();
    if !missing.is_empty() {
        // `sorted(missing)`: the set is sorted before it is printed.
        let printed = missing.iter().map(|one| pyrepr::string(one)).collect::<Vec<_>>().join(", ");
        return Err(Unencodable(format!("references to nothing defined or declared: [{printed}]")).into());
    }
    // masm.text's order, data externals first; LINK searches libraries in EXTDEF order.
    let mut declared: Vec<&String> = externs.keys().collect();
    declared.sort_by_key(|name| externs[*name] != "byte");
    let order: Vec<&String> =
        declared.into_iter().filter(|name| used.contains(*name) || module.requests.contains(*name)).collect();
    let numbered: IndexMap<String, i64> =
        order.iter().enumerate().map(|(n, name)| ((*name).clone(), n as i64 + 1)).collect();
    let mut data = Vec::new();
    for index in 1..=segments.len() {
        data.extend(_ledata(index, segments, symbols, &numbered, externs)?);
    }

    let mut records = vec![
        Rc::new(omf::Record::new(omf::THEADR, _string(source))),
        Rc::new(omf::Record::new(omf::LNAMES, lnames.iter().flat_map(|one| _string(one)).collect())),
    ];
    records.extend(segdefs);
    records.push(grpdef);
    if !order.is_empty() {
        let body = order.iter().flat_map(|name| [_string(name), vec![0]].concat()).collect();
        records.push(Rc::new(omf::Record::new(omf::EXTDEF, body)));
    }
    for (index, segment) in segments.iter().enumerate().map(|(index, segment)| (index + 1, segment)) {
        let defined: Vec<(&String, usize)> = symbols
            .iter()
            .filter(|(name, (seg, _))| *seg == index - 1 && module.publics.contains(name))
            .map(|(name, (_, at))| (name, *at))
            .collect();
        if !defined.is_empty() {
            let mut head = vec![if segment.grouped { GROUP as u8 } else { 0 }];
            head.extend(omf::as_index(index as i64)?);
            for (n, at) in defined {
                let at = u16::try_from(at)
                    .unwrap_or_else(|_| panic!("struct.error: 'H' format requires 0 <= number <= 65535"));
                head.extend(_string(n));
                head.extend(at.to_le_bytes());
                head.push(0);
            }
            records.push(Rc::new(omf::Record::new(omf::PUBDEF, head)));
        }
    }
    records.extend(data);
    records.push(Rc::new(omf::Record::new(omf::MODEND, vec![0])));
    Ok(records)
}

/// `segments[index - 1]` is Python's `segment`; the whole list is passed so
/// `_subrecord` may patch this segment's image while reading any segment's
/// grouping.
pub fn _ledata(
    index: usize,
    segments: &mut [Segment],
    symbols: &IndexMap<String, (usize, usize)>,
    externs: &IndexMap<String, i64>,
    kinds: &IndexMap<String, String>,
) -> Result<Vec<Rc<omf::Record>>, Error> {
    let mut fixups = segments[index - 1].fixups.clone();
    fixups.sort_by_key(|one| one.at);
    let mut subrecords: IndexMap<usize, Vec<u8>> = IndexMap::default();
    for one in &fixups {
        let made = _subrecord(one, index - 1, segments, symbols, externs, kinds)?;
        subrecords.insert(one.at, made);
    }
    let segment = &segments[index - 1];
    let (mut out, mut placed) = (Vec::new(), 0);
    for &[mut start, end] in &segment.spans {
        while start < end {
            let mut stop = end.min(start + CHUNK);
            for one in &fixups {
                if one.at < stop && stop < one.at + WIDE[&one.loc] {
                    stop = one.at;
                }
            }
            out.push(omf::ledata_record(index as i64, start as i64, &segment.image[start..stop])?);
            let inside: Vec<&Fixup> = fixups.iter().filter(|one| start <= one.at && one.at < stop).collect();
            if !inside.is_empty() {
                out.push(omf::fixupp_record(
                    &inside.iter().map(|one| _located(&subrecords[&one.at], one, start)).collect::<Vec<_>>(),
                ));
            }
            placed += inside.len();
            start = stop;
        }
    }
    if placed != fixups.len() || subrecords.len() != fixups.len() {
        return Err(Unencodable(format!("{}: a fixup outside the data, or two in one field", segment.name)).into());
    }
    Ok(out)
}

/// Everything after the location: fix data, frame datum, target datum.
///
/// `segments[segment]` is Python's `segment`, whose image this may patch.
pub fn _subrecord(
    one: &Fixup,
    segment: usize,
    segments: &mut [Segment],
    symbols: &IndexMap<String, (usize, usize)>,
    externs: &IndexMap<String, i64>,
    kinds: &IndexMap<String, String>,
) -> Result<Vec<u8>, omf::ValueError> {
    let name = _target(&one.name);
    let (method, datum, grouped);
    if name == "DGROUP" {
        (method, datum, grouped) = (GROUP_TARGET, GROUP, true);
    } else if let Some(&(seg, at)) = symbols.get(name) {
        (method, datum, grouped) = (SEGMENT_TARGET, seg as i64 + 1, segments[seg].grouped);
        if (one.loc == OFFSET || one.loc == POINTER || one.loc == OFFSET32) && !one.relative {
            let image = &mut segments[segment].image;
            let addend = field(image, one.at, one.loc);
            pack_field(image, one.at, one.loc, addend + at as i64);
        }
    } else {
        let index = *externs.get(name).unwrap_or_else(|| panic!("KeyError: {}", pyrepr::string(name)));
        (method, datum, grouped) = (EXTERNAL_TARGET, index, kinds[name] == "byte");
    }
    if (one.loc == OFFSET || one.loc == OFFSET32) && grouped && !one.relative {
        return Ok([vec![GROUP_FRAME << 4 | 4 | method], omf::as_index(GROUP)?, omf::as_index(datum)?].concat());
    }
    Ok([vec![TARGET_FRAME << 4 | 4 | method], omf::as_index(datum)?].concat())
}

pub fn _located(subrecord: &[u8], one: &Fixup, start: usize) -> Vec<u8> {
    let offset = one.at - start;
    let lead = 0x80 | if one.relative { 0 } else { 0x40 } | (one.loc as usize) << 2 | offset >> 8;
    [&[lead as u8, (offset & 0xFF) as u8][..], subrecord].concat()
}

pub fn _target(name: &str) -> &str {
    name.strip_prefix("seg ").unwrap_or(name)
}

pub fn _names(indices: &[i64]) -> Result<Vec<u8>, omf::ValueError> {
    Ok(indices.iter().map(|one| omf::as_index(*one)).collect::<Result<Vec<_>, _>>()?.concat())
}

pub fn _string(text: &str) -> Vec<u8> {
    let encoded: Vec<u8> = text
        .chars()
        .map(|one| u8::try_from(u32::from(one)).unwrap_or_else(|_| panic!("UnicodeEncodeError: 'latin-1' codec")))
        .collect();
    let length = u8::try_from(encoded.len()).unwrap_or_else(|_| panic!("ValueError: bytes must be in range(0, 256)"));
    [vec![length], encoded].concat()
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_omfwrite.py`'s C-path tests, and both emitters
    //! against Python on hand-built modules.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use iced_x86::Register;

    use super::*;
    use crate::model::ir::Addr;
    use crate::model::lir;

    fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
        Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
    }

    fn targeted(op: Operation, name: &str, target: i64) -> Semantics {
        Semantics { target: Some(target), ..semantics(op, name, vec![], vec![]) }
    }

    fn insn(at: i64, what: Semantics) -> Arc<lir::Insn> {
        Arc::new(lir::Insn::new(at, Some((at, 1)), Some(what), vec![], vec![]))
    }

    fn block(at: i64, insns: Vec<Arc<lir::Insn>>, succ: Vec<i64>) -> lir::LirBlock {
        lir::LirBlock { succ, ..lir::LirBlock::new(at, insns) }
    }

    fn body(name: &str, blocks: Vec<lir::LirBlock>) -> lir::LirBody {
        lir::LirBody::new(name, 1, blocks, IndexMap::default(), IndexMap::default())
    }

    fn procedure(name: &str, far: bool, body: lir::LirBody, reserve: i64, callees: Vec<(i64, masm::Callee)>) -> masm::Procedure {
        masm::Procedure { name: name.into(), public: true, far, body, reserve, callees: callees.into_iter().collect(), interrupt: None }
    }

    fn reg(register: Register) -> Loc {
        Loc::Reg(ir::Reg { register, width: 2 })
    }

    fn imm(value: i64, address: Option<Addr>) -> Loc {
        Loc::Imm(ir::Imm { value, width: 2, address })
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|one| (*one).to_owned()).collect()
    }

    fn label(name: &str) -> masm::Datum {
        masm::Datum::Label(masm::Label { name: name.into() })
    }

    fn pointer(name: &str, offset: i64, far: bool) -> masm::Datum {
        masm::Datum::Pointer(masm::Pointer { name: name.into(), offset, far })
    }

    fn fill(size: i64, byte: Option<u8>) -> masm::Datum {
        masm::Datum::Fill(masm::Fill { size, byte })
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
    }

    /// Fresh QB D_SURF retained 83 jumps whose target label was physically next.
    ///
    /// SC_INIT alone printed ``jmp L21_2`` immediately before ``L21_2``. A
    /// frontend is allowed to present explicit CFG edges; final emission owns the
    /// physical block order and must not encode an unconditional edge that has
    /// become fall-through.
    #[test]
    fn test_fresh_emission_omits_an_explicit_jump_to_the_next_block() {
        let jump = insn(1, targeted(Operation::Jump, "jmp", 2));
        let anchor = insn(1, semantics(Operation::Nothing, "", vec![], vec![]));
        let returned = insn(2, semantics(Operation::Return, "ret", vec![], vec![]));
        let body = body("next", vec![block(1, vec![jump, anchor], vec![2]), block(2, vec![returned], vec![])]);
        let procedure = procedure("_next", false, body, 0, vec![]);

        let lines: Vec<String> =
            masm::_procedure(&procedure, &IndexMap::default(), 0).unwrap().iter().map(|one| one.trim().to_owned()).collect();
        assert_eq!(lines, ["_next proc near", "L0_1:", "L0_2:", "ret", "_next endp"]);
    }

    /// The pass measured each item after a jump in it had grown, against labels
    /// from before: a backward branch whose target also moved looked a byte out of
    /// reach. 18 of 38 qcport objects came out longer than jwasm's.
    #[test]
    fn test_a_jump_growing_before_a_backward_target_leaves_that_branch_short() {
        let mut items = vec![
            Encoded::Jump(Jump::new("jmp", "far")),
            Encoded::Label(masm::Label { name: "top".into() }),
            Encoded::Piece(Piece::new(vec![0; 126])),
            Encoded::Jump(Jump::new("jmp", "top")),
            Encoded::Piece(Piece::new(vec![0; 200])),
            Encoded::Label(masm::Label { name: "far".into() }),
        ];
        let labels = _relaxed(&mut items).unwrap();
        let long = |item: &Encoded| matches!(item, Encoded::Jump(Jump { long: true, .. }));
        assert_eq!((long(&items[0]), long(&items[3])), (true, false));
        assert_eq!(labels["far"], 3 + 126 + 2 + 200);
    }

    /// The module of `test_externals_are_declared_in_the_order_jwasm_declares_them`,
    /// whose jwasm half is deferred: text and object bytes as Python writes them.
    #[test]
    fn test_externals_module_matches_python() {
        let load = semantics(
            Operation::Move,
            "mov",
            vec![reg(Register::AX)],
            vec![Loc::Mem(ir::Mem::new(Some(Addr { index: 7, ..Addr::new(Space::External, 0) }), 2))],
        );
        let call = semantics(Operation::Call, "call", vec![], vec![]);
        let leave = semantics(Operation::Return, "retf", vec![], vec![]);
        let insns = vec![insn(1, load), insn(2, call), insn(3, leave)];
        let built = masm::Module {
            code: "GET_TEXT".into(),
            names: IndexMap::from_iter([((Space::External, 7), "_d".to_owned())]),
            externs: vec![("_f".into(), "far".into()), ("_d".into(), "byte".into())],
            publics: strings(&["_get"]),
            data: vec![("_DATA".into(), vec![])],
            procedures: vec![procedure(
                "_get",
                true,
                body("get", vec![lir::LirBlock::new(1, insns)]),
                0,
                vec![(2, masm::Callee::new("_f", true))],
            )],
            private: BTreeSet::new(),
            requests: BTreeSet::new(),
        };
        assert_eq!(
            masm::text(&built).unwrap(),
            ".model medium\n.386\n\npublic _get\n.data\nextern _d:byte\nextern _f:far\n.code GET_TEXT\n_get proc far\n\
             L0_1:\n    mov ax, word ptr _d\n    call far ptr _f\n    retf\n_get endp\nend\n"
        );
        assert_eq!(
            written(&built, "get.c").unwrap(),
            hex("800700056765742e63a39622000004434f4445084745545f544558540444415441055f44415441064447524f55502a98\
                 07004809000302010a9807004800000504010f9a040006ff025b8c0900025f6400025f6600df900b000001045f676574\
                 000000c1a00d00010000a100009a00000000cb4c9c0a00c401160101cc045602558a02000074")
        );
    }

    /// Every datum kind, a private and a grouped extra segment, a reserved
    /// frame, a saved SI, a branch, a backward jump, a near call within the
    /// module and inline code: text and object bytes as Python writes them.
    #[test]
    fn test_rich_module_matches_python() {
        let ax = reg(Register::AX);
        let cell = Loc::Mem(ir::Mem::new(Some(Addr { index: 1, ..Addr::new(Space::Segment, 2) }), 2));
        let frame = Loc::Mem(ir::Mem { through: Register::BP, ..ir::Mem::new(Some(Addr::new(Space::Frame, 6)), 2) });
        let first = vec![
            insn(1, semantics(Operation::Move, "mov", vec![ax.clone()], vec![frame])),
            insn(2, semantics(Operation::Compare, "cmp", vec![], vec![ax.clone(), imm(3, None)])),
            insn(3, targeted(Operation::Branch, "je", 20)),
        ];
        let table = Some(Addr { index: 1, ..Addr::new(Space::Segment, 4) });
        let second = vec![
            insn(10, semantics(Operation::Move, "mov", vec![cell], vec![ax.clone()])),
            insn(11, semantics(Operation::Move, "mov", vec![reg(Register::BX)], vec![imm(0, table)])),
            insn(12, semantics(Operation::Call, "call", vec![], vec![])),
            insn(13, targeted(Operation::Jump, "jmp", 1)),
        ];
        let group = Some(Addr { index: 1, ..Addr::new(Space::Group, 0) });
        let third = vec![
            insn(20, semantics(Operation::Call, "call", vec![], vec![])),
            insn(21, semantics(Operation::Move, "mov", vec![reg(Register::ES)], vec![imm(0, group)])),
            insn(22, semantics(Operation::Return, "", vec![], vec![])),
        ];
        let f = body("f", vec![block(1, first, vec![10, 20]), block(10, second, vec![1]), block(20, third, vec![])]);
        let helper = vec![
            insn(1, semantics(Operation::Move, "mov", vec![reg(Register::SI)], vec![ax])),
            insn(2, semantics(Operation::Return, "ret", vec![], vec![])),
        ];
        let inline = vec![
            masm::InlinePart::Bytes(vec![0x90, 0x90]),
            masm::InlinePart::Fixup("offset".into(), "_table".into(), 2),
            masm::InlinePart::Fixup("segment".into(), "_far".into(), 0),
        ];
        let rich = masm::Module {
            code: "RICH_TEXT".into(),
            names: IndexMap::from_iter([((Space::Segment, 1), "_table".to_owned()), ((Space::Group, 1), "DGROUP".to_owned())]),
            externs: vec![("_ext".into(), "far".into()), ("_unused".into(), "near".into()), ("_b".into(), "byte".into())],
            publics: strings(&["_f", "_table"]),
            data: vec![
                (
                    "_DATA".into(),
                    vec![
                        label("_table"),
                        masm::Datum::Bytes(vec![1, 2, 3]),
                        masm::Datum::Align(masm::Align { to: 4 }),
                        fill(3, Some(7)),
                        pointer("_table", 2, false),
                        pointer("_b", 0, true),
                    ],
                ),
                ("_BSS".into(), vec![label("_zero"), fill(5, None)]),
                ("FAR_SEG".into(), vec![label("_far"), masm::Datum::Bytes(b"xyz".repeat(7))]),
                ("SHARED".into(), vec![pointer("_far", 1, false)]),
            ],
            procedures: vec![
                masm::Procedure { public: false, ..procedure("_h", false, body("h", vec![lir::LirBlock::new(1, helper)]), 0, vec![]) },
                procedure(
                    "_f",
                    true,
                    f,
                    3,
                    vec![
                        (12, masm::Callee::new("_h", false)),
                        (20, masm::Callee { code: inline, ..masm::Callee::new("_ext", false) }),
                    ],
                ),
            ],
            private: BTreeSet::from(["FAR_SEG".to_owned()]),
            requests: BTreeSet::new(),
        };
        assert_eq!(
            masm::text(&rich).unwrap(),
            ".model medium\n.386\n\npublic _f\npublic _table\n.data\nextern _b:byte\n_table label byte\n\
             db 001h,002h,003h\n    align 4\n    db 3 dup (7)\n    dw _table+2\n    dd _b\n.data?\nextern _b:byte\n\
             _zero label byte\n    db 5 dup (?)\nFAR_SEG segment word public 'FAR_DATA'\nextern _b:byte\n\
             _far label byte\ndb 078h,079h,07ah,078h,079h,07ah,078h,079h,07ah,078h,079h,07ah,078h,079h,07ah,078h\n\
             db 079h,07ah,078h,079h,07ah\nFAR_SEG ends\nSHARED segment word public 'DATA'\nextern _b:byte\n\
             \x20   dw _far+1\nSHARED ends\nDGROUP group SHARED\nextern _ext:far\nextern _unused:near\n.code RICH_TEXT\n\
             _h proc near\n    push si\nL0_1:\n    mov si, ax\n    pop si\n    ret\n_h endp\n_f proc far\n    push bp\n\
             \x20   mov bp, sp\n    sub sp, 4\nL1_1:\n    mov ax, word ptr [bp+6]\n    cmp ax, 3\n    je L1_20\nL1_10:\n\
             \x20   mov word ptr _table+2, ax\n    mov bx, offset _table+4\n    call _h\n    jmp L1_1\nL1_20:\n\
             \x20   db 090h,090h\n    dw offset _table+2\n    dw seg _far\n    pushw DGROUP\n    pop es\n    leave\n\
             \x20   retf\n_f endp\nend\n"
        );
        assert_eq!(
            written(&rich, "rich.c").unwrap(),
            hex("80080006726963682e633b9649000004434f444509524943485f544558540444415441055f4441544103425353045f\
                 425353084641525f44415441074641525f534547044441544106534841524544064447524f555033980700482a0003\
                 0201e9980700480d000504010298070048050007060106980700481500090801f29807004802000b0a01019a08000c\
                 ff02ff03ff054b8c0500025f6200ac9009000001025f660500009a900d000102065f7461626c65000000f3a02e0001\
                 0000568bf05ec3558bec83ec048b460683f803740ba30200bb0400e8e4ffebed90900200000068000007c9cb009c18\
                 00c414140102c417140102c420140102c8225404c8255501eba0110002000001020300070707020000000000309c0a\
                 00c407140102cc0956014ca0190004000078797a78797a78797a78797a78797a78797a78797a56a006000500000100\
                 549c0500c4005404438a02000074")
        );
    }

    fn bc_emitted(path: &str) -> crate::wholeseg::Emitted {
        crate::wholeseg::emitted(
            &std::fs::read(path).unwrap(),
            true,
            true,
            None,
            None,
            crate::backend::cpu::ProfileOrName::Name("386"),
            false,
            false,
            None,
            &crate::model::passes::O2(),
        )
        .unwrap()
    }

    /// hotlop used to finish by rewriting BC's record stream in place. BC's
    /// first FIXUPP defines THREAD state; a serializer built from decoded
    /// relocations emits every relocation explicitly.
    #[test]
    fn test_the_bc_frontend_uses_the_fresh_object_writer() {
        let source = std::fs::read(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/hotlop-p-g2.obj")).unwrap();
        let got = bc_emitted(concat!(env!("LLRM_ROOT"), "/tests/fixtures/omf/hotlop-p-g2.obj"));
        assert_eq!(got.outcome, crate::wholeseg::Emission::Lir);
        let records = omf::parse(&got.data).unwrap();
        assert_eq!(records[0].r#type, omf::THEADR);
        let fixupps = |records: &[Rc<omf::Record>]| {
            records.iter().filter(|one| one.r#type & 0xFE == omf::FIXUPP).map(|one| one.body[0]).collect::<Vec<u8>>()
        };
        assert!(fixupps(&omf::parse(&source).unwrap()).iter().any(|lead| lead & 0x80 == 0));
        assert!(fixupps(&records).iter().all(|lead| lead & 0x80 != 0));
    }

    /// A 32-bit symbolic address carries a disp32; an OFFSET fixup relocated
    /// only its low word and left the high word of the addend in place.
    #[test]
    fn test_a_wide_symbolic_address_takes_an_offset32_fixup() {
        let cell = ir::Mem {
            addr: Some(Addr { index: 3, ..Addr::new(Space::Segment, 1280) }),
            index: Some(ir::Held { value: 1, width: 4 }),
            index_through: Register::ESI,
            scale: 2,
            ..ir::Mem::new(None, 2)
        };
        let what = semantics(Operation::Move, "mov", vec![Loc::Reg(ir::Reg { register: Register::CX, width: 2 })], vec![Loc::Mem(cell)]);
        let names = IndexMap::from_iter([((Space::Segment, 3), "S%".to_owned())]);

        let piece = _encoded(&what, &names).unwrap();

        let [fixup] = piece.fixups.as_slice() else { panic!("{:?}", piece.fixups) };
        assert_eq!((fixup.loc, fixup.at + 4, field(&piece.code, fixup.at, fixup.loc)), (OFFSET32, piece.code.len(), 1280));
    }

    /// NDMAX's 60-dimensional HARY expansion exceeded one LEDATA and was refused.
    #[test]
    fn test_expanded_operation_can_cross_records_without_splitting_fixups() {
        let chunk = CHUNK as i64;
        let size = chunk * 3;
        let starts = [chunk - 1, 2 * chunk - 2];
        let mut fixups = Vec::new();
        for at in starts {
            let mut one = omf::target_offset_fixup(1, at, "segment", 1, 0).unwrap();
            one.loc = omf::LOC_PTR32;
            fixups.push((at, one, 0));
        }
        let records = _fresh_segment(1, &vec![0; size as usize], &[(0, size)], &mut fixups).unwrap().unwrap();
        let chunks: Vec<(i64, i64)> =
            omf::ledata(&records).into_iter().map(|(_record, _seg, at, data)| (at, at + data.len() as i64)).collect();
        assert!(chunks[0].0 == 0 && chunks.last().unwrap().1 == size);
        assert!(chunks.iter().all(|(left, right)| 0 < right - left && right - left <= chunk));
        assert!(!chunks.iter().any(|(_, cut)| starts.iter().any(|low| low < cut && *cut < low + 4)));
    }
}
