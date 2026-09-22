//! Port of `qbopt/backend/lower.py`: MIR to machine form.
//!
//! Ported so far: the operand and naming half, up to `lowered`.

use std::fmt;

use iced_x86::Register;
use num_traits::ToPrimitive;

use crate::model::floating::{Format, Rounding};
use crate::model::ir::nodes::Node;
use crate::model::ir::{self, Loc, Operation};
use crate::model::mir::{Arg, Kind, MemRef, MirBody, Op, OpCode};
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
