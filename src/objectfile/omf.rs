//! Port of `qbopt/objectfile/omf.py`: read and write Intel OMF object files
//! -- the format BC hands to LINK.
//!
//! Records are: type byte, then a length word covering the body AND a
//! trailing checksum byte, so a record occupies 3 + length bytes. Records are
//! kept as raw bodies so a file that is read and written again is identical
//! byte for byte.
//!
//! A Python `list[Record]` holds references, and callers compare them by
//! identity (`is`, `id()`), so records are carried as `Rc<Record>`.

use crate::support::hash::HashSet;
use std::fmt;
use std::path::Path;
use std::rc::Rc;
use std::sync::LazyLock;

use crate::support::hash::IndexMap;

use crate::support::pyrepr::{self, Repr};

pub const THEADR: u8 = 0x80;
pub const COMENT: u8 = 0x88;
pub const MODEND: u8 = 0x8A;
pub const EXTDEF: u8 = 0x8C;
pub const PUBDEF: u8 = 0x90;
pub const LINNUM: u8 = 0x94;
pub const LNAMES: u8 = 0x96;
pub const SEGDEF: u8 = 0x98;
pub const GRPDEF: u8 = 0x9A;
pub const FIXUPP: u8 = 0x9C;
pub const LEDATA: u8 = 0xA0;
pub const LIDATA: u8 = 0xA2;

pub static NAMES: LazyLock<IndexMap<u8, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (0x80, "THEADR"),
        (0x88, "COMENT"),
        (0x8A, "MODEND"),
        (0x8B, "MODEND32"),
        (0x8C, "EXTDEF"),
        (0x90, "PUBDEF"),
        (0x91, "PUBDEF32"),
        (0x94, "LINNUM"),
        (0x95, "LINNUM32"),
        (0x96, "LNAMES"),
        (0x98, "SEGDEF"),
        (0x99, "SEGDEF32"),
        (0x9A, "GRPDEF"),
        (0x9C, "FIXUPP"),
        (0x9D, "FIXUPP32"),
        (0xA0, "LEDATA"),
        (0xA1, "LEDATA32"),
        (0xA2, "LIDATA"),
        (0xA3, "LIDATA32"),
        (0xB0, "COMDEF"),
        (0xB4, "LEXTDEF"),
        (0xB6, "LPUBDEF"),
        (0xB8, "LCOMDEF"),
        (0x8E, "TYPDEF"),
    ])
});

/// Python's `ValueError`, carrying its message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueError(pub String);

impl fmt::Display for ValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ValueError {}

/// What `read` can raise: the file's own `OSError`, or `parse`'s `ValueError`.
#[derive(Debug)]
pub enum ReadError {
    OSError(std::io::Error),
    ValueError(ValueError),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::OSError(error) => write!(f, "{error}"),
            ReadError::ValueError(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ReadError {}

#[derive(Clone, Debug)]
pub struct Record {
    pub r#type: u8,
    pub body: Vec<u8>,
    // Exactly what was read, where this record was read rather than built.
    // Kept so an untouched record comes back untouched even where the
    // checksum is not the one this would compute: BC writes a FIXUPP after
    // a READ statement whose byte is not the sum. Self-checking -- the body
    // has to still match, so a modified record recomputes.
    pub raw: Option<Vec<u8>>,
}

/// `raw` is `field(compare=False)`.
impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.r#type == other.r#type && self.body == other.body
    }
}

impl Eq for Record {}

impl Record {
    pub fn new(r#type: u8, body: Vec<u8>) -> Record {
        Record {
            r#type,
            body,
            raw: None,
        }
    }

    pub fn name(&self) -> String {
        match NAMES.get(&self.r#type) {
            Some(name) => (*name).to_owned(),
            None => format!("{:02X}", self.r#type),
        }
    }

    pub fn emit(&self) -> Vec<u8> {
        // the checksum byte makes the record's bytes sum to zero mod 256;
        // a zero byte is also accepted and is what many tools write
        let body = &self.body;
        let head = [self.r#type]
            .into_iter()
            .chain(pack(body.len() as i64 + 1))
            .collect::<Vec<u8>>();
        if let Some(raw) = &self.raw {
            if slice(raw, 0, 3) == head.as_slice()
                && slice(raw, 3, raw.len().saturating_sub(1)) == body.as_slice()
            {
                return raw.clone();
            }
        }
        let total = head
            .iter()
            .chain(body.iter())
            .map(|&byte| byte as i64)
            .sum::<i64>();
        let mut out = head;
        out.extend_from_slice(body);
        out.push((-total & 0xFF) as u8);
        out
    }
}

impl Repr for Record {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Record",
            &[
                ("type", self.r#type.to_string()),
                ("body", pyrepr::bytes(&self.body)),
                (
                    "raw",
                    self.raw.as_deref().map_or("None".to_owned(), pyrepr::bytes),
                ),
            ],
        )
    }
}

impl Repr for Rc<Record> {
    fn repr(&self) -> String {
        (**self).repr()
    }
}

/// Every record in the file, in order.
pub fn read(path: impl AsRef<Path>) -> Result<Vec<Rc<Record>>, ReadError> {
    let data = std::fs::read(path).map_err(ReadError::OSError)?;
    parse(&data).map_err(ReadError::ValueError)
}

pub fn parse(d: &[u8]) -> Result<Vec<Rc<Record>>, ValueError> {
    let (mut out, mut i) = (Vec::new(), 0usize);
    while i + 3 <= d.len() {
        let (t, n) = (d[i], unpack_from(d, i + 1) as usize);
        if i + 3 + n > d.len() + 1 {
            return Err(ValueError(format!("record at {i} runs past the end")));
        }
        // body, less the checksum
        out.push(Rc::new(Record {
            r#type: t,
            body: slice(d, i + 3, i + 2 + n).to_vec(),
            raw: Some(slice(d, i, i + 3 + n).to_vec()),
        }));
        i += 3 + n;
    }
    if i != d.len() {
        return Err(ValueError(format!(
            "{} trailing bytes",
            d.len() as i64 - i as i64
        )));
    }
    Ok(out)
}

/// The named object modules in an OMF library, or no modules for an OBJ.
///
/// A library begins with its F0 header and aligns every THEADR..MODEND object
/// module to the page size recorded there. Its F1 dictionary is not an OMF
/// record stream and is deliberately not parsed.
pub fn library_modules(data: &[u8]) -> Result<Vec<(String, Vec<Rc<Record>>)>, ValueError> {
    if data.is_empty() || data[0] != 0xF0 {
        return Ok(Vec::new());
    }
    if data.len() < 3 {
        return Err(ValueError("truncated OMF library header".to_owned()));
    }
    let payload = unpack_from(data, 1) as usize;
    let page = payload + 3;
    if payload == 0 || page > data.len() {
        return Err(ValueError("invalid OMF library page size".to_owned()));
    }

    let mut modules: Vec<(String, Vec<Rc<Record>>)> = Vec::new();
    let mut current: Vec<Rc<Record>> = Vec::new();
    let mut name: Option<String> = None;
    let mut at = page;
    while at + 3 <= data.len() {
        let kind = data[at];
        if kind == 0xF1 {
            break;
        }
        let size = unpack_from(data, at + 1) as usize;
        if size == 0 || at + 3 + size > data.len() {
            return Err(ValueError(format!("invalid OMF library record at {at}")));
        }
        let raw = &data[at..at + 3 + size];
        let body = &data[at + 3..at + 2 + size];
        if kind == THEADR {
            if !current.is_empty() {
                return Err(ValueError("OMF library module has no MODEND".to_owned()));
            }
            if body.is_empty() || body.len() != body[0] as usize + 1 {
                return Err(ValueError("invalid OMF library THEADR".to_owned()));
            }
            name = Some(decode_latin1(&body[1..]));
        }
        if name.is_none() {
            return Err(ValueError(format!(
                "OMF library record at {at} precedes THEADR"
            )));
        }
        current.push(Rc::new(Record {
            r#type: kind,
            body: body.to_vec(),
            raw: Some(raw.to_vec()),
        }));
        at += 3 + size;
        if kind == MODEND || kind == MODEND | 1 {
            modules.push((name.take().unwrap(), std::mem::take(&mut current)));
            at = at.div_ceil(page) * page;
        }
    }
    if !current.is_empty() {
        return Err(ValueError("truncated OMF library module".to_owned()));
    }
    if modules.is_empty() {
        return Err(ValueError("OMF library contains no modules".to_owned()));
    }
    Ok(modules)
}

pub fn write(path: impl AsRef<Path>, recs: &[Rc<Record>]) -> std::io::Result<()> {
    std::fs::write(
        path,
        recs.iter().flat_map(|r| r.emit()).collect::<Vec<u8>>(),
    )
}

/// An OMF index: one byte under 128, otherwise two with the top bit set.
pub fn _index(b: &[u8], i: usize) -> (i64, usize) {
    if b[i] & 0x80 != 0 {
        return ((((b[i] & 0x7F) as i64) << 8) | b[i + 1] as i64, i + 2);
    }
    (b[i] as i64, i + 1)
}

/// The LNAMES strings, 1-based as every other record refers to them.
pub fn names(recs: &[Rc<Record>]) -> Vec<String> {
    let mut out = vec![String::new()];
    for r in recs {
        if r.r#type & 0xFE == LNAMES {
            let mut i = 0;
            while i < r.body.len() {
                let n = r.body[i] as usize;
                out.push(decode_latin1(slice(&r.body, i + 1, i + 1 + n)));
                i += 1 + n;
            }
        }
    }
    out
}

