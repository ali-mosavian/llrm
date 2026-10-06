//! A masm module as an object: its code and data laid out in sections, each name resolved to where
//! it is defined or declared, each reference to one a relocation. The result is a format-neutral
//! `llrm_object::Object`; a writer crate turns it into bytes. A reference to anything the module
//! defines is a relocation against its section with the addend in the code, as jwasm writes it;
//! anything else is an undefined symbol.

use std::collections::BTreeSet;
use std::fmt;

use llrm_object::{Arch, Binding, Definition, Kind, Object, OmfGroup, Reloc, Role, Section, Symbol, Target};

use crate::support::hash::IndexMap;

use crate::backend::masm;
use crate::objectfile::omf;
use crate::backend::select;
use crate::backend::target;
use crate::model::ir::{self, Loc, Operation, Semantics, Space};
use crate::support::pyrepr::{self, Repr};

/// How an address is relocated: a near offset, a segment's selector, a far pointer.
pub const OFFSET: Kind = Kind::Abs { width: 2 };
pub const BASE: Kind = Kind::SegmentBase;
pub const POINTER: Kind = Kind::FarPointer;
pub const OFFSET32: Kind = Kind::Abs { width: 4 };

/// A call's field: relative to its own end.
fn relative(width: usize) -> Kind {
    Kind::PcRel { width, from: width }
}

/// The role of a data segment `name` that the object's classes name, and that it is otherwise.
fn data_role(name: &str, far_bss: bool) -> Role {
    match name {
        "_DATA" => Role::Data,
        "_BSS" => Role::Bss,
        "CONST" => Role::ROData,
        _ if far_bss => Role::Bss,
        _ => Role::Data,
    }
}

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
    pub kind: Kind,
    pub name: String,
}

impl Fixup {
    pub fn new(at: usize, kind: Kind, name: impl Into<String>) -> Self {
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
    /// A mark, and the symbol a line's defines.
    Mark(masm::Mark, Option<String>),
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
    pub lines: Vec<(u32, usize)>,
    /// Each procedure body's bounds, in order.
    pub bodies: Vec<(masm::Mark, usize)>,
    /// The widest alignment a data item in it asks for.
    pub align: usize,
}

impl Segment {
    pub fn new(name: &str, role: Role, near: bool) -> Self {
        Self { name: name.to_owned(), role, near, image: Vec::new(), spans: Vec::new(), fixups: Vec::new(), lines: Vec::new(), bodies: Vec::new(), align: 1 }
    }

    /// `mark` is here.
    pub fn mark(&mut self, mark: masm::Mark) {
        match mark {
            masm::Mark::Line { line, .. } => self.line(line),
            bound => self.bodies.push((bound, self.image.len())),
        }
    }

    /// Code at the current offset is `line`'s: the last line named at an
    /// offset wins, as the one before it has no code.
    pub fn line(&mut self, line: u32) {
        let at = self.image.len();
        if self.lines.last().is_some_and(|&(_, last)| last == at) {
            self.lines.pop();
        }
        if self.lines.last().is_none_or(|&(last, _)| last != line) {
            self.lines.push((line, at));
        }
    }

