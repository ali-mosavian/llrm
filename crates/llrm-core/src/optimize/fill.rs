//! A counted loop that stores one value into consecutive cells is one fill.
//!
//! Port of `qbopt/optimize/fill.py`.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use num_bigint::BigInt;

use crate::analysis::induction::{self, Affine, AffineOperand};
use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::ssa;
use crate::model::ir::{Operation, Space};
use crate::model::mir::{
    self, Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Value,
};
use crate::model::memory::Provenance;
use crate::model::passes::MIRTransform;
use crate::support::pyset::PySet;

pub struct Fill;

impl MIRTransform for Fill {
    fn class_name(&self) -> &'static str {
        "Fill"
    }

    fn name(&self) -> &str {
        "fill"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        filled(&body)
    }
}

/// `body` with every such loop's body made one fill.
pub fn filled(body: &Rc<MirBody>) -> Result<Rc<MirBody>, String> {
    for loop_ in loopy::loops(&body.blocks, Some(body.entry)) {
        if let Some(made) = _filled(body, &loop_)? {
            return filled(&Rc::new(made));
        }
    }
    Ok(body.clone())
}

fn _work(ops: &[Op]) -> Vec<&Op> {
    ops.iter().filter(|op| op.kind != Kind::Nothing).collect()
}

fn held_values(args: &[Arg]) -> Vec<Value> {
    args.iter()
        .filter_map(|arg| match arg {
            Arg::Held(held) => Some(held.value),
            _ => None,
        })
        .collect()
}

fn width_of(arg: &Arg) -> u32 {
    match arg {
        Arg::Held(held) => held.width,
        Arg::Const(constant) => constant.width,
        other => panic!("AttributeError: {other:?} has no width"),
    }
}

/// What `_stored` and `_fill` find: value, cells, base, storage class, provenance and cell.
type Found = (Arg, BigInt, Held, Option<Space>, Option<Provenance>, Option<MemRef>);

