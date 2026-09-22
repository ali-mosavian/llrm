//! Port of `qbopt/frontend/pairs.py`.
//!
//! BC's two long register pairs, found over values instead of over bytes.
//! It answers "are these two ops the halves of one 32-bit value" for the
//! raise-side scalar recognizer; nothing here widens anything.

use iced_x86::Register;

use crate::backend::lower::{self, Place, Unlowered};
use crate::model::ir::{self, Loc, Operation, root};
use crate::model::mir::{self, MemRef, Op, OrderedMap, RaisedBody, Value};

/// BC's own two, in lift.py's numbering and `mir::restore_pair`'s: pair 0 is
/// ax:dx and pair 1 is cx:bx, low half first.
pub const PAIRS: [(i64, (Register, Register)); 2] =
    [(0, (Register::EAX, Register::EDX)), (1, (Register::ECX, Register::EBX))];

pub const HALF: u32 = 2;

type Origin = OrderedMap<Value, Register>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Load,
    Store,
    /// against memory
    Alu,
    /// against an immediate
    AluImm,
    /// against the other pair
    AluReg,
    Not,
    /// pair to pair
    Move,
    /// BC's three-instruction negate
    Neg,
    /// an INTEGER sign-extended into a pair
    Movsx,
}

impl Kind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Kind::Load => "load",
            Kind::Store => "store",
            Kind::Alu => "alu",
            Kind::AluImm => "alu-i",
            Kind::AluReg => "alu-v",
            Kind::Not => "not",
            Kind::Move => "move",
            Kind::Neg => "neg",
            Kind::Movsx => "movsx",
        }
    }
}

/// Two ops that are the halves of one 32-bit value.
#[derive(Clone, Copy, Debug)]
pub struct Pair<'a> {
    pub kind: Kind,
    /// which of BC's two
    pub pair: i64,
    pub low: &'a Op,
    pub high: &'a Op,
}

impl<'a> Pair<'a> {
    pub fn at(&self) -> (i64, i64) {
        (self.low.at, self.high.at)
    }

    pub fn first(&self) -> &'a Op {
        if self.high.at < self.low.at { self.high } else { self.low }
    }
}

/// This op's semantics if it is a plain move, or None.
fn _moved(op: &Op) -> Result<Option<ir::Semantics>, Unlowered> {
    let Some(what) = _shape(op)? else {
        return Ok(None);
    };
    if what.op != Operation::Move {
        return Ok(None);
    }
    Ok((what.dests.len() == 1 && what.sources.len() == 1).then_some(what))
}

/// (which pair, which half) for a register, or None if it is in neither.
pub fn _half_of(register: Register, _origin: &Origin) -> Option<(i64, i64)> {
    for (number, (low, high)) in PAIRS {
        if register == low {
            return Some((number, 0));
        }
        if register == high {
            return Some((number, 1));
        }
    }
    None
}

