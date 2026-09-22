//! Port of `qbopt/backend/lower.py`: MIR to machine form.
//!
//! Ported so far: the operand and naming half, up to `lowered`.

use std::fmt;

use iced_x86::Register;
use num_traits::ToPrimitive;

use crate::model::floating::{Format, Rounding};
use crate::model::ir::nodes::Node;
use crate::model::ir::{self, Loc, Operation};
use crate::model::mir::{Arg, Held, Kind, MemRef, MirBody, Op, OpCode};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyrepr::Repr;

/// An operand nothing here can turn into a machine location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unlowered(pub String);

impl fmt::Display for Unlowered {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unlowered {}

/// What `operand` hands back: a machine location, or -- Python's duck
/// typing -- a MIR cell `_located` has yet to encode, or `Opaque(None)`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Placed {
    Loc(Loc),
    Cell(MemRef),
    Nothing,
}

/// `ir.Semantics` before `_located`: its operands may still be cells.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Unlocated {
    pub op: Operation,
    pub name: Option<String>,
    pub dests: Vec<Placed>,
    pub sources: Vec<Placed>,
    pub target: Option<i64>,
    pub indirect: bool,
}

/// Python's `place` argument: `None`, `as_a_value`, or any other resolver.
#[derive(Clone, Copy)]
pub(crate) enum Place<'a> {
    Default,
    AsAValue,
    With(&'a dyn Fn(&Arg, &[Loc], usize) -> Result<Placed, Unlowered>),
}

impl Place<'_> {
    fn call(self, arg: &Arg, had: &[Loc], index: usize) -> Result<Placed, Unlowered> {
        match self {
            Place::Default => Ok(_place(arg, had, index)),
            Place::AsAValue => Ok(as_a_value(arg, had, index)),
            Place::With(place) => place(arg, had, index),
        }
    }
}

fn imm(value: &num_bigint::BigInt, width: u32, address: Option<Addr>) -> Loc {
    Loc::Imm(ir::Imm { value: value.to_i64().expect("an immediate fits an int64"), width, address })
}

/// One MIR operand as the machine's.
pub(crate) fn operand(arg: &Arg) -> Placed {
    match arg {
        Arg::Held(one) => Placed::Loc(Loc::Held(ir::Held { value: one.value.id, width: one.width })),
        Arg::Const(one) => Placed::Loc(imm(&one.n, one.width, None)),
        Arg::Symbol(one) => Placed::Loc(Loc::Imm(ir::Imm {
            value: one.addend,
            width: one.width,
            address: Some(Addr { index: one.index, ..Addr::new(one.space, one.offset) }),
        })),
        Arg::FrameAddress(one) => Placed::Loc(Loc::Address(ir::Address {
            through: Register::BP,
            offset: one.offset,
            disp_width: if (-128..=127).contains(&one.offset) { 1 } else { 2 },
            ..ir::Address::new(Some(Addr::new(Space::Frame, one.offset)))
        })),
        Arg::FrameSelector(one) => Placed::Loc(Loc::Reg(ir::Reg { register: Register::SS, width: one.width })),
        Arg::Cell(one) => Placed::Cell(one.r#ref.clone()),
        // The x87 stack, which has no MIR form.
        Arg::Opaque(one) => one.machine_payload().cloned().map_or(Placed::Nothing, Placed::Loc),
    }
}

/// What the machine calls each operation a pass can invent.
fn _machine_kind(kind: Kind) -> Option<(Operation, &'static str)> {
    Some(match kind {
        Kind::AddCarry => (Operation::Binary, "adc"),
        Kind::SubBorrow => (Operation::Binary, "sbb"),
        Kind::Increment => (Operation::Unary, "inc"),
        Kind::Decrement => (Operation::Unary, "dec"),
        Kind::Copy => (Operation::Move, "mov"),
        Kind::SignExtend => (Operation::Extend, "movsx"),
        Kind::ZeroExtend => (Operation::Extend, "movzx"),
        Kind::Load => (Operation::Move, "mov"),
        Kind::Store => (Operation::Move, "mov"),
        Kind::Jump => (Operation::Jump, "jmp"),
        Kind::Mul => (Operation::Multiply, "imul"),
        Kind::FixedMul => (Operation::Multiply, "fixed_mul"),
        Kind::FixedDiv => (Operation::Divide, "fixed_div"),
        Kind::Add => (Operation::Binary, "add"),
        Kind::Shl => (Operation::Binary, "shl"),
        Kind::Shr => (Operation::Binary, "shr"),
        Kind::Sar => (Operation::Binary, "sar"),
        _ => return None,
    })
}

fn _branches(test: Kind) -> Option<&'static str> {
    Some(match test {
        Kind::Eq => "je",
        Kind::Ne => "jne",
        Kind::Lt => "jl",
        Kind::Le => "jle",
        Kind::Gt => "jg",
        Kind::Ge => "jge",
        Kind::Below => "jb",
        Kind::BelowEq => "jbe",
        Kind::Above => "ja",
        Kind::AboveEq => "jae",
        _ => return None,
    })
}

/// The instruction each kind is, for MIR that says only what it computes.
fn _named(kind: Kind) -> Option<(Operation, &'static str)> {
    _machine_kind(kind).or(Some(match kind {
        Kind::Sub => (Operation::Binary, "sub"),
        Kind::And => (Operation::Binary, "and"),
        Kind::Or => (Operation::Binary, "or"),
        Kind::Xor => (Operation::Binary, "xor"),
        Kind::Neg => (Operation::Unary, "neg"),
        Kind::Not => (Operation::Unary, "not"),
        Kind::Divmod => (Operation::Divide, "idiv"),
        Kind::Udivmod => (Operation::Divide, "div"),
        Kind::Address => (Operation::Address, "lea"),
        Kind::Arg => (Operation::Push, "push"),
        Kind::Concat => (Operation::Move, ""),
        Kind::Branch => (Operation::Branch, ""),
        Kind::Fadd => (Operation::FloatArith, "fadd"),
        Kind::Fsub => (Operation::FloatArith, "fsub"),
        Kind::Fmul => (Operation::FloatArith, "fmul"),
        Kind::Fdiv => (Operation::FloatArith, "fdiv"),
        Kind::Fneg => (Operation::FloatUnary, "fchs"),
        Kind::Fabs => (Operation::FloatUnary, "fabs"),
        Kind::Fsqrt => (Operation::FloatUnary, "fsqrt"),
        // FloatAlloc picks fcom, fcomp or fcompp by what dies.
        Kind::Fcompare => (Operation::Compare, "fcom"),
        Kind::Fcheck => (Operation::Nothing, "fwait"),
        Kind::Call => (Operation::Call, "call"),
        Kind::Return => (Operation::Return, ""),
        _ => return None,
    }))
}