fn _filled(body: &Rc<MirBody>, loop_: &Loop) -> Result<Option<MirBody>, String> {
    let at_of = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<i64, &MirBlock>>();
    let inside = loop_.body.iter().copied().collect::<BTreeSet<i64>>();
    let Some(header) = at_of.get(&loop_.header).copied() else {
        return Ok(None);
    };
    if loop_.latches.len() != 1 || header.succ.len() != 2 {
        return Ok(None);
    }
    // The body is one straight line back to the header, in whatever order its blocks lie.
    let Some(chain) = _chain(&at_of, header, &inside) else {
        return Ok(None);
    };
    let latch = *chain.last().expect("a chain has blocks");
    let exit_at = *header.succ.iter().find(|to| !inside.contains(to)).expect("StopIteration");
    if !at_of.contains_key(&exit_at) {
        return Ok(None);
    }

    // How many trips is `induction`'s to prove, whatever the counter's step or test.
    let tested = _work(&header.ops);
    let last_two = &tested[tested.len().saturating_sub(2)..];
    let proofs = induction::counted(body, loop_, None, false)
        .into_iter()
        .filter(|proof| {
            !proof.posttested
                && last_two.len() == 2
                && *last_two[0] == *proof.compare_in(body)
                && *last_two[1] == *proof.branch_in(body)
        })
        .collect::<Vec<_>>();
    if proofs.is_empty() || !tested[..tested.len().saturating_sub(2)].iter().all(|op| _pure(op)) {
        return Ok(None);
    }
    let proof = proofs.into_iter().next().expect("one proof");
    let counters = induction::basics(body, loop_);
    if counters.len() != header.phis.len() {
        return Ok(None);
    }

    let mut defined = inside
        .iter()
        .flat_map(|at| at_of[at].phis.iter().map(|phi| phi.result))
        .collect::<BTreeSet<Value>>();
    defined.extend(inside.iter().flat_map(|at| at_of[at].ops.iter().flat_map(|op| op.defines.iter().copied())));
    let mut work = chain
        .iter()
        .flat_map(|block| _work(&block.ops))
        .filter(|op| op.kind != Kind::Jump)
        .collect::<Vec<&Op>>();
    work.push(latch.ops.last().expect("IndexError"));
    let last = *work.last().expect("one operation");
    if last.kind != Kind::Jump || last.target != Some(header.at) {
        return Ok(None);
    }
    let effects = work[..work.len() - 1]
        .iter()
        .copied()
        .filter(|op| matches!(op.kind, Kind::Store | Kind::Fill))
        .collect::<Vec<_>>();
    if effects.len() != 1 {
        return Ok(None);
    }
    let effect = effects[0];
    let others = work[..work.len() - 1].iter().copied().filter(|op| !std::ptr::eq(*op, effect)).collect::<Vec<_>>();
    if !_steps(header, latch, &others) {
        return Ok(None);
    }
    let made = tested
        .iter()
        .chain(&work)
        .flat_map(|op| op.defines.iter().map(move |value| (value.id, *op)))
        .collect::<BTreeMap<u32, &Op>>();
    let found = if effect.kind == Kind::Store { _stored(effect, &defined) } else { _fill(effect, &defined) };
    let Some((value, cells, base, space, provenance, reference)) = found else {
        return Ok(None);
    };
    let index = match counters.get(&base.value.id) {
        Some(found) => Some(found.clone()),
        None => _offset(made.get(&base.value.id).copied(), &counters, &defined, &made),
    };
    if !index.is_some_and(|index| _stepping(&index, &(&cells * width_of(&value)))) {
        return Ok(None);
    }
    // Nothing after the loop may read what it computed, but a counter the exit's phis take from the header.
    let mut left = PySet::<Value>::new();
    for block in &body.blocks {
        if inside.contains(&block.at) {
            continue;
        }
        if block.ops.iter().any(|op| op.uses.iter().any(|value| defined.contains(value))) {
            return Ok(None);
        }
        for phi in &block.phis {
            for (&where_, &one) in phi.incoming.iter() {
                if !defined.contains(&one) {
                    continue;
                }
                if block.at != exit_at || where_ != header.at || !counters.contains_key(&one.id) {
                    return Ok(None);
                }
                left.add(one);
            }
        }
    }
    if left.iter().any(|one| {
        let found = &counters.get(&one.id).expect("left holds counters");
        !matches!(found.step, AffineOperand::Const(_)) || found.start.width() != proof.width()
    }) {
        return Ok(None);
    }

    let mut fresh = _Fresh::new(body);
    let at = effect.at;
    let mut prefix = Vec::<Op>::new();

    let mut emit = |kind: Kind, operation: Operation, args: Vec<Arg>, width: u32, name: &str| -> Held {
        let result = fresh.held(at, width);
        let uses = held_values(&args);
        let name = if name.is_empty() { kind.as_str() } else { name };
        let mut op = Op::new(at, OpCode::Operation(operation), name, vec![result.value], uses);
        op.kind = kind;
        op.args = args;
        op.results = vec![Arg::Held(result)];
        prefix.push(op);
        result
    };

    let Some(trips) = induction::trips(&proof, &mut |kind, args| {
        let width = width_of(&args[0]);
        AffineOperand::Held(emit(kind, Operation::Binary, args, width, ""))
    }) else {
        return Ok(None);
    };
    let width = trips.width();
    let trips = trips.as_arg();
    let count = if cells == BigInt::from(1) {
        trips.clone()
    } else {
        Arg::Held(emit(
            Kind::Mul,
            Operation::Binary,
            vec![trips.clone(), Arg::Const(Const::new(cells.clone(), width))],
            width,
            "",
        ))
    };
    let mut finals = BTreeMap::<Value, Value>::new();
    for one in left.iter() {
        let AffineOperand::Const(step) = &counters.get(&one.id).expect("left holds counters").step else {
            unreachable!("left counters step by constants");
        };
        let moved = if step.n == BigInt::from(1) {
            trips.clone()
        } else {
            Arg::Held(emit(
                Kind::Mul,
                Operation::Binary,
                vec![trips.clone(), Arg::Const(Const::new(step.n.clone(), width))],
                width,
                "",
            ))
        };
        let sum = emit(Kind::Add, Operation::Binary, vec![Arg::Held(Held { value: *one, width }), moved], width, "");
        finals.insert(*one, sum.value);
    }
    let (address, segment) = match reference.as_ref() {
        // A fill's address is already its first cell's, as the first trip computes it.
        None => (base, effect.args[3..].to_vec()),
        Some(reference) => {
            let addr = reference.addr.expect("a stored cell has an address");
            let first = if matches!(addr.space, Space::Far | Space::Literal) {
                Arg::Const(Const::new(addr.disp, 2))
            } else if addr.space == Space::Frame {
                // The frame is reached through bp, which no immediate names.
                let mut frame = reference.clone();
                frame.base = None;
                frame.width = 2;
                Arg::Held(emit(Kind::Address, Operation::Address, vec![Arg::Cell(mir::Cell { r#ref: frame })], 2, "lea"))
            } else {
                Arg::Symbol(mir::Symbol::new(addr.space, addr.index, addr.disp, 2))
            };
            let base = reference.base.expect("a stored cell has a base");
            let address = emit(Kind::Add, Operation::Binary, vec![Arg::Held(Held { value: base, width: 2 }), first], 2, "");
            let segment = reference.segment.map(|segment| Arg::Held(Held { value: segment, width: 2 })).into_iter().collect();
            (address, segment)
        }
    };
    let mut args = vec![value.clone(), count, Arg::Held(address)];
    args.extend(segment);
    let mut fill = effect.clone();
    fill.op = Some(OpCode::Operation(Operation::Fill));
    fill.name = Kind::Fill.as_str().to_owned();
    fill.kind = Kind::Fill;
    fill.uses = held_values(&args);
    fill.args = args;
    fill.results = Vec::new();
    fill.defines = Vec::new();
    // The exact cells are no longer named one by one, but the storage
    // class remains semantic.  Lowering needs it to select SS rather
    // than DS for a fill reached through a frame-derived near pointer.
    let mut cells_ref = MemRef::new(None, width_of(&value));
    cells_ref.space = space;
    cells_ref.provenance = provenance;
    fill.stores = vec![cells_ref];
    fill.source_backed = false;
    fill.raised = None;
    let mut lines = BTreeMap::<i64, Vec<Op>>::new();
    for block in &chain {
        let mut ops = Vec::new();
        for op in &block.ops {
            if std::ptr::eq(op, effect) {
                ops.extend(prefix.iter().cloned());
                ops.push(fill.clone());
            } else if std::ptr::eq(op, last) {
                let mut op = op.clone();
                op.target = Some(exit_at);
                ops.push(op);
            } else {
                ops.push(op.clone());
            }
        }
        lines.insert(block.at, ops);
    }
    // A proven positive count means the header's test passes on entry: it guards nothing.
    let entered = proof.count.as_ref().is_some_and(|count| *count != BigInt::from(0));
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        if block.at == latch.at {
            block.ops = lines[&block.at].clone();
            block.succ = vec![exit_at];
        } else if let Some(ops) = lines.get(&block.at) {
            block.ops = ops.clone();
        } else if block.at == header.at {
            for phi in &mut block.phis {
                phi.incoming = phi
                    .incoming
                    .iter()
                    .filter(|(where_, _)| **where_ != latch.at)
                    .map(|(&where_, &one)| (where_, one))
                    .collect::<OrderedMap<_, _>>();
            }
            if entered {
                let first = chain[0].at;
                let jump = mir::jump(block.ops.last().expect("IndexError"), first);
                block.ops.pop();
                block.ops.push(jump);
                block.succ = vec![first];
            }
        } else if block.at == exit_at {
            for phi in &mut block.phis {
                let from_header = *phi.incoming.get(&header.at).expect("KeyError");
                let mut incoming = phi
                    .incoming
                    .iter()
                    .filter(|(where_, _)| !(entered && **where_ == header.at))
                    .map(|(&where_, &one)| (where_, one))
                    .collect::<OrderedMap<_, _>>();
                incoming.insert(latch.at, finals.get(&from_header).copied().unwrap_or(from_header));
                phi.incoming = incoming;
            }
        }
        blocks.push(block);
    }
    let swap = blocks
        .iter()
        .find(|block| block.at == header.at)
        .expect("StopIteration")
        .phis
        .iter()
        .filter(|phi| phi.incoming.len() == 1)
        .map(|phi| (phi.result.id, *phi.incoming.values().next().expect("one incoming")))
        .collect::<BTreeMap<u32, Value>>();
    if !swap.is_empty() {
        let mut swapped = Vec::new();
        for mut block in blocks {
            let mut phis = Vec::new();
            for phi in &block.phis {
                if swap.contains_key(&phi.result.id) {
                    continue;
                }
                let mut phi = phi.clone();
                let mut incoming = OrderedMap::new();
                for (&where_, &one) in phi.incoming.iter() {
                    incoming.insert(where_, ssa::provider(one, &swap).map_err(|error| error.to_string())?);
                }
                phi.incoming = incoming;
                phis.push(phi);
            }
            block.phis = phis;
            block.ops = block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &swap).map_err(|error| error.to_string()))
                .collect::<Result<_, _>>()?;
            swapped.push(block);
        }
        blocks = swapped;
    }
    Ok(Some(body.with_blocks(blocks)))
}

fn _stepping(counter: &Affine, by: &BigInt) -> bool {
    matches!(&counter.step, AffineOperand::Const(step) if &step.n == by)
}

/// The loop's blocks after its header, when each has one way in and one out and the last goes back.
fn _chain<'a>(
    at_of: &BTreeMap<i64, &'a MirBlock>,
    header: &MirBlock,
    inside: &BTreeSet<i64>,
) -> Option<Vec<&'a MirBlock>> {
    let mut chain: Vec<&MirBlock> = Vec::new();
    let mut at = header.succ.iter().copied().find(|to| inside.contains(to));
    while let Some(here) = at.filter(|here| *here != header.at) {
        let block = *at_of.get(&here)?;
        if chain.iter().any(|one| **one == *block) || block.succ.len() != 1 || !block.phis.is_empty() {
            return None;
        }
        chain.push(block);
        at = Some(block.succ[0]);
    }
    (!chain.is_empty() && chain.len() + 1 == inside.len()).then_some(chain)
}

