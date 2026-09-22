//! Port of `qbopt/optimize/transform.py`: MIR transforms, a body in, an
//! optimised body out.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::occurrence::OpOccurrence;
use crate::analysis::ssa::{provider as _provider, substituted as _substituted};
use crate::model::mir::{self, Arg, Held, Kind, MirBlock, MirBody, Op, OrderedMap, Phi, Value};

// ==== BEGIN S0: transform.py 60-129 (primary) ====
/// Remove selected computation while retaining exact source ownership.
///
/// Direct port of `qbopt.optimize.transform:_without`.
pub(crate) fn _without(ops: &[crate::model::mir::Op], drop: impl Fn(&crate::model::mir::Op) -> bool) -> Vec<crate::model::mir::Op> {
    let mut out = Vec::new();
    for op in ops {
        if !drop(op) {
            out.push(op.clone());
        } else if !op.absorbed.is_empty() || op.floating_origin.is_some() {
            out.push(_empty_operation(op));
        }
    }
    out
}

// ==== END S0 ====

// ==== BEGIN A: transform.py 130-784 (agent A) ====

pub(crate) fn _unchanged_float_environment(op: &Op) -> bool {
    use crate::model::floating::Exceptions;

    if op.barrier() || matches!(op.kind, Kind::Call | Kind::Opaque | Kind::Fcheck) {
        return false;
    }
    match &op.floating {
        None => op.stack.is_none(),
        Some(floating) => floating.exceptions == Exceptions::Deferred,
    }
}

// ==== END A ====

/// Keep opaque source ownership, but no computation or memory effect.
pub(crate) fn _empty_operation(op: &crate::model::mir::Op) -> crate::model::mir::Op {
    use crate::model::mir::{Kind, OpCode, OrderedMap};
    let mut result = op.clone();
    result.op = Some(OpCode::nothing());
    result.name = String::new();
    result.kind = Kind::Nothing;
    result.defines = Vec::new();
    result.uses = Vec::new();
    result.array = None;
    result.memory_values = Vec::new();
    result.floating = None;
    result.floating_origin = None;
    result.args = Vec::new();
    result.results = Vec::new();
    result.loads = Vec::new();
    result.stores = Vec::new();
    result.merges = OrderedMap::new();
    result.source_backed = false;
    result.raised = None;
    result.target = None;
    result.cases = Vec::new();
    result.symbol = Some(false);
    result.args_known = true;
    result.memory_complete = true;
    result.reads_complete = true;
    result.opaque_defs = Some(std::collections::BTreeSet::new());
    result.opaque_uses = Some(std::collections::BTreeSet::new());
    result.stack = None;
    result.test = None;
    result.indirect = false;
    result
}

// ==== BEGIN B: transform.py 819-1004 (agent B) ====
// ==== END B ====

/// Direct port of `qbopt.optimize.transform:_preheader`.
///
/// The one block entering `loop_` from outside it, if exactly one source
/// block occurrence does.  This deliberately walks `body.blocks` rather
/// than a predecessor map: Python preserves both source order and duplicate
/// block occurrences in the list it counts.
pub(crate) fn _preheader(body: &MirBody, loop_: &Loop) -> Option<i64> {
    let outside = body
        .blocks
        .iter()
        .filter(|block| block.succ.contains(&loop_.header) && !loop_.body.contains(&block.at))
        .map(|block| block.at)
        .collect::<Vec<_>>();
    if outside.len() == 1 {
        Some(outside[0])
    } else {
        None
    }
}

// ==== BEGIN C1: transform.py 1018-1396 (agent C) ====

/// Every value some instruction reads, rather than merely preserves.
///
/// Transitive through phis, and grown from what is definitely read so a
/// cycle cannot talk itself into being effective.
pub(crate) fn _effective(body: &MirBody, calls: &IndexMap<i64, String>) -> BTreeSet<Value> {
    let _ = calls;
    let mut wanted: BTreeSet<Value> = BTreeSet::new();
    let mut carrying: IndexMap<Value, BTreeSet<Value>> = IndexMap::new();
    for block in &body.blocks {
        for phi in &block.phis {
            for value in phi.incoming.values() {
                carrying.entry(phi.result).or_default().insert(*value);
            }
        }
        for op in &block.ops {
            // `merges` records preservation, not non-consumption: `and ax,ax`
            // reads AX and preserves EAX's upper half.  `mir::consumed` keeps
            // explicit operands and address bases even when merged.
            wanted.extend(_consumed(op));
        }
    }
    let mut changing = true;
    while changing {
        changing = false;
        for (result, incoming) in &carrying {
            if wanted.contains(result) && !incoming.is_subset(&wanted) {
                wanted.extend(incoming.iter().copied());
                changing = true;
            }
        }
    }
    wanted
}

/// Values a phi in this loop carries in from somewhere.
pub(crate) fn _starts(phis: &[&Phi]) -> BTreeSet<Value> {
    phis.iter().flat_map(|phi| phi.incoming.values().copied()).collect()
}