/// An x87 compare's answer reaches the flags through sahf, where an
/// unsigned compare leaves it.
fn _unordered(test: Kind) -> Option<Kind> {
    match test {
        Kind::Lt => Some(Kind::Below),
        Kind::Le => Some(Kind::BelowEq),
        Kind::Gt => Some(Kind::Above),
        Kind::Ge => Some(Kind::AboveEq),
        _ => None,
    }
}

fn _integers(format: Format) -> bool {
    matches!(format, Format::Signed16 | Format::Signed32 | Format::Signed64)
}

/// Where an operation with no node of its own returns values and a call
/// delivers them, by position: every x86 C convention's AX, then DX.
pub(crate) const _RETURNED: [Register; 2] = [Register::EAX, Register::EDX];

fn _instruction(op: &Op) -> Result<Option<(Operation, &'static str)>, Unlowered> {
    if op.kind == Kind::Sub && op.results.is_empty() {
        return Ok(Some((Operation::Compare, "cmp")));
    }
    if let Some(floating) = &op.floating {
        if op.kind == Kind::Fload && floating.inputs[0] == Format::Unsigned64 {
            return Err(Unlowered("unsigned 64-bit floating load needs target-specific expansion".into()));
        }
        if op.kind == Kind::Fstore && floating.result == Format::Unsigned64 {
            return Err(Unlowered("unsigned 64-bit floating store needs target-specific expansion".into()));
        }
        if op.kind == Kind::Fload {
            return Ok(Some((Operation::FloatLoad, if _integers(floating.inputs[0]) { "fild" } else { "fld" })));
        }
        if op.kind == Kind::Fstore {
            if !_integers(floating.result) {
                return Ok(Some((Operation::FloatStore, "fstp")));
            }
            // Toward zero is fisttp, which a 387 lacks: FloatAlloc spells it for one.
            let name = if floating.rounding == Rounding::TowardZero { "fisttp" } else { "fistp" };
            return Ok(Some((Operation::FloatStore, name)));
        }
    }
    Ok(_named(op.kind))
}

/// Every operation with no machine form given one from what it computes.
pub(crate) fn named(body: &MirBody) -> Result<MirBody, Unlowered> {
    let compared: std::collections::BTreeSet<_> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind == Kind::Fcompare)
        .flat_map(|op| op.defines.iter().copied())
        .collect();
    let one = |op: &Op| -> Result<Op, Unlowered> {
        let mut op = op.clone();
        if op.kind == Kind::Branch {
            if let Some(unordered) = op.test.and_then(_unordered) {
                if op.uses.iter().any(|value| compared.contains(value)) {
                    op.test = Some(unordered);
                }
            }
        }
        if !op.name.is_empty() || op.kind == Kind::Nothing {
            return Ok(op);
        }
        let Some((operation, name)) = _instruction(&op)? else {
            return Err(Unlowered(format!("{:#06x}: no instruction for {}", op.at, op.kind.as_str())));
        };
        op.op = Some(OpCode::Operation(operation));
        op.name = name.to_owned();
        Ok(op)
    };
    let mut body = body.clone();
    for block in &mut body.blocks {
        block.ops = block.ops.iter().map(one).collect::<Result<_, _>>()?;
    }
    Ok(body)
}

/// Whether a generated CALL's sole encoded operand is its target.
pub(crate) fn _indirect_call(op: &Op) -> Result<bool, Unlowered> {
    if !op.indirect {
        return Ok(false);
    }
    if op.kind != Kind::Call || op.args.len() != 1 || !matches!(op.args[0], Arg::Held(_)) {
        return Err(Unlowered(format!("{:#06x}: indirect call needs exactly one value target", op.at)));
    }
    Ok(true)
}

fn operation_of(op: &Op) -> Result<Operation, Unlowered> {
    match op.op {
        Some(OpCode::Operation(operation)) => Ok(operation),
        _ => Err(Unlowered(format!("{:#06x}: {:?} is no machine operation", op.at, op.op))),
    }
}

