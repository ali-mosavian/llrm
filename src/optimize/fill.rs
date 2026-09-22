//! A counted loop that stores one value into consecutive cells is one fill.
//!
//! Port of `qbopt/optimize/fill.py`.

use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;

use crate::analysis::induction::{self, Affine, AffineOperand};
use crate::analysis::loops::{self as loopy, Loop};
use crate::model::ir::{Operation, Space};
use crate::model::mir::{
    self, Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Value,
};
use crate::model::passes::MIRTransform;
use crate::support::pyset::PySet;

// The test that keeps the loop going, by whether the bound itself runs.
const _INCLUSIVE: [(Kind, bool); 4] = [
    (Kind::Lt, false),
    (Kind::Below, false),
    (Kind::Le, true),
    (Kind::BelowEq, true),
];
const _INVERSE: [(Kind, Kind); 4] = [
    (Kind::Ge, Kind::Lt),
    (Kind::AboveEq, Kind::Below),
    (Kind::Gt, Kind::Le),
    (Kind::Above, Kind::BelowEq),
];

fn lookup<K: PartialEq + Copy, V: Copy>(table: &[(K, V)], key: Option<K>) -> Option<V> {
    let key = key?;
    table.iter().find(|(one, _)| *one == key).map(|(_, value)| *value)
}

pub struct Fill;

impl MIRTransform for Fill {
    fn class_name(&self) -> &'static str {
        "Fill"
    }

    fn name(&self) -> &str {
        "fill"
    }

    fn transform(&mut self, body: MirBody) -> Result<MirBody, String> {
        Ok(filled(&body))
    }
}