/// Values a phi carries that the loop goes on to define again.
pub(crate) fn _rewritten(ops: &[&Op], phis: &[&Phi]) -> BTreeSet<Value> {
    let inside = ops
        .iter()
        .flat_map(|one| one.defines.iter().copied())
        .filter(|value| !value.flags)
        .collect::<BTreeSet<_>>();
    let mut out = BTreeSet::new();
    for phi in phis {
        let coming = phi.incoming.values().copied().collect::<BTreeSet<_>>();
        if !coming.is_disjoint(&inside) {
            out.extend(coming);
        }
    }
    out
}

/// Whether this divide can be performed where it might not have been.
pub(crate) fn _cannot_fault(op: &Op) -> bool {
    if op.kind != Kind::Divmod || op.args.len() != 2 {
        return false;
    }
    let Arg::Const(divisor) = &op.args[1] else {
        return false;
    };
    let masked = crate::analysis::consts::masked(&divisor.n, divisor.width);
    let all = (num_bigint::BigInt::from(1) << (divisor.width * 8)) - 1;
    masked != num_bigint::BigInt::from(0) && masked != all
}

/// A complete scalar definition needs no loop-carried destination contents.
pub(crate) fn _whole_shift(op: &Op, readable: Option<&BTreeSet<Value>>) -> bool {
    match (op.kind, op.args.as_slice(), op.results.as_slice()) {
        (Kind::Shl, [Arg::Held(Held { width, .. }), Arg::Const(count)], [Arg::Held(Held { width: result_width, .. })]) => {
            let bits = num_bigint::BigInt::from(width * 8);
            width == result_width
                && num_bigint::BigInt::from(0) < count.n
                && count.n < bits
                && op.merges.is_empty()
                && readable.is_some_and(|readable| {
                    !op.defines.iter().any(|value| value.flags && readable.contains(value))
                })
        }
        _ => false,
    }
}

/// Whether this pure operation defines one complete, reparentable value.
pub(crate) fn _complete_value(op: &Op, readable: Option<&BTreeSet<Value>>) -> bool {
    let Some(readable) = readable else {
        return false;
    };
    if matches!(
        op.kind,
        Kind::Copy | Kind::AddCarry | Kind::SubBorrow | Kind::Divmod | Kind::Udivmod
    ) || !op.loads.is_empty()
        || !op.stores.is_empty()
        || !op.merges.is_empty()
        || op.barrier()
        || op.floating.is_some()
        || op.stack.is_some()
        || op.results.len() != 1
    {
        return false;
    }
    let Arg::Held(result) = &op.results[0] else {
        return false;
    };
    let values = op.defines.iter().copied().filter(|value| !value.flags).collect::<BTreeSet<_>>();
    values == BTreeSet::from([result.value])
        && !op.defines.iter().any(|value| value.flags && readable.contains(value))
}

/// `mir.consumed`.
pub(crate) fn _consumed(op: &Op) -> BTreeSet<Value> {
    mir::consumed(op)
}