/// What this operation computes, in machine form, or None for verbatim.
pub(crate) fn semantics(op: &Op, was: Option<&ir::Semantics>, place: Place) -> Result<Option<Unlocated>, Unlowered> {
    let same_target = was.is_none_or(|was| op.target == was.target);
    if matches!(place, Place::Default)
        && op.raised.as_ref().is_some_and(|(args, results)| op.args == *args && op.results == *results)
        && same_target
    {
        // Nothing rewrote it, so the MIR emitter carries its own bytes.
        return Ok(None);
    }
    let bare = |op: Operation, name: &str, target: Option<i64>| Unlocated {
        op,
        name: Some(name.to_owned()),
        dests: vec![],
        sources: vec![],
        target,
        indirect: false,
    };
    if op.kind == Kind::Nothing && op.name.is_empty() {
        return Ok(Some(bare(Operation::Nothing, "", None)));
    }
    if op.kind == Kind::Fcheck {
        return Ok(Some(bare(Operation::Nothing, "wait", None)));
    }
    if op.kind == Kind::Jump && op.target.is_some() {
        return Ok(Some(bare(Operation::Jump, "jmp", op.target)));
    }
    let dests_of = |was: Option<&ir::Semantics>| was.map_or(&[][..], |was| &was.dests[..]).to_vec();
    let sources_of = |was: Option<&ir::Semantics>| was.map_or(&[][..], |was| &was.sources[..]).to_vec();
    if !op.source_backed
        && matches!(op.kind, Kind::Call | Kind::Return)
        && matches!(op.op, Some(OpCode::Operation(Operation::Call | Operation::Return)))
    {
        // Results arrive and return values leave in registers the ABI says,
        // rather than encoded operands. An indirect call's target is the one
        // exception: it is the r/m16 source named by the call instruction.
        let sources = if _indirect_call(op)? {
            op.args.iter().enumerate().map(|(i, one)| place.call(one, &[], i)).collect::<Result<_, _>>()?
        } else {
            vec![]
        };
        return Ok(Some(Unlocated { sources, indirect: op.indirect, ..bare(operation_of(op)?, &op.name, None) }));
    }
    if op.kind == Kind::Address && op.args.len() == 1 && op.results.len() == 1 {
        if let (Arg::Cell(cell), Arg::Held(result)) = (&op.args[0], &op.results[0]) {
            if let Some(addr) = cell.r#ref.addr.filter(|addr| matches!(addr.space, Space::Segment | Space::External)) {
                if cell.r#ref.base.is_none() && result.width == 2 {
                    // A named data object's near address is its relocated
                    // offset, which MOV imm16 can carry and LEA cannot.
                    let dest = place.call(&op.results[0], &dests_of(was), 0)?;
                    return Ok(Some(Unlocated {
                        dests: vec![dest],
                        sources: vec![Placed::Loc(Loc::Imm(ir::Imm { value: 0, width: 2, address: Some(addr) }))],
                        ..bare(Operation::Move, "mov", None)
                    }));
                }
            }
        }
    }
    if op.args.is_empty() && op.results.is_empty() && op.raised.is_none() && op.kind != Kind::Branch {
        return Ok(None); // nothing to build one from
    }
    let (mut was_op, mut name) = match _machine_kind(op.kind) {
        Some((operation, name)) => (Some(operation), name.to_owned()),
        None => (None, op.name.clone()),
    };
    if op.kind == Kind::Branch && name.is_empty() {
        if let Some(branch) = op.test.and_then(_branches) {
            (was_op, name) = (Some(Operation::Branch), branch.to_owned());
        }
    }
    if op.kind == Kind::Nothing && op.op != Some(OpCode::Operation(Operation::Nothing)) {
        (was_op, name) = (Some(Operation::Nothing), "nop".to_owned());
    }
    let was_op = match was_op {
        Some(operation) => operation,
        None => operation_of(op)?,
    };
    let mut args: Vec<Arg> = op.args.clone();
    if op.kind == Kind::Return {
        // Returned values constrain allocation but RET only encodes stack cleanup.
        args = args
            .into_iter()
            .zip(sources_of(was))
            .filter(|(_, original)| matches!(original, Loc::Imm(_)))
            .map(|(one, _)| one)
            .collect();
    }
    if matches!(place, Place::AsAValue)
        && matches!(op.kind, Kind::Add | Kind::And | Kind::Or | Kind::Xor)
        && args.len() == 2
        && matches!(args[0], Arg::Const(_))
        && matches!(args[1], Arg::Held(_))
    {
        args.swap(0, 1);
    }
    if matches!(place, Place::AsAValue)
        && op.kind == Kind::Mul
        && op.results.len() == 1
        && args.len() == 2
        && matches!(args[0], Arg::Held(_))
        && matches!(args[1], Arg::Const(_))
    {
        args = vec![args[0].clone(), args[0].clone(), args[1].clone()];
    }
    let (had_dests, had_sources) = (dests_of(was), sources_of(was));
    Ok(Some(Unlocated {
        op: was_op,
        name: Some(name),
        dests: op
            .results
            .iter()
            .enumerate()
            .map(|(i, one)| place.call(one, &had_dests, i))
            .collect::<Result<_, _>>()?,
        sources: args.iter().enumerate().map(|(i, one)| place.call(one, &had_sources, i)).collect::<Result<_, _>>()?,
        target: _target(op, was),
        indirect: false,
    }))
}

/// One operand, left as the value it is.
pub(crate) fn as_a_value(arg: &Arg, _had: &[Loc], _index: usize) -> Placed {
    operand(arg)
}

