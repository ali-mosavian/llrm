//! Algebraic identities over MIR values.
//!
//! Direct port of `qbopt/optimize/algebraic.py`.

use std::rc::Rc;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use crate::support::hash::HashMap;

use num_bigint::BigInt;
use num_traits::{One, Zero};

use crate::analysis::consts;
use crate::analysis::{loops, ssa};
use crate::model::ir::Operation;
use crate::model::mir::{
    self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Value,
};
use crate::optimize::{wholephis, wholestores};

type Definitions<'a> = BTreeMap<Value, &'a Op>;
/// Python's `Counter`: a missing key reads as zero.
type Counter = BTreeMap<Value, usize>;

fn times(uses: &Counter, value: &Value) -> usize {
    uses.get(value).copied().unwrap_or(0)
}

fn definitions_of(body: &MirBody) -> Definitions<'_> {
    body.blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect()
}

/// Python `getattr(arg, "width", None)`.
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

fn held_of(arg: &Arg) -> Option<Held> {
    match arg {
        Arg::Held(one) => Some(*one),
        _ => None,
    }
}

fn unique(values: impl IntoIterator<Item = Value>) -> Vec<Value> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

pub(crate) fn simplified(
    body: &Rc<MirBody>,
    wanted: &BTreeSet<Value>,
    wide: &BTreeSet<Value>,
) -> Result<MirBody, String> {
    let body = Rc::new(wholestores::joined(&wholephis::joined(body)));
    let body = _halved(&_divisions(&body));
    let body = _reassociated_recurrences(&body);
    let body = _forwarded_zero_tests(&body)?;
    let mut mentioned = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| {
            op.uses
                .iter()
                .filter(|value| !op.merges.contains_key(value))
                .copied()
        })
        .chain(
            body.blocks
                .iter()
                .flat_map(|block| &block.phis)
                .flat_map(|phi| phi.incoming.values().copied()),
        )
        .collect::<BTreeSet<_>>();
    mentioned.extend(
        body.blocks
            .iter()
            .flat_map(|block| &block.ops)
            .flat_map(_operands_read),
    );
    let definitions = definitions_of(&body);
    let mut uses = Counter::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        let mut read = _operands_read(op);
        read.extend(
            op.uses
                .iter()
                .filter(|value| !op.merges.contains_key(value))
                .copied(),
        );
        for value in read {
            *uses.entry(value).or_default() += 1;
        }
    }
    for value in body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.values())
    {
        *uses.entry(*value).or_default() += 1;
    }
    let seen = wanted | &mentioned;

    let simplify = |op: &Op| {
        let op = _extracted(op, &definitions);
        let op = _recombined(&op, &definitions);
        let op = _redundant_extension(&op, &definitions);
        let op = _zero_difference(&op, &definitions);
        let op = _negated_difference(&op, &definitions, &seen, &uses);
        let op = _shift_chain(&op, &definitions, &seen, &uses);
        let op = _product(&op, &seen, wide);
        let op = _scaled_chain(&op, &definitions, &seen, &uses);
        let op = _offset_chain(&op, &definitions, &seen, &uses);
        let op = _bitwise_chain(&op, &definitions, &seen, &uses);
        _simplified(&op, &seen, wide).into_owned()
    };

    let changed = body.with_blocks(body
            .blocks
            .iter()
            .map(|block| block.with_ops(block.ops.iter().map(simplify).collect()))
            .collect());
    let changed = _shared_shifts(&changed, &seen);
    let before = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().copied());
    let after = changed
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().copied())
        .collect::<BTreeSet<_>>();
    let removed = before
        .filter(|value| !after.contains(value))
        .collect::<BTreeSet<_>>();
    if removed.is_empty() {
        return Ok(changed);
    }
    Ok(changed.with_blocks(changed
            .blocks
            .iter()
            .map(|block| block.with_ops(block
                    .ops
                    .iter()
                    .map(|op| Op {
                        uses: op
                            .uses
                            .iter()
                            .filter(|value| {
                                !removed.contains(value) || !op.merges.contains_key(value)
                            })
                            .copied()
                            .collect(),
                        merges: op
                            .merges
                            .iter()
                            .filter(|(source, _)| !removed.contains(source))
                            .map(|(source, target)| (*source, *target))
                            .collect(),
                        ..op.clone()
                    })
                    .collect()))
            .collect()))
}

const _ZERO_FLAGS: [Kind; 11] = [
    Kind::Add,
    Kind::Sub,
    Kind::And,
    Kind::Or,
    Kind::Xor,
    Kind::Shl,
    Kind::Shr,
    Kind::Sar,
    Kind::Neg,
    Kind::Increment,
    Kind::Decrement,
];

