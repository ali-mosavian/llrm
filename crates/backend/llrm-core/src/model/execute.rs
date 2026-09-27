//! Small executable reference semantics for integer MIR.
//!
//! Port of `qbopt/model/execute.py`. An oracle for pass tests: run a body
//! before and after a transform on the same inputs and compare what each
//! returns and stores. Anything not modelled raises rather than guessing.
//!
//! Memory is bytes keyed by segment and 16-bit offset, as the machine keys
//! them; `_where` derives both for every reference. A far reference's segment
//! is its selector's value. Otherwise the frame is SS, with `bp` zero, and a
//! near reference is DS -- or SS where it says it points into the frame. With
//! the stack in data, SS is DS. A named segment in DGROUP is DS at its base
//! offset; any other named cell is its own segment.

// An oracle for tests; nothing in the compiler runs it.
#![cfg_attr(not(test), allow(dead_code))]

use std::collections::BTreeMap;
use std::fmt;

use iced_x86::Register;
use crate::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

use crate::model::mir::{self, Arg, Kind, MemRef, MirBody, Op, Value};
use crate::objectfile::module::Space;
use crate::support::pyrepr::Repr;

/// Python's segment object: `DS`, `SS`, a far selector's number, or a named `(space, index)`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Segment {
    Ds,
    Ss,
    Selector(BigInt),
    Named(Space, i64),
}

/// The body uses something this executor does not model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionError(pub String);

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExecutionError {}

fn error<T>(message: String) -> Result<T, ExecutionError> {
    Err(ExecutionError(message))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct _Flags {
    kind: Kind,
    left: BigInt,
    right: BigInt,
    result: BigInt,
    width: u32,
}

/// Python's `int | _Flags` value slot.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Stored {
    Integer(BigInt),
    Flags(_Flags),
}

pub type Memory = IndexMap<(Segment, i64), u8>;