/// `what` with every register the operation names as a value replaced by it.
pub(crate) fn _valueized(what: &ir::Semantics, op: &Op) -> ir::Semantics {
    let named = |side: &[Loc], mine: &[Arg]| -> Vec<Loc> {
        side.iter()
            .enumerate()
            .map(|(index, one)| match (one, mine.get(index)) {
                (Loc::Reg(one), Some(Arg::Held(was))) if !was.value.flags => {
                    Loc::Held(ir::Held { value: was.value.id, width: one.width })
                }
                (Loc::Mem(one), Some(Arg::Cell(was))) => {
                    let base = was.r#ref.base.map(|base| ir::Held { value: base.id, width: was.r#ref.base_width });
                    Loc::Mem(ir::Mem { base, selector: _selector(&was.r#ref), ..one.clone() })
                }
                _ => one.clone(),
            })
            .collect()
    };
    ir::Semantics { dests: named(&what.dests, &op.results), sources: named(&what.sources, &op.args), ..what.clone() }
}

/// One operand, keeping the register the instruction already had.
pub(crate) fn _place(arg: &Arg, had: &[Loc], index: usize) -> Placed {
    let got = operand(arg);
    let Placed::Loc(Loc::Held(held)) = &got else {
        return got;
    };
    match had.get(index) {
        Some(Loc::Reg(was)) if was.width == held.width => Placed::Loc(Loc::Reg(*was)),
        _ => got,
    }
}

/// What a pass made of this operation, in machine form, or None.
pub(crate) fn rewritten(op: &Op, place: Place, node: Option<&Node>) -> Result<Option<ir::Semantics>, Unlowered> {
    let was = node.map(Node::semantics);
    _located(semantics(op, was, place)?, was)
}

/// What this operation computes now, in machine form.
pub(crate) fn current(op: &Op, place: Place, node: Option<&Node>) -> Result<Option<ir::Semantics>, Unlowered> {
    if matches!(place, Place::Default) && op.loads.iter().chain(&op.stores).any(|one| one.pointer) {
        return Ok(None);
    }
    Ok(rewritten(op, place, node)?.or_else(|| node.map(|node| node.semantics().clone())))
}

/// A branch's destination, which is a block address and not an operand.
fn _target(op: &Op, was: Option<&ir::Semantics>) -> Option<i64> {
    op.target.or_else(|| was.and_then(|was| was.target))
}

/// `what` with every MIR operand in it replaced by a machine one.
fn _located(what: Option<Unlocated>, was: Option<&ir::Semantics>) -> Result<Option<ir::Semantics>, Unlowered> {
    let Some(what) = what else {
        return Ok(None);
    };
    let empty = Vec::new();
    let (had_dests, had_sources) = was.map_or((&empty, &empty), |was| (&was.dests, &was.sources));
    let dests = what.dests.iter().enumerate().map(|(i, one)| _machine(one, had_dests, i)).collect::<Result<_, _>>()?;
    let sources =
        what.sources.iter().enumerate().map(|(i, one)| _machine(one, had_sources, i)).collect::<Result<_, _>>()?;
    Ok(Some(ir::Semantics {
        op: what.op,
        name: what.name,
        dests,
        sources,
        target: what.target,
        indirect: what.indirect,
    }))
}

fn _machine(one: &Placed, had: &[Loc], index: usize) -> Result<Loc, Unlowered> {
    let one = match one {
        Placed::Loc(loc) => return Ok(loc.clone()),
        Placed::Nothing => return Err(Unlowered("an opaque operand has no machine location".into())),
        Placed::Cell(one) => one,
    };
    if one.pointer {
        return Err(Unlowered("whole-pointer memory operand escaped pointer materialization".into()));
    }
    let before = match had.get(index) {
        Some(Loc::Mem(mem)) => Some(mem),
        _ => had.iter().find_map(|x| match x {
            Loc::Mem(mem) => Some(mem),
            _ => None,
        }),
    };
    let base = one.base.map(|base| ir::Held { value: base.id, width: 2 });
    let addr = _address(one);
    if base.is_some() {
        // NONE until something places it.
        let made = match before {
            Some(before) => ir::Mem {
                through: Register::None,
                offset: before.offset,
                disp_width: before.disp_width,
                ..ir::Mem::new(addr, one.width)
            },
            None => ir::Mem { through: Register::None, ..(_addressed(one)?) },
        };
        return Ok(Loc::Mem(ir::Mem { base, selector: _selector(one), ..made }));
    }
    if let Some(before) = before {
        return Ok(Loc::Mem(ir::Mem {
            through: before.through,
            offset: before.offset,
            disp_width: before.disp_width,
            selector: _selector(one),
            ..ir::Mem::new(addr, one.width)
        }));
    }
    Ok(Loc::Mem(ir::Mem { selector: _selector(one), ..(_addressed(one)?) }))
}

/// Attach the machine selector implied by an abstract address space.
fn _address(reference: &MemRef) -> Option<Addr> {
    match reference.addr {
        Some(addr) if addr.space == Space::Literal && reference.space == Some(Space::Frame) => {
            Some(Addr { segment: Register::SS, ..addr })
        }
        addr => addr,
    }
}

/// The value a far cell's segment is in, for allocation to place.
pub(crate) fn _selector(reference: &MemRef) -> Option<ir::Held> {
    match (reference.segment, reference.addr) {
        (Some(segment), Some(addr)) if addr.space == Space::Far => Some(ir::Held { value: segment.id, width: 2 }),
        _ => None,
    }
}

/// A cell the original instruction had no memory operand for.
pub(crate) fn _addressed(one: &MemRef) -> Result<ir::Mem, Unlowered> {
    let Some(mut addr) = _address(one) else {
        return Err(Unlowered(
            "a cell with no address cannot be encoded: nothing says which register reaches it".into(),
        ));
    };
    let mem = |addr: Addr, through: Register, offset: i64| ir::Mem {
        through,
        offset,
        disp_width: 2,
        ..ir::Mem::new(Some(addr), one.width)
    };
    if addr.space == Space::Frame {
        return Ok(mem(addr, Register::BP, 0));
    }
    if addr.space == Space::Literal && one.base.is_some() {
        return Ok(mem(addr, Register::None, 0));
    }
    if addr.space == Space::Far && one.base.is_some() && one.segment.is_some() {
        if addr.segment == Register::None {
            // MIR names the selector only as a value, which _abi pins to ES.
            addr.segment = Register::ES;
        }
        return Ok(mem(addr, Register::None, addr.disp));
    }
    if matches!(addr.space, Space::Segment | Space::External) {
        return Ok(mem(addr, addr.base, 0));
    }
    Err(Unlowered(format!("a cell at {} in {} has no encoding this can derive", addr.repr(), addr.space)))
}

fn sem(op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>) -> ir::Semantics {
    ir::Semantics { name: Some(name.to_owned()), dests, sources, ..ir::Semantics::new(op) }
}

fn held(value: u32, width: u32) -> Loc {
    Loc::Held(ir::Held { value, width })
}

fn immediate(value: i64, width: u32) -> Loc {
    Loc::Imm(ir::Imm { value, width, address: None })
}

/// `operand(arg)` where the caller needs a machine location, as every
/// expansion does: Python would carry a cell on into LIR and fail later.
fn located(arg: &Arg) -> Result<Loc, Unlowered> {
    match operand(arg) {
        Placed::Loc(loc) => Ok(loc),
        other => Err(Unlowered(format!("{other:?} is no machine operand"))),
    }
}

/// Python's `arg.width`, which a cell and an opaque operand lack.
fn arg_width(arg: &Arg) -> Option<u32> {
    match arg {
        Arg::Held(one) => Some(one.width),
        Arg::Const(one) => Some(one.width),
        Arg::Symbol(one) => Some(one.width),
        Arg::FrameAddress(one) => Some(one.width),
        Arg::FrameSelector(one) => Some(one.width),
        Arg::Cell(_) | Arg::Opaque(_) => None,
    }
}

fn held_or_const(arg: &Arg) -> bool {
    matches!(arg, Arg::Held(_) | Arg::Const(_))
}

fn const_n(arg: &Arg) -> Option<i64> {
    match arg {
        Arg::Const(one) => one.n.to_i64(),
        _ => None,
    }
}

pub(crate) fn _check_inserted_conditions(
    ops: &[Op],
    leaving: &std::collections::BTreeSet<crate::model::mir::Value>,
) -> Result<(), Unlowered> {
    let mut alive: std::collections::BTreeSet<_> = leaving.iter().filter(|value| value.flags).copied().collect();
    for op in ops.iter().rev() {
        let preserved = alive.iter().any(|value| !op.defines.contains(value));
        if !op.source_backed && matches!(op.kind, Kind::Add | Kind::Mul | Kind::Smulhi | Kind::PtrOffset) && preserved {
            return Err(Unlowered(format!("inserted {} at {:#x} crosses a live condition", op.kind.as_str(), op.at)));
        }
        for one in &op.defines {
            alive.remove(one);
        }
        alive.extend(op.uses.iter().filter(|value| value.flags).copied());
    }
    Ok(())
}

/// Keep a pure single-use comparison adjacent to its branch during selection.
pub(crate) fn _branch_condition(
    block: &crate::model::mir::MirBlock,
    readers: &std::collections::HashMap<crate::model::mir::Value, usize>,
) -> Vec<Op> {
    let Some(branch) = block.ops.last().filter(|op| op.kind == Kind::Branch) else {
        return block.ops.clone();
    };
    let conditions: Vec<_> = branch.uses.iter().filter(|value| value.flags).collect();
    if conditions.len() != 1 || readers.get(conditions[0]).copied().unwrap_or(0) != 1 {
        return block.ops.clone();
    }
    let last = block.ops.len() - 1;
    for (index, op) in block.ops[..last].iter().enumerate() {
        if op.op == Some(OpCode::Operation(Operation::Compare))
            && op.defines == [*conditions[0]]
            && !(!op.loads.is_empty() || !op.stores.is_empty() || op.barrier())
            && op.args.iter().all(|arg| matches!(arg, Arg::Held(_) | Arg::Const(_) | Arg::Symbol(_)))
        {
            let mut out = block.ops[..index].to_vec();
            out.extend_from_slice(&block.ops[index + 1..last]);
            out.push(op.clone());
            out.push(branch.clone());
            return out;
        }
    }
    block.ops.clone()
}

type Parts = Result<Vec<ir::Semantics>, Unlowered>;

fn _extract(op: &Op, lowering: &mut Lowering) -> Parts {
    if let ([Arg::Held(source), Arg::Const(offset)], [Arg::Held(result)]) = (&op.args[..], &op.results[..]) {
        let offset = offset.n.to_i64();
        if source.width == 4 && result.width == 2 && matches!(offset, Some(0 | 16)) {
            let (source, result) = (source.value, result.value);
            // The low word of a dword register is itself an encodable word operand.
            if offset == Some(0) {
                return Ok(vec![sem(Operation::Move, "mov", vec![held(result.id, 2)], vec![held(source.id, 2)])]);
            }
            // The halves of a sign extension are the word itself and its sign.
            if let Some(word) = lowering.sign_extended(source.id).cloned() {
                let kept = held(result.id, 2);
                // Only where a divide already wanted dx:ax.
                if lowering.divides(result.id, &word) {
                    return Ok(vec![sem(Operation::Extend, "cwd", vec![kept], vec![located(&Arg::Held(word))?])]);
                }
            }
            let discarded = held(lowering.fresh(), 2);
            let kept = held(result.id, 2);
            return Ok(vec![
                sem(Operation::Push, "push", vec![], vec![held(source.id, 4)]),
                sem(Operation::Pop, "pop", vec![if offset == Some(0) { kept.clone() } else { discarded.clone() }], vec![]),
                sem(Operation::Pop, "pop", vec![if offset == Some(0) { discarded } else { kept }], vec![]),
            ]);
        }
    }
    Err(Unlowered(format!("unsupported extraction at {:#x}", op.at)))
}

fn _fixed_multiply(op: &Op, lowering: &mut Lowering) -> Parts {
    let valid = op.args.len() == 3
        && matches!(op.results[..], [Arg::Held(ref result)] if result.width == 4)
        && const_n(&op.args[2]).is_some_and(|n| (1..32).contains(&n))
        && op.args[..2].iter().all(|arg| held_or_const(arg) && arg_width(arg) == Some(4));
    if !valid {
        return Err(Unlowered(format!("unsupported fixed multiply at {:#x}", op.at)));
    }
    let mut setup = vec![];
    let mut factors = vec![];
    for arg in &op.args[..2] {
        let mut factor = located(arg)?;
        if matches!(arg, Arg::Const(_)) {
            let into = held(lowering.fresh(), 4);
            setup.push(sem(Operation::Move, "mov", vec![into.clone()], vec![factor]));
            factor = into;
        }
        factors.push(factor);
    }
    let (low, high) = (held(lowering.fresh(), 4), held(lowering.fresh(), 4));
    let result = located(&op.results[0])?;
    setup.push(sem(Operation::Multiply, "imul", vec![low.clone(), high.clone()], factors));
    setup.push(sem(Operation::Funnel, "shrd", vec![result], vec![low, high, immediate(const_n(&op.args[2]).unwrap(), 1)]));
    Ok(setup)
}

/// Compute wrapping fixed i32 division without general i64 arithmetic.
fn _fixed_division(op: &Op, lowering: &mut Lowering) -> Parts {
    let valid = op.args.len() == 3
        && matches!(op.results[..], [Arg::Held(ref result)] if result.width == 4)
        && const_n(&op.args[2]).is_some_and(|n| (1..32).contains(&n))
        && op.args[..2].iter().all(|arg| held_or_const(arg) && arg_width(arg) == Some(4));
    if !valid {
        return Err(Unlowered(format!("unsupported fixed divide at {:#x}", op.at)));
    }
    let fraction = const_n(&op.args[2]).unwrap();
    let (left_arg, right_arg) = (&op.args[0], &op.args[1]);
    let scale = num_bigint::BigInt::from(1) << fraction;
    let shifted_constant = match left_arg {
        Arg::Const(left) => Some(&left.n << fraction),
        _ => None,
    };
    let mut quotient_fits = shifted_constant.as_ref().is_some_and(|shifted| {
        num_bigint::BigInt::from(-(1i64 << 31)) <= *shifted && *shifted < num_bigint::BigInt::from(1i64 << 31)
    });
    if let Arg::Const(right) = right_arg {
        // |right| >= 1.0 cannot enlarge the stored numerator.
        quotient_fits |= right.n >= scale || right.n < -scale.clone();
    }
    let mut setup = vec![];
    let mut materialize = |arg: &Arg, setup: &mut Vec<ir::Semantics>, lowering: &mut Lowering| -> Result<Loc, Unlowered> {
        let value = located(arg)?;
        if matches!(value, Loc::Held(_)) {
            return Ok(value);
        }
        let into = held(lowering.fresh(), 4);
        setup.push(sem(Operation::Move, "mov", vec![into.clone()], vec![value]));
        Ok(into)
    };
    let divisor = materialize(right_arg, &mut setup, lowering)?;
    let result = located(&op.results[0])?;

    if quotient_fits {
        let mut low = held(lowering.fresh(), 4);
        let high;
        if let Some(shifted) = &shifted_constant {
            setup.push(sem(Operation::Move, "mov", vec![low.clone()], vec![immediate(shifted.to_i64().unwrap(), 4)]));
            high = held(lowering.fresh(), 4);
            setup.push(sem(Operation::Extend, "cdq", vec![high.clone()], vec![low.clone()]));
        } else {
            let left = materialize(left_arg, &mut setup, lowering)?;
            let unshifted_high = held(lowering.fresh(), 4);
            setup.push(sem(Operation::Move, "mov", vec![low.clone()], vec![left]));
            setup.push(sem(Operation::Extend, "cdq", vec![unshifted_high.clone()], vec![low.clone()]));
            high = held(lowering.fresh(), 4);
            setup.push(sem(
                Operation::Funnel,
                "shld",
                vec![high.clone()],
                vec![unshifted_high, low.clone(), immediate(fraction, 1)],
            ));
            let shifted_low = held(lowering.fresh(), 4);
            setup.push(sem(Operation::Binary, "shl", vec![shifted_low.clone()], vec![low, immediate(fraction, 1)]));
            low = shifted_low;
        }
        let remainder = held(lowering.fresh(), 4);
        setup.push(sem(Operation::Divide, "idiv", vec![result, remainder], vec![high, low, divisor]));
        return Ok(setup);
    }

    let left = materialize(left_arg, &mut setup, lowering)?;
    let (sign_left, sign_right) = (held(lowering.fresh(), 4), held(lowering.fresh(), 4));
    setup.push(sem(Operation::Binary, "sar", vec![sign_left.clone()], vec![left.clone(), immediate(31, 1)]));
    setup.push(sem(Operation::Binary, "sar", vec![sign_right.clone()], vec![divisor.clone(), immediate(31, 1)]));
    let mut magnitude = |value: Loc, sign: Loc, setup: &mut Vec<ir::Semantics>, lowering: &mut Lowering| {
        let changed = held(lowering.fresh(), 4);
        let absolute = held(lowering.fresh(), 4);
        setup.push(sem(Operation::Binary, "xor", vec![changed.clone()], vec![value, sign.clone()]));
        setup.push(sem(Operation::Binary, "sub", vec![absolute.clone()], vec![changed, sign]));
        absolute
    };
    let absolute_left = magnitude(left, sign_left.clone(), &mut setup, lowering);
    let absolute_divisor = magnitude(divisor, sign_right.clone(), &mut setup, lowering);
    let result_sign = held(lowering.fresh(), 4);
    let (high, low) = (held(lowering.fresh(), 4), held(lowering.fresh(), 4));
    let zero = held(lowering.fresh(), 4);
    setup.push(sem(Operation::Binary, "xor", vec![result_sign.clone()], vec![sign_left, sign_right]));
    setup.push(sem(Operation::Binary, "shr", vec![high.clone()], vec![absolute_left.clone(), immediate(32 - fraction, 1)]));
    setup.push(sem(Operation::Binary, "shl", vec![low.clone()], vec![absolute_left, immediate(fraction, 1)]));
    setup.push(sem(Operation::Move, "mov", vec![zero.clone()], vec![immediate(0, 4)]));
    let (upper_quotient, upper_remainder) = (held(lowering.fresh(), 4), held(lowering.fresh(), 4));
    let (low_quotient, low_remainder) = (held(lowering.fresh(), 4), held(lowering.fresh(), 4));
    let signed = held(lowering.fresh(), 4);
    setup.push(sem(
        Operation::Divide,
        "div",
        vec![upper_quotient, upper_remainder.clone()],
        vec![zero, high, absolute_divisor.clone()],
    ));
    setup.push(sem(
        Operation::Divide,
        "div",
        vec![low_quotient.clone(), low_remainder],
        vec![upper_remainder, low, absolute_divisor],
    ));
    setup.push(sem(Operation::Binary, "xor", vec![signed.clone()], vec![low_quotient, result_sign.clone()]));
    setup.push(sem(Operation::Binary, "sub", vec![result], vec![signed, result_sign]));
    Ok(setup)
}

fn _concat(op: &Op, _lowering: &mut Lowering) -> Parts {
    let word = |arg: &Arg| held_or_const(arg) && arg_width(arg) == Some(2);
    if op.args.len() == 2 && word(&op.args[0]) && word(&op.args[1]) {
        if let [Arg::Held(result)] = &op.results[..] {
            if result.width == 4 {
                let (high, low) = (located(&op.args[0])?, located(&op.args[1])?);
                return Ok(vec![
                    sem(Operation::Push, "push", vec![], vec![high]),
                    sem(Operation::Push, "push", vec![], vec![low]),
                    sem(Operation::Pop, "pop", vec![located(&op.results[0])?], vec![]),
                ]);
            }
        }
    }
    Err(Unlowered(format!("unsupported concatenation at {:#x}", op.at)))
}

fn _signed_high_product(op: &Op, lowering: &mut Lowering) -> Parts {
    let result = match &op.results[..] {
        [Arg::Held(result)] if op.args.len() == 2 && matches!(result.width, 2 | 4) => Some(result),
        _ => None,
    };
    let Some(result) =
        result.filter(|result| op.args.iter().all(|arg| held_or_const(arg) && arg_width(arg) == Some(result.width)))
    else {
        return Err(Unlowered(format!("unsupported signed high product at {:#x}", op.at)));
    };
    let (mut setup, mut sources) = (vec![], vec![]);
    for arg in &op.args {
        let mut source = located(arg)?;
        if matches!(arg, Arg::Const(_)) {
            let into = held(lowering.fresh(), result.width);
            setup.push(sem(Operation::Move, "mov", vec![into.clone()], vec![source]));
            source = into;
        }
        sources.push(source);
    }
    let low = held(lowering.fresh(), result.width);
    setup.push(sem(Operation::Multiply, "imul", vec![low, located(&op.results[0])?], sources));
    Ok(setup)
}

fn _pointer_offset(op: &Op, lowering: &mut Lowering) -> Parts {
    let Some(model) = lowering.pointer_model.clone() else {
        return Err(Unlowered(format!("pointer offset at {:#x} needs an established pointer ABI", op.at)));
    };
    if op.args.len() != 2 || op.results.len() != 1 {
        return Err(Unlowered(format!("unsupported pointer offset at {:#x}", op.at)));
    }
    let (pointer, displacement, result) = (located(&op.args[0])?, located(&op.args[1])?, located(&op.results[0])?);
    model.offset(&pointer, &displacement, &result, &mut || lowering.fresh()).map_err(Unlowered)
}

fn _pointer_access(op: &Op, lowering: &mut Lowering) -> Result<Option<Vec<ir::Semantics>>, Unlowered> {
    let mut references: Vec<&MemRef> = vec![];
    for arg in op.args.iter().chain(&op.results) {
        if let Arg::Cell(cell) = arg {
            if cell.r#ref.pointer && !references.contains(&&cell.r#ref) {
                references.push(&cell.r#ref);
            }
        }
    }
    if references.is_empty() {
        return Ok(None);
    }
    if lowering.pointer_model.is_none() {
        return Err(Unlowered(format!("pointer access at {:#x} needs an established pointer ABI", op.at)));
    }
    if references.len() != 1 || !matches!(op.kind, Kind::Load | Kind::Store) {
        return Err(Unlowered(format!("unsupported whole-pointer memory operation at {:#x}", op.at)));
    }
    let reference = references[0].clone();
    let Some(base) = reference.base.filter(|_| {
        reference.base_width == 4 && reference.addr.is_none() && reference.segment.is_none()
    }) else {
        return Err(Unlowered(format!("whole-pointer access has an unnormalized address at {:#x}", op.at)));
    };
    let offset = ir::Held { value: lowering.fresh(), width: 2 };
    let selector = Loc::Reg(ir::Reg { register: Register::ES, width: 2 });
    let cell = ir::Mem {
        base: Some(offset),
        ..ir::Mem::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) }), reference.width)
    };
    let place = |arg: &Arg, had: &[Loc], index: usize| -> Result<Placed, Unlowered> {
        Ok(match arg {
            Arg::Cell(one) if one.r#ref == reference => Placed::Loc(Loc::Mem(cell.clone())),
            _ => as_a_value(arg, had, index),
        })
    };
    let node = lowering.node(op).cloned();
    let Some(access) = current(op, Place::With(&place), node.as_ref())? else {
        return Err(Unlowered(format!("whole-pointer access has no operation at {:#x}", op.at)));
    };
    // Materialization is local to the memory instruction. Restore the segment
    // resource so other MIR addresses retain their address-space identity.
    Ok(Some(vec![
        sem(Operation::Push, "push", vec![], vec![selector.clone()]),
        sem(Operation::Push, "push", vec![], vec![held(base.id, 4)]),
        sem(Operation::Pop, "pop", vec![Loc::Held(offset)], vec![]),
        sem(Operation::Pop, "pop", vec![selector.clone()], vec![]),
        access,
        sem(Operation::Pop, "pop", vec![selector], vec![]),
    ]))
}