/// SEGDEFs as (name, length), 1-based by segment index.
pub fn segments(recs: &[Rc<Record>]) -> Vec<Option<(String, i64)>> {
    let (nm, mut out) = (names(recs), vec![None]);
    for r in recs {
        if r.r#type & 0xFE != SEGDEF {
            continue;
        }
        let (acbp, mut i) = (r.body[0], 1);
        if (acbp >> 5) == 0 {
            // absolute: frame and offset follow
            i += 3;
        }
        let mut ln = unpack_from(&r.body, i);
        if (acbp & 0x02) != 0 && ln == 0 {
            // the big bit: a full 64K
            ln = 0x10000;
        }
        i += 2;
        let (ni, _) = _index(&r.body, i);
        let name = if (ni as usize) < nm.len() {
            nm[ni as usize].clone()
        } else {
            "?".to_owned()
        };
        out.push(Some((name, ln)));
    }
    out
}

pub const COMBINE_COMMON: i64 = 6;

/// SEGDEF combine types, by 1-based segment index.
///
/// COMMON (6) lays every object's copy of the segment over the same bytes;
/// PUBLIC (2, 4, 7) concatenates them and private (0) keeps them apart.
pub fn combines(recs: &[Rc<Record>]) -> IndexMap<i64, i64> {
    let (mut out, mut index) = (IndexMap::default(), 0i64);
    for r in recs {
        if r.r#type & 0xFE != SEGDEF {
            continue;
        }
        index += 1;
        out.insert(index, ((r.body[0] >> 2) & 7) as i64);
    }
    out
}

/// GRPDEF's own segment membership, by group name.
///
/// A GRPDEF is a group-name index, then repeated (0xFF, segment-index)
/// pairs -- 0xFF is the only component type BC emits, "segment index".
pub fn groups(recs: &[Rc<Record>]) -> IndexMap<String, Vec<i64>> {
    let nm = names(recs);
    let mut out: IndexMap<String, Vec<i64>> = IndexMap::default();
    for r in recs {
        if r.r#type & 0xFE != GRPDEF {
            continue;
        }
        let (gi, mut i) = _index(&r.body, 0);
        let mut members = Vec::new();
        while i < r.body.len() {
            i += 1; // the component-type byte, always 0xFF
            let (si, next) = _index(&r.body, i);
            i = next;
            members.push(si);
        }
        let name = if (gi as usize) < nm.len() {
            nm[gi as usize].clone()
        } else {
            "?".to_owned()
        };
        out.insert(name, members);
    }
    out
}

/// EXTDEF names, 1-based -- FIXUPP targets refer to these by index.
pub fn externals(recs: &[Rc<Record>]) -> Vec<String> {
    let mut out = vec![String::new()];
    for r in recs {
        if r.r#type & 0xFE != EXTDEF {
            continue;
        }
        let mut i = 0;
        while i < r.body.len() {
            let n = r.body[i] as usize;
            out.push(decode_latin1(slice(&r.body, i + 1, i + 1 + n)));
            i += 1 + n;
            (_, i) = _index(&r.body, i); // the type index, unused here
        }
    }
    out
}

/// PUBDEF's own name for each offset it declares into segment `seg`.
///
/// Mirrors code_offsets()'s PUBDEF branch, but keeps the name it skips past.
pub fn pubdef_names(records: &[Rc<Record>], seg: i64) -> Result<IndexMap<i64, String>, ValueError> {
    Ok(public_definitions(records)?
        .into_iter()
        .filter(|(_name, (segment, _offset))| *segment == seg)
        .map(|(name, (_segment, offset))| (offset, name))
        .collect())
}

/// Every externally visible PUBDEF as `name -> (segment, offset)`.
///
/// Local PUBDEFs do not satisfy an EXTDEF and are deliberately absent.
pub fn public_definitions(
    records: &[Rc<Record>],
) -> Result<IndexMap<String, (i64, i64)>, ValueError> {
    let mut out: IndexMap<String, (i64, i64)> = IndexMap::default();
    for r in records {
        if r.r#type & 0xFE != PUBDEF {
            continue;
        }
        let body = &r.body;
        let (_group, mut at) = _index(body, 0);
        let base;
        (base, at) = _index(body, at);
        if base == 0 {
            // an absolute segment names its frame instead
            at += 2;
        }
        let width = if r.r#type == PUBDEF + 1 { 4 } else { 2 };
        while at < body.len() {
            let namelen = body[at] as usize;
            let name = decode_latin1(slice(body, at + 1, at + 1 + namelen));
            at += 1 + namelen;
            if at + width > body.len() {
                return Err(ValueError("truncated PUBDEF offset".to_owned()));
            }
            let offset = body[at..at + width]
                .iter()
                .rev()
                .fold(0i64, |sum, &byte| (sum << 8) | byte as i64);
            at += width;
            (_, at) = _index(body, at); // the type index, unused here
            if out.contains_key(&name) {
                return Err(ValueError(format!(
                    "duplicate PUBDEF {} in one object module",
                    pyrepr::string(&name)
                )));
            }
            out.insert(name, (base, offset));
        }
    }
    Ok(out)
}

/// The EXTDEF at `index` given a different name, its own index untouched.
///
/// Nothing anywhere names an EXTDEF by its bytes -- only FIXUPP subrecords and
/// THREAD definitions do, and always by this ordinal index.
pub fn rename_external(
    records: &[Rc<Record>],
    index: i64,
    name: &str,
) -> Result<Vec<Rc<Record>>, ValueError> {
    let mut seen = 0i64;
    let mut out = Vec::new();
    for r in records {
        if r.r#type & 0xFE != EXTDEF {
            out.push(r.clone());
            continue;
        }
        let (mut body, mut i, mut changed) = (Vec::new(), 0usize, false);
        while i < r.body.len() {
            let n = r.body[i] as usize;
            seen += 1;
            let entry_name = if seen == index {
                encode_latin1(name)
            } else {
                slice(&r.body, i + 1, i + 1 + n).to_vec()
            };
            let j = i + 1 + n;
            let (_, k) = _index(&r.body, j);
            body.extend(bytes(&[entry_name.len() as i64]));
            body.extend_from_slice(&entry_name);
            body.extend_from_slice(slice(&r.body, j, k));
            changed = changed || seen == index;
            i = k;
        }
        out.push(if changed {
            Rc::new(Record::new(r.r#type, body))
        } else {
            r.clone()
        });
    }
    if seen < index {
        return Err(ValueError(format!("only {seen} EXTDEFs; no entry {index}")));
    }
    Ok(out)
}

/// Each LEDATA as (record, segment index, offset, bytes).
pub fn ledata(recs: &[Rc<Record>]) -> Vec<(Rc<Record>, i64, i64, Vec<u8>)> {
    let mut out = Vec::new();
    for r in recs {
        if r.r#type & 0xFE != LEDATA {
            continue;
        }
        let (si, i) = _index(&r.body, 0);
        let off = unpack_from(&r.body, i);
        out.push((
            r.clone(),
            si,
            off,
            slice(&r.body, i + 2, r.body.len()).to_vec(),
        ));
    }
    out
}

