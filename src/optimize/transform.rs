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
/// Values that are a symbol's address, with the op owning its fixup.
type _SymbolCopies<'a> = IndexMap<Value, (crate::model::mir::Symbol, &'a Op)>;

pub(crate) fn _folded_division(op: &Op, numbers: (num_bigint::BigInt, num_bigint::BigInt), wanted: &BTreeSet<Value>) -> Vec<Op> {
    use crate::model::ir::Operation;
    use crate::model::mir::{Const, OpCode};

    let results = op
        .results
        .iter()
        .filter_map(|result| match result {
            Arg::Held(held) => Some(held.value),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if op.defines.iter().any(|value| wanted.contains(value) && !results.contains(value)) {
        return vec![op.clone()];
    }
    let numbers = [numbers.0, numbers.1];
    assert_eq!(op.results.len(), numbers.len(), "zip() argument 2 is shorter than argument 1");
    op.results
        .iter()
        .zip(numbers)
        .enumerate()
        .map(|(index, (result, number))| {
            let Arg::Held(held) = result else {
                unreachable!("consts.division answers only for held results")
            };
            let mut one = op.clone();
            one.op = Some(OpCode::Operation(Operation::Move));
            one.name = "mov".to_string();
            one.kind = Kind::Copy;
            one.args = vec![Arg::Const(Const::new(number, 4))];
            one.results = vec![result.clone()];
            one.defines = vec![held.value];
            one.uses = Vec::new();
            one.loads = Vec::new();
            one.merges = OrderedMap::new();
            one.raised = None;
            one.symbol = Some(false);
            one.source_backed = false;
            one.id = if index == 0 { op.id } else { None };
            one.absorbed = if index == 0 { op.absorbed.clone() } else { Vec::new() };
            one
        })
        .collect()
}

/// `_PURE` less copies, provenance carriers and trapping division.
pub(crate) const _EDGE_FOLDABLE: [Kind; 25] = {
    // A copy removes no computation.  Addresses and pointer offsets carry
    // provenance, while division and remainder may trap.  None is a pure
    // integer expression that this first, deliberately strict form may
    // speculate separately on incoming edges.
    const EXCLUDED: [Kind; 5] = [Kind::Copy, Kind::Address, Kind::PtrOffset, Kind::Div, Kind::Rem];
    let mut out = [Kind::Add; 25];
    let (mut index, mut count) = (0, 0);
    while index < _PURE.len() {
        let mut excluded = false;
        let mut other = 0;
        while other < EXCLUDED.len() {
            excluded |= _PURE[index] as u16 == EXCLUDED[other] as u16;
            other += 1;
        }
        if !excluded {
            out[count] = _PURE[index];
            count += 1;
        }
        index += 1;
    }
    assert!(count == out.len());
    out
};

/// Fold one pure join expression independently on every incoming edge.
pub(crate) fn _folded_phi_edges(
    body: &MirBody,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    wanted: &BTreeSet<Value>,
) -> Result<MirBody, String> {
    use crate::analysis::consts;
    use crate::model::ir::Operation;
    use crate::model::mir::{Const, OpCode};

    let predecessors = loopy::predecessors(&body.blocks);
    let by_at = body.blocks.iter().map(|block| (block.at, block)).collect::<IndexMap<_, _>>();
    let pointer_values = &body.pointer_values;
    let values = crate::analysis::ssa::values(body).collect::<Vec<_>>();
    let serial = values.iter().map(|value| value.id).max().unwrap_or(0) + 1;
    let variable = values.iter().map(|value| value.variable).max().unwrap_or(0) + 1;
    let none = BTreeSet::new();

    for block in &body.blocks {
        let parents = predecessors.get(&block.at).unwrap_or(&none);
        if parents.len() < 2 || block.phis.is_empty() {
            continue;
        }
        let parent_blocks = parents
            .iter()
            .map(|at| (*at, by_at.get(at).copied()))
            .collect::<IndexMap<_, _>>();
        if parent_blocks
            .values()
            .any(|parent| parent.is_none_or(|parent| parent.succ != [block.at]))
        {
            continue;
        }
        let parent_blocks = parent_blocks
            .into_iter()
            .map(|(at, parent)| (at, parent.expect("checked")))
            .collect::<IndexMap<_, _>>();
        // A terminal conditional with one surviving CFG successor still has
        // path semantics which are not represented by that tuple alone.
        if parent_blocks.values().any(|parent| {
            parent
                .ops
                .last()
                .is_some_and(|last| matches!(last.kind, Kind::Branch | Kind::Switch | Kind::Return))
        }) {
            continue;
        }
        let phis = block
            .phis
            .iter()
            .filter(|phi| phi.incoming.keys().copied().collect::<BTreeSet<_>>() == *parents)
            .map(|phi| (phi.result, phi))
            .collect::<IndexMap<_, _>>();
        if phis.is_empty() {
            continue;
        }

        let mut corridor = vec![block];
        let mut seen = BTreeSet::from([block.at]);
        while corridor.last().expect("nonempty").succ.len() == 1 {
            let last = *corridor.last().expect("nonempty");
            let Some(successor) = by_at.get(&last.succ[0]).copied() else {
                break;
            };
            if seen.contains(&successor.at)
                || predecessors.get(&successor.at).unwrap_or(&none) != &BTreeSet::from([last.at])
            {
                break;
            }
            corridor.push(successor);
            seen.insert(successor.at);
        }

        for operation_block in &corridor {
            for (index, op) in operation_block.ops.iter().enumerate() {
                if !_EDGE_FOLDABLE.contains(&op.kind)
                    || op.kind == Kind::Nothing
                    || op.barrier()
                    || !op.loads.is_empty()
                    || !op.stores.is_empty()
                    || op.floating.is_some()
                    || op.stack.is_some()
                    || !op.merges.is_empty()
                    || op.opaque_defs != Some(BTreeSet::new())
                    || op.opaque_uses != Some(BTreeSet::new())
                    || mir::partial(op)
                {
                    continue;
                }
                let target = consts::_defined(op);
                let results = op
                    .results
                    .iter()
                    .filter_map(|result| match result {
                        Arg::Held(held) => Some(held),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let Some(target) = target else {
                    continue;
                };
                if results.len() != 1
                    || results[0].value != target
                    || pointer_values.contains(&target)
                    || op.defines.iter().any(|value| *value != target && wanted.contains(value))
                {
                    continue;
                }
                let used = op
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        Arg::Held(held) => Some(held.value),
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>();
                if !used.iter().any(|value| phis.contains_key(value)) {
                    continue;
                }

                let width = results[0].width;
                let mut numbers: IndexMap<i64, num_bigint::BigInt> = IndexMap::new();
                for &parent in parents {
                    let swap = phis
                        .iter()
                        .map(|(result, phi)| (result.id, *phi.incoming.get(&parent).expect("complete phi")))
                        .collect::<BTreeMap<_, _>>();
                    let substituted = _substituted(op, &swap).map_err(|error| error.to_string())?;
                    let fact = consts::_result(&substituted, facts, None, None);
                    let Some(fact) = fact.filter(|fact| fact.width >= width) else {
                        break;
                    };
                    numbers.insert(parent, consts::masked(&fact.n, width));
                }
                if numbers.len() != parents.len() {
                    continue;
                }

                let mut changed = by_at
                    .iter()
                    .map(|(at, one)| (*at, (*one).clone()))
                    .collect::<IndexMap<_, _>>();
                let mut incoming = OrderedMap::new();
                for (offset, &parent_at) in parents.iter().enumerate() {
                    let parent = parent_blocks[&parent_at];
                    let offset = u32::try_from(offset).expect("few parents");
                    let edge_value = Value {
                        id: serial + offset,
                        at: parent.at,
                        flags: false,
                        variable: variable + offset,
                        version: 1,
                    };
                    incoming.insert(parent_at, edge_value);
                    let mut copy = Op::new(
                        parent.at,
                        OpCode::Operation(Operation::Move),
                        "mov",
                        vec![edge_value],
                        Vec::new(),
                    );
                    copy.kind = Kind::Copy;
                    copy.args = vec![Arg::Const(Const::new(numbers[&parent_at].clone(), width))];
                    copy.results = vec![Arg::Held(Held { value: edge_value, width })];
                    copy.source_backed = false;
                    let mut ops = parent.ops.clone();
                    let position = if ops.last().is_some_and(|last| last.kind == Kind::Jump) {
                        ops.len() - 1
                    } else {
                        ops.len()
                    };
                    ops.insert(position, copy);
                    changed[&parent_at] = MirBlock { ops, ..parent.clone() };
                }

                changed[&block.at].phis.push(Phi { result: target, incoming });
                changed[&operation_block.at].ops[index] = _empty_operation(op);
                return Ok(MirBody {
                    blocks: body.blocks.iter().map(|one| changed[&one.at].clone()).collect(),
                    ..body.clone()
                });
            }
        }
    }
    Ok(body.clone())
}

/// An operation whose result is a number, replaced by that number.
pub(crate) fn folded(body: &MirBody, dgroup: &BTreeSet<i64>, calls: &IndexMap<i64, String>) -> Result<MirBody, String> {
    use crate::analysis::{consts, floatfacts};
    use crate::optimize::floatfold;

    let edges = floatfacts::exit_cells(body, dgroup, calls);
    let facts = consts::known(body, Some(dgroup), Some(calls), Some(&edges), None);
    let floating_facts = if body.blocks.iter().any(|block| block.ops.iter().any(|op| op.floating.is_some())) {
        floatfacts::known(body, dgroup, calls, None)
    } else {
        IndexMap::new()
    };
    let conversions = floatfacts::converted(body, dgroup, calls, Some(&floating_facts));
    let mut argument_facts = facts.clone();
    argument_facts.extend(conversions.iter().map(|(value, fact)| (*value, fact.clone())));
    let memory = if body
        .blocks
        .iter()
        .any(|block| block.ops.iter().any(|op| !op.loads.is_empty() || op.kind == Kind::Divmod))
    {
        consts::cells(body, dgroup, calls, Some(&facts), None, Some(&edges), None, None)
    } else {
        IndexMap::new()
    };
    let symbols = _symbol_copies(body);
    if facts.is_empty() && memory.is_empty() && argument_facts.is_empty() && symbols.is_empty() {
        return Ok(body.clone());
    }

    // Live, not merely mentioned: see live()'s own note on hotlop's dx.
    let wanted = live(body);

    let nothing = consts::Cells::new();
    let mut out = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            let here = memory.get(&(block.at, index)).unwrap_or(&nothing);
            if let Some(numbers) = consts::division(op, &facts, here) {
                ops.extend(_folded_division(op, numbers, &wanted));
                continue;
            }
            let updated = _constant_update(op, &facts, here, &wanted);
            let made = _constant_operands(
                &_folded_op(&updated, &facts, &wanted),
                if op.kind == Kind::Arg { &argument_facts } else { &facts },
                Some(here),
                Some(&symbols),
            );
            ops.push(made);
        }
        out.push(MirBlock { ops, ..block.clone() });
    }

    let result = MirBody { blocks: out, ..body.clone() };
    let result = _folded_phi_edges(&result, &facts, &wanted)?;
    // An exact exit fact describes only the path leaving a numeric loop.  It
    // may fold a successor load, but it is not permission for ordinary
    // constant folding to replace the loop's strict x87 operations and their
    // observation points with stores.  That belongs to the dedicated FP loop
    // specialization, which retains its final checked iteration.
    if !loopy::loops(&result.blocks, Some(result.entry)).is_empty() {
        return Ok(result);
    }
    Ok(floatfold::stored(&floatfold::discarded(&result, &conversions), &floating_facts))
}

pub(crate) fn _constant_update(
    op: &Op,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    memory: &crate::analysis::consts::Cells,
    wanted: &BTreeSet<Value>,
) -> Op {
    use crate::model::ir::Operation;
    use crate::model::mir::{Const, OpCode};

    if op.defines.iter().any(|value| wanted.contains(value)) {
        return op.clone();
    }
    let Some(fact) = crate::analysis::consts::updated(op, facts, memory) else {
        return op.clone();
    };
    let address_values = op
        .stores
        .iter()
        .flat_map(|reference| [reference.base, reference.segment])
        .flatten()
        .collect::<BTreeSet<_>>();
    let mut result = op.clone();
    result.op = Some(OpCode::Operation(Operation::Move));
    result.kind = Kind::Store;
    result.name = "mov".to_string();
    result.defines = Vec::new();
    result.uses = op.uses.iter().copied().filter(|value| address_values.contains(value)).collect();
    result.loads = Vec::new();
    result.args = vec![Arg::Const(Const::new(fact.n, fact.width))];
    result.source_backed = false;
    result.raised = None;
    result.symbol = Some(false);
    result
}

/// Values that are a symbol's address, with the op that owns its fixup.
pub(crate) fn _symbol_copies(body: &MirBody) -> _SymbolCopies<'_> {
    body.blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| {
            op.kind == Kind::Copy
                && op.args.len() == op.defines.len()
                && op.defines.len() == 1
                && matches!(op.args[0], Arg::Symbol(_))
                && op.loads.is_empty()
                && op.merges.is_empty()
        })
        .map(|op| {
            let Arg::Symbol(symbol) = &op.args[0] else {
                unreachable!("filtered")
            };
            (op.defines[0], (*symbol, op))
        })
        .collect()
}