fn _constant_store(op: &Op, lowering: &mut Lowering) -> Result<Option<Vec<ir::Semantics>>, Unlowered> {
    if let ([Arg::Const(bits)], [Arg::Cell(cell)]) = (&op.args[..], &op.results[..]) {
        let reference = &cell.r#ref;
        if bits.width == 8 && reference.width == 8 {
            let Some(addr) = reference.addr.filter(|_| reference.base.is_none() && reference.segment.is_none()) else {
                return Err(Unlowered("wide constant store needs a static address".into()));
            };
            let mut parts = vec![];
            for offset in [0i64, 4] {
                let half = MemRef { width: 4, addr: Some(Addr { disp: addr.disp + offset, ..addr }), ..reference.clone() };
                let word = ((&bits.n >> (offset * 8)) & num_bigint::BigInt::from(0xFFFF_FFFFu32)).to_i64().unwrap();
                parts.push(sem(Operation::Move, "mov", vec![Loc::Mem(_addressed(&half)?)], vec![immediate(word, 4)]));
            }
            return Ok(Some(parts));
        }
    }
    _pointer_access(op, lowering)
}

/// `rep stos`: the count in cx, the value in the accumulator, the cells through es:di.
fn _fill(op: &Op, lowering: &mut Lowering) -> Parts {
    let [value, count, address, selector @ ..] = &op.args[..] else {
        return Err(Unlowered(format!("fill at {:#x} has too few operands", op.at)));
    };
    let value_width = arg_width(value).ok_or_else(|| Unlowered(format!("fill of a cell at {:#x}", op.at)))?;
    let name = match value_width {
        1 => "stosb",
        2 => "stosw",
        4 => "stosd",
        _ => return Err(Unlowered(format!("fill of {value_width}-byte cells at {:#x}", op.at))),
    };
    let mut setup = vec![];
    let mut in_register = |arg: &Arg, width: u32, setup: &mut Vec<ir::Semantics>| -> Result<Loc, Unlowered> {
        if matches!(arg, Arg::Held(_)) {
            return located(arg);
        }
        let into = held(lowering.fresh(), width);
        setup.push(sem(Operation::Move, "mov", vec![into.clone()], vec![located(arg)?]));
        Ok(into)
    };
    let stored = in_register(value, value_width, &mut setup)?;
    let counted = in_register(count, 2, &mut setup)?;
    let through = in_register(address, 2, &mut setup)?;
    let selected = match selector.first() {
        Some(selector) => Some(in_register(selector, 2, &mut setup)?),
        None => None,
    };
    let (stepped, emptied) = (held(lowering.fresh(), 2), held(lowering.fresh(), 2));
    let cells = Loc::Mem(ir::Mem::new(None, 0));
    if let Some(selected) = selected {
        setup.push(sem(Operation::Fill, name, vec![cells, stepped, emptied], vec![stored, counted, through, selected]));
        return Ok(setup);
    }
    let extra = Loc::Reg(ir::Reg { register: Register::ES, width: 2 });
    let source_segment =
        if op.stores.iter().any(|one| one.space == Some(Space::Frame)) { Register::SS } else { Register::DS };
    setup.extend([
        sem(Operation::Push, "push", vec![], vec![extra.clone()]),
        sem(Operation::Push, "push", vec![], vec![Loc::Reg(ir::Reg { register: source_segment, width: 2 })]),
        sem(Operation::Pop, "pop", vec![extra.clone()], vec![]),
        sem(Operation::Fill, name, vec![cells, stepped, emptied], vec![stored, counted, through, extra.clone()]),
        sem(Operation::Pop, "pop", vec![extra], vec![]),
    ]);
    Ok(setup)
}