/// The ops in this loop whose result never changes, in order.
///
/// Grown rather than filtered, to a fixed point.  A loop holding a call is
/// refused whole.  `intervals` and `floating_allowed` are keyed by Python's
/// `id(op)`, the operation's address.
#[allow(clippy::too_many_arguments)]
pub(crate) fn _invariant_run<'a>(
    ops: &[&'a Op],
    carried: &BTreeSet<Value>,
    stores: &[(&crate::model::mir::MemRef, Option<&BTreeMap<Value, crate::analysis::ranges::Interval>>)],
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    phis: &[&Phi],
    bounds: Option<&IndexMap<(crate::objectfile::module::Space, i64), Vec<i64>>>,
    starts: Option<&BTreeSet<Value>>,
    readable: Option<&BTreeSet<Value>>,
    intervals: Option<&std::collections::HashMap<usize, &BTreeMap<Value, crate::analysis::ranges::Interval>>>,
    nonempty: bool,
    floating_allowed: &BTreeSet<usize>,
) -> Result<Vec<&'a Op>, String> {
    let _ = (dgroup, calls);
    let id = |op: &Op| std::ptr::from_ref(op) as usize;
    let layout = bounds.map(|bounds| crate::analysis::regions::RegionLayout {
        shared_segments: None,
        landmarks: bounds.iter().map(|(key, marks)| (*key, marks.clone())).collect(),
    });
    // An opaque machine barrier makes every unmodelled resource observable;
    // a source volatile access has a complete footprint and only orders
    // itself against other volatile accesses.
    if ops
        .iter()
        .any(|one| matches!(one.kind, Kind::Call | Kind::Escape) || (one.barrier() && !one.volatile))
    {
        return Ok(Vec::new());
    }
    let mut made: BTreeSet<Value> = BTreeSet::new();
    let mut run: Vec<&'a Op> = Vec::new();
    let twice = _rewritten(ops, phis);
    let empty = BTreeSet::new();
    let begins = starts.unwrap_or(&empty);
    // A phi result is the loop-carried value itself.  Python rebuilds this
    // per candidate; it depends on neither.
    let inside = ops
        .iter()
        .flat_map(|other| other.defines.iter().copied())
        .chain(carried.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut changing = true;
    while changing {
        changing = false;
        for &one in ops {
            // `mir.instruction`
            let real = one.kind != Kind::Nothing;
            if run.iter().any(|other| **other == *one)
                || one.volatile
                || !one.stores.is_empty()
                || (one.floating.is_some() && !floating_allowed.contains(&id(one)))
                || !real
            {
                continue;
            }
            // A branch is where the loop is.
            if matches!(one.kind, Kind::Jump | Kind::Branch) {
                continue;
            }
            // Nor a divide that could trap on a zero-trip path.
            if matches!(one.kind, Kind::Divmod | Kind::Udivmod) && !_cannot_fault(one) {
                continue;
            }
            // Nor anything that computes nothing; and a long add must keep
            // its carry partner, so an opaque result stays.
            if one.results.iter().any(|result| matches!(result, Arg::Opaque(_))) {
                continue;
            }
            if one.kind == Kind::Copy
                && one.loads.is_empty()
                && !one.uses.iter().any(|value| made.contains(value))
                && !(one.args.len() == 1 && matches!(one.args[0], Arg::Symbol(_)))
                && !_literal(one)
            {
                continue;
            }
            // A value a phi carries whose variable the loop rewrites stays:
            // segld's inner counter.  A stable load may replace its carried
            // initial value only once an iteration is proven to execute.
            if one.defines.iter().filter(|value| !value.flags).any(|value| {
                begins.contains(value) && twice.contains(value) && readable.is_none_or(|readable| readable.contains(value))
            }) && !(one.kind == Kind::Copy
                && one.merges.is_empty()
                && one.args.len() == 1
                && matches!(one.args[0], Arg::Symbol(_)))
                && !_whole_shift(one, readable)
                && !_complete_value(one, readable)
                && !(nonempty && !one.loads.is_empty() && one.merges.is_empty())
            {
                continue;
            }
            let known = intervals.and_then(|intervals| intervals.get(&id(one)).copied());
            let mut overlaps = false;
            'refs: for reference in &one.loads {
                for (other, theirs) in stores {
                    if reference.addr.is_none()
                        || crate::analysis::regions::overlapping(reference, other, known, *theirs, layout.as_ref())
                            .map_err(|error| format!("{error:?}"))?
                    {
                        overlaps = true;
                        break 'refs;
                    }
                }
            }
            if overlaps {
                continue;
            }
            // Per use, not per value: the compare reads the counter, the
            // load of `n` only preserves the register it lives in.
            let blocking = _consumed(one).into_iter().filter(|value| !value.flags).collect::<Vec<_>>();
            if blocking.iter().any(|value| inside.contains(value) && !made.contains(value)) {
                continue;
            }
            run.push(one);
            made.extend(one.defines.iter().copied());
            changing = true;
        }
    }

    let mut thinning = true;
    while thinning {
        thinning = false;
        for &one in &run {
            if one.kind != Kind::Copy
                || !one.loads.is_empty()
                || one.args.iter().any(|arg| matches!(arg, Arg::Symbol(_)))
                || _literal(one)
            {
                continue;
            }
            if one.defines.iter().any(|value| {
                run.iter()
                    .any(|other| !std::ptr::eq(*other, one) && other.uses.contains(value))
            }) {
                continue;
            }
            let drop = one.defines.iter().copied().collect::<BTreeSet<_>>();
            run = _pruned(&run, &drop);
            thinning = true;
            break;
        }
    }
    Ok(run)
}

/// The run without the values named, and without whatever fed only them.
pub(crate) fn _pruned<'a>(run: &[&'a Op], drop: &BTreeSet<Value>) -> Vec<&'a Op> {
    let mut keep = run
        .iter()
        .copied()
        .filter(|one| !one.defines.iter().any(|value| drop.contains(value)))
        .collect::<Vec<_>>();
    let mut changing = true;
    while changing {
        changing = false;
        let gone = run
            .iter()
            .filter(|one| !keep.iter().any(|kept| **kept == ***one))
            .flat_map(|one| one.defines.iter().copied())
            .collect::<BTreeSet<_>>();
        for one in keep.clone() {
            if one.uses.iter().any(|value| gone.contains(value)) {
                // `list.remove`: the first equal element.
                if let Some(index) = keep.iter().position(|kept| **kept == *one) {
                    keep.remove(index);
                }
                changing = true;
            }
        }
    }
    keep
}

/// The values the run computes that the rest of the loop still reads.
///
/// None where any is a flag: a flag cannot cross into the loop in a register.
pub(crate) fn _crossing(
    run: &[&Op],
    rest: &[&Op],
    phis: Option<&[&Phi]>,
    wanted: Option<&BTreeSet<Value>>,
) -> Option<BTreeSet<Value>> {
    let crossing = _crossed_values(run, rest, phis, wanted);
    if crossing.is_empty() || crossing.iter().any(|value| value.flags) {
        return None;
    }
    Some(crossing)
}

