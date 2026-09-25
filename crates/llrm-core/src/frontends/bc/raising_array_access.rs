//! Port of `qbopt/frontend/raising_array_access.py`: expose numeric array
//! addressing as scalar MIR arithmetic.
//!
//! Static allocations and established non-huge far allocations are recognized.
//! Huge numeric accesses use whole pointers; string layouts remain unsupported.

use std::collections::BTreeSet;
use std::rc::Rc;

use iced_x86::Register;
use num_bigint::BigInt;

use super::raising_array_bounds;
use super::raising_calls::{_capture, _discarded};
use crate::abi::runtime;
use crate::analysis::{consts, ssa};
use crate::model::ir::{self, Loc, Operation};
use crate::model::mir::{
    self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Opaque, Op, OpCode, OrderedMap, RaisedBody, Symbol,
    Value,
};
use crate::objectfile::module::{self, Addr, Module, Space};
use crate::objectfile::omf;
use crate::support::hash::IndexMap;

// qb/ir/prsid.asm: MAXDIM, shared with BASCOM; validated with all three BCs.
pub const MAX_DIMENSIONS: i64 = 60;

/// `data` is a `Symbol` or a `Cell`; each dimension's count and lower bound
/// a `Const` (Python's int) or a `Cell`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub data: Arg,
    pub selector: MemRef,
    pub width: u32,
    pub dimensions: Vec<(Arg, Arg)>,
    pub huge: bool,
}

pub fn descriptor(found: &Module, symbol: Option<&Symbol>) -> Option<Descriptor> {
    let symbol = symbol?;
    if symbol.space != Space::Segment || symbol.width != 2 {
        return None;
    }
    let start = symbol.offset + symbol.addend;
    let segments = omf::segments(&found.records);
    if !(0 < symbol.index && (symbol.index as usize) < segments.len()) {
        return None;
    }
    let (_, size) = segments[symbol.index as usize].clone()?;
    if !(0 <= start && start <= size - 18) {
        return None;
    }
    let data = omf::segment_image(&found.records, symbol.index, size);
    let byte = |at: i64| data[at as usize];
    let word = |at: i64| u16::from_le_bytes([byte(at), byte(at + 1)]);
    let (rank, features) = (i64::from(byte(start + 8)), byte(start + 9));
    if !(1..=MAX_DIMENSIONS).contains(&rank) || features != 0x40 || start + 14 + 4 * rank > size {
        return None;
    }
    let fixups: Vec<omf::Fixup> =
        omf::fixups(&found.records).into_iter().filter(|one| one.seg == Some(symbol.index)).collect();
    let pointer: Vec<&omf::Fixup> = fixups
        .iter()
        .filter(|one| one.offset == start && one.loc == omf::LOC_PTR32 && one.target == "segment")
        .collect();
    if pointer.len() != 1
        || fixups
            .iter()
            .any(|one| start + 8 <= one.offset && one.offset < start + 14 + 4 * rank && one.offset != start + 10)
    {
        return None;
    }
    let pointer = pointer[0];
    if !found.dgroup.contains(pointer.index) || !(0 < pointer.index && (pointer.index as usize) < segments.len()) {
        return None;
    }
    let width = u32::from(word(start + 12));
    let dimensions: Vec<(i64, i64)> = (0..rank)
        .map(|dimension| start + 14 + 4 * dimension)
        .map(|at| (i64::from(word(at)), i64::from(word(at + 2) as i16)))
        .collect();
    if !matches!(width, 1 | 2 | 4 | 8) || dimensions.iter().any(|(count, _)| *count == 0) {
        return None;
    }
    let offset = pointer.disp + i64::from(word(start));
    let extent = dimensions.iter().fold(BigInt::from(width), |extent, (count, _)| extent * count);
    let (_, target) = segments[pointer.index as usize].clone()?;
    if !(BigInt::from(0) <= BigInt::from(offset) && BigInt::from(offset) <= BigInt::from(65536.min(target)) - extent) {
        return None;
    }
    Some(Descriptor {
        data: Arg::Symbol(Symbol::new(Space::Segment, pointer.index, offset, 2)),
        selector: MemRef::new(Some(Addr { index: symbol.index, ..Addr::new(Space::Segment, start + 2) }), 2),
        width,
        dimensions: dimensions
            .into_iter()
            .map(|(count, lower)| (Arg::Const(Const::new(count, 2)), Arg::Const(Const::new(lower, 2))))
            .collect(),
        huge: false,
    })
}