/// A dead AND destination needs flags, not a two-address temporary.
fn _flag_test(op: &Op, context: &Lowering) -> Option<Vec<ir::Semantics>> {
    if op.kind != Kind::And
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || !op.merges.is_empty()
        || op.barrier()
        || op.results.len() != 1
        || op.args.len() != 2
    {
        return None;
    }
    let Arg::Held(result) = &op.results[0] else {
        return None;
    };
    if context._read.contains(&result.value.id)
        || context._exposed.contains(&result.value.id)
        || !matches!(result.width, 2 | 4)
        || op.args.iter().any(|arg| !matches!(arg, Arg::Held(one) if one.width == result.width))
        || op.defines.iter().any(|value| *value != result.value && !value.flags)
    {
        return None;
    }
    Some(vec![sem(Operation::Compare, "test", vec![], op.args.iter().map(|arg| located(arg).unwrap()).collect())])
}

fn _sign_word(op: &Op) -> Option<Vec<ir::Semantics>> {
    if op.op != Some(OpCode::Operation(Operation::Extend)) || op.args.len() != 1 || op.results.len() != 1 {
        return None;
    }
    let (Arg::Held(source), Arg::Held(result)) = (&op.args[0], &op.results[0]) else {
        return None;
    };
    if source.width != result.width || !matches!(source.width, 2 | 4) {
        return None;
    }
    let (source, result) = (held(source.value.id, source.width), held(result.value.id, result.width));
    let width = source_width(&source);
    Some(vec![
        sem(Operation::Move, "mov", vec![result.clone()], vec![source]),
        sem(Operation::Binary, "sar", vec![result.clone()], vec![result, immediate(i64::from(width) * 8 - 1, 1)]),
    ])
}

