//! Port of `qbopt/frontend/arrayfacts.py`: must-facts for whole array
//! pointers across every CFG path.
//!
//! An in-bounds access establishes disjointness before its store is interpreted.
//! Unknown branches join numeric ranges; unknown effects invalidate allocation facts.
//! Loop-header widening and edge refinement establish inductive bounds without
//! enumerating iterations. Only converged states annotate accesses.
//! No original-register location participates in this analysis.

use std::collections::BTreeSet;

use num_bigint::BigInt;

use super::addressfacts::{Region, region};
use crate::analysis::ranges::{self, Interval};
use crate::analysis::{consts, loops};
use crate::model::mir::{self, Arg, Cell, Held, Kind, MemRef, MirBlock, Op, RaisedBody, Symbol, Value};
use crate::objectfile::module::{Addr, Space};
use crate::support::hash::IndexMap;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Pointer {
    pub allocation: Symbol,
    /// An `Int` or an `Interval`.
    pub offset: Box<Fact>,
    pub generation: (i64, usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Allocation {
    pub extent: i64,
    pub descriptor_size: u32,
    pub generation: (i64, usize),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Fact {
    Int(BigInt),
    Interval(Interval),
    Pointer(Pointer),
    Region(Region),
}

/// Python's `Addr | Pointer | Region`: what `_address` answers, and memory's keys.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Address {
    Addr(Addr),
    Pointer(Pointer),
    Region(Region),
}

/// What `_transfer` proved an access touches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Checked {
    Allocation(Symbol),
    Region(Region),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct State {
    pub values: IndexMap<Value, (Option<Fact>, u32)>,
    pub memory: IndexMap<(Address, u32), Fact>,
    pub allocations: IndexMap<Symbol, Allocation>,
    pub bindings: IndexMap<(Address, u32), Value>,
}

impl State {
    fn copy(&self) -> State {
        self.clone()
    }

    fn forget_memory(&mut self) {
        self.memory.clear();
        self.allocations.clear();
        self.bindings.clear();
    }
}

fn interval(low: impl Into<BigInt>, high: impl Into<BigInt>, width: u32) -> Interval {
    Interval { low: low.into(), high: high.into(), width }
}

fn sign(width: u32) -> BigInt {
    BigInt::from(1) << (width * 8 - 1)
}

/// Python `getattr(arg, "width", None)`.
fn width_of(arg: &Arg) -> Option<u32> {
    match arg {
        Arg::Held(one) => Some(one.width),
        Arg::Const(one) => Some(one.width),
        Arg::Symbol(one) => Some(one.width),
        Arg::FrameAddress(one) => Some(one.width),
        Arg::FrameSelector(one) => Some(one.width),
        Arg::Cell(_) | Arg::Opaque(_) => None,
    }
}

pub(crate) fn _span(fact: Option<&Fact>, width: u32) -> Option<Interval> {
    match fact? {
        Fact::Interval(fact) => (fact.width == width).then(|| fact.clone()),
        Fact::Int(fact) => {
            let sign = sign(width);
            let number: BigInt = ((fact & (BigInt::from(2) * &sign - 1)) ^ &sign) - &sign;
            Some(interval(number.clone(), number, width))
        }
        _ => None,
    }
}

pub(crate) fn _fitted(fact: Option<&Fact>, width: u32) -> Option<Fact> {
    match fact? {
        Fact::Int(fact) => Some(Fact::Int(consts::masked(fact, width))),
        Fact::Interval(fact) => {
            if fact.width < width {
                return None;
            }
            let sign = sign(width);
            if -&sign <= fact.low && fact.low <= fact.high && fact.high < sign {
                return Some(if fact.low == fact.high {
                    Fact::Int(consts::masked(&fact.low, width))
                } else {
                    Fact::Interval(Interval { width, ..fact.clone() })
                });
            }
            None
        }
        Fact::Region(_) if width == 2 => fact.cloned(),
        Fact::Pointer(_) if width == 4 => fact.cloned(),
        _ => None,
    }
}

pub(crate) fn _joined(facts: &[Option<Fact>], width: u32) -> Option<Fact> {
    if facts.is_empty() || facts.iter().any(Option::is_none) {
        return None;
    }
    let facts: Vec<&Fact> = facts.iter().flatten().collect();
    if facts.iter().all(|fact| *fact == facts[0]) {
        return Some(facts[0].clone());
    }
    let spans: Vec<Option<Interval>> = facts.iter().map(|fact| _span(Some(fact), width)).collect();
    if spans.iter().all(Option::is_some) {
        let spans: Vec<Interval> = spans.into_iter().flatten().collect();
        let low = spans.iter().map(|span| &span.low).min().expect("a span").clone();
        let high = spans.iter().map(|span| &span.high).max().expect("a span").clone();
        return Some(Fact::Interval(interval(low, high, width)));
    }
    if let Fact::Pointer(first) = facts[0] {
        let same = |fact: &&Fact| {
            matches!(fact, Fact::Pointer(one) if one.allocation == first.allocation && one.generation == first.generation)
        };
        if facts.iter().all(same) {
            let offsets: Vec<Option<Fact>> = facts
                .iter()
                .map(|fact| match fact {
                    Fact::Pointer(one) => Some((*one.offset).clone()),
                    _ => unreachable!("every fact is a pointer"),
                })
                .collect();
            let offset = _joined(&offsets, 4).expect("pointer offsets are width-4 numbers");
            return Some(Fact::Pointer(Pointer { offset: Box::new(offset), ..first.clone() }));
        }
    }
    None
}

pub(crate) fn _joined_values(facts: &[Option<(Option<Fact>, u32)>]) -> Option<(Option<Fact>, u32)> {
    if facts.is_empty() || facts.iter().any(Option::is_none) {
        return None;
    }
    let widths: BTreeSet<u32> = facts.iter().flatten().map(|fact| fact.1).collect();
    if widths.len() != 1 {
        return None;
    }
    let width = facts[0].as_ref().expect("checked").1;
    let joined = _joined(&facts.iter().flatten().map(|fact| fact.0.clone()).collect::<Vec<_>>(), width);
    joined.map(|joined| (Some(joined), width))
}

pub(crate) fn _meet(states: &[State]) -> State {
    let (first, rest) = states.split_first().expect("one state");
    let allocations = first
        .allocations
        .iter()
        .filter(|(key, value)| rest.iter().all(|other| other.allocations.get(*key) == Some(*value)))
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    let mut values = IndexMap::default();
    for key in first.values.keys() {
        let facts: Vec<_> = states.iter().map(|state| state.values.get(key).cloned()).collect();
        if let Some(joined) = _joined_values(&facts) {
            values.insert(*key, joined);
        }
    }
    let mut memory = IndexMap::default();
    for key in first.memory.keys() {
        let facts: Vec<_> = states.iter().map(|state| state.memory.get(key).cloned()).collect();
        if let Some(joined) = _joined(&facts, key.1) {
            memory.insert(key.clone(), joined);
        }
    }
    State { values, memory, allocations, bindings: IndexMap::default() }
}

fn plain(space: Space, disp: i64, index: i64) -> Addr {
    Addr { index, ..Addr::new(space, disp) }
}

pub(crate) fn _address(reference: &MemRef, state: &State) -> Option<Address> {
    if reference.pointer {
        if reference.base_width != 4 || reference.addr.is_some() || reference.segment.is_some() {
            return None;
        }
        let pointer = reference.base.and_then(|base| _read(&Arg::Held(Held { value: base, width: 4 }), state));
        if let Some(Fact::Pointer(pointer)) = pointer {
            if let Some(allocation) = state.allocations.get(&pointer.allocation) {
                let span = _span(Some(&pointer.offset), 4).expect("a pointer offset is a number");
                if pointer.generation == allocation.generation
                    && reference.width > 0
                    && BigInt::from(0) <= span.low
                    && span.high <= BigInt::from(allocation.extent - i64::from(reference.width))
                {
                    return Some(Address::Pointer(pointer));
                }
            }
        }
        return None;
    }
    let reference = mir::symbolic_ref(reference);
    if let (Some(base), 2, None, Some(addr)) = (reference.base, reference.base_width, reference.segment, reference.addr)
    {
        let base = _read(&Arg::Held(Held { value: base, width: 2 }), state);
        if addr.space == Space::Segment {
            if let Some(span) = _span(base.as_ref(), 2) {
                return region(plain(Space::Segment, addr.disp, addr.index), span, i64::from(reference.width))
                    .map(Address::Region);
            }
        }
        if addr.space == Space::Literal {
            if let Some(Fact::Region(base)) = &base {
                let shifted = base.shifted(&interval(addr.disp, addr.disp, 2));
                return shifted
                    .and_then(|shifted| region(shifted.anchor, shifted.offset, i64::from(reference.width)))
                    .map(Address::Region);
            }
        }
    }
    let addr = reference.addr?;
    (matches!(addr.space, Space::Segment | Space::Frame)
        && reference.base.is_none()
        && reference.segment.is_none()
        && addr == plain(addr.space, addr.disp, addr.index))
    .then_some(Address::Addr(addr))
}

pub(crate) fn _read(arg: &Arg, state: &State) -> Option<Fact> {
    match arg {
        Arg::Symbol(symbol) if symbol.space == Space::Segment && symbol.width == 2 => region(
            plain(symbol.space, symbol.offset + symbol.addend, symbol.index),
            interval(0, 0, 2),
            1,
        )
        .map(Fact::Region),
        Arg::Const(constant) => Some(Fact::Int(consts::masked(&constant.n, constant.width))),
        Arg::Held(held) => {
            let known = state.values.get(&held.value)?;
            if known.1 < held.width {
                return None;
            }
            _fitted(known.0.as_ref(), held.width)
        }
        Arg::Cell(cell) => {
            let address = _address(&cell.r#ref, state)?;
            match &address {
                Address::Region(_) => return None,
                Address::Pointer(pointer) if matches!(*pointer.offset, Fact::Interval(_)) => return None,
                _ => {}
            }
            state.memory.get(&(address, cell.r#ref.width)).cloned()
        }
        _ => None,
    }
}

pub(crate) fn _result(op: &Op, args: &[Option<Fact>]) -> Option<Fact> {
    if op.kind == Kind::Copy && op.args.len() == 1 && op.results.len() == 1 {
        if let Arg::Held(result) = &op.results[0] {
            if Some(result.width) != width_of(&op.args[0]) {
                return None;
            }
        }
    }
    if op.kind == Kind::Xor && op.args.len() == 2 && op.args[0] == op.args[1] {
        return Some(Fact::Int(0.into()));
    }
    if op.kind == Kind::Add && args.len() == 2 {
        let pair = match (&args[0], &args[1]) {
            (Some(Fact::Region(base)), delta) => Some((base, delta)),
            (delta, Some(Fact::Region(base))) => Some((base, delta)),
            _ => None,
        };
        if let Some((base, delta)) = pair {
            if op.args.iter().chain(&op.results).all(|arg| width_of(arg) == Some(2))
                && op.results.len() == 1
                && matches!(op.results[0], Arg::Held(_))
            {
                return _span(delta.as_ref(), 2).and_then(|span| base.shifted(&span)).map(Fact::Region);
            }
        }
    }
    match (op.kind, args) {
        (Kind::Copy | Kind::Load | Kind::Store, [value]) => return value.clone(),
        (Kind::PtrOffset, [Some(Fact::Pointer(pointer)), Some(delta @ (Fact::Int(_) | Fact::Interval(_)))]) => {
            let (Some(left), Some(right)) = (_span(Some(&pointer.offset), 4), _span(Some(delta), 4)) else {
                return None;
            };
            let offset = _fitted(Some(&Fact::Interval(interval(left.low + right.low, left.high + right.high, 4))), 4);
            return offset.map(|offset| Fact::Pointer(Pointer { offset: Box::new(offset), ..pointer.clone() }));
        }
        (Kind::SignExtend, [Some(Fact::Int(value))]) => {
            let sign = sign(width_of(&op.args[0]).expect("'Cell' object has no attribute 'width'"));
            return Some(Fact::Int((value ^ &sign) - &sign));
        }
        (Kind::Increment, [Some(Fact::Int(value))]) => return Some(Fact::Int(value + 1)),
        (Kind::Decrement, [Some(Fact::Int(value))]) => return Some(Fact::Int(value - 1)),
        (kind, [Some(Fact::Int(left)), Some(Fact::Int(right))]) => {
            if let Some((_, arith)) = consts::ARITH.iter().find(|(one, _)| *one == kind) {
                return Some(Fact::Int(arith(left, right)));
            }
        }
        _ => {}
    }
    if args.iter().any(|arg| matches!(arg, Some(Fact::Interval(_)))) {
        let mut known = IndexMap::default();
        for (operand, fact) in op.args.iter().zip(args) {
            if let Arg::Held(operand) = operand {
                if let Some(span) = _span(fact.as_ref(), operand.width) {
                    known.insert(operand.value, span);
                }
            }
        }
        return ranges::_computed(op, &known, &IndexMap::default()).map(Fact::Interval);
    }
    None
}

pub(crate) fn _overlap(left: &Address, width: u32, right: &Address, size: u32) -> bool {
    match (left, right) {
        (Address::Region(left), Address::Addr(right)) => left.overlaps(width, *right, size),
        (Address::Region(left), Address::Region(right)) => left.overlaps(width, right.clone(), size),
        (Address::Addr(left), Address::Region(right)) => right.overlaps(size, *left, width),
        (Address::Addr(left), Address::Addr(right)) => {
            left.space == right.space
                && left.index == right.index
                && left.disp < right.disp + i64::from(size)
                && right.disp < left.disp + i64::from(width)
        }
        (Address::Pointer(left), Address::Pointer(right)) => {
            let one = _span(Some(&left.offset), 4).expect("a pointer offset is a number");
            let other = _span(Some(&right.offset), 4).expect("a pointer offset is a number");
            left.allocation == right.allocation
                && left.generation == right.generation
                && one.low < other.high + size
                && other.low < one.high + width
        }
        _ => false,
    }
}

pub(crate) fn _transfer(block: &MirBlock, arriving: &State, cyclic: bool) -> (State, IndexMap<(usize, MemRef), Checked>) {
    let mut state = arriving.copy();
    state.bindings.clear();
    let mut checked = IndexMap::default();
    for (index, op) in block.ops.iter().enumerate() {
        if op.kind == Kind::Call || op.barrier() || op.floating.is_some() || op.kind == Kind::Fcheck {
            state.forget_memory();
            for value in &op.defines {
                state.values.shift_remove(value);
            }
            if let Some(request) = &op.array {
                if !request.replaces
                    && !op.memory_values.is_empty()
                    && !cyclic
                    && request.element_width > 0
                    && request.bounds.iter().all(|(low, high)| high >= low)
                {
                    let extent = request
                        .bounds
                        .iter()
                        .fold(BigInt::from(request.element_width), |extent, (low, high)| extent * (high - low + 1));
                    if BigInt::from(0) < extent && extent < BigInt::from(1_i64 << 31) {
                        let descriptor = request.descriptor;
                        let generation = (block.at, index);
                        state.allocations.insert(
                            descriptor,
                            Allocation {
                                extent: i64::try_from(extent).expect("below 2**31"),
                                descriptor_size: 14 + 4 * request.bounds.len() as u32,
                                generation,
                            },
                        );
                        let known: Vec<_> = op
                            .memory_values
                            .iter()
                            .filter_map(|(reference, value)| {
                                _address(reference, &state).map(|address| ((address, reference.width), Fact::Int(value.n.clone())))
                            })
                            .collect();
                        state.memory.extend(known);
                        state.memory.insert(
                            (Address::Addr(plain(descriptor.space, descriptor.offset + descriptor.addend, descriptor.index)), 4),
                            Fact::Pointer(Pointer { allocation: descriptor, offset: Box::new(Fact::Int(0.into())), generation }),
                        );
                    }
                }
            }
            continue;
        }
        let args: Vec<Option<Fact>> = op.args.iter().map(|arg| _read(arg, &state)).collect();
        let result = _result(op, &args);
        for value in &op.defines {
            state.values.shift_remove(value);
        }
        let held: Vec<&Held> = op
            .results
            .iter()
            .filter_map(|arg| match arg {
                Arg::Held(one) => Some(one),
                _ => None,
            })
            .collect();
        if held.len() == 1 && result.is_some() {
            if let Some(fitted) = _fitted(result.as_ref(), held[0].width) {
                state.values.insert(held[0].value, (Some(fitted), held[0].width));
            }
        }
        if op.kind == Kind::Load
            && held.len() == 1
            && op.loads.len() == 1
            && op.stores.is_empty()
            && op.args == [Arg::Cell(Cell { r#ref: op.loads[0].clone() })]
        {
            let reference = &op.loads[0];
            if let Some(Address::Addr(address)) = _address(reference, &state) {
                if matches!(reference.width, 2 | 4) && held[0].width == reference.width {
                    let key = (Address::Addr(address), reference.width);
                    if !state.memory.contains_key(&key) {
                        let sign = sign(reference.width);
                        state.memory.insert(key.clone(), Fact::Interval(interval(-&sign, sign - 1, reference.width)));
                    }
                    state.bindings.insert(key, held[0].value);
                }
            }
        }
        for reference in op.loads.iter().chain(&op.stores) {
            match _address(reference, &state) {
                Some(Address::Pointer(address)) if reference.pointer => {
                    checked.insert((index, reference.clone()), Checked::Allocation(address.allocation));
                }
                Some(Address::Region(address)) => {
                    checked.insert((index, reference.clone()), Checked::Region(address));
                }
                _ => {}
            }
        }
        for reference in &op.stores {
            let Some(address) = _address(reference, &state) else {
                state.forget_memory();
                continue;
            };
            if matches!(address, Address::Addr(_) | Address::Region(_)) {
                let descriptors: Vec<Symbol> = state.allocations.keys().copied().collect();
                for descriptor in descriptors {
                    let base = Address::Addr(plain(descriptor.space, descriptor.offset + descriptor.addend, descriptor.index));
                    if _overlap(&address, reference.width, &base, state.allocations[&descriptor].descriptor_size) {
                        // The complete descriptor is protected, not just its pointer word.
                        state.forget_memory();
                        break;
                    }
                }
            }
            state.memory.retain(|key, _| !_overlap(&address, reference.width, &key.0, key.1));
            state.bindings.retain(|key, _| !_overlap(&address, reference.width, &key.0, key.1));
            let fitted = _fitted(result.as_ref(), reference.width);
            let interval_pointer = matches!(&address, Address::Pointer(pointer) if matches!(*pointer.offset, Fact::Interval(_)));
            if let Some(fitted) = fitted {
                if !matches!(address, Address::Region(_)) && !interval_pointer {
                    state.memory.insert((address.clone(), reference.width), fitted);
                    if op.kind == Kind::Store && op.args.len() == 1 {
                        if let Arg::Held(value) = &op.args[0] {
                            state.bindings.insert((address, reference.width), value.value);
                        }
                    }
                }
            }
        }
    }
    (state, checked)
}

pub(crate) fn _edge(state: &State, block: &MirBlock, successor: i64) -> Option<State> {
    let mut known = IndexMap::default();
    for (value, (fact, width)) in &state.values {
        if let Some(span) = _span(fact.as_ref(), *width) {
            known.insert(*value, span);
        }
    }
    let refined = ranges::on_edge(block, successor, &known, None).expect("a successor")?;
    let mut result = state.copy();
    for (value, interval) in refined {
        let width = interval.width;
        result.values.insert(value, (_fitted(Some(&Fact::Interval(interval)), width), width));
    }
    for (key, value) in &state.bindings {
        let fact = _read(&Arg::Held(Held { value: *value, width: key.1 }), &result);
        if let Some(fact) = fact {
            if let Some(slot) = result.memory.get_mut(key) {
                *slot = fact;
            }
        }
    }
    Some(result)
}

pub(crate) fn _widened(previous: Option<&Fact>, current: Option<&Fact>, width: u32) -> Option<Fact> {
    let (Some(before), Some(after)) = (_span(previous, width), _span(current, width)) else {
        return current.cloned();
    };
    let sign = sign(width);
    let low = if after.low < before.low { -&sign } else { after.low };
    let high = if after.high > before.high { sign - 1 } else { after.high };
    _fitted(Some(&Fact::Interval(interval(low, high, width))), width)
}

pub(crate) fn _widen(previous: &State, current: &mut State) {
    for (value, (fact, width)) in current.values.iter_mut() {
        if let Some(before) = previous.values.get(value) {
            if before.1 == *width {
                *fact = _widened(before.0.as_ref(), fact.as_ref(), *width);
            }
        }
    }
    for (key, fact) in current.memory.iter_mut() {
        if let Some(before) = previous.memory.get(key) {
            *fact = _widened(Some(before), Some(fact), key.1).expect("a widened memory fact stays in range");
        }
    }
}

pub fn proven(body: RaisedBody, mut limit: i64) -> RaisedBody {
    let references: Vec<&MemRef> =
        body.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.loads.iter().chain(&op.stores)).collect();
    if !references.iter().any(|reference| reference.pointer || reference.base.is_some()) {
        return body;
    }
    let mut statics: Vec<(Addr, u32)> = Vec::new();
    for reference in &references {
        let Some(addr) = reference.addr else {
            continue;
        };
        let key = (addr, reference.width);
        if reference.base.is_none()
            && reference.segment.is_none()
            && addr == plain(addr.space, addr.disp, addr.index)
            && matches!(addr.space, Space::Segment | Space::Frame)
            && reference.width > 0
            && !statics.contains(&key)
        {
            statics.push(key);
        }
    }
    statics.sort_by_key(|key| (key.0.index, key.0.disp, key.1));
    let predecessors = loops::predecessors(&body.blocks);
    let by_at: IndexMap<i64, &MirBlock> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let revisited = |block: &MirBlock| {
        let (mut pending, mut seen) = (block.succ.clone(), BTreeSet::new());
        while let Some(at) = pending.pop() {
            if at == block.at {
                return true;
            }
            if !seen.contains(&at) {
                if let Some(next) = by_at.get(&at) {
                    seen.insert(at);
                    pending.extend(next.succ.iter().copied());
                }
            }
        }
        false
    };
    let cyclic: BTreeSet<i64> = body
        .blocks
        .iter()
        .filter(|block| block.ops.iter().any(|op| op.array.is_some()) && revisited(block))
        .map(|block| block.at)
        .collect();
    let headers: BTreeSet<i64> = loops::loops(&body.blocks, Some(body.entry)).into_iter().map(|one| one.header).collect();
    let mut outgoing: IndexMap<i64, State> = IndexMap::default();
    let mut entries: IndexMap<i64, State> = IndexMap::default();
    loop {
        let mut changed = false;
        for block in &body.blocks {
            let mut incoming_edges: IndexMap<i64, State> = IndexMap::default();
            for parent in &predecessors[&block.at] {
                if let Some(leaving) = outgoing.get(parent) {
                    if let Some(edge) = _edge(leaving, by_at[parent], block.at) {
                        incoming_edges.insert(*parent, edge);
                    }
                }
            }
            let mut incoming: Vec<State> = incoming_edges.values().cloned().collect();
            if block.at == body.entry {
                incoming.push(State::default());
            }
            if incoming.is_empty() {
                if outgoing.shift_remove(&block.at).is_some() {
                    entries.shift_remove(&block.at);
                    changed = true;
                }
                continue;
            }
            limit -= std::cmp::max(1, block.ops.len() as i64);
            if limit < 0 {
                return body;
            }
            let mut arriving = _meet(&incoming);
            for phi in &block.phis {
                let facts: Vec<_> = phi
                    .incoming
                    .iter()
                    .filter_map(|(parent, value)| incoming_edges.get(parent).map(|edge| edge.values.get(value).cloned()))
                    .collect();
                match _joined_values(&facts) {
                    Some(joined) => {
                        arriving.values.insert(phi.result, joined);
                    }
                    None => {
                        arriving.values.shift_remove(&phi.result);
                    }
                }
            }
            if headers.contains(&block.at) {
                if let Some(previous) = entries.get(&block.at) {
                    _widen(previous, &mut arriving);
                }
            }
            let (leaving, _) = _transfer(block, &arriving, cyclic.contains(&block.at));
            entries.insert(block.at, arriving);
            if outgoing.get(&block.at) != Some(&leaving) {
                outgoing.insert(block.at, leaving);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let Some(entry) = entries.get(&block.at) else {
            blocks.push(block.clone());
            continue;
        };
        let (_, checked) = _transfer(block, entry, cyclic.contains(&block.at));
        changed |= !checked.is_empty();
        let mut ops = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            let reference = |reference: &MemRef| -> MemRef {
                match checked.get(&(index, reference.clone())) {
                    Some(Checked::Region(allocation)) => {
                        let excludes = statics
                            .iter()
                            .filter(|key| !allocation.overlaps(reference.width, key.0, key.1))
                            .copied()
                            .collect();
                        MemRef { excludes, ..reference.clone() }
                    }
                    Some(Checked::Allocation(allocation)) => MemRef { allocation: Some(*allocation), ..reference.clone() },
                    None => reference.clone(),
                }
            };
            let argument = |arg: &Arg| match arg {
                Arg::Cell(cell) => Arg::Cell(Cell { r#ref: reference(&cell.r#ref) }),
                _ => arg.clone(),
            };
            let mut made = op.clone();
            made.loads = op.loads.iter().map(reference).collect();
            made.stores = op.stores.iter().map(reference).collect();
            made.args = op.args.iter().map(argument).collect();
            made.results = op.results.iter().map(argument).collect();
            ops.push(made);
        }
        blocks.push(block.with_ops(ops));
    }
    if changed { body.with_blocks(blocks) } else { body }
}

#[cfg(test)]
#[path = "arrayfacts_tests.rs"]
mod tests;