    pub fn put(&mut self, code: &[u8], fixups: &[Fixup]) {
        let at = self.image.len();
        self.fixups.extend(
            fixups.iter().map(|one| Fixup { at: at + one.at, ..one.clone() }),
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
pub fn pack_into(buffer: &mut [u8], at: usize, value: i64) {
    buffer[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

/// `value` into the bytes of a relocated field at `at` that hold an offset.
fn pack_field(buffer: &mut [u8], at: usize, kind: Kind, value: i64) {
    if kind.width() == 4 && kind != POINTER {
        buffer[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
    } else {
        pack_into(buffer, at, value & 0xFFFF);
    }
}

/// What a relocated field holds, sign-extended from the bytes that hold an offset.
fn field(buffer: &[u8], at: usize, kind: Kind) -> i64 {
    if kind.width() == 4 && kind != POINTER {
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
    if !module.data.iter().any(|(_, items)| items.iter().any(|item| matches!(item, masm::Datum::Object(_)))) {
        return Ok(module.clone());
    }
    let mut reached: BTreeSet<String> = module.publics.iter().cloned().collect();
    for (number, procedure) in module.procedures.iter().enumerate() {
        reached.insert(procedure.name.clone());
        for item in masm::listing(procedure, number)? {
            for one in _items(&item, &module.names, number, module.object.bitness)? {
                match one {
                    Encoded::Piece(Piece { fixups, .. }) => {
                        reached.extend(fixups.iter().map(|fixup| _target(&fixup.name).to_owned()));
                    }
                    Encoded::Near(Near { name }) => {
                        reached.insert(name);
                    }
                    Encoded::Label(_) | Encoded::Jump(_) | Encoded::Mark(..) => {}
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
    /// linker's `option eliminate` drops each one nothing calls. A near call
    /// between them is a relative fixup to the other segment, which the linker
    /// merges.
    PerProcedure,
}

pub fn written(module: &masm::Module, source: &str) -> Result<Vec<u8>, Error> {
    written_as(module, source, CodeLayout::OneSegment)
}

pub fn written_as(module: &masm::Module, source: &str, layout: CodeLayout) -> Result<Vec<u8>, Error> {
    llrm_support::debug::timed("omf write", || llrm_omf::write::write(&built_inner(module, source, layout)?).map_err(Error::from))
}

/// `module` laid out as an object.
pub fn built(module: &masm::Module, source: &str, layout: CodeLayout) -> Result<Object, Error> {
    built_inner(module, source, layout)
}

fn built_inner(module: &masm::Module, source: &str, layout: CodeLayout) -> Result<Object, Error> {
    let module = &live(module)?;
    let groups: Vec<Vec<usize>> = match layout {
        CodeLayout::OneSegment => vec![(0..module.procedures.len()).collect()],
        CodeLayout::PerProcedure => (0..module.procedures.len()).map(|one| vec![one]).collect(),
    };
    // A flat target has no group: every segment is reached by its own offset.
    let mut segments: Vec<Segment> = groups.iter().map(|_| Segment::new(&module.code, Role::Text, true)).collect();
    let mut named: IndexMap<String, Segment> = IndexMap::from_iter([("_DATA".to_owned(), Segment::new("_DATA", Role::Data, true))]);
    for (name, _items) in &module.data {
        if !named.contains_key(name) {
            named.insert(name.clone(), Segment::new(name, data_role(name, module.far_bss.contains(name)), !module.selector_addressed(name)));
        }
    }
    segments.extend(named.into_values());
    if module.stack > 0 {
        let mut stack = Segment::new("STACK", Role::Stack, true);
        stack.image = vec![0; module.stack as usize];
        segments.push(stack);
    }
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
    if let Some(debug) = &module.debug {
        if groups.len() != 1 {
            return Err(Unencodable("-g with a code segment per procedure".into()).into());
        }
        let described = super::codeview::segments(debug, module, source, &segments[0], &symbols).map_err(Unencodable)?;
        segments.extend(described);
    }
    let externs: IndexMap<String, String> = module.externs.iter().cloned().collect();
    object_of(module, source, segments, &symbols, &externs)
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
                segment.put(&vec![0; loc.width()], &[Fixup::new(0, loc, name.clone())]);
                let at = segment.image.len() - loc.width();
                pack_into(&mut segment.image, at, offset & 0xFFFF);
            }
            masm::Datum::Align(masm::Align { to }) => {
                segment.align = segment.align.max(*to as usize);
                segment.put(&vec![0; (-(segment.image.len() as i64)).rem_euclid(*to) as usize], &[]);
            }
            masm::Datum::Bytes(item) => segment.put(item, &[]),
            masm::Datum::SegmentWord(name) => segment.put(&[0, 0], &[Fixup::new(0, BASE, name.clone())]),
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
    _code_by(segment, index, module, group, symbols, |procedure, number| masm::listing(procedure, number).map_err(|error| error.0))
}

/// [`_code`], each procedure's items as `listed` gives them.
pub fn _code_by(
    segment: &mut Segment,
    index: usize,
    module: &masm::Module,
    group: &[usize],
    symbols: &mut IndexMap<String, (usize, usize)>,
    listed: impl Fn(&masm::Procedure, usize) -> Result<Vec<masm::Item>, String>,
) -> Result<(), Error> {
    let mut items: Vec<Encoded> = Vec::new();
    for &number in group {
        let procedure = &module.procedures[number];
        items.push(Encoded::Label(masm::Label { name: procedure.name.clone() }));
        for item in listed(procedure, number).map_err(Unencodable)? {
            match _items(&item, &module.names, number, module.object.bitness) {
                Ok(encoded) => items.extend(encoded),
                Err(error) => return Err(Unencodable(format!("{}: {error}", procedure.name)).into()),
            }
        }
    }
    // Tuned for size, a long conditional jump may go through a `jmp` within reach.
    let size = !group.is_empty() && group.iter().all(|&number| module.procedures[number].size);
    let bits = module.object.bitness;
    let labels = if size { _trampolined(&mut items, bits)? } else { _relaxed(&mut items, bits)? };
    let mut at = 0;
    for item in &items {
        match item {
            Encoded::Label(masm::Label { name }) => {
                symbols.insert(name.clone(), (index, at));
            }
            Encoded::Mark(mark, name) => {
                segment.mark(*mark);
                if let Some(name) = name {
                    symbols.insert(name.clone(), (index, at));
                }
            }
            Encoded::Piece(Piece { code, fixups }) => segment.put(code, fixups),
            Encoded::Jump(Jump { name, label, long }) => segment.put(&_jump(name, labels[label], at, *long, bits)?.code, &[]),
            Encoded::Near(Near { name }) if labels.contains_key(name) => {
                let distance = labels[name] - (at as i64 + 1 + i64::from(bits) / 8);
                let far = || Unencodable(format!("a near call to {name} {distance} bytes away"));
                let displacement = if bits == 32 { i32::try_from(distance).map_err(|_| far())?.to_le_bytes().to_vec() } else { i16::try_from(distance).map_err(|_| far())?.to_le_bytes().to_vec() };
                segment.put(&[&[0xE8][..], &displacement].concat(), &[]);
            }
            Encoded::Near(Near { name }) => {
                segment.put(&vec![0; 1 + bits as usize / 8], &[Fixup::new(1, relative(bits as usize / 8), name.clone())]);
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
    bits: u32,
) -> Result<Vec<Encoded>, Unencodable> {
    Ok(match item {
        masm::Item::Label(label) => vec![Encoded::Label(label.clone())],
        masm::Item::Mark(mark @ masm::Mark::Line { index, .. }) => vec![Encoded::Mark(*mark, Some(masm::line_label(number, *index)))],
        masm::Item::Mark(mark) => vec![Encoded::Mark(*mark, None)],
        masm::Item::Callee(masm::Callee { code, .. }) if !code.is_empty() => {
            code.iter().map(|part| _part(part, bits)).collect::<Result<Vec<_>, _>>()?.into_iter().map(Encoded::Piece).collect()
        }
        masm::Item::Callee(masm::Callee { name, far: true, .. }) if bits == 32 => {
            return Err(Unencodable(format!("a far call to {name} in flat code")));
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
        masm::Item::Semantics(what) => vec![Encoded::Piece(_encoded(what, names, bits)?)],
    })
}

pub fn _part(part: &masm::InlinePart, bits: u32) -> Result<Piece, Unencodable> {
    match part {
        masm::InlinePart::Bytes(part) => Ok(Piece::new(part.clone())),
        masm::InlinePart::Fixup(kind, name, offset) if kind == "offset" && bits == 32 => {
            Ok(Piece { code: (*offset as u32).to_le_bytes().to_vec(), fixups: vec![Fixup::new(0, OFFSET32, name.clone())] })
        }
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

pub fn _encoded(what: &Semantics, names: &IndexMap<(Space, i64), String>, bits: u32) -> Result<Piece, Unencodable> {
    let relocated = what.sources.iter().any(|one| matches!(one, Loc::Imm(ir::Imm { address: Some(_), .. })));
    // A near address is the `bits`-bit offset.
    let near = if bits == 32 { OFFSET32 } else { OFFSET };
    let Some(made) = select::emit_in(bits, what, 0, None, false, relocated, None) else {
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
                (made.displacement_at, if wide { OFFSET32 } else { near }, addr.disp, addr)
            }
            Loc::Address(ir::Address { addr: Some(addr), .. }) if matches!(addr.space, Space::Segment | Space::External) => {
                (made.displacement_at, near, addr.disp, addr)
            }
            Loc::Imm(ir::Imm { address: Some(addr), .. }) if addr.space == Space::Group => {
                (made.immediate_at, BASE, 0, addr)
            }
            Loc::Imm(ir::Imm { address: Some(addr), value, .. }) => (made.immediate_at, near, addr.disp + value, addr),
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

/// A short jump's length: opcode and an 8-bit displacement.
pub const SHORT_JUMP: i64 = 2;

/// Whether a short jump reaches `displacement`, counted from its end.
pub fn short_reaches(displacement: i64) -> bool {
    (-128..=127).contains(&displacement)
}

/// Every label's offset, with each jump short unless its target is out of reach.
///
/// Short first and lengthened to a fixed point, as jwasm does: lengthening
/// only moves targets further away, so it ends, and at the smallest layout.
pub fn _relaxed(items: &mut [Encoded], bits: u32) -> Result<IndexMap<String, i64>, Unencodable> {
    loop {
        let (mut labels, mut at) = (IndexMap::default(), 0i64);
        for item in items.iter() {
            if let Encoded::Label(label) = item {
                labels.insert(label.name.clone(), at);
            }
            at += _length(item, bits) as i64;
        }
        let mut changed = false;
        at = 0;
        for item in items.iter_mut() {
            // Measured before the jump may grow: `labels` is this pass's layout.
            let length = _length(item, bits) as i64;
            if let Encoded::Jump(item) = item {
                if !item.long {
                    let Some(target) = labels.get(&item.label) else {
                        return Err(Unencodable(format!("a jump to {}, which is nowhere", item.label)));
                    };
                    if !short_reaches(target - (at + SHORT_JUMP)) {
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

/// Tuned for size, [`_relaxed`], then each conditional jump still long (a 386 `jcc rel16`, 4
/// bytes) aimed at a label some `jmp` to it lies within short reach of becomes a short jump to
/// that `jmp`, which carries on: 2 bytes saved, and the 3 clocks of a taken `jmp` more.
/// Repeated while the shorter layout brings more within reach; Watcom's `SetBranches`.
pub fn _trampolined(items: &mut Vec<Encoded>, bits: u32) -> Result<IndexMap<String, i64>, Unencodable> {
    let mut labels = _relaxed(items, bits)?;
    loop {
        let mut starts = Vec::with_capacity(items.len());
        let mut at = 0i64;
        for item in items.iter() {
            starts.push(at);
            at += _length(item, bits) as i64;
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
            let near = jumps.get(label.as_str()).into_iter().flatten().find(|&&jump| short_reaches(starts[jump] - (starts[index] + SHORT_JUMP)));
            if let Some(&jump) = near {
                retargeted.push((index, jump));
            }
        }
        if retargeted.is_empty() {
            return Ok(labels);
        }
        // A label before each `jmp` taken, from the back so the indices below stay.
        let mut taken: Vec<usize> = retargeted.iter().map(|(_, jump)| *jump).collect();
        taken.sort_unstable();
        taken.dedup();
        let name = |jump: usize| format!("{}$t{jump}", match &items[jump] { Encoded::Jump(one) => one.label.as_str(), _ => unreachable!("a jump") });
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
        labels = _relaxed(items, bits)?;
    }
}

/// An item's bytes in `bits`-bit mode: a near displacement is `bits` wide.
pub fn _length(item: &Encoded, bits: u32) -> usize {
    match item {
        Encoded::Label(_) | Encoded::Mark(..) => 0,
        Encoded::Piece(Piece { code, .. }) => code.len(),
        Encoded::Jump(Jump { name, long, .. }) => {
            if !long {
                SHORT_JUMP as usize
            } else if name == "jmp" {
                1 + bits as usize / 8
            } else {
                2 + bits as usize / 8
            }
        }
        Encoded::Near(_) => 1 + bits as usize / 8,
    }
}

pub fn _jump(name: &str, target: i64, at: usize, long: bool, bits: u32) -> Result<select::Emitted, Unencodable> {
    let at_ip = select::At { ip: at as u64, bits };
    let made = if name == "jmp" {
        select::jump(target, at_ip, !long)
    } else {
        select::branch(name, target, at_ip, !long)
    };
    match made {
        Some(made) if made.code.len() == _length(&Encoded::Jump(Jump { long, ..Jump::new(name, "") }), bits) => Ok(made),
        _ => Err(Unencodable(format!("{name} from {at:#x} to {target:#x}"))),
    }
}

/// The sections, symbols and relocations of `segments`, in `symbols`, the offsets of what they
/// define, and `externs`, the kinds of what they declare.
pub fn object_of(
    module: &masm::Module,
    source: &str,
    segments: Vec<Segment>,
    symbols: &IndexMap<String, (usize, usize)>,
    externs: &IndexMap<String, String>,
) -> Result<Object, Error> {
    let bits = module.object.bitness;
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
    let order: Vec<&String> = declared.into_iter().filter(|name| used.contains(*name) || module.requests.contains(*name)).collect();
    // Every segment but the code and the debug sections is addressed in DGROUP, where there is one.
    let grouped = |segment: &Segment| bits == 16 && segment.near && matches!(segment.role, Role::Data | Role::Bss | Role::ROData | Role::Stack);
    let members: Vec<usize> = segments.iter().enumerate().filter(|(_, segment)| grouped(segment)).map(|(index, _)| index).collect();
    let omf_groups = if members.is_empty() { Vec::new() } else { vec![OmfGroup { name: "DGROUP".into(), members }] };

    let mut table: Vec<Symbol> = symbols
        .iter()
        .map(|(name, &(section, offset))| Symbol { name: name.clone(), binding: if module.publics.contains(name) { Binding::Public } else { Binding::Local }, definition: Definition::Defined { section, offset }, group: None })
        .collect();
    // Data an object does not define is addressed in the group as its own data is.
    table.extend(order.iter().map(|name| Symbol { name: (*name).clone(), binding: Binding::Public, definition: Definition::Undefined, group: (externs[*name] == "byte").then_some(0) }));
    let index: IndexMap<String, usize> = table.iter().enumerate().map(|(at, symbol)| (symbol.name.clone(), at)).collect();

    let mut sections = Vec::new();
    for segment in segments {
        let Segment { name, role, near, mut image, spans, fixups, lines, align, .. } = segment;
        let mut relocs = Vec::new();
        for one in &fixups {
            let target = _target(&one.name);
            let target = if target == "DGROUP" { Target::OmfGroup(0) } else { Target::Symbol(index[target]) };
            let addend = if one.kind == BASE { 0 } else { field(&image, one.at, one.kind) };
            relocs.push(Reloc { at: one.at, kind: one.kind, target, addend });
        }
        for one in &relocs {
            if one.kind != BASE {
                pack_field(&mut image, one.at, one.kind, 0);
            }
        }
        let lines = if module.debug.is_some() { lines } else { Vec::new() };
        sections.push(Section { name, role, near, align, image, spans, relocs, lines });
    }
    Ok(Object {
        name: source.to_owned(),
        arch: if bits == 32 { Arch::I386 } else { Arch::I8086 },
        sections,
        symbols: table,
        omf_groups,
        debug: module.debug.as_ref().map(|_| llrm_object::DebugFormat::CodeView),
    })
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
        masm::Procedure { name: name.into(), public: true, far, body, reserve, callees: callees.into_iter().collect(), interrupt: None, size: false, entry: 0, stack_check: None, registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16) }
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

    /// A segment word is two bytes LINK fills with the selector. `_data`
    /// dropped it, so a far pointer to near data lost its selector and
    /// every datum after it moved two bytes down.
    #[test]
    fn test_a_segment_word_is_a_selector_fixup() {
        let mut segment = Segment::new("_DATA", Role::Data, true);
        let mut symbols = IndexMap::default();
        let items = [pointer("_x", 0, false), masm::Datum::SegmentWord("DGROUP".into()), label("_after")];
        _data(&mut segment, 0, &items, &mut symbols);
        assert_eq!(segment.image.len(), 4);
        assert_eq!(symbols["_after"], (0, 4));
        assert_eq!((segment.fixups[1].at, segment.fixups[1].kind, segment.fixups[1].name.as_str()), (2, BASE, "DGROUP"));
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
        let labels = _relaxed(&mut items, 16).unwrap();
        let long = |item: &Encoded| matches!(item, Encoded::Jump(Jump { long: true, .. }));
        assert_eq!((long(&items[0]), long(&items[3])), (true, false));
        assert_eq!(labels["far"], 3 + 126 + 2 + 200);
    }

    /// A `jcc rel16` (4 bytes) to a label a `jmp` to it is within short reach of is a short `jcc`
    /// to that `jmp` (2): tuned for size, 141 of QCport's 1333 long conditional jumps. One with no
    /// such `jmp`, or one out of reach, stays.
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
            let labels = _trampolined(&mut items, 16).unwrap();
            (items.iter().map(|item| _length(item, 16)).sum::<usize>(), labels["far"])
        };
        // 2 for the `je` short, the piece, a 3-byte `jmp`, and the 300 bytes: 2 + 20 + 3 + 300.
        assert_eq!(layout(20, "far"), (2 + 20 + 3 + 300, 2 + 20 + 3 + 300));
        // The `jmp` aims elsewhere: the `je` is 4 long.
        assert_eq!(layout(20, "elsewhere").0, 4 + 20 + 3 + 300);
        // The `jmp` is past a short jump's reach.
        assert_eq!(layout(200, "far").0, 4 + 200 + 3 + 300);
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
            far_bss: BTreeSet::new(),
            object: llrm_target::Target::object(&llrm_x86_m16::M16),
            requests: BTreeSet::new(),
            stack: 0,
            debug: None,
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
                code: "M_TEXT".into(),
                names: IndexMap::default(),
                externs: vec![],
                publics: strings(&["_a"]),
                data: vec![("_DATA".into(), items)],
                procedures: vec![],
                private: BTreeSet::new(),
                far_bss: BTreeSet::new(),
                object: llrm_target::Target::object(&llrm_x86_m16::M16),
                requests: BTreeSet::new(),
            stack: 0,
                debug: None,
            };
            let records = omf::parse(&written(&module, "m.c").unwrap()).unwrap();
            // The code segment, then _DATA.
            records.iter().filter(|one| one.r#type & 0xFE == omf::SEGDEF).map(|one| one.body[0]).collect()
        };
        // word, dword and paragraph aligned, as SEGDEF's attribute byte says.
        let (ACBP, DWORD, PARAGRAPH) = (0x48, 0xA8, 0x68);
        assert_eq!(acbps(None)[1], ACBP);
        assert_eq!(acbps(Some(2))[1], ACBP);
        assert_eq!(acbps(Some(4))[1], DWORD);
        assert_eq!(acbps(Some(16))[1], PARAGRAPH);
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
            far_bss: BTreeSet::new(),
            object: llrm_target::Target::object(&llrm_x86_m16::M16),
            requests: BTreeSet::new(),
            stack: 0,
            debug: None,
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
            written(&rich, "rich.c").unwrap(),
            hex("80080006726963682e633b9649000004434f444509524943485f544558540444415441055f4441544103425353045f\
                 425353084641525f44415441074641525f534547044441544106534841524544064447524f555033980700482a0003\
                 0201e9980700a80d00050401a298070048050007060106980700681500090801d29807004802000b0a01019a08000c\
                 ff02ff03ff054b8c0500025f6200ac9009000001025f660500009a900d000102065f7461626c65000000f3a02e0001\
                 0000568bf05ec3558bec83ec048b460683f803740ba30200bb0400e8e4ffebed90900200000068000007c9cb009c18\
                 00c414140102c417140102c420140102c8225404c8255501eba0110002000001020300070707020000000000309c0a\
                 00c407140102cc0956014ca0190004000078797a78797a78797a78797a78797a78797a78797a56a006000500000100\
                 549c0500c4005404438a02000074")
        );
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

        let piece = _encoded(&what, &names, 16).unwrap();

        let [fixup] = piece.fixups.as_slice() else { panic!("{:?}", piece.fixups) };
        assert_eq!((fixup.kind, fixup.at + 4, field(&piece.code, fixup.at, fixup.kind)), (OFFSET32, piece.code.len(), 1280));
    }
}
