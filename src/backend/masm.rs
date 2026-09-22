//! Allocated LIR as jwasm source.
//!
//! Port of `qbopt/backend/masm.py`. `listing` is the procedure as emitted,
//! frame and all; this prints it and omfwrite.rs encodes it, so the two
//! cannot drift. Every operand is already placed; an unplaced one is an
//! error.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, LazyLock};

use iced_x86::Register;
use indexmap::IndexMap;

use crate::backend::{select, target};
use crate::model::ir::{self, Addr, Loc, Operation, Semantics, Space};
use crate::model::lir;
use crate::support::pyrepr::Repr;

/// `SIZES`.
pub static SIZES: LazyLock<IndexMap<u32, &'static str>> =
    LazyLock::new(|| IndexMap::from([(1, "byte"), (2, "word"), (4, "dword"), (8, "qword"), (10, "tbyte")]));
/// `SAVED`: callee-saved under the C convention. A Borland caller keeps SI
/// and DI, not their upper halves, and a caller built here keeps nothing
/// across a call.
pub static SAVED: LazyLock<IndexMap<Register, Register>> =
    LazyLock::new(|| IndexMap::from([(Register::ESI, Register::SI), (Register::EDI, Register::DI)]));
/// `SEGMENTS`.
pub static SEGMENTS: LazyLock<IndexMap<&'static str, &'static str>> =
    LazyLock::new(|| IndexMap::from([("_DATA", ".data"), ("_BSS", ".data?"), ("CONST", ".const")]));

/// An instruction or operand this printer has no spelling for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unprintable(pub String);

impl fmt::Display for Unprintable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unprintable {}

