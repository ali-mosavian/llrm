//! Port of `qbopt/objectfile/module.py`: one BC module, as the analysis
//! layer needs to see it.
//!
//! `defines` panics where Python's `omf.pubdef_names` raises: every
//! production path reads the object through `LinkUnit.read` first, whose
//! `public_definitions` returns that same error as a `Result`, so the
//! panic is Python's uncaught exception and nothing else.

use std::any::Any;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use iced_x86::Register;

use crate::omf::{self, Fixup, Record};
use llrm_support::hash::IndexMap;
use llrm_support::pyrepr::{self, Repr};

pub const CALL_FAR: u8 = 0x9A;

/// BC's own linker directive segment group.
pub const DGROUP: &str = "DGROUP";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Space {
    /// relocated: an offset into the segment `index` names
    Segment,
    /// relocated against EXTDEF; distinct symbols may alias
    External,
    /// bp-relative, so the displacement really is in the code
    Frame,
    /// a displacement in the code that no fixup claims
    Literal,
    /// relocated against a GRPDEF rather than a SEGDEF
    Group,
    /// a $DYNAMIC array element: `es:[bx]`, `es:[bx+2]`
    Far,
    /// a slot the code itself pushed, addressed by its depth
    Stack,
}

impl Space {
    pub const ALL: [Space; 7] =
        [Space::Segment, Space::External, Space::Frame, Space::Literal, Space::Group, Space::Far, Space::Stack];

    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Space::Segment => "SEGMENT",
            Space::External => "EXTERNAL",
            Space::Frame => "FRAME",
            Space::Literal => "LITERAL",
            Space::Group => "GROUP",
            Space::Far => "FAR",
            Space::Stack => "STACK",
        }
    }

    /// The `StrEnum` value, which is also `str(space)`.
    pub fn value(self) -> &'static str {
        match self {
            Space::Segment => "seg",
            Space::External => "external",
            Space::Frame => "bp",
            Space::Literal => "abs",
            Space::Group => "grp",
            Space::Far => "far",
            Space::Stack => "sp",
        }
    }
}

/// A `StrEnum` orders as its string value.
impl Ord for Space {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value().cmp(other.value())
    }
}

impl PartialOrd for Space {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Space {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.value())
    }
}

impl Repr for Space {
    fn repr(&self) -> String {
        pyrepr::str_enum("Space", self.name(), self.value())
    }
}

/// `INDEX_NAMES`: base is si/di for a SEGMENT array element and bx for a FAR one.
pub fn index_names(register: Register) -> Option<&'static str> {
    match register {
        Register::SI => Some("si"),
        Register::DI => Some("di"),
        Register::BX => Some("bx"),
        _ => None,
    }
}