/// A one-cell store's value, cells, base, storage class, provenance and cell.
fn _stored(store: &Op, defined: &BTreeSet<Value>) -> Option<Found> {
    if !store.loads.is_empty() || store.barrier() || store.args.len() != 1 || store.results.len() != 1 {
        return None;
    }
    // The effect may carry what it is known to miss; the cell is the same.
    let Arg::Cell(cell) = &store.results[0] else {
        return None;
    };
    if store.stores.len() != 1 || {
        let mut one = store.stores[0].clone();
        one.excludes = Vec::new();
        let mut other = cell.r#ref.clone();
        other.excludes = Vec::new();
        one != other
    } {
        return None;
    }
    let (reference, value) = (&cell.r#ref, &store.args[0]);
    let (Some(addr), Some(base)) = (reference.addr, reference.base) else {
        return None;
    };
    if ![1, 2, 4].contains(&reference.width)
        || reference.segment.is_some() != (addr.space == Space::Far)
        || reference.segment.is_some_and(|segment| defined.contains(&segment))
        || reference.pointer
        || reference.symbolic.is_some()
        || reference.allocation.is_some()
        || reference.base_width != 2
        || !_unchanged(value, defined)
        || width_of(value) != reference.width
    {
        return None;
    }
    Some((
        value.clone(),
        BigInt::from(1),
        Held { value: base, width: 2 },
        reference.space,
        reference.provenance.clone(),
        Some(reference.clone()),
    ))
}

/// The same of a fill of a constant number of cells: a loop of them is one fill.
fn _fill(fill: &Op, defined: &BTreeSet<Value>) -> Option<Found> {
    let [value, count, address, segment @ ..] = fill.args.as_slice() else {
        panic!("ValueError: not enough values to unpack");
    };
    if !fill.loads.is_empty()
        || fill.barrier()
        || fill.stores.len() != 1
        || !matches!(count, Arg::Const(_))
        || !matches!(address, Arg::Held(_))
        || !_unchanged(value, defined)
        || !segment.iter().all(|one| _unchanged(one, defined))
    {
        return None;
    }
    let (Arg::Const(count), Arg::Held(address)) = (count, address) else { unreachable!("checked above") };
    Some((value.clone(), count.n.clone(), *address, fill.stores[0].space, fill.stores[0].provenance.clone(), None))
}

fn _unchanged(arg: &Arg, defined: &BTreeSet<Value>) -> bool {
    match arg {
        Arg::Const(_) => true,
        Arg::Held(held) => !defined.contains(&held.value),
        _ => false,
    }
}

/// The counter `op` adds things the loop does not change to, stepping as it does.
fn _offset(
    op: Option<&Op>,
    counters: &OrderedMap<u32, Affine>,
    defined: &BTreeSet<Value>,
    made: &BTreeMap<u32, &Op>,
) -> Option<Affine> {
    let op = op?;
    if op.kind != Kind::Add || !op.loads.is_empty() || !op.stores.is_empty() || op.barrier() || op.args.len() != 2 {
        return None;
    }
    for (one, other) in [(&op.args[0], &op.args[1]), (&op.args[1], &op.args[0])] {
        let Arg::Held(one) = one else {
            continue;
        };
        if !(matches!(other, Arg::Symbol(_)) || _unchanged(other, defined)) {
            continue;
        }
        let found = match counters.get(&one.value.id) {
            Some(found) => Some(found.clone()),
            None => _offset(made.get(&one.value.id).copied(), counters, defined, made),
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Work that stores nothing, reads nothing and cannot trap.
fn _pure(op: &Op) -> bool {
    !(!op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.floating.is_some()
        || matches!(op.kind, Kind::Call | Kind::Escape | Kind::Opaque | Kind::Divmod | Kind::Udivmod)
        || !op.results.iter().all(|result| matches!(result, Arg::Held(_))))
}

/// Whether each header counter steps once in `ops`, and the rest compute without effect.
///
/// Done once instead of every time round, work that stores nothing, reads
/// nothing and cannot trap leaves only values nothing after the loop reads.
fn _steps(header: &MirBlock, latch: &MirBlock, ops: &[&Op]) -> bool {
    let mut stepped = BTreeSet::new();
    for op in ops {
        let phi = _stepped(header, latch, op);
        if let Some(phi) = phi.filter(|phi| !stepped.contains(&phi.result)) {
            stepped.insert(phi.result);
        } else if !_pure(op) {
            return false;
        }
    }
    stepped.len() == header.phis.len()
}

/// The header phi `op` steps by a constant, if it is one.
fn _stepped<'a>(header: &'a MirBlock, latch: &MirBlock, op: &Op) -> Option<&'a mir::Phi> {
    if op.kind != Kind::Add
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.args.len() != 2
        || op.results.len() != 1
    {
        return None;
    }
    let (Arg::Held(source), Arg::Const(_), Arg::Held(result)) = (&op.args[0], &op.args[1], &op.results[0]) else {
        return None;
    };
    let phi = header.phis.iter().find(|phi| phi.result == source.value)?;
    if phi.incoming.get(&latch.at) != Some(&result.value) {
        return None;
    }
    Some(phi)
}

/// Values no operation in the body names yet.
struct _Fresh {
    serial: u32,
    variable: u32,
}

impl _Fresh {
    fn new(body: &MirBody) -> Self {
        let mut values = body
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter())
            .flat_map(|op| op.defines.iter().chain(op.uses.iter()).copied())
            .collect::<BTreeSet<Value>>();
        values.extend(body.blocks.iter().flat_map(|block| {
            block.phis.iter().flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.values().copied()))
        }));
        Self {
            serial: values.iter().map(|value| value.id).max().unwrap_or(0),
            variable: values.iter().map(|value| value.variable).max().unwrap_or(0),
        }
    }

    fn held(&mut self, at: i64, width: u32) -> Held {
        self.serial += 1;
        self.variable += 1;
        Held { value: Value { id: self.serial, at, flags: false, variable: self.variable, version: 1 }, width }
    }
}