type Half = for<'o> fn(&'o Op) -> Result<Option<(Register, &'o MemRef)>, Unlowered>;

/// (destination root, cell) for `mov <half>,[x]`, or None.
fn _loads_from(op: &Op) -> Result<Option<(Register, &MemRef)>, Unlowered> {
    let Some(what) = _moved(op)? else {
        return Ok(None);
    };
    if op.loads.len() != 1 || !op.stores.is_empty() || op.loads[0].addr.is_none() {
        return Ok(None);
    }
    match &what.dests[0] {
        Loc::Reg(dest) if dest.width == HALF => Ok(Some((root(dest.register), &op.loads[0]))),
        _ => Ok(None),
    }
}

/// (source root, cell) for `mov [x],<half>`, or None.
fn _stores_to(op: &Op) -> Result<Option<(Register, &MemRef)>, Unlowered> {
    let Some(what) = _moved(op)? else {
        return Ok(None);
    };
    if op.stores.len() != 1 || !op.loads.is_empty() || op.stores[0].addr.is_none() {
        return Ok(None);
    }
    match &what.sources[0] {
        Loc::Reg(source) if source.width == HALF => Ok(Some((root(source.register), &op.stores[0]))),
        _ => Ok(None),
    }
}

/// Whether `high` names the two bytes just above `low`: lift.py's own `+2`.
pub fn _adjacent(low: &MemRef, high: &MemRef) -> bool {
    let (Some(a), Some(b)) = (low.addr, high.addr) else {
        return false;
    };
    if low.width != HALF || high.width != HALF {
        return false;
    }
    a.plus(HALF as i64) == b
}

/// The two ops as one pair, if they are one, in either order.
fn _matched<'a>(first: &'a Op, second: &'a Op, read: Half, origin: &Origin) -> Result<Option<Pair<'a>>, Unlowered> {
    let (Some(a), Some(b)) = (read(first)?, read(second)?) else {
        return Ok(None);
    };
    let kind = if std::ptr::fn_addr_eq(read, _loads_from as Half) { Kind::Load } else { Kind::Store };
    for ((one, cell), (other, next_cell), low_op, high_op) in [(a, b, first, second), (b, a, second, first)] {
        let (Some(low_at), Some(high_at)) = (_half_of(one, origin), _half_of(other, origin)) else {
            continue;
        };
        if low_at.0 != high_at.0 || low_at.1 != 0 || high_at.1 != 1 {
            continue;
        }
        if !_adjacent(cell, next_cell) {
            continue;
        }
        return Ok(Some(Pair { kind, pair: low_at.0, low: low_op, high: high_op }));
    }
    Ok(None)
}

/// Every adjacent load or store pair in `body`.
pub fn found(body: &RaisedBody) -> Result<Vec<Pair<'_>>, Unlowered> {
    let mut out = Vec::new();
    for block in &body.blocks {
        let ops = &block.ops;
        for (index, (first, second)) in ops.iter().zip(ops.iter().skip(1)).enumerate() {
            if let Some(made) = _negate(ops, index, &body.origin)? {
                out.push(made);
                continue;
            }
            if let Some(made) = _sign_extended(first, second, &body.origin)? {
                out.push(made);
                continue;
            }
            let mut made = _alu_adjacent(first, second, &body.origin)?;
            if made.is_none() {
                made = _paired_alu(first, second, &body.origin, Want::Imm, Kind::AluImm)?;
            }
            if made.is_none() {
                made = _paired_alu(first, second, &body.origin, Want::Reg, Kind::AluReg)?;
            }
            if made.is_none() {
                made = _unary_pair(first, second, &body.origin)?;
            }
            if made.is_none() {
                made = _move_pair(first, second, &body.origin)?;
            }
            if let Some(made) = made {
                out.push(made);
                continue;
            }
            for read in [_loads_from as Half, _stores_to as Half] {
                if let Some(made) = _matched(first, second, read, &body.origin)? {
                    out.push(made);
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// What the high half's mnemonic must be, given the low half's. Only add
/// and sub carry.
pub const PARTNER: [(&str, &str); 5] = [("and", "and"), ("or", "or"), ("xor", "xor"), ("add", "adc"), ("sub", "sbb")];

/// (destination root, mnemonic, cell) for `<alu> <half>,[x]`, or None.
fn _binary_on(op: &Op) -> Result<Option<(Register, String, &MemRef)>, Unlowered> {
    let Some(what) = _shape(op)? else {
        return Ok(None);
    };
    if what.op != Operation::Binary {
        return Ok(None);
    }
    if what.dests.len() != 1 || op.loads.len() != 1 || !op.stores.is_empty() || op.loads[0].addr.is_none() {
        return Ok(None);
    }
    let dest = &what.dests[0];
    match dest {
        Loc::Reg(reg) if reg.width == HALF && what.sources.contains(dest) => {
            Ok(Some((root(reg.register), what.name.clone().unwrap_or_default(), &op.loads[0])))
        }
        _ => Ok(None),
    }
}

/// An arithmetic pair recognised the way a load pair is.
fn _alu_adjacent<'a>(first: &'a Op, second: &'a Op, origin: &Origin) -> Result<Option<Pair<'a>>, Unlowered> {
    let (Some(a), Some(b)) = (_binary_on(first)?, _binary_on(second)?) else {
        return Ok(None);
    };
    let ((one, low_name, cell), (other, high_name, next_cell)) = (a, b);
    let (Some(low_at), Some(high_at)) = (_half_of(one, origin), _half_of(other, origin)) else {
        return Ok(None);
    };
    if low_at.0 != high_at.0 || low_at.1 != 0 || high_at.1 != 1 {
        return Ok(None);
    }
    if !PARTNER.contains(&(low_name.as_str(), high_name.as_str())) || !_adjacent(cell, next_cell) {
        return Ok(None);
    }
    Ok(Some(Pair { kind: Kind::Alu, pair: low_at.0, low: first, high: second }))
}

/// (pair number, low op, high op) where these two write one pair's halves.
fn _halves_named<'a>(first: &'a Op, second: &'a Op, origin: &Origin) -> Option<(i64, &'a Op, &'a Op)> {
    for (one, other, low_op, high_op) in [(first, second, first, second), (second, first, second, first)] {
        let low = _half_of(_written(one, origin), origin);
        let high = _half_of(_written(other, origin), origin);
        let (Some(low), Some(high)) = (low, high) else {
            continue;
        };
        if low.0 == high.0 && low.1 == 0 && high.1 == 1 {
            return Some((low.0, low_op, high_op));
        }
    }
    None
}

/// The one tracked register this op writes, or NONE.
fn _written(op: &Op, origin: &Origin) -> Register {
    let made: Vec<&Value> = op.defines.iter().filter(|one| !one.flags).collect();
    if made.len() != 1 {
        return Register::None;
    }
    origin.get(made[0]).copied().unwrap_or(Register::None)
}

fn _shape(op: &Op) -> Result<Option<ir::Semantics>, Unlowered> {
    if op.floating_origin.is_some() {
        return Ok(None);
    }
    lower::current(op, Place::Default, op.node().map(|node| &**node))
}

/// Python's `want` type argument to `_binary_against`: `ir.Imm` or `ir.Reg`.
#[derive(Clone, Copy)]
enum Want {
    Imm,
    Reg,
}

/// This op's mnemonic if it is `<alu> <half>,<want>` in place, else None.
fn _binary_against(op: &Op, want: Want) -> Result<Option<String>, Unlowered> {
    let Some(what) = _shape(op)? else {
        return Ok(None);
    };
    if what.op != Operation::Binary || what.dests.len() != 1 {
        return Ok(None);
    }
    let dest = &what.dests[0];
    if !matches!(dest, Loc::Reg(reg) if reg.width == HALF) || !what.sources.contains(dest) {
        return Ok(None);
    }
    if !op.loads.is_empty() || !op.stores.is_empty() {
        return Ok(None);
    }
    let wanted = |one: &Loc| match want {
        Want::Imm => matches!(one, Loc::Imm(_)),
        Want::Reg => matches!(one, Loc::Reg(_)),
    };
    if !what.sources.iter().any(wanted) {
        return Ok(None);
    }
    Ok(Some(what.name.unwrap_or_default()))
}

/// An arithmetic pair whose operand is not memory.
fn _paired_alu<'a>(
    first: &'a Op,
    second: &'a Op,
    origin: &Origin,
    want: Want,
    kind: Kind,
) -> Result<Option<Pair<'a>>, Unlowered> {
    let Some((number, low_op, high_op)) = _halves_named(first, second, origin) else {
        return Ok(None);
    };
    let (low_name, high_name) = (_binary_against(low_op, want)?, _binary_against(high_op, want)?);
    let (Some(low_name), Some(high_name)) = (low_name, high_name) else {
        return Ok(None);
    };
    if !PARTNER.contains(&(low_name.as_str(), high_name.as_str())) {
        return Ok(None);
    }
    Ok(Some(Pair { kind, pair: number, low: low_op, high: high_op }))
}

/// `not ax` with `not dx` -- one 32-bit not, and BC's own shape for it.
fn _unary_pair<'a>(first: &'a Op, second: &'a Op, origin: &Origin) -> Result<Option<Pair<'a>>, Unlowered> {
    let Some((number, low_op, high_op)) = _halves_named(first, second, origin) else {
        return Ok(None);
    };
    let mut names = Vec::new();
    for op in [low_op, high_op] {
        let what = _shape(op)?;
        match what {
            Some(what) if what.op == Operation::Unary && op.loads.is_empty() && op.stores.is_empty() => {
                names.push(what.name.unwrap_or_default());
            }
            _ => return Ok(None),
        }
    }
    if names[0] != "not" || names[1] != "not" {
        return Ok(None);
    }
    Ok(Some(Pair { kind: Kind::Not, pair: number, low: low_op, high: high_op }))
}

/// `mov ax,cx` with `mov dx,bx` -- one pair copied into the other.
fn _move_pair<'a>(first: &'a Op, second: &'a Op, origin: &Origin) -> Result<Option<Pair<'a>>, Unlowered> {
    let Some((number, low_op, high_op)) = _halves_named(first, second, origin) else {
        return Ok(None);
    };
    let mut sources = Vec::new();
    for op in [low_op, high_op] {
        let Some(what) = _shape(op)? else {
            return Ok(None);
        };
        if what.op != Operation::Move || !op.loads.is_empty() || !op.stores.is_empty() {
            return Ok(None);
        }
        if what.sources.len() != 1 {
            return Ok(None);
        }
        let Loc::Reg(source) = &what.sources[0] else {
            return Ok(None);
        };
        if source.width != HALF {
            return Ok(None);
        }
        sources.push(_half_of(root(source.register), origin));
    }
    let (Some(low), Some(high)) = (sources[0], sources[1]) else {
        return Ok(None);
    };
    if low.0 != high.0 || low.1 != 0 || high.1 != 1 {
        return Ok(None);
    }
    if low.0 == number {
        return Ok(None); // a pair copied onto itself is not a move between pairs
    }
    Ok(Some(Pair { kind: Kind::Move, pair: number, low: low_op, high: high_op }))
}

/// `neg ax / adc dx,0 / neg dx` -- BC's own three-instruction long negate.
pub fn _negate<'a>(ops: &'a [Op], index: usize, origin: &Origin) -> Result<Option<Pair<'a>>, Unlowered> {
    if index + 2 >= ops.len() {
        return Ok(None);
    }
    let (first, middle, last) = (&ops[index], &ops[index + 1], &ops[index + 2]);
    let low = _half_of(_written(first, origin), origin);
    let high = _half_of(_written(last, origin), origin);
    let (Some(low), Some(high)) = (low, high) else {
        return Ok(None);
    };
    if low.0 != high.0 || low.1 != 0 || high.1 != 1 {
        return Ok(None);
    }
    let mut shapes = Vec::new();
    for one in [first, middle, last] {
        shapes.push(_shape(one)?);
    }
    if shapes.iter().any(Option::is_none) {
        return Ok(None);
    }
    let shapes: Vec<ir::Semantics> = shapes.into_iter().flatten().collect();
    let named = |what: &ir::Semantics, op: Operation, name: &str| {
        what.op == op && what.name.as_deref().unwrap_or("") == name
    };
    if !named(&shapes[0], Operation::Unary, "neg") {
        return Ok(None);
    }
    if !named(&shapes[2], Operation::Unary, "neg") {
        return Ok(None);
    }
    if !named(&shapes[1], Operation::Binary, "adc") {
        return Ok(None);
    }
    if _half_of(_written(middle, origin), origin) != Some((low.0, 1)) {
        return Ok(None);
    }
    Ok(Some(Pair { kind: Kind::Neg, pair: low.0, low: first, high: last }))
}