pub const LOC_LOBYTE: i64 = 0;
pub const LOC_OFF16: i64 = 1;
pub const LOC_BASE: i64 = 2;
pub const LOC_PTR32: i64 = 3;
pub const LOC_HIBYTE: i64 = 4;
pub const LOC_OFF32: i64 = 9;

pub static TARGET_KIND: LazyLock<IndexMap<i64, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([(0, "segment"), (1, "group"), (2, "external")]));

pub static LOCNAME: LazyLock<IndexMap<i64, &'static str>> = LazyLock::new(|| {
    IndexMap::from_iter([
        (0, "lobyte"),
        (1, "offset16"),
        (2, "base"),
        (3, "ptr16:16"),
        (4, "hibyte"),
        (5, "offset16(ldr)"),
        (9, "offset32"),
        (11, "ptr16:32"),
        (13, "offset32(ldr)"),
    ])
});

/// A frame or target a later fixup refers to by number.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Thread {
    pub method: i64,
    pub index: i64,
}

impl Repr for Thread {
    fn repr(&self) -> String {
        pyrepr::dataclass(
            "Thread",
            &[("method", self.method.repr()), ("index", self.index.repr())],
        )
    }
}

/// `Fixup.frame`'s `Thread | int`; `None` is the `Option` around it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Frame {
    Thread(Thread),
    Int(i64),
}

impl Repr for Frame {
    fn repr(&self) -> String {
        match self {
            Frame::Thread(thread) => thread.repr(),
            Frame::Int(value) => value.repr(),
        }
    }
}

/// One relocation, resolved to where in the segment it patches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixup {
    pub seg: Option<i64>,
    pub offset: i64,
    pub loc: i64,
    pub selfrel: bool,
    pub target: String,
    pub index: i64,
    // In an object a label's address is not in the code -- the bytes there are
    // zero and this is where the offset lives.
    pub disp: i64,
    pub frame: Option<Frame>,
    pub record: Rc<Record>,
    pub lo: usize,
    pub hi: usize,
    pub disp_pos: Option<usize>,
    // Which of the frame methods this fixup used, where it named one
    // explicitly. `frame` alone is an index and cannot say whether it
    // counts segments, groups or externals, and pruning an EXTDEF has to
    // know which of the three it is looking at.
    pub frame_method: Option<i64>,
}

impl Fixup {
    pub fn raw(&self) -> &[u8] {
        slice(&self.record.body, self.lo, self.hi)
    }
}

impl Repr for Fixup {
    fn repr(&self) -> String {
        let loc = match LOCNAME.get(&self.loc) {
            Some(name) => (*name).to_owned(),
            None => self.loc.to_string(),
        };
        format!(
            "<{} {:04X} {} {} {}+{}>",
            self.seg.repr(),
            self.offset,
            loc,
            self.target,
            self.index,
            self.disp
        )
    }
}

/// (is a frame thread, its number, what it names, where the next starts).
pub fn read_thread(body: &[u8], mut at: usize) -> (bool, i64, Thread, usize) {
    let lead = body[at];
    at += 1;
    let (method, number) = (((lead >> 2) & 7) as i64, (lead & 3) as i64);
    let mut index = 0;
    // Frame methods 4 and 5 -- the location's segment, and the target's frame --
    // carry no index. Testing method & 3 says both of them do, and eats a byte
    // that is not there, which desynchronises the rest of the record.
    if method < 3 {
        (index, at) = _index(body, at);
    }
    (lead & 0x40 != 0, number, Thread { method, index }, at)
}

pub fn code_segment(records: &[Rc<Record>]) -> Option<(i64, String, i64)> {
    for (index, segment) in segments(records).into_iter().enumerate() {
        if let Some((name, length)) = segment {
            if name.ends_with("_CODE") {
                return Some((index as i64, name, length));
            }
        }
    }
    let headers: Vec<&Vec<u8>> = records
        .iter()
        .filter(|record| record.r#type == THEADR)
        .map(|record| &record.body)
        .collect();
    if headers.len() != 1 || headers[0].is_empty() {
        return None;
    }
    let header = headers[0];
    let lower = header[1..].to_ascii_lowercase();
    if header.len() != header[0] as usize + 1
        || !(lower.ends_with(b".c") || lower.ends_with(b".asm"))
    {
        return None;
    }
    let candidates: Vec<(i64, String, i64)> = segments(records)
        .into_iter()
        .enumerate()
        .filter_map(|(index, segment)| segment.map(|(name, length)| (index as i64, name, length)))
        .filter(|(_index, name, _length)| name.ends_with("_TEXT"))
        .collect();
    if candidates.len() == 1 {
        return candidates.into_iter().next();
    }
    None
}

/// The segment's bytes, with every LEDATA applied in file order.
///
/// BC emits overlapping LEDATA: short backpatch records arrive later in the
/// file at earlier offsets. The last write to a byte is the one that counts.
pub fn segment_image(records: &[Rc<Record>], seg: i64, size: i64) -> Vec<u8> {
    let mut image = vec![0u8; size as usize];
    for (_record, index, offset, payload) in ledata(records) {
        if index == seg {
            // bytearray slice assignment: clamped to the image, which grows
            let lo = (offset as usize).min(image.len());
            let hi = (offset as usize + payload.len()).min(image.len()).max(lo);
            image.splice(lo..hi, payload);
        }
    }
    image
}

// The odd-numbered twin of each record type is its 32-bit form. Every decoder
// here matches with & 0xFE, so it accepts them, and then reads them with 16-bit
// struct formats and fixed two-byte skips. BC emits none of them.
pub const WIDE: [u8; 7] = [
    MODEND + 1,
    PUBDEF + 1,
    LINNUM + 1,
    SEGDEF + 1,
    FIXUPP + 1,
    LEDATA + 1,
    LIDATA + 1,
];
pub const COMDAT: [u8; 2] = [0xC2, 0xC3];

/// Per byte of the segment, which LEDATA record finally wrote it.
///
/// The value is the record's `id()`.
pub fn last_writers(records: &[Rc<Record>], seg: i64, size: i64) -> IndexMap<i64, usize> {
    let mut owner = IndexMap::default();
    for (record, index, offset, payload) in ledata(records) {
        if index == seg {
            for at in offset..(offset + payload.len() as i64).min(size) {
                owner.insert(at, id(&record));
            }
        }
    }
    owner
}

/// Why this module must be left alone, if it must. Empty means it may be read.
pub fn refusals(records: &[Rc<Record>]) -> Vec<String> {
    let mut reasons = Vec::new();
    let kinds: HashSet<u8> = records.iter().map(|record| record.r#type).collect();
    if [LIDATA, LIDATA + 1].iter().any(|kind| kinds.contains(kind)) {
        // fixups() tracks its base from LEDATA only, so a FIXUPP after a LIDATA
        // is attributed to the previous LEDATA and comes out at the wrong offset
        reasons.push("LIDATA: fixup offsets after it would be wrong".to_owned());
    }
    if COMDAT.iter().any(|kind| kinds.contains(kind)) {
        reasons.push("COMDAT is not decoded".to_owned());
    }
    let wide: Vec<u8> = WIDE
        .iter()
        .copied()
        .filter(|kind| kinds.contains(kind))
        .collect();
    if !wide.is_empty() {
        let mut hexes: Vec<String> = wide.iter().map(|kind| format!("{kind:#x}")).collect();
        hexes.sort();
        reasons.push(format!(
            "32-bit records are decoded as 16-bit: {}",
            pyrepr::list(&hexes)
        ));
    }
    reasons
}

/// Every FIXUP subrecord, with its offset made absolute in the segment.
///
/// A FIXUPP's offsets are relative to the LEDATA it follows, which is why this
/// walks the records in order rather than gathering them by type. THREAD
/// subrecords set a default frame or target that later fixups refer to by
/// number.
pub fn fixups(records: &[Rc<Record>]) -> Vec<Fixup> {
    let mut found: Vec<Fixup> = Vec::new();
    let mut seg: Option<i64> = None;
    let mut base = 0i64;
    let mut frame_threads: [Option<Thread>; 4] = [None; 4];
    let mut target_threads: [Option<Thread>; 4] = [None; 4];

    for record in records {
        if record.r#type & 0xFE == LEDATA {
            let (segment_index, at) = _index(&record.body, 0);
            (seg, base) = (Some(segment_index), unpack_from(&record.body, at));
            continue;
        }
        if record.r#type & 0xFE != FIXUPP {
            continue;
        }

        let (body, mut at) = (&record.body, 0usize);
        while at < body.len() {
            let start = at;
            if body[at] & 0x80 == 0 {
                let (is_frame, number, thread, next) = read_thread(body, at);
                at = next;
                if is_frame {
                    frame_threads[number as usize] = Some(thread);
                } else {
                    target_threads[number as usize] = Some(thread);
                }
                continue;
            }

            let loc = ((body[at] >> 2) & 0x0F) as i64;
            let selfrel = body[at] & 0x40 == 0;
            let offset = (((body[at] & 0x03) as i64) << 8) | body[at + 1] as i64;
            at += 2;
            let fixdata = body[at];
            at += 1;

            let mut frame: Option<Frame> = None;
            let mut frame_method: Option<i64> = None;
            if fixdata & 0x80 != 0 {
                frame = frame_threads[((fixdata >> 4) & 3) as usize].map(Frame::Thread);
            } else if ((fixdata >> 4) & 7) < 3 {
                frame_method = Some(((fixdata >> 4) & 7) as i64);
                let value;
                (value, at) = _index(body, at);
                frame = Some(Frame::Int(value));
            }

            let (method, index);
            if fixdata & 0x08 != 0 {
                let named = target_threads[(fixdata & 3) as usize];
                (method, index) = match named {
                    Some(named) => (named.method, named.index),
                    None => (7, 0),
                };
            } else {
                method = (fixdata & 3) as i64;
                (index, at) = _index(body, at);
            }

            let (mut disp, mut disp_pos) = (0i64, None);
            if fixdata & 0x04 == 0 {
                (disp, disp_pos) = (unpack_from(body, at), Some(at));
                at += 2;
            }

            found.push(Fixup {
                seg,
                offset: base + offset,
                loc,
                selfrel,
                target: TARGET_KIND
                    .get(&(method & 3))
                    .copied()
                    .unwrap_or("frame")
                    .to_owned(),
                index,
                disp,
                frame,
                record: record.clone(),
                lo: start,
                hi: at,
                disp_pos,
                frame_method,
            });
        }
    }
    found
}

