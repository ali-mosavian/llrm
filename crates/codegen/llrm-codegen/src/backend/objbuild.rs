//! Encodes a masm module as a format-neutral object: code and data laid out in
//! sections, each name resolved, each reference to one a relocation.
//!
//! # How it works
//! `built` drops the data objects nothing reaches (`live`), then makes one
//! segment per kind: code (one for the module, or one per procedure with
//! `CodeLayout::PerProcedure` so the linker can drop uncalled procedures),
//! each named data segment, and a stack segment if the module asks for one.
//!
//! Data (`_data`) is written first, recording the offset of each label. Code
//! (`_code`) takes each procedure's items from `masm::listing`, encodes the
//! instructions (`_items`, through the target's encoder) and leaves branches
//! as jump items of unknown length. Their lengths are settled before any byte
//! is placed.
//!
//! ```text
//!   items  : piece  jump(L)  piece  label L  piece  near-call(f)
//!   pass 1 : every jump short (2 bytes)
//!   pass n : a jump whose target is out of 8-bit reach grows; repeat
//!   fixed  : growth only moves targets further, so it ends, at the smallest
//!            layout
//! ```
//!
//! `_relaxed` does that fixed point, as jwasm does. When the procedures are
//! tuned for size, `_trampolined` follows it: a conditional jump still long
//! (on a 386 `jcc rel16`, 4 bytes) whose target label has an unconditional
//! `jmp` within short reach is aimed at that `jmp` instead, saving two bytes
//! at the price of a taken `jmp` (Watcom's `SetBranches`), and the fixed point
//! runs again.
//!
//! A call to a procedure of the same segment that no other module can replace
//! is a relative displacement filled in; any other is a relocation. `object_of`
//! turns what the segments hold into [`llrm_object::Object`]: a reference to a
//! symbol the module defines becomes a relocation against that symbol with the
//! addend in the code (as jwasm writes it), anything else is an undefined
//! symbol, and a reference to a name neither defined nor declared is an error.
//! 16-bit data segments of the near kind join the `DGROUP` group.
//!
//! Code is padded as MC does: an `Align` item (a block's `align` and
//! `max_skip`) pads with `nops` (`writeNopData`), and each function not tuned
//! for size starts at the target's preferred function alignment: `.p2align` in
//! OMF and COFF, ELF's `.prefalign` (no more than the body fills).
//!
//! `written_in` hands the object to the writer crate of the target's format
//! (OMF, ELF32/64, COFF32/64, Mach-O x86-64), or refuses a pair none writes.
//!
//! DEVIATE 1: the pin's MC leaves a call or jump to every global to the linker,
//! at `-fPIE` and `-fno-pic` too (`R_X86_64_PLT32`, 5 bytes). Ours binds a
//! definition no other module can replace (`masm::Module::preemptible` lists
//! the ones that can: weak, and default-visibility at `-fPIC`), as gcc and gas
//! do for an executable: `jmp f` is 2 bytes. Bar: gcc. Owner: x86-m64; leaves
//! with the pin's behaviour or when the bar changes.
//!
//! # Example
//! No honest example: its output is the bytes and relocations of an object, not
//! LIR. `-S` prints the items it encodes (see `masm`); `tests` in this file
//! assert the bytes.
//!
//! # Runs
//! Once per module, after `assemble` and `data layout` have produced the masm
//! module, when a frontend writes the object (`llrm-cc`'s `compile.rs` calls
//! `written_in`). The BASIC driver (`driver/basic.rs`) reuses `_data`,
//! `_code_by` and `object_of` with its own listing and header.
//!
//! # See also
//! [`crate::backend::masm`] for the items, `asm` for the encoder.

use std::collections::BTreeSet;
use std::fmt;

use llrm_debug::{DebugObjectTarget, Described};
use llrm_lir::registers::Regs;
use llrm_object::assembler::AsmBackend;
use llrm_object::{Binding, Definition, Fix, Object, ObjectTarget, OmfGroup, Reloc, Role, Section, Symbol, Target};
use llrm_support::hash::HashSet;
use llrm_target::object::{CodeModel, Format};
use llrm_x86::mc::backend::X86Backend;

use crate::backend::masm;
use crate::backend::select;
use crate::model::ir::{self, Loc, Operation, Semantics, Space};
use crate::model::lir::SrcLoc;
use crate::objectfile::omf;
use crate::support::hash::IndexMap;
use crate::support::pyrepr::{self, Repr};

/// How an address is relocated: a near offset, a segment's selector, a far
/// pointer.
pub const OFFSET: Fix = Fix::abs(2);
pub const BASE: Fix = Fix::plain(llrm_x86::mc::fixup::SEGMENT_BASE);
pub const POINTER: Fix = Fix::plain(llrm_x86::mc::fixup::FAR_POINTER);

/// The bytes `fix` patches. `FixupKind::width` knows only the generic kinds and
/// says 0 for x86's, which made a far pointer in a 16-bit object trip the
/// width assertion.
fn width(fix: Fix) -> usize {
    usize::from(backend(false, 0).kind_info(fix.kind).bits).div_ceil(8)
}

/// x86's assembler backend for `sixteen`-bit mode and a longest no-operation of
/// `longest` bytes (its fixup table needs neither).
fn backend(
    sixteen: bool,
    longest: usize,
) -> X86Backend {
    X86Backend { sixteen, wide: false, longest }
}
pub const OFFSET32: Fix = Fix::abs(4);
pub const OFFSET64: Fix = Fix::abs(8);

/// A call's field: relative to its own end.
fn relative(width: usize) -> Fix {
    llrm_x86::mc::fixup::branch(width)
}

/// The role of a data segment `name` that the object's classes name, and that
/// it is otherwise.
fn data_role(
    name: &str,
    far_bss: bool,
) -> Role {
    match name {
        "_DATA" => Role::Data,
        "_BSS" | "BSSB" => Role::Bss,
        "CONST" => Role::ROData,
        _ if name.starts_with(".rdata$") => Role::ROData,
        _ if name.starts_with(".bss$") => Role::Bss,
        // The pin's mergeable constant sections (`.rodata.cst4`...).
        _ if name.starts_with(".rodata.cst") => Role::ROData,
        crate::backend::globals::INIT_SEGMENT => Role::InitFunctions,
        _ if far_bss => Role::Bss,
        _ => Role::Data,
    }
}

/// The name of the segment the COMDAT member `symbol` is written in: one per
/// member, so the members of one comdat stay apart.
fn comdat_segment(
    symbol: &str,
    comdat: &masm::Comdat,
) -> String {
    let base = match (&comdat.extent, comdat.constant) {
        (None, _) => ".text",
        (Some((segment, _)), _) if segment == "_BSS" => ".bss",
        (Some(_), true) => ".rdata",
        (Some(_), false) => ".data",
    };
    format!("{base}${symbol}")
}

/// COFF: each COMDAT function in a code segment of its own, and each COMDAT
/// global's items out of their segment into one of its own, so the linker
/// can keep one definition of each.
fn comdat_sections(module: &mut masm::Module) {
    let comdats: Vec<(String, masm::Comdat)> = module.comdats.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let mut cuts: Vec<(String, std::ops::Range<usize>, String)> = Vec::new();
    for (symbol, comdat) in &comdats {
        match &comdat.extent {
            None => {
                module.code_segments.entry(symbol.clone()).or_insert_with(|| comdat_segment(symbol, comdat));
            }
            Some((segment, range)) => cuts.push((segment.clone(), range.clone(), comdat_segment(symbol, comdat))),
        }
    }
    // Later ranges first, so the earlier ones stay where they were recorded.
    cuts.sort_by(|a, b| b.1.start.cmp(&a.1.start));
    for (segment, range, into) in cuts {
        let Some(at) = module.data.iter().position(|(name, _)| *name == segment) else { continue };
        let items: Vec<masm::Datum> = module.data[at].1.drain(range).collect();
        match module.data.iter_mut().find(|(name, _)| *name == into) {
            Some((_, existing)) => existing.splice(0..0, items).for_each(drop),
            None => module.data.push((into, items)),
        }
    }
}

/// The sections `comdat_sections` made, as the object's COMDAT records.
fn comdat_records(
    module: &masm::Module,
    object: &Object,
) -> Vec<llrm_object::Comdat> {
    // A member that is not named like its comdat follows the one that is, when
    // the module has it: associative with that member's section.
    let named = |symbol: &str, comdat: &masm::Comdat| {
        module.comdats.get(&comdat.name).map(|key| comdat_segment(&comdat.name, key)).filter(|_| symbol != comdat.name)
    };
    let by_name: IndexMap<String, (llrm_object::Selection, Option<String>)> = module
        .comdats
        .iter()
        .map(|(symbol, one)| {
            let key = named(symbol, one);
            let selection = if key.is_some() { llrm_object::Selection::Associative } else { one.selection };
            (comdat_segment(symbol, one), (selection, key))
        })
        .collect();
    let section_of = |name: &str| object.sections.iter().position(|one| one.name == name);
    object
        .sections
        .iter()
        .enumerate()
        .filter_map(|(section, one)| {
            let (selection, key) = by_name.get(&one.name)?;
            Some(llrm_object::Comdat {
                section,
                selection: *selection,
                associated: key.as_deref().and_then(section_of).filter(|&at| at != section),
            })
        })
        .collect()
}

/// An instruction or reference this writer has no bytes for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unencodable(pub String);