/// `mov ax,<source>` then `cwd` -- an INTEGER widened into pair 0.
pub fn _sign_extended<'a>(first: &'a Op, second: &'a Op, origin: &Origin) -> Result<Option<Pair<'a>>, Unlowered> {
    let low = _half_of(_written(first, origin), origin);
    let high = _half_of(_written(second, origin), origin);
    if low != Some((0, 0)) || high != Some((0, 1)) {
        return Ok(None);
    }
    let Some(made) = _shape(first)? else {
        return Ok(None);
    };
    if made.op != Operation::Move || made.sources.len() != 1 {
        return Ok(None);
    }
    if matches!(made.sources[0], Loc::Imm(_)) {
        return Ok(None); // calls.widened_constant_at()'s own, narrower shape
    }
    if let Loc::Reg(source) = &made.sources[0] {
        if mir::PHYSICAL.contains(&source.register) {
            // `mov ax,es / cwd` is the shape and not the meaning: a segment
            // register is not a value here (mir::PHYSICAL says so).
            return Ok(None);
        }
    }
    let widening = _shape(second)?;
    if widening.is_none_or(|widening| widening.name.as_deref().unwrap_or("") != "cwd") {
        return Ok(None);
    }
    Ok(Some(Pair { kind: Kind::Movsx, pair: 0, low: first, high: second }))
}

#[cfg(test)]
#[path = "pairs_tests.rs"]
mod tests;
