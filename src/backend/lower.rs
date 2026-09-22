//! Port of `qbopt/backend/lower.py`: MIR to machine form.
//!
//! Ported so far: the operand and naming half, up to `lowered`.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use indexmap::IndexMap;

use iced_x86::Register;
use num_traits::ToPrimitive;

use super::{addressforms, arithmetic, comparefold, cpu, division, farload, lower_floats, lower_switches, rmw, target};
use crate::abi::runtime;
use crate::legacy::calls;
use crate::analysis::{consts, induction, liveness, loops, ssa};
use crate::model::floating::{Format, Rounding};
use crate::support::pyset::PySet;
use crate::model::lir::{self, Insn};
use crate::model::ir::nodes::Node;
use crate::model::ir::{self, Loc, Operation};
use crate::model::mir::{AllocationHints, Arg, Held, Kind, MemRef, MirBody, Op, OpCode, Value};
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
    if let Some(found) = _machine_kind(kind) {
        return Some(found);
    }
    Some(match kind {
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
    })
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
    let materialize = |arg: &Arg, setup: &mut Vec<ir::Semantics>, lowering: &mut Lowering| -> Result<Loc, Unlowered> {
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
    let magnitude = |value: Loc, sign: Loc, setup: &mut Vec<ir::Semantics>, lowering: &mut Lowering| {
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
    let Some(access) = current(op, Place::With(&place), node.as_deref())? else {
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


/// Python's `Counter`: a missing key reads as zero.
fn count(counter: &IndexMap<u32, i64>, value: u32) -> i64 {
    counter.get(&value).copied().unwrap_or(0)
}

fn plain(one: &Insn) -> bool {
    !(!one.clobbers.is_empty() || !one.requires.is_empty() || !one.delivers.is_empty() || !one.spread.is_empty())
}

/// The value of a one-operand `push` of a held value.
fn pushed_value(what: &ir::Semantics) -> Option<(u32, u32)> {
    match (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]) {
        (Operation::Push, Some("push"), [], [Loc::Held(one)]) => Some((one.value, one.width)),
        _ => None,
    }
}

/// A `mov` of an immediate into a held value.
fn immediate_copy(what: &ir::Semantics) -> Option<(u32, u32, ir::Imm)> {
    match (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..]) {
        (Operation::Move, Some("mov"), [Loc::Held(one)], [Loc::Imm(immediate)]) => {
            Some((one.value, one.width, immediate.clone()))
        }
        _ => None,
    }
}

/// Select immediate pushes without keeping literal addresses live across calls.
fn _rematerialized_arguments(
    insns: &[Arc<Insn>],
    uses: &IndexMap<u32, i64>,
    exposed: &BTreeSet<u32>,
) -> Vec<Arc<Insn>> {
    let mut literals: IndexMap<u32, (Arc<Insn>, ir::Imm)> = IndexMap::new();
    // Only values whose every known use is an eligible PUSH may be recreated
    // independently of an ordered setup sequence.
    let mut pushed: IndexMap<u32, i64> = IndexMap::new();
    for one in insns {
        if let Some(what) = &one.what {
            if what.op == Operation::Push
                && what.sources.len() == 1
                && matches!(&what.sources[0], Loc::Held(held) if one.uses.contains(&held.value))
            {
                let Loc::Held(held) = &what.sources[0] else { unreachable!() };
                for value in &one.uses {
                    if *value == held.value {
                        *pushed.entry(*value).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut consumed: IndexMap<u32, i64> = IndexMap::new();
    let mut out = Vec::new();
    for one in insns {
        let mut one = Arc::clone(one);
        if let Some((value, width)) = one.what.as_ref().and_then(pushed_value) {
            if literals.contains_key(&value)
                && count(uses, value) == count(&pushed, value)
                && plain(&one)
                && one.defines.is_empty()
                && one.uses == [value]
            {
                let (definition, immediate) = literals[&value].clone();
                if width == immediate.width {
                    let mut rewritten = (*one).clone();
                    let mut what = rewritten.what.clone().unwrap();
                    what.sources = vec![Loc::Imm(immediate.clone())];
                    rewritten.what = Some(what);
                    rewritten.uses = vec![];
                    rewritten.op = definition.op.clone();
                    rewritten.symbol = Some(immediate.address.is_some());
                    rewritten.rematerialized = true;
                    one = Arc::new(rewritten);
                    *consumed.entry(value).or_insert(0) += 1;
                }
            }
        }
        for value in &one.defines {
            literals.shift_remove(value);
        }
        if let Some((value, width, immediate)) = one.what.as_ref().and_then(immediate_copy) {
            if width == immediate.width
                && matches!(width, 2 | 4)
                && one.defines == [value]
                && one.uses.is_empty()
                && plain(&one)
            {
                literals.insert(value, (Arc::clone(&one), immediate));
            }
        }
        out.push(one);
    }
    let dead: Vec<Arc<Insn>> = literals
        .iter()
        .filter(|(value, _)| {
            count(&consumed, **value) == count(uses, **value) && count(&consumed, **value) != 0 && !exposed.contains(value)
        })
        .map(|(_, (definition, _))| Arc::clone(definition))
        .collect();
    lir::without(&out, |one| dead.iter().any(|dead| Arc::ptr_eq(dead, one)), None::<fn(&Arc<Insn>) -> Arc<Insn>>)
}

/// Fold a single-use load into the PUSH that consumes it.
fn _memory_arguments(insns: &[Arc<Insn>], uses: &IndexMap<u32, i64>, exposed: &BTreeSet<u32>) -> Vec<Arc<Insn>> {
    let mut loaded: IndexMap<u32, (Arc<Insn>, ir::Mem)> = IndexMap::new();
    let mut consumed: IndexMap<u32, i64> = IndexMap::new();
    let mut out: Vec<Arc<Insn>> = Vec::new();
    for one in insns {
        let mut one = Arc::clone(one);
        let mut folded = false;
        if let Some((value, width)) = one.what.as_ref().and_then(pushed_value) {
            if let Some((definition, source)) = loaded.get(&value).cloned() {
                if width == source.width
                    && matches!(source.width, 2 | 4)
                    && count(uses, value) == 1
                    && !exposed.contains(&value)
                    && one.defines.is_empty()
                    && plain(&one)
                {
                    let mut rewritten = (*one).clone();
                    let mut what = rewritten.what.clone().unwrap();
                    what.sources = vec![Loc::Mem(source)];
                    rewritten.what = Some(what);
                    rewritten.uses = vec![];
                    rewritten.op = definition.op.clone();
                    rewritten.symbol = definition.symbol;
                    one = Arc::new(rewritten);
                    *consumed.entry(value).or_insert(0) += 1;
                    folded = true;
                }
            }
        }

        let what = one.what.as_ref();
        let writes_memory = what.is_none_or(|what| what.dests.iter().any(|dest| matches!(dest, Loc::Mem(_))))
            || one.op.as_ref().is_some_and(|op| !op.stores.is_empty());
        let writes_fixed = what.is_none_or(|what| what.dests.iter().any(|dest| matches!(dest, Loc::Reg(_))));
        let barrier =
            what.is_none_or(|what| matches!(what.op, Operation::Barrier | Operation::Call | Operation::Return));
        if writes_memory || writes_fixed || barrier || !one.clobbers.is_empty() {
            loaded.clear();
        }

        for value in &one.defines {
            loaded.shift_remove(value);
        }
        if !folded {
            if let Some(what) = what {
                if let (Operation::Move, Some("mov"), [Loc::Held(dest)], [Loc::Mem(source)]) =
                    (what.op, what.name.as_deref(), &what.dests[..], &what.sources[..])
                {
                    if dest.width == source.width
                        && matches!(source.width, 2 | 4)
                        && source.addr.is_some_and(|addr| addr.space == Space::Frame && addr.disp >= 4)
                        && ir::root(source.through) != Register::ESP
                        && ir::root(source.index_through) != Register::ESP
                        && !source.stack_argument
                        && one.defines == [dest.value]
                        && one.uses.is_empty()
                        && plain(&one)
                    {
                        loaded.insert(dest.value, (Arc::clone(&one), source.clone()));
                    }
                }
            }
        }
        out.push(one);
    }

    let dead: BTreeSet<u32> = consumed
        .keys()
        .filter(|value| {
            count(&consumed, **value) == count(uses, **value) && count(&consumed, **value) != 0 && !exposed.contains(value)
        })
        .copied()
        .collect();
    let mut previous: Option<Vec<*const Insn>> = None;
    loop {
        let now: Vec<*const Insn> = out.iter().map(Arc::as_ptr).collect();
        if previous.as_ref() == Some(&now) {
            break;
        }
        previous = Some(now);
        out = lir::without(
            &out,
            |one| one.defines.iter().any(|value| dead.contains(value)),
            None::<fn(&Arc<Insn>) -> Arc<Insn>>,
        );
    }
    out
}

/// Select a direct push for an adjacent, single-use immediate definition.
fn _immediate_arguments(insns: &[Arc<Insn>], uses: &IndexMap<u32, i64>) -> Vec<Arc<Insn>> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < insns.len() {
        if index + 1 < insns.len() && plain(&insns[index]) && plain(&insns[index + 1]) {
            let (copy, push) = (&insns[index], &insns[index + 1]);
            let copied = copy.what.as_ref().and_then(immediate_copy);
            let pushed = push.what.as_ref().and_then(pushed_value);
            if let (Some((value, width, immediate)), Some((pushed, pushed_width))) = (copied, pushed) {
                if value == pushed
                    && width == pushed_width
                    && pushed_width == immediate.width
                    && matches!(width, 2 | 4)
                    && count(uses, value) == 1
                    && copy.uses.is_empty()
                    && copy.defines == [value]
                    && push.uses == [value]
                    && push.defines.is_empty()
                {
                    let mut combined = (**copy).clone();
                    let mut what = push.what.clone().unwrap();
                    what.sources = vec![Loc::Imm(immediate)];
                    combined.what = Some(what);
                    combined.defines = vec![];
                    combined.uses = vec![];
                    let folded = lir::without(
                        &[Arc::new(combined), Arc::clone(push)],
                        |one| Arc::ptr_eq(one, push),
                        None::<fn(&Arc<Insn>) -> Arc<Insn>>,
                    );
                    if folded.len() == 1 {
                        out.extend(folded);
                        index += 2;
                        continue;
                    }
                }
            }
        }
        out.push(Arc::clone(&insns[index]));
        index += 1;
    }
    out
}

/// One instruction an expansion inserted, beside the operation it came from.
fn _follows(op: &Op, what: ir::Semantics) -> Arc<Insn> {
    let defines = _written(&what.dests);
    let uses = _read(&what);
    Arc::new(Insn::new(op.at, Some((op.at, op.at)), Some(what), defines, uses))
}

/// Which phi results this body still reads, transitively.
fn _phis_worth_keeping(body: &MirBody, made: &IndexMap<i64, Vec<Arc<Insn>>>) -> BTreeSet<u32> {
    let mut wanted: BTreeSet<u32> =
        made.values().flat_map(|insns| insns.iter().flat_map(|one| one.uses.iter().copied())).collect();
    let phis: IndexMap<u32, Vec<u32>> = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .filter(|phi| !phi.result.flags)
        .map(|phi| (phi.result.id, phi.incoming.values().map(|value| value.id).collect()))
        .collect();
    let mut changing = true;
    while changing {
        changing = false;
        for (result, incoming) in &phis {
            if wanted.contains(result) && !incoming.iter().all(|one| wanted.contains(one)) {
                wanted.extend(incoming.iter().copied());
                changing = true;
            }
        }
    }
    wanted
}

/// The abstract values an operand list names, `ir.values` deciding.
fn _named_values(where_: &[Loc]) -> Vec<u32> {
    where_.iter().flat_map(|operand| ir::values(operand).into_iter().map(|one| one.value)).collect()
}

/// The values an instruction writes: a destination that *is* a value.
fn _written(dests: &[Loc]) -> Vec<u32> {
    dests
        .iter()
        .filter_map(|one| match one {
            Loc::Held(one) => Some(one.value),
            _ => None,
        })
        .collect()
}

/// The values an instruction reads: its sources, and the addresses its
/// destinations are reached by.
fn _read(what: &ir::Semantics) -> Vec<u32> {
    let mut out = _named_values(&what.sources);
    for where_ in &what.dests {
        if !matches!(where_, Loc::Held(_)) {
            out.extend(ir::values(where_).into_iter().map(|one| one.value));
        }
    }
    out
}

/// What an absorbed divide destroys, for a caller with no call map.
pub fn clobbering(op: &Op) -> BTreeSet<Register> {
    if op.kind == Kind::Divmod { _clobbers(op, &IndexMap::new(), None, None) } else { BTreeSet::new() }
}

/// Which registers this instruction destroys without naming them.
fn _clobbers(
    op: &Op,
    calls: &IndexMap<i64, String>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
    node: Option<&Node>,
) -> BTreeSet<Register> {
    if let Some(Node::Restore(node)) = node {
        // The second pop overwrites the other register even when its result is dead.
        return BTreeSet::from([crate::model::mir::restore_pair(node.pair as i64).unwrap().1]);
    }
    if op.kind == Kind::Divmod {
        // An absorbed divide writes the dividend's, the divisor's, idiv's own
        // edx, and wherever the answer it was not asked for is kept.
        return [calls::RESULT, calls::DIVISOR, Register::EDX, calls::OTHER]
            .into_iter()
            .filter(|register| target::AVAILABLE.contains(register))
            .collect();
    }
    if op.kind != Kind::Call {
        return BTreeSet::new();
    }
    let contract = _contract(op, calls, contracts);
    let names = _names();
    let disturbed = runtime::disturbs(&contract);
    // A contract is about the 8086 and names no FS or GS; one reaching user
    // code, or written for the 386, runs code that may use them.
    let mut out: BTreeSet<Register> = if disturbed == *runtime::EVERY || contract.i386 {
        target::SELECTORS.into_iter().filter(|register| !names.contains_key(register)).collect()
    } else {
        BTreeSet::new()
    };
    for (register, spelled) in &names {
        if disturbed.iter().any(|named| spelled.contains(&named.value().to_lowercase())) {
            out.insert(*register);
        }
    }
    out
}

fn _contract(
    op: &Op,
    calls: &IndexMap<i64, String>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
) -> runtime::Contract {
    contracts
        .and_then(|contracts| contracts.get(&op.at).cloned())
        .unwrap_or_else(|| runtime::contract(calls.get(&op.at).map(String::as_str)))
}

/// The registers a 386 callee keeps only the 16-bit half of.
fn _clobbered_high(
    op: &Op,
    calls: &IndexMap<i64, String>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
) -> BTreeSet<Register> {
    if op.kind != Kind::Call {
        return BTreeSet::new();
    }
    if !_contract(op, calls, contracts).i386 {
        return BTreeSet::new();
    }
    let whole: BTreeSet<Register> = _clobbers(op, calls, contracts, None).into_iter().map(ir::root).collect();
    target::AVAILABLE.into_iter().filter(|register| !whole.contains(&ir::root(*register))).collect()
}

/// Each allocatable register by the names runtime.py's own Reg enum uses.
fn _names() -> IndexMap<Register, BTreeSet<String>> {
    target::AVAILABLE
        .into_iter()
        .chain([Register::ES])
        .map(|register| {
            (
                register,
                BTreeSet::from([
                    target::name_of(target::named(register, 2)),
                    target::name_of(target::named(register, 4)),
                ]),
            )
        })
        .collect()
}

fn _word_division(op: &Op, lowering: &mut Lowering) -> Result<Option<Vec<ir::Semantics>>, Unlowered> {
    if op.args.len() != 2 || op.results.len() != 2 {
        return Ok(None);
    }
    let width = match &op.results[0] {
        Arg::Held(one) => one.width,
        _ => 0,
    };
    if width == 4 && op.source_backed {
        return Ok(None); // Legacy folded sites order results by their runtime entry point.
    }
    if !matches!(width, 2 | 4)
        || !std::iter::once(&op.args[0]).chain(&op.results).all(|arg| matches!(arg, Arg::Held(one) if one.width == width))
        || !held_or_const(&op.args[1])
        || arg_width(&op.args[1]) != Some(width)
    {
        return Ok(None);
    }
    let (dividend, mut divisor) = (located(&op.args[0])?, located(&op.args[1])?);
    let unsigned = op.kind == Kind::Udivmod;
    if let (Arg::Const(constant), false) = (&op.args[1], unsigned) {
        let Loc::Held(held_dividend) = dividend else { unreachable!() };
        let results: Vec<ir::Held> = op
            .results
            .iter()
            .map(|one| match one {
                Arg::Held(one) => ir::Held { value: one.value.id, width: one.width },
                _ => unreachable!(),
            })
            .collect();
        let Arg::Held(remainder) = &op.results[1] else { unreachable!() };
        let remainder = lowering._read.contains(&remainder.value.id);
        let n = constant.n.to_i64().expect("a word divisor fits an int64");
        let cpu = lowering.cpu;
        let reciprocal = division::reciprocal(held_dividend, n, &results, &mut || lowering.fresh(), cpu, remainder)
            .map_err(Unlowered)?;
        if reciprocal.is_some() {
            return Ok(reciprocal);
        }
    }
    let mut setup = vec![];
    if matches!(op.args[1], Arg::Const(_)) {
        let into = held(lowering.fresh(), width);
        setup.push(sem(Operation::Move, "mov", vec![into.clone()], vec![divisor]));
        divisor = into;
    }
    let high = held(lowering.fresh(), width);
    setup.push(if unsigned {
        sem(Operation::Move, "mov", vec![high.clone()], vec![immediate(0, width)])
    } else {
        sem(Operation::Extend, if width == 2 { "cwd" } else { "cdq" }, vec![high.clone()], vec![dividend.clone()])
    });
    let name = if unsigned { "div" } else { "idiv" };
    let results = op.results.iter().map(located).collect::<Result<_, _>>()?;
    setup.push(sem(Operation::Divide, name, results, vec![high, dividend, divisor]));
    Ok(Some(setup))
}

/// Select a cheaper target-specific chain when multiply flags are unobserved.
fn _scaled(op: &Op, context: &mut Lowering) -> Result<Option<Vec<ir::Semantics>>, Unlowered> {
    if op.args.len() != 2 || op.results.len() != 1 {
        return Ok(None);
    }
    let (mut source, mut scale) = (&op.args[0], &op.args[1]);
    if matches!(source, Arg::Const(_)) {
        (source, scale) = (scale, source);
    }
    let result = &op.results[0];
    let (Arg::Held(held_source), Arg::Const(constant), Arg::Held(held_result)) = (source, scale, result) else {
        return Ok(None);
    };
    let width = held_source.width;
    if !(width == held_result.width
        && matches!(width, 2 | 4)
        && constant.width == width
        && num_bigint::BigInt::from(1) < constant.n
        && constant.n < (num_bigint::BigInt::from(1) << (width * 8))
        && op.loads.is_empty()
        && op.stores.is_empty()
        && op.merges.is_empty())
    {
        return Ok(None);
    }
    let n = constant.n.to_i64().unwrap();
    let Some(chain) = arithmetic::scale(n, context.cpu).map_err(Unlowered)? else {
        return Ok(None);
    };
    if chain.iter().any(|(name, count)| *name == "shl" && *count >= i64::from(width) * 8) {
        return Ok(None);
    }
    let mut parts = vec![];
    let mut current = located(source)?;
    for (index, (name, count)) in chain.iter().enumerate() {
        let into = if index == chain.len() - 1 { located(result)? } else { held(context.fresh(), width) };
        let other = if *name == "shl" { immediate(*count, 1) } else { located(source)? };
        parts.push(sem(Operation::Binary, name, vec![into.clone()], vec![current, other]));
        current = into;
    }
    Ok(Some(parts))
}

/// One body being lowered, and the values the expansion invents.
///
/// The first instruction an operation becomes is its leader and carries the
/// operation; every one after it is an insertion with no bytes of its own.
pub struct Lowering<'a> {
    pub cpu: &'static cpu::Profile,
    pub pointer_model: Option<super::pointers::Model>,
    _read: BTreeSet<u32>,
    /// Which dword values are a word's sign extension, and which word.
    _extended: IndexMap<u32, Held>,
    _dividends: IndexMap<u32, u32>,
    _exposed: BTreeSet<u32>,
    _coverage: IndexMap<u32, Vec<(i64, i64)>>,
    _occurrences: Option<&'a IndexMap<u32, Vec<(i64, i64)>>>,
    _nodes: IndexMap<u32, Arc<Node>>,
    _origin: IndexMap<Value, Register>,
    _calls: &'a IndexMap<i64, String>,
    _contracts: Option<&'a IndexMap<i64, runtime::Contract>>,
    /// Python's `set[int] | dict[int, tuple]`: only the id form is ported.
    /// The dict carries BC's folded-site records, whose answers `_delivered`
    /// places; with ids alone `_sites` is empty, exactly as Python makes it.
    _absorbed: BTreeSet<u32>,
    _address_forms: IndexMap<u32, (ir::Held, num_bigint::BigInt)>,
    _indexed: IndexMap<u32, addressforms::FoldedForm>,
    _folded: BTreeSet<u32>,
    _address_promoted: BTreeSet<u32>,
    _next: u32,
}

/// Python's keyword arguments to `Lowering`.
#[derive(Default)]
pub struct Options<'a> {
    pub coverage: IndexMap<u32, Vec<(i64, i64)>>,
    pub nodes: IndexMap<u32, Arc<Node>>,
    pub occurrences: Option<&'a IndexMap<u32, Vec<(i64, i64)>>>,
    pub origin: IndexMap<Value, Register>,
    pub pointer_model: Option<super::pointers::Model>,
}

impl<'a> Lowering<'a> {
    pub fn new(
        body: &MirBody,
        read: BTreeSet<u32>,
        calls: &'a IndexMap<i64, String>,
        absorbed: BTreeSet<u32>,
        contracts: Option<&'a IndexMap<i64, runtime::Contract>>,
        cpu: impl Into<cpu::ProfileOrName<'static>>,
        options: Options<'a>,
    ) -> Result<Self, Unlowered> {
        let cpu = cpu::profile(cpu).map_err(Unlowered)?;
        let ops = || body.blocks.iter().flat_map(|block| &block.ops);
        let mut extended = IndexMap::new();
        for one in ops() {
            if let ([Arg::Held(source)], [Arg::Held(result)]) = (&one.args[..], &one.results[..]) {
                if one.kind == Kind::SignExtend && source.width == 2 && result.width == 4 {
                    extended.insert(result.value.id, source.clone());
                }
            }
        }
        // A value used once, as a divide's high half over the low half beside it.
        let mut readers: IndexMap<u32, i64> = IndexMap::new();
        let mut dividends: IndexMap<u32, u32> = IndexMap::new();
        for one in ops() {
            for arg in &one.args {
                if let Arg::Held(arg) = arg {
                    *readers.entry(arg.value.id).or_insert(0) += 1;
                }
            }
            if one.kind == Kind::Div && one.args.len() == 3 {
                if let (Arg::Held(high), Arg::Held(low)) = (&one.args[0], &one.args[1]) {
                    dividends.insert(high.value.id, low.value.id);
                }
            }
        }
        let dividends = dividends.into_iter().filter(|(high, _)| count(&readers, *high) == 1).collect();
        let exposed: BTreeSet<u32> = crate::model::mir::exposed(body).into_iter().map(|value| value.id).collect();
        let address_forms = addressforms::offsets(body);
        let (indexed, folded, address_promoted) =
            addressforms::indexed(body, &exposed, &cpu.address_forms, Some(&cpu.operations)).map_err(Unlowered)?;
        let mut every: Vec<u32> = ops().flat_map(|op| op.defines.iter().chain(&op.uses)).map(|one| one.id).collect();
        every.extend(body.blocks.iter().flat_map(|block| &block.phis).map(|phi| phi.result.id));
        Ok(Self {
            cpu,
            pointer_model: options.pointer_model,
            _read: read,
            _extended: extended,
            _dividends: dividends,
            _exposed: exposed,
            _coverage: options.coverage,
            _occurrences: options.occurrences,
            _nodes: options.nodes,
            _origin: options.origin,
            _calls: calls,
            _contracts: contracts,
            _absorbed: absorbed,
            _address_forms: address_forms,
            _indexed: indexed,
            _folded: folded,
            _address_promoted: address_promoted,
            _next: every.into_iter().max().unwrap_or(0) + 1,
        })
    }

    /// The decoded occurrence for this source-backed operation, if any.
    pub fn node(&self, op: &Op) -> Option<&Arc<Node>> {
        if op.source_backed { op.id.and_then(|id| self._nodes.get(&id)) } else { None }
    }

    /// How wide each value an operand-less operation names is.
    fn _widths(&self, op: &Op) -> Vec<(u32, u32)> {
        let mut out: IndexMap<u32, u32> = IndexMap::new();
        for one in op.results.iter().chain(&op.args) {
            if let Arg::Held(one) = one {
                if one.width != 0 {
                    out.insert(one.value.id, one.width);
                }
            }
        }
        let mut out: Vec<(u32, u32)> = out.into_iter().collect();
        out.sort();
        out
    }

    /// Where an operation that names no operand leaves what it writes.
    fn _idiom(&self, op: &Op, speaks: bool) -> Result<Vec<(ir::Held, Register)>, Unlowered> {
        let mut out: indexmap::IndexSet<(ir::Held, Register)> = self._selectors(op, speaks).into_iter().collect();
        out.extend(self._delivered(op)?);
        Ok(out.into_iter().collect())
    }

    /// A selector is delivered in ES by whatever writes ES.
    fn _selectors(&self, op: &Op, speaks: bool) -> Vec<(ir::Held, Register)> {
        let carried: BTreeSet<u32> = op
            .results
            .iter()
            .filter_map(|one| match one {
                Arg::Held(one) => Some(one.value.id),
                _ => None,
            })
            .collect();
        let mut out = vec![];
        for one in &op.results {
            if let Arg::Held(one) = one {
                if !speaks && self._origin.get(&one.value) == Some(&Register::ES) {
                    out.push((ir::Held { value: one.value.id, width: one.width }, Register::ES));
                }
            }
        }
        for one in &op.defines {
            if !carried.contains(&one.id)
                && !one.flags
                && self._read.contains(&one.id)
                && self._origin.get(one) == Some(&Register::ES)
            {
                out.push((ir::Held { value: one.id, width: 2 }, Register::ES));
            }
        }
        out
    }

    /// Where an operation leaves a result no ordinary destination names.
    fn _delivered(&self, op: &Op) -> Result<Vec<(ir::Held, Register)>, Unlowered> {
        if matches!(op.kind, Kind::Call | Kind::Fcompare) {
            let widths: IndexMap<u32, u32> = self._widths(op).into_iter().collect();
            let width = |id: u32| widths.get(&id).copied().unwrap_or(2);
            let mut r#where: IndexMap<Value, Register> = IndexMap::new();
            if self.node(op).is_none() {
                // A float result is on the x87, not in a register.
                let integers = op.defines.iter().filter(|one| !one.flags && widths.get(&one.id) != Some(&10));
                r#where.extend(integers.copied().zip(_RETURNED));
            }
            r#where.extend(self._origin.iter().map(|(value, register)| (*value, *register)));
            return Ok(op
                .defines
                .iter()
                .filter(|value| !value.flags && self._read.contains(&value.id))
                .filter_map(|value| {
                    r#where.get(value).map(|register| {
                        (
                            ir::Held { value: value.id, width: width(value.id) },
                            target::named(*register, i64::from(width(value.id))),
                        )
                    })
                })
                .collect());
        }
        let node = self.node(op).map(|node| &**node);
        if op.kind == Kind::Opaque && !matches!(node, Some(Node::Restore(_))) {
            return self._implicit_values(op, false);
        }
        if op.kind == Kind::Divmod {
            // A folded site's answers arrive where its record says; with ids
            // alone there is no record, and Python returns () here too.
            return Ok(vec![]);
        }
        let Some(Node::Restore(node)) = node else {
            return Ok(vec![]);
        };
        let pair = crate::model::mir::restore_pair(node.pair as i64).unwrap();
        let mut out = vec![];
        for one in &op.defines {
            if one.flags || !self._read.contains(&one.id) {
                continue;
            }
            let root = self._origin.get(one).map(|register| ir::root(*register));
            if !root.is_some_and(|root| root == pair.0 || root == pair.1) {
                return Err(Unlowered(format!(
                    "{:#06x}: the restore's {} is in no register the idiom writes",
                    op.at,
                    one.repr()
                )));
            }
            // A half, and the idiom pops one word into each.
            out.push((ir::Held { value: one.id, width: 2 }, target::named(root.unwrap(), 2)));
        }
        Ok(out)
    }

    fn _abi(&self, op: &Op, what: Option<&ir::Semantics>) -> Result<Vec<(ir::Held, Register)>, Unlowered> {
        // A cell that names its selector value is placed by allocation; one
        // emitted without an operand for it still goes through ES.
        let placed: BTreeSet<u32> = what
            .map(|what| {
                what.dests
                    .iter()
                    .chain(&what.sources)
                    .filter_map(|operand| match operand {
                        Loc::Mem(mem) => mem.selector.map(|selector| selector.value),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut out: indexmap::IndexSet<(ir::Held, Register)> = self._fixed_inputs(op)?.into_iter().collect();
        for reference in op.loads.iter().chain(&op.stores) {
            if let Some(segment) = reference.segment.filter(|segment| !placed.contains(&segment.id)) {
                out.insert((ir::Held { value: segment.id, width: 2 }, Register::ES));
            }
        }
        Ok(out.into_iter().collect())
    }

    /// Which registers a call reads its arguments in.
    fn _fixed_inputs(&self, op: &Op) -> Result<Vec<(ir::Held, Register)>, Unlowered> {
        let node = self.node(op).map(|node| &**node);
        let materialize = || Unlowered(format!("{:#06x}: return operand needs materialization", op.at));
        if op.kind == Kind::Return && node.is_none() && !op.args.is_empty() {
            // Placed by position, not by pinning the value.
            let integers = op.args.iter().filter(|arg| !matches!(arg, Arg::Held(one) if one.width == 10));
            let mut returned = vec![];
            for (arg, register) in integers.zip(_RETURNED) {
                let Arg::Held(arg) = arg else {
                    return Err(materialize());
                };
                returned.push((ir::Held { value: arg.value.id, width: arg.width }, target::named(register, i64::from(arg.width))));
            }
            return Ok(returned);
        }
        if let (Kind::Return, Some(node)) = (op.kind, node) {
            let mut returned = vec![];
            for (arg, location) in op.args.iter().zip(&node.semantics().sources) {
                if let Loc::Reg(location) = location {
                    let Arg::Held(arg) = arg else {
                        return Err(materialize());
                    };
                    returned.push((ir::Held { value: arg.value.id, width: arg.width }, location.register));
                }
            }
            return Ok(returned);
        }
        if op.kind == Kind::Opaque && !matches!(node, Some(Node::Restore(_))) {
            return self._implicit_values(op, true);
        }
        // `op.id in self._sites`: never, with ids alone.
        if let Some(Node::Restore(node)) = node {
            // The idiom reads the widened value in the pair's own register.
            let (source, _into) = crate::model::mir::restore_pair(node.pair as i64).unwrap();
            return Ok(op
                .uses
                .iter()
                .filter(|one| {
                    !one.flags
                        && !op.merges.contains_key(one)
                        && self._origin.get(*one).map(|register| ir::root(*register)) == Some(source)
                })
                .map(|one| (ir::Held { value: one.id, width: 4 }, target::named(source, 4)))
                .collect());
        }
        if op.kind != Kind::Call {
            return self._unencoded(op);
        }
        if _indirect_call(op)? {
            // An indirect call names its target as an ordinary encoded source.
            return Ok(vec![]);
        }
        let name = || self._calls.get(&op.at).cloned();
        if !op.args_known {
            return Err(Unlowered(format!(
                "{:#06x}: {}'s interface is not established",
                op.at,
                name().unwrap_or_else(|| "this call".into())
            )));
        }
        let Some(routine) = self._contracts.and_then(|contracts| contracts.get(&op.at)) else {
            return Err(Unlowered(format!("{:#06x}: no contract for {}", op.at, name().unwrap_or_else(|| "None".into()))));
        };
        if !runtime::established_inputs(routine) {
            return Err(Unlowered(format!(
                "{:#06x}: {} has no established inputs",
                op.at,
                name().unwrap_or_else(|| "None".into())
            )));
        }
        let r#where = runtime::direct_slots(routine);
        if r#where.is_empty() {
            return Ok(vec![]);
        }
        if r#where.len() != op.args.len() {
            return Err(Unlowered(format!(
                "{:#06x}: {} arguments for {} declared inputs",
                op.at,
                op.args.len(),
                r#where.len()
            )));
        }
        let mut made = vec![];
        for (one, slot) in op.args.iter().zip(r#where) {
            match one {
                Arg::Held(held) if !held.value.flags => made.push((
                    ir::Held { value: held.value.id, width: held.width },
                    crate::model::mir::as_named(slot).expect("a slot names a register"),
                )),
                _ => {
                    return Err(Unlowered(format!("{:#06x}: {} is not a value a register can hold", op.at, one.repr())));
                }
            }
        }
        Ok(made)
    }

    /// Unencoded operands of an opaque instruction still have machine locations.
    fn _implicit_values(&self, op: &Op, _inputs: bool) -> Result<Vec<(ir::Held, Register)>, Unlowered> {
        if !matches!(self.node(op).map(|node| &**node), Some(Node::Opaque(_))) {
            return Ok(vec![]);
        }
        Err(Unlowered("not yet ported: qbopt.backend.lower.Lowering._implicit_values".into()))
    }

    /// An operand the raise left opaque is emitted in BC's registers.
    fn _unencoded(&self, op: &Op) -> Result<Vec<(ir::Held, Register)>, Unlowered> {
        let mut registers = false;
        for arg in &op.args {
            if let Arg::Opaque(opaque) = arg {
                registers |= match opaque.machine_payload() {
                    Some(Loc::Mem(mem)) => mem.through != Register::None || mem.index_through != Register::None,
                    Some(Loc::Address(address)) => address.through != Register::None || address.index != Register::None,
                    _ => false,
                };
            }
        }
        if registers && self.node(op).is_some() {
            return Err(Unlowered("not yet ported: qbopt.backend.lower.Lowering._positional".into()));
        }
        Ok(vec![])
    }

    /// A value id nothing in this body already uses.
    pub fn fresh(&mut self) -> u32 {
        self._next += 1;
        self._next - 1
    }

    /// The source ranges this operation owns, resolved at the boundary.
    fn ownership(&self, op: &Op) -> Result<((i64, i64), Vec<(i64, i64)>), Unlowered> {
        let Some(occurrences) = self._occurrences else {
            // Public MIR has no `covers`; a private raising occurrence would.
            let spread = if op.absorbed.is_empty() {
                vec![]
            } else {
                op.id.and_then(|id| self._coverage.get(&id).cloned()).unwrap_or_default()
            };
            return Ok(((op.at, op.at), spread));
        };
        if op.absorbed.is_empty() {
            return Ok(((op.at, op.at), vec![]));
        }
        let missing: Vec<u32> =
            op.absorbed.iter().filter(|identity| !occurrences.contains_key(*identity)).copied().collect();
        if !missing.is_empty() {
            return Err(Unlowered(format!(
                "{:#06x}: source occurrences {} have no byte ranges",
                op.at,
                crate::support::pyrepr::tuple(&missing)
            )));
        }
        let mut spans: Vec<(i64, i64)> = op
            .absorbed
            .iter()
            .flat_map(|identity| occurrences[identity].iter().copied())
            .filter(|span| span.0 < span.1)
            .collect();
        spans.sort();
        let mut ranges: Vec<(i64, i64)> = vec![];
        for (low, high) in spans {
            match ranges.last_mut() {
                Some(last) if low <= last.1 => last.1 = last.1.max(high),
                _ => ranges.push((low, high)),
            }
        }
        if ranges.is_empty() {
            return Ok(((op.at, op.at), vec![]));
        }
        let identity_ranges = op.id.and_then(|id| occurrences.get(&id)).cloned().unwrap_or_default();
        let anchor = identity_ranges.first().map_or(op.at, |span| span.0);
        let primary = ranges.iter().find(|span| span.0 <= anchor && anchor < span.1).copied().unwrap_or(ranges[0]);
        Ok((primary, if ranges.len() > 1 { ranges } else { vec![] }))
    }

    /// The word this dword is the sign extension of, if it is one.
    pub fn sign_extended(&self, value: u32) -> Option<&Held> {
        self._extended.get(&value)
    }

    /// Whether `high` is only ever a divide's high half over that word.
    pub fn divides(&self, high: u32, word: &Held) -> bool {
        self._dividends.get(&high).is_some_and(|found| *found == word.value.id)
    }

    /// Every instruction this operation becomes, the leader first.
    pub fn expand(&mut self, op: &Op, preserve_flags: bool) -> Result<Vec<Arc<Insn>>, Unlowered> {
        let (covers, spread) = self.ownership(op)?;
        let folded_op;
        let op = if op.defines.iter().any(|one| self._folded.contains(&one.id)) {
            // The address is its cells' base and index now; see addressforms.indexed.
            let mut nothing = op.clone();
            nothing.kind = Kind::Nothing;
            nothing.name = String::new();
            nothing.args = vec![];
            nothing.results = vec![];
            nothing.defines = vec![];
            nothing.uses = vec![];
            nothing.source_backed = false;
            folded_op = nothing;
            &folded_op
        } else {
            op
        };
        let mut parts = match op.kind {
            Kind::Fill => Some(_fill(op, self)?),
            Kind::Store => _constant_store(op, self)?,
            Kind::Extract => Some(_extract(op, self)?),
            Kind::Divmod | Kind::Udivmod => _word_division(op, self)?,
            Kind::Concat => Some(_concat(op, self)?),
            Kind::Smulhi => Some(_signed_high_product(op, self)?),
            Kind::FixedMul => Some(_fixed_multiply(op, self)?),
            Kind::FixedDiv => Some(_fixed_division(op, self)?),
            Kind::PtrOffset => Some(_pointer_offset(op, self)?),
            _ => _pointer_access(op, self)?,
        };
        let or = |made: Option<Vec<ir::Semantics>>, parts: Option<Vec<ir::Semantics>>| {
            made.filter(|made| !made.is_empty()).or(parts)
        };
        parts = or(_flag_test(op, self), parts);
        if op.kind == Kind::Convert && !preserve_flags {
            parts = or(_sign_word(op), parts);
        }
        if op.kind == Kind::Mul && !preserve_flags {
            parts = or(_scaled(op, self)?, parts);
        }
        let Some(parts) = parts.filter(|parts| !parts.is_empty()) else {
            // The site's own sequence emits it, so there is nothing for this
            // to say -- while it is still that operation.
            let node = self.node(op).cloned();
            let folded = op.id.is_some_and(|id| self._absorbed.contains(&id)) && node.is_some();
            let what = if folded { None } else { current(op, Place::AsAValue, node.as_deref())? };
            let what = addressforms::scaled(addressforms::selected(what.as_ref(), &self._address_forms).as_ref(), &self._indexed);
            let speaks = what.as_ref().is_some_and(|what| !what.dests.is_empty() || !what.sources.is_empty());
            let made = if speaks { _written(&what.as_ref().unwrap().dests) } else { vec![] };
            let read = if speaks { _read(what.as_ref().unwrap()) } else { vec![] };
            let requires = self._abi(op, what.as_ref())?;
            let delivers = self._idiom(op, speaks)?;
            if speaks && op.kind != Kind::Call {
                // A register operand MIR has no value for is emitted as itself.
                let given: BTreeSet<u32> = made.iter().copied().chain(delivers.iter().map(|(held, _)| held.value)).collect();
                let lost: Vec<Value> = op
                    .defines
                    .iter()
                    .filter(|one| !one.flags && self._read.contains(&one.id) && !given.contains(&one.id))
                    .copied()
                    .collect();
                if !lost.is_empty() {
                    return Err(Unlowered(format!(
                        "{:#06x}: {} defines {} through no operand",
                        op.at,
                        what.repr(),
                        crate::support::pyrepr::list(&lost)
                    )));
                }
            }
            // A node-less call's float result and a return's float operand are in st(0).
            let floats: Vec<u32> = if node.is_none() && matches!(op.kind, Kind::Call | Kind::Return) {
                op.args
                    .iter()
                    .chain(&op.results)
                    .filter_map(|one| match one {
                        Arg::Held(one) if one.width == 10 => Some(one.value.id),
                        _ => None,
                    })
                    .collect()
            } else {
                vec![]
            };
            let st0 = || Loc::St(ir::St { index: 0 });
            let before: Vec<Arc<Insn>> = if op.kind == Kind::Return {
                floats.iter().map(|one| _follows(op, sem(Operation::FloatStore, "", vec![], vec![held(*one, 10)]))).collect()
            } else {
                vec![]
            };
            let after: Vec<Arc<Insn>> = if op.kind == Kind::Call {
                floats
                    .iter()
                    .map(|one| {
                        if self._read.contains(one) {
                            _follows(op, sem(Operation::FloatLoad, "", vec![held(*one, 10)], vec![]))
                        } else {
                            _follows(op, sem(Operation::FloatStore, "fstp", vec![st0()], vec![st0()]))
                        }
                    })
                    .collect()
            } else {
                vec![]
            };
            let inputs: Vec<u32> = if speaks {
                read
            } else {
                op.uses.iter().filter(|one| !one.flags && !floats.contains(&one.id)).map(|one| one.id).collect()
            };
            let inputs: indexmap::IndexSet<u32> =
                inputs.into_iter().chain(requires.iter().map(|(held, _)| held.value)).collect();
            let defines: Vec<u32> = if speaks && op.kind != Kind::Call {
                made.into_iter().chain(delivers.iter().map(|(held, _)| held.value)).collect::<indexmap::IndexSet<u32>>().into_iter().collect()
            } else {
                op.defines
                    .iter()
                    .filter(|one| !one.flags && self._read.contains(&one.id) && !floats.contains(&one.id))
                    .map(|one| one.id)
                    .collect()
            };
            let widths = if what.is_none() { self._widths(op) } else { vec![] };
            let mut leader = Insn::new(op.at, Some(covers), what, defines, inputs.into_iter().collect());
            leader.requires = requires;
            leader.clobbers = _clobbers(op, self._calls, self._contracts, node.as_deref());
            leader.clobbers_high = _clobbered_high(op, self._calls, self._contracts);
            leader.spread = spread;
            leader.delivers = delivers;
            leader.widths = widths;
            leader.op = Some(Arc::new(op.clone()));
            leader.node = node;
            leader.symbol = op.symbol;
            let mut out = before;
            out.push(Arc::new(leader));
            out.extend(self._caller_cleanup(op));
            out.extend(after);
            return Ok(out);
        };
        // The leader keeps the operation's identity and nothing else.
        let parts: Vec<ir::Semantics> =
            parts.iter().map(|one| addressforms::scaled(Some(one), &self._indexed).unwrap()).collect();
        let mut leader = Insn::new(op.at, Some(covers), Some(parts[0].clone()), _written(&parts[0].dests), _read(&parts[0]));
        leader.spread = spread;
        leader.op = Some(Arc::new(op.clone()));
        leader.node = self.node(op).cloned();
        let mut out = vec![Arc::new(leader)];
        out.extend(parts[1..].iter().map(|one| _follows(op, one.clone())));
        Ok(out)
    }

    /// `add sp` after a call whose contract leaves its arguments to the caller.
    fn _caller_cleanup(&self, op: &Op) -> Vec<Arc<Insn>> {
        let contract = if op.kind == Kind::Call { self._contracts.and_then(|one| one.get(&op.at)) } else { None };
        let count = contract.map_or(0, |contract| contract.caller_cleanup);
        if count == 0 {
            return vec![];
        }
        let sp = Loc::Reg(ir::Reg { register: Register::SP, width: 2 });
        vec![_follows(op, sem(Operation::Binary, "add", vec![sp.clone()], vec![sp, immediate(count, 2)]))]
    }
}

/// Python's keyword arguments to `lowered`.
#[derive(Default)]
pub struct Lowered<'a> {
    pub coverage: IndexMap<u32, Vec<(i64, i64)>>,
    pub nodes: IndexMap<u32, Arc<Node>>,
    pub occurrences: Option<&'a IndexMap<u32, Vec<(i64, i64)>>>,
    pub hints: Option<&'a AllocationHints>,
    pub pointer_model: Option<super::pointers::Model>,
    pub noreturn: bool,
}

fn recount(made: &IndexMap<i64, Vec<Arc<Insn>>>, body: &MirBody) -> IndexMap<u32, i64> {
    let mut uses: IndexMap<u32, i64> = IndexMap::new();
    for one in made.values().flatten() {
        for value in &one.uses {
            *uses.entry(*value).or_insert(0) += 1;
        }
    }
    for one in made.values().flatten() {
        for (held, _) in &one.requires {
            if !one.uses.contains(&held.value) {
                *uses.entry(held.value).or_insert(0) += 1;
            }
        }
    }
    for value in body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values()) {
        *uses.entry(value.id).or_insert(0) += 1;
    }
    uses
}

/// One MIR body as machine instructions, and nothing else.
pub fn lowered(
    name: &str,
    body: &MirBody,
    calls: Option<&IndexMap<i64, String>>,
    absorbed: BTreeSet<u32>,
    contracts: Option<&IndexMap<i64, runtime::Contract>>,
    cpu: impl Into<cpu::ProfileOrName<'static>>,
    options: Lowered,
) -> Result<lir::LirBody, Unlowered> {
    // Width does not identify a type here: the C path runs lower_int64 first.
    let default_hints = AllocationHints::new();
    let hints = options.hints.unwrap_or(&default_hints);
    let body = lower_switches::expanded(body).map_err(Unlowered)?;
    let body = named(&body)?;
    lower_floats::checked(&body)?;
    let roots: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| &block.phis).map(|phi| phi.result).filter(|value| !value.flags).collect();
    let body = ssa::pruned_phis(&body, &roots);
    let values: PySet<Value> = ssa::values(&body).collect();
    let origin: IndexMap<Value, Register> =
        values.iter().filter_map(|value| hints.origin_of(*value).map(|r#where| (*value, r#where))).collect();
    let mut pins: IndexMap<Value, Register> = IndexMap::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        for (index, value) in op.defines.iter().enumerate() {
            if let Some(r#where) = hints.pin_of(op, index) {
                pins.insert(*value, r#where);
            }
        }
    }

    // What anything reads, so a definition nothing reads can become what it
    // always was: a statement that the register is destroyed.
    let mut read: BTreeSet<u32> =
        body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| &op.uses).map(|one| one.id).collect();
    read.extend(body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values()).map(|v| v.id));
    let no_calls = IndexMap::new();
    let calls = calls.unwrap_or(&no_calls);
    let mut making = Lowering::new(
        &body,
        read,
        calls,
        absorbed,
        contracts,
        cpu,
        Options {
            coverage: options.coverage,
            nodes: options.nodes,
            occurrences: options.occurrences,
            origin: origin.clone(),
            pointer_model: options.pointer_model,
        },
    )?;
    let mut readers: std::collections::HashMap<Value, usize> = std::collections::HashMap::new();
    for value in body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| &op.uses) {
        *readers.entry(*value).or_insert(0) += 1;
    }
    for value in body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values()) {
        *readers.entry(*value).or_insert(0) += 1;
    }
    let live = liveness::live(&body);
    let scheduled: IndexMap<i64, Vec<Op>> =
        body.blocks.iter().map(|block| (block.at, _branch_condition(block, &readers))).collect();
    for block in &body.blocks {
        _check_inserted_conditions(&scheduled[&block.at], &live.live_out[&block.at])?;
    }
    let mut made: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::new();
    for block in &body.blocks {
        let ops = &scheduled[&block.at];
        let mut alive: BTreeSet<Value> = live.live_out[&block.at].iter().filter(|value| value.flags).copied().collect();
        let mut preserve: BTreeSet<usize> = BTreeSet::new();
        for (index, op) in ops.iter().enumerate().rev() {
            if !alive.is_empty() {
                preserve.insert(index);
            }
            for one in &op.defines {
                alive.remove(one);
            }
            alive.extend(op.uses.iter().filter(|value| value.flags).copied());
        }
        let mut insns = vec![];
        for (index, op) in ops.iter().enumerate() {
            insns.extend(making.expand(op, preserve.contains(&index))?);
        }
        made.insert(block.at, insns);
    }
    // A far-pointer field is two language-visible word loads but one target instruction.
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, farload::selected(&insns))).collect();
    let promoted = making._address_promoted.clone();
    let made = addressforms::promote(&made, &promoted, &mut || making.fresh()).map_err(Unlowered)?;
    let uses = recount(&made, &body);
    // A one-use comparison load is a legal memory operand.
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, comparefold::selected(&insns, &uses, &making._exposed))).collect();
    let uses = recount(&made, &body);
    // x86 can express a C read-modify-write update in one memory operand.
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, rmw::selected(&insns, &uses))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, _memory_arguments(&insns, &uses, &making._exposed))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> =
        made.into_iter().map(|(at, insns)| (at, _immediate_arguments(&insns, &uses))).collect();
    let made: IndexMap<i64, Vec<Arc<Insn>>> = made
        .into_iter()
        .map(|(at, insns)| (at, _rematerialized_arguments(&insns, &uses, &making._exposed)))
        .collect();
    let live = _phis_worth_keeping(&body, &made);
    let facts = consts::known(&body);
    let mut trip_counts: Vec<(i64, i64)> = loops::loops(&body.blocks, Some(body.entry))
        .iter()
        .filter_map(|one| {
            induction::trip_count(&body, one, &facts)
                .map(|count| (one.header, count.to_i64().expect("a trip count fits an int64")))
        })
        .collect();
    trip_counts.sort();
    let blocks = body
        .blocks
        .iter()
        .map(|block| lir::LirBlock {
            at: block.at,
            insns: made[&block.at].clone(),
            succ: block.succ.clone(),
            phis: block
                .phis
                .iter()
                .filter(|phi| !phi.result.flags && live.contains(&phi.result.id))
                .map(|phi| lir::Phi {
                    result: phi.result.id,
                    incoming: phi.incoming.iter().map(|(at, value)| (*at, value.id)).collect(),
                })
                .collect(),
        })
        .collect();
    let mut all_pins: IndexMap<u32, Register> = pins.iter().map(|(value, r#where)| (value.id, *r#where)).collect();
    for value in values.iter() {
        if origin.get(value) == Some(&Register::ES) {
            all_pins.insert(value.id, Register::ES);
        }
    }
    let mut out = lir::LirBody::new(
        name,
        body.entry,
        blocks,
        origin.iter().map(|(value, r#where)| (value.id, *r#where)).collect(),
        all_pins,
    );
    out.noreturn = options.noreturn;
    out.inputs = liveness::entry_values(&body).into_iter().filter(|value| !value.flags).map(|value| value.id).collect();
    out.loop_trip_counts = trip_counts;
    out.ordered = true;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::mir::MirBlock;

    /// `_NAMED` extends `_MACHINE`: a raised load named nothing, so every C
    /// body refused with "no instruction for load".
    #[test]
    fn test_named_gives_machine_kinds_their_instruction() {
        let mut op = Op::new(2, None, "", vec![], vec![]);
        op.kind = Kind::Load;
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![op], vec![])]);
        let named = named(&body).unwrap();
        let op = &named.blocks[0].ops[0];
        assert_eq!((op.op, op.name.as_str()), (Some(OpCode::Operation(Operation::Move)), "mov"));
    }
    fn switched() -> MirBody {
        let selector = Value { variable: 1, version: 1, ..Value::new(1, 0) };
        let merged = Value { variable: 2, version: 1, ..Value::new(2, 20) };
        let mut op = Op::new(5, OpCode::Operation(Operation::Jump), "", vec![], vec![selector]);
        op.kind = Kind::Switch;
        op.args = vec![Arg::Held(Held { value: selector, width: 2 })];
        op.target = Some(30);
        op.cases = vec![(1, 20), (2, 20), (3, 30)];
        op.absorbed = vec![5];
        let phi = crate::model::mir::Phi { result: merged, incoming: [(0, selector)].into_iter().collect() };
        MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![op], vec![20, 30]),
                MirBlock::new(20, vec![phi], vec![], vec![]),
                MirBlock::new(30, vec![], vec![], vec![]),
            ],
        )
    }

    fn occurrences() -> IndexMap<u32, Vec<(i64, i64)>> {
        [(5, vec![(5, 12)])].into_iter().collect()
    }

    fn lower(name: &str, body: &MirBody) -> Result<lir::LirBody, Unlowered> {
        let occurrences = occurrences();
        let contracts = IndexMap::new();
        lowered(
            name,
            body,
            Some(&IndexMap::new()),
            BTreeSet::new(),
            Some(&contracts),
            "386",
            Lowered { occurrences: Some(&occurrences), ..Default::default() },
        )
    }

    #[test]
    fn test_switch_comparisons_cannot_overwrite_a_live_condition() {
        let mut body = switched();
        let flags = Value { flags: true, ..Value::new(9, 0) };
        let mut branch = Op::new(20, OpCode::Operation(Operation::Branch), "", vec![], vec![flags]);
        branch.kind = Kind::Branch;
        branch.test = Some(Kind::Eq);
        branch.target = Some(30);
        body.blocks[1].ops = vec![branch];
        body.blocks[1].succ = vec![30];
        let error = lower("condition", &body).unwrap_err();
        assert!(error.0.contains("live condition"), "{error}");
    }

    #[test]
    fn test_lowering_consumes_switches_as_compare_and_branch_operations() {
        let body = lower("switch", &switched()).unwrap();
        let names: Vec<Option<String>> = body
            .blocks
            .iter()
            .flat_map(|block| &block.insns)
            .map(|insn| insn.what.as_ref().and_then(|what| what.name.clone()))
            .collect();
        assert_eq!(names.iter().filter(|name| name.as_deref() == Some("cmp")).count(), 2);
        assert_eq!(names.iter().filter(|name| name.as_deref() == Some("je")).count(), 2);
    }

    #[test]
    fn test_lowered_switch_comparisons_encode_after_allocation() {
        let body = lower("switch", &switched()).unwrap();
        let pins: IndexMap<u32, Register> = [(1, Register::EAX)].into_iter().collect();
        let assignment = super::super::allocate::allocate(&body, Some(&pins), None, None, None, "386".into()).unwrap();
        assert!(assignment.spilled.is_empty());
        let body = super::super::allocate::applied(&body, &assignment).unwrap();
        let mut encoded: Vec<Vec<u8>> = Vec::new();
        for insn in body.insns() {
            if let Some(what) = insn.what.as_ref().filter(|what| what.name.as_deref() == Some("cmp")) {
                let result = super::super::select::emit(what, 0, None, false, false, None);
                assert!(result.is_some());
                encoded.push(result.unwrap().code);
            }
        }
        assert_eq!(encoded, [vec![0x83, 0xF8, 0x01], vec![0x83, 0xF8, 0x02]]);
    }

    #[test]
    fn test_constant_switch_emits_only_a_jump() {
        for (value, target) in [(1, 20), (2, 20), (3, 30), (0, 30), (65537, 20)] {
            let mut body = switched();
            let op = &mut body.blocks[0].ops[0];
            op.args = vec![Arg::Const(crate::model::mir::Const::new(value, 2))];
            op.uses = vec![];
            let lowered = lower("constant", &body).unwrap();
            let [jump] = &lowered.blocks[0].insns[..] else { panic!("{value}: not one instruction") };
            assert_eq!(jump.what.as_ref().unwrap().name.as_deref(), Some("jmp"));
            assert_eq!(lowered.blocks[0].succ, [target], "{value}");
        }
    }

    #[test]
    fn test_invalid_switch_is_rejected_atomically() {
        for invalid in ["duplicate", "missing", "effects", "successors"] {
            let mut body = switched();
            let block = &mut body.blocks[0];
            match invalid {
                "duplicate" => block.ops[0].cases = vec![(1, 20), (65537, 30)],
                "missing" => block.ops[0].target = Some(40),
                "effects" => block.ops[0].defines = vec![Value::new(9, 5)],
                _ => block.succ = vec![20],
            }
            assert!(lower("invalid", &body).is_err(), "{invalid}");
            assert_eq!(switched(), switched());
        }
    }

    fn pointer_offset() -> Op {
        let (base, offset, result) = (Value::new(1, 0), Value::new(2, 0), Value::new(3, 0));
        let mut op = Op::new(0, OpCode::Operation(Operation::Nothing), "", vec![result], vec![base, offset]);
        op.kind = Kind::PtrOffset;
        op.args = vec![Arg::Held(Held { value: base, width: 4 }), Arg::Held(Held { value: offset, width: 4 })];
        op.results = vec![Arg::Held(Held { value: result, width: 4 })];
        op
    }

    /// A huge INTEGER at byte 65536 must advance the selector, not read element zero again.
    #[test]
    fn test_dos_pointer_offset_carries_and_borrows() {
        for (pointer, offset, expected) in [
            (0x2000_0000, 0, 0x2000_0000),
            (0x2000_fffe, 2, 0x3000_0000),
            (0x2000_0000, 0x20002, 0x4000_0002),
            (0x2000_0004, -8, 0x1000_fffc),
            (0xf000_fffe_i64, 2, 0x0000_0000),
        ] {
            let op = pointer_offset();
            let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![op.clone()], vec![])]);
            let calls = IndexMap::new();
            let model = super::super::pointers::Model::new(super::super::pointers::HugeShift::Fixed(12)).unwrap();
            let mut making = Lowering::new(
                &body,
                BTreeSet::from([1, 2, 3]),
                &calls,
                BTreeSet::new(),
                None,
                "386",
                Options { pointer_model: Some(model), ..Default::default() },
            )
            .unwrap();
            let parts: Vec<ir::Semantics> =
                making.expand(&op, true).unwrap().iter().map(|part| part.what.clone().unwrap()).collect();
            let none = std::collections::HashMap::new();
            let got = super::super::pointers::tests::execute(&parts, pointer, offset & 0xffff_ffff, &none);
            assert_eq!(got, expected);
            assert!(parts
                .iter()
                .flat_map(|part| part.sources.iter().chain(&part.dests))
                .all(|arg| matches!(arg, Loc::Held(_) | Loc::Imm(_))));
        }
    }

    #[test]
    fn test_pointer_abi_is_not_inferred_from_cpu() {
        let op = pointer_offset();
        let body = MirBody::new(0, vec![MirBlock::new(0, vec![], vec![op.clone()], vec![])]);
        let calls = IndexMap::new();
        let mut making =
            Lowering::new(&body, BTreeSet::from([3]), &calls, BTreeSet::new(), None, "386", Options::default()).unwrap();
        assert!(making.expand(&op, true).unwrap_err().0.contains("pointer ABI"));
    }

    #[test]
    fn test_pointer_lowering_cannot_destroy_an_unrelated_live_condition() {
        let leaving = BTreeSet::from([Value { flags: true, ..Value::new(4, 0) }]);
        assert!(_check_inserted_conditions(&[pointer_offset()], &leaving).unwrap_err().0.contains("live condition"));
    }
}