/// Every external index this fixup depends on, threads resolved.
///
/// Not the THREAD declarations: one may name an external no fixup ever
/// uses, and dropping the EXTDEF it names is exactly what pruning is for.
pub fn names_externals(one: &Fixup) -> HashSet<i64> {
    let mut out: HashSet<i64> = HashSet::default();
    if one.target == "external" {
        out.insert(one.index);
    }
    match one.frame {
        Some(Frame::Thread(frame)) => {
            if frame.method == 2 {
                out.insert(frame.index);
            }
        }
        Some(Frame::Int(frame)) if one.frame_method == Some(2) => {
            out.insert(frame);
        }
        _ => {}
    }
    out
}

/// One EXTDEF record's (name, type index) pairs, in order.
pub fn extdef_entries(record: &Record) -> Vec<(Vec<u8>, Vec<u8>)> {
    let (mut out, mut at) = (Vec::new(), 0usize);
    let body = &record.body;
    while at < body.len() {
        let n = body[at] as usize;
        let name = slice(body, at + 1, at + 1 + n).to_vec();
        at += 1 + n;
        let start = at;
        (_, at) = _index(body, at);
        out.push((name, slice(body, start, at).to_vec()));
    }
    out
}

/// An EXTDEF record holding exactly these names.
pub fn extdef_record(entries: &[(Vec<u8>, Vec<u8>)]) -> Rc<Record> {
    let mut body = Vec::new();
    for (name, kind) in entries {
        body.extend(bytes(&[name.len() as i64]));
        body.extend_from_slice(name);
        body.extend_from_slice(kind);
    }
    Rc::new(Record::new(EXTDEF, body))
}