/// `SEGMENT_NAMES`.
pub fn segment_names(register: Register) -> Option<&'static str> {
    match register {
        Register::ES => Some("es"),
        Register::DS => Some("ds"),
        Register::SS => Some("ss"),
        Register::CS => Some("cs"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Addr {
    pub space: Space,
    pub disp: i64,
    pub index: i64,
    /// NONE for a bare displacement; an array element's index register.
    pub base: Register,
    /// NONE except for an address whose selector differs from the default.
    pub segment: Register,
}

impl Addr {
    /// `Addr(space, disp)`.
    pub const fn new(space: Space, disp: i64) -> Self {
        Addr { space, disp, index: 0, base: Register::None, segment: Register::None }
    }

    /// Whether the address names bytes without a run-time index.
    pub fn direct(&self) -> bool {
        self.base == Register::None
    }

    pub fn plus(&self, bytes_along: i64) -> Addr {
        Addr { disp: self.disp + bytes_along, ..*self }
    }
}

/// `f"{value:+#x}"`.
pub fn signed_hex(value: i64) -> String {
    if value < 0 { format!("-{:#x}", value.unsigned_abs()) } else { format!("+{value:#x}") }
}

/// `f"r{register}"`: iced's register number, as Python prints the int.
fn numbered(register: Register) -> String {
    format!("r{}", register as u32)
}

impl Repr for Addr {
    fn repr(&self) -> String {
        if self.space == Space::Far {
            let seg = segment_names(self.segment).map_or_else(|| numbered(self.segment), str::to_owned);
            let base = index_names(self.base).map_or_else(|| numbered(self.base), str::to_owned);
            return format!("[{seg}:{base}{}]", signed_hex(self.disp));
        }
        let where_ = if matches!(self.space, Space::Segment | Space::External) {
            format!("{}:{}", self.space, self.index)
        } else {
            self.space.to_string()
        };
        let indexed = if self.base != Register::None {
            format!("+{}", index_names(self.base).map_or_else(|| numbered(self.base), str::to_owned))
        } else {
            String::new()
        };
        format!("[{where_}{indexed}{}]", signed_hex(self.disp))
    }
}

/// DGROUP's segment indexes, and which of them the link overlays.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Group {
    pub members: BTreeSet<i64>,
    /// the COMMON-combined ones
    pub shared: BTreeSet<i64>,
}

impl Group {
    pub fn new(members: impl IntoIterator<Item = i64>, shared: impl IntoIterator<Item = i64>) -> Self {
        Group { members: members.into_iter().collect(), shared: shared.into_iter().collect() }
    }

    pub fn contains(&self, index: i64) -> bool {
        self.members.contains(&index)
    }
}

/// Which compiler made an object.
///
/// Not a preference: B$ENRA reads bx under VBDOS's runtime and does not
/// touch it under PDS's, so a routine's contract is not one fact for
/// every toolchain. Read from COMENT class 0x00, which every object
/// carries and which names the compiler outright.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Family {
    Quickbasic,
    Pds,
    Vbdos,
    Unknown,
}

impl Family {
    /// The member name.
    pub fn name(self) -> &'static str {
        match self {
            Family::Quickbasic => "QUICKBASIC",
            Family::Pds => "PDS",
            Family::Vbdos => "VBDOS",
            Family::Unknown => "UNKNOWN",
        }
    }

    /// The `StrEnum` value, which is also `str(family)`.
    pub fn value(self) -> &'static str {
        match self {
            Family::Quickbasic => "qb45",
            Family::Pds => "pds71",
            Family::Vbdos => "vbdos",
            Family::Unknown => "unknown",
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.value())
    }
}

impl Repr for Family {
    fn repr(&self) -> String {
        pyrepr::str_enum("Family", self.name(), self.value())
    }
}

const _MADE_BY: [(&[u8], Family); 3] = [
    (b"QuickBASIC Compiler 4.5", Family::Quickbasic),
    (b"BASIC Compiler 7.1", Family::Pds),
    (b"VBDOS", Family::Vbdos),
];

/// Every name this module declares itself: BC compiles a SUB as a
/// PUBDEF and calls it through an EXTDEF fixup of the same object, so the
/// call target alone cannot say whether it is the runtime's or its own.
pub fn defines(records: &[Rc<Record>], seg: i64) -> BTreeSet<String> {
    match omf::pubdef_names(records, seg) {
        Ok(names) => names.into_values().collect(),
        Err(error) => panic!("ValueError: {error}"),
    }
}

/// Which compiler wrote these records, from its own comment.
pub fn family(records: &[Rc<Record>]) -> Family {
    for one in records {
        if one.r#type != 0x88 || one.body.len() < 2 || one.body[1] != 0x00 {
            continue;
        }
        let said = &one.body[2..];
        for (mark, which) in _MADE_BY {
            if said.windows(mark.len()).any(|window| window == mark) {
                return which;
            }
        }
    }
    Family::Unknown
}

/// Python's `object`: what `absorbed` and `SourceMap.nodes`-like maps hold
/// where module.py sits below the module that names the type.
pub type Object = Rc<dyn Any>;