/// A register operand as the literal it holds, a number or a symbol's address.
pub(crate) fn _literal_of(
    arg: &Arg,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    symbols: &_SymbolCopies<'_>,
) -> Option<Arg> {
    use crate::model::mir::Const;

    let Arg::Held(arg) = arg else {
        return None;
    };
    if let Some(fact) = facts.get(&arg.value).filter(|fact| fact.width >= arg.width) {
        return Some(Arg::Const(Const::new(
            crate::analysis::consts::masked(&fact.n, arg.width),
            arg.width,
        )));
    }
    let symbol = symbols.get(&arg.value).map(|known| known.0)?;
    (symbol.width == arg.width).then_some(Arg::Symbol(symbol))
}

/// Propagate width-proven constants without reversing ordered operands.
pub(crate) fn _constant_operands(
    op: &Op,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    memory: Option<&crate::analysis::consts::Cells>,
    symbols: Option<&_SymbolCopies<'_>>,
) -> Op {
    use crate::analysis::consts;
    use crate::model::mir::Const;

    let no_memory = consts::Cells::new();
    let no_symbols = _SymbolCopies::new();
    let memory = memory.unwrap_or(&no_memory);
    let symbols = symbols.unwrap_or(&no_symbols);
    if op.kind == Kind::Arg {
        return _constant_argument(op, facts, memory, Some(symbols));
    }
    if op.kind == Kind::Store
        && op.args.len() == op.stores.len()
        && op.stores.len() == 1
        && op.defines.is_empty()
        && op.merges.is_empty()
        && op.loads.is_empty()
        && !op.barrier()
        && op.floating.is_none()
    {
        if let Arg::Held(arg) = &op.args[0] {
            if arg.width == op.stores[0].width {
                if let Some(literal) = _literal_of(&op.args[0], facts, symbols) {
                    let address_values = op
                        .stores
                        .iter()
                        .flat_map(|reference| [reference.base, reference.segment])
                        .flatten()
                        .collect::<BTreeSet<_>>();
                    // The node and its field stay: the destination is still this op's,
                    // and segld's `mov [x],ax` folded to `mov [x],6` counted its fixup
                    // as gone while emitting a new one.
                    let mut result = op.clone();
                    result.args = vec![literal];
                    result.uses = op
                        .uses
                        .iter()
                        .copied()
                        .filter(|value| *value != arg.value || address_values.contains(value))
                        .collect();
                    result.raised = None;
                    return result;
                }
            }
        }
    }
    if !matches!(
        op.kind,
        Kind::Add
            | Kind::AddCarry
            | Kind::And
            | Kind::Or
            | Kind::Xor
            | Kind::Mul
            | Kind::Sub
            | Kind::SubBorrow
            | Kind::Divmod
            | Kind::PtrOffset
    ) || op.args.len() != 2
    {
        return op.clone();
    }
    if op.kind == Kind::Mul && op.results.len() != 1 {
        return op.clone();
    }
    let mut replaced = BTreeSet::new();
    let mut removed = Vec::new();
    let mut args = Vec::new();
    let ordered = matches!(op.kind, Kind::Sub | Kind::SubBorrow | Kind::Divmod | Kind::PtrOffset);
    for (index, arg) in op.args.iter().enumerate() {
        let position = !ordered || index == 1;
        if let (true, Arg::Held(held)) = (position, arg) {
            if let Some(fact) = facts.get(&held.value).filter(|fact| fact.width >= held.width) {
                args.push(Arg::Const(Const::new(consts::masked(&fact.n, held.width), held.width)));
                replaced.insert(held.value);
                continue;
            }
        }
        if let (true, Arg::Cell(cell)) = (position, arg) {
            if op.stores.is_empty() && !op.barrier() && op.loads.contains(&cell.r#ref) {
                if let Some(fact) = consts::_cell(memory, &cell.r#ref) {
                    args.push(Arg::Const(Const::new(fact.n, cell.r#ref.width)));
                    removed.push(cell.r#ref.clone());
                    continue;
                }
            }
        }
        args.push(arg.clone());
    }
    if replaced.is_empty() && removed.is_empty() {
        return op.clone();
    }
    if !ordered && matches!(args[0], Arg::Const(_)) && matches!(args[1], Arg::Held(_)) {
        args.reverse();
    }
    let mut retained = args
        .iter()
        .filter_map(|arg| match arg {
            Arg::Held(held) => Some(held.value),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    retained.extend(
        op.loads
            .iter()
            .chain(&op.stores)
            .flat_map(|reference| [reference.base, reference.segment])
            .flatten(),
    );
    let mut result = op.clone();
    result.args = args;
    result.loads = op.loads.iter().filter(|reference| !removed.contains(reference)).cloned().collect();
    if !removed.is_empty() {
        result.source_backed = false;
        result.raised = None;
    }
    result.uses = op
        .uses
        .iter()
        .copied()
        .filter(|value| !replaced.contains(value) || op.merges.contains_key(value) || retained.contains(value))
        .collect();
    result
}

/// Substitute the value read for an argument, keeping its stack write.
pub(crate) fn _constant_argument(
    op: &Op,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    memory: &crate::analysis::consts::Cells,
    symbols: Option<&_SymbolCopies<'_>>,
) -> Op {
    use crate::analysis::consts;
    use crate::model::mir::Const;

    if op.args.len() != 1 || !op.defines.is_empty() || !op.merges.is_empty() || op.barrier() {
        return op.clone();
    }
    let arg = &op.args[0];
    let width = match arg {
        Arg::Held(held) => {
            if !op.loads.is_empty() {
                return op.clone();
            }
            held.width
        }
        Arg::Cell(cell) => {
            if op.loads != [cell.r#ref.clone()] || op.stores.contains(&cell.r#ref) {
                return op.clone();
            }
            cell.r#ref.width
        }
        _ => return op.clone(),
    };
    let mut owner = None;
    let literal = match consts::_operand(op, arg, facts, Some(memory)).filter(|fact| fact.width >= width) {
        Some(fact) => Arg::Const(Const::new(consts::masked(&fact.n, width), width)),
        None => {
            let known = match (symbols, arg) {
                (Some(symbols), Arg::Held(held)) => symbols.get(&held.value),
                _ => None,
            };
            let Some((symbol, defining)) = known.filter(|known| known.0.width == width) else {
                return op.clone();
            };
            owner = Some(*defining);
            Arg::Symbol(*symbol)
        }
    };
    let kept = op
        .loads
        .iter()
        .filter(|reference| match arg {
            Arg::Cell(cell) => **reference != cell.r#ref,
            _ => true,
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut uses = Vec::new();
    for value in kept
        .iter()
        .chain(&op.stores)
        .flat_map(|reference| [reference.base, reference.segment])
        .flatten()
    {
        if !uses.contains(&value) {
            uses.push(value);
        }
    }
    let mut result = op.clone();
    result.args = vec![literal];
    result.uses = uses;
    result.loads = kept;
    result.source_backed = false;
    result.raised = None;
    // A number owns no relocation.  A symbol takes the defining copy's
    // identity as well as its value: that is how emission moves the
    // original fixup, including its frame, onto this argument.  Keeping
    // the argument's id instead emitted `push 0`; inventing a fresh
    // fixup instead mistook DIVMOD's CS-relative handler for DGROUP data.
    result.id = owner.map_or(op.id, |owner| owner.id);
    result.symbol = Some(owner.is_some());
    result
}

/// The operation as a move of its own answer, where that is possible.
pub(crate) fn _folded_op(op: &Op, facts: &IndexMap<Value, crate::analysis::consts::Known>, wanted: &BTreeSet<Value>) -> Op {
    use crate::model::mir::Const;

    if op.kind == Kind::Nothing || !op.stores.is_empty() {
        return op.clone();
    }
    // Not a register move. Rewriting `mov ax,cx` to `mov ax,3` removes no
    // work, and it undoes an allocation: a live range split is exactly that
    // move, so folding it puts the computation back inside the loop the
    // hoist took it out of, and the hoist lifts it again next round.
    if op.kind == Kind::Copy {
        return op.clone();
    }
    if matches!(op.kind, Kind::Jump | Kind::Branch | Kind::Call | Kind::Return) {
        return op.clone();
    }
    // It has to write a value. A widening multiply writes two -- and its
    // answer is the first, which the operation says itself.
    let Some(Arg::Held(into)) = op.results.first() else {
        return op.clone();
    };

    let Some(target) = crate::analysis::consts::_defined(op).filter(|target| facts.contains_key(target)) else {
        return op.clone();
    };
    // A second result that something reads is not expressible as one move.
    if op.defines.iter().any(|one| *one != target && wanted.contains(one)) {
        return op.clone();
    }

    let fact = &facts[&target];
    if fact.width < into.width {
        return op.clone();
    }
    if op.kind == Kind::Copy && op.args.iter().any(|one| matches!(one, Arg::Const(_))) {
        return op.clone(); // already says so
    }

    let mut result = op.clone();
    result.kind = Kind::Copy;
    result.defines = vec![target];
    result.uses = Vec::new();
    result.loads = Vec::new();
    result.args = vec![Arg::Const(Const::new(fact.n.clone(), into.width))];
    result.results = vec![Arg::Held(Held { value: target, width: into.width })];
    result.symbol = Some(false);
    result.source_backed = false;
    result.raised = None;
    result
}
// ==== END E ====

// ==== BEGIN C2: transform.py 2564-2869 (agent C) ====
// ==== END C2 ====

// ==== BEGIN F: transform.py 2870-3297 (primary) ====
// ==== END F ====

#[cfg(test)]
#[path = "transform_tests.rs"]
mod transform_tests;