pub(crate) fn _crossed_values(
    run: &[&Op],
    rest: &[&Op],
    phis: Option<&[&Phi]>,
    wanted: Option<&BTreeSet<Value>>,
) -> BTreeSet<Value> {
    // A phi carries a value out of the run as surely as an instruction reads
    // one, and `rest` holds no phis.
    let mut taken = rest
        .iter()
        .flat_map(|other| other.uses.iter().copied())
        .chain(phis.unwrap_or(&[]).iter().flat_map(|phi| phi.incoming.values().copied()))
        .collect::<BTreeSet<_>>();
    if let Some(wanted) = wanted {
        taken = taken.intersection(wanted).copied().collect();
    }
    run.iter()
        .flat_map(|one| one.defines.iter().copied())
        .filter(|value| taken.contains(value))
        .collect()
}

// ==== END C1 ====

/// Direct port of `qbopt/optimize/transform.py:_OBSERVED`.
pub(crate) const _OBSERVED: [crate::model::mir::Kind; 20] = {
    use crate::model::mir::Kind;
    [
        Kind::Call,
        Kind::Return,
        Kind::Jump,
        Kind::Branch,
        Kind::Switch,
        Kind::Escape,
        Kind::Arg,
        Kind::Result,
        Kind::Opaque,
        Kind::Fload,
        Kind::Fstore,
        Kind::Fadd,
        Kind::Fsub,
        Kind::Fmul,
        Kind::Fdiv,
        Kind::Fneg,
        Kind::Fabs,
        Kind::Fsqrt,
        Kind::Fcompare,
        Kind::Fcheck,
    ]
};

/// Direct port of `qbopt/optimize/transform.py:_leaving`.
pub(crate) fn _leaving(body: &MirBody) -> std::collections::BTreeSet<crate::model::mir::Value> {
    crate::model::mir::exposed(body)
}

/// A value is a 32-bit register and this machine's code is 16-bit.
pub(crate) const LOW: u8 = 0;
pub(crate) const HIGH: u8 = 1;

// ==== BEGIN S1: transform.py 1433-1447 (primary) ====
// ==== END S1 ====

/// Which half of which value something reads, to a fixed point.
///
/// Direct port of `qbopt/optimize/transform.py:halves`, without the
/// `_reusing_halves` memo, which is not ported here.
pub(crate) fn halves(body: &MirBody) -> std::collections::BTreeSet<(crate::model::mir::Value, u8)> {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::model::mir::{Arg, Kind, Op, Value};

    let mut out: BTreeSet<(Value, u8)> = BTreeSet::new();
    for value in _leaving(body) {
        out.insert((value, LOW));
        out.insert((value, HIGH));
    }

    let widths = |op: &Op| -> BTreeMap<Value, u32> {
        let mut found: BTreeMap<Value, u32> = BTreeMap::new();
        for one in &op.args {
            if let Arg::Held(held) = one {
                let widest = (*found.get(&held.value).unwrap_or(&0)).max(held.width);
                found.insert(held.value, widest);
            }
        }
        for reference in op.loads.iter().chain(&op.stores) {
            if let Some(base) = reference.base {
                let widest = (*found.get(&base).unwrap_or(&0)).max(reference.base_width);
                found.insert(base, widest);
            }
        }
        found
    };

    let mut changing = true;
    while changing {
        let before = out.len();
        for block in &body.blocks {
            for op in &block.ops {
                if !_kept(op)
                    && !op
                        .defines
                        .iter()
                        .any(|one| [LOW, HIGH].iter().any(|half| out.contains(&(*one, *half))))
                {
                    continue;
                }
                let carried = &op.merges;
                let read = widths(op);
                let described = op.kind != Kind::Opaque && !op.barrier();
                for one in &op.uses {
                    if let Some(into) = carried.get(one) {
                        if out.contains(&(*into, HIGH)) {
                            out.insert((*one, HIGH));
                        }
                        if !read.contains_key(one) {
                            continue;
                        }
                    }
                    if !described || !read.contains_key(one) {
                        out.insert((*one, LOW));
                        out.insert((*one, HIGH));
                        continue;
                    }
                    out.insert((*one, LOW));
                    if read[one] >= 4 {
                        out.insert((*one, HIGH));
                    }
                }
                for reference in op.loads.iter().chain(&op.stores) {
                    for one in [reference.base, reference.segment].into_iter().flatten() {
                        out.insert((one, LOW));
                        if Some(one) == reference.segment || reference.base_width >= 4 {
                            out.insert((one, HIGH));
                        }
                    }
                }
            }
            for phi in &block.phis {
                for half in [LOW, HIGH] {
                    if out.contains(&(phi.result, half)) {
                        out.extend(phi.incoming.values().map(|one| (*one, half)));
                    }
                }
            }
        }
        changing = out.len() != before;
    }
    out
}

/// Values some half of which something reads.
///
/// Direct port of `qbopt/optimize/transform.py:live`.
pub(crate) fn live(body: &MirBody) -> std::collections::BTreeSet<crate::model::mir::Value> {
    halves(body).into_iter().map(|(one, _)| one).collect()
}

