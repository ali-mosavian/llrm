//! Allocated LIR as jwasm source.
//!
//! Port of `qbopt/backend/masm.py`. `listing` is the procedure as emitted,
//! frame and all; this prints it and objbuild.rs encodes it, so the two
//! cannot drift. Every operand is already placed; an unplaced one is an
//! error.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, LazyLock};

use iced_x86::Register;
use crate::support::hash::IndexMap;

use crate::backend::{select, target};
use crate::model::ir::{self, Addr, Loc, Operation, Semantics, Space};
use crate::model::lir;
use crate::support::pyrepr::Repr;

/// What LIR calls the frame register and the stack pointer, whatever the target: `spelled` gives each its own.
const FRAME: Register = Register::BP;
const STACK: Register = Register::SP;

/// `SIZES`.
pub static SIZES: LazyLock<IndexMap<u32, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([(1, "byte"), (2, "word"), (4, "dword"), (8, "qword"), (10, "tbyte")]));
/// `SEGMENTS`.
pub static SEGMENTS: LazyLock<IndexMap<&'static str, &'static str>> =
    LazyLock::new(|| IndexMap::from_iter([("_DATA", ".data"), ("_BSS", ".data?"), ("CONST", ".const")]));

/// An instruction or operand this printer has no spelling for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unprintable(pub String);

impl fmt::Display for Unprintable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unprintable {}

impl From<Unprintable> for crate::model::passes::Exception {
    fn from(one: Unprintable) -> Self {
        Self::defined_in("qbopt.backend.masm", "Unprintable", one.0)
    }
}

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
    /// The argument bytes it pops as it returns: the stack is that much higher after the call.
    pub pops: i64,
    /// Inline assembly laid down in place of a call.
    pub code: Vec<InlinePart>,
}

impl Callee {
    /// `Callee(name, far)`, with no inline code.
    pub fn new(name: impl Into<String>, far: bool) -> Self {
        Self { name: name.into(), far, pops: 0, code: Vec::new() }
    }
}

pub use crate::hir::model::StackCheck;

/// The data object `StackCheck::limit` names, which no global takes.
pub const STACK_LIMIT_ID: i64 = i64::MAX - 1;

#[derive(Clone, Debug, PartialEq)]
pub struct Procedure {
    pub name: String,
    pub public: bool,
    pub far: bool,
    pub body: lir::LirBody,
    /// bytes below bp: locals and spill slots
    pub reserve: i64,
    pub callees: IndexMap<i64, Callee>,
    /// An interrupt handler's data group, whose selector it loads into DS
    /// and ES; `None` for a procedure entered by a call.
    pub interrupt: Option<Addr>,
    /// Tuned for size (-Os): the jumps `objbuild` lays out.
    pub size: bool,
    /// Bytes the runtime's entry call (B$ENSA) takes below BP, which no instruction of the
    /// procedure shows: its header and the locals `cx` names.
    pub entry: i64,
    /// Compare SP with the runtime's limit once the frame is allocated.
    pub stack_check: Option<StackCheck>,
    /// The target's frame register, stack pointer and callee-saved registers.
    pub registers: llrm_target::FrameRegisters,
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
    /// A selector and an offset, where the target has selectors.
    pub far: bool,
    /// The bytes of the cell it fills: the target's pointer width (or the offset's, for a far one made of an offset
    /// word and a selector word).
    pub bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Align {
    pub to: i64,
}

/// `Datum`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Datum {
    Label(Label),
    /// A label heading data a writer drops when nothing names it: up to the next Object.
    Object(Label),
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
    /// Of those, the segments of uninitialised data: class FAR_BSS, which an image does not store.
    pub far_bss: BTreeSet<String>,
    /// Externs nothing references, declared so LINK pulls in their module.
    pub requests: BTreeSet<String>,
    /// `-g`'s debug information.
    pub debug: Option<super::debuginfo::Debug>,
    /// Bytes of the linker's stack this module adds to the others' (OMF stack
    /// segments concatenate): where its call graph is the whole program's.
    pub stack: i64,
    /// The target's object format: the listing's header, the writer's mode.
    pub object: llrm_target::object::ObjectFormat,
}

impl Module {
    /// Whether `segment` is outside DGROUP, so reached by its own selector alone
    /// and paragraph aligned: its first byte is offset 0 of its frame, and a 64K
    /// object in it fits. Every writer asks this.
    pub fn selector_addressed(&self, segment: &str) -> bool {
        self.private.contains(segment)
    }
}

pub fn text(module: &Module) -> Result<String, Unprintable> {
    text_by(module, listing)
}