/// `InlinePart`: bytes, or `(kind, symbol, offset)` for a fixup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InlinePart {
    Bytes(Vec<u8>),
    Fixup(String, String, i64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callee {
    pub name: String,
    pub far: bool,
    /// Inline assembly laid down in place of a call.
    pub code: Vec<InlinePart>,
}

impl Callee {
    /// `Callee(name, far)`, with no inline code.
    pub fn new(name: impl Into<String>, far: bool) -> Self {
        Self { name: name.into(), far, code: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub public: bool,
    pub far: bool,
    pub body: lir::LirBody,
    /// bytes below bp: locals and spill slots
    pub reserve: i64,
    pub callees: IndexMap<i64, Callee>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fill {
    pub size: i64,
    /// None: uninitialised
    pub byte: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pointer {
    pub name: String,
    pub offset: i64,
    pub far: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Align {
    pub to: i64,
}

/// `Datum`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Datum {
    Label(Label),
    Fill(Fill),
    Pointer(Pointer),
    Align(Align),
    Bytes(Vec<u8>),
    /// A frontend's own item, which Python's untyped `Module.data` carries
    /// through: QB's `_SegmentWord`, a `dw seg name` only its writer encodes.
    SegmentWord(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// the code segment's name, MODULE_TEXT
    pub code: String,
    pub names: IndexMap<(Space, i64), String>,
    /// (name, "far" | "near" | "byte")
    pub externs: Vec<(String, String)>,
    pub publics: Vec<String>,
    /// (segment, items)
    pub data: Vec<(String, Vec<Datum>)>,
    pub procedures: Vec<Procedure>,
    /// data segments outside DGROUP; only ever asked for membership
    pub private: BTreeSet<String>,
}

pub fn text(module: &Module) -> Result<String, Unprintable> {
    let mut out: Vec<String> = vec![".model medium".into(), ".386".into(), String::new()];
    out.extend(module.publics.iter().map(|name| format!("public {name}")));
    for (segment, items) in &module.data {
        let private = module.private.contains(segment);
        out.push(SEGMENTS.get(segment.as_str()).map_or_else(
            || format!("{segment} segment word public '{}'", if private { "FAR_DATA" } else { "DATA" }),
            |one| (*one).to_owned(),
        ));
        out.extend(
            module.externs.iter().filter(|(_, kind)| kind == "byte").map(|(name, _)| format!("extern {name}:byte")),
        );
        out.extend(items.iter().flat_map(datum));
        if !SEGMENTS.contains_key(segment.as_str()) {
            out.push(format!("{segment} ends"));
            if !private {
                out.push(format!("DGROUP group {segment}"));
            }
        }
    }
    out.extend(module.externs.iter().filter(|(_, kind)| kind != "byte").map(|(name, kind)| {
        format!("extern {name}:{}", if kind == "far-byte" { "byte" } else { kind })
    }));
    out.push(format!(".code {}", module.code));
    for (number, procedure) in module.procedures.iter().enumerate() {
        out.extend(_procedure(procedure, &module.names, number)?);
    }
    out.push("end".into());
    Ok(out.join("\n") + "\n")
}

pub fn datum(item: &Datum) -> Vec<String> {
    match item {
        Datum::Label(Label { name }) => vec![format!("{name} label byte")],
        Datum::Fill(Fill { size, byte }) => {
            vec![format!("    db {size} dup ({})", byte.map_or_else(|| "?".to_owned(), |one| one.to_string()))]
        }
        Datum::Pointer(Pointer { name, offset, far }) => {
            vec![format!("    {} {name}{}", if *far { "dd" } else { "dw" }, _signed(*offset))]
        }
        Datum::Align(Align { to }) => vec![format!("    align {to}")],
        Datum::Bytes(item) => _code(&[InlinePart::Bytes(item.clone())]),
        // `_code((item,))` yields nothing for an item it cannot match.
        Datum::SegmentWord(_) => Vec::new(),
    }
}

/// `Item`.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Label(Label),
    Callee(Callee),
    Semantics(Semantics),
}

fn reg(register: Register) -> Loc {
    Loc::Reg(ir::Reg { register, width: 2 })
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

/// The implicit entry and return sequences shared by text and OMF emission.
pub fn _frame_parts(procedure: &Procedure) -> (Vec<Semantics>, Vec<Semantics>) {
    let roots = _roots(&procedure.body);
    let saved: Vec<Register> =
        SAVED.iter().filter(|(whole, _)| roots.contains(whole)).map(|(_, low)| *low).collect();
    let reserve = procedure.reserve + (procedure.reserve & 1);
    // Inline code is bytes this printer cannot read, so it may address the frame.
    let framed = reserve != 0
        || roots.contains(&Register::EBP)
        || procedure.callees.values().any(|one| !one.code.is_empty());
    let (bp, sp) = (reg(Register::BP), reg(Register::SP));
    let mut leave: Vec<Semantics> =
        saved.iter().rev().map(|one| semantics(Operation::Pop, "pop", vec![reg(*one)], vec![])).collect();
    if reserve != 0 {
        leave.push(semantics(Operation::Nothing, "leave", vec![], vec![]));
    } else if framed {
        leave.push(semantics(Operation::Pop, "pop", vec![bp.clone()], vec![]));
    }
    let mut enter: Vec<Semantics> = Vec::new();
    if framed {
        enter.extend([
            semantics(Operation::Push, "push", vec![], vec![bp.clone()]),
            semantics(Operation::Move, "mov", vec![bp], vec![sp.clone()]),
        ]);
    }
    if reserve != 0 {
        enter.push(semantics(
            Operation::Binary,
            "sub",
            vec![sp.clone()],
            vec![sp, Loc::Imm(ir::Imm { value: reserve, width: 2, address: None })],
        ));
    }
    enter.extend(saved.iter().map(|one| semantics(Operation::Push, "push", vec![], vec![reg(*one)])));
    (enter, leave)
}

/// Bytes emitted before each RETURN but absent from allocated LIR.
pub fn return_overhead_bytes(procedure: &Procedure) -> Result<usize, Unprintable> {
    let (_enter, leave) = _frame_parts(procedure);
    let emitted: Vec<Option<select::Emitted>> =
        leave.iter().map(|one| select::emit(one, 0, None, false, false, None)).collect();
    if emitted.iter().any(Option::is_none) {
        return Err(Unprintable(format!("{}: implicit return sequence is not encodable", procedure.name)));
    }
    Ok(emitted.iter().flatten().map(|one| one.code.len()).sum())
}

/// The procedure as emitted, frame included: what this prints and omfwrite
/// encodes. A branch's target is still a block; `label(number, at)` names it.
pub fn listing(procedure: &Procedure, number: usize) -> Result<Vec<Item>, Unprintable> {
    let (enter, leave) = _frame_parts(procedure);
    let mut out: Vec<Item> = enter.into_iter().map(Item::Semantics).collect();
    let blocks = &procedure.body.blocks;
    for (index, block) in blocks.iter().enumerate() {
        out.push(Item::Label(Label { name: label(number, block.at) }));
        let following = if index + 1 < blocks.len() { Some(blocks[index + 1].at) } else { None };
        let fallthrough = _fallthrough_jump(block, following);
        for one in &block.insns {
            if fallthrough.is_some_and(|jump| Arc::ptr_eq(jump, one)) {
                continue;
            }
            let Some(what) = &one.what else {
                return Err(Unprintable(format!("{} at {}: an instruction with no semantics", procedure.name, one.at)));
            };
            match what.op {
                Operation::Move if _segment(&what.dests[0]) && matches!(what.sources[0], Loc::Imm(_)) => {
                    // x86 has no immediate move into a segment register; the stack holds it for one instruction.
                    let Loc::Imm(source) = &what.sources[0] else { unreachable!() };
                    out.extend([
                        Item::Semantics(semantics(
                            Operation::Push,
                            "push",
                            vec![],
                            vec![Loc::Imm(ir::Imm { width: 2, ..source.clone() })],
                        )),
                        Item::Semantics(semantics(Operation::Pop, "pop", what.dests.clone(), vec![])),
                    ]);
                }
                Operation::Call => {
                    if what.indirect {
                        out.push(Item::Semantics(what.clone()));
                    } else {
                        let Some(callee) = procedure.callees.get(&one.at) else {
                            return Err(Unprintable(format!("{} at {}: a call with no callee", procedure.name, one.at)));
                        };
                        out.push(Item::Callee(callee.clone()));
                    }
                }
                Operation::Return => {
                    out.extend(leave.iter().cloned().map(Item::Semantics));
                    let name = match what.name.as_deref() {
                        Some(name) if !name.is_empty() => name.to_owned(),
                        _ => (if procedure.far { "retf" } else { "ret" }).to_owned(),
                    };
                    out.push(Item::Semantics(Semantics { name: Some(name), ..what.clone() }));
                }
                _ => out.push(Item::Semantics(what.clone())),
            }
        }
        let fall = _falls_to(block, &procedure.name)?;
        if let Some(fall) = fall {
            if index + 1 == blocks.len() || blocks[index + 1].at != fall {
                out.push(Item::Semantics(Semantics {
                    target: Some(fall),
                    ..semantics(Operation::Jump, "jmp", vec![], vec![])
                }));
            }
        }
    }
    Ok(out)
}

/// The explicit edge that physical adjacency makes free, if there is one.
///
/// Frontends may keep every CFG edge explicit through allocation. Listing is
/// where block order becomes physical, and therefore the first common layer
/// that can say an unconditional edge reaches the instruction already next.
/// NOTHING anchors after the edge own source locations but emit no bytes, so
/// the last instruction that actually prints is the one that matters.
pub fn _fallthrough_jump(block: &lir::LirBlock, following: Option<i64>) -> Option<&Arc<lir::Insn>> {
    let following = following?;
    if block.succ != [following] {
        return None;
    }
    let last = block.insns.iter().rev().find(|one| match &one.what {
        None => true,
        Some(what) => what.op != Operation::Nothing || !matches!(what.name.as_deref().unwrap_or(""), "" | "nop"),
    })?;
    let what = last.what.as_ref()?;
    if what.op != Operation::Jump || what.target != Some(following) {
        return None;
    }
    Some(last)
}

pub fn label(number: usize, at: i64) -> String {
    format!("L{number}_{at}")
}

pub fn _procedure(
    procedure: &Procedure,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
) -> Result<Vec<String>, Unprintable> {
    let mut out = vec![format!("{} proc {}", procedure.name, if procedure.far { "far" } else { "near" })];
    for item in listing(procedure, number)? {
        match item {
            Item::Label(Label { name }) => out.push(format!("{name}:")),
            Item::Callee(Callee { code, .. }) if !code.is_empty() => {
                out.extend(_code(&code).into_iter().map(|line| format!("    {line}")));
            }
            Item::Callee(Callee { name, far, .. }) => {
                out.push(format!("    call {}{name}", if far { "far ptr " } else { "" }));
            }
            Item::Semantics(item) => match _instruction(&item, names, number) {
                Ok(lines) => out.extend(lines.into_iter().map(|line| format!("    {line}"))),
                Err(error) => return Err(Unprintable(format!("{}: {error}", procedure.name))),
            },
        }
    }
    out.push(format!("{} endp", procedure.name));
    Ok(out)
}

/// The successor control reaches by running off the block's end, if any.
pub fn _falls_to(block: &lir::LirBlock, name: &str) -> Result<Option<i64>, Unprintable> {
    let last = block
        .insns
        .iter()
        .rev()
        .filter_map(|one| one.what.as_ref())
        .find(|what| what.op != Operation::Nothing);
    if last.is_some_and(|last| matches!(last.op, Operation::Jump | Operation::Return)) {
        return Ok(None);
    }
    let taken = match last {
        Some(last) if last.op == Operation::Branch => last.target,
        _ => None,
    };
    let mut rest: Vec<i64> = block.succ.iter().copied().filter(|one| Some(*one) != taken).collect();
    if rest.is_empty() {
        rest = block.succ.clone();
    }
    if rest.len() > 1 {
        return Err(Unprintable(format!(
            "{name}: block {} leaves for {} with no instruction choosing",
            block.at,
            crate::support::pyrepr::tuple(&block.succ)
        )));
    }
    Ok(rest.first().copied())
}

pub fn _roots(body: &lir::LirBody) -> BTreeSet<Register> {
    let mut found = BTreeSet::new();
    for one in body.insns() {
        let Some(what) = &one.what else { continue };
        for r#where in what.dests.iter().chain(&what.sources) {
            match r#where {
                Loc::Reg(ir::Reg { register, .. }) => {
                    found.insert(ir::root(*register));
                }
                Loc::Mem(ir::Mem { through, index_through, .. }) => {
                    found.extend([*through, *index_through].map(ir::root));
                }
                _ => {}
            }
        }
    }
    found
}

pub fn _instruction(
    what: &Semantics,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
) -> Result<Vec<String>, Unprintable> {
    let name = what.name.as_deref().unwrap_or("");
    if what.op == Operation::Fill {
        // Its operands are the registers the instruction names in its opcode.
        return Ok(vec![format!("{}{name}", if what.sources.len() == 4 { "rep " } else { "" })]);
    }
    let dests = what.dests.iter().map(|x| _operand(x, names)).collect::<Result<Vec<_>, _>>()?;
    let sources = what.sources.iter().map(|x| _operand(x, names)).collect::<Result<Vec<_>, _>>()?;
    let last = |items: &[String]| items[items.len() - 1].clone();
    Ok(match what.op {
        Operation::Nothing => {
            if matches!(name, "" | "nop") {
                vec![]
            } else {
                vec![name.to_owned()]
            }
        }
        Operation::Move | Operation::Address => vec![format!("{name} {}, {}", dests[0], sources[0])],
        Operation::Binary => vec![format!("{name} {}, {}", dests[0], sources[1])],
        Operation::Unary => vec![format!("{name} {}", dests[0])],
        Operation::Compare if name.starts_with('f') => {
            let memory: Vec<&String> = sources
                .iter()
                .zip(&what.sources)
                .filter(|(_, source)| matches!(source, Loc::Mem(_)))
                .map(|(text, _)| text)
                .collect();
            vec![if memory.is_empty() { name.to_owned() } else { format!("{name} {}", memory[0]) }]
        }
        Operation::Compare => {
            vec![format!("{} {}, {}", if name.is_empty() { "cmp" } else { name }, sources[0], sources[1])]
        }
        Operation::Multiply if dests.len() == 1 => {
            if sources.len() == 3 {
                vec![format!("imul {}, {}, {}", dests[0], sources[1], sources[2])]
            } else if matches!(what.sources[1], Loc::Imm(_)) {
                vec![format!("imul {}, {}, {}", dests[0], sources[0], sources[1])]
            } else {
                vec![format!("imul {}, {}", dests[0], sources[1])]
            }
        }
        Operation::Multiply | Operation::Divide => vec![format!("{name} {}", last(&sources))],
        Operation::Extend => {
            if matches!(name, "movsx" | "movzx") {
                vec![format!("{name} {}, {}", dests[0], sources[0])]
            } else {
                vec![name.to_owned()]
            }
        }
        Operation::Push => match &what.sources[0] {
            Loc::Imm(ir::Imm { address: None, width, .. })
            | Loc::Imm(ir::Imm { address: Some(Addr { space: Space::Group, .. }), width, .. }) => {
                vec![format!("push{} {}", if *width == 4 { "d" } else { "w" }, sources[0])]
            }
            _ => vec![format!("push {}", sources[0])],
        },
        Operation::Pop => vec![format!("pop {}", dests[0])],
        Operation::Exchange if name == "fxch" => vec![format!("fxch {}", dests[1])],
        Operation::Exchange => vec![format!("xchg {}, {}", dests[0], dests[1])],
        Operation::Funnel => vec![format!("{name} {}, {}, {}", dests[0], sources[1], sources[2])],
        Operation::Branch | Operation::Jump => {
            let Some(target) = what.target else {
                return Err(Unprintable(format!("{} with no target", if name.is_empty() { "jump" } else { name })));
            };
            vec![format!("{name} {}", label(number, target))]
        }
        Operation::Call if what.indirect && sources.len() == 1 => vec![format!("call {}", sources[0])],
        Operation::Barrier => {
            vec![format!("{name} {}", if dests.is_empty() { &sources[0] } else { &dests[0] })]
        }
        Operation::FloatLoad => {
            if matches!(name, "fldz" | "fld1") || sources.is_empty() {
                vec![name.to_owned()]
            } else {
                vec![format!("{name} {}", sources[0])]
            }
        }
        Operation::FloatStore => vec![format!("{name} {}", dests[0])],
        Operation::FloatArith if matches!(what.sources[what.sources.len() - 1], Loc::Mem(_)) => {
            vec![format!("{name} {}", last(&sources))]
        }
        Operation::FloatArith | Operation::FloatArithPop => vec![format!("{name} {}, {}", dests[0], last(&sources))],
        Operation::FloatUnary => vec![name.to_owned()],
        Operation::Return => vec![name.to_owned()],
        _ => return Err(Unprintable(what.repr())),
    })
}

pub fn _code(parts: &[InlinePart]) -> Vec<String> {
    let mut out = Vec::new();
    for part in parts {
        match part {
            InlinePart::Bytes(part) => {
                for chunk in part.chunks(16) {
                    out.push(
                        "db ".to_owned()
                            + &chunk.iter().map(|byte| format!("0{byte:02x}h")).collect::<Vec<_>>().join(","),
                    );
                }
            }
            InlinePart::Fixup(kind, name, offset) if kind == "offset" => {
                out.push(format!("dw offset {name}{}", _signed(*offset)));
            }
            InlinePart::Fixup(kind, name, _) if kind == "segment" => out.push(format!("dw seg {name}")),
            InlinePart::Fixup(..) => {}
        }
    }
    out
}

pub fn _segment(r#where: &Loc) -> bool {
    matches!(
        r#where,
        Loc::Reg(ir::Reg { register: Register::ES | Register::DS | Register::SS | Register::FS | Register::GS, .. })
    )
}

fn named(names: &IndexMap<(Space, i64), String>, address: &Addr) -> String {
    let key = (address.space, address.index);
    names.get(&key).cloned().unwrap_or_else(|| panic!("KeyError: ({}, {})", address.space.repr(), address.index))
}

pub fn _operand(r#where: &Loc, names: &IndexMap<(Space, i64), String>) -> Result<String, Unprintable> {
    Ok(match r#where {
        Loc::Reg(ir::Reg { register, .. }) => target::name_of(*register),
        Loc::St(ir::St { index }) => format!("st({index})"),
        Loc::Imm(ir::Imm { value, address: None, .. }) => value.to_string(),
        Loc::Imm(ir::Imm { value, address: Some(address), .. }) => {
            if address.space == Space::Group {
                named(names, address)
            } else {
                format!("offset {}{}", named(names, address), _signed(address.disp + value))
            }
        }
        Loc::Mem(cell) => _memory(cell, names)?,
        Loc::Address(ir::Address { addr: Some(address), index: Register::None, .. }) => {
            let text = _memory(&ir::Mem::new(Some(*address), 2), names)?;
            text.strip_prefix("word ptr ").map_or(text.clone(), str::to_owned)
        }
        Loc::Address(ir::Address { through, index, scale, offset, .. }) => {
            format!("[{}{}]", _registers(*through, *index, *scale), _signed(*offset))
        }
        Loc::Held(_) => return Err(Unprintable(format!("operand {}", r#where.repr()))),
    })
}

pub fn _registers(base: Register, index: Register, scale: i64) -> String {
    let mut parts = if base != Register::None { vec![target::name_of(base)] } else { vec![] };
    if index != Register::None {
        parts.push(target::name_of(index) + &(if scale != 1 { format!("*{scale}") } else { String::new() }));
    }
    parts.join("+")
}

pub fn _memory(cell: &ir::Mem, names: &IndexMap<(Space, i64), String>) -> Result<String, Unprintable> {
    let size = format!(
        "{} ptr ",
        SIZES.get(&cell.width).unwrap_or_else(|| panic!("KeyError: {}", cell.width))
    );
    // As select.operand_of: a named address carries the displacement, and
    // `offset` is only the displacement of a cell with none.
    let Some(address) = &cell.addr else {
        if cell.through == Register::None {
            return Err(Unprintable(format!("cell {}", cell.repr())));
        }
        return Ok(format!("{size}[{}{}]", target::name_of(cell.through), _signed(cell.offset)));
    };
    let registers = _registers(cell.through, cell.index_through, cell.scale);
    let disp = _signed(address.disp);
    match address.space {
        Space::Frame => {
            return Ok(format!("{size}[{}{disp}]", if registers.is_empty() { "bp" } else { &registers }));
        }
        Space::Segment | Space::External => {
            let symbol = named(names, address);
            let indexed = if registers.is_empty() { String::new() } else { format!("[{registers}]") };
            return Ok(format!("{size}{symbol}{disp}{indexed}"));
        }
        Space::Literal if !registers.is_empty() => {
            let segment = if address.segment == Register::None {
                String::new()
            } else {
                format!("{}:", target::name_of(address.segment))
            };
            return Ok(format!("{size}{segment}[{registers}{disp}]"));
        }
        Space::Far if !registers.is_empty() => {
            return Ok(format!("{size}{}:[{registers}{disp}]", target::name_of(address.segment)));
        }
        _ => {}
    }
    Err(Unprintable(format!("cell {}", cell.repr())))
}

pub fn _signed(n: i64) -> String {
    if n > 0 {
        format!("+{n}")
    } else if n < 0 {
        n.to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    //! Port of `tests/test_masm.py`: the jwasm printer spells each operand as
    //! select encodes it.

    use super::*;
    use crate::backend::lower::{self, Placed};
    use crate::model::mir;

    fn no_names() -> IndexMap<(Space, i64), String> {
        IndexMap::new()
    }

    fn insn(at: i64, what: Semantics) -> Arc<lir::Insn> {
        Arc::new(lir::Insn::new(at, Some((at, 1)), Some(what), vec![], vec![]))
    }

    fn ax() -> Loc {
        Loc::Reg(ir::Reg { register: Register::AX, width: 2 })
    }

    /// `lea bx,[bp-20]` for `&n` at bp-10: lower carries the displacement in
    /// both addr and offset, and the printer added them. asset_seek wrote n
    /// ten bytes below where qglsurf then read it.
    #[test]
    fn test_frame_address_displacement_once() {
        let Placed::Loc(placed) = lower::operand(&mir::Arg::FrameAddress(mir::FrameAddress::new(-10, 2))) else {
            panic!("a frame address places as a location");
        };
        assert_eq!(_operand(&placed, &no_names()).unwrap(), "[bp-10]");
    }

    /// `es:[bx+si+4]` for field 2 of a 3-byte record: pal_install copied the
    /// palette's blue from the next entry's green.
    #[test]
    fn test_far_cell_displacement_once() {
        let addr = Addr { base: Register::BX, segment: Register::ES, ..Addr::new(Space::Far, 2) };
        let cell = ir::Mem { through: Register::BX, offset: 2, ..ir::Mem::new(Some(addr), 1) };
        assert_eq!(_operand(&Loc::Mem(cell), &no_names()).unwrap(), "byte ptr es:[bx+2]");
    }

    /// FloatAlloc's `fxch st(1)` printed as `xchg st(0), st(1)`, which jwasm
    /// refuses: 122 errors over qcport.
    #[test]
    fn test_x87_exchange_is_fxch() {
        let st = |index| Loc::St(ir::St { index });
        let swap = semantics(Operation::Exchange, "fxch", vec![st(0), st(1)], vec![st(0), st(1)]);
        assert_eq!(_instruction(&swap, &no_names(), 0).unwrap(), ["fxch st(1)"]);
    }

    fn _printed(sources: Vec<Loc>, reserve: i64) -> Vec<String> {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], sources));
        let leave = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, leave])];
        let body = lir::LirBody::new("get", 1, blocks, IndexMap::new(), IndexMap::new());
        let procedure =
            Procedure { name: "_get".into(), public: true, far: true, body, reserve, callees: IndexMap::new() };
        _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect()
    }

    fn through_bp() -> Loc {
        Loc::Mem(ir::Mem { through: Register::BP, ..ir::Mem::new(Some(Addr::new(Space::Frame, 6)), 2) })
    }

    fn has(lines: &[String], text: &str) -> bool {
        lines.iter().any(|one| one == text)
    }

    /// Every procedure got `push bp; mov bp,sp` .. `mov sp,bp; pop bp`: snd_mix_loops
    /// read one global in seven instructions where bcc used two.
    #[test]
    fn test_frame_only_where_something_uses_it() {
        let bx = Loc::Reg(ir::Reg { register: Register::BX, width: 2 });
        assert_eq!(_printed(vec![bx], 0), ["_get proc far", "L0_1:", "mov ax, bx", "retf", "_get endp"]);
        let params = _printed(vec![through_bp()], 0);
        let tail = &params[params.len() - 3..params.len() - 1];
        assert!(params[1..3] == ["push bp", "mov bp, sp"] && tail == ["pop bp", "retf"]);
        assert!(!has(&params, "mov sp, bp") && !has(&params, "leave"));
    }

    /// `mov sp,bp; pop bp` where bcc writes `leave`: 261 instructions over qcport.
    #[test]
    fn test_reserved_frame_leaves_in_one_instruction() {
        let lines = _printed(vec![through_bp()], 4);
        assert!(lines[lines.len() - 3..lines.len() - 1] == ["leave", "retf"]);
        assert!(!has(&lines, "mov sp, bp") && !has(&lines, "pop bp"));
    }

    /// Return-tail layout must count the pop/leave absent from allocated LIR.
    #[test]
    fn test_return_overhead_prices_the_implicit_frame_teardown() {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let returned = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, returned])];
        let body = lir::LirBody::new("get", 1, blocks, IndexMap::new(), IndexMap::new());
        let procedure = |reserve| Procedure {
            name: "_get".into(),
            public: true,
            far: true,
            body: body.clone(),
            reserve,
            callees: IndexMap::new(),
        };

        assert_eq!(return_overhead_bytes(&procedure(0)).unwrap(), 1);
        assert_eq!(return_overhead_bytes(&procedure(4)).unwrap(), 1);
    }

    /// SI and DI were pushed and popped whole: an operand-size prefix on every save
    /// and restore, for upper halves no Borland caller keeps across a call.
    #[test]
    fn test_callee_saves_only_what_the_convention_keeps() {
        let lines = _printed(vec![Loc::Reg(ir::Reg { register: Register::ESI, width: 4 })], 0);
        assert!(has(&lines, "push si") && has(&lines, "pop si"));
        assert!(!has(&lines, "push esi") && !has(&lines, "pop esi"));
    }

    /// peephole's multiply by three has no address, only base, index and scale.
    #[test]
    fn test_arithmetic_lea_scales_its_index() {
        let r#where = ir::Address { through: Register::EBX, index: Register::EBX, scale: 2, ..ir::Address::new(None) };
        assert_eq!(_operand(&Loc::Address(r#where), &no_names()).unwrap(), "[ebx+ebx*2]");
    }
}
