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
use std::sync::Arc;

use iced_x86::{FlowControl, Mnemonic, OpKind, Register};

use crate::frontends::bc::declen::{self, Insn};
use crate::model::ir::nodes::Node;
use crate::objectfile::omf::{self, Fixup, Record};
use crate::support::hash::IndexMap;
use crate::support::pyrepr::{self, Repr};

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

/// Machine provenance produced by raising, kept beside rather than in MIR.
#[derive(Clone, Default)]
pub struct SourceMap {
    pub refs: IndexMap<u32, Vec<i64>>,
    pub nodes: IndexMap<u32, Arc<Node>>,
    pub float_protocols: IndexMap<u32, i64>,
    pub absorbed: IndexMap<u32, Object>,
    pub coverage: IndexMap<u32, Vec<(i64, i64)>>,
    // Immutable byte ranges of each raw raise-time occurrence.
    pub occurrences: IndexMap<u32, Vec<(i64, i64)>>,
}

impl SourceMap {
    pub fn from_module(found: &Module) -> SourceMap {
        SourceMap {
            refs: found.refs.clone(),
            nodes: IndexMap::default(),
            float_protocols: found.float_protocols.clone(),
            absorbed: found.absorbed.clone(),
            coverage: found.coverage.clone(),
            occurrences: IndexMap::default(),
        }
    }

    /// A legacy test view carrying this provenance, without mutation.
    pub fn applied(&self, found: &Module) -> Module {
        Module {
            refs: self.refs.clone(),
            float_protocols: self.float_protocols.clone(),
            absorbed: self.absorbed.clone(),
            coverage: self.coverage.clone(),
            ..found.clone()
        }
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

/// Every (segment, displacement) this object hands out the address of.
///
/// A relocated immediate inside a `push`, a register `mov` subsequently
/// pushed without being overwritten, or a `lea` is what handing one over
/// looks like. The object it names, not the byte: an escaped address poisons
/// the whole landmark object.
pub fn escaped(found: &Module) -> BTreeSet<(i64, i64)> {
    let fields: Vec<Fixup> =
        omf::fixups(&found.records).into_iter().filter(|one| one.seg == Some(found.seg)).collect();
    if fields.is_empty() {
        return BTreeSet::new();
    }
    let instructions = _instructions(found);
    let Some(instructions) = instructions else {
        return fields.iter().map(|one| (one.index, one.disp)).collect();
    };
    let mut out = BTreeSet::new();
    let values = _numeric_arguments(found, Some(&instructions));
    for (at, end, pushed) in _pushes(found, Some(&instructions)) {
        if values.contains(&at) || pushed.is_some_and(|pushed| values.contains(&pushed)) {
            continue;
        }
        for one in &fields {
            if at <= one.offset && one.offset < end {
                out.insert((one.index, one.disp));
            }
        }
    }
    out
}

/// The code map's instructions in address order, or None where there is no map.
///
/// Not a sweep from the segment's start: that decodes the module header as
/// code.
pub fn _instructions(found: &Module) -> Option<Vec<Insn>> {
    use crate::frontends::bc::blocks;

    let mapped = blocks::code_map(found).ok()?;
    Some(mapped.starts.iter().filter_map(|&at| declen::decode(&found.code, at)).collect())
}

/// Numeric argument pushes, including nested long-arithmetic call frames.
pub fn _numeric_arguments(found: &Module, instructions: Option<&[Insn]>) -> BTreeSet<i64> {
    use crate::abi::runtime;
    use crate::frontends::bc::{blocks, stack};

    let local = defines(&found.records, found.seg);
    let calls: IndexMap<i64, String> =
        found.calls.iter().filter(|(_, name)| !local.contains(*name)).map(|(&at, name)| (at, name.clone())).collect();
    let (mut pending, mut values, mut end): (Vec<&Insn>, BTreeSet<i64>, Option<i64>) =
        (Vec::new(), BTreeSet::new(), None);
    let owned;
    let instructions = match instructions {
        Some(instructions) => instructions,
        None => {
            owned = _instructions(found).unwrap_or_default();
            &owned
        }
    };
    for insn in instructions {
        let at = insn.at as i64;
        if Some(at) != end {
            pending = Vec::new();
        }
        end = Some(insn.end() as i64);
        if insn.insn.mnemonic() == Mnemonic::Push {
            pending.push(insn);
        } else {
            let width = runtime::numeric_stack_arguments(calls.get(&at).map_or("", String::as_str));
            if let Some(width) = width {
                let (mut consumed, mut total) = (Vec::new(), 0i64);
                for one in pending.iter().rev() {
                    total -= one.insn.stack_pointer_increment() as i64;
                    consumed.push(one.at as i64);
                    if total >= width {
                        if total == width {
                            values.extend(consumed.iter().copied());
                        }
                        break;
                    }
                }
            }
            pending = Vec::new();
        }
    }

    let long_arity = |name: &str| if runtime::numeric_stack_arguments(name) == Some(8) { Some(2) } else { None };

    if calls.values().any(|name| long_arity(name).is_some()) {
        let mapped = match blocks::code_map(found) {
            Ok(mapped) => mapped,
            Err(_) => panic!("AttributeError: 'str' object has no attribute 'starts'"),
        };
        for block in blocks::partition(found, &mapped) {
            for frame in stack::frames(&block, &calls, &long_arity) {
                values.extend(frame.pushed.iter().map(|one| one.at as i64));
            }
        }
    }
    values
}

/// Address-bearing spans and the push consuming a materialized address.
pub fn _pushes(found: &Module, instructions: Option<&[Insn]>) -> Vec<(i64, i64, Option<i64>)> {
    let owned;
    let instructions = match instructions {
        Some(instructions) => instructions,
        None => {
            owned = _instructions(found).unwrap_or_default();
            &owned
        }
    };
    let mut out = Vec::new();
    for insn in instructions {
        let at = insn.at as i64;
        // `str(insn.insn).lower()`'s first word is the mnemonic: measured
        // equal on every decode of every fixture's code segment.
        let text = format!("{:?}", insn.insn.mnemonic()).to_lowercase();
        // A `mov [x],ax` also carries a relocated field, but that is the
        // store's own displacement -- the address of the cell being written,
        // not an address being handed to anybody.
        let mut materialized = insn.insn.mnemonic() == Mnemonic::Mov
            && insn.insn.op0_kind() == OpKind::Register
            && matches!(insn.insn.op1_kind(), OpKind::Immediate16 | OpKind::Immediate32);
        let pushed = if materialized {
            _pushed_before_write(found, insn.end() as i64, insn.insn.op0_register())
        } else {
            None
        };
        materialized = pushed.is_some();
        if text.starts_with("push") || text.starts_with("lea") || materialized {
            out.push((at, insn.end() as i64, pushed));
        }
    }
    out
}

pub fn _pushed_before_write(found: &Module, mut at: i64, register: Register) -> Option<i64> {
    let root = register.full_register32();
    let mut info = declen::instruction_info_factory();
    while at < found.end {
        let one = declen::decode(&found.code, at as usize)?;
        if one.insn.flow_control() != FlowControl::Next {
            return None;
        }
        if one.insn.mnemonic() == Mnemonic::Push
            && one.insn.op0_kind() == OpKind::Register
            && one.insn.op0_register().full_register32() == root
        {
            return Some(at);
        }
        if info
            .info(&one.insn)
            .used_registers()
            .iter()
            .any(|access| declen::WRITES.contains(&access.access()) && access.register().full_register32() == root)
        {
            return None;
        }
        at = one.end() as i64;
    }
    None
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
pub mod tests;