// ==== BEGIN D: transform.py 1551-2090 (agent D) ====
/// Whether each branch test is taken, given (a, b, unsigned view).
#[allow(clippy::type_complexity)]
pub(crate) const _TAKEN: [(
    crate::model::mir::Kind,
    fn(&num_bigint::BigInt, &num_bigint::BigInt, &dyn Fn(&num_bigint::BigInt) -> num_bigint::BigInt) -> bool,
); 10] = {
    use crate::model::mir::Kind;
    [
        (Kind::Eq, |a, b, _| a == b),
        (Kind::Ne, |a, b, _| a != b),
        (Kind::Lt, |a, b, _| a < b),
        (Kind::Le, |a, b, _| a <= b),
        (Kind::Gt, |a, b, _| a > b),
        (Kind::Ge, |a, b, _| a >= b),
        (Kind::Below, |a, b, u| u(a) < u(b)),
        (Kind::BelowEq, |a, b, u| u(a) <= u(b)),
        (Kind::Above, |a, b, u| u(a) > u(b)),
        (Kind::AboveEq, |a, b, u| u(a) >= u(b)),
    ]
};

/// The modeled comparison supplying this branch's condition value.
pub(crate) fn _comparison<'a>(
    block: &'a crate::model::mir::MirBlock,
    op: &crate::model::mir::Op,
) -> Option<(usize, &'a crate::model::mir::Op)> {
    use crate::model::mir::{Arg, Kind};
    if op.kind != Kind::Branch || !op.test.is_some_and(|test| _TAKEN.iter().any(|(kind, _)| *kind == test)) {
        return None;
    }
    let reads = op.uses.iter().filter(|one| one.flags).collect::<Vec<_>>();
    if reads.len() != 1 {
        return None;
    }

    // The comparison this branch reads, which must be the last thing to
    // write the flags before it -- SSA says so by naming the value.
    let (index, compare) = block.ops.iter().enumerate().find(|(_, one)| one.defines.contains(reads[0]))?;
    if compare.kind == Kind::Sub && compare.args.len() == 2 && compare.results.is_empty() {
        return Some((index, compare));
    }
    if matches!(compare.kind, Kind::And | Kind::Or | Kind::Xor)
        && matches!(op.test, Some(Kind::Eq | Kind::Ne))
        && !compare.barrier()
        && compare.args.len() == 2
        && compare.results.len() == 1
    {
        if let Arg::Held(result) = &compare.results[0] {
            if [2, 4, 8].contains(&result.width)
                && compare.args.iter().all(|arg| match arg {
                    Arg::Held(held) => held.width == result.width,
                    Arg::Const(constant) => constant.width == result.width,
                    _ => false,
                })
            {
                return Some((index, compare));
            }
        }
    }
    None
}

/// Dead blocks retain byte ownership, but no instructions or outgoing edges.
pub(crate) fn _unreachable(body: &MirBody) -> MirBody {
    use std::collections::{BTreeMap, BTreeSet};
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let (mut reached, mut pending) = (BTreeSet::new(), vec![body.entry]);
    while let Some(at) = pending.pop() {
        if reached.contains(&at) || !blocks.contains_key(&at) {
            continue;
        }
        reached.insert(at);
        pending.extend(blocks[&at].succ.iter().copied());
    }
    let kept = body
        .blocks
        .iter()
        .filter(|block| reached.contains(&block.at) || !block.ops.is_empty())
        .map(|block| {
            if reached.contains(&block.at) {
                block.clone()
            } else {
                crate::model::mir::MirBlock {
                    succ: Vec::new(),
                    phis: Vec::new(),
                    ops: block.ops.iter().map(_empty_operation).collect(),
                    ..block.clone()
                }
            }
        })
        .collect();
    MirBody { blocks: kept, ..body.clone() }
}

/// Resolve single-valued joins after an edge disappears, without discarding
/// byte ownership.  Python's `ValueError` is the `Err` text.
pub(crate) fn _trivial_phis(body: &MirBody) -> Result<MirBody, String> {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::analysis::{loops as loopy, ssa};
    use crate::model::mir::{MirBlock, Phi};
    let mut body = body.clone();
    let predecessors = loopy::predecessors(&body.blocks);
    let mut swaps = BTreeMap::new();
    loop {
        let mut changed = false;
        let mut out = Vec::new();
        for block in &body.blocks {
            let mut phis = Vec::new();
            for phi in &block.phis {
                let incoming = phi
                    .incoming
                    .iter()
                    .filter(|(at, _)| predecessors[&block.at].contains(at))
                    .map(|(&at, &value)| ssa::provider(value, &swaps).map(|one| (at, one)))
                    .collect::<Result<crate::model::mir::OrderedMap<_, _>, _>>()
                    .map_err(|error| error.to_string())?;
                let mut values = incoming.values().copied().collect::<BTreeSet<_>>();
                values.remove(&phi.result);
                if values.len() == 1 {
                    swaps.insert(phi.result.id, *values.iter().next().expect("one"));
                    changed = true;
                } else {
                    phis.push(Phi { result: phi.result, incoming });
                }
            }
            let ops = block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &swaps))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            out.push(MirBlock { phis, ops, ..block.clone() });
        }
        body = MirBody { blocks: out, ..body };
        if !changed {
            return Ok(body);
        }
    }
}