/// Make an idempotent zero test consume its producer's condition.
///
/// The idempotent operation's result must be dead and every condition
/// consumer must ask solely whether it is zero.  The dead pass then removes
/// the redundant operation while retaining its source ownership.
pub(crate) fn _forwarded_zero_tests(body: &MirBody) -> Result<MirBody, String> {
    let definitions = definitions_of(body);
    let phi_inputs = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.values().copied())
        .collect::<BTreeSet<_>>();
    let exposed = mir::exposed(body);
    // One relation over the body, not one whole-body search per definition.
    let defined = definitions.keys().copied().collect::<BTreeSet<_>>();
    let users = ssa::use_index(body, Some(&defined), true);
    let user = |value: &Value| {
        users
            .get(value)
            .into_iter()
            .flatten()
            .map(|at| &body.blocks[at.block_index()].ops[at.operation_index()])
            .collect::<Vec<_>>()
    };
    let mut swaps = BTreeMap::<u32, Value>::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        if !matches!(op.kind, Kind::And | Kind::Or)
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || op.args.len() != 2
            || op.args[0] != op.args[1]
        {
            continue;
        }
        let Arg::Held(source) = op.args[0] else {
            continue;
        };
        if op.results.len() != 1 {
            continue;
        }
        let Arg::Held(result) = op.results[0] else {
            continue;
        };
        if result.width != source.width
            || phi_inputs.contains(&result.value)
            || exposed.contains(&result.value)
            || !user(&result.value).is_empty()
        {
            continue;
        }
        let conditions = op
            .defines
            .iter()
            .filter(|value| value.flags)
            .copied()
            .collect::<Vec<_>>();
        if conditions.len() != 1 {
            continue;
        }
        let condition = conditions[0];
        let consumers = user(&condition);
        if consumers.is_empty()
            || phi_inputs.contains(&condition)
            || exposed.contains(&condition)
            || consumers.iter().any(|one| {
                one.kind != Kind::Branch || !matches!(one.test, Some(Kind::Eq | Kind::Ne))
            })
        {
            continue;
        }
        let Some(producer) = definitions.get(&source.value) else {
            continue;
        };
        if !_ZERO_FLAGS.contains(&producer.kind) {
            continue;
        }
        let produced = producer
            .defines
            .iter()
            .filter(|value| value.flags)
            .copied()
            .collect::<Vec<_>>();
        if produced.len() != 1 || !producer.results.iter().any(|one| *one == Arg::Held(source)) {
            continue;
        }
        swaps.insert(condition.id, produced[0]);
    }
    if swaps.is_empty() {
        return Ok(body.clone());
    }
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let ops = block
            .ops
            .iter()
            .map(|op| ssa::substituted(op, &swaps).map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        blocks.push(block.with_ops(ops));
    }
    Ok(body.with_blocks(blocks))
}

/// Put a loop-carried operand at the root of an integer ADD tree.
///
/// Rotate only a two-level, single-use, memory-free ADD tree that is the
/// actual back-edge value of that phi.
pub(crate) fn _reassociated_recurrences(body: &MirBody) -> MirBody {
    let indexed = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let mut updates = BTreeMap::<Value, Value>::new();
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let header = indexed[&loop_.header];
        for phi in &header.phis {
            for latch in &loop_.latches {
                if let Some(value) = phi.incoming.get(latch) {
                    updates.insert(*value, phi.result);
                }
            }
        }
    }
    if updates.is_empty() {
        return body.clone();
    }

    let definitions = definitions_of(body);
    let mut uses = Counter::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        let mut read = _operands_read(op);
        read.extend(
            op.uses
                .iter()
                .filter(|value| !op.merges.contains_key(value))
                .copied(),
        );
        for value in read {
            *uses.entry(value).or_default() += 1;
        }
    }
    for value in body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.values())
    {
        *uses.entry(*value).or_default() += 1;
    }
    // Python's `id(op)`.
    let id = |op: &Op| std::ptr::from_ref(op) as usize;
    let locations = body
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter().map(move |op| (id(op), block.at)))
        .collect::<HashMap<_, _>>();
    let mut replacements = HashMap::<usize, Op>::default();

    let plain = |op: Option<&Op>| {
        op.is_some_and(|op| {
            op.kind == Kind::Add
                && op.op == Some(OpCode::Operation(Operation::Binary))
                && !(!op.loads.is_empty()
                    || !op.stores.is_empty()
                    || op.barrier()
                    || !op.merges.is_empty())
                && op.args.len() == 2
                && op.results.len() == 1
                && matches!(op.results[0], Arg::Held(result) if op.defines == [result.value])
        })
    };

    for block in &body.blocks {
        for outer in &block.ops {
            if !plain(Some(outer)) {
                continue;
            }
            let Arg::Held(result) = outer.results[0] else {
                continue;
            };
            let Some(&recurrence) = updates.get(&result.value) else {
                continue;
            };
            for (position, candidate) in outer.args.iter().enumerate() {
                let Arg::Held(candidate) = *candidate else {
                    continue;
                };
                let inner = definitions.get(&candidate.value).copied();
                if !plain(inner) {
                    continue;
                }
                let inner = inner.expect("plain");
                if locations[&id(inner)] != block.at
                    || times(&uses, &candidate.value) != 1
                    || inner.results != [Arg::Held(candidate)]
                {
                    continue;
                }
                let recurrent = inner
                    .args
                    .iter()
                    .filter(|arg| matches!(arg, Arg::Held(held) if held.value == recurrence))
                    .cloned()
                    .collect::<Vec<_>>();
                if recurrent.len() != 1 {
                    continue;
                }
                let other = outer.args[1 - position].clone();
                let leaves = inner
                    .args
                    .iter()
                    .filter(|arg| **arg != recurrent[0])
                    .cloned()
                    .chain([other])
                    .collect::<Vec<_>>();
                if leaves.len() != 2
                    || leaves.iter().any(|arg| {
                        !matches!(arg, Arg::Held(_) | Arg::Const(_))
                            || arg_width(arg) != Some(result.width)
                    })
                {
                    continue;
                }
                let inner_result = held_of(&inner.results[0]).expect("plain");
                let recurrent = held_of(&recurrent[0]).expect("held");
                replacements.insert(
                    id(inner),
                    Op {
                        uses: leaves
                            .iter()
                            .filter_map(held_of)
                            .map(|held| held.value)
                            .collect(),
                        args: leaves,
                        source_backed: false,
                        raised: None,
                        ..inner.clone()
                    },
                );
                replacements.insert(
                    id(outer),
                    Op {
                        args: vec![Arg::Held(recurrent), Arg::Held(inner_result)],
                        uses: vec![recurrent.value, inner_result.value],
                        source_backed: false,
                        raised: None,
                        ..outer.clone()
                    },
                );
                break;
            }
        }
    }

    if replacements.is_empty() {
        return body.clone();
    }
    body.with_blocks(body
            .blocks
            .iter()
            .map(|block| block.with_ops(block
                    .ops
                    .iter()
                    .map(|op| replacements.get(&id(op)).unwrap_or(op).clone())
                    .collect()))
            .collect())
}