impl fmt::Display for Unencodable {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unencodable {}

/// A phi reached emission. Always a bug in phi elimination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Survived(pub String);

impl fmt::Display for Survived {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
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
    /// The object says something its format cannot.
    Unsupported(llrm_object::Unsupported),
}

impl fmt::Display for Error {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Error::Unprintable(one) => one.fmt(formatter),
            Error::Unencodable(one) => one.fmt(formatter),
            Error::Value(one) => formatter.write_str(&one.0),
            Error::Survived(one) => one.fmt(formatter),
            Error::Unsupported(one) => one.fmt(formatter),
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

impl From<llrm_object::Unsupported> for Error {
    fn from(one: llrm_object::Unsupported) -> Self {
        Error::Unsupported(one)
    }
}

impl From<llrm_omf::write::Error> for Error {
    fn from(one: llrm_omf::write::Error) -> Self {
        match one {
            llrm_omf::write::Error::Unencodable(text) => Error::Unencodable(Unencodable(text)),
            llrm_omf::write::Error::Value(one) => Error::Value(one),
        }
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
    pub kind: Fix,
    pub name: String,
}

impl Fixup {
    pub fn new(
        at: usize,
        kind: Fix,
        name: impl Into<String>,
    ) -> Self {
        Self { at, kind, name: name.into() }
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
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self { name: name.into(), label: label.into(), long: false }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Near {
    pub name: String,
    /// A sibling call's `jmp` (`E9`), not a `call` (`E8`).
    pub tail: bool,
    /// The conditional jump a sibling call is folded into (`jcc name`).
    pub cond: Option<String>,
}

impl Near {
    /// The opcode bytes before the displacement.
    fn opcode(
        &self,
        model: CodeModel,
    ) -> Result<Vec<u8>, Unencodable> {
        let Some(cond) = &self.cond else {
            return Ok(vec![if self.tail { 0xE9 } else { 0xE8 }]);
        };
        let at = select::At { ip: 0, bits: model.bits() };
        let long = select::branch(cond, 0, at, false).filter(|made| made.code.len() == 2 + model.displacement_bytes());
        long.map(|made| made.code[..2].to_vec()).ok_or_else(|| Unencodable(format!("{cond} to {}", self.name)))
    }
}

/// `Encoded`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Encoded {
    Label(masm::Label),
    Piece(Piece),
    Jump(Jump),
    Near(Near),
    /// A mark, and the symbol a line's defines.
    Mark(masm::Mark, Option<String>),
    /// A `.cfi_*` directive: no bytes.
    Cfi(llrm_object::assembler::Cfi),
    /// A `.seh_*` directive: no bytes.
    Seh(llrm_object::Seh),
    /// `MCAlignFragment`: nops up to the alignment.
    Align(masm::CodeAlign),
    /// `.prefalign` (`MCAssembler::relaxPrefAlign`): a function aligned to
    /// at most `preferred` (a power of two), and to no more than the least
    /// power of two its body fills, which ends at the `End` named `end`;
    /// `computed` is what the layout has settled on so far.
    Prefer {
        preferred: u8,
        computed: u8,
        end: String,
    },
    /// Where a `Prefer`'s body ends.
    End(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
    pub name: String,
    pub role: Role,
    /// Not reached by a selector of its own.
    pub near: bool,
    pub image: Vec<u8>,
    /// [start, end) holding data
    pub spans: Vec<[usize; 2]>,
    pub fixups: Vec<Fixup>,
    /// (source line, offset) of each line's first code, in order.
    pub lines: Vec<(SrcLoc, usize)>,
    /// Where a line row is a statement whatever the row before it says.
    pub forced: Vec<usize>,
    /// Each procedure body's bounds, in order.
    pub bodies: Vec<(masm::Mark, usize)>,
    /// The declared call frame information and where it stands.
    pub cfi: Vec<(llrm_object::assembler::Cfi, usize)>,
    /// The declared `.seh_*` directives and the offset after each.
    pub seh: Vec<(llrm_object::Seh, usize)>,
    /// Where a procedure that declares call frame information starts.
    pub declared: Vec<usize>,
    /// The widest alignment a data item in it asks for.
    pub align: usize,
}

impl Segment {
    pub fn new(
        name: &str,
        role: Role,
        near: bool,
    ) -> Self {
        Self {
            name: name.to_owned(),
            role,
            near,
            image: Vec::new(),
            spans: Vec::new(),
            fixups: Vec::new(),
            lines: Vec::new(),
            forced: Vec::new(),
            bodies: Vec::new(),
            cfi: Vec::new(),
            seh: Vec::new(),
            declared: Vec::new(),
            align: 1,
        }
    }

    /// `mark` is here.
    pub fn mark(
        &mut self,
        mark: masm::Mark,
    ) {
        match mark {
            masm::Mark::Line { line, .. } => self.line(line),
            masm::Mark::ForceStmt => self.forced.push(self.image.len()),
            masm::Mark::Declared => self.declared.push(self.image.len()),
            bound => self.bodies.push((bound, self.image.len())),
        }
    }

    /// Code at the current offset is `line`'s: the last line named at an
    /// offset wins, as the one before it has no code.
    pub fn line(
        &mut self,
        line: SrcLoc,
    ) {
        let at = self.image.len();
        if self.lines.last().is_some_and(|&(_, last)| last == at) {
            self.lines.pop();
        }
        if self.lines.last().is_none_or(|&(last, _)| last != line) {
            self.lines.push((line, at));
        }
    }

    pub fn put(
        &mut self,
        code: &[u8],
        fixups: &[Fixup],
    ) {
        let at = self.image.len();
        self.fixups.extend(fixups.iter().map(|one| Fixup { at: at + one.at, ..one.clone() }));
        self.image.extend_from_slice(code);
        match self.spans.last_mut() {
            Some(last) if last[1] == at => last[1] += code.len(),
            _ if !code.is_empty() => self.spans.push([at, at + code.len()]),
            _ => {}
        }
    }

    pub fn skip(
        &mut self,
        size: usize,
    ) {
        self.image.extend(std::iter::repeat_n(0, size));
    }
}

/// `struct.pack_into("<H", buffer, at, value)` of a value already masked.
pub fn pack_into(
    buffer: &mut [u8],
    at: usize,
    value: i64,
) {
    buffer[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

/// `value` into the bytes of a relocated field at `at` that hold an offset (a
/// far pointer's are its first two), refused where it does not fit them.
fn pack_field(
    buffer: &mut [u8],
    at: usize,
    kind: Fix,
    value: i64,
) -> Result<(), llrm_object::Unsupported> {
    let bytes = if kind == POINTER { 2 } else { width(kind) };
    // A far pointer's offset is its first two bytes: patched as data of that
    // width.
    llrm_x86::mc::backend::FIELDS.apply_fixup(buffer, at, Fix::abs(bytes).kind, kind.pc_rel, value, "")
}

/// What a relocated field holds, sign-extended from the bytes that hold an
/// offset.
fn field(
    buffer: &[u8],
    at: usize,
    kind: Fix,
) -> i64 {
    if width(kind) == 8 {
        i64::from_le_bytes(buffer[at..at + 8].try_into().expect("eight bytes"))
    } else if width(kind) == 4 && kind != POINTER {
        i64::from(i32::from_le_bytes([buffer[at], buffer[at + 1], buffer[at + 2], buffer[at + 3]]))
    } else {
        i64::from(i16::from_le_bytes([buffer[at], buffer[at + 1]]))
    }
}

/// `module` without the data objects nothing reaches.
///
/// A `Datum::Object` starts a unit that stays only if code, a public, or a
/// kept unit names one of its labels. Items before a segment's first Object
/// always stay.
pub fn live(module: &masm::Module) -> Result<masm::Module, Error> {
    llrm_support::debug::timed("omf live", || live_inner(module))
}

fn live_inner(module: &masm::Module) -> Result<masm::Module, Error> {
    let regs = module.registers;
    if !module.data.iter().any(|(_, items)| items.iter().any(|item| matches!(item, masm::Datum::Object(_)))) {
        return Ok(module.clone());
    }
    let mut reached: BTreeSet<String> =
        module.publics.iter().cloned().chain(module.aliases.iter().map(|alias| alias.target.clone())).collect();
    for (number, procedure) in module.procedures.iter().enumerate() {
        reached.insert(procedure.name.clone());
        for item in masm::listing(procedure, number)? {
            for one in _items(regs, &item, &module.names, number, module.object.code_model())? {
                match one {
                    Encoded::Piece(Piece { fixups, .. }) => {
                        reached.extend(fixups.iter().map(|fixup| _target(&fixup.name).to_owned()));
                    }
                    Encoded::Near(Near { name, .. }) => {
                        reached.insert(name);
                    }
                    Encoded::Label(_)
                    | Encoded::Jump(_)
                    | Encoded::Mark(..)
                    | Encoded::Cfi(_)
                    | Encoded::Seh(_)
                    | Encoded::Align(_)
                    | Encoded::Prefer { .. }
                    | Encoded::End(_) => {}
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
            if let masm::Datum::Pointer(masm::Pointer { name, .. })
            | masm::Datum::SegmentWord(name)
            | masm::Datum::Difference(masm::Difference { name, .. }) = item
            {
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
    /// Each procedure its own unit for the linker, which drops each one nothing
    /// calls (`option eliminate`, /PACKFUNCTIONS). An OMF object writes each as
    /// a COMDAT; a near call between them is a relative fixup to the other's
    /// name.
    PerProcedure,
}

pub fn written(
    module: &masm::Module,
    source: &str,
    objects: &dyn DebugObjectTarget,
) -> Result<Vec<u8>, Error> {
    written_as(module, source, CodeLayout::OneSegment, objects)
}

pub fn written_as(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    objects: &dyn DebugObjectTarget,
) -> Result<Vec<u8>, Error> {
    written_in(module, source, layout, Format::Omf, objects)
}

/// `module` as an object file of `format`.
pub fn written_in(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    format: Format,
    objects: &dyn DebugObjectTarget,
) -> Result<Vec<u8>, Error> {
    llrm_support::debug::timed("object write", || {
        let object = built_inner(module, source, layout, format, objects)?;
        let bytes = serialized(objects, &object, format)?;
        if let Some(dir) = llrm_target::object::all_writers_dir() {
            for other in Format::ALL {
                let written = if other == format { Ok(bytes.clone()) } else { serialized(objects, &object, other) };
                match written {
                    Ok(written) => llrm_target::object::record_writer(&dir, other, &written),
                    Err(why) => llrm_target::object::record_refusal(&dir, other, &format!("{why:?}")),
                }
            }
        }
        Ok(bytes)
    })
}

/// The bytes of `module` as an object holds them without alignment padding: each function's
/// body (its symbol's size, not the padding before it) and each data section's image. The
/// size a code-size trial compares: the padding between functions is the layout's, not the
/// code's.
pub fn unpadded_bytes(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    format: Format,
    objects: &dyn DebugObjectTarget,
) -> Result<i64, Error> {
    let (code, data) = unpadded_split(module, source, layout, format, objects, false)?;
    Ok(code + data)
}

/// [`unpadded_bytes`] as the functions' code and the data sections' images. `flat`: the code as the
/// instructions fill it, without the alignment a block pads to inside a function either (a code-size
/// column decodes the instructions and leaves the padding out).
pub fn unpadded_split(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    format: Format,
    objects: &dyn DebugObjectTarget,
    flat: bool,
) -> Result<(i64, i64), Error> {
    let mut unaligned;
    let module = if flat {
        unaligned = module.clone();
        for block in unaligned.procedures.iter_mut().flat_map(|procedure| procedure.body.blocks.iter_mut()) {
            block.align = 0;
        }
        &unaligned
    } else {
        module
    };
    use llrm_object::{Definition, Role};
    let object = built_inner(module, source, layout, format, objects)?.object;
    let code: usize = object
        .symbols
        .iter()
        .filter_map(|symbol| match symbol.definition {
            Definition::Defined { section, .. } if object.sections[section].role == Role::Text => Some(symbol.size),
            _ => None,
        })
        .sum();
    let data: usize = object
        .sections
        .iter()
        .filter(|section| matches!(section.role, Role::ROData | Role::Data))
        .map(|section| section.image.len())
        .sum();
    Ok((code as i64, data as i64))
}

/// What `format` says of the code it holds, from the target's table.
pub fn asm_info(
    objects: &dyn ObjectTarget,
    format: Format,
) -> llrm_object::AsmInfo {
    objects.asm_info(format.name()).unwrap_or_default()
}

/// `object` as a file of `format`.
pub fn serialized(
    objects: &dyn DebugObjectTarget,
    object: &Described,
    format: Format,
) -> Result<Vec<u8>, Error> {
    Ok(objects.write_described(format.name(), &object.object, object.debug.as_ref())?)
}

/// `module` laid out as an object.
pub fn built(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    objects: &dyn DebugObjectTarget,
) -> Result<Described, Error> {
    built_inner(module, source, layout, Format::Omf, objects)
}

/// [`built`] for `format`, which decides the code too (a Windows 64-bit frame
/// past a page is probed).
pub fn built_in(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    format: Format,
    objects: &dyn DebugObjectTarget,
) -> Result<Described, Error> {
    built_inner(module, source, layout, format, objects)
}

/// A Windows 64-bit function probes the frames past a page through `__chkstk`
/// (`X86FrameLowering::emitStackProbe`): the format decides it, as the triple
/// does for LLVM. Only a target whose description names the probe's scratch
/// register has the probe.
fn stack_probed(
    module: &mut masm::Module,
    format: Format,
) {
    if format == Format::Coff {
        for procedure in module.procedures.iter_mut().filter(|one| one.registers.slot_scratch.is_some()) {
            procedure.probe_stack = Some("__chkstk".to_owned());
        }
        // Win64 has no red zone.
        for procedure in module.procedures.iter_mut() {
            procedure.registers.red_zone = 0;
        }
    }
}

/// The pin states no frame address at an exit on Darwin or Windows
/// (`X86FrameLowering::emitEpilogue`: `NeedsDwarfCFI` is off for them); the
/// format decides, as the triple does.
fn exits_unstated(
    module: &mut masm::Module,
    format: Format,
) {
    if matches!(format, Format::MachO | Format::Coff) {
        for procedure in &mut module.procedures {
            procedure.registers.exit_cfi = false;
        }
    }
}

/// [`built`] for the writer of `format`: what that format's object holds.
pub fn built_for(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    format: Format,
    objects: &dyn DebugObjectTarget,
) -> Result<Described, Error> {
    built_inner(module, source, layout, format, objects)
}

/// Whether `format` drops the data no code reaches: an OMF object does; the
/// jump tables of the others are read by the procedure that has them.
fn drops_unreached_data(format: Format) -> bool {
    format == Format::Omf
}

/// Whether `format` rewrites the procedures before they are encoded
/// (`stack_probed`, `exits_unstated`).
fn changes_procedures(format: Format) -> bool {
    matches!(format, Format::Coff | Format::MachO)
}

fn built_inner(
    module: &masm::Module,
    source: &str,
    layout: CodeLayout,
    format: Format,
    objects: &dyn DebugObjectTarget,
) -> Result<Described, Error> {
    // Only an OMF object drops data no code reaches (a jump table is always
    // read by the procedure that has it), and only COFF and Mach-O change the
    // procedures: the module is copied for those alone.
    let mut adjusted = std::borrow::Cow::Borrowed(module);
    if drops_unreached_data(format) {
        adjusted = std::borrow::Cow::Owned(live(module)?);
    }
    if changes_procedures(format) {
        stack_probed(adjusted.to_mut(), format);
        exits_unstated(adjusted.to_mut(), format);
    }
    if format == Format::Coff && !module.comdats.is_empty() {
        comdat_sections(adjusted.to_mut());
    }
    let module = &*adjusted;
    // A procedure the program put in a segment of its own naming (`code_seg`)
    // is in that one, the rest in `code`.
    let segment_of = |procedure: usize| -> &str {
        module.code_segments.get(&module.procedures[procedure].name).map_or(module.code.as_str(), String::as_str)
    };
    let groups: Vec<Vec<usize>> = match layout {
        CodeLayout::OneSegment => {
            let mut by_name: IndexMap<&str, Vec<usize>> = IndexMap::default();
            by_name.entry(module.code.as_str()).or_default();
            for one in 0..module.procedures.len() {
                by_name.entry(segment_of(one)).or_default().push(one);
            }
            by_name.into_values().collect()
        }
        CodeLayout::PerProcedure => (0..module.procedures.len()).map(|one| vec![one]).collect(),
    };
    // A flat target has no group: every segment is reached by its own offset.
    let mut segments: Vec<Segment> = groups
        .iter()
        .map(|group| Segment::new(group.first().map_or(module.code.as_str(), |&one| segment_of(one)), Role::Text, true))
        .collect();
    let mut named: IndexMap<String, Segment> =
        IndexMap::from_iter([("_DATA".to_owned(), Segment::new("_DATA", Role::Data, true))]);
    // The pin keeps a Mach-O x86-64 jump table in `__text`, after the code: the
    // labels it differences are then in its own section and need no relocation.
    let in_text =
        |name: &str| format == Format::MachO && module.object.code_model() == CodeModel::Flat64 && name == "CONST";
    for (name, _items) in module.data.iter().filter(|(name, _)| !in_text(name)) {
        if !named.contains_key(name) {
            named.insert(
                name.clone(),
                Segment::new(name, data_role(name, module.far_bss.contains(name)), !module.selector_addressed(name)),
            );
        }
    }
    segments.extend(named.into_values());
    if module.stack > 0 {
        let mut stack = Segment::new("STACK", Role::Stack, true);
        stack.image = vec![0; module.stack as usize];
        segments.push(stack);
    }
    let mut symbols: IndexMap<String, (usize, usize)> = IndexMap::default();
    for (name, items) in module.data.iter().filter(|(name, _)| !in_text(name)) {
        let index = segments
            .iter()
            .position(|one| &one.name == name)
            .unwrap_or_else(|| panic!("ValueError: {} is not in list", pyrepr::string(name)));
        _data(&mut segments[index], index, items, &mut symbols, module.object.code_model());
    }
    let mut sizes: IndexMap<String, usize> = IndexMap::default();
    for (index, group) in groups.iter().enumerate() {
        _code(
            &mut segments[index],
            index,
            module,
            group,
            &mut symbols,
            &mut sizes,
            asm_info(objects, format).pref_align,
        )?;
    }
    for (_, items) in module.data.iter().filter(|(name, _)| in_text(name)) {
        _data(&mut segments[0], 0, items, &mut symbols, module.object.code_model());
    }
    // The BASIC dialect's records name an address by its offset alone, so they
    // need the one code segment the module's code is in; CodeView 4 as C7
    // writes it has the segment in each address.
    if module.debug.as_ref().is_some_and(|debug| debug.dialect == llrm_debug::Dialect::Bc) && groups.len() != 1 {
        return Err(Unencodable("-g of the BASIC dialect with a code segment per procedure".into()).into());
    }
    let mut externs: IndexMap<String, String> = module.externs.iter().cloned().collect();
    // The probe routine is named by the frames that need it, nowhere else.
    for probe in module.procedures.iter().filter_map(|one| one.probe_stack.as_ref()) {
        if segments.iter().any(|segment| segment.fixups.iter().any(|one| _target(&one.name) == probe)) {
            externs.entry(probe.clone()).or_insert_with(|| "near".to_owned());
        }
    }
    let mut object = object_of(module, source, segments, &symbols, &sizes, &externs)?;
    if format == Format::Coff {
        object.comdats = comdat_records(module, &object);
    }
    // A segment per procedure of one name is one segment to Microsoft's LINK,
    // which keeps the last one's length and loses the rest; a COMDAT per
    // procedure is what MSC's /Gy and Watcom's -zm write, which every linker
    // places, packs and drops where nothing refers to it.
    if layout == CodeLayout::PerProcedure {
        object.omf_comdat = (0..groups.len()).collect();
    }
    Ok(object)
}

pub fn _data(
    segment: &mut Segment,
    index: usize,
    items: &[masm::Datum],
    symbols: &mut IndexMap<String, (usize, usize)>,
    model: CodeModel,
) {
    for item in items {
        match item {
            masm::Datum::Label(masm::Label { name }) | masm::Datum::Object(masm::Label { name }) => {
                symbols.insert(name.clone(), (index, segment.image.len()));
            }
            masm::Datum::Fill(masm::Fill { size, byte: None }) => segment.skip(*size as usize),
            masm::Datum::Fill(masm::Fill { size, byte: Some(byte) }) => segment.put(&vec![*byte; *size as usize], &[]),
            masm::Datum::Pointer(masm::Pointer { name, offset, far, bytes }) => {
                // A far pointer is an offset and a selector; a near one is the
                // offset alone, as wide as the target's pointer
                // (a 32-bit object's is a dword).
                let loc = if *far {
                    POINTER
                } else {
                    match model {
                        CodeModel::Flat64 => OFFSET64,
                        CodeModel::Flat32 => OFFSET32,
                        CodeModel::Segmented16 => OFFSET,
                    }
                };
                assert_eq!(width(loc), *bytes as usize, "a {bytes}-byte pointer in a {}-bit object", model.bits());
                segment.put(&vec![0; width(loc)], &[Fixup::new(0, loc, name.clone())]);
                let at = segment.image.len() - width(loc);
                if loc == OFFSET64 {
                    segment.image[at..at + 8].copy_from_slice(&offset.to_le_bytes());
                } else if width(loc) == 4 && !*far {
                    // A dword offset: both words, `pack_into` stores only the low one.
                    segment.image[at..at + 4].copy_from_slice(&(*offset as u32).to_le_bytes());
                } else {
                    pack_into(&mut segment.image, at, offset & ((1_i64 << (8 * (*bytes).min(4))) - 1));
                }
            }
            masm::Datum::Align(masm::Align { to }) => {
                segment.align = segment.align.max(*to as usize);
                let pad = (-(segment.image.len() as i64)).rem_euclid(*to) as usize;
                // Uninitialised data is zeroed at start-up: its padding is not stored (a stored byte
                // makes the linker store the segment up to it).
                if segment.role == Role::Bss {
                    segment.skip(pad);
                } else {
                    segment.put(&vec![0; pad], &[]);
                }
            }
            masm::Datum::Bytes(item) => segment.put(item, &[]),
            masm::Datum::SegmentWord(name) => segment.put(&[0, 0], &[Fixup::new(0, BASE, name.clone())]),
            // `name - base`: the pc-relative fixup is `name - slot`, so the
            // field holds `slot - base`.
            masm::Datum::Difference(masm::Difference { name, base }) => {
                let slot = segment.image.len();
                let table = symbols[base].1;
                match symbols.get(name) {
                    // Both in this segment: the assembler's answer, no
                    // relocation (Mach-O keeps the table in `__text`).
                    Some(&(at, offset)) if at == index => {
                        segment.put(&[0; 4], &[]);
                        segment.image[slot..slot + 4]
                            .copy_from_slice(&((offset as i64 - table as i64) as i32).to_le_bytes());
                    }
                    _ => {
                        segment.put(&[0; 4], &[Fixup::new(0, Fix::pc_rel(4, 0), name.clone())]);
                        segment.image[slot..slot + 4]
                            .copy_from_slice(&((slot as i64 - table as i64) as i32).to_le_bytes());
                    }
                }
            }
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
    sizes: &mut IndexMap<String, usize>,
    pref_align: bool,
) -> Result<(), Error> {
    _code_by(segment, index, module, group, symbols, sizes, pref_align, |procedure, number| {
        masm::listing(procedure, number).map_err(|error| error.0)
    })
}

/// [`_code`], each procedure's items as `listed` gives them.
pub fn _code_by(
    segment: &mut Segment,
    index: usize,
    module: &masm::Module,
    group: &[usize],
    symbols: &mut IndexMap<String, (usize, usize)>,
    sizes: &mut IndexMap<String, usize>,
    pref_align: bool,
    listed: impl for<'p> Fn(&'p masm::Procedure, usize) -> Result<Vec<masm::Item<'p>>, String>,
) -> Result<(), Error> {
    let regs = module.registers;
    let mut items: Vec<Encoded> = Vec::new();
    for &number in group {
        let procedure = &module.procedures[number];
        // `MachineFunction::getPreferredAlignment`: none for a function
        // optimized for size. ELF writes `.prefalign` (the alignment is no
        // more than the body fills), the other formats `.p2align`.
        let function = module.padding.function;
        let end = format!("{}$end", procedure.name);
        let aligned = function > 0 && !procedure.size;
        if aligned && pref_align {
            items.push(Encoded::Prefer { preferred: function, computed: 0, end: end.clone() });
        } else if aligned {
            items.push(Encoded::Align(masm::CodeAlign { log2: function, max_skip: 0 }));
        }
        items.push(Encoded::Label(masm::Label { name: procedure.name.clone() }));
        for item in listed(procedure, number).map_err(Unencodable)? {
            match _items(regs, &item, &module.names, number, module.object.code_model()) {
                Ok(encoded) => items.extend(encoded),
                Err(error) => return Err(Unencodable(format!("{}: {error}", procedure.name)).into()),
            }
        }
        items.push(Encoded::End(end));
        let table = module.code_data.get(&procedure.name).map_or(&[][..], Vec::as_slice);
        items.extend(_table(table, module.object.code_model())?);
    }
    left_to_the_linker(&mut items, &module.preemptible, module.object.code_model())?;
    near_within_group(&mut items, module.object.code_model());
    // Tuned for size, a long conditional jump may go through a `jmp` within
    // reach.
    let size = !group.is_empty() && group.iter().all(|&number| module.procedures[number].size);
    let model = module.object.code_model();
    let labels = if size { _trampolined(&mut items, model)? } else { _relaxed(&mut items, model)? };
    let mut at = 0;
    for item in &items {
        match item {
            Encoded::Label(masm::Label { name }) => {
                symbols.insert(name.clone(), (index, at));
            }
            Encoded::Cfi(cfi) => segment.cfi.push((*cfi, segment.image.len())),
            Encoded::Seh(seh) => segment.seh.push((*seh, segment.image.len())),
            Encoded::Mark(mark, name) => {
                segment.mark(*mark);
                if let Some(name) = name {
                    symbols.insert(name.clone(), (index, at));
                }
            }
            Encoded::Piece(Piece { code, fixups }) => segment.put(code, fixups),
            Encoded::End(end) => {
                // `.size`: the body alone, not the padding before it.
                let name = end.strip_suffix("$end").expect("an end label is named for its procedure");
                sizes.insert(name.to_string(), at - symbols[name].1);
            }
            Encoded::Align(_) | Encoded::Prefer { .. } => {
                let pad = _size(item, at as i64, model);
                let mut nops = Vec::new();
                backend(model.segmented(), usize::from(module.padding.nop)).write_nop_data(&mut nops, pad);
                segment.put(&nops, &[]);
                let log2 = match item {
                    Encoded::Align(masm::CodeAlign { log2, .. }) => *log2,
                    Encoded::Prefer { computed, .. } => *computed,
                    _ => 0,
                };
                segment.align = segment.align.max(1 << log2);
            }
            Encoded::Jump(Jump { name, label, long }) => {
                segment.put(&_jump(name, labels[label], at, *long, model)?.code, &[])
            }
            Encoded::Near(near) if labels.contains_key(&near.name) => {
                let name = &near.name;
                let opcode = near.opcode(model)?;
                let distance = labels[name] - (at as i64 + opcode.len() as i64 + model.displacement_bytes() as i64);
                let far = || Unencodable(format!("a near call to {name} {distance} bytes away"));
                let displacement = if model.flat() {
                    i32::try_from(distance).map_err(|_| far())?.to_le_bytes().to_vec()
                } else {
                    // Real mode adds the 16-bit displacement to IP modulo 64K, so
                    // any offset of the segment is reachable (+33288 is -32248);
                    // only a callee beyond the segment is not.
                    if distance.unsigned_abs() >= 0x1_0000 {
                        return Err(far().into());
                    }
                    (distance as i16).to_le_bytes().to_vec()
                };
                segment.put(&[&opcode[..], &displacement].concat(), &[]);
            }
            Encoded::Near(near) => {
                let opcode = near.opcode(model)?;
                segment.put(
                    &vec![0; opcode.len() + model.displacement_bytes()],
                    &[Fixup::new(opcode.len(), relative(model.displacement_bytes()), near.name.clone())],
                );
                segment.image[at..at + opcode.len()].copy_from_slice(&opcode);
            }
        }
        at = segment.image.len();
    }
    Ok(())
}

/// A far call or jump to a procedure of the group the linker puts in the call's
/// own segment is `push cs; call near` (a near `jmp`): four bytes (three),
/// where jwlink's own rewrite of `call far` keeps five and pads one with a `ds`
/// prefix at one parity and a `nop` that runs at the other (0.60-0.85% of
/// stft_fixed's executed instructions). The callee returns with `retf` either
/// way.
fn near_within_group(
    items: &mut Vec<Encoded>,
    model: CodeModel,
) {
    if !model.segmented() {
        return;
    }
    let defined: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Encoded::Label(masm::Label { name }) => Some(name.clone()),
            _ => None,
        })
        .collect();
    let mut made = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        match &item {
            Encoded::Piece(Piece { code, fixups })
                if matches!(code.as_slice(), [0x9A | 0xEA, 0, 0, 0, 0])
                    && fixups.len() == 1
                    && fixups[0].at == 1
                    && defined.contains(&fixups[0].name) =>
            {
                let tail = code[0] == 0xEA;
                if !tail {
                    made.push(Encoded::Piece(Piece::new(vec![0x0E])));
                }
                made.push(Encoded::Near(Near { name: fixups[0].name.clone(), tail, cond: None }));
            }
            _ => made.push(item),
        }
    }
    *items = made;
}

/// The read-only data that follows a procedure in its code segment
/// (`masm::Module::code_data`), as bytes between labels.
fn _table(
    datums: &[masm::Datum],
    model: CodeModel,
) -> Result<Vec<Encoded>, Unencodable> {
    let near = if model.flat() { OFFSET32 } else { OFFSET };
    datums
        .iter()
        .map(|item| match item {
            masm::Datum::Label(label) | masm::Datum::Object(label) => Ok(Encoded::Label(label.clone())),
            masm::Datum::Bytes(bytes) => Ok(Encoded::Piece(Piece::new(bytes.clone()))),
            masm::Datum::Align(masm::Align { to }) => {
                Ok(Encoded::Align(masm::CodeAlign { log2: to.max(&1).ilog2() as u8, max_skip: 0 }))
            }
            masm::Datum::Pointer(masm::Pointer { name, offset, far: false, bytes }) => {
                let mut code = vec![0; *bytes as usize];
                pack_field(&mut code, 0, near, *offset).map_err(|error| Unencodable(error.0))?;
                Ok(Encoded::Piece(Piece { code, fixups: vec![Fixup::new(0, near, name.clone())] }))
            }
            other => Err(Unencodable(format!("{other:?} in a code segment"))),
        })
        .collect()
}

pub fn _items(
    regs: Regs,
    item: &masm::Item,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
    model: CodeModel,
) -> Result<Vec<Encoded>, Unencodable> {
    Ok(match item {
        masm::Item::Align(align) => vec![Encoded::Align(*align)],
        masm::Item::Label(label) => vec![Encoded::Label(label.clone())],
        masm::Item::Mark(mark @ masm::Mark::Line { index, .. }) => {
            vec![Encoded::Mark(*mark, Some(masm::line_label(number, *index)))]
        }
        masm::Item::Mark(mark) => vec![Encoded::Mark(*mark, None)],
        masm::Item::Cfi(cfi) => vec![Encoded::Cfi(*cfi)],
        masm::Item::Seh(seh) => vec![Encoded::Seh(*seh)],
        masm::Item::Callee(masm::Callee { code, .. }) if !code.is_empty() => code
            .iter()
            .map(|part| _part(part, model))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(Encoded::Piece)
            .collect(),
        masm::Item::Callee(masm::Callee { name, far: true, .. }) if model == CodeModel::Flat32 => {
            return Err(Unencodable(format!("a far call to {name} in flat code")));
        }
        masm::Item::Callee(masm::Callee { name, far: true, tail, .. }) => vec![Encoded::Piece(Piece {
            code: vec![if *tail { 0xEA } else { 0x9A }, 0, 0, 0, 0],
            fixups: vec![Fixup::new(1, POINTER, name.clone())],
        })],
        masm::Item::Callee(masm::Callee { name, tail, .. }) => {
            vec![Encoded::Near(Near { name: name.clone(), tail: *tail, cond: None })]
        }
        masm::Item::TailIf { condition, callee } => {
            vec![Encoded::Near(Near { name: callee.clone(), tail: true, cond: Some(condition.clone()) })]
        }
        masm::Item::Semantics(jump) if matches!(jump.op, Operation::Branch | Operation::Jump) && !jump.indirect => {
            let Semantics { name, target, .. } = &**jump;
            let name = name.as_deref().filter(|one| !one.is_empty());
            let Some(target) = target else {
                return Err(Unencodable(format!("{} with no target", name.unwrap_or("jump"))));
            };
            vec![Encoded::Jump(Jump::new(name.unwrap_or("jmp"), masm::label(number, *target)))]
        }
        masm::Item::Semantics(what) => vec![Encoded::Piece(_encoded(regs, what, names, model)?)],
    })
}

pub fn _part(
    part: &masm::InlinePart,
    model: CodeModel,
) -> Result<Piece, Unencodable> {
    match part {
        masm::InlinePart::Bytes(part) => Ok(Piece::new(part.clone())),
        masm::InlinePart::Near(opcode, name) => Ok(Piece {
            code: [&[*opcode][..], &vec![0; model.bits() as usize / 8][..]].concat(),
            fixups: vec![Fixup::new(1, relative(model.bits() as usize / 8), name.clone())],
        }),
        masm::InlinePart::Fixup(kind, name, offset) if kind == "offset" && model == CodeModel::Flat32 => Ok(Piece {
            code: (*offset as u32).to_le_bytes().to_vec(),
            fixups: vec![Fixup::new(0, OFFSET32, name.clone())],
        }),
        masm::InlinePart::Fixup(kind, name, offset) if kind == "offset" => Ok(Piece {
            code: (*offset as u16).to_le_bytes().to_vec(),
            fixups: vec![Fixup::new(0, OFFSET, name.clone())],
        }),
        masm::InlinePart::Fixup(kind, name, _) if kind == "segment" => {
            Ok(Piece { code: vec![0; 2], fixups: vec![Fixup::new(0, BASE, name.clone())] })
        }
        masm::InlinePart::Fixup(kind, name, offset) => {
            Err(Unencodable(format!("inline part ({}, {}, {offset})", pyrepr::string(kind), pyrepr::string(name))))
        }
    }
}

pub fn _encoded(
    regs: Regs,
    what: &Semantics,
    names: &IndexMap<(Space, i64), String>,
    model: CodeModel,
) -> Result<Piece, Unencodable> {
    let relocated = what.sources.iter().any(|one| matches!(one, Loc::Imm(ir::Imm { address: Some(_), .. })));
    // A near address is the code's offset.
    let near = if model.flat() { OFFSET32 } else { OFFSET };
    let Some(made) = select::emit_in(model.bits(), what, 0, None, false, relocated, None) else {
        return Err(Unencodable(what.repr()));
    };
    let mut code = made.code.clone();
    let mut fixups: IndexMap<usize, Fixup> = IndexMap::default();
    let beside_memory = what.dests.iter().chain(&what.sources).any(|one| matches!(one, Loc::Mem(_)));
    for one in what.dests.iter().chain(&what.sources) {
        let (at, loc, addend, addr) = match one {
            Loc::Mem(ir::Mem { addr: Some(addr), through, index_through, .. })
                if matches!(addr.space, Space::Segment | Space::External) =>
            {
                let wide = [through, index_through].into_iter().any(|one| regs.width_of(*one) == Some(4));
                (made.displacement_at, if wide { OFFSET32 } else { near }, addr.disp, addr)
            }
            Loc::Address(ir::AddressRef { addr: Some(addr), .. })
                if matches!(addr.space, Space::Segment | Space::External) =>
            {
                (made.displacement_at, near, addr.disp, addr)
            }
            Loc::Imm(ir::Imm { address: Some(addr), .. }) if addr.space == Space::Group => {
                (made.immediate_at, BASE, 0, addr)
            }
            // Long mode loads a symbol's address with `lea` from the
            // instruction pointer.
            // Static code loads it with `mov r32, imm32`: an absolute field.
            Loc::Imm(ir::Imm { address: Some(addr), value, width: 4, .. }) if model == CodeModel::Flat64 => {
                (made.immediate_at, near, addr.disp + value, addr)
            }
            // Beside a memory operand the displacement is the memory's: the
            // symbol is the sign-extended imm32 (`mov qword ptr [m], offset
            // a`).
            Loc::Imm(ir::Imm { address: Some(addr), value, .. }) if model == CodeModel::Flat64 && beside_memory => {
                (made.immediate_at, near, addr.disp + value, addr)
            }
            Loc::Imm(ir::Imm { address: Some(addr), value, .. }) if model == CodeModel::Flat64 => {
                (made.displacement_at, near, addr.disp + value, addr)
            }
            Loc::Imm(ir::Imm { address: Some(addr), value, .. }) => (made.immediate_at, near, addr.disp + value, addr),
            _ => continue,
        };
        let Some(at) = at else {
            return Err(Unencodable(format!("{}: no field for {}", what.repr(), one.repr())));
        };
        // Static code at 64 bits: a symbol beside a register is the absolute
        // sign-extended `disp32`; alone it is `[rip + disp32]`.
        let based = match one {
            Loc::Mem(ir::Mem { through, index_through, .. })
            | Loc::Address(ir::AddressRef { through, index_through, .. }) => {
                *through != llrm_lir::registers::RegId::None || *index_through != llrm_lir::registers::RegId::None
            }
            _ => false,
        };
        // Long mode reaches a symbol from the instruction pointer: the field is
        // relative to the instruction's end, and an address loaded for
        // a symbol defined elsewhere is its table entry's.
        let loc = if model == CodeModel::Flat64 && matches!(one, Loc::Imm(ir::Imm { width: 4, .. })) {
            loc
        } else if model == CodeModel::Flat64 && (based || (beside_memory && matches!(one, Loc::Imm(_)))) {
            llrm_x86::mc::fixup::abs_signed_4()
        } else if model == CodeModel::Flat64 {
            let from = code.len() - at;
            if matches!(one, Loc::Imm(_)) && addr.space == Space::External {
                llrm_x86::mc::fixup::got_pc_rel(from)
            } else {
                Fix::pc_rel(4, from)
            }
        } else {
            loc
        };
        pack_field(&mut code, at, loc, addend).map_err(|error| Unencodable(error.0))?;
        let name = names
            .get(&(addr.space, addr.index))
            .unwrap_or_else(|| panic!("KeyError: ({}, {})", addr.space.repr(), addr.index));
        fixups.insert(at, Fixup::new(at, loc, name.clone()));
    }
    Ok(Piece { code, fixups: fixups.into_values().collect() })
}

/// A short jump's length: opcode and an 8-bit displacement.
pub const SHORT_JUMP: i64 = 2;

/// Whether a short jump reaches `displacement`, counted from its end.
pub fn short_reaches(displacement: i64) -> bool {
    (-128..=127).contains(&displacement)
}

/// A sibling `jmp f` or `jcc f` to a function this object defines is a jump to
/// a label like any other, so it is relaxed like one: short, and long only
/// where `f` is out of 8-bit reach. Its displacement was filled in at 4 bytes
/// whatever the distance. LLVM's `JMP_1` / `JCC_1` start short and
/// `relaxInstruction` makes them `JMP_4` / `JCC_4` only for a target the
/// fixup cannot resolve in range
/// ([X86AsmBackend.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/MCTargetDesc/X86AsmBackend.cpp#L845)).
/// A call has no short form; a name not defined here stays a relocation.
fn resolved_siblings(items: &mut [Encoded]) {
    let defined: std::collections::HashSet<String> = items
        .iter()
        .filter_map(|item| match item {
            Encoded::Label(masm::Label { name }) => Some(name.clone()),
            _ => None,
        })
        .collect();
    for item in items {
        if let Encoded::Near(Near { name, tail: true, cond }) = item {
            if defined.contains(name) {
                *item = Encoded::Jump(Jump::new(cond.as_deref().unwrap_or("jmp"), name.clone()));
            }
        }
    }
}

/// A call or jump to a procedure another module may replace at link time stays
/// a relocation, as for a name defined elsewhere: MC resolves a PC-relative
/// reference only to a local symbol
/// ([ELFObjectWriter.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/MC/ELFObjectWriter.cpp#L1391)),
/// and gcc leaves the `-fPIC` ones to the PLT. Neither this nor `_relaxed`
/// ever binds such a name here.
fn left_to_the_linker(
    items: &mut [Encoded],
    preemptible: &std::collections::BTreeSet<String>,
    model: CodeModel,
) -> Result<(), Unencodable> {
    if preemptible.is_empty() {
        return Ok(());
    }
    for item in items {
        let Encoded::Near(near) = item else { continue };
        if !preemptible.contains(&near.name) {
            continue;
        }
        let opcode = near.opcode(model)?;
        let mut code = opcode.clone();
        code.resize(opcode.len() + model.displacement_bytes(), 0);
        *item = Encoded::Piece(Piece {
            code,
            fixups: vec![Fixup::new(opcode.len(), relative(model.displacement_bytes()), near.name.clone())],
        });
    }
    Ok(())
}

/// Every label's offset, with each jump short unless its target is out of
/// reach.
///
/// Short first and lengthened to a fixed point, as jwasm does: lengthening
/// only moves targets further away, so it ends, and at the smallest layout.
pub fn _relaxed(
    items: &mut [Encoded],
    model: CodeModel,
) -> Result<IndexMap<String, i64>, Unencodable> {
    resolved_siblings(items);
    let labels = llrm_object::assembler::layout(items, &model).map_err(|error| Unencodable(error.0))?;
    let mut named = IndexMap::default();
    for (name, at) in labels {
        named.insert(name, at);
    }
    Ok(named)
}

impl llrm_object::assembler::Fragment for Encoded {
    type Context = CodeModel;

    fn label(&self) -> Option<&str> {
        match self {
            Encoded::Label(masm::Label { name }) | Encoded::End(name) => Some(name),
            _ => None,
        }
    }

    fn size(
        &self,
        at: i64,
        model: &CodeModel,
    ) -> usize {
        _size(self, at, *model)
    }

    fn relax(
        &mut self,
        at: i64,
        labels: &llrm_support::hash::HashMap<String, i64>,
        _: &CodeModel,
    ) -> Result<bool, llrm_object::Unsupported> {
        match self {
            Encoded::Jump(item) if !item.long => {
                let Some(target) = labels.get(&item.label) else {
                    return Err(llrm_object::Unsupported(format!("a jump to {}, which is nowhere", item.label)));
                };
                if short_reaches(target - (at + SHORT_JUMP)) {
                    return Ok(false);
                }
                item.long = true;
                Ok(true)
            }
            // `relaxPrefAlign`: the body is what lies between the padding and
            // its end, and the alignment only grows.
            Encoded::Prefer { preferred, computed, end } => {
                let wanted = llrm_object::assembler::relax_pref_align(at, *computed, *preferred, labels[end.as_str()]);
                if wanted == *computed {
                    return Ok(false);
                }
                *computed = wanted;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

/// Tuned for size, [`_relaxed`], then each conditional jump still long (a 386
/// `jcc rel16`, 4 bytes) aimed at a label some `jmp` to it lies within short
/// reach of becomes a short jump to that `jmp`, which carries on: 2 bytes
/// saved, and the 3 clocks of a taken `jmp` more. Repeated while the shorter
/// layout brings more within reach; Watcom's `SetBranches`.
pub fn _trampolined(
    items: &mut Vec<Encoded>,
    model: CodeModel,
) -> Result<IndexMap<String, i64>, Unencodable> {
    let mut labels = _relaxed(items, model)?;
    loop {
        let mut starts = Vec::with_capacity(items.len());
        let mut at = 0i64;
        for item in items.iter() {
            starts.push(at);
            at += _size(item, at, model) as i64;
        }
        let mut jumps: IndexMap<&str, Vec<usize>> = IndexMap::default();
        for (index, item) in items.iter().enumerate() {
            if let Encoded::Jump(Jump { name, label, .. }) = item {
                if name == "jmp" {
                    jumps.entry(label.as_str()).or_default().push(index);
                }
            }
        }
        let mut retargeted: Vec<(usize, usize)> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let Encoded::Jump(Jump { name, label, long: true }) = item else { continue };
            if name == "jmp" {
                continue;
            }
            let near = jumps
                .get(label.as_str())
                .into_iter()
                .flatten()
                .find(|&&jump| short_reaches(starts[jump] - (starts[index] + SHORT_JUMP)));
            if let Some(&jump) = near {
                retargeted.push((index, jump));
            }
        }
        if retargeted.is_empty() {
            return Ok(labels);
        }
        // A label before each `jmp` taken, from the back so the indices below
        // stay.
        let mut taken: Vec<usize> = retargeted.iter().map(|(_, jump)| *jump).collect();
        taken.sort_unstable();
        taken.dedup();
        let name = |jump: usize| {
            format!(
                "{}$t{jump}",
                match &items[jump] {
                    Encoded::Jump(one) => one.label.as_str(),
                    _ => unreachable!("a jump"),
                }
            )
        };
        let named: IndexMap<usize, String> = taken.iter().map(|&jump| (jump, name(jump))).collect();
        for (index, jump) in &retargeted {
            if let Encoded::Jump(item) = &mut items[*index] {
                item.label = named[jump].clone();
            }
        }
        for &jump in taken.iter().rev() {
            items.insert(jump, Encoded::Label(masm::Label { name: named[&jump].clone() }));
        }
        for item in items.iter_mut() {
            if let Encoded::Jump(item) = item {
                item.long = false;
            }
        }
        labels = _relaxed(items, model)?;
    }
}

/// An item's bytes in the code model: a near displacement is
/// `CodeModel::displacement_bytes` wide.
pub fn _length(
    item: &Encoded,
    model: CodeModel,
) -> usize {
    match item {
        Encoded::Label(_) | Encoded::Mark(..) | Encoded::Cfi(_) | Encoded::Seh(_) | Encoded::End(_) => 0,
        // Padding depends on where it lands: see `_size`.
        Encoded::Align(_) | Encoded::Prefer { .. } => 0,
        Encoded::Piece(Piece { code, .. }) => code.len(),
        Encoded::Jump(Jump { name, long, .. }) => {
            if !long {
                SHORT_JUMP as usize
            } else if name == "jmp" {
                1 + model.displacement_bytes()
            } else {
                2 + model.displacement_bytes()
            }
        }
        Encoded::Near(near) => near.cond.as_ref().map_or(1, |_| 2) + model.displacement_bytes(),
    }
}

/// An item's bytes where it lies at offset `at`: `_length`, and the padding of
/// an alignment (none where it takes more than the most it may skip).
pub fn _size(
    item: &Encoded,
    at: i64,
    model: CodeModel,
) -> usize {
    match item {
        Encoded::Align(masm::CodeAlign { log2, max_skip }) => {
            llrm_object::assembler::align_padding(at, *log2, *max_skip)
        }
        Encoded::Prefer { computed, .. } => llrm_object::assembler::align_padding(at, *computed, 0),
        other => _length(other, model),
    }
}

pub fn _jump(
    name: &str,
    target: i64,
    at: usize,
    long: bool,
    model: CodeModel,
) -> Result<select::Emitted, Unencodable> {
    let at_ip = select::At { ip: at as u64, bits: model.bits() };
    let made =
        if name == "jmp" { select::jump(target, at_ip, !long) } else { select::branch(name, target, at_ip, !long) };
    match made {
        Some(made) if made.code.len() == _length(&Encoded::Jump(Jump { long, ..Jump::new(name, "") }), model) => {
            Ok(made)
        }
        _ => Err(Unencodable(format!("{name} from {at:#x} to {target:#x}"))),
    }
}

/// The sections, symbols and relocations of `segments`, in `symbols`, the
/// offsets of what they define, and `externs`, the kinds of what they declare.
pub fn object_of(
    module: &masm::Module,
    source: &str,
    segments: Vec<Segment>,
    symbols: &IndexMap<String, (usize, usize)>,
    sizes: &IndexMap<String, usize>,
    externs: &IndexMap<String, String>,
) -> Result<Described, Error> {
    let model = module.object.code_model();
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
    // masm.text's order, data externals first; LINK searches libraries in
    // EXTDEF order.
    let mut declared: Vec<&String> = externs.keys().collect();
    declared.sort_by_key(|name| externs[*name] != "byte");
    let order: Vec<&String> =
        declared.into_iter().filter(|name| used.contains(*name) || module.requests.contains(*name)).collect();
    // Every segment but the code and the debug sections is addressed in DGROUP,
    // where there is one.
    let grouped = |segment: &Segment| {
        model.segmented() && segment.near && matches!(
            segment.role,
            Role::Data | Role::Bss | Role::ROData | Role::Stack
        )
    };
    let members: Vec<usize> =
        segments.iter().enumerate().filter(|(_, segment)| grouped(segment)).map(|(index, _)| index).collect();
    let omf_groups = if members.is_empty() { Vec::new() } else { vec![OmfGroup { name: "DGROUP".into(), members }] };

    let publics: HashSet<&String> = module.publics.iter().collect();
    let mut table: Vec<Symbol> = symbols
        .iter()
        .map(|(name, &(section, offset))| Symbol {
            name: name.clone(),
            binding: if publics.contains(name) {
                Binding::Public
            } else if module.weak.contains(name) {
                Binding::Weak
            } else {
                Binding::Local
            },
            definition: Definition::Defined { section, offset },
            size: sizes.get(name).copied().unwrap_or(0),
            group: None,
            visibility: module.visibility.get(name).copied().unwrap_or_default(),
        })
        .collect();
    // An alias is the address of what it names, under its own name.
    for alias in &module.aliases {
        let Some(&(section, offset)) = symbols.get(&alias.target) else {
            return Err(Unencodable(format!(
                "alias {} names {}, which this object does not define",
                alias.name, alias.target
            ))
            .into());
        };
        table.push(Symbol {
            name: alias.name.clone(),
            binding: alias.binding,
            definition: Definition::Defined { section, offset },
            size: sizes.get(&alias.target).copied().unwrap_or(0),
            group: None,
            visibility: module.visibility.get(&alias.name).copied().unwrap_or_default(),
        });
    }
    // Data an object does not define is addressed in the group as its own data
    // is.
    table.extend(order.iter().map(|name| Symbol {
        name: (*name).clone(),
        binding: Binding::Public,
        definition: Definition::Undefined,
        size: 0,
        group: (externs[*name] == "byte").then_some(0),
        visibility: Default::default(),
    }));
    let index: IndexMap<String, usize> =
        table.iter().enumerate().map(|(at, symbol)| (symbol.name.clone(), at)).collect();

    let debug = module
        .debug
        .as_ref()
        .map(|debug| super::debuginfo::laid_out(debug, module, source, &segments, symbols, sizes, &index))
        .transpose()
        .map_err(Unencodable)?;
    let frames = if model == CodeModel::Segmented16 {
        Vec::new()
    } else {
        declared_frames(model.bits(), &segments, symbols, sizes)
    };
    let mut sections = Vec::new();
    for segment in segments {
        let Segment { name, role, near, mut image, spans, fixups, align, .. } = segment;
        let mut relocs = Vec::new();
        for one in &fixups {
            let target = _target(&one.name);
            let target = if target == "DGROUP" { Target::OmfGroup(0) } else { Target::Symbol(index[target]) };
            let addend = if one.kind == BASE { 0 } else { field(&image, one.at, one.kind) };
            relocs.push(Reloc::new(one.at, one.kind, target, addend));
        }
        for one in &relocs {
            if one.kind != BASE.kind {
                pack_field(
                    &mut image,
                    one.at,
                    Fix { kind: one.kind, pc_rel: one.pc_rel, pc_bias: 0, spec: one.spec },
                    0,
                )?;
            }
        }
        sections.push(Section { name, role, near, align, image, spans, relocs });
    }
    // The module is named for the source the debug info names: CodeView finds
    // the lines' file by the module's name (THEADR), not the `.ll` a compiler
    // was handed.
    let name =
        debug.as_ref().and_then(|info| info.files.first()).map_or_else(|| source.to_owned(), |file| file.name.clone());
    Ok(Described {
        debug,
        object: Object {
            name,
            bits: match model {
                CodeModel::Flat64 => 64u8,
                CodeModel::Flat32 => 32u8,
                CodeModel::Segmented16 => 16u8,
            },
            sections,
            symbols: table,
            omf_groups,
            omf_comdat: Vec::new(),
            comdats: Vec::new(),
            frames,
        },
    })
}

/// The frames the procedures declared (`Item::Cfi`): one row where the frame
/// address or a save changes, as the state after the directives at that offset.
fn declared_frames(
    bits: u32,
    segments: &[Segment],
    symbols: &IndexMap<String, (usize, usize)>,
    sizes: &IndexMap<String, usize>,
) -> Vec<llrm_object::Frame> {
    use llrm_debug::FrameRow;
    use llrm_object::assembler::Cfi;
    let name = |number: u16| llrm_x86::mc::unwind::register_label(bits, number);
    let mut frames = Vec::new();
    for (section, segment) in segments.iter().enumerate() {
        for (symbol, &(home, start)) in symbols {
            let Some(&length) = sizes.get(symbol).filter(|_| home == section) else { continue };
            let range = start..start + length;
            if within(&segment.declared, |at| *at, &range).is_empty() {
                continue;
            }
            let entry = llrm_x86::mc::unwind::entry_row(bits);
            let mut state = entry.clone();
            let mut rows: Vec<FrameRow> = Vec::new();
            let mut ops = Vec::new();
            for &(cfi, at) in within(&segment.cfi, |one| one.1, &range) {
                ops.push(cfi);
                match cfi {
                    Cfi::DefCfa { register, offset } => {
                        state.cfa_register = name(register);
                        state.cfa_offset = offset;
                    }
                    Cfi::DefCfaRegister { register } => state.cfa_register = name(register),
                    Cfi::DefCfaOffset { offset } => state.cfa_offset = offset,
                    Cfi::Offset { register, offset } => state.saved.push((name(register), offset)),
                    Cfi::Restore { register } => state.saved.retain(|(one, _)| *one != name(register)),
                }
                state.offset = at - start;
                if rows.last().is_some_and(|row| row.offset == state.offset) {
                    rows.pop();
                }
                rows.push(state.clone());
            }
            // A row is a change: one that repeats the one before it, or ends at
            // the function's end, says nothing.
            let mut kept: Vec<FrameRow> = Vec::new();
            for row in rows {
                let before = kept.last().unwrap_or(&entry);
                let same = (&row.cfa_register, row.cfa_offset, &row.saved)
                    == (&before.cfa_register, before.cfa_offset, &before.saved);
                if !same && row.offset < length {
                    kept.push(row);
                }
            }
            let seh = within(&segment.seh, |one| one.1, &range).iter().map(|&(seh, at)| (at - start, seh)).collect();
            frames.push(llrm_object::Frame { section, offset: start, length, rows: kept, cfi: ops, seh });
        }
    }
    frames
}

#[cfg(test)]
thread_local! {
    /// Offsets `within` has read: the work `declared_frames` does, counted.
    static PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The items whose offset lies in `range`. Items are pushed at the image's
/// length, so they are sorted by offset: a search per function, not a scan.
fn within<'a, T>(
    items: &'a [T],
    at: impl Fn(&T) -> usize,
    range: &std::ops::Range<usize>,
) -> &'a [T] {
    let at = |item: &T| {
        #[cfg(test)]
        PROBES.with(|probes| probes.set(probes.get() + 1));
        at(item)
    };
    let from = items.partition_point(|item| at(item) < range.start);
    let to = items.partition_point(|item| at(item) < range.end);
    &items[from..to]
}

pub fn _target(name: &str) -> &str {
    name.strip_prefix("seg ").unwrap_or(name)
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_objbuild.py`'s C-path tests, and both emitters
    //! against Python on hand-built modules.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::*;
    use crate::model::ir::Addr;
    use crate::model::lir;

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
    }

    fn targeted(
        op: Operation,
        name: &str,
        target: i64,
    ) -> Semantics {
        Semantics { target: Some(target), ..semantics(op, name, vec![], vec![]) }
    }

    fn insn(
        at: i64,
        what: Semantics,
    ) -> Arc<lir::Insn> {
        Arc::new(lir::Insn::new(at, Some((at, 1)), Some(what), vec![], vec![]))
    }

    fn block(
        at: i64,
        insns: Vec<Arc<lir::Insn>>,
        succ: Vec<i64>,
    ) -> lir::LirBlock {
        lir::LirBlock { succ: succ.into(), ..lir::LirBlock::new(at, insns) }
    }

    fn body(
        name: &str,
        blocks: Vec<lir::Block>,
    ) -> lir::LirBody {
        lir::LirBody::new(name, 1, blocks, IndexMap::default(), IndexMap::default())
    }

    fn procedure(
        name: &str,
        far: bool,
        body: lir::LirBody,
        reserve: i64,
        callees: Vec<(i64, masm::Callee)>,
    ) -> masm::Procedure {
        masm::Procedure {
            name: name.into(),
            public: true,
            far,
            body,
            reserve,
            callees: callees.into_iter().collect(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            probe_stack: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        }
    }

    fn reg(register: RegId) -> Loc {
        Loc::Reg(ir::Reg { register, width: 2 })
    }

    fn imm(
        value: i64,
        address: Option<Addr>,
    ) -> Loc {
        Loc::Imm(ir::Imm { value, width: 2, address })
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|one| (*one).to_owned()).collect()
    }

    fn label(name: &str) -> masm::Datum {
        masm::Datum::Label(masm::Label { name: name.into() })
    }

    fn pointer(
        name: &str,
        offset: i64,
        far: bool,
    ) -> masm::Datum {
        masm::Datum::Pointer(masm::Pointer { name: name.into(), offset, far, bytes: if far { 4 } else { 2 } })
    }

    fn fill(
        size: i64,
        byte: Option<u8>,
    ) -> masm::Datum {
        masm::Datum::Fill(masm::Fill { size, byte })
    }

    /// Alignment padding in an uninitialised segment was stored: jpeg_fixed's `_BSS` (15850 bytes) carried two
    /// one-byte zero data records and jwlink stored the whole segment up to them, 16380 data bytes against bcc's 474.
    #[test]
    fn alignment_padding_in_a_bss_segment_is_not_stored() {
        let mut segment = Segment::new("_BSS", Role::Bss, true);
        let mut symbols = IndexMap::default();
        let items = [fill(3, None), masm::Datum::Align(masm::Align { to: 4 }), fill(10, None)];
        _data(&mut segment, 0, &items, &mut symbols, CodeModel::Segmented16);
        assert_eq!(segment.image.len(), 14);
        assert!(segment.spans.is_empty(), "{:?}", segment.spans);
    }

    /// A data segment keeps its padding: bytes between stored data are part of the image.
    #[test]
    fn alignment_padding_in_a_data_segment_is_stored() {
        let mut segment = Segment::new("_DATA", Role::Data, true);
        let mut symbols = IndexMap::default();
        let items = [masm::Datum::Bytes(vec![1, 2, 3]), masm::Datum::Align(masm::Align { to: 4 }), masm::Datum::Bytes(vec![9])];
        _data(&mut segment, 0, &items, &mut symbols, CodeModel::Segmented16);
        assert_eq!(segment.spans, vec![[0, 5]]);
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
    }

    /// A segment word is two bytes LINK fills with the selector. `_data`
    /// dropped it, so a far pointer to near data lost its selector and
    /// every datum after it moved two bytes down.
    /// x86's segment kinds left the object crate's table, and a far pointer's
    /// width read 0: every 16-bit object with one panicked.
    #[test]
    fn a_far_pointer_is_four_bytes_and_a_segment_base_two() {
        assert_eq!((super::width(super::POINTER), super::width(super::BASE)), (4, 2));
    }

    #[test]
    fn test_a_segment_word_is_a_selector_fixup() {
        let mut segment = Segment::new("_DATA", Role::Data, true);
        let mut symbols = IndexMap::default();
        let items = [pointer("_x", 0, false), masm::Datum::SegmentWord("DGROUP".into()), label("_after")];
        _data(&mut segment, 0, &items, &mut symbols, CodeModel::Segmented16);
        assert_eq!(segment.image.len(), 4);
        assert_eq!(symbols["_after"], (0, 4));
        assert_eq!(
            (segment.fixups[1].at, segment.fixups[1].kind, segment.fixups[1].name.as_str()),
            (2, BASE, "DGROUP")
        );
    }

    /// A position-independent jump table's entry is `name - table` in four
    /// bytes: the slot is a pc-relative fixup to the code label, and its field
    /// carries `slot - table`, so the linker's `S + A - P` is `name -
    /// table`. A pointer-wide absolute entry (a PIE refuses) was all there was.
    #[test]
    fn test_a_table_entry_is_a_pc_relative_difference_from_the_table() {
        let mut segment = Segment::new(".rodata", Role::Data, true);
        let mut symbols = IndexMap::default();
        let entry = |name: &str| masm::Datum::Difference(masm::Difference { name: name.into(), base: "$J1".into() });
        let items = [
            masm::Datum::Bytes(vec![9, 9]),
            masm::Datum::Align(masm::Align { to: 4 }),
            label("$J1"),
            entry("L0"),
            entry("L1"),
        ];
        _data(&mut segment, 0, &items, &mut symbols, CodeModel::Flat64);
        assert_eq!(symbols["$J1"], (0, 4));
        assert_eq!(segment.image.len(), 12);
        let fixups: Vec<_> = segment.fixups.iter().map(|f| (f.at, f.name.as_str())).collect();
        assert_eq!(fixups, [(4, "L0"), (8, "L1")]);
        assert_eq!(&segment.image[4..12], &[0, 0, 0, 0, 4, 0, 0, 0]);
    }

    /// A table jump through a register names its table by the jump's
    /// `table_id` (no memory cell to take it from) and gets entries that are
    /// differences from the table: `jump_tables` found none and the symbol was
    /// a `KeyError` at print. A jump through a cell keeps pointer-wide entries.
    #[test]
    fn test_a_register_table_jump_gets_a_table_of_differences() {
        let held = Loc::Held(llrm_lir::Held { value: 1, width: 8, offset: 0 });
        let jump = Semantics {
            indirect: true,
            table: vec![1, 2],
            table_id: Some(7),
            ..semantics(Operation::Jump, "jmp", vec![], vec![held])
        };
        let procedure = procedure(
            "_f",
            false,
            body(
                "f",
                vec![
                    block(0, vec![insn(0, jump)], vec![1, 2]).into(),
                    block(1, vec![], vec![]).into(),
                    block(2, vec![], vec![]).into(),
                ],
            ),
            0,
            vec![],
        );
        let module = llrm_mir::parse::module("target datalayout = \"e-p:64:64\"\n").unwrap();
        let mut names = IndexMap::default();
        let data = masm::jump_tables(&[procedure], &mut names, &module);
        assert_eq!(names[&(Space::Segment, 7)], "$J7");
        let entries: Vec<_> = data
            .iter()
            .filter_map(|datum| match datum {
                masm::Datum::Difference(masm::Difference { base, .. }) => Some(base.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(entries, ["$J7", "$J7"]);
    }

    /// Fresh QB D_SURF retained 83 jumps whose target label was physically
    /// next.
    ///
    /// SC_INIT alone printed ``jmp L21_2`` immediately before ``L21_2``. A
    /// frontend is allowed to present explicit CFG edges; final emission owns
    /// the physical block order and must not encode an unconditional edge
    /// that has become fall-through.
    #[test]
    fn test_fresh_emission_omits_an_explicit_jump_to_the_next_block() {
        let jump = insn(1, targeted(Operation::Jump, "jmp", 2));
        let anchor = insn(1, semantics(Operation::Nothing, "", vec![], vec![]));
        let returned = insn(2, semantics(Operation::Return, "ret", vec![], vec![]));
        let body =
            body("next", vec![block(1, vec![jump, anchor], vec![2]).into(), block(2, vec![returned], vec![]).into()]);
        let procedure = procedure("_next", false, body, 0, vec![]);

        let lines: Vec<String> = masm::_procedure(&procedure, &IndexMap::default(), 0)
            .unwrap()
            .iter()
            .map(|one| one.trim().to_owned())
            .collect();
        assert_eq!(lines, ["_next proc near", "L0_1:", "L0_2:", "ret", "_next endp"]);
    }

    /// The pass measured each item after a jump in it had grown, against labels
    /// from before: a backward branch whose target also moved looked a byte out
    /// of reach. 18 of 38 qcport objects came out longer than jwasm's.
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
        let labels = _relaxed(&mut items, CodeModel::Segmented16).unwrap();
        let long = |item: &Encoded| matches!(item, Encoded::Jump(Jump { long: true, .. }));
        assert_eq!((long(&items[0]), long(&items[3])), (true, false));
        assert_eq!(labels["far"], 3 + 126 + 2 + 200);
    }

    /// A sibling `jmp f` to a function of the same object was `jmp rel32` (5
    /// bytes) however near `f` lay: its displacement was filled in, so it
    /// resolved, but only `Jump` items were relaxed. MC starts a resolved
    /// `JMP_1` short and grows it where the target is out of reach; gcc's 2
    /// bytes against our 5 were 3 of the 61 bytes tail_calls -Os lost at m64.
    #[test]
    fn test_a_resolved_sibling_jump_takes_the_short_form_where_it_reaches() {
        let near = |tail, cond: Option<&str>| Near { name: "f".into(), tail, cond: cond.map(str::to_owned) };
        let layout = |item: Near, between: usize, defined: bool| {
            let mut items = vec![
                Encoded::Label(masm::Label { name: "g".into() }),
                Encoded::Near(item),
                Encoded::Piece(Piece::new(vec![0; between])),
            ];
            if defined {
                items.push(Encoded::Label(masm::Label { name: "f".into() }));
            }
            let labels = _relaxed(&mut items, CodeModel::Flat64).unwrap();
            let code: usize = items.iter().map(|item| _length(item, CodeModel::Flat64)).sum();
            (code - between, labels.get("f").copied())
        };
        // `jmp f` and `je f`: 2 bytes each in reach; 5 and 6 out of it.
        assert_eq!(layout(near(true, None), 10, true), (2, Some(2 + 10)));
        assert_eq!(layout(near(true, Some("je")), 10, true), (2, Some(2 + 10)));
        assert_eq!(layout(near(true, None), 200, true), (5, Some(5 + 200)));
        assert_eq!(layout(near(true, Some("je")), 200, true), (6, Some(6 + 200)));
        // A call has no short form, and a name nothing here defines is a
        // relocation: both stay at 5 and 6.
        assert_eq!(layout(near(false, None), 10, true).0, 5);
        assert_eq!(layout(near(true, None), 10, false).0, 5);
    }

    /// A `jcc rel16` (4 bytes) to a label a `jmp` to it is within short reach
    /// of is a short `jcc` to that `jmp` (2): tuned for size, 141 of
    /// QCport's 1333 long conditional jumps. One with no such `jmp`, or one
    /// out of reach, stays.
    #[test]
    fn test_a_long_conditional_jump_goes_through_a_jump_to_its_target_within_reach() {
        let layout = |between: usize, jump_to: &str| {
            let mut items = vec![
                Encoded::Jump(Jump::new("je", "far")),
                Encoded::Piece(Piece::new(vec![0; between])),
                Encoded::Jump(Jump::new("jmp", jump_to)),
                Encoded::Piece(Piece::new(vec![0; 300])),
                Encoded::Label(masm::Label { name: "far".into() }),
                Encoded::Label(masm::Label { name: "elsewhere".into() }),
            ];
            let labels = _trampolined(&mut items, CodeModel::Segmented16).unwrap();
            (items.iter().map(|item| _length(item, CodeModel::Segmented16)).sum::<usize>(), labels["far"])
        };
        // 2 for the `je` short, the piece, a 3-byte `jmp`, and the 300 bytes: 2
        // + 20 + 3 + 300.
        assert_eq!(layout(20, "far"), (2 + 20 + 3 + 300, 2 + 20 + 3 + 300));
        // The `jmp` aims elsewhere: the `je` is 4 long.
        assert_eq!(layout(20, "elsewhere").0, 4 + 20 + 3 + 300);
        // The `jmp` is past a short jump's reach.
        assert_eq!(layout(200, "far").0, 4 + 200 + 3 + 300);
    }

    /// The module of
    /// `test_externals_are_declared_in_the_order_jwasm_declares_them`,
    /// whose jwasm half is deferred: text and object bytes as Python writes
    /// them.
    #[test]
    fn test_externals_module_matches_python() {
        let load = semantics(
            Operation::Move,
            "mov",
            vec![reg(RegId::AX)],
            vec![Loc::Mem(ir::Mem::new(Some(Addr { index: 7, ..Addr::new(Space::External, 0) }), 2))],
        );
        let call = semantics(Operation::Call, "call", vec![], vec![]);
        let leave = semantics(Operation::Return, "retf", vec![], vec![]);
        let insns = vec![insn(1, load), insn(2, call), insn(3, leave)];
        let built = masm::Module {
            debug: None,
            registers: crate::backend::registerinfo::test_regs(),
            code: "GET_TEXT".into(),
            names: IndexMap::from_iter([((Space::External, 7), "_d".to_owned())]),
            externs: vec![("_f".into(), "far".into()), ("_d".into(), "byte".into())],
            publics: strings(&["_get"]),
            visibility: Default::default(),
            preemptible: Default::default(),
            aliases: Vec::new(),
            data: vec![("_DATA".into(), vec![])],
            procedures: vec![procedure(
                "_get",
                true,
                body("get", vec![lir::LirBlock::new(1, insns).into()]),
                0,
                vec![(2, masm::Callee::new("_f", true))],
            )],
            private: BTreeSet::new(),
            far_bss: BTreeSet::new(),
            weak: BTreeSet::new(),
            code_segments: Default::default(),
            code_data: Default::default(),
            comdats: Default::default(),
            padding: Default::default(),
            object: llrm_target::Target::object(&llrm_x86_m16::M16),
            requests: BTreeSet::new(),
            stack: 0,
        };
        assert_eq!(
            masm::text(&built).unwrap(),
            ".model medium\n.386\n\npublic _get\n.data\nextern _d:byte\nextern _f:far\n.code GET_TEXT\n_get proc far\n\
             L0_1:\n    mov ax, word ptr _d\n    call far ptr _f\n    retf\n_get endp\nend\n"
        );
        assert_eq!(
            written(&built, "get.c", llrm_target::Target::objects(&llrm_x86_m32::M32)).unwrap(),
            hex("800700056765742e63a39622000004434f4445084745545f544558540444415441055f44415441064447524f55502a98\
                 07004809000302010a9807004800000504010f9a040006ff025b8c0900025f6400025f6600df900b000001045f676574\
                 000000c1a00d00010000a100009a00000000cb4c9c0a00c401160101cc045602558a02000074")
        );
    }

    /// One group of three procedures: `_a` calls `_b` across a `_pad` of `pad` bytes.
    fn call_across(pad: usize) -> Result<(Segment, IndexMap<String, (usize, usize)>), Error> {
        let procedures: Vec<masm::Procedure> =
            ["_a", "_pad", "_b"].iter().map(|name| procedure(name, true, body(name, vec![]), 0, vec![])).collect();
        let module = masm::Module {
            debug: None,
            registers: crate::backend::registerinfo::test_regs(),
            code: "M_TEXT".into(),
            names: IndexMap::default(),
            externs: vec![],
            publics: strings(&["_a", "_pad", "_b"]),
            visibility: Default::default(),
            preemptible: Default::default(),
            aliases: Vec::new(),
            data: vec![],
            procedures,
            private: BTreeSet::new(),
            far_bss: BTreeSet::new(),
            weak: BTreeSet::new(),
            code_segments: Default::default(),
            code_data: Default::default(),
            comdats: Default::default(),
            padding: Default::default(),
            object: llrm_target::Target::object(&llrm_x86_m16::M16),
            requests: BTreeSet::new(),
            stack: 0,
        };
        let returned = || masm::Item::Semantics(std::borrow::Cow::Owned(semantics(Operation::Return, "ret", vec![], vec![])));
        let mut segment = Segment::new("M_TEXT", Role::Text, true);
        let (mut symbols, mut sizes) = (IndexMap::default(), IndexMap::default());
        _code_by(&mut segment, 0, &module, &[0, 1, 2], &mut symbols, &mut sizes, false, |procedure, _| {
            Ok(match procedure.name.as_str() {
                "_a" => vec![masm::Item::Callee(masm::Callee::new("_b", true)), returned()],
                "_pad" => (0..pad).map(|_| returned()).collect(),
                _ => vec![returned()],
            })
        })?;
        Ok((segment, symbols))
    }

    /// A far call to a procedure of the same group is `push cs; call near`; deedlines'
    /// DEEDSAKS.BAS at -O1 had `_SUCKING` 33288 bytes past its caller and failed to
    /// build ("a near call to SUCKING 33288 bytes away"): a rel16 wraps modulo 64K in
    /// real mode, so the call is encodable, and its displacement must land on the callee.
    #[test]
    fn a_near_call_more_than_32k_ahead_wraps_to_its_callee() {
        let (segment, symbols) = call_across(33_000).expect("a call 33 KB ahead is encodable");
        let image = &segment.image;
        let at = symbols["_a"].1;
        assert_eq!(&image[at..at + 2], [0x0E, 0xE8], "push cs; call near");
        let rel = i16::from_le_bytes([image[at + 2], image[at + 3]]);
        let target = (at + 4).wrapping_add_signed(isize::from(rel)) & 0xFFFF;
        assert_eq!(target, symbols["_b"].1, "the displacement {rel} must reach _b");
    }

    /// The limit is the segment: 65_534 bytes ahead wraps to -2 and lands on the
    /// callee, 65_536 is back at the call and cannot. (The distance is the pad
    /// plus the caller's `ret`.)
    #[test]
    fn a_near_call_reaches_65534_bytes_and_refuses_65536() {
        let (segment, symbols) = call_across(65_533).expect("65534 bytes ahead is encodable");
        let at = symbols["_a"].1;
        let rel = i16::from_le_bytes([segment.image[at + 2], segment.image[at + 3]]);
        assert_eq!((at + 4).wrapping_add_signed(isize::from(rel)) & 0xFFFF, symbols["_b"].1 & 0xFFFF);
        assert!(call_across(65_535).is_err());
    }

    /// Past the 64K segment no displacement reaches the callee.
    #[test]
    fn a_near_call_a_segment_or_more_ahead_is_still_refused() {
        assert!(call_across(65_600).is_err());
    }

    /// `object_of` looked each symbol up in the module's public list, a
    /// `Vec`: 4x the symbols cost about 16x the work (2400 tiny functions
    /// spent 27% of the object write there). Counted in work (instructions, or
    /// thread CPU time), not wall clock: at load 17 a preempted 0.6 ms run
    /// read 5.5 ms and failed a linear table.
    #[test]
    fn test_the_symbol_table_is_linear_in_the_symbols() {
        let work = |count: usize| -> u64 {
            let names: Vec<String> = (0..count).map(|at| format!("f{at}")).collect();
            let module = masm::Module {
                registers: crate::backend::registerinfo::test_regs(),
                code: "M_TEXT".into(),
                names: IndexMap::default(),
                externs: vec![],
                publics: names.clone(),
                visibility: Default::default(),
                preemptible: Default::default(),
                aliases: vec![],
                data: vec![],
                procedures: vec![],
                private: BTreeSet::new(),
                far_bss: BTreeSet::new(),
                weak: BTreeSet::new(),
                code_segments: Default::default(),
                code_data: Default::default(),
                comdats: Default::default(),
                padding: Default::default(),
                object: llrm_target::Target::object(&llrm_x86_m16::M16),
                requests: BTreeSet::new(),
                stack: 0,
                debug: None,
            };
            let symbols: IndexMap<String, (usize, usize)> = names.into_iter().map(|name| (name, (0, 0))).collect();
            (0..3)
                .map(|_| {
                    llrm_support::debug::work_of(|| {
                        object_of(&module, "t.c", Vec::new(), &symbols, &IndexMap::default(), &IndexMap::default())
                            .unwrap();
                    })
                    .1
                })
                .min()
                .unwrap()
        };
        let (small, large) = (work(8000), work(32000));
        assert!(large < small * 8, "4x the symbols took {large} work against {small}");
    }

    /// A second name and a hidden export reach the object's symbol table: the
    /// writers dropped an alias's symbol (another unit naming it failed to
    /// link) and wrote every export with default visibility.
    #[test]
    fn an_alias_is_a_symbol_at_its_targets_address_and_a_hidden_export_is_hidden() {
        let leave = semantics(Operation::Return, "retf", vec![], vec![]);
        let module = masm::Module {
            debug: None,
            registers: crate::backend::registerinfo::test_regs(),
            code: "GET_TEXT".into(),
            names: IndexMap::default(),
            externs: vec![],
            publics: strings(&["_get"]),
            visibility: std::collections::BTreeMap::from([("_get".to_owned(), llrm_object::Visibility::Hidden)]),
            preemptible: Default::default(),
            aliases: vec![masm::Alias { name: "_other".into(), target: "_get".into(), binding: Binding::Weak }],
            data: vec![("_DATA".into(), vec![])],
            procedures: vec![procedure(
                "_get",
                true,
                body("get", vec![lir::LirBlock::new(1, vec![insn(1, leave)]).into()]),
                0,
                vec![],
            )],
            private: BTreeSet::new(),
            far_bss: BTreeSet::new(),
            weak: BTreeSet::new(),
            code_segments: Default::default(),
            code_data: Default::default(),
            comdats: Default::default(),
            padding: Default::default(),
            object: llrm_target::Target::object(&llrm_x86_m16::M16),
            requests: BTreeSet::new(),
            stack: 0,
        };
        let described =
            built(&module, "get.c", CodeLayout::OneSegment, llrm_target::Target::objects(&llrm_x86_m32::M32)).unwrap();
        let symbol = |name: &str| described.object.symbols.iter().find(|one| one.name == name).unwrap();
        let (get, other) = (symbol("_get"), symbol("_other"));
        assert_eq!((get.visibility, get.binding), (llrm_object::Visibility::Hidden, Binding::Public));
        assert_eq!((other.definition, other.binding), (get.definition, Binding::Weak));
    }

    /// `align 4` in a data segment pads to an offset, and an offset is an
    /// address only where the segment starts aligned: the segment was word
    /// aligned whatever its items asked, so a dword the data placed at 4 was
    /// at 2 mod 4 wherever the linker put the segment.
    #[test]
    fn test_a_segment_is_aligned_as_its_widest_item_asks() {
        let acbps = |to: Option<i64>| -> Vec<u8> {
            let mut items = vec![label("_a"), masm::Datum::Bytes(vec![1])];
            items.extend(to.map(|to| masm::Datum::Align(masm::Align { to })));
            items.push(masm::Datum::Bytes(vec![2]));
            let module = masm::Module {
                debug: None,
                registers: crate::backend::registerinfo::test_regs(),
                code: "M_TEXT".into(),
                names: IndexMap::default(),
                externs: vec![],
                publics: strings(&["_a"]),
                visibility: Default::default(),
                preemptible: Default::default(),
                aliases: Vec::new(),
                data: vec![("_DATA".into(), items)],
                procedures: vec![],
                private: BTreeSet::new(),
                far_bss: BTreeSet::new(),
                weak: BTreeSet::new(),
                code_segments: Default::default(),
                code_data: Default::default(),
                comdats: Default::default(),
                padding: Default::default(),
                object: llrm_target::Target::object(&llrm_x86_m16::M16),
                requests: BTreeSet::new(),
                stack: 0,
            };
            let records =
                omf::parse(&written(&module, "m.c", llrm_target::Target::objects(&llrm_x86_m32::M32)).unwrap())
                    .unwrap();
            // The code segment, then _DATA.
            records.iter().filter(|one| one.r#type & 0xFE == omf::SEGDEF).map(|one| one.body[0]).collect()
        };
        // word, dword and paragraph aligned, as SEGDEF's attribute byte says.
        let (acbp, dword, paragraph) = (0x48, 0xA8, 0x68);
        assert_eq!(acbps(None)[1], acbp);
        assert_eq!(acbps(Some(2))[1], acbp);
        assert_eq!(acbps(Some(4))[1], dword);
        assert_eq!(acbps(Some(16))[1], paragraph);
    }

    /// Every datum kind, a private and a grouped extra segment, a reserved
    /// frame, a saved SI, a branch, a backward jump, a near call within the
    /// module and inline code: text and object bytes as Python writes them.
    #[test]
    fn test_rich_module_matches_python() {
        let ax = reg(RegId::AX);
        let cell = Loc::Mem(ir::Mem::new(Some(Addr { index: 1, ..Addr::new(Space::Segment, 2) }), 2));
        let frame = Loc::Mem(ir::Mem { through: RegId::BP, ..ir::Mem::new(Some(Addr::new(Space::Frame, 6)), 2) });
        let first = vec![
            insn(1, semantics(Operation::Move, "mov", vec![ax.clone()], vec![frame])),
            insn(2, semantics(Operation::Compare, "cmp", vec![], vec![ax.clone(), imm(3, None)])),
            insn(3, targeted(Operation::Branch, "je", 20)),
        ];
        let table = Some(Addr { index: 1, ..Addr::new(Space::Segment, 4) });
        let second = vec![
            insn(10, semantics(Operation::Move, "mov", vec![cell], vec![ax.clone()])),
            insn(11, semantics(Operation::Move, "mov", vec![reg(RegId::BX)], vec![imm(0, table)])),
            insn(12, semantics(Operation::Call, "call", vec![], vec![])),
            insn(13, targeted(Operation::Jump, "jmp", 1)),
        ];
        let group = Some(Addr { index: 1, ..Addr::new(Space::Group, 0) });
        let third = vec![
            insn(20, semantics(Operation::Call, "call", vec![], vec![])),
            insn(21, semantics(Operation::Move, "mov", vec![reg(RegId::ES)], vec![imm(0, group)])),
            insn(22, semantics(Operation::Return, "", vec![], vec![])),
        ];
        let f = body(
            "f",
            vec![
                block(1, first, vec![10, 20]).into(),
                block(10, second, vec![1]).into(),
                block(20, third, vec![]).into(),
            ],
        );
        let helper = vec![
            insn(1, semantics(Operation::Move, "mov", vec![reg(RegId::SI)], vec![ax])),
            insn(2, semantics(Operation::Return, "ret", vec![], vec![])),
        ];
        let inline = vec![
            masm::InlinePart::Bytes(vec![0x90, 0x90]),
            masm::InlinePart::Fixup("offset".into(), "_table".into(), 2),
            masm::InlinePart::Fixup("segment".into(), "_far".into(), 0),
        ];
        let rich = masm::Module {
            debug: None,
            registers: crate::backend::registerinfo::test_regs(),
            code: "RICH_TEXT".into(),
            names: IndexMap::from_iter([
                ((Space::Segment, 1), "_table".to_owned()),
                ((Space::Group, 1), "DGROUP".to_owned()),
            ]),
            externs: vec![
                ("_ext".into(), "far".into()),
                ("_unused".into(), "near".into()),
                ("_b".into(), "byte".into()),
            ],
            publics: strings(&["_f", "_table"]),
            visibility: Default::default(),
            preemptible: Default::default(),
            aliases: Vec::new(),
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
                masm::Procedure {
                    public: false,
                    ..procedure("_h", false, body("h", vec![lir::LirBlock::new(1, helper).into()]), 0, vec![])
                },
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
            far_bss: BTreeSet::new(),
            weak: BTreeSet::new(),
            code_segments: Default::default(),
            code_data: Default::default(),
            comdats: Default::default(),
            padding: Default::default(),
            object: llrm_target::Target::object(&llrm_x86_m16::M16),
            requests: BTreeSet::new(),
            stack: 0,
        };
        assert_eq!(
            masm::text(&rich).unwrap(),
            ".model medium\n.386\n\npublic _f\npublic _table\n.data\nextern _b:byte\n_table label byte\n\
             db 001h,002h,003h\n    align 4\n    db 3 dup (7)\n    dw _table+2\n    dd _b\n.data?\nextern _b:byte\n\
             _zero label byte\n    db 5 dup (?)\nFAR_SEG segment para public 'FAR_DATA'\nextern _b:byte\n\
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
            written(&rich, "rich.c", llrm_target::Target::objects(&llrm_x86_m32::M32)).unwrap(),
            hex("80080006726963682e633b9649000004434f444509524943485f544558540444415441055f4441544103425353045f\
                 425353084641525f44415441074641525f534547044441544106534841524544064447524f555033980700482a0003\
                 0201e9980700a80d00050401a298070048050007060106980700681500090801d29807004802000b0a01019a08000c\
                 ff02ff03ff054b8c0500025f6200ac9009000001025f660500009a900d000102065f7461626c65000000f3a02e0001\
                 00005689c65ec35589e583ec048b460683f803740ba30200bb0400e8e4ffebed90900200000068000007c9cb359c18\
                 00c414140102c417140102c420140102c8225404c8255501eba0110002000001020300070707020000000000309c0a\
                 00c407140102cc0956014ca0190004000078797a78797a78797a78797a78797a78797a78797a56a006000500000100\
                 549c0500c4005404438a02000074")
        );
    }

    /// A 32-bit symbolic address carries a disp32; an OFFSET fixup relocated
    /// only its low word and left the high word of the addend in place.
    #[test]
    fn test_a_wide_symbolic_address_takes_an_offset32_fixup() {
        let regs = crate::backend::registerinfo::test_regs();
        let cell = ir::Mem {
            addr: Some(Addr { index: 3, ..Addr::new(Space::Segment, 1280) }),
            index: Some(ir::Held { value: 1, width: 4, offset: 0 }),
            index_through: RegId::ESI,
            scale: 2,
            ..ir::Mem::new(None, 2)
        };
        let what = semantics(
            Operation::Move,
            "mov",
            vec![Loc::Reg(ir::Reg { register: RegId::CX, width: 2 })],
            vec![Loc::Mem(cell)],
        );
        let names = IndexMap::from_iter([((Space::Segment, 3), "S%".to_owned())]);

        let piece = _encoded(regs, &what, &names, CodeModel::Segmented16).unwrap();

        let [fixup] = piece.fixups.as_slice() else { panic!("{:?}", piece.fixups) };
        assert_eq!(
            (fixup.kind, fixup.at + 4, field(&piece.code, fixup.at, fixup.kind)),
            (OFFSET32, piece.code.len(), 1280)
        );
    }

    /// `store ptr @a, ptr @sink` at -O2 -fno-pic encoded `mov [rip+disp], imm`
    /// with one fixup: the immediate's symbol took the displacement's field, so
    /// the store wrote 0 over the function `a` (sfm16 c64 -O2 segfaulted).
    #[test]
    fn test_a_store_of_a_symbol_to_a_symbol_has_a_fixup_for_each() {
        let regs = crate::backend::registerinfo::test_regs();
        let cell = ir::Mem { addr: Some(Addr { index: 3, ..Addr::new(Space::External, 0) }), ..ir::Mem::new(None, 8) };
        let value = ir::Imm { value: 0, width: 8, address: Some(Addr { index: 4, ..Addr::new(Space::External, 0) }) };
        let what = semantics(Operation::Move, "mov", vec![Loc::Mem(cell)], vec![Loc::Imm(value)]);
        let names =
            IndexMap::from_iter([((Space::External, 3), "sink".to_owned()), ((Space::External, 4), "a".to_owned())]);

        let piece = _encoded(regs, &what, &names, CodeModel::Flat64).unwrap();

        let mut named: Vec<_> = piece.fixups.iter().map(|one| (one.name.as_str(), one.at)).collect();
        named.sort_unstable();
        assert_eq!(named.len(), 2, "{named:?}");
        assert_eq!(named[0].0, "a");
        assert_eq!(named[1].0, "sink");
        assert_ne!(named[0].1, named[1].1, "both fixups on one field: {named:?}");
    }

    /// Code was never aligned: every function began where the last ended.
    /// `emitFunctionHeader` aligns to 16 (`.p2align 4`, OMF and COFF); ELF's
    /// `.prefalign` aligns to no more than the body fills, so a 3-byte
    /// function is aligned to 4; none for a function tuned for size.
    #[test]
    fn test_a_function_is_aligned_as_emitfunctionheader_does() {
        let small = |size: bool| {
            let mut module = masm::Module {
                debug: None,
                registers: crate::backend::registerinfo::test_regs(),
                code: "M_TEXT".into(),
                names: IndexMap::default(),
                externs: vec![],
                publics: vec![],
                visibility: Default::default(),
                preemptible: Default::default(),
                aliases: Vec::new(),
                data: vec![],
                procedures: (0..3)
                    .map(|n| masm::Procedure {
                        size,
                        ..procedure(&format!("_f{n}"), false, body("f", vec![]), 0, vec![])
                    })
                    .collect(),
                private: BTreeSet::new(),
                far_bss: BTreeSet::new(),
                weak: BTreeSet::new(),
                code_segments: Default::default(),
                code_data: Default::default(),
                comdats: Default::default(),
                padding: masm::Padding { function: 4, nop: 1 },
                object: llrm_target::Target::object(&llrm_x86_m32::M32),
                requests: BTreeSet::new(),
                stack: 0,
            };
            module.object = llrm_target::Target::object(&llrm_x86_m32::M32);
            module
        };
        fn body_of<'p>(
            _: &'p masm::Procedure,
            _: usize,
        ) -> Result<Vec<masm::Item<'p>>, String> {
            Ok(vec![masm::Item::Callee(masm::Callee {
                code: vec![masm::InlinePart::Bytes(vec![0x41, 0x42, 0xC3])],
                ..masm::Callee::new("", false)
            })])
        }
        let offsets_with = |pref_align: bool, size: bool| {
            let module = small(size);
            let mut segment = Segment::new("M_TEXT", Role::Text, true);
            let mut symbols = IndexMap::default();
            let mut sizes = IndexMap::default();
            _code_by(&mut segment, 0, &module, &[0, 1, 2], &mut symbols, &mut sizes, pref_align, body_of).unwrap();
            // `.size` is the body alone: the padding before a function is no
            // symbol's.
            assert_eq!(["_f0", "_f1", "_f2"].map(|name| sizes[name]), [3, 3, 3], "pref_align {pref_align}");
            (["_f0", "_f1", "_f2"].map(|name| symbols[name].1), segment.align)
        };
        let offsets = |format: Format, size: bool| {
            offsets_with(asm_info(llrm_target::Target::objects(&llrm_x86_m32::M32), format).pref_align, size)
        };
        // `.prefalign` was `Format::Elf` by name: a format that only says
        // `pref_align` got `.p2align`.
        assert_eq!(offsets_with(true, false), ([0, 4, 8], 4), "a format that says pref_align");
        assert_eq!(offsets(Format::Coff, false), ([0, 16, 32], 16), "p2align 4 each");
        assert_eq!(offsets(Format::Elf, false), ([0, 4, 8], 4), "prefalign: a 3-byte body fills 4");
        assert_eq!(offsets(Format::Coff, true), ([0, 3, 6], 1), "no alignment tuned for size");
    }

    /// `MachineBasicBlock::setAlignment`: a block that asks for alignment is
    /// preceded by the padding item, and one that does not is not. Before the
    /// field the listing could say nothing of it.
    #[test]
    fn test_a_block_that_asks_for_alignment_is_preceded_by_the_padding() {
        let returned = insn(2, semantics(Operation::Return, "ret", vec![], vec![]));
        let aligned = lir::LirBlock { align: 4, max_skip: 7, ..block(2, vec![returned], vec![]) };
        let anchor = insn(1, semantics(Operation::Nothing, "", vec![], vec![]));
        let body = body("a", vec![block(1, vec![anchor], vec![2]).into(), aligned.into()]);
        let held = procedure("_a", false, body, 0, vec![]);
        let items = masm::listing(&held, 0).unwrap();
        let at = items.iter().position(|one| matches!(one, masm::Item::Align(_))).expect("an align item");
        assert_eq!(items[at], masm::Item::Align(masm::CodeAlign { log2: 4, max_skip: 7 }));
        assert!(matches!(&items[at + 1], masm::Item::Label(label) if label.name == "L0_2"), "{items:?}");
        assert_eq!(items.iter().filter(|one| matches!(one, masm::Item::Align(_))).count(), 1);
    }

    /// An addend past its field's two bytes was wrapped into the field and
    /// written (`0x12345` as `0x2345`); it is refused, and the ones that fit
    /// (`-1`, `0xFFFF`) are written as before.
    #[test]
    fn an_addend_a_near_field_cannot_hold_is_refused() {
        let mut image = [0xAA; 4];
        assert!(pack_field(&mut image, 0, OFFSET, 0x12345).is_err());
        assert_eq!(image, [0xAA; 4]);
        pack_field(&mut image, 0, OFFSET, -1).unwrap();
        pack_field(&mut image, 2, OFFSET, 0xFFFF).unwrap();
        assert_eq!(image, [0xFF; 4]);
    }

    /// A procedure's `.seh_*` directives reached `Object.frames` only through
    /// the call frame information, so a Win64 prologue declared on its own was
    /// dropped and `.xdata` was read back from the bytes. They are the
    /// function's, at offsets from its start.
    #[test]
    fn declared_seh_directives_reach_the_frame_at_function_relative_offsets() {
        let mut segment = Segment::new("A_TEXT", Role::Text, true);
        segment.declared.push(8);
        segment.seh.push((llrm_object::Seh::PushReg(6), 9));
        segment.seh.push((llrm_object::Seh::StackAlloc(32), 13));
        let symbols: IndexMap<String, (usize, usize)> = [("f".to_owned(), (0, 8))].into_iter().collect();
        let sizes: IndexMap<String, usize> = [("f".to_owned(), 20)].into_iter().collect();
        let frames = declared_frames(64, &[segment], &symbols, &sizes);
        assert_eq!(
            frames.iter().map(|frame| (frame.offset, frame.seh.clone())).collect::<Vec<_>>(),
            vec![(8, vec![(1, llrm_object::Seh::PushReg(6)), (5, llrm_object::Seh::StackAlloc(32))])]
        );
    }

    /// `declared_frames` scanned every declared mark, CFI and SEH entry of a
    /// segment for each function: four times the functions cost sixteen
    /// times the work (object write 1.08/1.16 per doubling at 600/2400).
    /// Counted in offsets read, not time: a loaded machine cannot fail it.
    #[test]
    fn the_frames_of_a_segment_are_linear_in_its_functions() {
        let frames_of = |functions: usize| {
            let mut segment = Segment::new("A_TEXT", Role::Text, true);
            let mut symbols = IndexMap::default();
            let mut sizes = IndexMap::default();
            for at in 0..functions {
                let name = format!("f{at}");
                segment.declared.push(16 * at);
                segment.cfi.push((llrm_object::assembler::Cfi::DefCfaOffset { offset: 16 }, 16 * at + 1));
                symbols.insert(name.clone(), (0, 16 * at));
                sizes.insert(name, 16);
            }
            PROBES.with(|probes| probes.set(0));
            let frames = declared_frames(64, &[segment], &symbols, &sizes);
            assert_eq!(frames.len(), functions);
            PROBES.with(|probes| probes.get())
        };
        let (small, large) = (frames_of(4_000), frames_of(16_000));
        assert!(large < small * 8, "4x the functions read {large} offsets against {small}");
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    /// An ELF x86-64 object ran `live`, which encoded every procedure and
    /// cloned the module, to find no table dead: 13 M of jpeg_fixed's 27 M
    /// instructions in `object write`. Only OMF reads a module that way.
    #[test]
    fn test_only_an_omf_object_looks_for_unreached_data_and_only_two_formats_change_procedures() {
        let formats = [Format::Omf, Format::Coff, Format::MachO, Format::Elf];
        assert_eq!(formats.map(drops_unreached_data), [true, false, false, false]);
        assert_eq!(formats.map(changes_procedures), [false, true, true, false]);
    }
}