/// `body` with every such loop's body made one fill.
pub fn filled(body: &MirBody) -> MirBody {
    for loop_ in loopy::loops(&body.blocks, Some(body.entry)) {
        if let Some(made) = _filled(body, &loop_) {
            return filled(&made);
        }
    }
    body.clone()
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

fn _filled(body: &MirBody, loop_: &Loop) -> Option<MirBody> {
    let at_of = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<i64, &MirBlock>>();
    let inside = loop_.body.clone();
    if inside.len() != 2 || loop_.latches.len() != 1 {
        return None;
    }
    let header = at_of.get(&loop_.header).copied();
    let latch = at_of.get(loop_.latches.iter().next()?).copied();
    let (Some(header), Some(latch)) = (header, latch) else {
        return None;
    };
    if latch.at == header.at || latch.succ != [header.at] {
        return None;
    }
    if header.succ.len() != 2 || !header.succ.contains(&latch.at) {
        return None;
    }
    let exit_at = *header.succ.iter().find(|&&to| to != latch.at)?;
    let leaving = at_of.get(&exit_at);
    if leaving.is_none() || inside.contains(&exit_at) {
        return None;
    }

    let tested = _work(&header.ops);
    if tested.len() != 2 {
        return None;
    }
    let (compare, branch) = (tested[0], tested[1]);
    if branch.kind != Kind::Branch || !branch.target.is_some_and(|to| header.succ.contains(&to)) {
        return None;
    }
    let test = if branch.target == Some(latch.at) {
        branch.test
    } else {
        lookup(&_INVERSE, branch.test)
    };
    lookup(&_INCLUSIVE, test)?;
    let test = test?;
    let flags = compare
        .defines
        .iter()
        .filter(|value| value.flags)
        .copied()
        .collect::<Vec<_>>();
    if compare.kind != Kind::Sub
        || !compare.results.is_empty()
        || compare.args.len() != 2
        || !compare.loads.is_empty()
        || !compare.stores.is_empty()
        || compare.barrier()
        || flags.is_empty()
        || !flags.iter().any(|value| branch.uses.contains(value))
    {
        return None;
    }
    let (counter, bound) = (&compare.args[0], &compare.args[1]);
    let counters = induction::basics(body, loop_);
    let Arg::Held(counter) = counter else {
        return None;
    };
    if counters.len() != header.phis.len() || !counters.contains_key(&counter.value.id) {
        return None;
    }
    if !_stepping(counters.get(&counter.value.id)?, 1) {
        return None;
    }

    let mut defined = inside
        .iter()
        .flat_map(|at| at_of[at].phis.iter().map(|phi| phi.result))
        .collect::<BTreeSet<Value>>();
    defined.extend(
        inside
            .iter()
            .flat_map(|at| at_of[at].ops.iter().flat_map(|op| op.defines.iter().copied())),
    );
    let bound_width = match bound {
        Arg::Const(constant) => constant.width,
        Arg::Held(held) if !defined.contains(&held.value) => held.width,
        _ => return None,
    };
    if bound_width != counter.width {
        return None;
    }

    let work = _work(&latch.ops);
    let last = *work.last()?;
    if last.kind != Kind::Jump || last.target != Some(header.at) {
        return None;
    }
    let body_work = &work[..work.len() - 1];
    let stores = body_work
        .iter()
        .copied()
        .filter(|op| op.kind == Kind::Store)
        .collect::<Vec<_>>();
    if stores.len() != 1 {
        return None;
    }
    let store = stores[0];
    let others = body_work
        .iter()
        .copied()
        .filter(|op| !std::ptr::eq(*op, store))
        .collect::<Vec<_>>();
    if !_steps(header, latch, &others) {
        return None;
    }
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
        || addr.space == Space::Frame
    {
        return None;
    }
    let made = work
        .iter()
        .flat_map(|op| op.defines.iter().map(move |value| (value.id, *op)))
        .collect::<BTreeMap<u32, &Op>>();
    let index = match counters.get(&base.id) {
        Some(found) => Some(found.clone()),
        None => _offset(made.get(&base.id).copied(), &counters, &defined),
    };
    if !index.is_some_and(|index| _stepping(&index, reference.width)) {
        return None;
    }
    match value {
        Arg::Const(constant) if constant.width == reference.width => {}
        Arg::Held(held) if held.width == reference.width => {
            if defined.contains(&held.value) {
                return None;
            }
        }
        _ => return None,
    }
    // Nothing after the loop may read what it computed, but a counter the exit's phis take from the header.
    let mut left = PySet::<Value>::new();
    for block in &body.blocks {
        if inside.contains(&block.at) {
            continue;
        }
        if block
            .ops
            .iter()
            .any(|op| op.uses.iter().any(|value| defined.contains(value)))
        {
            return None;
        }
        for phi in &block.phis {
            for (&where_, &one) in phi.incoming.iter() {
                if !defined.contains(&one) {
                    continue;
                }
                if block.at != exit_at || where_ != header.at || !counters.contains_key(&one.id) {
                    return None;
                }
                left.add(one);
            }
        }
    }
    if left.iter().any(|one| {
        let found = &counters.get(&one.id).expect("left holds counters");
        !matches!(found.step, AffineOperand::Const(_)) || found.start.width() != counter.width
    }) {
        return None;
    }

    let mut fresh = _Fresh::new(body);
    let at = store.at;
    let mut prefix = Vec::<Op>::new();

    let mut emit = |kind: Kind, operation: Operation, args: Vec<Arg>, width: u32| -> Held {
        let result = fresh.held(at, width);
        let uses = held_values(&args);
        let mut op = Op::new(at, OpCode::Operation(operation), kind.as_str(), vec![result.value], uses);
        op.kind = kind;
        op.args = args;
        op.results = vec![Arg::Held(result)];
        prefix.push(op);
        result
    };

    let width = counter.width;
    let inclusive = lookup(&_INCLUSIVE, Some(test)).expect("test is inclusive or not");
    let mut count = match bound {
        Arg::Const(bound) => {
            let negated = emit(Kind::Neg, Operation::Unary, vec![Arg::Held(*counter)], width);
            emit(
                Kind::Add,
                Operation::Binary,
                vec![
                    Arg::Held(negated),
                    Arg::Const(Const::new(&bound.n + BigInt::from(u8::from(inclusive)), width)),
                ],
                width,
            )
        }
        _ => emit(Kind::Sub, Operation::Binary, vec![bound.clone(), Arg::Held(*counter)], width),
    };
    if !matches!(bound, Arg::Const(_)) && inclusive {
        count = emit(
            Kind::Add,
            Operation::Binary,
            vec![Arg::Held(count), Arg::Const(Const::new(1, width))],
            width,
        );
    }
    let mut finals = BTreeMap::<Value, Value>::new();
    for one in left.iter() {
        let AffineOperand::Const(step) = &counters.get(&one.id).expect("left holds counters").step else {
            unreachable!("left counters step by constants");
        };
        let moved = if step.n == BigInt::from(1) {
            count
        } else {
            emit(
                Kind::Mul,
                Operation::Binary,
                vec![Arg::Held(count), Arg::Const(Const::new(step.n.clone(), width))],
                width,
            )
        };
        let sum = emit(
            Kind::Add,
            Operation::Binary,
            vec![Arg::Held(Held { value: *one, width }), Arg::Held(moved)],
            width,
        );
        finals.insert(*one, sum.value);
    }
    let first = if matches!(addr.space, Space::Far | Space::Literal) {
        Arg::Const(Const::new(addr.disp, 2))
    } else {
        Arg::Symbol(mir::Symbol::new(addr.space, addr.index, addr.disp, 2))
    };
    let address = emit(
        Kind::Add,
        Operation::Binary,
        vec![Arg::Held(Held { value: base, width: 2 }), first],
        2,
    );
    let mut args = vec![value.clone(), Arg::Held(count), Arg::Held(address)];
    if let Some(segment) = reference.segment {
        args.push(Arg::Held(Held { value: segment, width: 2 }));
    }
    let mut fill = store.clone();
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
    let mut cells = MemRef::new(None, reference.width);
    cells.space = reference.space;
    cells.provenance = reference.provenance.clone();
    fill.stores = vec![cells];
    fill.source_backed = false;
    fill.raised = None;
    let mut ops = Vec::new();
    for op in &latch.ops {
        if std::ptr::eq(op, store) {
            ops.append(&mut prefix);
            ops.push(fill.clone());
        } else if std::ptr::eq(op, last) {
            let mut op = op.clone();
            op.target = Some(exit_at);
            ops.push(op);
        } else {
            ops.push(op.clone());
        }
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        if block.at == latch.at {
            block.ops = ops.clone();
            block.succ = vec![exit_at];
        } else if block.at == header.at {
            for phi in &mut block.phis {
                phi.incoming = phi
                    .incoming
                    .iter()
                    .filter(|(where_, _)| **where_ != latch.at)
                    .map(|(&where_, &one)| (where_, one))
                    .collect::<OrderedMap<_, _>>();
            }
        } else if block.at == exit_at {
            for phi in &mut block.phis {
                let from_header = *phi.incoming.get(&header.at).expect("KeyError");
                phi.incoming
                    .insert(latch.at, finals.get(&from_header).copied().unwrap_or(from_header));
            }
        }
        blocks.push(block);
    }
    Some(MirBody {
        blocks,
        ..body.clone()
    })
}

fn _stepping(counter: &Affine, by: u32) -> bool {
    matches!(&counter.step, AffineOperand::Const(step) if step.n == BigInt::from(by))
}

/// The counter `op` adds something the loop does not change to, stepping as it does.
fn _offset(
    op: Option<&Op>,
    counters: &OrderedMap<u32, Affine>,
    defined: &BTreeSet<Value>,
) -> Option<Affine> {
    let op = op?;
    if op.kind != Kind::Add
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.args.len() != 2
    {
        return None;
    }
    for (one, other) in [(&op.args[0], &op.args[1]), (&op.args[1], &op.args[0])] {
        let unchanged = match other {
            Arg::Const(_) | Arg::Symbol(_) => true,
            Arg::Held(held) => !defined.contains(&held.value),
            _ => false,
        };
        if let Arg::Held(one) = one {
            if unchanged {
                if let Some(found) = counters.get(&one.value.id) {
                    return Some(found.clone());
                }
            }
        }
    }
    None
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
        } else if !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || op.floating.is_some()
            || matches!(
                op.kind,
                Kind::Call | Kind::Escape | Kind::Opaque | Kind::Divmod | Kind::Udivmod
            )
            || !op.results.iter().all(|result| matches!(result, Arg::Held(_)))
        {
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
    let (Arg::Held(source), Arg::Const(_), Arg::Held(result)) = (&op.args[0], &op.args[1], &op.results[0])
    else {
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
            block
                .phis
                .iter()
                .flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.values().copied()))
        }));
        Self {
            serial: values.iter().map(|value| value.id).max().unwrap_or(0),
            variable: values.iter().map(|value| value.variable).max().unwrap_or(0),
        }
    }

    fn held(&mut self, at: i64, width: u32) -> Held {
        self.serial += 1;
        self.variable += 1;
        Held {
            value: Value {
                id: self.serial,
                at,
                flags: false,
                variable: self.variable,
                version: 1,
            },
            width,
        }
    }
}