/// Add a linker dependency without renumbering any existing external.
pub fn with_external(records: &[Rc<Record>], name: &str) -> (Vec<Rc<Record>>, i64) {
    let names = externals(records);
    if let Some(at) = names.iter().position(|one| one == name) {
        return (records.to_vec(), at as i64);
    }
    let position = records
        .iter()
        .enumerate()
        .filter(|(_index, record)| record.r#type & 0xFE == EXTDEF)
        .map(|(index, _record)| index + 1)
        .max()
        .unwrap_or(1);
    let added = extdef_record(&[(encode_latin1(name), b"\x00".to_vec())]);
    let mut out = slice_records(records, 0, position).to_vec();
    out.push(added);
    out.extend_from_slice(slice_records(records, position, records.len()));
    (out, names.len() as i64)
}

// A comment class no tool in this toolchain writes: BC's own are 0x00, 0x9f
// and 0xa1. The attribute byte sets NOPURGE and NOLIST, so LINK drops the
// record from what it produces and never prints it.
pub const FINALISED: (u8, u8) = (0xC0, 0x9C);

/// `records` with a marker saying this pass already emitted them.
///
/// Only after a rebuild that worked. A refusal leaves BC's own code,
/// which is still worth another look.
pub fn finalised(records: &[Rc<Record>], made_by: &str) -> Result<Vec<Rc<Record>>, ValueError> {
    if finalised_at(records)?.is_some() {
        return Err(ValueError(
            "these records are already marked; one object has one account of itself".to_owned(),
        ));
    }
    let (attribute, kind) = FINALISED;
    let text = encode_ascii(made_by);
    if text.len() > 240 {
        return Err(ValueError("the marker does not hold that much".to_owned()));
    }
    let mut body = vec![attribute, kind];
    body.extend_from_slice(b"qbopt\x00");
    body.extend_from_slice(&text);
    let mark = Rc::new(Record::new(COMENT, body));
    if records.is_empty() {
        return Ok(vec![mark]);
    }
    let mut out = records[..1].to_vec();
    out.push(mark);
    out.extend_from_slice(&records[1..]);
    Ok(out)
}

/// What made these records, or None.
///
/// The string names the marker's own schema, the pipeline that wrote it
/// and every option that can change what it emits.
pub fn finalised_at(records: &[Rc<Record>]) -> Result<Option<String>, ValueError> {
    let (_attribute, kind) = FINALISED;
    let mut found = None;
    for one in records {
        if one.r#type & 0xFE == COMENT
            && one.body.len() >= 8
            && one.body[1] == kind
            && &one.body[2..8] == b"qbopt\x00"
        {
            if found.is_some() {
                return Err(ValueError(
                    "two markers disagree about what made this object".to_owned(),
                ));
            }
            found = Some(decode_ascii_replace(&one.body[8..]));
        }
    }
    Ok(found)
}

/// An OMF index, in the shorter of its two encodings.
pub fn as_index(value: i64) -> Result<Vec<u8>, ValueError> {
    if !(0..=0x7FFF).contains(&value) {
        return Err(ValueError(format!("{value} is not an OMF index")));
    }
    Ok(if value < 128 {
        vec![value as u8]
    } else {
        vec![0x80 | (value >> 8) as u8, (value & 0xFF) as u8]
    })
}

/// A FIXUPP record with every external index put through `mapping`.
///
/// An index is named in four places: a fixup's own target, a fixup's own
/// frame, and either of those when a THREAD stands for it instead. The body
/// is rebuilt rather than patched: an index is one byte under 128 and two
/// above it. A record nothing moves is returned as it was.
pub fn renumbered(
    record: &Rc<Record>,
    mapping: &IndexMap<i64, i64>,
) -> Result<Rc<Record>, ValueError> {
    if record.r#type & 0xFE != FIXUPP || mapping.is_empty() {
        return Ok(record.clone());
    }
    let (body, mut at, mut out) = (&record.body, 0usize, Vec::new());
    let mut changed = false;

    let index = |r#where: usize| -> (i64, usize) { _index(body, r#where) };

    while at < body.len() {
        if body[at] & 0x80 == 0 {
            let lead = body[at];
            let method = (lead >> 2) & 7;
            let mut after = at + 1;
            if method >= 3 {
                // carries no index
                out.extend_from_slice(slice(body, at, after));
                at = after;
                continue;
            }
            let was;
            (was, after) = index(after);
            // A frame thread names an external only by method 2; a target
            // thread's method is the same three, and 2 is the external one.
            let now = if method == 2 {
                *mapping.get(&was).unwrap_or(&was)
            } else {
                was
            };
            changed = changed || now != was;
            out.extend_from_slice(slice(body, at, at + 1));
            out.extend(as_index(now)?);
            at = after;
            continue;
        }

        let start = at;
        out.extend_from_slice(slice(body, at, at + 2));
        at += 2;
        let fixdata = body[at];
        out.push(fixdata);
        at += 1;

        if fixdata & 0x80 == 0 && ((fixdata >> 4) & 7) < 3 {
            let was;
            (was, at) = index(at);
            let now = if ((fixdata >> 4) & 7) == 2 {
                *mapping.get(&was).unwrap_or(&was)
            } else {
                was
            };
            changed = changed || now != was;
            out.extend(as_index(now)?);
        }

        if fixdata & 0x08 == 0 {
            let was;
            (was, at) = index(at);
            let now = if (fixdata & 3) == 2 {
                *mapping.get(&was).unwrap_or(&was)
            } else {
                was
            };
            changed = changed || now != was;
            out.extend(as_index(now)?);
        }

        if fixdata & 0x04 == 0 {
            out.extend_from_slice(slice(body, at, at + 2));
            at += 2;
        }
        let _ = start;
    }

    Ok(if !changed {
        record.clone()
    } else {
        Rc::new(Record::new(record.r#type, out))
    })
}

/// A new absolute offset16 relocation, framed relative to a data group.
pub fn offset_fixup(
    seg: i64,
    offset: i64,
    target: &str,
    index: i64,
    disp: i64,
    group: i64,
) -> Result<Fixup, ValueError> {
    let method = target_method(target);
    let mut raw = bytes(&[0xC4, 0, 0x10 | method]);
    raw.extend(as_index(group)?);
    raw.extend(as_index(index)?);
    raw.extend(pack(disp));
    let record = fixupp_record(&[raw.clone()]);
    Ok(Fixup {
        seg: Some(seg),
        offset,
        loc: LOC_OFF16,
        selfrel: false,
        target: target.to_owned(),
        index,
        disp,
        frame: Some(Frame::Int(group)),
        record,
        lo: 0,
        hi: raw.len(),
        disp_pos: Some(raw.len() - 2),
        frame_method: Some(1),
    })
}

/// A new absolute offset16 relocation framed by its own target.
///
/// An explicitly segmented operand such as `es:[bx+symbol]` does not use
/// DGROUP as its runtime frame, so the offset relocation must use that same
/// SEGDEF or EXTDEF as both frame and target.
pub fn target_offset_fixup(
    seg: i64,
    offset: i64,
    target: &str,
    index: i64,
    disp: i64,
) -> Result<Fixup, ValueError> {
    let method = target_method(target);
    // Frame method 5 means "the target's frame" and carries no frame datum.
    // d32x's independent OMF writer emits 54/56 for the zero-displacement
    // forms; clearing P (bit 2) to 50/52 adds the displacement below.
    let mut raw = bytes(&[0xC4, 0, 0x50 | method]);
    raw.extend(as_index(index)?);
    raw.extend(pack(disp));
    let record = fixupp_record(&[raw.clone()]);
    Ok(Fixup {
        seg: Some(seg),
        offset,
        loc: LOC_OFF16,
        selfrel: false,
        target: target.to_owned(),
        index,
        disp,
        frame: None,
        record,
        lo: 0,
        hi: raw.len(),
        disp_pos: Some(raw.len() - 2),
        frame_method: None,
    })
}

/// This fixup's own bytes, with the fields given replaced. None leaves one alone.
///
/// Only two fixed positions ever change; the thread encoding survives.
pub fn reemit(
    fixup: &Fixup,
    offset: Option<i64>,
    disp: Option<i64>,
) -> Result<Vec<u8>, ValueError> {
    let mut out = fixup.raw().to_vec();
    if let Some(offset) = offset {
        if !(0..1024).contains(&offset) {
            return Err(ValueError(format!(
                "a fixup offset is ten bits; {} does not fit",
                hex(offset)
            )));
        }
        out[0] = (out[0] & 0xFC) | (offset >> 8) as u8;
        out[1] = (offset & 0xFF) as u8;
    }
    if let Some(disp) = disp {
        let Some(disp_pos) = fixup.disp_pos else {
            return Err(ValueError("this fixup carries no displacement".to_owned()));
        };
        pack_into(&mut out, disp_pos - fixup.lo, disp);
    }
    Ok(out)
}

pub fn ledata_record(seg: i64, offset: i64, payload: &[u8]) -> Result<Rc<Record>, ValueError> {
    if payload.len() > 1024 {
        return Err(ValueError(format!(
            "LEDATA holds at most 1024 bytes, not {}",
            payload.len()
        )));
    }
    let mut body = _emit_index(seg);
    body.extend(pack(offset));
    body.extend_from_slice(payload);
    Ok(Rc::new(Record::new(LEDATA, body)))
}

pub fn fixupp_record(subrecords: &[Vec<u8>]) -> Rc<Record> {
    Rc::new(Record::new(FIXUPP, subrecords.concat()))
}

pub fn _emit_index(value: i64) -> Vec<u8> {
    if value < 0x80 {
        bytes(&[value])
    } else {
        bytes(&[0x80 | (value >> 8), value & 0xFF])
    }
}

/// Where SEGDEF keeps its length. Absolute segments push it three bytes on.
pub fn segment_length_at(record: &Record) -> usize {
    if record.body[0] >> 5 == 0 { 4 } else { 1 }
}

/// Byte positions in `record.body` of 16-bit offsets into segment `seg`.
///
/// The fixups themselves and the self-relative branches have their own paths.
pub fn code_offsets(record: &Record, seg: i64) -> Vec<usize> {
    let body = &record.body;
    match record.r#type & 0xFE {
        t if t == PUBDEF => {
            let (_group, mut at) = _index(body, 0);
            let base;
            (base, at) = _index(body, at);
            if base == 0 {
                // an absolute segment names its frame instead
                at += 2;
            }
            if base != seg {
                return Vec::new();
            }
            let mut found = Vec::new();
            while at < body.len() {
                at += 1 + body[at] as usize; // the name
                found.push(at);
                at += 2; // the offset
                (_, at) = _index(body, at);
            }
            found
        }
        t if t == LINNUM => {
            let (_group, mut at) = _index(body, 0);
            let base;
            (base, at) = _index(body, at);
            if base != seg {
                return Vec::new();
            }
            (at + 2..body.len()).step_by(4).collect() // (line, offset) pairs
        }
        _ => Vec::new(),
    }
}

/// A copy of `record` with 16-bit fields replaced. Unchanged records are not copied.
pub fn patched(record: &Rc<Record>, values: &IndexMap<usize, i64>) -> Rc<Record> {
    if values.is_empty() {
        return record.clone();
    }
    let mut body = record.body.clone();
    for (&at, &value) in values {
        pack_into(&mut body, at, value);
    }
    Rc::new(Record::new(record.r#type, body))
}

pub fn has_start_address(record: &Record) -> bool {
    record.body[0] & 0x40 != 0
}

pub fn main(path: impl AsRef<Path>) -> Result<(), ReadError> {
    let path = path.as_ref();
    let recs = read(path)?;
    let (segs, exts) = (segments(&recs), externals(&recs));
    let mut counts: IndexMap<String, i64> = IndexMap::default();
    for r in &recs {
        *counts.entry(r.name()).or_insert(0) += 1;
    }
    println!("{}", path.display());
    let mut items: Vec<(&String, &i64)> = counts.iter().collect();
    items.sort();
    let listed: Vec<String> = items.iter().map(|(k, v)| format!("{k} {v}")).collect();
    println!("  records: {}", listed.join(", "));
    println!("  segments:");
    for (i, s) in segs.iter().enumerate() {
        if let Some((name, length)) = s {
            println!("    {i:2} {name:<16} {length:6}");
        }
    }
    let code: usize = ledata(&recs).iter().map(|(_, _, _, b)| b.len()).sum();
    println!("  LEDATA bytes: {code}");
    if exts.len() > 1 {
        println!("  externals: {}", exts[1..].join(", "));
    }
    let fx = fixups(&recs);
    println!("  fixups: {}", fx.len());
    for f in &fx {
        if f.target == "external" {
            let nm = if (f.index as usize) < exts.len() {
                exts[f.index as usize].clone()
            } else {
                format!("?{}", f.index)
            };
            let loc = match LOCNAME.get(&f.loc) {
                Some(name) => (*name).to_owned(),
                None => f.loc.to_string(),
            };
            println!("    seg {} {:04X}  {loc:<10} {nm}", f.seg.repr(), f.offset);
        }
    }
    Ok(())
}

/// `{"segment": 0, "external": 2}[target]`.
fn target_method(target: &str) -> i64 {
    match target {
        "segment" => 0,
        "external" => 2,
        _ => panic!("KeyError: {}", pyrepr::string(target)),
    }
}

/// `b[lo:hi]` for non-negative bounds: clamped, and empty where `hi < lo`.
fn slice(b: &[u8], lo: usize, hi: usize) -> &[u8] {
    let hi = hi.min(b.len());
    if lo >= hi { &[] } else { &b[lo..hi] }
}

/// `records[lo:hi]`.
fn slice_records(records: &[Rc<Record>], lo: usize, hi: usize) -> &[Rc<Record>] {
    let hi = hi.min(records.len());
    if lo >= hi { &[] } else { &records[lo..hi] }
}

/// `struct.unpack_from("<H", b, at)[0]`.
fn unpack_from(b: &[u8], at: usize) -> i64 {
    if at + 2 > b.len() {
        panic!(
            "struct.error: unpack_from requires a buffer of at least {} bytes",
            at + 2
        );
    }
    u16::from_le_bytes([b[at], b[at + 1]]) as i64
}

/// `struct.pack("<H", value)`.
fn pack(value: i64) -> [u8; 2] {
    if !(0..=0xFFFF).contains(&value) {
        panic!("struct.error: 'H' format requires 0 <= number <= 65535");
    }
    (value as u16).to_le_bytes()
}

/// `struct.pack_into("<H", buffer, at, value)`.
fn pack_into(buffer: &mut [u8], at: usize, value: i64) {
    let packed = pack(value);
    if at + 2 > buffer.len() {
        panic!(
            "struct.error: pack_into requires a buffer of at least {} bytes",
            at + 2
        );
    }
    buffer[at..at + 2].copy_from_slice(&packed);
}

/// `bytes([...])`.
fn bytes(values: &[i64]) -> Vec<u8> {
    values
        .iter()
        .map(|&value| {
            u8::try_from(value)
                .unwrap_or_else(|_| panic!("ValueError: bytes must be in range(0, 256)"))
        })
        .collect()
}

/// `bytes.decode("latin1")`.
fn decode_latin1(b: &[u8]) -> String {
    b.iter().map(|&byte| byte as char).collect()
}

/// `str.encode("latin1")`.
fn encode_latin1(s: &str) -> Vec<u8> {
    s.chars()
        .map(|character| {
            u8::try_from(character as u32).unwrap_or_else(|_| {
                panic!("UnicodeEncodeError: 'latin-1' codec can't encode character")
            })
        })
        .collect()
}

/// `str.encode("ascii")`.
fn encode_ascii(s: &str) -> Vec<u8> {
    if !s.is_ascii() {
        panic!("UnicodeEncodeError: 'ascii' codec can't encode character");
    }
    s.as_bytes().to_vec()
}

/// `bytes.decode("ascii", "replace")`.
fn decode_ascii_replace(b: &[u8]) -> String {
    b.iter()
        .map(|&byte| {
            if byte < 0x80 {
                byte as char
            } else {
                '\u{FFFD}'
            }
        })
        .collect()
}

/// `f"{value:#x}"`.
fn hex(value: i64) -> String {
    if value < 0 {
        format!("-{:#x}", -value)
    } else {
        format!("{value:#x}")
    }
}

/// `id(record)`.
fn id(record: &Rc<Record>) -> usize {
    Rc::as_ptr(record) as usize
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/omf")
    }

    /// The `obj` fixture: every committed OMF object, sorted by name.
    fn objects() -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(fixtures())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "obj"))
            .collect();
        found.sort();
        assert!(!found.is_empty());
        found
    }

    const OPERATOR_OBJECTS: [&str; 4] = ["pds-g2.obj", "qb45.obj", "vbdos-g2.obj", "vbdos-g3.obj"];

    fn jumptable() -> Vec<Rc<Record>> {
        read(fixtures().join("jumptable.obj")).unwrap()
    }

    fn rec(kind: u8, body: &[u8]) -> Rc<Record> {
        Rc::new(Record::new(kind, body.to_vec()))
    }

    // tests/test_omf.py

    #[test]
    fn test_round_trip_is_byte_identical() {
        for obj in objects() {
            let data = std::fs::read(&obj).unwrap();
            let out: Vec<u8> = parse(&data)
                .unwrap()
                .iter()
                .flat_map(|r| r.emit())
                .collect();
            assert_eq!(out, data, "{}", obj.display());
        }
    }

    #[test]
    fn test_decodes_more_than_a_handful_of_records() {
        for obj in objects() {
            assert!(parse(&std::fs::read(&obj).unwrap()).unwrap().len() > 10);
        }
    }

    #[test]
    fn test_module_code_segment_is_found() {
        for obj in objects() {
            let segs = segments(&read(&obj).unwrap());
            let code: Vec<&(String, i64)> = segs[1..]
                .iter()
                .flatten()
                .filter(|s| s.0.ends_with("_CODE"))
                .collect();
            assert!(!code.is_empty(), "no _CODE segment");
            assert!(code[0].1 > 0);
        }
    }

    #[test]
    fn test_runtime_call_is_named_not_guessed_at() {
        for name in OPERATOR_OBJECTS {
            let recs = read(fixtures().join(name)).unwrap();
            let exts = externals(&recs);
            let named: Vec<String> = fixups(&recs)
                .iter()
                .filter(|x| x.target == "external" && exts[x.index as usize] == "B$CPI4")
                .map(|x| {
                    LOCNAME
                        .get(&x.loc)
                        .map_or(x.loc.to_string(), |n| (*n).to_owned())
                })
                .collect();
            assert_eq!(named, ["ptr16:16"]);
        }
    }

    #[test]
    fn test_groups_parses_dgroup_the_same_way_everywhere() {
        for obj in objects() {
            let records = read(&obj).unwrap();
            let segs = segments(&records);
            let groups = groups(&records);
            assert_eq!(groups.keys().collect::<Vec<_>>(), ["DGROUP"]);
            let named: Vec<&Option<(String, i64)>> = groups["DGROUP"]
                .iter()
                .map(|&i| &segs[i as usize])
                .collect();
            assert!(
                named.iter().all(|s| s.is_some()),
                "every DGROUP member is a real segment"
            );
            let mut names: Vec<&str> = named
                .iter()
                .copied()
                .flatten()
                .map(|s| s.0.as_str())
                .collect();
            names.sort();
            assert_eq!(
                names,
                [
                    "BC_CN", "BC_DATA", "BC_DS", "BC_FT", "BC_SA", "BC_SAB", "BR_DATA", "BR_SKYS",
                    "COMMON", "ENMALLOC", "NMALLOC",
                ]
            );
        }
    }

    #[test]
    fn test_threads_are_resolved() {
        let fx = fixups(&jumptable());
        assert_eq!(fx.len(), 40);
        assert!(fx.iter().all(|x| x.target != "thread"));
    }

    fn jt_code(segs: &[Option<(String, i64)>]) -> i64 {
        segs.iter()
            .position(|s| s.as_ref().is_some_and(|s| s.0 == "JT_CODE"))
            .unwrap() as i64
    }

    #[test]
    fn test_on_goto_table_is_relocated_so_code_may_be_moved() {
        let jumptable = jumptable();
        let code_i = jt_code(&segments(&jumptable));
        let mut table: Vec<i64> = fixups(&jumptable)
            .iter()
            .filter(|x| x.target == "segment" && x.index == code_i && x.loc == LOC_OFF16)
            .map(|x| x.offset)
            .collect();
        table.sort();
        assert_eq!(table, [0x0A, 0x40, 0x42, 0x44]);
    }

    #[test]
    fn test_a_fixup_carries_the_address_it_relocates() {
        let jumptable = jumptable();
        let code = jt_code(&segments(&jumptable));
        let mut disps: Vec<i64> = fixups(&jumptable)
            .iter()
            .filter(|f| f.target == "segment" && f.index == code && f.loc == LOC_OFF16)
            .map(|f| f.disp)
            .collect();
        disps.sort();
        assert_eq!(disps, [0x46, 0x52, 0x5E, 0xEA]);
    }

    #[test]
    fn test_a_frame_thread_that_carries_no_index_is_not_read_as_one() {
        let thread = 0x40 | (5 << 2); // frame thread 0, method 5
        let fixup = [0xC4, 0x10, 0x80, 0x01, 0x34, 0x12]; // offset16 at 0x10, disp 0x1234
        let mut body = vec![thread];
        body.extend_from_slice(&fixup);
        let found = fixups(&[rec(FIXUPP, &body)]);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].offset, found[0].loc, found[0].disp),
            (0x10, LOC_OFF16, 0x1234)
        );
    }

    #[test]
    fn test_every_byte_of_a_fixupp_is_accounted_for() {
        for obj in objects() {
            let records = read(&obj).unwrap();
            let mut by_record: IndexMap<usize, Vec<Fixup>> = IndexMap::default();
            for fixup in fixups(&records) {
                by_record.entry(id(&fixup.record)).or_default().push(fixup);
            }
            for found in by_record.values() {
                let body = &found[0].record.body;
                assert_eq!(
                    found.last().unwrap().hi,
                    body.len(),
                    "the last subrecord must end the record"
                );
                for pair in found.windows(2) {
                    assert!(pair[0].hi <= pair[1].lo, "subrecords must not overlap");
                }
                for fixup in found {
                    assert_eq!(fixup.raw(), &body[fixup.lo..fixup.hi]);
                }
            }
        }
    }

    fn _thread(is_frame: bool, number: u8, method: u8, index: Option<i64>) -> Vec<u8> {
        let lead = (if is_frame { 0x40 } else { 0 }) | (method << 2) | number;
        let mut out = vec![lead];
        if let Some(index) = index {
            out.extend(_as_index(index));
        }
        out
    }

    fn _as_index(value: i64) -> Vec<u8> {
        if value < 128 {
            vec![value as u8]
        } else {
            vec![0x80 | (value >> 8) as u8, (value & 0xFF) as u8]
        }
    }

    fn _explicit(offset: i64, method: u8, index: i64, disp: Option<i64>) -> Vec<u8> {
        let mut out = vec![0x80 | ((offset >> 8) & 3) as u8, (offset & 0xFF) as u8];
        out.push((1 << 7) | (if disp.is_none() { 0x04 } else { 0 }) | method);
        out.extend(_as_index(index));
        if let Some(disp) = disp {
            out.extend(pack(disp));
        }
        out
    }

    fn _threaded(offset: i64, number: u8, disp: Option<i64>) -> Vec<u8> {
        let mut out = vec![0x80 | ((offset >> 8) & 3) as u8, (offset & 0xFF) as u8];
        out.push((1 << 7) | 0x08 | (if disp.is_none() { 0x04 } else { 0 }) | number);
        if let Some(disp) = disp {
            out.extend(pack(disp));
        }
        out
    }

    type Walked = (Option<i64>, i64, i64, bool, String, i64, i64);

    /// Every fixup's decoded meaning, which a remap must leave alone but for
    /// the indices it was asked to change.
    fn _walked(records: &[Rc<Record>]) -> Vec<Walked> {
        fixups(records)
            .into_iter()
            .map(|one| {
                (
                    one.seg,
                    one.offset,
                    one.loc,
                    one.selfrel,
                    one.target,
                    one.index,
                    one.disp,
                )
            })
            .collect()
    }

    fn _with_fixups(bodies: &[Vec<u8>]) -> Vec<Rc<Record>> {
        let ledata = ledata_record(1, 0, &[0; 64]).unwrap();
        vec![ledata, rec(FIXUPP, &bodies.concat())]
    }

    fn mapping(pairs: &[(i64, i64)]) -> IndexMap<i64, i64> {
        pairs.iter().copied().collect()
    }

    fn renumber_all(made: &[Rc<Record>], mapping: &IndexMap<i64, i64>) -> Vec<Rc<Record>> {
        made.iter()
            .map(|one| renumbered(one, mapping).unwrap())
            .collect()
    }

    fn indices(walked: &[Walked]) -> Vec<i64> {
        walked.iter().map(|one| one.5).collect()
    }

    #[test]
    fn test_renumbering_externals_leaves_an_identity_mapping_byte_identical() {
        let made = _with_fixups(&[
            _thread(false, 0, 2, Some(7)),
            _threaded(0x10, 0, None),
            _explicit(0x20, 2, 9, Some(0)),
        ]);
        let got = renumber_all(&made, &mapping(&[]));
        assert_eq!(
            got.iter().map(|one| &one.body).collect::<Vec<_>>(),
            made.iter().map(|one| &one.body).collect::<Vec<_>>()
        );
        assert!(
            Rc::ptr_eq(&got[1], &made[1]),
            "an untouched record is not copied"
        );
    }

    #[test]
    fn test_renumbering_moves_a_target_thread_and_the_fixups_that_use_it() {
        let made = _with_fixups(&[
            _thread(false, 0, 2, Some(9)),
            _threaded(0x10, 0, None),
            _threaded(0x14, 0, None),
        ]);
        let before = _walked(&made);
        let got = renumber_all(&made, &mapping(&[(9, 8)]));
        let after = _walked(&got);
        assert_eq!(indices(&before), [9, 9]);
        assert_eq!(indices(&after), [8, 8]);
        let rest = |w: &Walked| (w.0, w.1, w.2, w.3, w.4.clone(), w.6);
        assert_eq!(
            before.iter().map(rest).collect::<Vec<_>>(),
            after.iter().map(rest).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_renumbering_moves_externals_on_both_sides_of_a_removal() {
        let made = _with_fixups(&[
            _explicit(0x10, 2, 3, Some(0)),
            _explicit(0x20, 2, 9, Some(0)),
            _explicit(0x30, 2, 11, Some(0)),
        ]);
        let got = renumber_all(&made, &mapping(&[(9, 8), (11, 10)]));
        assert_eq!(indices(&_walked(&got)), [3, 8, 10]);
    }

    #[test]
    fn test_renumbering_crosses_the_index_length_boundary() {
        let made = _with_fixups(&[
            _explicit(0x10, 2, 128, Some(0)),
            _explicit(0x20, 2, 127, Some(0)),
        ]);
        let got = renumber_all(&made, &mapping(&[(128, 127), (127, 128)]));
        assert_eq!(indices(&_walked(&got)), [127, 128]);
        assert_eq!(
            got[1].body.len(),
            made[1].body.len(),
            "one grew and one shrank"
        );
    }

    #[test]
    fn test_renumbering_a_frame_that_names_an_external_moves_it_too() {
        let mut body = vec![0x80, 0x10];
        body.push((2 << 4) | 0x04 | 2); // explicit frame method 2, target external
        body.extend(_as_index(9));
        body.extend(_as_index(9));
        let made = _with_fixups(&[body]);
        let got = renumber_all(&made, &mapping(&[(9, 4)]));
        assert_eq!(indices(&_walked(&got)), [4]);
        assert_eq!(fixups(&got)[0].frame, Some(Frame::Int(4)));
    }

    #[test]
    fn test_renumbering_the_corpus_moves_the_indices_and_nothing_else() {
        for obj in objects() {
            let records = parse(&std::fs::read(&obj).unwrap()).unwrap();
            let mut every: Vec<i64> = fixups(&records)
                .iter()
                .filter(|one| one.target == "external")
                .map(|one| one.index)
                .collect();
            every.sort();
            every.dedup();
            if every.is_empty() {
                continue; // pytest.skip("no external fixups")
            }
            let mapping: IndexMap<i64, i64> =
                every.iter().rev().map(|&one| (one, one + 1)).collect();
            let moved = renumber_all(&records, &mapping);
            let (before, after) = (fixups(&records), fixups(&moved));
            assert_eq!(before.len(), after.len());
            for (one, other) in before.iter().zip(&after) {
                assert_eq!(
                    (
                        one.seg,
                        one.offset,
                        one.loc,
                        one.selfrel,
                        &one.target,
                        one.disp
                    ),
                    (
                        other.seg,
                        other.offset,
                        other.loc,
                        other.selfrel,
                        &other.target,
                        other.disp
                    )
                );
                let want = if one.target == "external" {
                    *mapping.get(&one.index).unwrap_or(&one.index)
                } else {
                    one.index
                };
                assert_eq!(other.index, want);
            }
        }
    }

    #[test]
    fn test_a_thread_outlives_the_record_it_was_declared_in() {
        let ledata = ledata_record(1, 0, &[0; 64]).unwrap();
        let first = rec(
            FIXUPP,
            &[_thread(false, 0, 2, Some(9)), _threaded(0x10, 0, None)].concat(),
        );
        let second = rec(
            FIXUPP,
            &[_threaded(0x20, 0, None), _threaded(0x24, 0, None)].concat(),
        );
        let made = vec![ledata, first, second.clone()];
        assert_eq!(
            fixups(&made)
                .iter()
                .map(|one| one.index)
                .collect::<Vec<_>>(),
            [9, 9, 9]
        );
        let got = renumber_all(&made, &mapping(&[(9, 5)]));
        assert_eq!(
            fixups(&got).iter().map(|one| one.index).collect::<Vec<_>>(),
            [5, 5, 5]
        );
        assert_eq!(
            got[2].body, second.body,
            "a record that only refers to a thread is untouched"
        );
    }

    #[test]
    fn test_a_private_comment_survives_a_parse_and_emit() {
        let raw = std::fs::read(fixtures().join("hotlop-p-g2.obj")).unwrap();
        let records = parse(&raw).unwrap();
        let marked = finalised(&records, "1:whole+absorb").unwrap();
        assert_eq!(
            finalised_at(&marked).unwrap().as_deref(),
            Some("1:whole+absorb")
        );
        assert_eq!(finalised_at(&records).unwrap(), None);
        let out: Vec<u8> = marked.iter().flat_map(|one| one.emit()).collect();
        assert_eq!(
            finalised_at(&parse(&out).unwrap()).unwrap().as_deref(),
            Some("1:whole+absorb")
        );
        // And nothing else about the object moved.
        let kinds = |records: &[Rc<Record>]| -> Vec<u8> {
            records
                .iter()
                .filter(|one| one.r#type & 0xFE != COMENT)
                .map(|one| one.r#type)
                .collect()
        };
        assert_eq!(kinds(&parse(&out).unwrap()), kinds(&records));
    }

    #[test]
    fn test_a_second_marker_is_refused_rather_than_added() {
        for obj in objects() {
            let records =
                finalised(&parse(&std::fs::read(&obj).unwrap()).unwrap(), "1:whole").unwrap();
            let error = finalised(&records, "1:whole").unwrap_err();
            assert!(error.0.contains("already"));
        }
    }

    #[test]
    fn test_no_fixture_already_carries_the_marker_signature() {
        for obj in objects() {
            assert_eq!(
                finalised_at(&parse(&std::fs::read(&obj).unwrap()).unwrap()).unwrap(),
                None
            );
        }
    }

    // tests/test_module.py, the omf-only cases

    const WITH_PAIRS: &str = "jumptable.obj";

    #[test]
    fn test_the_last_write_to_a_byte_is_the_one_that_counts() {
        for (at, written) in [(0x50, 21), (0x5C, 9), (0x89, 80), (0xA7, 50), (0xC5, 20)] {
            let records = read(fixtures().join(WITH_PAIRS)).unwrap();
            let (seg, _name, size) = code_segment(&records).unwrap();
            assert_eq!(segment_image(&records, seg, size)[at], written);
        }
    }

    #[test]
    fn test_nothing_in_the_corpus_has_to_be_refused() {
        for obj in objects() {
            assert!(refusals(&read(&obj).unwrap()).is_empty());
        }
    }

    #[test]
    fn test_a_record_nothing_here_decodes_is_refused() {
        for (kind, why) in [(LIDATA, "LIDATA"), (FIXUPP + 1, "32-bit"), (0xC2, "COMDAT")] {
            let mut records = read(fixtures().join(WITH_PAIRS)).unwrap();
            records.push(rec(kind, b"\x00"));
            assert!(refusals(&records).iter().any(|reason| reason.contains(why)));
        }
    }

    // tests/test_pointer_dependency.py

    #[test]
    fn test_adding_pointer_dependency_keeps_existing_fixups() {
        for tag in ["p-g2", "q-O", "v-g3"] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("fixtures/regressions/huge2-{tag}.obj").to_lowercase());
            let records = read(path).unwrap();
            let before = externals(&records);
            let (added, index) = with_external(&records, "b$HugeShift");
            assert_eq!(index, before.len() as i64);
            let mut want = before.clone();
            want.push("b$HugeShift".to_owned());
            assert_eq!(externals(&added), want);
            assert_eq!(fixups(&added), fixups(&records));
            let new: Vec<&Rc<Record>> = added
                .iter()
                .filter(|record| !records.contains(record))
                .collect();
            assert_eq!(
                new,
                [&extdef_record(&[(
                    b"b$HugeShift".to_vec(),
                    b"\x00".to_vec()
                )])]
            );
            let (repeated, again) = with_external(&added, "b$HugeShift");
            assert!(again == index && repeated == added);
            let emitted: Vec<u8> = added.iter().flat_map(|record| record.emit()).collect();
            assert_eq!(parse(&emitted).unwrap(), added);
        }
    }
}