/// Negating a single-use modular difference reverses its operands.
pub(crate) fn _negated_difference<'a>(
    op: &'a Op,
    definitions: &Definitions<'_>,
    wanted: &BTreeSet<Value>,
    uses: &Counter,
) -> Cow<'a, Op> {
    if op.kind != Kind::Neg
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || !op.merges.is_empty()
        || op.args.len() != 1
        || op.results.len() != 1
        || !op
            .args
            .iter()
            .chain(&op.results)
            .all(|arg| matches!(arg, Arg::Held(_)))
    {
        return Cow::Borrowed(op);
    }
    let (source, result) = (
        held_of(&op.args[0]).expect("held"),
        held_of(&op.results[0]).expect("held"),
    );
    if source.width != result.width || times(uses, &source.value) != 1 {
        return Cow::Borrowed(op);
    }
    let Some(difference) = definitions.get(&source.value) else {
        return Cow::Borrowed(op);
    };
    if difference.kind != Kind::Sub
        || !difference.loads.is_empty()
        || !difference.stores.is_empty()
        || difference.barrier()
        || !difference.merges.is_empty()
        || difference.results != [Arg::Held(source)]
        || difference.args.len() != 2
        || difference.args.iter().any(|arg| {
            !matches!(arg, Arg::Held(_) | Arg::Const(_)) || arg_width(arg) != Some(result.width)
        })
        || [op, *difference].iter().any(|one| {
            let first = held_of(&one.results[0]).map(|held| held.value);
            one.defines
                .iter()
                .any(|value| Some(*value) != first && wanted.contains(value))
        })
    {
        return Cow::Borrowed(op);
    }
    let args = difference.args.iter().rev().cloned().collect::<Vec<_>>();
    Cow::Owned(Op {
        kind: Kind::Sub,
        name: "sub".to_owned(),
        op: Some(OpCode::Operation(Operation::Binary)),
        defines: vec![result.value],
        uses: args
            .iter()
            .filter_map(held_of)
            .map(|held| held.value)
            .collect(),
        args,
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

/// Reuse a smaller available scale instead of shifting the original again.
pub(crate) fn _shared_shifts(body: &MirBody, wanted: &BTreeSet<Value>) -> MirBody {
    let mut used = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(_operands_read)
        .collect::<BTreeSet<_>>();
    used.extend(
        body.blocks
            .iter()
            .flat_map(|block| &block.phis)
            .flat_map(|phi| phi.incoming.values().copied()),
    );
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut available = BTreeMap::<Held, BTreeMap<BigInt, Held>>::new();
        let mut ops = Vec::new();
        for op in &block.ops {
            let mut op = op.clone();
            let scale = if op.kind == Kind::Shl {
                _scale(&op, wanted, true)
            } else {
                None
            };
            if let Some((source, factor)) = scale {
                let count = BigInt::from(factor.bits() - 1);
                let candidates = available.entry(source).or_default();
                let smaller = candidates
                    .keys()
                    .filter(|amount| **amount < count)
                    .cloned()
                    .collect::<Vec<_>>();
                let result = held_of(&op.results[0]).expect("scale");
                if let Some(amount) = smaller.into_iter().max() {
                    let previous = candidates[&amount];
                    let swap = |value: &Value| {
                        if *value == source.value {
                            previous.value
                        } else {
                            *value
                        }
                    };
                    op = Op {
                        args: vec![
                            Arg::Held(previous),
                            Arg::Const(Const::new(&count - &amount, 1)),
                        ],
                        defines: vec![result.value],
                        uses: op.uses.iter().map(swap).collect(),
                        merges: op
                            .merges
                            .iter()
                            .map(|(value, target)| (swap(value), *target))
                            .collect(),
                        source_backed: false,
                        raised: None,
                        ..op
                    };
                }
                if used.contains(&result.value) {
                    candidates.insert(count, result);
                }
            }
            ops.push(op);
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}

pub(crate) fn _scale(op: &Op, wanted: &BTreeSet<Value>, tied: bool) -> Option<(Held, BigInt)> {
    if !matches!(op.kind, Kind::Mul | Kind::Shl)
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || (!op.merges.is_empty() && (!tied || mir::partial(op)))
        || op.args.len() != 2
        || op.results.len() != 1
    {
        return None;
    }
    let Arg::Held(result) = op.results[0] else {
        return None;
    };
    if op
        .defines
        .iter()
        .any(|value| *value != result.value && wanted.contains(value))
    {
        return None;
    }
    let (Arg::Held(source), Arg::Const(factor)) = (&op.args[0], &op.args[1]) else {
        return None;
    };
    if source.width != result.width {
        return None;
    }
    if op.kind == Kind::Shl {
        if !(BigInt::zero() < factor.n && factor.n < BigInt::from(source.width * 8)) {
            return None;
        }
        let shift = u32::try_from(&factor.n).expect("bounded");
        return Some((*source, BigInt::one() << shift));
    }
    (factor.width == source.width).then(|| (*source, factor.n.clone()))
}

/// Combine single-use integer scales at an unchanged modular width.
pub(crate) fn _scaled_chain<'a>(
    op: &'a Op,
    definitions: &Definitions<'_>,
    wanted: &BTreeSet<Value>,
    uses: &Counter,
) -> Cow<'a, Op> {
    let Some((middle, factor)) = _scale(op, wanted, false) else {
        return Cow::Borrowed(op);
    };
    let Some(previous) = definitions.get(&middle.value) else {
        return Cow::Borrowed(op);
    };
    if times(uses, &middle.value) != 1 || previous.results != [Arg::Held(middle)] {
        return Cow::Borrowed(op);
    }
    let Some((source, initial)) = _scale(previous, wanted, false) else {
        return Cow::Borrowed(op);
    };
    let factor = consts::masked(&(initial * factor), source.width);
    Cow::Owned(Op {
        kind: Kind::Mul,
        args: vec![
            Arg::Held(source),
            Arg::Const(Const::new(factor, source.width)),
        ],
        defines: vec![held_of(&op.results[0]).expect("scale").value],
        uses: vec![source.value],
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

pub(crate) fn _offset(op: &Op, wanted: &BTreeSet<Value>) -> Option<(Held, BigInt)> {
    if !matches!(op.kind, Kind::Add | Kind::Sub)
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || !op.merges.is_empty()
        || op.args.len() != 2
        || op.results.len() != 1
    {
        return None;
    }
    let Arg::Held(result) = op.results[0] else {
        return None;
    };
    if op
        .defines
        .iter()
        .any(|value| *value != result.value && wanted.contains(value))
    {
        return None;
    }
    let (mut source, mut amount) = (&op.args[0], &op.args[1]);
    if op.kind == Kind::Add && matches!(source, Arg::Const(_)) {
        (source, amount) = (amount, source);
    }
    let (Arg::Held(source), Arg::Const(amount)) = (source, amount) else {
        return None;
    };
    if source.width != amount.width || source.width != result.width {
        return None;
    }
    Some((
        *source,
        if op.kind == Kind::Add {
            amount.n.clone()
        } else {
            -&amount.n
        },
    ))
}

/// Compose single-use modular offsets without preserving intermediate flags.
pub(crate) fn _offset_chain<'a>(
    op: &'a Op,
    definitions: &Definitions<'_>,
    wanted: &BTreeSet<Value>,
    uses: &Counter,
) -> Cow<'a, Op> {
    let Some((middle, amount)) = _offset(op, wanted) else {
        return Cow::Borrowed(op);
    };
    let Some(previous) = definitions.get(&middle.value) else {
        return Cow::Borrowed(op);
    };
    if times(uses, &middle.value) != 1 || previous.results != [Arg::Held(middle)] {
        return Cow::Borrowed(op);
    }
    let Some((source, initial)) = _offset(previous, wanted) else {
        return Cow::Borrowed(op);
    };
    let amount = consts::masked(&(initial + amount), source.width);
    Cow::Owned(Op {
        kind: Kind::Add,
        name: "add".to_owned(),
        op: Some(OpCode::Operation(Operation::Binary)),
        args: vec![
            Arg::Held(source),
            Arg::Const(Const::new(amount, source.width)),
        ],
        defines: vec![held_of(&op.results[0]).expect("offset").value],
        uses: vec![source.value],
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

const _ASSOCIATIVE_BITS: [Kind; 3] = [Kind::And, Kind::Or, Kind::Xor];

/// A pure fixed-width bitwise operation with one constant operand.
pub(crate) fn _bitwise(
    op: &Op,
    wanted: &BTreeSet<Value>,
    preserve_flags: bool,
) -> Option<(Held, BigInt)> {
    if !_ASSOCIATIVE_BITS.contains(&op.kind)
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || !op.merges.is_empty()
        || op.args.len() != 2
        || op.results.len() != 1
    {
        return None;
    }
    let Arg::Held(result) = op.results[0] else {
        return None;
    };
    let extra = op
        .defines
        .iter()
        .filter(|value| **value != result.value)
        .collect::<Vec<_>>();
    if extra.iter().any(|value| !value.flags)
        || (!preserve_flags && extra.iter().any(|value| wanted.contains(value)))
    {
        return None;
    }
    let (mut source, mut constant) = (&op.args[0], &op.args[1]);
    if matches!(source, Arg::Const(_)) {
        (source, constant) = (constant, source);
    }
    let (Arg::Held(source), Arg::Const(constant)) = (source, constant) else {
        return None;
    };
    if source.width != constant.width || source.width != result.width {
        return None;
    }
    Some((*source, consts::masked(&constant.n, source.width)))
}

/// Compose single-use associative bitwise constants at one modular width.
pub(crate) fn _bitwise_chain<'a>(
    op: &'a Op,
    definitions: &Definitions<'_>,
    wanted: &BTreeSet<Value>,
    uses: &Counter,
) -> Cow<'a, Op> {
    // The final bitwise operation still computes identical flags from its
    // identical result.  An intermediate's flags would disappear and are only
    // admissible when unobserved.
    let Some((middle, constant)) = _bitwise(op, wanted, true) else {
        return Cow::Borrowed(op);
    };
    let Some(previous) = definitions.get(&middle.value) else {
        return Cow::Borrowed(op);
    };
    if previous.kind != op.kind
        || times(uses, &middle.value) != 1
        || previous.results != [Arg::Held(middle)]
    {
        return Cow::Borrowed(op);
    }
    let Some((source, initial)) = _bitwise(previous, wanted, false) else {
        return Cow::Borrowed(op);
    };
    let arith = consts::ARITH.iter().find(|(kind, _)| *kind == op.kind).map(|(_, arith)| *arith).expect("bitwise");
    let combined = consts::masked(&arith(&initial, &constant), source.width);
    Cow::Owned(Op {
        args: vec![
            Arg::Held(source),
            Arg::Const(Const::new(combined, source.width)),
        ],
        uses: vec![source.value],
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

/// Joining both extracted halves of one value is that value, without a round trip.
pub(crate) fn _recombined<'a>(op: &'a Op, definitions: &Definitions<'_>) -> Cow<'a, Op> {
    if op.kind != Kind::Concat
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.args.len() != 2
        || op.results.len() != 1
        || !matches!(op.results[0], Arg::Held(result) if result.width == 4 && op.defines == [result.value])
    {
        return Cow::Borrowed(op);
    }
    let Some(original) = mir::extracted_whole(&op.args[0], &op.args[1], definitions) else {
        return Cow::Borrowed(op);
    };
    Cow::Owned(Op {
        kind: Kind::Copy,
        args: vec![Arg::Held(original)],
        uses: vec![original.value],
        merges: OrderedMap::new(),
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

/// Extracting a word just concatenated from two words recovers that word.
pub(crate) fn _extracted<'a>(op: &'a Op, definitions: &Definitions<'_>) -> Cow<'a, Op> {
    if op.kind != Kind::Extract
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.args.len() != 2
        || op.results.len() != 1
    {
        return Cow::Borrowed(op);
    }
    let (Arg::Held(whole), Arg::Const(offset)) = (&op.args[0], &op.args[1]) else {
        return Cow::Borrowed(op);
    };
    if whole.width != 4
        || !(offset.n == BigInt::zero() || offset.n == BigInt::from(16))
        || !matches!(op.results[0], Arg::Held(result) if result.width == 2)
    {
        return Cow::Borrowed(op);
    }
    let Some(joined) = definitions.get(&whole.value) else {
        return Cow::Borrowed(op);
    };
    if joined.kind != Kind::Concat
        || !joined.loads.is_empty()
        || !joined.stores.is_empty()
        || joined.barrier()
        || joined.args.len() != 2
        || joined.results.len() != 1
        || joined.results != [Arg::Held(*whole)]
        || joined
            .args
            .iter()
            .any(|arg| !matches!(arg, Arg::Held(_) | Arg::Const(_)) || arg_width(arg) != Some(2))
    {
        return Cow::Borrowed(op);
    }
    let (high, low) = (&joined.args[0], &joined.args[1]);
    let source = if offset.n.is_zero() { low } else { high };
    Cow::Owned(Op {
        op: Some(OpCode::Operation(Operation::Move)),
        name: "mov".to_owned(),
        kind: Kind::Copy,
        args: vec![source.clone()],
        uses: held_of(source).map(|held| held.value).into_iter().collect(),
        merges: OrderedMap::new(),
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

/// Reuse bits an earlier same-kind extension has already established.
///
/// A value zero-extended from 8 to 16 bits remains zero-extended when viewed
/// through any 8..16-bit slice; likewise for sign extension.  Mixed
/// signedness is deliberately excluded.
pub(crate) fn _redundant_extension<'a>(op: &'a Op, definitions: &Definitions<'_>) -> Cow<'a, Op> {
    if !matches!(op.kind, Kind::ZeroExtend | Kind::SignExtend)
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || !op.merges.is_empty()
        || op.args.len() != 1
        || op.results.len() != 1
    {
        return Cow::Borrowed(op);
    }
    let (Arg::Held(viewed), Arg::Held(result)) = (&op.args[0], &op.results[0]) else {
        return Cow::Borrowed(op);
    };
    let (viewed, result) = (*viewed, *result);
    if op.defines != [result.value] {
        return Cow::Borrowed(op);
    }
    let Some(previous) = definitions.get(&viewed.value) else {
        return Cow::Borrowed(op);
    };
    if previous.kind != op.kind
        || !previous.loads.is_empty()
        || !previous.stores.is_empty()
        || previous.barrier()
        || !previous.merges.is_empty()
        || previous.args.len() != 1
        || previous.results.len() != 1
        || !matches!(previous.results[0], Arg::Held(held) if held.value == viewed.value)
        || previous.defines != [viewed.value]
    {
        return Cow::Borrowed(op);
    }
    let source_width = arg_width(&previous.args[0]);
    let established = held_of(&previous.results[0]).expect("held").width;
    let Some(source_width) = source_width else {
        return Cow::Borrowed(op);
    };
    if !(source_width <= viewed.width && viewed.width < result.width && result.width <= established)
    {
        return Cow::Borrowed(op);
    }
    let known = Held {
        value: viewed.value,
        width: result.width,
    };
    Cow::Owned(Op {
        kind: Kind::Copy,
        args: vec![Arg::Held(known)],
        uses: vec![viewed.value],
        merges: OrderedMap::new(),
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

/// `0 - x` is the unary modular negation of x, with identical flags.
pub(crate) fn _zero_difference<'a>(op: &'a Op, definitions: &Definitions<'_>) -> Cow<'a, Op> {
    if op.kind != Kind::Sub
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || !op.merges.is_empty()
        || op.args.len() != 2
        || op.results.len() != 1
    {
        return Cow::Borrowed(op);
    }
    let (Arg::Held(source), Arg::Held(result)) = (&op.args[1], &op.results[0]) else {
        return Cow::Borrowed(op);
    };
    let (source, result) = (*source, *result);
    if op
        .defines
        .iter()
        .any(|value| *value != result.value && !value.flags)
    {
        return Cow::Borrowed(op);
    }
    let zero = &op.args[0];
    if arg_width(zero) != Some(source.width)
        || source.width != result.width
        || op.uses.iter().copied().collect::<BTreeSet<_>>()
            != op
                .args
                .iter()
                .filter_map(held_of)
                .map(|held| held.value)
                .collect::<BTreeSet<_>>()
        || !_copied_zero(zero, definitions)
    {
        return Cow::Borrowed(op);
    }
    Cow::Owned(Op {
        kind: Kind::Neg,
        name: "neg".to_owned(),
        op: Some(OpCode::Operation(Operation::Unary)),
        args: vec![Arg::Held(source)],
        uses: vec![source.value],
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

/// Whether an operand is zero through width-preserving, effect-free copies.
pub(crate) fn _copied_zero(arg: &Arg, definitions: &Definitions<'_>) -> bool {
    let width = arg_width(arg);
    let mut arg = arg.clone();
    let mut seen = BTreeSet::new();
    while let Arg::Held(held) = arg {
        if !seen.insert(held.value) {
            break;
        }
        let Some(made) = definitions.get(&held.value) else {
            return false;
        };
        if made.kind != Kind::Copy
            || !made.loads.is_empty()
            || !made.stores.is_empty()
            || made.barrier()
            || !made.merges.is_empty()
            || made.results != [Arg::Held(held)]
            || made.defines != [held.value]
            || made.args.len() != 1
            || arg_width(&made.args[0]) != width
        {
            return false;
        }
        arg = made.args[0].clone();
    }
    matches!(&arg, Arg::Const(constant)
        if Some(constant.width) == width && consts::masked(&constant.n, constant.width).is_zero())
}

pub(crate) fn _halves(op: &Op) -> Option<(Arg, Arg)> {
    if op.kind != Kind::Concat
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.args.len() != 2
        || !op
            .args
            .iter()
            .all(|arg| matches!(arg, Arg::Held(_) | Arg::Const(_)) && arg_width(arg) == Some(2))
        || op.results.len() != 1
        || !matches!(op.results[0], Arg::Held(result) if result.width == 4 && op.defines == [result.value])
    {
        return None;
    }
    Some((op.args[0].clone(), op.args[1].clone()))
}

/// Whether this reader of a joined value can read its two words instead.
pub(crate) fn _takes_halves(op: &Op, whole: Held, readers: &BTreeMap<Value, Vec<&Op>>) -> bool {
    if op.barrier() || !op.loads.is_empty() || !op.merges.is_empty() {
        return false;
    }
    match op.kind {
        Kind::Store => {
            let r#ref = if op.stores.len() == 1 {
                Some(&op.stores[0])
            } else {
                None
            };
            op.args == [Arg::Held(whole)]
                && op.defines.is_empty()
                && r#ref.is_some_and(|r#ref| {
                    r#ref.width == 4
                        && r#ref.addr.is_some()
                        && r#ref.base != Some(whole.value)
                        && r#ref.segment != Some(whole.value)
                })
        }
        Kind::Arg => op.args == [Arg::Held(whole)] && op.stores.is_empty() && op.defines.is_empty(),
        // Only the zero flag of `h | l` agrees with `whole - 0`.
        Kind::Sub => {
            op.args == [Arg::Held(whole), Arg::Const(Const::new(0, 4))]
                && op.stores.is_empty()
                && op.results.is_empty()
                && op.defines.iter().all(|value| value.flags)
                && op.defines.iter().all(|value| {
                    readers.get(value).into_iter().flatten().all(|reader| {
                        reader.kind == Kind::Branch
                            && matches!(reader.test, Some(Kind::Eq | Kind::Ne))
                    })
                })
        }
        _ => false,
    }
}

/// A value joined from two words, read only where words will do, is never joined.
pub(crate) fn _halved(body: &MirBody) -> MirBody {
    let joins = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter_map(|op| {
            _halves(op).map(|halves| (held_of(&op.results[0]).expect("halves").value, halves))
        })
        .collect::<BTreeMap<_, _>>();
    if joins.is_empty() {
        return body.clone();
    }
    let mut readers = BTreeMap::<Value, Vec<&Op>>::new();
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        let mut read = _operands_read(op);
        read.extend(op.uses.iter().copied());
        for value in read {
            readers.entry(value).or_default().push(op);
        }
    }
    let phied = body
        .blocks
        .iter()
        .flat_map(|block| &block.phis)
        .flat_map(|phi| phi.incoming.values().copied())
        .collect::<BTreeSet<_>>();
    let split = joins
        .into_iter()
        .filter(|(value, _halves)| {
            !phied.contains(value)
                && readers.get(value).into_iter().flatten().all(|op| {
                    _takes_halves(
                        op,
                        Held {
                            value: *value,
                            width: 4,
                        },
                        &readers,
                    )
                })
        })
        .collect::<BTreeMap<_, _>>();
    if split.is_empty() {
        return body.clone();
    }
    let mut values = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().chain(&op.uses).copied())
        .collect::<BTreeSet<_>>();
    values.extend(
        body.blocks
            .iter()
            .flat_map(|block| &block.phis)
            .flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.values().copied())),
    );
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);

    let mut rewritten = |op: &Op| -> Vec<Op> {
        let whole = op
            .args
            .iter()
            .filter_map(held_of)
            .map(|held| held.value)
            .find(|value| split.contains_key(value));
        let Some(whole) = whole else {
            return vec![op.clone()];
        };
        let (high, low) = split[&whole].clone();
        let fresh = |op: Op| Op {
            source_backed: false,
            raised: None,
            merges: OrderedMap::new(),
            ..op
        };
        let later = |op: Op| Op {
            absorbed: vec![],
            id: None,
            ..fresh(op)
        };
        let reads = |args: &[&Arg], r#ref: Option<&MemRef>| {
            let held = args
                .iter()
                .filter_map(|arg| held_of(arg))
                .map(|held| held.value);
            match r#ref {
                Some(r#ref) => {
                    unique(held.chain([r#ref.base, r#ref.segment].into_iter().flatten()))
                }
                None => unique(held),
            }
        };

        match op.kind {
            Kind::Store => {
                let r#ref = &op.stores[0];
                let words = [
                    (
                        low,
                        MemRef {
                            width: 2,
                            ..r#ref.clone()
                        },
                    ),
                    (
                        high,
                        MemRef {
                            addr: r#ref.addr.map(|addr| addr.plus(2)),
                            width: 2,
                            ..r#ref.clone()
                        },
                    ),
                ];
                words
                    .into_iter()
                    .enumerate()
                    .map(|(index, (word, cell))| {
                        let made = Op {
                            uses: reads(&[&word], Some(&cell)),
                            args: vec![word],
                            results: vec![Arg::Cell(Cell {
                                r#ref: cell.clone(),
                            })],
                            stores: vec![cell],
                            ..op.clone()
                        };
                        if index != 0 { later(made) } else { fresh(made) }
                    })
                    .collect()
            }
            Kind::Arg => [high, low]
                .into_iter()
                .enumerate()
                .map(|(index, word)| {
                    let made = Op {
                        uses: reads(&[&word], None),
                        args: vec![word],
                        ..op.clone()
                    };
                    if index != 0 { later(made) } else { fresh(made) }
                })
                .collect(),
            Kind::Sub => {
                serial += 1;
                variable += 1;
                let result = Held {
                    value: Value {
                        variable,
                        version: 1,
                        ..Value::new(serial, op.at)
                    },
                    width: 2,
                };
                vec![fresh(Op {
                    kind: Kind::Or,
                    name: "or".to_owned(),
                    op: Some(OpCode::Operation(Operation::Binary)),
                    uses: reads(&[&high, &low], None),
                    args: vec![high, low],
                    results: vec![Arg::Held(result)],
                    defines: std::iter::once(result.value)
                        .chain(op.defines.iter().copied())
                        .collect(),
                    ..op.clone()
                })]
            }
            _ => vec![op.clone()],
        }
    };

    body.with_blocks(body
            .blocks
            .iter()
            .map(|block| block.with_ops(block.ops.iter().flat_map(&mut rewritten).collect()))
            .collect())
}

pub(crate) fn _shift_chain<'a>(
    op: &'a Op,
    definitions: &Definitions<'_>,
    wanted: &BTreeSet<Value>,
    uses: &Counter,
) -> Cow<'a, Op> {
    if op.kind != Kind::Shl
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.args.len() != 2
        || op.results.len() != 1
    {
        return Cow::Borrowed(op);
    }
    let (Arg::Held(source), Arg::Const(count)) = (&op.args[0], &op.args[1]) else {
        return Cow::Borrowed(op);
    };
    let Some(previous) = definitions.get(&source.value) else {
        return Cow::Borrowed(op);
    };
    if times(uses, &source.value) != 1
        || previous.kind != Kind::Shl
        || previous.args.len() != 2
        || previous.results.len() != 1
    {
        return Cow::Borrowed(op);
    }
    let (Arg::Held(original), Arg::Const(first_count)) = (&previous.args[0], &previous.args[1])
    else {
        return Cow::Borrowed(op);
    };
    let first_result = held_of(&op.results[0]).map(|held| held.value);
    if previous.results[0] != Arg::Held(*source)
        || arg_width(&op.results[0]) != Some(source.width)
        || original.width != source.width
        || op
            .defines
            .iter()
            .any(|value| Some(*value) != first_result && wanted.contains(value))
    {
        return Cow::Borrowed(op);
    }
    let total = &first_count.n + &count.n;
    if first_count.n.clone().min(count.n.clone()) <= BigInt::zero()
        || total >= BigInt::from(source.width * 8)
    {
        return Cow::Borrowed(op);
    }
    Cow::Owned(Op {
        args: vec![
            Arg::Held(*original),
            Arg::Const(Const::new(total, count.width)),
        ],
        uses: unique(op.uses.iter().map(|value| {
            if *value == source.value {
                original.value
            } else {
                *value
            }
        })),
        source_backed: false,
        raised: None,
        ..op.clone()
    })
}

/// Divide by positive powers of two, biasing negatives to truncate toward zero.
pub(crate) fn _divisions(body: &Rc<MirBody>) -> Rc<MirBody> {
    if !body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .any(|op| op.kind == Kind::Divmod)
    {
        return body.clone();
    }
    let facts = consts::known(body, None, None, None, None);
    let mut values = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().chain(&op.uses).copied())
        .collect::<BTreeSet<_>>();
    values.extend(
        body.blocks
            .iter()
            .flat_map(|block| &block.phis)
            .flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.values().copied())),
    );
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let dividend_width = op.args.first().and_then(arg_width);
            if op.kind != Kind::Divmod
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || op.barrier()
                || op.args.len() != 2
                || op.results.len() != 2
                || !op
                    .results
                    .iter()
                    .all(|arg| matches!(arg, Arg::Held(held) if Some(held.width) == dividend_width))
                || !matches!(op.args[0], Arg::Held(_))
                || !matches!(dividend_width, Some(2 | 4))
                || !matches!(op.args[1], Arg::Held(_) | Arg::Const(_))
                || arg_width(&op.args[1]) != dividend_width
                || op.defines.iter().copied().collect::<BTreeSet<_>>()
                    != op
                        .results
                        .iter()
                        .filter_map(held_of)
                        .map(|held| held.value)
                        .collect::<BTreeSet<_>>()
            {
                ops.push(op.clone());
                continue;
            }
            let width = dividend_width.expect("checked");
            let bits = 8 * width;
            let fact = consts::_operand(op, &op.args[1], &facts, None);
            let divisor = match fact {
                Some(fact) if fact.width >= width => consts::masked(&fact.n, width),
                _ => BigInt::zero(),
            };
            if divisor <= BigInt::one()
                || divisor >= BigInt::one() << (bits - 1)
                || !(&divisor & (&divisor - 1u8)).is_zero()
            {
                ops.push(op.clone());
                continue;
            }
            let shift = u32::try_from(divisor.bits() - 1).expect("small");
            let mut sequence: Vec<Op> = Vec::new();

            let mut emit =
                |kind: Kind, args: Vec<Arg>, result: Option<Held>, sequence: &mut Vec<Op>| {
                    let result = result.unwrap_or_else(|| {
                        serial += 1;
                        variable += 1;
                        Held {
                            value: Value {
                                variable,
                                version: 1,
                                ..Value::new(serial, op.at)
                            },
                            width,
                        }
                    });
                    let first = sequence.is_empty();
                    sequence.push(Op {
                        kind,
                        uses: args
                            .iter()
                            .filter_map(held_of)
                            .map(|held| held.value)
                            .collect(),
                        args,
                        results: vec![Arg::Held(result)],
                        id: if first { op.id } else { None },
                        absorbed: if first { op.absorbed.clone() } else { vec![] },
                        ..Op::new(
                            op.at,
                            OpCode::Operation(Operation::Binary),
                            kind.to_string(),
                            vec![result.value],
                            vec![],
                        )
                    });
                    result
                };

            let dividend = op.args[0].clone();
            let sign = emit(
                Kind::Sar,
                vec![dividend.clone(), Arg::Const(Const::new(bits - 1, 1))],
                None,
                &mut sequence,
            );
            let adjusted = if divisor == BigInt::from(2) {
                // The bias is the sign's low bit, 0 or 1: subtracting the sign word adds it.
                emit(
                    Kind::Sub,
                    vec![dividend.clone(), Arg::Held(sign)],
                    None,
                    &mut sequence,
                )
            } else {
                let bias = emit(
                    Kind::And,
                    vec![
                        Arg::Held(sign),
                        Arg::Const(Const::new(&divisor - 1u8, width)),
                    ],
                    None,
                    &mut sequence,
                );
                emit(
                    Kind::Add,
                    vec![dividend.clone(), Arg::Held(bias)],
                    None,
                    &mut sequence,
                )
            };
            let quotient = emit(
                Kind::Sar,
                vec![Arg::Held(adjusted), Arg::Const(Const::new(shift, 1))],
                held_of(&op.results[0]),
                &mut sequence,
            );
            let product = emit(
                Kind::Shl,
                vec![Arg::Held(quotient), Arg::Const(Const::new(shift, 1))],
                None,
                &mut sequence,
            );
            emit(
                Kind::Sub,
                vec![dividend, Arg::Held(product)],
                held_of(&op.results[1]),
                &mut sequence,
            );
            ops.extend(sequence);
        }
        blocks.push(block.with_ops(ops));
    }
    Rc::new(body.with_blocks(blocks))
}

pub(crate) fn _product<'a>(op: &'a Op, wanted: &BTreeSet<Value>, wide: &BTreeSet<Value>) -> Cow<'a, Op> {
    if op.kind != Kind::Mul
        || op.barrier()
        || !op.stores.is_empty()
        || op.results.len() != 2
        || op.args.len() != 2
    {
        return Cow::Borrowed(op);
    }
    if !op
        .results
        .iter()
        .all(|result| matches!(result, Arg::Held(held) if held.width == 2))
    {
        return Cow::Borrowed(op);
    }
    if !op.args.iter().all(|arg| match arg {
        Arg::Held(_) | Arg::Const(_) => arg_width(arg) == Some(2),
        Arg::Cell(cell) => cell.r#ref.width == 2,
        _ => false,
    }) {
        return Cow::Borrowed(op);
    }
    let result = held_of(&op.results[0]).expect("held");
    if wide.contains(&result.value)
        || op
            .defines
            .iter()
            .any(|value| *value != result.value && wanted.contains(value))
    {
        return Cow::Borrowed(op);
    }
    let read = _operands_read(op);
    Cow::Owned(Op {
        results: vec![Arg::Held(result)],
        defines: vec![result.value],
        uses: op
            .uses
            .iter()
            .filter(|value| !op.merges.contains_key(value) || read.contains(value))
            .copied()
            .collect(),
        merges: OrderedMap::new(),
        ..op.clone()
    })
}

pub(crate) fn _operands_read(op: &Op) -> BTreeSet<Value> {
    op.args
        .iter()
        .filter_map(held_of)
        .map(|held| held.value)
        .chain(
            op.loads
                .iter()
                .chain(&op.stores)
                .flat_map(|r#ref| [r#ref.base, r#ref.segment].into_iter().flatten()),
        )
        .collect()
}

pub(crate) fn _simplified<'a>(op: &'a Op, wanted: &BTreeSet<Value>, wide: &BTreeSet<Value>) -> Cow<'a, Op> {
    if !op.loads.is_empty()
        || !op.stores.is_empty()
        || op.barrier()
        || op.results.len() != 1
        || op.args.len() != 2
    {
        return Cow::Borrowed(op);
    }
    let Arg::Held(result) = op.results[0] else {
        return Cow::Borrowed(op);
    };
    if !matches!(result.width, 2 | 4) {
        return Cow::Borrowed(op);
    }
    if op.kind == Kind::Concat {
        if let (Arg::Const(high), Arg::Const(low)) = (&op.args[0], &op.args[1]) {
            if high.width + low.width == result.width {
                let mask = |width: u32| (BigInt::one() << (width * 8)) - 1u8;
                let number =
                    ((&high.n & mask(high.width)) << (low.width * 8)) | (&low.n & mask(low.width));
                return Cow::Owned(Op {
                    kind: Kind::Copy,
                    args: vec![Arg::Const(Const::new(number, result.width))],
                    uses: vec![],
                    ..op.clone()
                });
            }
        }
        return Cow::Borrowed(op);
    }
    if result.width == 2 && wide.contains(&result.value) {
        return Cow::Borrowed(op);
    }
    if op
        .defines
        .iter()
        .any(|value| *value != result.value && wanted.contains(value))
    {
        return Cow::Borrowed(op);
    }
    let (mut left, mut right) = (&op.args[0], &op.args[1]);
    if ![left, right]
        .iter()
        .all(|arg| matches!(arg, Arg::Held(_) | Arg::Const(_) | Arg::Symbol(_)))
    {
        return Cow::Borrowed(op);
    }
    let shift = matches!(op.kind, Kind::Shl | Kind::Shr | Kind::Sar);
    if arg_width(left) != Some(result.width)
        || (arg_width(right) != Some(result.width) && !(shift && matches!(right, Arg::Const(_))))
    {
        return Cow::Borrowed(op);
    }
    if matches!(
        op.kind,
        Kind::Add | Kind::Mul | Kind::And | Kind::Or | Kind::Xor
    ) && matches!(left, Arg::Const(_))
    {
        (left, right) = (right, left);
    }
    let Arg::Const(right) = right else {
        return Cow::Borrowed(op);
    };
    let mask = (BigInt::one() << (result.width * 8)) - 1u8;
    let number = &right.n & ((BigInt::one() << (right.width * 8)) - 1u8);
    let answer = match op.kind {
        Kind::Add
        | Kind::Sub
        | Kind::Or
        | Kind::Xor
        | Kind::Shl
        | Kind::Shr
        | Kind::Sar
        | Kind::PtrOffset
            if number.is_zero() =>
        {
            left.clone()
        }
        Kind::Mul if number.is_one() => left.clone(),
        Kind::And if number == mask => left.clone(),
        Kind::Mul | Kind::And if number.is_zero() => Arg::Const(Const::new(0, result.width)),
        Kind::Or if number == mask => Arg::Const(Const::new(mask, result.width)),
        _ => return Cow::Borrowed(op),
    };
    Cow::Owned(Op {
        kind: Kind::Copy,
        uses: held_of(&answer)
            .map(|held| held.value)
            .into_iter()
            .collect(),
        args: vec![answer],
        defines: vec![result.value],
        merges: OrderedMap::new(),
        ..op.clone()
    })
}

#[cfg(test)]
#[path = "algebraic_tests.rs"]
mod tests;