/// `module`'s text, each procedure's items as `listed` gives them.
pub fn text_by(module: &Module, listed: impl Fn(&Procedure, usize) -> Result<Vec<Item>, Unprintable>) -> Result<String, Unprintable> {
    let mut out: Vec<String> = module.object.header.iter().cloned().chain([String::new()]).collect();
    out.extend(module.publics.iter().map(|name| format!("public {name}")));
    if module.stack > 0 {
        out.push(format!(".stack {}", module.stack));
    }
    for (segment, items) in &module.data {
        let private = module.selector_addressed(segment);
        out.push(SEGMENTS.get(segment.as_str()).map_or_else(
            || format!("{segment} segment {} public '{}'", if private { "para" } else { "word" }, if module.far_bss.contains(segment) { "FAR_BSS" } else if private { "FAR_DATA" } else { "DATA" }),
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
        out.extend(_procedure_of(procedure, listed(procedure, number)?, &module.names, number)?);
    }
    out.push("end".into());
    Ok(out.join("\n") + "\n")
}

pub fn datum(item: &Datum) -> Vec<String> {
    match item {
        Datum::Label(Label { name }) | Datum::Object(Label { name }) => vec![format!("{name} label byte")],
        Datum::Fill(Fill { size, byte }) => {
            vec![format!("    db {size} dup ({})", byte.map_or_else(|| "?".to_owned(), |one| one.to_string()))]
        }
        Datum::Pointer(Pointer { name, offset, bytes, .. }) => {
            vec![format!("    {} {name}{}", match bytes { 4 => "dd", 2 => "dw", other => panic!("a pointer of {other} bytes") }, _signed(*offset))]
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
    Mark(Mark),
}

/// `-g`: a place in the code a debugger is told of.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mark {
    /// The code after it is this source line's: the procedure's `index`th
    /// line, where [`line_label`] names it.
    Line { line: u32, index: u32 },
    /// The procedure's own code starts, its prologue done: the first code
    /// of a source line.
    BodyStart,
    /// The procedure's own code ends, its epilogue next: after the last
    /// code of a source line but a return.
    BodyEnd,
    /// The call before it popped this many bytes of its arguments as it returned: the stack is that much higher
    /// from here.
    Pops(i64),
}

fn reg(register: Register) -> Loc {
    Loc::Reg(ir::Reg { register, width: 2 })
}

fn semantics(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

/// Every return of `body` as `retf bytes`, for a callee that removes its
/// arguments. `ret imm16` cannot remove 64 KB or more: a near return of a 32-bit target keeps the count, and the epilogue
/// writes `pop [esp + n]; add esp, n; ret` (`moved_return`); `address` is the bytes of a return address.
pub fn cleaned_returns(body: &lir::LirBody, bytes: i64, address: i64) -> Result<lir::LirBody, String> {
    if !(0..=llrm_x86::calling::RET_POPS_MOST).contains(&bytes) && !(bytes > llrm_x86::calling::RET_POPS_MOST && address == 4) {
        return Err("far-return cleanup exceeds 16 bits".into());
    }
    if bytes == 0 {
        return Ok(body.clone());
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let insns = block
                .insns
                .iter()
                .map(|one| match one.what.as_ref() {
                    Some(what) if what.op == Operation::Return => {
                        let mut replaced = (**one).clone();
                        replaced.what = Some(Semantics {
                            sources: vec![Loc::Imm(ir::Imm { value: bytes, width: if bytes > llrm_x86::calling::RET_POPS_MOST { 4 } else { 2 }, address: None })],
                            ..what.clone()
                        });
                        Arc::new(replaced)
                    }
                    _ => Arc::clone(one),
                })
                .collect();
            block.with_insns(insns)
        })
        .collect();
    Ok(body.with_blocks(blocks))
}

/// Names the limit each procedure's stack check compares with, and the externs those checks
/// need: the limit a data object, the handler a far routine, unless a procedure here is it.
pub fn stack_externs(procedures: &[Procedure], names: &mut IndexMap<(Space, i64), String>) -> Vec<(String, String)> {
    let mut externs = Vec::new();
    for check in procedures.iter().filter_map(|one| one.stack_check.as_ref()) {
        names.insert((Space::External, STACK_LIMIT_ID), check.limit.clone());
        externs.push((check.limit.clone(), "byte".to_owned()));
        if !procedures.iter().any(|one| one.name == check.handler) {
            externs.push((check.handler.clone(), if check.far { "far" } else { "near" }.to_owned()));
        }
    }
    externs
}

/// Whether a procedure, by its symbol, is entered only by the direct calls `module`'s own
/// functions make: its address is taken nowhere (`callgraph::addressed`).
pub fn entered_directly<'a>(module: &'a llrm_mir::Module, names: &'a IndexMap<(Space, i64), String>) -> impl Fn(&str) -> bool + 'a {
    let addressed: BTreeSet<&String> = llrm_mir::callgraph::addressed(module).iter().filter_map(|id| names.get(&(Space::Segment, i64::from(id.0)))).collect();
    move |name| !addressed.contains(&name.to_owned())
}

/// The block number the overflow call takes: past every instruction's.
pub fn cold_at(body: &lir::LirBody) -> i64 {
    body.blocks.iter().map(|block| block.at).chain(body.insns().into_iter().map(|one| one.at)).max().unwrap_or(0) + 1
}

fn sp_reg() -> Loc {
    reg(STACK)
}

/// The registers `procedure` keeps for its caller: each one it names, or a call it makes disturbs by its
/// convention, that this convention leaves to the callee, by its low half. A call to a routine whose
/// convention disturbs more than this one's (cdecl's ECX and EDX under Watcom's) takes the caller's value.
fn saved_of(procedure: &Procedure) -> Vec<Register> {
    let mut roots = _roots(&procedure.body);
    for one in procedure.body.insns() {
        roots.extend(one.call.iter().flat_map(|call| call.disturbs.iter().copied().map(ir::root)));
    }
    // An interrupt handler has saved everything before its frame, and a runtime-built one has saved
    // SI and DI (B$ENRA/B$ENRD, restored by B$EXSA).
    let owned = procedure.interrupt.is_some() || procedure.entry != 0;
    procedure.registers.saved.iter().filter(|(whole, _)| !owned && roots.contains(whole)).map(|(_, low)| *low).collect()
}

/// Where `procedure` saves them, where that is not its entry (`shrinkwrap`).
fn wrap_of(procedure: &Procedure) -> Option<crate::backend::shrinkwrap::Wrap> {
    let kept: BTreeSet<Register> = procedure.registers.saved.iter().filter(|(_, low)| saved_of(procedure).contains(low)).map(|(whole, _)| *whole).collect();
    crate::backend::shrinkwrap::wrapped(&procedure.body, &kept)
}

/// The implicit entry and return sequences shared by text and OMF emission.
pub fn _frame_parts(procedure: &Procedure) -> (Vec<Semantics>, Vec<Semantics>) {
    parts(procedure, stack_addressed(procedure, 0).is_some())
}

/// `_frame_parts`, with no frame register where `omit`: the entry sets none and the return takes back
/// what the entry reserved.
fn parts(procedure: &Procedure, omit: bool) -> (Vec<Semantics>, Vec<Semantics>) {
    let roots = _roots(&procedure.body);
    let saved = saved_of(procedure);
    let slot = procedure.registers.slot;
    let reserve = (procedure.reserve + slot - 1) / slot * slot;
    // Where the frame register's own cell was stays reserved: a cell addressed through an index has
    // no sign to say whether it lies above that cell or below it, so every one keeps its distance from it.
    let reserve = if omit && reserve != 0 { reserve + slot } else { reserve };
    // Inline code is bytes this printer cannot read, so it may address the frame.
    let framed = !omit
        && (reserve != 0
            || roots.contains(&ir::root(procedure.registers.pointer))
            || procedure.callees.values().any(|one| !one.code.is_empty()));
    let (bp, sp) = (reg(FRAME), reg(STACK));
    let mut leave: Vec<Semantics> =
        saved.iter().rev().map(|one| semantics(Operation::Pop, "pop", vec![reg(*one)], vec![])).collect();
    if omit && reserve != 0 {
        leave.push(semantics(Operation::Binary, "add", vec![sp_reg()], vec![sp_reg(), Loc::Imm(ir::Imm { value: reserve, width: 2, address: None })]));
    } else if reserve != 0 && framed {
        leave.push(semantics(Operation::Nothing, "leave", vec![], vec![]));
    } else if framed {
        leave.push(semantics(Operation::Pop, "pop", vec![bp.clone()], vec![]));
    }
    // `enter` only where the target's description takes it for size (`frame_enter`).
    let opened = framed && reserve != 0 && procedure.size && procedure.registers.enter && procedure.stack_check.is_none();
    let mut enter: Vec<Semantics> = Vec::new();
    if opened {
        let count = |value, width| Loc::Imm(ir::Imm { value, width, address: None });
        enter.push(semantics(Operation::Nothing, "enter", vec![], vec![count(reserve, 2), count(0, 1)]));
    } else if framed {
        enter.extend([
            semantics(Operation::Push, "push", vec![], vec![bp.clone()]),
            semantics(Operation::Move, "mov", vec![bp], vec![sp.clone()]),
        ]);
    }
    if reserve != 0 && !opened {
        enter.push(semantics(
            Operation::Binary,
            "sub",
            vec![sp.clone()],
            vec![sp, Loc::Imm(ir::Imm { value: reserve, width: 2, address: None })],
        ));
    }
    if procedure.stack_check.is_some() {
        let cold = Some(cold_at(&procedure.body));
        let overflow = |name: &str| Semantics { target: cold, ..semantics(Operation::Branch, name, vec![], vec![]) };
        let limit = Addr { index: STACK_LIMIT_ID, ..Addr::new(Space::External, 0) };
                // `sub` carries where SP wrapped past zero.
        if reserve != 0 {
            enter.push(overflow("jb"));
        }
        enter.extend([
            // The limit is a stack slot's word.
            semantics(Operation::Compare, "cmp", vec![], vec![sp_reg(), Loc::Mem(ir::Mem::new(Some(limit), procedure.registers.slot as u32))]),
            overflow("jb"),
        ]);
    }
    enter.extend(saved.iter().map(|one| semantics(Operation::Push, "push", vec![], vec![reg(*one)])));
    if let Some(group) = procedure.interrupt {
        let (before, after) = _interrupt_parts(group);
        enter.splice(0..0, before);
        leave.extend(after);
    }
    (enter, leave)
}

/// Where the frame an interrupt handler saved starts above its own BP: the
/// last register it saved, past what its entry pushes after those and the BP
/// its frame pushes. the target's `interrupt_frame` (calling.toml) is the frame.
pub fn interrupt_parameters() -> i64 {
    let (enter, _) = _interrupt_parts(Addr::new(Space::Group, 0));
    let pushed: i64 = enter[INTERRUPT_SAVED..]
        .iter()
        .map(|one| match (one.op, one.name.as_deref()) {
            (_, Some("pushad")) => 32,
            (Operation::Push, _) => 2,
            (Operation::Pop, _) => -2,
            _ => 0,
        })
        .sum();
    pushed + 2
}

/// The entry's first instructions, which save the frame: PUSHAD, then the
/// segments, so the last one pushed is the lowest slot.
const INTERRUPT_SAVED: usize = 5;

/// What an interrupt handler wraps its frame in. It may interrupt anything,
/// so it saves every register it or a callee may change once, as the frame
/// the target's `interrupt_frame` (calling.toml) lays out, and gives compiled code what
/// it assumes: DGROUP in DS and ES, the direction flag clear. A handler's
/// register parameters are slots of that frame, so what it writes to them is
/// what POPAD or `iret` goes back with. The x87 state is not saved.
fn _interrupt_parts(group: Addr) -> (Vec<Semantics>, Vec<Semantics>) {
    let push = |one: Loc| semantics(Operation::Push, "push", vec![], vec![one]);
    let pop = |one: Register| semantics(Operation::Pop, "pop", vec![reg(one)], vec![]);
    let enter = vec![
        semantics(Operation::Nothing, "pushad", vec![], vec![]),
        push(reg(Register::DS)),
        push(reg(Register::ES)),
        push(reg(Register::FS)),
        push(reg(Register::GS)),
        push(Loc::Imm(ir::Imm { value: 0, width: 2, address: Some(group) })),
        pop(Register::DS),
        push(reg(Register::DS)),
        pop(Register::ES),
        semantics(Operation::Nothing, "cld", vec![], vec![]),
    ];
    let leave = vec![
        pop(Register::GS),
        pop(Register::FS),
        pop(Register::ES),
        pop(Register::DS),
        semantics(Operation::Nothing, "popad", vec![], vec![]),
    ];
    (enter, leave)
}

/// Bytes emitted before each RETURN but absent from allocated LIR.
pub fn return_overhead_bytes(procedure: &Procedure) -> Result<usize, Unprintable> {
    let (_enter, leave) = _frame_parts(procedure);
    let emitted: Vec<Option<select::Emitted>> =
        leave.iter().map(|one| select::emit_in(procedure.body.bits, one, 0, None, false, false, None)).collect();
    if emitted.iter().any(Option::is_none) {
        return Err(Unprintable(format!("{}: implicit return sequence is not encodable", procedure.name)));
    }
    Ok(emitted.iter().flatten().map(|one| one.code.len()).sum())
}

/// The procedure as emitted, frame included: what this prints and objbuild
/// encodes. A branch's target is still a block; `label(number, at)` names it.
pub fn listing(procedure: &Procedure, number: usize) -> Result<Vec<Item>, Unprintable> {
    let items = match stack_addressed(procedure, number) {
        Some(items) => items,
        None => built(procedure, number, false)?,
    };
    Ok(items.into_iter().map(|item| spelled(item, &procedure.registers)).collect())
}

/// The procedure's items with the frame register LIR names BP, or not where `omit`.
fn built(procedure: &Procedure, number: usize, omit: bool) -> Result<Vec<Item>, Unprintable> {
    let (mut enter, mut leave) = parts(procedure, omit);
    // Saved where first needed, restored where that path returns: not at the entry and every return.
    let wrap = wrap_of(procedure);
    let (saves, restores): (Vec<Semantics>, Vec<Semantics>) = match &wrap {
        Some(_) => {
            let count = saved_of(procedure).len();
            enter.truncate(enter.len() - count);
            let pops: Vec<Semantics> = leave.drain(..count).collect();
            (saved_of(procedure).iter().map(|one| semantics(Operation::Push, "push", vec![], vec![reg(*one)])).collect(), pops)
        }
        None => (Vec::new(), Vec::new()),
    };
    let blocks = &procedure.body.blocks;
    let lined = || blocks.iter().flat_map(|block| &block.insns).filter(|one| one.line.is_some());
    let first = lined().next();
    let last = lined().filter(|one| one.what.as_ref().is_none_or(|what| what.op != Operation::Return)).last();
    // The prologue is the first line's, not the previous procedure's last.
    let mut line = first.and_then(|one| one.line);
    let mut lines = 0..;
    let mut marked = |line: u32| Item::Mark(Mark::Line { line, index: lines.next().expect("unbounded") });
    let mut out: Vec<Item> = line.map(&mut marked).into_iter().chain(enter.into_iter().map(Item::Semantics)).collect();
    for (index, block) in blocks.iter().enumerate() {
        out.push(Item::Label(Label { name: label(number, block.at) }));
        if wrap.as_ref().is_some_and(|wrap| wrap.at == block.at) {
            out.extend(saves.iter().cloned().map(Item::Semantics));
        }
        let following = if index + 1 < blocks.len() { Some(blocks[index + 1].at) } else { None };
        let fallthrough = _fallthrough_jump(block, following);
        for one in &block.insns {
            if first.is_some_and(|first| Arc::ptr_eq(first, one)) {
                out.push(Item::Mark(Mark::BodyStart));
            }
            if fallthrough.is_some_and(|jump| Arc::ptr_eq(jump, one)) {
                if last.is_some_and(|last| Arc::ptr_eq(last, one)) {
                    out.push(Item::Mark(Mark::BodyEnd));
                }
                continue;
            }
            let Some(what) = &one.what else {
                return Err(Unprintable(format!("{} at {}: an instruction with no semantics", procedure.name, one.at)));
            };
            if one.line.is_some() && one.line != line {
                line = one.line;
                out.extend(line.map(&mut marked));
            }
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
                        // Said for call frame information, which `-g` writes: no code of its own.
                        if callee.pops != 0 && callee.code.is_empty() && first.is_some() {
                            out.push(Item::Mark(Mark::Pops(callee.pops)));
                        }
                    }
                }
                Operation::Return => {
                    if wrap.as_ref().is_some_and(|wrap| wrap.restored.contains(&block.at)) {
                        out.extend(restores.iter().cloned().map(Item::Semantics));
                    }
                    out.extend(leave.iter().cloned().map(Item::Semantics));
                    if let Some(Loc::Imm(popped)) = what.sources.first().filter(|_| procedure.interrupt.is_none() && !procedure.far) {
                        if popped.value > llrm_x86::calling::RET_POPS_MOST {
                            out.extend(moved_return(popped.value, procedure.registers.slot, procedure.registers.spelled(Register::SP)).into_iter().map(Item::Semantics));
                            out.push(Item::Semantics(Semantics { name: Some("ret".to_owned()), sources: vec![], ..what.clone() }));
                            continue;
                        }
                    }
                    let name = match what.name.as_deref() {
                        _ if procedure.interrupt.is_some() => "iret".to_owned(),
                        Some(name) if !name.is_empty() => name.to_owned(),
                        _ => (if procedure.far { "retf" } else { "ret" }).to_owned(),
                    };
                    out.push(Item::Semantics(Semantics { name: Some(name), ..what.clone() }));
                }
                _ => out.push(Item::Semantics(what.clone())),
            }
            if last.is_some_and(|last| Arc::ptr_eq(last, one)) {
                out.push(Item::Mark(Mark::BodyEnd));
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
    // Last, where nothing falls into it: the call that does not return.
    if let Some(check) = &procedure.stack_check {
        out.push(Item::Label(Label { name: label(number, cold_at(&procedure.body)) }));
        out.push(Item::Callee(Callee::new(check.handler.clone(), check.far)));
    }
    Ok(out)
}

/// What removes `bytes` of arguments (`address` bytes of return address on top) where `ret imm16` cannot: the return address is
/// popped into the last slot of the area, the stack raised to it, and a plain `ret` follows. LLVM's `pop ecx; add esp, n; push
/// ecx; ret` (X86ExpandPseudo, the RET pseudo) holds the address in ECX, which Open Watcom's register convention keeps for its
/// caller; the stack alone is as long and clobbers nothing.
fn moved_return(bytes: i64, address: i64, stack: Register) -> Vec<Semantics> {
    let up = bytes - address;
    let above = ir::Mem { through: stack, offset: up, disp_width: 4, ..ir::Mem::new(None, address as u32) };
    let sp = Loc::Reg(ir::Reg { register: stack, width: address as u32 });
    let raise = Loc::Imm(ir::Imm { value: up, width: 4, address: None });
    vec![semantics(Operation::Pop, "pop", vec![Loc::Mem(above)], vec![]), semantics(Operation::Binary, "add", vec![sp.clone()], vec![sp, raise])]
}

/// The items of a procedure that needs no frame register, its cells addressed through the stack
/// pointer: gcc's `-fomit-frame-pointer`, as the target's `frame_optional` allows. A cell the frame
/// register held at `[bp+d]` is at `[sp+d+depth-slot]`, `depth` the bytes pushed since the entry, which
/// each instruction's stack effect moves and every way into a block must agree on. Where anything
/// puts that out of reach (a stack pointer written another way, inline code, an indirect call whose
/// callee may pop, a frame register read as a value) the frame stays.
fn stack_addressed(procedure: &Procedure, number: usize) -> Option<Vec<Item>> {
    let registers = &procedure.registers;
    if !registers.optional
        || procedure.far
        || procedure.interrupt.is_some()
        || procedure.entry != 0
        || procedure.body.bits != 32
        || (!procedure.body.variables.is_empty() && !procedure.body.cfa_variables)
        || procedure.callees.values().any(|one| !one.code.is_empty())
    {
        return None;
    }
    let slot = registers.slot;
    let items = built(procedure, number, true).ok()?;
    let mut out = Vec::with_capacity(items.len());
    // The depth at each label, from whichever edge reached it first.
    let mut known: IndexMap<String, i64> = IndexMap::default();
    let mut depth: Option<i64> = Some(0);
    let name_of = |target: i64| label(number, target);
    for item in items {
        match item {
            Item::Label(label) => {
                match (depth, known.get(&label.name).copied()) {
                    (Some(here), Some(there)) if here != there => return refused(procedure, &format!("label {} reached at depth {here} and {there}", label.name)),
                    (None, Some(there)) => depth = Some(there),
                    (None, None) => return refused(procedure, &format!("label {} follows a jump and none reaches it", label.name)),
                    _ => {}
                }
                known.insert(label.name.clone(), depth?);
                out.push(Item::Label(label));
            }
            Item::Callee(callee) => {
                depth = Some(depth? - callee.pops);
                out.push(Item::Callee(callee));
            }
            Item::Mark(mark) => out.push(Item::Mark(mark)),
            Item::Semantics(mut what) => {
                let here = depth?;
                for place in what.dests.iter_mut().chain(what.sources.iter_mut()) {
                    *place = match through_stack(place, here, slot, registers.pointer) {
                        Some(placed) => placed,
                        None => return refused(procedure, &format!("the frame register is read as a value in {place:?}")),
                    };
                }
                match what.op {
                    Operation::Push | Operation::Pop => {
                        // A push or pop moves the stack by the width it names: a 16-bit `pop cx` takes back two bytes.
                        let (places, sign) = if what.op == Operation::Push { (&what.sources, 1) } else { (&what.dests, -1) };
                        let [place] = &places[..] else { return refused(procedure, &format!("{what:?}")) };
                        if what.op == Operation::Pop && !matches!(place, Loc::Reg(_)) {
                            return refused(procedure, &format!("{what:?}"));
                        }
                        match width_of(place) {
                            Some(bytes @ (2 | 4)) => depth = Some(here + sign * bytes),
                            _ => return refused(procedure, &format!("{what:?}")),
                        }
                    }
                    Operation::Leave | Operation::Exchange | Operation::Escape | Operation::Barrier | Operation::Call => return refused(procedure, &format!("{what:?}")),
                    // `pushf` and its kind move the stack and carry no operand to read the amount from.
                    Operation::Nothing if what.name.as_deref().is_some_and(|name| name.starts_with("push") || name.starts_with("pop")) => return refused(procedure, &format!("{what:?}")),
                    Operation::Return => {
                        if here != 0 {
                            return refused(procedure, &format!("a return at depth {here}"));
                        }
                        depth = None;
                    }
                    Operation::Jump | Operation::Branch => {
                        if what.indirect {
                            return refused(procedure, "an indirect jump");
                        }
                        let target = name_of(what.target?);
                        match known.get(&target) {
                            Some(&there) if there != here => return refused(procedure, &format!("a jump to {target} at depth {here} and {there}")),
                            Some(_) => {}
                            None => {
                                known.insert(target, here);
                            }
                        }
                        if what.op == Operation::Jump {
                            depth = None;
                        }
                    }
                    _ if what.dests.iter().any(writes_stack_pointer) => {
                        // Only `add sp, n` and `sub sp, n` are understood.
                        let amount = match (&what.dests[..], &what.sources[..]) {
                            ([Loc::Reg(dest)], [Loc::Reg(source), Loc::Imm(ir::Imm { value, address: None, .. })]) if dest.register == STACK && source.register == STACK => *value,
                            _ => return refused(procedure, &format!("the stack pointer written by {what:?}")),
                        };
                        depth = Some(match what.name.as_deref() {
                            Some("add") => here - amount,
                            Some("sub") => here + amount,
                            _ => return refused(procedure, &format!("the stack pointer written by {what:?}")),
                        });
                    }
                    _ => {}
                }
                out.push(Item::Semantics(what));
            }
        }
    }
    Some(out)
}

/// Why `procedure` keeps its frame register, for `LLRM_DEBUG=frame`.
fn refused<T>(procedure: &Procedure, why: &str) -> Option<T> {
    llrm_support::debug!("frame", "{}: kept: {why}", procedure.name);
    None
}

/// The width of a register or memory operand, where it has one.
fn width_of(place: &Loc) -> Option<i64> {
    match place {
        // A register is as wide as it is: its own `width` is where it was selected.
        Loc::Reg(one) => Some(one.register.size() as i64),
        Loc::Imm(one) => Some(i64::from(one.width)),
        Loc::Mem(one) => Some(i64::from(one.width)),
        _ => None,
    }
}

fn writes_stack_pointer(place: &Loc) -> bool {
    matches!(place, Loc::Reg(one) if one.register == STACK)
}

/// `place` with a frame cell addressed through the stack pointer; none where it reads the frame register
/// as anything but a base. The frame register would have held the entry's stack pointer less `slot`.
fn through_stack(place: &Loc, depth: i64, slot: i64, pointer: Register) -> Option<Loc> {
    let shift = |_disp: i64| depth - slot;
    let is_pointer = |one: Register| one == FRAME || one == pointer;
    // A frame place with no register, or a cell the frame register's own address names: a 32-bit
    // index has no frame space, only `[ebp+index+d]`.
    let based = |through: Register, addr: &Option<Addr>| match addr {
        Some(addr) if addr.space == Space::Frame => through == Register::None || is_pointer(through),
        Some(addr) if addr.space == Space::Literal => is_pointer(through),
        _ => false,
    };
    match place {
        Loc::Reg(one) if is_pointer(one.register) => None,
        Loc::Mem(cell) if is_pointer(cell.index_through) => None,
        Loc::Mem(cell) if based(cell.through, &cell.addr) => {
            let addr = cell.addr.as_ref()?;
            Some(Loc::Mem(ir::Mem { through: STACK, addr: Some(Addr { disp: addr.disp + shift(addr.disp), ..addr.clone() }), disp_width: 0, ..cell.clone() }))
        }
        Loc::Mem(cell) if is_pointer(cell.through) => None,
        Loc::Address(address) if is_pointer(address.index) => None,
        Loc::Address(address) if based(address.through, &address.addr) => {
            let addr = address.addr.as_ref()?;
            Some(Loc::Address(ir::Address { through: STACK, addr: Some(Addr { disp: addr.disp + shift(addr.disp), ..addr.clone() }), disp_width: 0, ..address.clone() }))
        }
        Loc::Address(address) if is_pointer(address.through) => None,
        other => Some(other.clone()),
    }
}

/// `item` with the frame register and stack pointer LIR calls BP and SP as the
/// target has them, which is BP and SP where it is real mode.
fn spelled(item: Item, registers: &llrm_target::FrameRegisters) -> Item {
    let register = |one: Register| registers.spelled(one);
    // A frame place with no register is addressed through the frame register.
    let framed = |through: Register, addr: Option<Addr>, index: Register| {
        if through == Register::None && index == Register::None && addr.is_some_and(|addr| addr.space == Space::Frame) { registers.pointer } else { register(through) }
    };
    let place = |loc: &Loc| match loc {
        Loc::Reg(one) if register(one.register) != one.register => {
            let spelled = register(one.register);
            Loc::Reg(ir::Reg { register: spelled, width: spelled.size() as u32 })
        }
        Loc::Mem(cell) => Loc::Mem(ir::Mem { through: framed(cell.through, cell.addr, cell.index_through), index_through: register(cell.index_through), ..cell.clone() }),
        Loc::Address(address) => Loc::Address(ir::Address { through: framed(address.through, address.addr, address.index), index: register(address.index), ..address.clone() }),
        other => other.clone(),
    };
    match item {
        Item::Semantics(what) => Item::Semantics(Semantics { dests: what.dests.iter().map(place).collect(), sources: what.sources.iter().map(place).collect(), ..what }),
        other => other,
    }
}

/// The explicit edge that physical adjacency makes free, if there is one.
///
/// Frontends may keep every CFG edge explicit through allocation. Listing is
/// where block order becomes physical, and therefore the first common layer
/// that can say an unconditional edge reaches the instruction already next.
/// Whether an instruction prints: a NOTHING anchor owns a source location
/// but emits no bytes.
pub fn prints(one: &lir::Insn) -> bool {
    one.what.as_ref().is_none_or(|what| {
        what.op != Operation::Nothing || !matches!(what.name.as_deref().unwrap_or(""), "" | "nop")
    })
}

/// NOTHING anchors after the edge own source locations but emit no bytes, so
/// the last instruction that actually prints is the one that matters.
pub fn _fallthrough_jump(block: &lir::LirBlock, following: Option<i64>) -> Option<&Arc<lir::Insn>> {
    let following = following?;
    if block.succ != [following] {
        return None;
    }
    let last = block.insns.iter().rev().find(|one| prints(one))?;
    let what = last.what.as_ref()?;
    if what.op != Operation::Jump || what.target != Some(following) {
        return None;
    }
    Some(last)
}

/// The procedure numbered `number`'s `index`th line mark's symbol.
pub fn line_label(number: usize, index: u32) -> String {
    format!("L{number}_line{index}")
}

/// The procedure numbered `number`'s lines as its listing marks them: each
/// one's symbol and source line, in order.
pub fn line_starts(procedure: &Procedure, number: usize) -> Result<Vec<(String, u32)>, Unprintable> {
    Ok(listing(procedure, number)?
        .into_iter()
        .filter_map(|item| match item {
            Item::Mark(Mark::Line { line, index }) => Some((line_label(number, index), line)),
            _ => None,
        })
        .collect())
}

pub fn label(number: usize, at: i64) -> String {
    format!("L{number}_{at}")
}

pub fn _procedure(
    procedure: &Procedure,
    names: &IndexMap<(Space, i64), String>,
    number: usize,
) -> Result<Vec<String>, Unprintable> {
    _procedure_of(procedure, listing(procedure, number)?, names, number)
}

/// The procedure's text, of the `items` a listing gives.
pub fn _procedure_of(procedure: &Procedure, items: Vec<Item>, names: &IndexMap<(Space, i64), String>, number: usize) -> Result<Vec<String>, Unprintable> {
    let mut out = vec![format!("{} proc {}", procedure.name, if procedure.far { "far" } else { "near" })];
    for item in items {
        match item {
            Item::Label(Label { name }) => out.push(format!("{name}:")),
            Item::Mark(_) => {}
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
    // Both edges of a branch may meet at one block (two cases of a switch emptied to the same place): it falls there.
    rest.dedup();
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
        // A count (and so REP) is the extra result, whatever segments the target names.
        return Ok(vec![format!("{}{name}", if what.dests.len() == 3 { "rep " } else { "" })]);
    }
    if what.op == Operation::Copy {
        // Its operands are the registers the instruction names in its opcode;
        // a source read through another segment than ds says so.
        let counted = what.dests.len() == 4;
        let rep = if counted { "rep " } else { "" };
        let size = match name {
            "movsb" => "byte",
            "movsw" => "word",
            _ => "dword",
        };
        let segmented = what.sources.len() == if counted { 5 } else { 4 };
        return Ok(vec![match what.sources.get(what.sources.len().wrapping_sub(2)).filter(|_| segmented) {
            Some(Loc::Reg(one)) if one.register != Register::DS => {
                format!("{rep}movs {size} ptr es:[di], {size} ptr {}:[si]", format!("{:?}", one.register).to_lowercase())
            }
            _ => format!("{rep}{name}"),
        }]);
    }
    let dests = what.dests.iter().map(|x| _operand(x, names)).collect::<Result<Vec<_>, _>>()?;
    let sources = what.sources.iter().map(|x| _operand(x, names)).collect::<Result<Vec<_>, _>>()?;
    let last = |items: &[String]| items[items.len() - 1].clone();
    Ok(match what.op {
        Operation::Nothing => {
            if matches!(name, "" | "nop") {
                vec![]
            } else if name == "enter" {
                vec![format!("enter {}, {}", sources[0], sources[1])]
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
            vec![format!("{name} {}", dests.iter().chain(&sources).cloned().collect::<Vec<_>>().join(", "))]
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
        // `retf n` removes the arguments too.
        Operation::Return if !sources.is_empty() => vec![format!("{name} {}", sources[0])],
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
        Loc::Address(ir::Address { addr: Some(address), index: Register::None, through, .. }) => {
            let text = _memory(&ir::Mem { through: *through, ..ir::Mem::new(Some(*address), 2) }, names)?;
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
            let segment = if address.segment == Register::None {
                String::new()
            } else {
                format!("{}:", target::name_of(address.segment))
            };
            return Ok(format!("{size}{segment}{symbol}{disp}{indexed}"));
        }
        Space::Literal if !registers.is_empty() => {
            let segment = if crate::backend::select::overriding(cell.through, address.segment) == Register::None {
                String::new()
            } else {
                format!("{}:", target::name_of(address.segment))
            };
            return Ok(format!("{size}{segment}[{registers}{disp}]"));
        }
        // A direct address, as `[disp16]`: the offset is unsigned.
        Space::Literal if registers.is_empty() && address.segment == Register::None => {
            return Ok(format!("{size}[{}]", address.disp & 0xFFFF));
        }
        Space::Far if address.segment != Register::None => {
            let inside = if registers.is_empty() { address.disp.to_string() } else { format!("{registers}{disp}") };
            return Ok(format!("{size}{}:[{inside}]", target::name_of(address.segment)));
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

    fn no_names() -> IndexMap<(Space, i64), String> {
        IndexMap::default()
    }

    fn insn(at: i64, what: Semantics) -> Arc<lir::Insn> {
        Arc::new(lir::Insn::new(at, Some((at, 1)), Some(what), vec![], vec![]))
    }

    fn ax() -> Loc {
        Loc::Reg(ir::Reg { register: Register::AX, width: 2 })
    }

    /// A string move prints as the instruction it is: `rep` where it has a count.
    #[test]
    fn test_a_string_move_prints_with_rep_where_it_repeats() {
        let held = |value| Loc::Held(crate::model::ir::Held { value, width: 2 });
        // A count is the extra result; the segments, where the target has them, the last two sources.
        let results = |count: usize| (0..count).map(|one| held(10 + one as u32)).collect::<Vec<_>>();
        let semantics = |dests: usize, sources: Vec<Loc>| Semantics { name: Some("movsw".to_owned()), dests: results(dests), sources, ..Semantics::new(Operation::Copy) };
        let repeated = semantics(4, vec![held(1), held(2), held(3), held(4), held(5)]);
        let single = semantics(3, vec![held(2), held(3), held(4), held(5)]);
        assert_eq!(_instruction(&repeated, &no_names(), 0).unwrap(), ["rep movsw"]);
        assert_eq!(_instruction(&single, &no_names(), 0).unwrap(), ["movsw"]);
        // A target with no segment registers names none: its operands are the registers.
        assert_eq!(_instruction(&semantics(4, vec![held(1), held(2), held(3)]), &no_names(), 0).unwrap(), ["rep movsw"]);
        assert_eq!(_instruction(&semantics(3, vec![held(2), held(3)]), &no_names(), 0).unwrap(), ["movsw"]);
        let ss = |name: &str, sources: Vec<Loc>| Semantics { name: Some(name.to_owned()), dests: results(4), sources, ..Semantics::new(Operation::Copy) };
        let segment = |register| Loc::Reg(ir::Reg { register, width: 2 });
        let through = ss("movsd", vec![held(1), held(2), held(3), segment(Register::SS), segment(Register::ES)]);
        assert_eq!(_instruction(&through, &no_names(), 0).unwrap(), ["rep movs dword ptr es:[di], dword ptr ss:[si]"]);
    }

    /// `lea bx,[bp-20]` for `&n` at bp-10: lower carries the displacement in
    /// both addr and offset, and the printer added them. asset_seek wrote n
    /// ten bytes below where qglsurf then read it.
    #[test]
    fn test_frame_address_displacement_once() {
        let placed = Loc::Address(ir::Address {
            through: Register::BP,
            offset: -10,
            disp_width: 1,
            ..ir::Address::new(Some(Addr::new(Space::Frame, -10)))
        });
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

    /// What the interrupt entry saves, lowest slot first: each push's register
    /// name, PUSHAD's eight dwords (EDI lowest) in its place.
    fn saved_by_the_entry() -> Vec<(String, i64)> {
        let (enter, _) = _interrupt_parts(Addr::new(Space::Group, 0));
        let mut pushed = Vec::new();
        for one in &enter[..INTERRUPT_SAVED] {
            match (one.op, one.name.as_deref(), one.sources.first()) {
                (_, Some("pushad"), _) => pushed.push(vec!["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"].into_iter().map(|name| (name.to_owned(), 4)).collect::<Vec<_>>()),
                (Operation::Push, _, Some(Loc::Reg(register))) => pushed.push(vec![(format!("{:?}", register.register).to_lowercase(), 2)]),
                other => panic!("the entry saves with {other:?}"),
            }
        }
        pushed.into_iter().rev().flat_map(|group| group.into_iter().rev()).collect()
    }

    /// The handler pushed AX, BX, CX, DX, SI, DI and BP in BCC's nine words and
    /// again in PUSHAD: 14 bytes of code, 14 of stack and 14-35 cycles per
    /// interrupt, saved twice for a frame nothing else read.
    #[test]
    fn test_an_interrupt_handler_saves_each_register_once() {
        let mut names: Vec<String> = saved_by_the_entry().into_iter().map(|(name, size)| if size == 4 { name[1..].to_owned() } else { name }).collect();
        names.sort();
        let unique: BTreeSet<_> = names.iter().cloned().collect();
        assert_eq!(names.len(), unique.len(), "saved twice: {names:?}");
        assert_eq!(names, ["bp", "bx", "cx", "di", "ds", "dx", "es", "fs", "gs", "ax", "si", "sp"].iter().map(|one| one.to_string()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>());
    }

    /// The frame the target's calling.toml states (`interrupt_frame`) is the one the entry builds, so
    /// a handler's parameters read the registers they name.
    #[test]
    fn test_the_entry_builds_the_frame_mir_states() {
        let calling = llrm_target::Target::calling(&llrm_x86_m16::M16);
        let stated = calling.interrupt().expect("an interrupt convention").interrupt_frame.clone();
        assert_eq!(saved_by_the_entry()[..], stated[..12]);
        assert_eq!(interrupt_parameters(), 2);
        assert_eq!(llrm_x86::calling::interrupt_slot(calling.interrupt().unwrap(), "ax"), Some(36));
        assert_eq!(llrm_x86::calling::interrupt_slot(calling.interrupt().unwrap(), "ds"), Some(6));
    }

    fn _printed(sources: Vec<Loc>, reserve: i64) -> Vec<String> {
        _printed_entering(sources, reserve, false)
    }

    fn _printed_entering(sources: Vec<Loc>, reserve: i64, enter: bool) -> Vec<String> {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], sources));
        let leave = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, leave])];
        let body = lir::LirBody::new("get", 1, blocks, IndexMap::default(), IndexMap::default());
        let procedure =
            Procedure { name: "_get".into(), public: true, far: true, body, reserve, callees: IndexMap::default(), interrupt: None, size: enter, entry: 0, stack_check: None, registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16) };
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

    /// Tuned for size, a frame with locals opened with `enter N,0` (4 bytes against 6, but 14 clocks against 3
    /// on the 486, 407 QCport functions): no level opens one.
    #[test]
    fn test_no_level_opens_a_frame_with_enter() {
        for size in [true, false] {
            let entered = _printed_entering(vec![through_bp()], 4, size);
            assert_eq!(entered[1..4], ["push bp", "mov bp, sp", "sub sp, 4"], "size {size}: {entered:?}");
        }
        let bare = _printed_entering(vec![through_bp()], 0, true);
        assert_eq!(bare[1..3], ["push bp", "mov bp, sp"]);
    }

    fn _checked(check: Option<StackCheck>, reserve: i64) -> (Vec<String>, Vec<(String, String)>) {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let leave = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let body = lir::LirBody::new("get", 1, vec![lir::LirBlock::new(1, vec![r#move, leave])], IndexMap::default(), IndexMap::default());
        let procedure = Procedure { name: "_get".into(), public: true, far: true, body, reserve, callees: IndexMap::default(), interrupt: None, size: true, entry: 0, stack_check: check, registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16) };
        let mut names = no_names();
        let externs = stack_externs(std::slice::from_ref(&procedure), &mut names);
        (_procedure(&procedure, &names, 0).unwrap().iter().map(|line| line.trim().to_owned()).collect(), externs)
    }

    /// `-fsanitize=stack` compares SP with the word and calls the routine the runtime's description
    /// names, here made-up ones: a pass that wrote `b$pendchk` itself would not follow them. The
    /// call is last, where no block falls into it, and the default has neither.
    #[test]
    fn test_a_stack_check_names_what_the_runtime_states() {
        let check = StackCheck { limit: "FOO".into(), handler: "BAR".into(), far: true, red_zone: 0, entry: None };
        let (lines, externs) = _checked(Some(check), 4);
        assert_eq!(lines[1..7], ["push bp", "mov bp, sp", "sub sp, 4", "jb L0_2", "cmp sp, word ptr FOO", "jb L0_2"], "{lines:?}");
        assert_eq!(lines[lines.len() - 3..], ["L0_2:", "call far ptr BAR", "_get endp"]);
        assert_eq!(externs, [("FOO".to_owned(), "byte".to_owned()), ("BAR".to_owned(), "far".to_owned())]);
        let (plain, none) = _checked(None, 4);
        assert!(plain.iter().all(|one| !one.contains("cmp sp") && !one.contains("BAR")) && none.is_empty(), "{plain:?}");
        assert_eq!(plain[1..4], ["push bp", "mov bp, sp", "sub sp, 4"]);
    }

    /// Return-tail layout must count the pop/leave absent from allocated LIR.
    #[test]
    fn test_return_overhead_prices_the_implicit_frame_teardown() {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let returned = insn(1, semantics(Operation::Return, "retf", vec![], vec![]));
        let blocks = vec![lir::LirBlock::new(1, vec![r#move, returned])];
        let body = lir::LirBody::new("get", 1, blocks, IndexMap::default(), IndexMap::default());
        let procedure = |reserve| Procedure {
            name: "_get".into(),
            public: true,
            far: true,
            body: body.clone(),
            reserve,
            callees: IndexMap::default(),
            interrupt: None,
            size: false,
            entry: 0,
            stack_check: None,
            registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16),
        };

        assert_eq!(return_overhead_bytes(&procedure(0)).unwrap(), 1);
        assert_eq!(return_overhead_bytes(&procedure(4)).unwrap(), 1);
    }

    /// `ret imm16` pops at most 64 KB less one: a flat callee that pops more raises the stack over its arguments with the return
    /// address moved to the top of them (pr20621-1: a 64 KB struct by value), not `ret 65540`.
    #[test]
    fn test_a_callee_that_pops_64_kb_moves_its_return_address_over_the_arguments() {
        let popping = |bytes: i64| {
            let returned = insn(1, semantics(Operation::Return, "ret", vec![], vec![]));
            let body = lir::LirBody::new("get", 1, vec![lir::LirBlock::new(1, vec![returned])], IndexMap::default(), IndexMap::default());
            let body = cleaned_returns(&body, bytes, 4).unwrap();
            let procedure = Procedure { name: "_get".into(), public: true, far: false, body, reserve: 0, callees: IndexMap::default(), interrupt: None, size: false, entry: 0, stack_check: None, registers: llrm_target::Target::frame_registers(&llrm_x86_m32::M32) };
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect::<Vec<_>>()
        };
        assert!(popping(65535).iter().any(|line| line == "ret 65535"), "{:?}", popping(65535));
        let lines = popping(65540);
        assert!(lines.windows(3).any(|three| three == ["pop dword ptr [esp+65536]", "add esp, 65536", "ret"]), "{lines:?}");
        assert!(cleaned_returns(&lir::LirBody::new("get", 1, Vec::new(), IndexMap::default(), IndexMap::default()), 65540, 2).is_err());
    }

    /// The listing opened `.model medium` whatever the target: a flat program's header is its target's.
    #[test]
    fn test_a_listing_opens_with_its_targets_header() {
        let module = Module {
            object: llrm_target::object::ObjectFormat { formats: vec![llrm_target::object::Format::Omf], default: llrm_target::object::Format::Omf, bitness: 32, header: vec![".386".to_owned(), ".model flat".to_owned()] },
            code: "T_TEXT".into(),
            names: no_names(),
            externs: Vec::new(),
            publics: Vec::new(),
            data: Vec::new(),
            procedures: Vec::new(),
            private: BTreeSet::new(),
            far_bss: BTreeSet::new(),
            requests: BTreeSet::new(),
            debug: None,
            stack: 0,
        };
        assert!(text(&module).unwrap().starts_with(".386\n.model flat\n\n"));
    }

    /// The frame register and stack pointer are LIR's BP and SP whatever the
    /// target: a flat one listed `push bp; mov bp,sp` and `[bp+6]`, 16-bit code in a
    /// 32-bit program.
    #[test]
    fn test_a_flat_frame_is_spelled_with_its_own_registers() {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let leave = insn(1, semantics(Operation::Return, "ret", vec![], vec![]));
        let body = lir::LirBody::new("get", 1, vec![lir::LirBlock::new(1, vec![r#move, leave])], IndexMap::default(), IndexMap::default());
        let registers = llrm_target::FrameRegisters { pointer: Register::EBP, stack: Register::ESP, saved: Vec::new(), slot: 4, optional: false, enter: false };
        let procedure = Procedure { name: "_get".into(), public: true, far: false, body, reserve: 4, callees: IndexMap::default(), interrupt: None, size: false, entry: 0, stack_check: None, registers };
        let lines: Vec<String> = _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        assert!(has(&lines, "push ebp") && has(&lines, "mov ebp, esp") && has(&lines, "sub esp, 4"), "{lines:?}");
        assert!(lines.iter().any(|one| one.contains("[ebp+6]")), "{lines:?}");
        assert!(!lines.iter().any(|one| one.contains("[bp") || one.contains(" bp") || one.contains(" sp")), "{lines:?}");
    }

    /// `enter` is the target's to take for size (`frame_enter`): a target that states it gets the 4-byte frame at -Os
    /// only, where the 486's own description (false) gets push/mov/sub at both levels.
    #[test]
    fn test_a_target_that_states_frame_enter_opens_a_size_frame_with_it() {
        let body = lir::LirBody { bits: 32, ..lir::LirBody::new("get", 1, vec![lir::LirBlock::new(1, vec![ret()])], IndexMap::default(), IndexMap::default()) };
        let listing = |enter: bool, size: bool| {
            let registers = llrm_target::FrameRegisters { pointer: Register::EBP, stack: Register::ESP, saved: Vec::new(), slot: 4, optional: false, enter };
            let procedure = Procedure { name: "_get".into(), public: true, far: false, body: body.clone(), reserve: 4, callees: IndexMap::default(), interrupt: None, size, entry: 0, stack_check: None, registers };
            _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect::<Vec<_>>()
        };
        assert_eq!(listing(true, true)[1], "enter 4, 0");
        assert_eq!(listing(true, false)[1..4], ["push ebp", "mov ebp, esp", "sub esp, 4"]);
        assert_eq!(listing(false, true)[1..4], ["push ebp", "mov ebp, esp", "sub esp, 4"]);
    }

    /// A flat procedure whose frame register the target may leave out, `body` its instructions.
    fn flat(optional: bool, reserve: i64, body: Vec<Arc<lir::Insn>>, callees: IndexMap<i64, Callee>) -> Vec<String> {
        let body = lir::LirBody { bits: 32, ..lir::LirBody::new("get", 1, vec![lir::LirBlock::new(1, body)], IndexMap::default(), IndexMap::default()) };
        let registers = llrm_target::FrameRegisters { pointer: Register::EBP, stack: Register::ESP, saved: Vec::new(), slot: 4, optional, enter: false };
        let procedure = Procedure { name: "_get".into(), public: true, far: false, body, reserve, callees, interrupt: None, size: false, entry: 0, stack_check: None, registers };
        _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect()
    }

    fn eax() -> Loc {
        Loc::Reg(ir::Reg { register: Register::EAX, width: 4 })
    }

    /// A cell of the frame, `disp` from the frame register.
    fn cell(disp: i64) -> Loc {
        Loc::Mem(ir::Mem { through: Register::BP, ..ir::Mem::new(Some(Addr::new(Space::Frame, disp)), 4) })
    }

    fn ret() -> Arc<lir::Insn> {
        insn(9, semantics(Operation::Return, "ret", vec![], vec![]))
    }

    /// bench/fib's `push ebp; mov ebp,esp ... leave` cost three instructions a call. A function whose
    /// cells the stack pointer can name keeps no frame register: an argument, above the return address,
    /// is `[esp + depth + 4]` and a local `[esp + depth - 4 - n]`, `depth` the bytes pushed so far, which
    /// a push moves and a callee's pop moves back.
    #[test]
    fn test_a_frame_the_stack_pointer_can_address_has_no_frame_register() {
        let push = insn(1, semantics(Operation::Push, "push", vec![], vec![eax()]));
        let call = insn(2, semantics(Operation::Call, "call", vec![], vec![]));
        let callees = IndexMap::from_iter([(2, Callee { pops: 4, ..Callee::new("_f", false) })]);
        let body = vec![
            insn(0, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
            push,
            insn(3, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
            insn(4, semantics(Operation::Move, "mov", vec![eax()], vec![cell(-4)])),
            call,
            insn(5, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])),
            ret(),
        ];
        let lines = flat(true, 8, body, callees);
        assert!(!lines.iter().any(|one| one.contains("ebp")), "{lines:?}");
        assert_eq!(lines.iter().filter(|one| one.starts_with("mov eax, dword ptr")).collect::<Vec<_>>(), ["mov eax, dword ptr [esp+16]", "mov eax, dword ptr [esp+20]", "mov eax, dword ptr [esp+8]", "mov eax, dword ptr [esp+16]"], "{lines:?}");
        assert!(has(&lines, "sub esp, 12") && has(&lines, "add esp, 12"), "{lines:?}");
    }

    /// A target that does not let its frame register go, and a body that reads it as a value or calls
    /// through a pointer, keep the frame.
    #[test]
    fn test_a_frame_register_that_is_read_or_may_not_go_stays() {
        let read = || vec![insn(0, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])), ret()];
        assert!(has(&flat(false, 8, read(), IndexMap::default()), "push ebp"));
        let value = vec![insn(0, semantics(Operation::Move, "mov", vec![eax()], vec![Loc::Reg(ir::Reg { register: Register::EBP, width: 4 })])), ret()];
        assert!(has(&flat(true, 8, value, IndexMap::default()), "push ebp"));
        let indirect = vec![insn(0, Semantics { indirect: true, ..semantics(Operation::Call, "call", vec![], vec![eax()]) }), ret()];
        assert!(has(&flat(true, 8, indirect, IndexMap::default()), "push ebp"));
        let flags = vec![insn(0, semantics(Operation::Nothing, "pushf", vec![], vec![])), insn(1, semantics(Operation::Move, "mov", vec![eax()], vec![cell(8)])), ret()];
        assert!(has(&flat(true, 8, flags, IndexMap::default()), "push ebp"));
        assert!(!has(&flat(true, 8, read(), IndexMap::default()), "push ebp"));
    }

    /// The frame's reserve was rounded to a word: a flat target keeps its stack in dwords, and
    /// `sub esp, 70` left the next push misaligned.
    #[test]
    fn test_the_reserve_is_a_multiple_of_the_targets_slot() {
        let r#move = insn(0, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()]));
        let leave = insn(1, semantics(Operation::Return, "ret", vec![], vec![]));
        let body = lir::LirBody::new("get", 1, vec![lir::LirBlock::new(1, vec![r#move, leave])], IndexMap::default(), IndexMap::default());
        let registers = llrm_target::FrameRegisters { pointer: Register::EBP, stack: Register::ESP, saved: Vec::new(), slot: 4, optional: false, enter: false };
        let procedure = Procedure { name: "_get".into(), public: true, far: false, body, reserve: 70, callees: IndexMap::default(), interrupt: None, size: false, entry: 0, stack_check: None, registers };
        let lines: Vec<String> = _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        assert!(has(&lines, "sub esp, 72"), "{lines:?}");
    }

    /// SI and DI were pushed and popped whole: an operand-size prefix on every save
    /// and restore, for upper halves no Borland caller keeps across a call.
    #[test]
    fn test_callee_saves_only_what_the_convention_keeps() {
        let lines = _printed(vec![Loc::Reg(ir::Reg { register: Register::ESI, width: 4 })], 0);
        assert!(has(&lines, "push si") && has(&lines, "pop si"));
        assert!(!has(&lines, "push esi") && !has(&lines, "pop esi"));
    }

    /// `place` saved SI on every call, entered or not, though only its loop names it: the early
    /// `return 1` pushed and popped it for nothing. The save is where the loop starts.
    #[test]
    fn test_a_register_only_a_later_block_names_is_saved_there() {
        let branch = Semantics { target: Some(3), ..semantics(Operation::Branch, "je", vec![], vec![]) };
        let si = || Loc::Reg(ir::Reg { register: Register::SI, width: 2 });
        let blocks = vec![
            lir::LirBlock { succ: vec![3, 2], ..lir::LirBlock::new(1, vec![insn(1, semantics(Operation::Move, "mov", vec![ax()], vec![through_bp()])), insn(2, branch)]) },
            lir::LirBlock::new(2, vec![insn(3, semantics(Operation::Move, "mov", vec![si()], vec![through_bp()])), insn(4, semantics(Operation::Return, "ret", vec![], vec![]))]),
            lir::LirBlock::new(3, vec![insn(5, semantics(Operation::Return, "ret", vec![], vec![]))]),
        ];
        let body = lir::LirBody::new("get", 1, blocks, IndexMap::default(), IndexMap::default());
        let procedure = Procedure { name: "_get".into(), public: true, far: false, body, reserve: 0, callees: IndexMap::default(), interrupt: None, size: false, entry: 0, stack_check: None, registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16) };
        let lines: Vec<String> = _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        let at = |text: &str| lines.iter().position(|one| one == text).unwrap_or_else(|| panic!("{text} in {lines:?}"));
        assert_eq!(lines.iter().filter(|one| *one == "push si").count(), 1, "{lines:?}");
        assert!(at("push si") > at("L0_2:") && at("push si") < at("L0_3:"), "{lines:?}");
        // The return that path makes pops it; the early one has nothing to pop.
        assert_eq!(lines.iter().filter(|one| *one == "pop si").count(), 1, "{lines:?}");
        assert!(at("pop si") < at("L0_3:"), "{lines:?}");
    }

    /// Reached around the block that names it as well, a return cannot pop what was not pushed: the
    /// entry saves, as before.
    #[test]
    fn test_a_return_reached_around_the_save_keeps_the_save_at_the_entry() {
        let branch = Semantics { target: Some(3), ..semantics(Operation::Branch, "je", vec![], vec![]) };
        let si = || Loc::Reg(ir::Reg { register: Register::SI, width: 2 });
        let blocks = vec![
            lir::LirBlock { succ: vec![3, 2], ..lir::LirBlock::new(1, vec![insn(1, branch)]) },
            lir::LirBlock { succ: vec![3], ..lir::LirBlock::new(2, vec![insn(3, semantics(Operation::Move, "mov", vec![si()], vec![through_bp()]))]) },
            lir::LirBlock::new(3, vec![insn(5, semantics(Operation::Return, "ret", vec![], vec![]))]),
        ];
        let body = lir::LirBody::new("get", 1, blocks, IndexMap::default(), IndexMap::default());
        let procedure = Procedure { name: "_get".into(), public: true, far: false, body, reserve: 0, callees: IndexMap::default(), interrupt: None, size: false, entry: 0, stack_check: None, registers: llrm_target::Target::frame_registers(&llrm_x86_m16::M16) };
        let lines: Vec<String> = _procedure(&procedure, &no_names(), 0).unwrap().iter().map(|line| line.trim().to_owned()).collect();
        assert!(lines.iter().position(|one| one == "push si").unwrap() < lines.iter().position(|one| one == "L0_1:").unwrap(), "{lines:?}");
    }

    /// peephole's multiply by three has no address, only base, index and scale.
    #[test]
    fn test_arithmetic_lea_scales_its_index() {
        let r#where = ir::Address { through: Register::EBX, index: Register::EBX, scale: 2, ..ir::Address::new(None) };
        assert_eq!(_operand(&Loc::Address(r#where), &no_names()).unwrap(), "[ebx+ebx*2]");
    }
}