/// Whether this operation stays whatever the liveness says.
///
/// Direct port of `qbopt/optimize/transform.py:_kept`.
pub(crate) fn _kept(op: &crate::model::mir::Op) -> bool {
    use crate::model::mir::Kind;
    if _OBSERVED.contains(&op.kind) || !op.stores.is_empty() || op.barrier() {
        return true;
    }
    if op.kind == Kind::Opaque {
        return true;
    }
    if op.kind == Kind::Sub && op.results.is_empty() && !op.defines.is_empty() {
        return false;
    }
    op.defines.iter().all(|one| one.flags)
}
// ==== END D ====

// ==== BEGIN E: transform.py 2091-2563 (agent E) ====
// ==== END E ====

// ==== BEGIN C2: transform.py 2564-2869 (agent C) ====

/// The latest place in the preheader every value the run reads is defined.
pub(crate) fn _placement(block: &MirBlock, run: &[&Op], alive: &crate::analysis::liveness::Liveness) -> Option<usize> {
    let made = run.iter().flat_map(|one| one.defines.iter().copied()).collect::<BTreeSet<_>>();
    let wants = run
        .iter()
        .flat_map(|one| _consumed(one))
        .filter(|value| !value.flags && !made.contains(value))
        .collect::<BTreeSet<_>>();
    let mut ready = alive.live_in.get(&block.at).cloned().unwrap_or_default();
    let mut index = 0;
    for (number, one) in block.ops.iter().enumerate() {
        if wants.is_subset(&ready) {
            index = number;
        }
        ready.extend(one.defines.iter().copied());
    }
    if wants.is_subset(&ready) {
        index = block.ops.len();
    }
    if wants.is_subset(&ready) {
        Some(index)
    } else {
        None
    }
}

