//! Port of `qbopt/frontend/raising_array_bounds.py`: a bounded, exact path
//! proof for numeric array accesses at the raise boundary.
//!
//! No unknown branch is selected. Every access is checked before its effect is
//! interpreted, so memory disjointness is a conclusion, not a loop assumption.
//! Failure or exhaustion discards the entire proof. This recognizes finite constant
//! control flow; it is not a substitute for general symbolic range analysis.

use std::collections::BTreeSet;

use iced_x86::Register;
use num_bigint::BigInt;

use super::arrayfacts;
use crate::analysis::consts;
use crate::model::ir::{Loc, Operation};
use crate::model::mir::{self, Arg, Cell, Kind, MemRef, MirBody, Op, OpCode, RaisedBody, Value};
use crate::objectfile::module::{Addr, Space};
use crate::support::hash::{HashSet, IndexMap};

/// A walked value: Python's `int`, `True` (a selector loaded from the
/// descriptor), or `Pointer(offset)`.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Walked {
    Int(BigInt),
    True,
    Pointer(BigInt),
}

/// Python's `Addr | ("element", offset)`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Place {
    Addr(Addr),
    Element(BigInt),
}

pub fn proven(body: RaisedBody, limit: i64) -> RaisedBody {
    let body = arrayfacts::proven(body, limit);
    let allocations: Vec<&Op> =
        body.blocks.iter().flat_map(|block| &block.ops).filter(|op| op.array.is_some()).collect();
    if allocations.len() != 1
        || allocations[0].array.as_ref().expect("an array").replaces
        || allocations[0].memory_values.is_empty()
    {
        return body;
    }
    let allocation = allocations[0];
    let request = allocation.array.as_ref().expect("an array");
    let whole = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.loads.iter().chain(&op.stores))
        .any(|reference| reference.pointer);
    if !whole && request.bounds.iter().any(|(low, _)| *low != 0) {
        return body;
    }
    let extent = request
        .bounds
        .iter()
        .fold(BigInt::from(request.element_width), |extent, (low, high)| extent * (high - low + 1));
    let ceiling = BigInt::from(if whole { 1_i64 << 31 } else { 32768 });
    if !(BigInt::from(0) < extent && extent < ceiling) {
        return body;
    }
    let extent = i64::try_from(extent).expect("below 2**31");
    let checked = _walk(&body, allocation, extent, limit);
    if checked.is_empty() {
        return body;
    }
    let descriptor = request.descriptor;

    let reference = |reference: &MemRef| -> MemRef {
        if checked.contains(reference) {
            MemRef { allocation: Some(descriptor), ..reference.clone() }
        } else {
            reference.clone()
        }
    };
    let argument = |arg: &Arg| match arg {
        Arg::Cell(cell) => Arg::Cell(Cell { r#ref: reference(&cell.r#ref) }),
        _ => arg.clone(),
    };
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            block.with_ops(
                block
                    .ops
                    .iter()
                    .map(|op| {
                        let mut made = op.clone();
                        made.loads = op.loads.iter().map(reference).collect();
                        made.stores = op.stores.iter().map(reference).collect();
                        made.args = op.args.iter().map(argument).collect();
                        made.results = op.results.iter().map(argument).collect();
                        made
                    })
                    .collect(),
            )
        })
        .collect();
    body.with_blocks(blocks)
}

/// Python's `ValueError` or `AttributeError`, which discard the whole proof.
struct Refused;

/// Python `arg.width`, an `AttributeError` on a cell or an opaque resource.
fn width(arg: &Arg) -> Result<u32, Refused> {
    match arg {
        Arg::Held(one) => Ok(one.width),
        Arg::Const(one) => Ok(one.width),
        Arg::Symbol(one) => Ok(one.width),
        Arg::FrameAddress(one) => Ok(one.width),
        Arg::FrameSelector(one) => Ok(one.width),
        Arg::Cell(_) | Arg::Opaque(_) => Err(Refused),
    }
}

