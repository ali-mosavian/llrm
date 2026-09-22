//! Port of `qbopt/optimize/transform.py`: MIR transforms, a body in, an
//! optimised body out.

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::occurrence::OpOccurrence;
use crate::analysis::ssa::{provider as _provider, substituted as _substituted};
use crate::model::mir::{self, Arg, Held, Kind, MirBlock, MirBody, Op, OrderedMap, Phi, Value};

// ==== BEGIN S0: transform.py 60-129 (primary) ====
/// Erase the operations whose address is in `gone`.
pub(crate) fn _absorb(ops: &[Op], gone: &BTreeSet<i64>) -> Vec<Op> {
    _without(ops, |one| gone.contains(&one.at))
}

/// Remove selected computation while retaining exact source ownership.
///
/// A deleted source occurrence becomes an inert marker owning the same
/// opaque ids; source-free operations disappear completely.
pub(crate) fn _without(ops: &[Op], drop: impl Fn(&Op) -> bool) -> Vec<Op> {
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

/// One phi taking the provider along any edge that named the load.
pub(crate) fn _phi_reading(phi: &Phi, swap: &BTreeMap<u32, Value>) -> Result<Phi, String> {
    if !phi.incoming.values().any(|one| swap.contains_key(&one.id)) {
        return Ok(phi.clone());
    }
    let incoming = phi
        .incoming
        .iter()
        .map(|(at, one)| _provider(*one, swap).map(|value| (*at, value)))
        .collect::<Result<OrderedMap<_, _>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(Phi { result: phi.result, incoming })
}

// CSE only ever removes an operation whose whole answer is in its operands.
pub(crate) const _PURE: [Kind; 30] = [
    Kind::Add,
    Kind::Sub,
    Kind::Mul,
    Kind::FixedMul,
    Kind::Smulhi,
    Kind::PtrOffset,
    Kind::Div,
    Kind::Rem,
    Kind::And,
    Kind::Or,
    Kind::Xor,
    Kind::Shl,
    Kind::Shr,
    Kind::Sar,
    Kind::Neg,
    Kind::Not,
    Kind::Convert,
    Kind::SignExtend,
    Kind::Extract,
    Kind::Concat,
    Kind::Copy,
    Kind::Address,
    Kind::Lt,
    Kind::Le,
    Kind::Gt,
    Kind::Ge,
    Kind::Eq,
    Kind::Ne,
    Kind::Below,
    Kind::Above,
];

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
type _HalvesReuse = std::collections::HashMap<usize, (MirBody, BTreeSet<(Value, u8)>)>;

thread_local! {
    /// Python's `_halves_reuse` context variable.  The saved body is compared
    /// as well as its address: Python holds the body alive, Rust cannot.
    #[allow(non_upper_case_globals)]
    static _halves_reuse: std::cell::RefCell<Option<_HalvesReuse>> = const { std::cell::RefCell::new(None) };
}

/// Share half-liveness for immutable states in one transaction.
pub(crate) fn _reusing_halves<T>(inside: impl FnOnce() -> T) -> T {
    let token = _halves_reuse.with(|reuse| reuse.replace(Some(std::collections::HashMap::new())));
    let result = inside();
    _halves_reuse.with(|reuse| *reuse.borrow_mut() = token);
    result
}
// ==== END S1 ====

/// Which half of which value something reads, to a fixed point.
///
/// Both halves of everything reaching an exit are live.
pub(crate) fn halves(body: &MirBody) -> BTreeSet<(Value, u8)> {
    let key = std::ptr::from_ref(body) as usize;
    let saved = _halves_reuse.with(|reuse| {
        reuse
            .borrow()
            .as_ref()
            .and_then(|reused| reused.get(&key))
            .filter(|saved| saved.0 == *body)
            .map(|saved| saved.1.clone())
    });
    if let Some(saved) = saved {
        return saved;
    }

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
    _halves_reuse.with(|reuse| {
        if let Some(reused) = reuse.borrow_mut().as_mut() {
            reused.insert(key, (body.clone(), out.clone()));
        }
    });
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

/// A Known as the number the machine would compare.
pub(crate) fn _signed(fact: &crate::analysis::consts::Known) -> num_bigint::BigInt {
    use num_bigint::BigInt;
    let top = BigInt::from(1) << (fact.width * 8 - 1);
    if (&fact.n & &top) != BigInt::from(0) {
        &fact.n - (top << 1)
    } else {
        fact.n.clone()
    }
}

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

/// Whether this branch is taken, where both its operands are numbers.
pub(crate) fn _outcome(
    block: &MirBlock,
    op: &Op,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    held: &IndexMap<(i64, usize), crate::analysis::consts::Cells>,
    pointers: Option<&crate::analysis::alias::PointsTo>,
) -> Option<bool> {
    use crate::analysis::consts;
    let (index, compare) = _comparison(block, op)?;
    if compare.kind != Kind::Sub {
        let result = consts::_result(compare, facts, None, None)?;
        let Arg::Held(first) = &compare.results[0] else {
            unreachable!("_comparison admits only a held result")
        };
        if result.width < first.width {
            return None;
        }
        let zero = consts::masked(&result.n, first.width) == num_bigint::BigInt::from(0);
        return Some(if op.test == Some(Kind::Eq) { zero } else { !zero });
    }
    let parts = compare
        .args
        .iter()
        .map(|one| consts::_operand(compare, one, facts, held.get(&(block.at, index))))
        .collect::<Vec<_>>();
    if parts.iter().any(Option::is_none) {
        if !matches!(op.test, Some(Kind::Eq | Kind::Ne)) {
            return None;
        }
        let pointers = pointers?;
        let pointer = compare
            .args
            .iter()
            .zip(compare.args.iter().rev())
            .find_map(|(arg, other)| match (arg, other) {
                (Arg::Held(arg), Arg::Const(other))
                    if consts::masked(&other.n, other.width) == num_bigint::BigInt::from(0) =>
                {
                    Some(arg.value)
                }
                _ => None,
            })?;
        if !pointers.nonnull(pointer) {
            return None;
        }
        return Some(op.test == Some(Kind::Ne));
    }
    let (left, right) = (parts[0].as_ref().expect("known"), parts[1].as_ref().expect("known"));
    let width = left.width.max(right.width);
    let taken = _TAKEN.iter().find(|(kind, _)| Some(*kind) == op.test).expect("a test _comparison admits").1;
    Some(taken(&_signed(left), &_signed(right), &|n| consts::masked(n, width)))
}

pub(crate) fn _switch_target(op: &Op, facts: &IndexMap<Value, crate::analysis::consts::Known>) -> Option<i64> {
    use crate::analysis::consts;
    if op.kind != Kind::Switch || op.args.len() != 1 || op.target.is_none() {
        return None;
    }
    let width = match &op.args[0] {
        Arg::Held(held) => held.width,
        Arg::Const(constant) => constant.width,
        _ => return None,
    };
    if ![1, 2, 4].contains(&width)
        || !op.defines.is_empty()
        || !op.results.is_empty()
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || !op.merges.is_empty()
        || op.barrier()
        || op.stack.is_some()
        || op.floating.is_some()
    {
        return None;
    }
    let cases = op.cases.iter().map(|(number, _)| consts::masked(&(*number).into(), width)).collect::<Vec<_>>();
    if cases.iter().collect::<BTreeSet<_>>().len() != cases.len() {
        return None;
    }
    let value = consts::_operand(op, &op.args[0], facts, None)?;
    op.cases
        .iter()
        .find(|(number, _)| consts::masked(&(*number).into(), value.width) == value.n)
        .map(|(_, target)| *target)
        .or(op.target)
}

pub(crate) fn _executable_successors(
    block: &MirBlock,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    states: &IndexMap<Value, crate::analysis::constant_cycles::State>,
    held: &IndexMap<(i64, usize), crate::analysis::consts::Cells>,
    pointers: Option<&crate::analysis::alias::PointsTo>,
) -> Option<Vec<i64>> {
    use crate::analysis::constant_cycles::State;
    let pending = |args: &[Arg]| {
        args.iter()
            .any(|arg| matches!(arg, Arg::Held(held) if states.get(&held.value) == Some(&State::Pending)))
    };

    let Some(last) = block.ops.last() else {
        return Some(block.succ.clone());
    };
    if last.kind == Kind::Switch {
        if let Some(target) = _switch_target(last, facts).filter(|target| block.succ.contains(target)) {
            return Some(vec![target]);
        }
        if pending(&last.args) {
            return None;
        }
        return Some(block.succ.clone());
    }
    if block.succ.len() != 2 {
        return Some(block.succ.clone());
    }
    if !last.target.is_some_and(|target| block.succ.contains(&target)) {
        return Some(block.succ.clone());
    }
    if let Some(answer) = _outcome(block, last, facts, held, pointers) {
        return Some(if answer {
            vec![last.target.expect("a successor")]
        } else {
            block.succ.iter().copied().filter(|at| Some(*at) != last.target).collect()
        });
    }
    if let Some((_, compare)) = _comparison(block, last) {
        if pending(&compare.args) {
            return None;
        }
    }
    Some(block.succ.clone())
}

/// Bypass empty control-flow blocks without changing any incoming phi value.
pub(crate) fn _threaded(body: &MirBody) -> Result<MirBody, String> {
    let known = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let predecessors = loopy::predecessors(&body.blocks);
    let mut loop_edges = BTreeSet::new();
    for loop_ in loopy::loops(&body.blocks, Some(body.entry)) {
        let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
        if outside.len() == 1 {
            let parent = outside[0];
            if known[&parent].succ == [loop_.header] {
                loop_edges.insert(parent);
            }
        }
        if loop_.latches.len() == 1 {
            let parent = *loop_.latches.iter().next().expect("one latch");
            if known[&parent].succ == [loop_.header] {
                loop_edges.insert(parent);
            }
        }
        loop_edges.extend(
            body.blocks
                .iter()
                .filter(|block| {
                    !loop_.body.contains(&block.at)
                        && !predecessors[&block.at].is_empty()
                        && predecessors[&block.at].is_subset(&loop_.body)
                        && block.succ.len() == 1
                        && !loop_.body.contains(&block.succ[0])
                })
                .map(|block| block.at),
        );
    }
    let mut redirects = BTreeMap::new();
    let mut explicit_jumps = BTreeSet::new();
    for block in &body.blocks {
        // Loop-simplify form deliberately keeps a unique entry edge and a
        // unique backedge and dedicated exits as blocks of their own.
        if !block.phis.is_empty() || block.succ.len() != 1 || loop_edges.contains(&block.at) {
            continue;
        }
        let mut ops = &block.ops[..];
        if let Some(last) = ops.last() {
            if last.kind == Kind::Jump && last.target == Some(block.succ[0]) {
                explicit_jumps.insert(block.at);
                ops = &ops[..ops.len() - 1];
            }
        }
        if ops.iter().any(|op| {
            op.kind != Kind::Nothing
                || !op.defines.is_empty()
                || !op.uses.is_empty()
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || !op.args.is_empty()
                || !op.results.is_empty()
                || !op.merges.is_empty()
                || op.barrier()
                || op.floating.is_some()
                || op.stack.is_some()
        }) {
            continue;
        }
        if let Some(successor) = known.get(&block.succ[0]) {
            if successor.phis.is_empty() {
                redirects.insert(block.at, successor.at);
            }
        }
    }

    let destination = |start: i64, source: i64, implicit: bool| -> i64 {
        let (mut target, mut seen) = (start, BTreeSet::from([source]));
        while redirects.contains_key(&target) && !seen.contains(&target) {
            if implicit && explicit_jumps.contains(&target) {
                break;
            }
            seen.insert(target);
            target = redirects[&target];
        }
        if seen.contains(&target) { start } else { target }
    };
    let jump = |last: &Op, target: i64| Op {
        op: Some(mir::OpCode::jump()),
        kind: Kind::Jump,
        name: "jmp".to_owned(),
        uses: Vec::new(),
        args: Vec::new(),
        results: Vec::new(),
        test: None,
        target: Some(target),
        ..last.clone()
    };

    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let last = block.ops.last();
        if last.is_some_and(|last| last.kind == Kind::Switch) {
            blocks.push(block.clone());
            continue;
        }
        let explicit =
            last.filter(|last| matches!(last.kind, Kind::Jump | Kind::Branch)).and_then(|last| last.target);
        let mut successors = Vec::new();
        for at in &block.succ {
            let one = destination(*at, block.at, Some(*at) != explicit);
            if !successors.contains(&one) {
                successors.push(one);
            }
        }
        if successors == block.succ {
            blocks.push(block.clone());
            continue;
        }
        changed = true;
        let mut ops = block.ops.clone();
        if let Some(last) = ops.last().filter(|last| matches!(last.kind, Kind::Jump | Kind::Branch)) {
            let mut last =
                Op { target: last.target.map(|target| destination(target, block.at, false)), ..last.clone() };
            if last.kind == Kind::Branch && successors.len() == 1 {
                last = jump(&last, successors[0]);
            }
            *ops.last_mut().expect("a last operation") = last;
        }
        blocks.push(MirBlock { ops, succ: successors, ..block.clone() });
    }

    // If both arms reach the same block through otherwise empty jump
    // trampolines, the condition has no semantic successor to choose.  Keep a
    // real jump at the source: the implicit arm may need one when source
    // bodies are interleaved.
    let mut converged = Vec::new();
    for block in blocks {
        let Some(last) = block.ops.last().filter(|last| last.kind == Kind::Branch && block.succ.len() == 2) else {
            converged.push(block);
            continue;
        };
        let destinations = block.succ.iter().map(|at| destination(*at, block.at, false)).collect::<Vec<_>>();
        if destinations.iter().collect::<BTreeSet<_>>().len() != 1 {
            converged.push(block);
            continue;
        }
        let target = destinations[0];
        let jumped = jump(last, target);
        let mut ops = block.ops.clone();
        *ops.last_mut().expect("a last operation") = jumped;
        converged.push(MirBlock { ops, succ: vec![target], ..block });
        changed = true;
    }
    Ok(if changed { _unreachable(&MirBody { blocks: converged, ..body.clone() }) } else { body.clone() })
}

/// A branch on two numbers, resolved.
///
/// Taken becomes an unconditional jump and not-taken becomes an inert owner.
pub(crate) fn decided(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
) -> Result<MirBody, String> {
    use crate::analysis::{alias, constant_cycles, consts, ranges};

    let body = _threaded(body)?;
    let facts = consts::known(&body, Some(dgroup), Some(calls), None, None);
    let held = consts::cells(&body, dgroup, calls, Some(&facts), None, None, None, None);
    let pointers = alias::points_to(&body, None, None)?;
    let successors = |block: &MirBlock,
                      values: &IndexMap<Value, consts::Known>,
                      states: &IndexMap<Value, constant_cycles::State>| {
        _executable_successors(block, values, states, &held, Some(&pointers))
    };
    let facts = constant_cycles::propagated(&body, &facts, Some(&successors));
    let scoped = ranges::bounded(&body)?;

    let mut out = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let Some(last) = block.ops.last() else {
            out.push(block.clone());
            continue;
        };
        if last.kind == Kind::Switch {
            let Some(target) = _switch_target(last, &facts).filter(|target| block.succ.contains(target)) else {
                out.push(block.clone());
                continue;
            };
            let jump = Op {
                kind: Kind::Jump,
                target: Some(target),
                cases: Vec::new(),
                args: Vec::new(),
                uses: Vec::new(),
                defines: Vec::new(),
                results: Vec::new(),
                name: String::new(),
                raised: None,
                ..last.clone()
            };
            let mut ops = block.ops.clone();
            *ops.last_mut().expect("a last operation") = jump;
            out.push(MirBlock { ops, succ: vec![target], ..block.clone() });
            changed = true;
            continue;
        }
        let mut answer = _outcome(block, last, &facts, &held, Some(&pointers));
        if answer.is_none() && last.kind == Kind::Branch && block.succ.len() == 2 {
            if let Some(scope) = scoped.get(&block.at) {
                let mut possible = Vec::new();
                for at in &block.succ {
                    if ranges::on_edge(block, *at, scope, Some(&facts))?.is_some() {
                        possible.push(*at);
                    }
                }
                if possible.len() == 1 {
                    answer = Some(Some(possible[0]) == last.target);
                }
            }
        }
        let Some(answer) = answer else {
            out.push(block.clone());
            continue;
        };
        let Some(target) = last.target.filter(|target| body.blocks.iter().any(|one| one.at == *target)) else {
            out.push(block.clone());
            continue;
        };
        changed = true;
        if answer {
            let jump = Op {
                kind: Kind::Jump,
                uses: Vec::new(),
                args: Vec::new(),
                results: Vec::new(),
                target: Some(target),
                ..last.clone()
            };
            let mut ops = block.ops.clone();
            *ops.last_mut().expect("a last operation") = jump;
            out.push(MirBlock { ops, succ: vec![target], ..block.clone() });
        } else {
            let kept = _absorb(&block.ops, &BTreeSet::from([last.at]));
            if kept == block.ops {
                out.push(block.clone());
                continue;
            }
            out.push(MirBlock {
                ops: kept,
                succ: block.succ.iter().copied().filter(|at| *at != target).collect(),
                ..block.clone()
            });
        }
    }
    if !changed {
        return Ok(body);
    }
    // Remove dead edges without losing the unreachable blocks' byte ownership.
    _trivial_phis(&_unreachable(&MirBody { blocks: out, ..body }))
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

/// Operations whose results nothing reads, removed.
///
/// A removed computation leaves an empty ownership marker.
pub(crate) fn dead(body: &MirBody) -> Result<MirBody, String> {
    // Incomplete readers forbid global removal, but a result overwritten
    // locally before reaching one cannot supply its hidden inputs.
    let limited = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .any(|one| (one.barrier() || one.kind == Kind::Opaque) && !one.reads_complete);
    let pruned;
    let body = if limited {
        body
    } else {
        pruned = crate::analysis::ssa::pruned_phis(body, &live(body));
        &pruned
    };
    let mut alive = live(body);
    if limited {
        for block in &body.blocks {
            let overwritten = _overwritten_locally(block);
            alive.extend(
                block.ops.iter().flat_map(|op| &op.defines).filter(|value| !overwritten.contains(value)).copied(),
            );
            alive.extend(block.phis.iter().flat_map(|phi| phi.incoming.values()).copied());
        }
        loop {
            let before = alive.len();
            let reached = body
                .blocks
                .iter()
                .flat_map(|block| &block.ops)
                .filter(|op| op.defines.iter().any(|value| alive.contains(value)))
                .flat_map(|op| op.uses.iter().copied())
                .collect::<Vec<_>>();
            alive.extend(reached);
            if alive.len() == before {
                break;
            }
        }
    }
    let mut out = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        // Several semantic operations may share an input address. Their
        // computations are independent even when their provenance is not.
        let overwritten = limited.then(|| _overwritten_locally(block));
        let gone = block
            .ops
            .iter()
            .enumerate()
            .filter(|(_, op)| {
                _removable(op, &alive)
                    && overwritten
                        .as_ref()
                        .is_none_or(|overwritten| op.defines.iter().all(|one| overwritten.contains(one)))
            })
            .map(|(index, _)| index)
            .collect::<BTreeSet<_>>();
        if gone.is_empty() {
            out.push(block.clone());
            continue;
        }
        let ops = block
            .ops
            .iter()
            .enumerate()
            .map(|(index, op)| if gone.contains(&index) { _empty_operation(op) } else { op.clone() })
            .collect();
        changed = true;
        out.push(MirBlock { ops, ..block.clone() });
    }
    if !changed {
        return Ok(body.clone());
    }
    let after = out.iter().flat_map(|block| &block.ops).flat_map(|op| &op.defines).collect::<BTreeSet<_>>();
    let removed = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| &op.defines)
        .filter(|value| !after.contains(value))
        .copied()
        .collect::<BTreeSet<_>>();
    Ok(MirBody {
        blocks: out
            .into_iter()
            .map(|block| MirBlock {
                ops: block
                    .ops
                    .iter()
                    .map(|op| Op {
                        uses: op
                            .uses
                            .iter()
                            .filter(|value| !removed.contains(value) || !op.merges.contains_key(value))
                            .copied()
                            .collect(),
                        merges: op
                            .merges
                            .iter()
                            .filter(|(before, _)| !removed.contains(before))
                            .map(|(before, after)| (*before, *after))
                            .collect(),
                        ..op.clone()
                    })
                    .collect(),
                ..block
            })
            .collect(),
        ..body.clone()
    })
}