pub fn dynamic(body: &MirBody, symbol: Option<&Symbol>) -> Option<Descriptor> {
    let symbol = symbol?;
    let start = symbol.offset + symbol.addend;

    let field = |offset: i64, width: u32| {
        MemRef::new(Some(Addr { index: symbol.index, ..Addr::new(symbol.space, start + offset) }), width)
    };
    // Python's `dict(op.memory_values)`: the last binding of a cell wins.
    let fact = |values: &[(MemRef, Const)], cell: &MemRef| {
        values.iter().rev().find(|(reference, _)| reference == cell).map(|(_, value)| value.clone())
    };

    let allocations: Vec<&Op> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind == Kind::Call && fact(&op.memory_values, &field(9, 1)).is_some())
        .collect();
    if allocations.len() != 1 {
        return None;
    }
    let facts = &allocations[0].memory_values;

    // All three DDIM implementations zero the base offset for numeric FAR
    // arrays and reject sizes above 64K. Unlike HUGE, a valid access never
    // needs selector carry. Load mutable fields at each access; do not turn
    // allocation-time bounds or a movable heap address into eternal constants.
    let (rank, width) = (fact(facts, &field(8, 1)), fact(facts, &field(12, 2)));
    let features = fact(facts, &field(9, 1))?;
    if ![Const::new(1, 1), Const::new(2, 1), Const::new(3, 1)].contains(&features) {
        return None;
    }
    let (Some(rank), Some(width)) = (rank, width) else {
        return None;
    };
    let rank = i64::try_from(&rank.n).ok().filter(|rank| (1..=MAX_DIMENSIONS).contains(rank))?;
    let width = u32::try_from(&width.n).ok().filter(|width| matches!(width, 1 | 2 | 4 | 8))?;
    let huge = features.n.clone() & BigInt::from(2) != BigInt::from(0);
    Some(Descriptor {
        data: Arg::Cell(Cell { r#ref: field(0, if huge { 4 } else { 2 }) }),
        selector: field(2, 2),
        width,
        dimensions: (0..rank)
            .map(|index| {
                (Arg::Cell(Cell { r#ref: field(14 + 4 * index, 2) }), Arg::Cell(Cell { r#ref: field(16 + 4 * index, 2) }))
            })
            .collect(),
        huge,
    })
}

pub fn _overwrites_offset(body: &MirBody, op: &Op, value: Value) -> bool {
    if op.kind != Kind::Copy || op.args.len() != 1 || op.results.len() != 1 {
        return false;
    }
    let width = match &op.args[0] {
        Arg::Symbol(one) => one.width,
        Arg::Const(one) => one.width,
        Arg::Held(one) => one.width,
        _ => return false,
    };
    if width != 2 || matches!(&op.args[0], Arg::Held(one) if one.value == value) {
        return false;
    }
    let Arg::Held(result) = &op.results[0] else {
        return false;
    };
    if result.width != 2 || op.merges.len() != 1 || op.merges.get(&value) != Some(&result.value) {
        return false;
    }
    let (mut pending, mut visited) = (vec![result.value], BTreeSet::new());
    while let Some(result) = pending.pop() {
        if !visited.insert(result) {
            continue;
        }
        for block in &body.blocks {
            if block.phis.iter().any(|phi| phi.incoming.values().any(|one| *one == result)) {
                return false;
            }
            for later in &block.ops {
                if later.args.iter().any(|arg| matches!(arg, Arg::Held(one) if one.value == result && one.width > 2))
                    || later
                        .loads
                        .iter()
                        .chain(&later.stores)
                        .any(|reference| reference.base == Some(result) && reference.base_width > 2)
                {
                    return false;
                }
                if let Some(merged) = later.merges.get(&result) {
                    pending.push(*merged);
                }
            }
        }
    }
    true
}

pub fn _selector_dead(
    body: &MirBody,
    block: &MirBlock,
    position: usize,
    contracts: &IndexMap<i64, runtime::Contract>,
) -> bool {
    let blocks: IndexMap<i64, &MirBlock> = body.blocks.iter().map(|one| (one.at, one)).collect();
    let (mut pending, mut visited) = (vec![(block.at, position)], BTreeSet::new());
    'pending: while let Some((at, start)) = pending.pop() {
        if !visited.insert((at, start)) {
            continue;
        }
        let Some(current) = blocks.get(&at) else {
            return false;
        };
        for later in current.ops.iter().skip(start) {
            if later.kind == Kind::Nothing {
                continue;
            }
            if later.kind == Kind::Call {
                let Some(contract) = contracts.get(&later.at) else {
                    return false;
                };
                let Some(inputs) = &contract.inputs else {
                    return false;
                };
                if inputs.contains(&runtime::Reg::Es) {
                    return false;
                }
                if contract.clobbers.contains(&runtime::Reg::Es) {
                    continue 'pending;
                }
            } else {
                let Some(node) = later.node() else {
                    return false;
                };
                let effects = node.effects();
                let Some(uses) = &effects.uses else {
                    return false;
                };
                if uses.contains(&Register::ES) {
                    return false;
                }
                if effects.defs.as_ref().is_some_and(|defs| defs.contains(&Register::ES)) {
                    continue 'pending;
                }
            }
        }
        if current.succ.is_empty() {
            return false;
        }
        pending.extend(current.succ.iter().map(|successor| (*successor, 0)));
    }
    true
}

/// Prove the helper's machine outputs are only one local memory address.
pub fn _whole_consumer<'a>(
    body: &MirBody,
    block: &'a MirBlock,
    position: usize,
    value: Value,
    contracts: &IndexMap<i64, runtime::Contract>,
) -> Option<(usize, &'a Op)> {
    let mut found = None;
    for consumer_position in position + 1..block.ops.len() {
        let candidate = &block.ops[consumer_position];
        if candidate.uses.contains(&value) {
            found = Some(consumer_position);
            break;
        }
        let effects = candidate.node().map(|node| node.effects());
        let (Some(uses), Some(defs)) =
            (effects.and_then(|one| one.uses.as_ref()), effects.and_then(|one| one.defs.as_ref()))
        else {
            return None;
        };
        if candidate.kind == Kind::Call || uses.contains(&Register::ES) || defs.contains(&Register::ES) {
            return None;
        }
    }
    let consumer_position = found?;
    let consumer = &block.ops[consumer_position];
    if !matches!(consumer.kind, Kind::Load | Kind::Store | Kind::Arg) {
        return None;
    }
    let refs = if consumer.kind != Kind::Store { &consumer.loads } else { &consumer.stores };
    if refs.len() != 1 {
        return None;
    }
    let reference = &refs[0];
    let Some(addr) = reference.addr else {
        return None;
    };
    if reference.base != Some(value)
        || !matches!(reference.width, 1 | 2 | 4)
        || addr.space != Space::Far
        || addr.segment != Register::ES
        || addr.disp != 0
        || reference.segment.is_some()
        || consumer.args.iter().any(|arg| matches!(arg, Arg::Held(one) if one.value == value))
    {
        return None;
    }
    if body.blocks.iter().any(|other| {
        other.ops.iter().any(|op| {
            op.uses.contains(&value)
                && !std::ptr::eq(op, consumer)
                && !(std::ptr::eq(other, block) && _overwrites_offset(body, op, value))
        })
    }) {
        return None;
    }
    if body.blocks.iter().flat_map(|other| &other.phis).any(|phi| phi.incoming.values().any(|one| *one == value)) {
        return None;
    }
    _selector_dead(body, block, consumer_position + 1, contracts).then_some((consumer_position, consumer))
}

/// Every subscript fits the descriptor currently in memory, not just its total extent.
pub fn _checked(
    shape: Option<&Descriptor>,
    symbol: Option<&Symbol>,
    indices: &[Option<consts::Known>],
    memory: &consts::Cells,
) -> bool {
    let (Some(shape), Some(symbol)) = (shape, symbol) else {
        return false;
    };
    if !matches!(shape.data, Arg::Cell(_)) {
        return false;
    }

    let field = |offset: i64, width: u32| {
        consts::_cell(
            memory,
            &MemRef::new(
                Some(Addr { index: symbol.index, ..Addr::new(symbol.space, symbol.offset + symbol.addend + offset) }),
                width,
            ),
        )
    };
    let features: &[i64] = if shape.huge { &[2, 3] } else { &[1] };
    if field(8, 1) != Some(consts::Known::new(shape.dimensions.len() as i64, 1))
        || field(12, 2) != Some(consts::Known::new(shape.width, 2))
        || !features.iter().any(|feature| field(9, 1) == Some(consts::Known::new(*feature, 1)))
    {
        return false;
    }
    if indices.len() != shape.dimensions.len() {
        return false;
    }
    let signed = |number: &BigInt| {
        let low = i64::try_from(number & BigInt::from(0xffff)).expect("16 bits");
        (low ^ 0x8000) - 0x8000
    };
    for (index, (count, lower)) in indices.iter().rev().zip(&shape.dimensions) {
        let cell = |arg: &Arg| match arg {
            Arg::Cell(cell) => consts::_cell(memory, &cell.r#ref),
            _ => unreachable!("a dynamic descriptor's fields are cells"),
        };
        let (count, lower) = (cell(count), cell(lower));
        let (Some(index), Some(count), Some(lower)) = (index, count, lower) else {
            return false;
        };
        if index.width < 2 || count.width < 2 || lower.width < 2 {
            return false;
        }
        let adjusted = signed(&index.n) - signed(&lower.n);
        let count = i64::try_from(&count.n & BigInt::from(0xffff)).expect("16 bits");
        if !(0 <= adjusted && adjusted < count) {
            return false;
        }
    }
    true
}

/// Expose accesses until newly proven allocation identities unlock no further checks.
pub fn native(body: RaisedBody, found: &Module, bounds_checks: bool) -> Result<RaisedBody, String> {
    let remaining = |body: &RaisedBody| {
        body.blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|op| op.kind == Kind::Call && found.calls.get(&op.at).map(String::as_str) == Some("B$HARY"))
            .count()
    };

    let mut body = body;
    let mut pending = remaining(&body);
    while pending > 0 {
        body = _native(body, found, bounds_checks)?;
        let following = remaining(&body);
        if !bounds_checks || following >= pending {
            break;
        }
        pending = following;
    }
    Ok(body)
}

fn _native(body: RaisedBody, found: &Module, bounds_checks: bool) -> Result<RaisedBody, String> {
    let local = module::defines(&found.records, found.seg);
    let calls: IndexMap<i64, String> =
        found.calls.iter().filter(|(_, name)| !local.contains(*name)).map(|(at, name)| (*at, name.clone())).collect();
    let hary = |op: &Op| op.kind == Kind::Call && calls.get(&op.at).map(String::as_str) == Some("B$HARY");
    if !body.blocks.iter().flat_map(|block| &block.ops).any(hary) {
        return Ok(body);
    }

    let overwritten = |op: &Op| -> Op {
        if op.merges.len() == 1 {
            let value = *op.merges.keys().next().expect("one merge");
            if _overwrites_offset(&body, op, value) {
                let mut made = op.clone();
                made.merges = OrderedMap::new();
                made.uses = op.uses.iter().copied().filter(|one| *one != value).collect();
                return made;
            }
        }
        op.clone()
    };
    let blocks: Vec<MirBlock> =
        body.blocks.iter().map(|block| block.with_ops(block.ops.iter().map(overwritten).collect())).collect();
    let mut body = body.with_blocks(blocks);
    let roots: BTreeSet<Value> = body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.uses.iter().copied()).collect();
    let pruned = ssa::pruned_phis(&Rc::new(std::mem::replace(&mut body.body, MirBody::new(0, Vec::new()))), &roots);
    body.body = Rc::try_unwrap(pruned).unwrap_or_else(|shared| (*shared).clone());
    let contracts = runtime::for_module(found, None).map_err(|error| error.0)?;
    let shared = Rc::new(body.body.clone());
    let known = consts::known(&shared, None, None, None, None);
    let memory = if bounds_checks {
        consts::cells(&body, &found.dgroup.members, &calls, Some(&known), None, None, None, None)
    } else {
        IndexMap::default()
    };
    let empty = consts::Cells::default();
    let mut argument_facts: IndexMap<(i64, usize), Option<consts::Known>> = IndexMap::default();
    if bounds_checks {
        for block in &body.blocks {
            for (index, op) in block.ops.iter().enumerate() {
                if op.kind == Kind::Arg && op.args.len() == 1 {
                    let here = memory.get(&(block.at, index)).map(|here| &**here).unwrap_or(&empty);
                    argument_facts.insert((block.at, index), consts::_operand(op, &op.args[0], &known, Some(here)));
                }
            }
        }
    }
    let definitions: IndexMap<Value, &Op> = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect();
    let mut read: BTreeSet<Value> =
        body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.uses.iter().copied()).collect();
    read.extend(body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values().copied()));
    let values: BTreeSet<Value> = ssa::values(&body).chain(body.origin.keys().copied()).collect();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);

    let mut fresh = |at: i64, width: u32| {
        serial += 1;
        variable += 1;
        Held { value: Value { id: serial, at, flags: false, variable, version: 1 }, width }
    };

    fn loaded(expanded: &mut Vec<Op>, fresh: &mut dyn FnMut(i64, u32) -> Held, at: i64, source: &Arg) -> Arg {
        let Arg::Cell(cell) = source else {
            return source.clone();
        };
        let result = fresh(at, cell.r#ref.width);
        let mut load = Op::new(at, OpCode::Operation(Operation::Move), "mov", vec![result.value], Vec::new());
        load.kind = Kind::Load;
        load.args = vec![source.clone()];
        load.results = vec![Arg::Held(result)];
        load.loads = vec![cell.r#ref.clone()];
        load.symbol = Some(true);
        expanded.push(load);
        Arg::Held(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn arithmetic(
        expanded: &mut Vec<Op>,
        fresh: &mut dyn FnMut(i64, u32) -> Held,
        at: i64,
        width: u32,
        kind: Kind,
        left: Arg,
        right: Arg,
        result: Option<Held>,
    ) -> Arg {
        let result = result.unwrap_or_else(|| fresh(at, width));
        let uses = [&left, &right]
            .into_iter()
            .filter_map(|arg| match arg {
                Arg::Held(one) => Some(one.value),
                _ => None,
            })
            .collect();
        let mut made = Op::new(at, OpCode::Operation(Operation::Binary), kind.as_str(), vec![result.value], uses);
        made.kind = kind;
        made.symbol = Some(matches!(right, Arg::Symbol(_)));
        made.args = vec![left, right];
        made.results = vec![Arg::Held(result)];
        expanded.push(made);
        Arg::Held(result)
    }

    fn extended(
        expanded: &mut Vec<Op>,
        fresh: &mut dyn FnMut(i64, u32) -> Held,
        at: i64,
        width: u32,
        source: &Arg,
        unsigned: bool,
    ) -> Arg {
        let source = loaded(expanded, fresh, at, source);
        if width == 2 {
            return source;
        }
        if let Arg::Const(source) = &source {
            let number = if unsigned { &source.n & BigInt::from(0xffff) } else { source.n.clone() };
            return Arg::Const(Const::new(number, 4));
        }
        let Arg::Held(held) = &source else {
            unreachable!("a huge descriptor's fields are cells");
        };
        let result = fresh(at, 4);
        let mut made =
            Op::new(at, OpCode::Operation(Operation::Extend), "sign_extend", vec![result.value], vec![held.value]);
        made.kind = Kind::SignExtend;
        made.args = vec![source.clone()];
        made.results = vec![Arg::Held(result)];
        expanded.push(made);
        if unsigned {
            let mask = Arg::Const(Const::new(0xffff, 4));
            arithmetic(expanded, fresh, at, width, Kind::And, Arg::Held(result), mask, None)
        } else {
            Arg::Held(result)
        }
    }

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let (mut arguments, mut ops): (Vec<usize>, Vec<Op>) = (Vec::new(), Vec::new());
        // Python's `id(ops[index])` names an original operation only:
        // where each op came from in this block, if it is one.
        let mut origins: Vec<Option<usize>> = Vec::new();
        let mut replacements: IndexMap<usize, (Option<Op>, Op)> = IndexMap::default();
        for (position, original) in block.ops.iter().enumerate() {
            let (load, op, origin) = match replacements.get(&position) {
                Some((load, op)) => (load.clone(), op.clone(), None),
                None => (None, original.clone(), Some(position)),
            };
            if let Some(load) = load {
                ops.push(load);
                origins.push(None);
            }
            let width = match op.args.as_slice() {
                [Arg::Cell(cell)] => cell.r#ref.width,
                [Arg::Held(one)] => one.width,
                [Arg::Const(one)] => one.width,
                [Arg::Symbol(one)] => one.width,
                [Arg::FrameAddress(one)] => one.width,
                [Arg::FrameSelector(one)] => one.width,
                _ => 0,
            };
            if op.kind == Kind::Arg && op.args.len() == 1 && width == 2 {
                arguments.push(ops.len());
            } else if op.kind == Kind::Call && calls.get(&op.at).map(String::as_str) == Some("B$HARY") {
                let source = match op.args.as_slice() {
                    [Arg::Held(one)] => definitions.get(&one.value).copied(),
                    _ => None,
                };
                let symbol = match source {
                    Some(source) if source.kind == Kind::Copy && source.args.len() == 1 => match &source.args[0] {
                        Arg::Symbol(symbol) => Some(*symbol),
                        _ => None,
                    },
                    _ => None,
                };
                let shape = descriptor(found, symbol.as_ref()).or_else(|| dynamic(&body, symbol.as_ref()));
                let outputs: Vec<Value> = op.defines.iter().filter(|value| !value.flags).copied().collect();
                let mut rank = arguments.last().map(|index| ops[*index].args[0].clone());
                if let Some(Arg::Held(held)) = &rank {
                    if let Some(fact) = known.get(&held.value) {
                        if fact.width >= 2 {
                            rank = Some(Arg::Const(Const::new(&fact.n & BigInt::from(0xffff), 2)));
                        }
                    }
                }
                // QB can reuse a register for rank across debug calls. In
                // unchecked mode the static descriptor and complete argument
                // group establish arity; runtime rank validation is omitted.
                let exact =
                    |shape: &Descriptor| rank == Some(Arg::Const(Const::new(shape.dimensions.len() as i64, 2)));
                let mut valid = shape.as_ref().is_some_and(|shape| {
                    (exact(shape) || matches!(&rank, Some(Arg::Held(one)) if one.width == 2))
                        && arguments.len() == shape.dimensions.len() + 1
                        && outputs.len() == 1
                        && body.origin.get(&outputs[0]) == Some(&Register::EBX)
                        && !op.defines.iter().any(|value| value.flags && read.contains(value))
                });
                if bounds_checks {
                    valid = valid && exact(shape.as_ref().expect("valid")) && {
                        let indices: Vec<Option<consts::Known>> = arguments[..arguments.len() - 1]
                            .iter()
                            .map(|index| {
                                origins[*index].and_then(|at| argument_facts.get(&(block.at, at)).cloned().flatten())
                            })
                            .collect();
                        _checked(
                            shape.as_ref(),
                            symbol.as_ref(),
                            &indices,
                            memory.get(&(block.at, position)).map(|here| &**here).unwrap_or(&empty),
                        )
                    };
                }
                let huge = valid && shape.as_ref().expect("valid").huge;
                let consumer = if huge { _whole_consumer(&body, block, position, outputs[0], &contracts) } else { None };
                valid = valid && (!huge || consumer.is_some());
                if valid {
                    let shape = shape.expect("valid");
                    let mut indices = Vec::new();
                    for index in &arguments[..arguments.len() - 1] {
                        let push = ops[*index].clone();
                        let held = fresh(push.at, 2);
                        ops[*index] = _capture(&push, &push.args[0], &held);
                        origins[*index] = None;
                        indices.push(held);
                    }
                    let rank_push = *arguments.last().expect("a rank");
                    ops[rank_push] = _discarded(&ops[rank_push]);
                    origins[rank_push] = None;
                    let mut expanded: Vec<Op> = Vec::new();
                    let width = if shape.huge { 4 } else { 2 };
                    let (at, mut offset) = (op.at, None::<Arg>);
                    for (index, (count, lower)) in indices.iter().rev().zip(&shape.dimensions) {
                        let left = extended(&mut expanded, &mut fresh, at, width, &Arg::Held(*index), false);
                        let right = extended(&mut expanded, &mut fresh, at, width, lower, false);
                        let adjusted = arithmetic(&mut expanded, &mut fresh, at, width, Kind::Sub, left, right, None);
                        offset = Some(match offset {
                            None => adjusted,
                            Some(offset) => {
                                let count = extended(&mut expanded, &mut fresh, at, width, count, true);
                                let scaled =
                                    arithmetic(&mut expanded, &mut fresh, at, width, Kind::Mul, offset, count, None);
                                arithmetic(&mut expanded, &mut fresh, at, width, Kind::Add, scaled, adjusted, None)
                            }
                        });
                    }
                    let size = Arg::Const(Const::new(shape.width, width));
                    let offset = offset.expect("at least one dimension");
                    let offset = arithmetic(&mut expanded, &mut fresh, at, width, Kind::Mul, offset, size, None);
                    if shape.huge {
                        let data = loaded(&mut expanded, &mut fresh, at, &shape.data);
                        let Arg::Held(pointer) =
                            arithmetic(&mut expanded, &mut fresh, at, width, Kind::PtrOffset, data, offset, None)
                        else {
                            unreachable!("arithmetic holds its result");
                        };
                        for (following, later) in block.ops.iter().enumerate().skip(position + 2) {
                            if later.uses.contains(&outputs[0]) && _overwrites_offset(&body, later, outputs[0]) {
                                let mut made = later.clone();
                                made.merges = OrderedMap::new();
                                made.uses = later.uses.iter().copied().filter(|value| *value != outputs[0]).collect();
                                replacements.insert(following, (None, made));
                            }
                        }
                        let (consumer_position, consumer) = consumer.expect("valid");
                        let mut reference = MemRef::new(
                            None,
                            consumer.loads.first().or(consumer.stores.first()).expect("one reference").width,
                        );
                        reference.base = Some(pointer.value);
                        reference.pointer = true;
                        let cell = |arg: &Arg| match arg {
                            Arg::Cell(_) => Arg::Cell(Cell { r#ref: reference.clone() }),
                            _ => arg.clone(),
                        };
                        let mut changed = consumer.clone();
                        changed.name = "mov".to_owned();
                        changed.op = Some(OpCode::Operation(Operation::Move));
                        changed.args = consumer.args.iter().map(cell).collect();
                        changed.results = consumer.results.iter().map(cell).collect();
                        changed.uses = consumer
                            .uses
                            .iter()
                            .map(|value| if *value == outputs[0] { pointer.value } else { *value })
                            .collect();
                        changed.loads = if consumer.loads.is_empty() { Vec::new() } else { vec![reference.clone()] };
                        changed.stores =
                            if consumer.kind == Kind::Store { vec![reference.clone()] } else { Vec::new() };
                        changed.merges = OrderedMap::new();
                        changed.symbol = Some(true);
                        let changed = mir::detached(changed);
                        if consumer.kind == Kind::Arg {
                            let value = fresh(consumer.at, reference.width);
                            let mut load = changed;
                            load.kind = Kind::Load;
                            load.defines = vec![value.value];
                            load.results = vec![Arg::Held(value)];
                            load.stores = Vec::new();
                            load.id = None;
                            load.raised = None;
                            let load = mir::source_free(load);
                            let mut argument = consumer.clone();
                            argument.args = vec![Arg::Held(value)];
                            argument.uses = vec![value.value];
                            argument.loads = Vec::new();
                            replacements.insert(consumer_position, (Some(load), argument));
                        } else {
                            replacements.insert(consumer_position, (None, changed));
                        }
                    } else {
                        let data = loaded(&mut expanded, &mut fresh, at, &shape.data);
                        let result = Held { value: outputs[0], width: 2 };
                        arithmetic(&mut expanded, &mut fresh, at, width, Kind::Add, offset, data, Some(result));
                        let mut selector =
                            Op::new(op.at, OpCode::Operation(Operation::Move), "mov", Vec::new(), Vec::new());
                        selector.kind = Kind::Load;
                        selector.args = vec![Arg::Cell(Cell { r#ref: shape.selector.clone() })];
                        selector.results = vec![Arg::Opaque(Opaque::named(
                            Some(Loc::Reg(ir::Reg { register: Register::ES, width: 2 })),
                            "es",
                        ))];
                        selector.loads = vec![shape.selector.clone()];
                        selector.symbol = Some(true);
                        expanded.push(selector);
                    }
                    expanded[0] = mir::raising_owned(expanded[0].clone(), &[&op]);
                    origins.extend(expanded.iter().map(|_| None));
                    ops.extend(expanded);
                    arguments.clear();
                    continue;
                }
                arguments.clear();
            } else if !matches!(op.kind, Kind::Copy | Kind::Nothing)
                && !(matches!(
                    op.op,
                    Some(OpCode::Operation(Operation::Move | Operation::Binary | Operation::Unary | Operation::Extend))
                ) && op.stores.is_empty()
                    && !op.barrier()
                    && op.loads.iter().all(|reference| reference.addr.is_some() && reference.space != Some(Space::Stack))
                    && op.node().is_some_and(|node| {
                        let effects = node.effects();
                        matches!((&effects.uses, &effects.defs), (Some(uses), Some(defs))
                            if !uses.contains(&Register::ESP) && !defs.contains(&Register::ESP))
                    }))
            {
                arguments.clear();
            }
            ops.push(op);
            origins.push(origin);
        }
        blocks.push(block.with_ops(ops));
    }
    Ok(raising_array_bounds::proven(body.with_blocks(blocks), 10000))
}

#[cfg(test)]
#[path = "raising_array_access_tests.rs"]
mod tests;