fn _walk(body: &MirBody, allocation: &Op, extent: i64, mut limit: i64) -> HashSet<MemRef> {
    let blocks: IndexMap<i64, &mir::MirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut values: IndexMap<Value, Option<Walked>> = IndexMap::default();
    let mut memory: IndexMap<(Place, u32), Option<Walked>> = IndexMap::default();
    let mut segments: IndexMap<Register, Option<Walked>> = IndexMap::default();
    let mut comparisons: IndexMap<Value, (BigInt, BigInt, u32)> = IndexMap::default();
    let mut checked: HashSet<MemRef> = HashSet::default();
    let mut active = false;
    let request = allocation.array.as_ref().expect("an array");
    let descriptor = request.descriptor;
    let base = descriptor.offset + descriptor.addend;
    let descriptor_end = base + 14 + 4 * request.bounds.len() as i64;
    let (mut previous, mut current): (Option<i64>, i64) = (None, body.entry);
    let at = |disp: i64| Addr { index: descriptor.index, ..Addr::new(Space::Segment, disp) };

    let no_revisited_accesses = |checked: &HashSet<MemRef>, at: i64, start: usize| -> bool {
        let (mut pending, mut seen) = (vec![(at, start)], BTreeSet::new());
        while let Some((at, start)) = pending.pop() {
            if !seen.insert((at, start)) {
                continue;
            }
            let Some(block) = blocks.get(&at) else {
                return false;
            };
            if block.ops.iter().skip(start).flat_map(|op| op.loads.iter().chain(&op.stores)).any(|one| checked.contains(one))
            {
                return false;
            }
            pending.extend(block.succ.iter().map(|successor| (*successor, 0)));
        }
        true
    };

    #[allow(clippy::too_many_arguments)]
    fn address(
        reference: &MemRef,
        active: bool,
        values: &IndexMap<Value, Option<Walked>>,
        segments: &IndexMap<Register, Option<Walked>>,
        extent: i64,
        checked: &mut HashSet<MemRef>,
    ) -> Result<Place, Refused> {
        let pointer = |values: &IndexMap<Value, Option<Walked>>| match reference.base.and_then(|one| values.get(&one)) {
            Some(Some(Walked::Pointer(offset))) => Some(offset.clone()),
            _ => None,
        };
        if reference.pointer && active {
            if let Some(offset) = pointer(values) {
                if BigInt::from(0) <= offset && offset <= BigInt::from(extent - i64::from(reference.width)) {
                    checked.insert(reference.clone());
                    return Ok(Place::Element(offset));
                }
            }
            return Err(Refused);
        }
        let resolved = mir::symbolic_ref(reference);
        let Some(addr) = resolved.addr else {
            return Err(Refused);
        };
        if resolved.base.is_none() && resolved.segment.is_none() && matches!(addr.space, Space::Segment | Space::Frame) {
            return Ok(Place::Addr(addr));
        }
        let original = reference.addr.ok_or(Refused)?;
        let selector = match reference.segment {
            Some(segment) => values.get(&segment),
            None => segments.get(&original.segment),
        };
        if original.space == Space::Far && active && matches!(selector, Some(Some(Walked::True))) {
            if let Some(offset) = pointer(values) {
                let element = offset + original.disp;
                if BigInt::from(0) <= element && element <= BigInt::from(extent - i64::from(reference.width)) {
                    checked.insert(reference.clone());
                    return Ok(Place::Element(element));
                }
            }
        }
        Err(Refused)
    }

    let walked = (|| -> Result<HashSet<MemRef>, Refused> {
        while let Some(block) = blocks.get(&current).copied() {
            let incoming: Vec<(Value, Option<Walked>)> = block
                .phis
                .iter()
                .map(|phi| {
                    let source = previous.and_then(|previous| phi.incoming.get(&previous).copied());
                    (phi.result, source.and_then(|source| values.get(&source).cloned().flatten()))
                })
                .collect();
            values.extend(incoming);
            let mut following = if block.succ.len() == 1 { Some(block.succ[0]) } else { None };
            for (position, op) in block.ops.iter().enumerate() {
                limit -= 1;
                if limit < 0 {
                    return Err(Refused);
                }
                if op.kind == Kind::Call {
                    if std::ptr::eq(op, allocation) && !active {
                        active = true;
                        memory = op
                            .memory_values
                            .iter()
                            .filter_map(|(reference, value)| {
                                reference.addr.map(|addr| ((Place::Addr(addr), reference.width), Some(Walked::Int(value.n.clone()))))
                            })
                            .collect();
                        memory.insert((Place::Addr(at(base)), 4), Some(Walked::Pointer(0.into())));
                        memory.insert((Place::Addr(at(base + 10)), 2), Some(Walked::Pointer(0.into())));
                        memory.insert((Place::Addr(at(base + 2)), 2), Some(Walked::True));
                        values.clear();
                        segments.clear();
                        continue;
                    }
                    // Keep only prefix references which no future execution can revisit.
                    if no_revisited_accesses(&checked, block.at, position + 1) {
                        return Ok(std::mem::take(&mut checked));
                    }
                    return Err(Refused);
                }
                if matches!(op.kind, Kind::Arg | Kind::Jump | Kind::Nothing) {
                    continue;
                }
                let arith = consts::ARITH.iter().find(|(one, _)| *one == op.kind).map(|(_, arith)| arith);
                if op.barrier()
                    || !(arith.is_some()
                        || matches!(
                            op.kind,
                            Kind::Copy
                                | Kind::Load
                                | Kind::Store
                                | Kind::Increment
                                | Kind::Branch
                                | Kind::PtrOffset
                                | Kind::SignExtend
                        ))
                {
                    return Err(Refused);
                }
                if op.kind == Kind::Branch {
                    let flags: Vec<Value> = op.uses.iter().filter(|value| value.flags).copied().collect();
                    if flags.len() != 1 || !comparisons.contains_key(&flags[0]) {
                        return Err(Refused);
                    }
                    let (left, right, width) = comparisons[&flags[0]].clone();
                    let sign = BigInt::from(1) << (width * 8 - 1);
                    let (left, right) = ((left ^ &sign) - &sign, (right ^ &sign) - &sign);
                    let taken = match op.test {
                        Some(Kind::Le) => left <= right,
                        Some(Kind::Lt) => left < right,
                        Some(Kind::Ge) => left >= right,
                        Some(Kind::Gt) => left > right,
                        Some(Kind::Eq) => left == right,
                        Some(Kind::Ne) => left != right,
                        _ => return Err(Refused),
                    };
                    let others: Vec<i64> = block.succ.iter().filter(|at| Some(**at) != op.target).copied().collect();
                    if others.len() != 1 {
                        return Err(Refused);
                    }
                    following = if taken { op.target } else { Some(others[0]) };
                    continue;
                }
                let mut args = Vec::new();
                for arg in &op.args {
                    args.push(match arg {
                        Arg::Const(one) => Some(Walked::Int(consts::masked(&one.n, one.width))),
                        Arg::Held(one) => {
                            if !matches!(one.width, 1 | 2 | 4) {
                                return Err(Refused);
                            }
                            match values.get(&one.value).cloned().flatten() {
                                Some(Walked::Int(result)) => Some(Walked::Int(consts::masked(&result, one.width))),
                                Some(Walked::True) => Some(Walked::Int(consts::masked(&BigInt::from(1), one.width))),
                                other => other,
                            }
                        }
                        Arg::Cell(one) => {
                            if !matches!(one.r#ref.width, 1 | 2 | 4) {
                                return Err(Refused);
                            }
                            let place = address(&one.r#ref, active, &values, &segments, extent, &mut checked)?;
                            memory.get(&(place, one.r#ref.width)).cloned().flatten()
                        }
                        _ => None,
                    });
                }
                for reference in &op.loads {
                    address(reference, active, &values, &segments, extent, &mut checked)?;
                }
                let int = |one: &Option<Walked>| match one {
                    Some(Walked::Int(number)) => Some(number.clone()),
                    _ => None,
                };
                let mut result: Option<Walked> = None;
                if op.kind == Kind::Xor && op.args.len() == 2 && op.args[0] == op.args[1] {
                    result = Some(Walked::Int(0.into()));
                } else if matches!(op.kind, Kind::Copy | Kind::Load | Kind::Store) && args.len() == 1 {
                    result = args[0].clone();
                } else if let (Kind::PtrOffset, [Some(Walked::Pointer(offset)), Some(Walked::Int(delta))]) =
                    (op.kind, args.as_slice())
                {
                    let sign = BigInt::from(0x8000_0000_u32);
                    let displacement = (delta ^ &sign) - &sign;
                    result = Some(Walked::Pointer(offset + displacement));
                } else if let (Kind::SignExtend, [Some(Walked::Int(value))]) = (op.kind, args.as_slice()) {
                    let sign = BigInt::from(1) << (width(&op.args[0])? * 8 - 1);
                    result = Some(Walked::Int((value ^ &sign) - &sign));
                } else if let (Kind::Add, [Some(number @ (Walked::Int(_) | Walked::True)), Some(Walked::Pointer(offset))]) =
                    (op.kind, args.as_slice())
                {
                    let number = match number {
                        Walked::Int(number) => number.clone(),
                        _ => BigInt::from(1),
                    };
                    result = Some(Walked::Pointer(number + offset));
                } else if let (Some(arith), [left, right]) = (arith, args.as_slice()) {
                    if let (Some(left), Some(right)) = (int(left), int(right)) {
                        result = Some(Walked::Int(arith(&left, &right)));
                    }
                } else if let (Kind::Increment, [Some(Walked::Int(value))]) = (op.kind, args.as_slice()) {
                    result = Some(Walked::Int(value + 1));
                }
                for value in &op.defines {
                    values.shift_remove(value);
                    comparisons.shift_remove(value);
                }
                let numbers: Option<Vec<BigInt>> = args.iter().map(int).collect();
                if op.op == Some(OpCode::Operation(Operation::Compare)) && args.len() == 2 && numbers.is_some() {
                    let numbers = numbers.expect("checked");
                    let width = width(&op.args[0])?;
                    if width != 2 || self::width(&op.args[1])? != width {
                        return Err(Refused);
                    }
                    for value in op.defines.iter().filter(|value| value.flags) {
                        comparisons.insert(*value, (numbers[0].clone(), numbers[1].clone(), width));
                    }
                } else if matches!(op.kind, Kind::And | Kind::Or | Kind::Xor)
                    && matches!(result, Some(Walked::Int(_)))
                    && op.defines.iter().any(|value| value.flags)
                {
                    let width = width(&op.args[0])?;
                    for arg in &op.args {
                        if self::width(arg)? != width {
                            return Err(Refused);
                        }
                    }
                    if width != 2 {
                        return Err(Refused);
                    }
                    let number = int(&result).expect("checked");
                    for value in op.defines.iter().filter(|value| value.flags) {
                        comparisons.insert(*value, (consts::masked(&number, width), 0.into(), width));
                    }
                }
                let fitted = |result: &Option<Walked>, width: u32| match result {
                    Some(Walked::Int(number)) => Some(Walked::Int(consts::masked(number, width))),
                    other => other.clone(),
                };
                for (index, output) in op.results.iter().enumerate() {
                    match output {
                        Arg::Held(output) => {
                            if !matches!(output.width, 1 | 2 | 4) {
                                return Err(Refused);
                            }
                            values.insert(output.value, fitted(&result, output.width));
                            if index > 0 && op.kind == Kind::Mul {
                                values.insert(output.value, None);
                            }
                        }
                        Arg::Opaque(output) => {
                            if let Some(Loc::Reg(register)) = output.machine_payload() {
                                segments.insert(register.register, result.clone());
                            }
                        }
                        _ => {}
                    }
                }
                for reference in &op.stores {
                    let target = address(reference, active, &values, &segments, extent, &mut checked)?;
                    if !matches!(reference.width, 1 | 2 | 4)
                        || matches!(&target, Place::Addr(target) if target.space == Space::Segment && target.index != descriptor.index)
                    {
                        return Err(Refused);
                    }
                    if let Place::Addr(target) = &target {
                        if target.index == descriptor.index
                            && target.disp < descriptor_end
                            && base < target.disp + i64::from(reference.width)
                        {
                            return Err(Refused);
                        }
                    }
                    memory.retain(|(location, width), _| {
                        let overlaps = match (&target, location) {
                            (Place::Addr(target), Place::Addr(location)) => {
                                location.space == target.space
                                    && location.index == target.index
                                    && location.disp < target.disp + i64::from(reference.width)
                                    && target.disp < location.disp + i64::from(*width)
                            }
                            (Place::Element(target), Place::Element(location)) => {
                                *location < target + reference.width && *target < location + *width
                            }
                            _ => false,
                        };
                        !overlaps
                    });
                    memory.insert((target, reference.width), fitted(&result, reference.width));
                }
            }
            if block.succ.is_empty() {
                return Ok(std::mem::take(&mut checked));
            }
            let Some(next) = following else {
                return Err(Refused);
            };
            (previous, current) = (Some(current), next);
        }
        Ok(HashSet::default())
    })();
    walked.unwrap_or_default()
}