/// Every value in `crossed` made a variable of its own.
///
/// A rename and nothing else: the value keeps its identity and its readers.
pub(crate) fn _reparented(body: &MirBody, crossed: &crate::support::pyset::PySet<Value>) -> MirBody {
    use crate::model::mir::{Cell, MemRef};

    let taken = body
        .values()
        .into_iter()
        .chain(body.blocks.iter().flat_map(|block| block.ops.iter().flat_map(|op| op.uses.iter().copied())))
        .map(|one| one.variable)
        .max()
        .unwrap_or(0);
    let mut sorted = crossed.iter().copied().collect::<Vec<_>>();
    sorted.sort_by_key(|one| (one.variable, one.version));
    let mut instead: IndexMap<Value, Value> = IndexMap::new();
    for (number, one) in sorted.into_iter().enumerate() {
        instead.insert(
            one,
            Value {
                variable: taken + u32::try_from(number + 1).expect("value count fits u32"),
                version: 1,
                ..one
            },
        );
    }

    let value = |one: Value| -> Value { instead.get(&one).copied().unwrap_or(one) };
    let cell = |one: &MemRef| -> MemRef {
        let mut out = one.clone();
        out.base = one.base.map(value);
        out.segment = one.segment.map(value);
        out
    };
    let arg = |one: &Arg| -> Arg {
        match one {
            Arg::Held(held) if instead.contains_key(&held.value) => Arg::Held(Held {
                value: instead[&held.value],
                width: held.width,
            }),
            Arg::Cell(inner) => Arg::Cell(Cell { r#ref: cell(&inner.r#ref) }),
            _ => one.clone(),
        }
    };
    let op = |one: &Op| -> Op {
        let mut out = one.clone();
        out.defines = one.defines.iter().map(|x| value(*x)).collect();
        out.uses = one.uses.iter().map(|x| value(*x)).collect();
        out.args = one.args.iter().map(arg).collect();
        out.results = one.results.iter().map(arg).collect();
        out.loads = one.loads.iter().map(cell).collect();
        out.stores = one.stores.iter().map(cell).collect();
        out.merges = one.merges.iter().map(|(a, b)| (value(*a), value(*b))).collect();
        out.raised = one
            .raised
            .as_ref()
            .map(|(args, results)| (args.iter().map(arg).collect(), results.iter().map(arg).collect()));
        out
    };

    let mut out = body.clone();
    out.blocks = body
        .blocks
        .iter()
        .map(|block| {
            let mut changed = block.clone();
            changed.phis = block
                .phis
                .iter()
                .map(|phi| Phi {
                    result: value(phi.result),
                    incoming: phi.incoming.iter().map(|(at, x)| (*at, value(*x))).collect(),
                })
                .collect();
            changed.ops = block.ops.iter().map(op).collect();
            changed
        })
        .collect();
    out.pointer_values = body.pointer_values.iter().map(|one| value(*one)).collect();
    out.pointer_seeds = body
        .pointer_seeds
        .iter()
        .map(|(one, provenance)| (value(*one), provenance.clone()))
        .collect();
    out.integer_ranges = body
        .integer_ranges
        .iter()
        .map(|(one, interval)| (value(*one), interval.clone()))
        .collect();
    out
}

/// Floating operations a nonempty loop certainly executes, by `id(op)`.
pub(crate) fn _guaranteed_float_work(body: &MirBody, loop_: &Loop, nonempty: bool) -> BTreeSet<usize> {
    if !nonempty {
        return BTreeSet::new();
    }
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<i64, &MirBlock>>();
    if loop_
        .body
        .iter()
        .any(|at| blocks[at].ops.iter().any(|op| !_unchanged_float_environment(op)))
    {
        return BTreeSet::new();
    }
    let starts = blocks[&loop_.header]
        .succ
        .iter()
        .copied()
        .filter(|at| loop_.body.contains(at) && *at != loop_.header)
        .collect::<Vec<_>>();
    if starts.len() != 1 {
        return BTreeSet::new();
    }

    fn reaches(
        blocks: &BTreeMap<i64, &MirBlock>,
        loop_: &Loop,
        at: i64,
        target: i64,
        visiting: &BTreeSet<i64>,
    ) -> bool {
        if at == target {
            return true;
        }
        if at == loop_.header || !loop_.body.contains(&at) || visiting.contains(&at) {
            return false;
        }
        let successors = &blocks[&at].succ;
        let mut deeper = visiting.clone();
        deeper.insert(at);
        !successors.is_empty() && successors.iter().all(|to| reaches(blocks, loop_, *to, target, &deeper))
    }

    loop_
        .body
        .iter()
        .filter(|at| **at == loop_.header || reaches(&blocks, loop_, starts[0], **at, &BTreeSet::new()))
        .flat_map(|at| blocks[at].ops.iter())
        .filter(|op| op.floating.is_some())
        .map(|op| std::ptr::from_ref(op) as usize)
        .collect()
}

/// A constant that stayed a definition: fold puts every literal its reader
/// can take into the reader, so what is left is work, like a selector.
pub(crate) fn _literal(op: &Op) -> bool {
    op.kind == Kind::Copy && !op.args.is_empty() && op.args.iter().all(|arg| matches!(arg, Arg::Const(_)))
}

/// A loop-invariant run of operations, done once before the loop.
///
/// The whole run moves, so implicit operands are used inside it in the
/// preheader and only its result crosses into the loop.
#[allow(dead_code)] // Called by section F's pass table.
pub(crate) fn hoisted(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    bounds: Option<&IndexMap<(crate::objectfile::module::Space, i64), Vec<i64>>>,
) -> Result<MirBody, String> {
    use std::collections::HashMap;

    use crate::analysis::ranges::{self, Interval};
    use crate::support::pyset::PySet;

    let id = |op: &Op| std::ptr::from_ref(op) as usize;
    let inside = loopy::loops(&body.blocks, Some(body.entry));
    if inside.is_empty() {
        return Ok(body.clone());
    }

    let scoped = ranges::bounded(body)?;
    let constant = ranges::constants(body, None, None).into_iter().collect::<BTreeMap<Value, Interval>>();
    let facts = scoped
        .iter()
        .map(|(at, inside)| {
            let mut merged = constant.clone();
            merged.extend(inside.iter().map(|(value, interval)| (*value, interval.clone())));
            (*at, merged)
        })
        .collect::<BTreeMap<i64, BTreeMap<Value, Interval>>>();
    let intervals = body
        .blocks
        .iter()
        .flat_map(|block| {
            let here = facts.get(&block.at).unwrap_or(&constant);
            block.ops.iter().map(move |op| (id(op), here))
        })
        .collect::<HashMap<usize, &BTreeMap<Value, Interval>>>();
    let at_of = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<i64, &MirBlock>>();
    let alive = crate::analysis::liveness::live(body);
    let readable = live(body);
    let effective = _effective(body, calls);
    let mut crossed: PySet<Value> = PySet::new();
    let demanded = halves(body);
    let mut moved: IndexMap<i64, Vec<Op>> = IndexMap::new();
    let mut gone: BTreeSet<usize> = BTreeSet::new();
    let mut placing: IndexMap<i64, usize> = IndexMap::new();

    for loop_ in &inside {
        let Some(into) = _preheader(body, loop_) else {
            continue;
        };
        if loop_.body.contains(&into) {
            continue;
        }
        let originals = loop_.body.iter().flat_map(|at| at_of[at].ops.iter()).collect::<Vec<&Op>>();
        let replaced = originals
            .iter()
            .map(|one| {
                (one.kind == Kind::Copy
                    && one.args.len() == 1
                    && matches!(one.args[0], Arg::Symbol(_))
                    && one.defines.iter().all(|value| !demanded.contains(&(*value, HIGH))))
                .then(|| {
                    let mut out = (*one).clone();
                    out.uses = one.uses.iter().copied().filter(|value| !one.merges.contains_key(value)).collect();
                    out.merges = OrderedMap::new();
                    out
                })
            })
            .collect::<Vec<Option<Op>>>();
        let ops = originals
            .iter()
            .zip(&replaced)
            .map(|(original, instead)| instead.as_ref().unwrap_or(original))
            .collect::<Vec<&Op>>();
        let identities = ops
            .iter()
            .zip(&originals)
            .map(|(one, original)| (id(one), id(original)))
            .collect::<HashMap<usize, usize>>();
        let stores = ops
            .iter()
            .zip(&originals)
            .flat_map(|(one, original)| {
                let known = intervals.get(&id(original)).copied();
                one.stores.iter().map(move |reference| (reference, known))
            })
            .collect::<Vec<_>>();
        let carried = loop_
            .body
            .iter()
            .flat_map(|at| at_of[at].phis.iter().map(|phi| phi.result))
            .collect::<BTreeSet<Value>>();
        let phis = loop_.body.iter().flat_map(|at| at_of[at].phis.iter()).collect::<Vec<&Phi>>();

        let nonempty = crate::analysis::induction::nonempty(body, loop_);
        let mut run = _invariant_run(
            &ops,
            &carried,
            &stores,
            dgroup,
            calls,
            &phis,
            bounds,
            Some(&_starts(&phis)),
            Some(&readable),
            Some(&intervals),
            nonempty,
            &_guaranteed_float_work(body, loop_, nonempty),
        )?;
        // Track operations, not source addresses: hoisted definitions share
        // their anchor's address with other computations and the jump.
        run.retain(|one| !gone.contains(&identities[&id(one)]));
        while !run.is_empty() {
            let rest = ops.iter().copied().filter(|one| !run.iter().any(|other| *other == *one)).collect::<Vec<_>>();
            let retained = _crossed_values(&run, &rest, Some(&phis), Some(&effective))
                .into_iter()
                .filter(|value| value.flags)
                .collect::<BTreeSet<_>>();
            if retained.is_empty() {
                break;
            }
            run = _pruned(&run, &retained);
        }
        if run.is_empty() {
            continue;
        }

        let rest = ops.iter().copied().filter(|one| !run.iter().any(|other| *other == *one)).collect::<Vec<_>>();
        if _crossing(&run, &rest, Some(&phis), Some(&effective)).is_none() {
            continue;
        }

        // The latest point every value the run reads is defined.
        let Some(index) = _placement(at_of[&into], &run, &alive) else {
            continue;
        };

        // Everything the run defines crosses, not only what leaves the loop:
        // its intermediate values live in the preheader too.
        let defined = run
            .iter()
            .flat_map(|one| one.defines.iter().copied())
            .filter(|value| !value.flags)
            .collect::<PySet<Value>>();
        for value in defined.iter() {
            crossed.add(*value);
        }
        let least = (*placing.get(&into).unwrap_or(&index)).min(index);
        placing.insert(into, least);
        moved.entry(into).or_default().extend(run.iter().map(|one| (*one).clone()));
        gone.extend(run.iter().map(|one| identities[&id(one)]));
    }

    if gone.is_empty() {
        return Ok(body.clone());
    }

    let mut out = Vec::new();
    for block in &body.blocks {
        let mut ops = block.ops.iter().filter(|one| !gone.contains(&id(one))).cloned().collect::<Vec<Op>>();

        if !ops.is_empty() && !block.ops.is_empty() && gone.contains(&id(&block.ops[0])) {
            // Onto the block's own address so branches still land.
            ops[0].at = block.ops[0].at;
        }
        if let Some(lifted) = moved.get(&block.at) {
            let leaves = ops.last().is_some_and(|last| matches!(last.kind, Kind::Jump | Kind::Branch));
            let mut lifted = lifted.clone();
            // The address orders these once emitted, so they take the
            // address of whatever they go in front of.
            let mut index = placing.get(&block.at).copied().unwrap_or(ops.len());
            if leaves && index >= ops.len() {
                index = ops.len().saturating_sub(1);
            }
            if !ops.is_empty() {
                let anchor = if index < ops.len() { ops[index].at } else { ops[ops.len() - 1].at };
                for one in &mut lifted {
                    one.at = anchor;
                }
            }
            // Python's slice clamps an index past the end.
            let index = index.min(ops.len());
            ops.splice(index..index, lifted);
        }
        let mut changed = block.clone();
        changed.ops = ops;
        out.push(changed);
    }

    // What crossed the loop edge is its own variable now, so re-deriving SSA
    // cannot join it to the counter that shared its register.
    let mut moved_out = body.clone();
    moved_out.blocks = out;
    if !crossed.is_empty() {
        moved_out = _reparented(&moved_out, &crossed);
    }
    Ok(moved_out)
}

// ==== END C2 ====

// ==== BEGIN F: transform.py 2870-3297 (primary) ====
// ==== END F ====

#[cfg(test)]
#[path = "transform_tests.rs"]
mod transform_tests;