pub struct State {
    values: IndexMap<Value, Stored>,
    memory: Memory,
    steps: u64,
    stack: Segment,
    // A DGROUP segment's index and where it starts in DS.
    dgroup: BTreeMap<i64, i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Outcome {
    pub returned: Vec<BigInt>,
    pub memory: Memory,
    pub steps: u64,
}

pub type Call<'a> = dyn FnMut(&Op, &[BigInt]) -> Result<Vec<BigInt>, ExecutionError> + 'a;

fn _mask(n: &BigInt, width: u32) -> BigInt {
    n & ((BigInt::from(1_u8) << (8 * width)) - 1)
}

fn _signed(n: &BigInt, width: u32) -> BigInt {
    let n = _mask(n, width);
    if (&n >> (8 * width - 1)).is_zero() { n } else { n - (BigInt::from(1_u8) << (8 * width)) }
}

fn _compare(test: Kind, left: &BigInt, right: &BigInt, width: u32) -> Result<bool, ExecutionError> {
    let (a, b) = (_signed(left, width), _signed(right, width));
    let (u, v) = (_mask(left, width), _mask(right, width));
    Ok(match test {
        Kind::Eq => u == v,
        Kind::Ne => u != v,
        Kind::Lt => a < b,
        Kind::Le => a <= b,
        Kind::Gt => a > b,
        Kind::Ge => a >= b,
        Kind::Below => u < v,
        Kind::BelowEq => u <= v,
        Kind::Above => u > v,
        Kind::AboveEq => u >= v,
        _ => return error(format!("no comparison {test}")),
    })
}

fn _taken(flags: &_Flags, test: Kind) -> Result<bool, ExecutionError> {
    if flags.kind == Kind::Sub {
        return _compare(test, &flags.left, &flags.right, flags.width);
    }
    // Logic leaves carry and overflow clear: every test is the result against zero.
    if matches!(flags.kind, Kind::And | Kind::Or | Kind::Xor) || matches!(test, Kind::Eq | Kind::Ne) {
        return _compare(test, &flags.result, &BigInt::zero(), flags.width);
    }
    error(format!("{test} on the flags of {}", flags.kind))
}

/// The segment and offset of the first byte `ref` names.
fn _where(r#ref: &MemRef, state: &State) -> Result<(Segment, i64), ExecutionError> {
    let Some(addr) = &r#ref.addr else {
        return error(format!("no address for {}", r#ref.repr()));
    };
    let mut offset = BigInt::from(addr.disp)
        + match r#ref.base {
            Some(base) => _integer(state, base)?,
            None => BigInt::zero(),
        };
    let short = |offset: &BigInt| _mask(offset, 2).to_i64().expect("16 bits");
    if let Some(segment) = r#ref.segment {
        return Ok((Segment::Selector(_integer(state, segment)?), short(&offset)));
    }
    let segment = match addr.space {
        Space::Frame => state.stack.clone(),
        Space::Literal => {
            let framed = r#ref.space == Some(Space::Frame) || addr.segment == Register::SS;
            if framed { state.stack.clone() } else { Segment::Ds }
        }
        Space::Segment if state.dgroup.contains_key(&addr.index) => {
            offset += state.dgroup[&addr.index];
            Segment::Ds
        }
        Space::Segment | Space::External => Segment::Named(addr.space, addr.index),
        _ => return error(format!("no segment for {}", r#ref.repr())),
    };
    Ok((segment, short(&offset)))
}

fn _load(state: &State, r#ref: &MemRef, width: u32) -> Result<BigInt, ExecutionError> {
    let (region, offset) = _where(r#ref, state)?;
    Ok((0..width)
        .map(|at| {
            let byte = state.memory.get(&(region.clone(), (offset + i64::from(at)) & 0xFFFF)).copied().unwrap_or(0);
            BigInt::from(byte) << (8 * at)
        })
        .sum())
}

fn _store(state: &mut State, r#ref: &MemRef, n: &BigInt, width: u32) -> Result<(), ExecutionError> {
    let (region, offset) = _where(r#ref, state)?;
    for at in 0..width {
        let byte = ((n >> (8 * at)) & BigInt::from(0xFF_u8)).to_u8().expect("a byte");
        state.memory.insert((region.clone(), (offset + i64::from(at)) & 0xFFFF), byte);
    }
    Ok(())
}

fn _integer(state: &State, value: Value) -> Result<BigInt, ExecutionError> {
    match state.values.get(&value) {
        None => error(format!("{} read before it is defined", value.repr())),
        Some(Stored::Flags(_)) => error(format!("{} is flags, not a number", value.repr())),
        Some(Stored::Integer(got)) => Ok(got.clone()),
    }
}

fn _read(state: &State, arg: &Arg) -> Result<BigInt, ExecutionError> {
    match arg {
        Arg::Held(held) => Ok(_mask(&_integer(state, held.value)?, held.width)),
        Arg::Const(constant) => Ok(_mask(&constant.n, constant.width)),
        Arg::Cell(cell) => _load(state, &cell.r#ref, cell.r#ref.width),
        Arg::FrameAddress(frame) => Ok(_mask(&BigInt::from(frame.offset), frame.width)), // bp is zero
        _ => error(format!("cannot read {}", arg.repr())),
    }
}

fn _width(op: &Op) -> Result<u32, ExecutionError> {
    for one in op.results.iter().chain(&op.args) {
        match one {
            Arg::Held(held) => return Ok(held.width),
            Arg::Const(constant) => return Ok(constant.width),
            Arg::Cell(cell) => return Ok(cell.r#ref.width),
            _ => {}
        }
    }
    error(format!("no width for {}", op.kind))
}

/// Python's `.width` of a `Held | Const` operand.
fn _operand_width(arg: &Arg) -> Option<u32> {
    match arg {
        Arg::Held(held) => Some(held.width),
        Arg::Const(constant) => Some(constant.width),
        _ => None,
    }
}

fn _computed(op: &Op, args: &[BigInt], width: u32) -> Result<Vec<BigInt>, ExecutionError> {
    let kind = op.kind;
    let bits = 8 * width;
    let shift = |b: &BigInt| (b & BigInt::from(31_u8)).to_usize().expect("five bits");
    match (kind, args) {
        (Kind::Copy | Kind::Convert | Kind::ZeroExtend | Kind::Load | Kind::Address, [a]) => {
            return Ok(vec![a.clone()]);
        }
        (Kind::SignExtend, [a]) => {
            return Ok(vec![_operand_width(&op.args[0]).map_or_else(|| a.clone(), |source| _signed(a, source))]);
        }
        (Kind::Add, [a, b]) => return Ok(vec![a + b]),
        (Kind::Sub, [a, b]) => return Ok(vec![a - b]),
        (Kind::Mul, [a, b]) => return Ok(vec![_signed(a, width) * _signed(b, width)]),
        (Kind::And, [a, b]) => return Ok(vec![a & b]),
        (Kind::Or, [a, b]) => return Ok(vec![a | b]),
        (Kind::Xor, [a, b]) => return Ok(vec![a ^ b]),
        (Kind::Shl, [a, b]) => return Ok(vec![a << shift(b)]),
        (Kind::Shr, [a, b]) => return Ok(vec![_mask(a, width) >> shift(b)]),
        (Kind::Sar, [a, b]) => return Ok(vec![_signed(a, width) >> shift(b)]),
        (Kind::Neg, [a]) => return Ok(vec![-a]),
        (Kind::Not, [a]) => return Ok(vec![!a]),
        (Kind::Increment, [a]) => return Ok(vec![a + 1]),
        (Kind::Decrement, [a]) => return Ok(vec![a - 1]),
        (Kind::Div | Kind::Rem | Kind::Divmod, [a, b]) => {
            let (a, b) = (_signed(a, width), _signed(b, width));
            if b.is_zero() {
                return error("division by zero".to_owned());
            }
            let sign = if a.is_negative() == b.is_negative() { 1 } else { -1 };
            let quotient = a.abs() / b.abs() * sign;
            let remainder = &a - &quotient * &b;
            return Ok(match kind {
                Kind::Div => vec![quotient],
                Kind::Rem => vec![remainder],
                _ => vec![quotient, remainder],
            });
        }
        (Kind::Udivmod, [a, b]) => {
            if b.is_zero() {
                return error("division by zero".to_owned());
            }
            return Ok(vec![a / b, a % b]);
        }
        (Kind::Extract, [a, offset]) => return Ok(vec![a >> offset.to_usize().expect("a bit offset")]),
        (Kind::Concat, [high, low]) => {
            let low_width = _operand_width(&op.args[1]).unwrap_or(width / 2);
            return Ok(vec![(high << (8 * low_width)) | _mask(low, low_width)]);
        }
        _ => {}
    }
    if mir::MIRRORED(kind).is_some() && args.len() == 2 {
        let compared = _operand_width(&op.args[0]).unwrap_or(width);
        return Ok(vec![BigInt::from(u8::from(_compare(kind, &args[0], &args[1], compared)?))]);
    }
    error(format!("{kind} with {} operands (bits {bits})", args.len()))
}

fn _executed(op: &Op, state: &mut State, call: Option<&mut Call<'_>>) -> Result<(), ExecutionError> {
    let args = match (op.kind, op.args.as_slice()) {
        (Kind::Address, [Arg::Cell(cell)]) => vec![BigInt::from(_where(&cell.r#ref, state)?.1)],
        _ => op.args.iter().map(|arg| _read(state, arg)).collect::<Result<Vec<_>, _>>()?,
    };
    if op.kind == Kind::Store {
        let [target] = op.results.as_slice() else {
            panic!("ValueError: a store has one result");
        };
        let Arg::Cell(target) = target else {
            return error("a store without a cell".to_owned());
        };
        return _store(state, &target.r#ref, &args[0], target.r#ref.width);
    }
    let answers = if op.kind == Kind::Call {
        let Some(call) = call else {
            return error(format!("call {}", op.name));
        };
        call(op, &args)?
    } else {
        let width = _width(op)?;
        _computed(op, &args, width)?.iter().map(|n| _mask(n, width)).collect()
    };
    let flags = op.defines.iter().filter(|value| value.flags).copied().collect::<Vec<_>>();
    if !flags.is_empty() && op.kind != Kind::Call {
        let width = _width(op)?;
        let result = match answers.first() {
            Some(first) => first.clone(),
            None if op.kind == Kind::Sub => _mask(&(&args[0] - &args[1]), width),
            None => args[0].clone(),
        };
        let padded = args.iter().cloned().chain([BigInt::zero(), BigInt::zero()]).collect::<Vec<_>>();
        let (left, right) = (padded[0].clone(), padded[1].clone());
        for value in flags {
            let stored = _Flags { kind: op.kind, left: left.clone(), right: right.clone(), result: result.clone(), width };
            state.values.insert(value, Stored::Flags(stored));
        }
    }
    for (target, n) in op.results.iter().zip(&answers) {
        match target {
            Arg::Held(held) => {
                state.values.insert(held.value, Stored::Integer(_mask(n, held.width)));
            }
            Arg::Cell(cell) => _store(state, &cell.r#ref, n, cell.r#ref.width)?,
            _ => {}
        }
    }
    Ok(())
}

/// Execute `body` from its entry until it returns.
///
/// `values` are the live-in values; `memory` the bytes it starts with,
/// keyed as `_where` keys them; `dgroup` each DGROUP segment's base in DS.
pub fn run(
    body: &MirBody,
    values: &IndexMap<Value, BigInt>,
    memory: &Memory,
    mut call: Option<&mut Call<'_>>,
    limit: u64,
    dgroup: &BTreeMap<i64, i64>,
) -> Result<Outcome, ExecutionError> {
    let stack = if body.stack_in_data { Segment::Ds } else { Segment::Ss };
    let mut state = State {
        values: values.iter().map(|(value, n)| (*value, Stored::Integer(n.clone()))).collect(),
        memory: memory.clone(),
        steps: 0,
        stack,
        dgroup: dgroup.clone(),
    };
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    for (r#ref, constant) in &body.initial {
        _store(&mut state, r#ref, &constant.n, r#ref.width)?;
    }
    let (mut at, mut came) = (body.entry, None::<i64>);
    loop {
        let block = blocks[&at];
        if let Some(came) = came {
            let incoming = block
                .phis
                .iter()
                .filter_map(|phi| {
                    let source = phi.incoming.get(&came)?;
                    Some((phi.result, state.values.get(source).cloned()))
                })
                .collect::<IndexMap<_, _>>();
            for (result, got) in incoming {
                let Some(got) = got else {
                    return error(format!("phi {} reads an undefined value from b{came}", result.repr()));
                };
                state.values.insert(result, got);
            }
        }
        let mut following = block.succ.first().copied();
        for op in &block.ops {
            state.steps += 1;
            if state.steps > limit {
                return error("step limit".to_owned());
            }
            match op.kind {
                Kind::Nothing => continue,
                Kind::Jump => {
                    following = op.target.or(following);
                    break;
                }
                Kind::Return => {
                    let returned = op.args.iter().map(|arg| _read(&state, arg)).collect::<Result<Vec<_>, _>>()?;
                    return Ok(Outcome { returned, memory: state.memory, steps: state.steps });
                }
                Kind::Branch => {
                    let [flags] = op.uses.iter().filter(|value| value.flags).collect::<Vec<_>>()[..] else {
                        panic!("ValueError: a branch reads one flags value");
                    };
                    let got = match state.values.get(flags) {
                        Some(Stored::Flags(got)) if op.test.is_some() => got,
                        _ => return error(format!("branch on {}", flags.repr())),
                    };
                    let test = op.test.expect("checked above");
                    let others = block.succ.iter().filter(|one| Some(**one) != op.target).collect::<Vec<_>>();
                    following = if _taken(got, test)? { op.target } else { others.first().map(|one| **one).or(op.target) };
                    break;
                }
                Kind::Switch | Kind::Escape | Kind::Opaque | Kind::Arg | Kind::Result => {
                    return error(format!("{} is not modelled", op.kind));
                }
                _ => {
                    if op.floating.is_some() || op.kind.as_str().starts_with('f') {
                        return error(format!("{} is not modelled", op.kind));
                    }
                    _executed(op, &mut state, call.as_deref_mut())?;
                }
            }
        }
        let Some(next) = following else {
            return error(format!("b{at} ends without a successor"));
        };
        (at, came) = (next, Some(at));
    }
}

#[cfg(test)]
#[path = "execute_tests.rs"]
mod tests;