#[derive(Clone)]
pub struct Module {
    pub records: Vec<Rc<Record>>,
    pub seg: i64,
    pub name: String,
    pub code: Vec<u8>,
    // The predecessor put module-level code after a 0x30-byte header. These
    // fixtures neither confirm nor refute it, so no boundary is claimed here.
    pub start: i64,
    pub end: i64,
    pub operands: IndexMap<i64, Addr>,
    pub calls: IndexMap<i64, String>,
    pub targets: BTreeSet<i64>,
    pub publics: BTreeSet<i64>,
    // line-number table entries, which name code offsets like everything else
    pub lines: BTreeSet<i64>,
    // the byte ranges BC split the segment into; a rewrite may not span two
    pub chunks: Vec<(i64, i64)>,
    pub sites: BTreeSet<i64>,
    // the fixup that named each operand field, so a widened form can reuse it
    pub fixup_at: IndexMap<i64, Fixup>,
    // segment indices DGROUP's own GRPDEF names -- a stack slot (Space.FRAME)
    // can never be the same byte as a segment outside this set.
    pub dgroup: Group,
    // The segment BC put this program's own variables in -- the one whose
    // SEGDEF class is BC_DATA.
    pub program_data: Option<i64>,
    // Which fixups each operation's own operands carry, by mir.Op.id.
    pub refs: IndexMap<u32, Vec<i64>>,
    // Encoding provenance for FP operations raised from runtime helpers.
    pub float_protocols: IndexMap<u32, i64>,
    // Which absorbable call each folded operation stands for, by mir.Op.id.
    // calls.CallSite, but module.py sits below calls.py and cannot say so.
    pub absorbed: IndexMap<u32, Object>,
    // The full, ordered set of disjoint byte ranges a folded operation
    // stands for, by mir.Op.id -- present only for the rare op whose bytes
    // are not one run.
    pub coverage: IndexMap<u32, Vec<(i64, i64)>>,
}

impl Module {
    /// What the operand whose displacement field sits here points at.
    pub fn resolve(&self, field_offset: i64, literal: i64) -> Addr {
        self.operands.get(&field_offset).copied().unwrap_or(Addr::new(Space::Literal, literal))
    }
}

pub fn frame_relative(literal: i64) -> Addr {
    Addr::new(Space::Frame, literal)
}

pub fn far_pointer(literal: i64, base: Register, segment: Register) -> Addr {
    Addr { base, segment, ..Addr::new(Space::Far, literal) }
}

/// The resolver for code with no fixups behind it, as every unit test has.
pub fn literal_only(_field_offset: i64, literal: i64) -> Addr {
    Addr::new(Space::Literal, literal)
}

/// The widest access anything here can name -- an x87 qword load.
pub const WIDEST: i64 = 8;

/// Whether [disp, disp+width) intersect -- arithmetic, not analysis.
pub fn _overlaps(a: &Addr, a_width: i64, b: &Addr, b_width: i64) -> bool {
    a.disp < b.disp + b_width && b.disp < a.disp + a_width
}

/// Every displacement in each segment that some operand names exactly.
///
/// The assumption is that a subscript is in range: the array beginning at
/// one displacement cannot run past the next thing named after it.
pub fn landmarks(found: &Module) -> IndexMap<(Space, i64), Vec<i64>> {
    let mut found_at: IndexMap<(Space, i64), BTreeSet<i64>> = IndexMap::default();
    for addr in found.operands.values() {
        if addr.base == Register::None && matches!(addr.space, Space::Segment | Space::Frame) {
            found_at.entry((addr.space, addr.index)).or_default().insert(addr.disp);
        }
    }
    found_at.into_iter().map(|(at, disps)| (at, disps.into_iter().collect())).collect()
}

/// The bytes an operand can touch, or None where nothing bounds it.
pub fn reach(addr: &Addr, width: i64, bounds: &IndexMap<(Space, i64), Vec<i64>>) -> Option<(i64, i64)> {
    if addr.base == Register::None {
        return Some((addr.disp, addr.disp + width));
    }
    let known = bounds.get(&(addr.space, addr.index))?;
    if known.is_empty() {
        return None;
    }
    known.iter().copied().find(|&one| one > addr.disp).map(|after| (addr.disp, after))
}

/// The SEGDEF name BC gives the segment holding a program's own variables.
pub const PROGRAM_DATA: &str = "BC_DATA";