fn source_width(loc: &Loc) -> u32 {
    match loc {
        Loc::Held(one) => one.width,
        _ => unreachable!(),
    }
}

/// One body being lowered, and the values the expansion invents.
pub struct Lowering {
    pub pointer_model: Option<super::pointers::Model>,
    _read: std::collections::BTreeSet<u32>,
    /// Which dword values are a word's sign extension, and which word.
    _extended: std::collections::HashMap<u32, Held>,
    _dividends: std::collections::HashMap<u32, u32>,
    _exposed: std::collections::BTreeSet<u32>,
    _nodes: std::collections::HashMap<u32, Node>,
    _next: u32,
}

impl Lowering {
    /// The decoded occurrence for this source-backed operation, if any.
    pub fn node(&self, op: &Op) -> Option<&Node> {
        if op.source_backed { op.id.and_then(|id| self._nodes.get(&id)) } else { None }
    }

    /// A value id nothing in this body already uses.
    pub fn fresh(&mut self) -> u32 {
        self._next += 1;
        self._next - 1
    }

    /// The word this dword is the sign extension of, if it is one.
    pub fn sign_extended(&self, value: u32) -> Option<&Held> {
        self._extended.get(&value)
    }

    /// Whether `high` is only ever a divide's high half over that word.
    pub fn divides(&self, high: u32, word: &Held) -> bool {
        self._dividends.get(&high).is_some_and(|found| *found == word.value.id)
    }
}