/// Results replaced before reaching an opaque reader or a block exit.
pub(crate) fn _overwritten_locally(block: &MirBlock) -> BTreeSet<Value> {
    let mut written = BTreeSet::new();
    let mut overwritten = BTreeSet::new();
    for op in block.ops.iter().rev() {
        if op.barrier() || op.kind == Kind::Opaque {
            written.clear();
            continue;
        }
        overwritten.extend(
            op.defines
                .iter()
                .filter(|value| value.version != 0 && written.contains(&(value.variable, value.flags)))
                .copied(),
        );
        written.extend(op.defines.iter().filter(|value| value.version != 0).map(|value| (value.variable, value.flags)));
    }
    overwritten
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

/// Whether anything at all would notice this operation going.
pub(crate) fn _removable(op: &Op, alive: &BTreeSet<Value>) -> bool {
    if _kept(op) {
        return false;
    }
    !op.defines.iter().any(|one| alive.contains(one))
}
// ==== END D ====

// ==== BEGIN E: transform.py 2091-2563 (agent E) ====
// ==== END E ====

// ==== BEGIN C2: transform.py 2564-2869 (agent C) ====
// ==== END C2 ====

// ==== BEGIN F: transform.py 2870-3297 (primary) ====
// ==== END F ====

#[cfg(test)]
#[path = "transform_tests.rs"]
mod transform_tests;