/// Which segment index holds this program's own variables.
pub fn _program_data(records: &[Rc<Record>]) -> Option<i64> {
    for (index, one) in omf::segments(records).into_iter().enumerate() {
        if let Some((name, _)) = one {
            if name == PROGRAM_DATA {
                return Some(index as i64);
            }
        }
    }
    None
}

pub fn of(records: &[Rc<Record>]) -> Option<Module> {
    let (seg, name, size) = omf::code_segment(records)?;
    let code = omf::segment_image(records, seg, size);
    let fixups: Vec<Fixup> = omf::fixups(records).into_iter().filter(|fixup| fixup.seg == Some(seg)).collect();

    let spaces = |target: &str| match target {
        "segment" => Some(Space::Segment),
        "group" => Some(Space::Group),
        "external" => Some(Space::External),
        _ => None,
    };
    let mut operands: IndexMap<i64, Addr> = IndexMap::default();
    for fixup in &fixups {
        if fixup.loc == omf::LOC_OFF16 {
            if let Some(space) = spaces(&fixup.target) {
                operands.insert(fixup.offset, Addr { index: fixup.index, ..Addr::new(space, fixup.disp) });
            }
        }
    }
    let externals = omf::externals(records);
    let mut calls: IndexMap<i64, String> = IndexMap::default();
    for fixup in &fixups {
        // code[offset - 1 : offset] is one byte only for 1 <= offset <= len
        let byte = (1..=code.len() as i64).contains(&fixup.offset).then(|| code[fixup.offset as usize - 1]);
        if fixup.loc == omf::LOC_PTR32 && fixup.target == "external" && byte == Some(CALL_FAR) {
            calls.insert(fixup.offset - 1, externals[fixup.index as usize].clone());
        }
    }
    let targets: BTreeSet<i64> =
        fixups.iter().filter(|fixup| fixup.target == "segment" && fixup.index == seg).map(|fixup| fixup.disp).collect();
    let named = |kind: u8| -> BTreeSet<i64> {
        records
            .iter()
            .flat_map(|record| omf::code_offsets(record, seg).into_iter().map(move |at| (record, at)))
            .filter(|(record, _)| record.r#type & 0xFE == kind)
            .map(|(record, at)| {
                assert!(at + 2 <= record.body.len(), "struct.error: unpack_from requires a buffer of at least {} bytes", at + 2);
                u16::from_le_bytes([record.body[at], record.body[at + 1]]) as i64
            })
            .collect()
    };

    let chunks: Vec<(i64, i64)> = omf::ledata(records)
        .into_iter()
        .filter(|(_record, index, _offset, _payload)| *index == seg)
        .map(|(_record, _index, offset, payload)| (offset, offset + payload.len() as i64))
        .collect();
    let sites: BTreeSet<i64> = fixups.iter().map(|fixup| fixup.offset).collect();
    let mut fixup_at: IndexMap<i64, Fixup> = IndexMap::default();
    for fixup in &fixups {
        if operands.contains_key(&fixup.offset) {
            fixup_at.insert(fixup.offset, fixup.clone());
        }
    }
    let shared: BTreeSet<i64> = omf::combines(records)
        .into_iter()
        .filter(|(_index, kind)| *kind == omf::COMBINE_COMMON)
        .map(|(index, _kind)| index)
        .collect();
    let dgroup = Group::new(omf::groups(records).get(DGROUP).cloned().unwrap_or_default(), shared);
    let program_data = _program_data(records);

    let end = code.len() as i64;
    Some(Module {
        records: records.to_vec(),
        seg,
        name,
        code,
        start: 0,
        end,
        operands,
        calls,
        targets,
        publics: named(omf::PUBDEF),
        lines: named(omf::LINNUM),
        chunks,
        sites,
        fixup_at,
        dgroup,
        program_data,
        refs: IndexMap::default(),
        float_protocols: IndexMap::default(),
        absorbed: IndexMap::default(),
        coverage: IndexMap::default(),
    })
}

pub fn load(path: impl AsRef<Path>) -> Result<Option<Module>, omf::ReadError> {
    Ok(of(&omf::read(path)?))
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
