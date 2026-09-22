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

/// One operation where two computed the same thing from the same values.
pub(crate) fn subexpressions(body: &MirBody, dgroup: &BTreeSet<i64>, avoid_store_crossing: bool) -> Result<MirBody, String> {
    use crate::analysis::{floatbounds, floatfacts, occurrence};

    let doms = loopy::dominators(&body.blocks, Some(body.entry));
    let order: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    let whole = _widths(body);
    let demanded = halves(body);

    let exact = if body.blocks.iter().any(|block| block.ops.iter().any(|op| op.floating.is_some())) {
        floatfacts::known(body, dgroup, &IndexMap::new(), None)
    } else {
        IndexMap::new()
    };
    let bounded = floatbounds::exact(body, &exact, dgroup)?;

    let mut seen: IndexMap<_Computation, Vec<(usize, usize, Op)>> = IndexMap::new();
    // What a name numbers as -- copies included.  `standing` mirrors it for
    // `_provider`, which takes an ordered map.
    let mut stands: IndexMap<u32, Value> = IndexMap::new();
    let mut standing: BTreeMap<u32, Value> = BTreeMap::new();
    let mut swap: BTreeMap<u32, Value> = BTreeMap::new(); // what a name is rewritten to -- only what folded
    let mut gone: BTreeSet<OpOccurrence> = BTreeSet::new();
    let mut floating_gone: BTreeSet<OpOccurrence> = BTreeSet::new();
    for (occurrence, block, op) in occurrence::operations(body) {
        let index = occurrence.operation_index();
        let here = order[&block.at];
        if let Some(stored) = _exact_stored_load(op, &exact) {
            if let Some(key) = _computation(&stored, &stands, &whole) {
                seen.entry(key).or_default().push((here, index, stored));
            }
        }
        let preserved_dead =
            !op.merges.is_empty() && op.merges.values().all(|value| !demanded.contains(&(*value, HIGH)));
        let mut source = _copied(op, &whole);
        if source.is_none() && preserved_dead {
            source = _copied(&Op { merges: OrderedMap::new(), ..op.clone() }, &whole);
        }
        if let Some(found) = source {
            let result = op.defines[0];
            let found = _provider(found, &standing).map_err(|error| error.to_string())?;
            stands.insert(result.id, found);
            standing.insert(result.id, found);
            if op.merges.is_empty() || _width(result, op) == Some(4) || !demanded.contains(&(result, HIGH)) {
                swap.insert(result.id, found);
                gone.insert(occurrence);
            }
            continue;
        }
        // A narrow result carries the prior value's high half as a merge,
        // a real dependency only while somebody reads the preserved half.
        // When half-liveness proves it dead, the operation's identity is
        // its width-sized arithmetic.
        let stripped;
        let semantic = if preserved_dead {
            stripped = Op { merges: OrderedMap::new(), ..op.clone() };
            &stripped
        } else {
            op
        };
        let Some(key) = _computation(semantic, &stands, &whole) else {
            continue;
        };
        let candidates = seen.entry(key).or_default();
        let first = candidates
            .iter()
            .rev()
            .find(|candidate| _reaches(candidate.0, candidate.1, here, index, &doms, body, block))
            .cloned();
        let Some((at, where_, earlier)) = first else {
            candidates.push((here, index, op.clone()));
            continue;
        };
        if op.floating.is_some()
            && !_reusable_float_path(body, body.blocks[at].at, where_, block.at, index, &exact, &bounded)
        {
            candidates.push((here, index, op.clone()));
            continue;
        }
        if !op.loads.is_empty()
            && (at != here || !_undisturbed(op, &earlier, &block.ops[where_ + 1..index], dgroup))
        {
            candidates.push((here, index, op.clone()));
            continue;
        }
        if !op.loads.is_empty()
            && avoid_store_crossing
            && block.ops[where_ + 1..index].iter().any(|crossed| !crossed.stores.is_empty())
        {
            candidates.push((here, index, op.clone()));
            continue;
        }
        if earlier.defines.len() != op.defines.len() {
            continue;
        }
        if op.defines.iter().any(|one| one.flags && _read(body, *one)) {
            continue;
        }
        for (mine, theirs) in op.defines.iter().zip(&earlier.defines) {
            stands.insert(mine.id, *theirs);
            standing.insert(mine.id, *theirs);
            swap.insert(mine.id, *theirs);
        }
        if op.floating.is_some() {
            floating_gone.insert(occurrence);
        } else {
            gone.insert(occurrence);
        }
    }

    if gone.is_empty() && floating_gone.is_empty() {
        return Ok(body.clone());
    }
    let mut body = body.clone();
    if !floating_gone.is_empty() {
        let erased = floating_gone
            .iter()
            .map(|one| (one.block_index(), one.operation_index()))
            .collect::<BTreeSet<_>>();
        for (block_index, block) in body.blocks.iter_mut().enumerate() {
            for (operation_index, op) in block.ops.iter_mut().enumerate() {
                if erased.contains(&(block_index, operation_index)) {
                    *op = _erased_floating(op);
                }
            }
        }
    }
    if !gone.is_empty() {
        body = _reclaimed(&body, &gone);
    }
    let mut blocks = Vec::with_capacity(body.blocks.len());
    for block in &body.blocks {
        let phis = block.phis.iter().map(|phi| _phi_reading(phi, &swap)).collect::<Result<Vec<_>, _>>()?;
        let ops = block
            .ops
            .iter()
            .map(|op| _substituted(op, &swap))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        blocks.push(MirBlock { phis, ops, ..block.clone() });
    }
    Ok(MirBody { blocks, ..body })
}

pub(crate) fn _reusable_float_path(
    body: &MirBody,
    source: i64,
    first: usize,
    destination: i64,
    last: usize,
    facts: &IndexMap<Value, crate::analysis::floatfacts::Finite>,
    bounded: &BTreeSet<OpOccurrence>,
) -> bool {
    use crate::model::floating::Exceptions;

    #[allow(clippy::too_many_arguments)]
    fn visit(
        at: i64,
        (source, first, destination, last, deferred): (i64, usize, i64, usize, bool),
        blocks: &IndexMap<i64, (usize, &MirBlock)>,
        predecessors: &BTreeMap<i64, BTreeSet<i64>>,
        facts: &IndexMap<Value, crate::analysis::floatfacts::Finite>,
        bounded: &BTreeSet<(usize, usize)>,
        active: &mut BTreeSet<i64>,
        checked: &mut BTreeMap<i64, bool>,
    ) -> bool {
        if active.contains(&at) {
            return false;
        }
        if let Some(result) = checked.get(&at) {
            return *result;
        }
        active.insert(at);
        let (block_index, block) = blocks[&at];
        let ops = &block.ops;
        let start = if at == source { first } else { 0 };
        let end = if at == destination { last + 1 } else { ops.len() };
        let exact = ops[start..end].iter().enumerate().all(|(offset, op)| {
            if deferred {
                _unchanged_float_environment(op)
            } else {
                bounded.contains(&(block_index, start + offset)) || _exact_floating(op, facts)
            }
        });
        let parents = &predecessors[&at];
        let result = exact
            && (at == source
                || (!parents.is_empty()
                    && parents.iter().all(|parent| {
                        let path = (source, first, destination, last, deferred);
                        visit(*parent, path, blocks, predecessors, facts, bounded, active, checked)
                    })));
        active.remove(&at);
        checked.insert(at, result);
        result
    }

    let blocks: IndexMap<i64, (usize, &MirBlock)> =
        body.blocks.iter().enumerate().map(|(index, block)| (block.at, (index, block))).collect();
    let candidates = [&blocks[&source].1.ops[first], &blocks[&destination].1.ops[last]];
    let deferred = candidates
        .iter()
        .all(|op| op.floating.as_ref().is_some_and(|floating| floating.exceptions == Exceptions::Deferred));
    let predecessors = loopy::predecessors(&body.blocks);
    let bounded = bounded
        .iter()
        .map(|one| (one.block_index(), one.operation_index()))
        .collect::<BTreeSet<_>>();
    let mut active: BTreeSet<i64> = BTreeSet::new();
    let mut checked: BTreeMap<i64, bool> = BTreeMap::new();
    let path = (source, first, destination, last, deferred);
    visit(destination, path, &blocks, &predecessors, facts, &bounded, &mut active, &mut checked)
}

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

/// An exact storage conversion leaves the source value available for reloads.
pub(crate) fn _exact_stored_load(op: &Op, facts: &IndexMap<Value, crate::analysis::floatfacts::Finite>) -> Option<Op> {
    use crate::model::floating::{Format, Precision, Rounding, Semantics};

    let floating = op.floating.as_ref()?;
    if op.kind != Kind::Fstore
        || *floating.inputs != [Format::Extended80]
        || !matches!(floating.result, Format::Binary32 | Format::Binary64)
        || op.stores.len() != 1
        || !op.loads.is_empty()
        || !_exact_floating(op, facts)
    {
        return None;
    }
    // Python's `(source,) = op.args` raises here; an FSTORE has one operand.
    let [source] = op.args.as_slice() else {
        return None;
    };
    let Arg::Held(source) = source else {
        return None;
    };
    if source.width != 10 {
        return None;
    }
    let rule = Semantics::with_exceptions(
        vec![floating.result],
        Format::Extended80,
        Precision::Exact,
        Rounding::None,
        floating.exceptions,
    );
    Some(Op {
        kind: Kind::Fload,
        args: vec![Arg::Cell(mir::Cell { r#ref: op.stores[0].clone() })],
        results: vec![Arg::Held(*source)],
        defines: vec![source.value],
        uses: Vec::new(),
        loads: op.stores.clone(),
        stores: Vec::new(),
        merges: OrderedMap::new(),
        floating: Some(rule),
        ..op.clone()
    })
}

/// No intervening exceptional FP work or unmodelled environment change.
pub(crate) fn _exact_floating(op: &Op, facts: &IndexMap<Value, crate::analysis::floatfacts::Finite>) -> bool {
    use crate::analysis::floatfacts;

    if op.barrier() || matches!(op.kind, Kind::Call | Kind::Opaque) {
        return false;
    }
    let Some(floating) = &op.floating else {
        return op.stack.is_none();
    };
    if op.kind == Kind::Fstore {
        return match op.args.as_slice() {
            [Arg::Held(held)] => facts
                .get(&held.value)
                .is_some_and(|fact| floatfacts::evaluated(op.kind, floating, std::slice::from_ref(fact)).is_some()),
            _ => false,
        };
    }
    matches!(op.results.as_slice(), [Arg::Held(held)] if facts.contains_key(&held.value))
}

pub(crate) fn _erased_floating(op: &Op) -> Op {
    Op {
        op: Some(mir::OpCode::nothing()),
        kind: Kind::Nothing,
        name: String::new(),
        args: Vec::new(),
        results: Vec::new(),
        uses: Vec::new(),
        defines: Vec::new(),
        loads: Vec::new(),
        stores: Vec::new(),
        merges: OrderedMap::new(),
        source_backed: false,
        raised: None,
        floating: None,
        stack: None,
        ..op.clone()
    }
}

/// Erase redundant computations without moving source provenance.
pub(crate) fn _reclaimed(body: &MirBody, gone: &BTreeSet<OpOccurrence>) -> MirBody {
    let gone = gone.iter().map(|one| (one.block_index(), one.operation_index())).collect::<BTreeSet<_>>();
    let blocks = body
        .blocks
        .iter()
        .enumerate()
        .map(|(block_index, block)| MirBlock {
            ops: block
                .ops
                .iter()
                .enumerate()
                .map(|(operation_index, op)| {
                    if gone.contains(&(block_index, operation_index)) {
                        _empty_operation(op)
                    } else {
                        op.clone()
                    }
                })
                .collect(),
            ..block.clone()
        })
        .collect();
    MirBody { blocks, ..body.clone() }
}

/// The width each value was defined at, keyed by value id.
///
/// A half of one is `Held(value, 2)` and so is the other half, so anything
/// narrow is refused rather than told apart.
pub(crate) fn _widths(body: &MirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::new();
    for block in &body.blocks {
        for op in &block.ops {
            for one in &op.results {
                if let Arg::Held(held) = one {
                    out.entry(held.value.id).or_insert(held.width);
                }
            }
        }
    }
    let mut changing = true;
    while changing {
        changing = false;
        for block in &body.blocks {
            for phi in &block.phis {
                if out.contains_key(&phi.result.id) || phi.incoming.is_empty() {
                    continue;
                }
                let widths = phi.incoming.values().map(|value| out.get(&value.id).copied()).collect::<BTreeSet<_>>();
                if widths.len() == 1 && !widths.contains(&None) {
                    out.insert(phi.result.id, widths.into_iter().next().flatten().expect("one width"));
                    changing = true;
                }
            }
        }
    }
    out
}

/// Whether this operand names the whole of its value, not one half.
pub(crate) fn _full(one: &Held, whole: &IndexMap<u32, u32>) -> bool {
    whole.get(&one.value.id) == Some(&one.width)
}

/// The value this operation is another name for, if it is only that.
pub(crate) fn _copied(op: &Op, whole: &IndexMap<u32, u32>) -> Option<Value> {
    if !matches!(op.kind, Kind::Copy | Kind::Load) || op.defines.len() != 1 {
        return None;
    }
    if !op.loads.is_empty() || !op.stores.is_empty() || !op.merges.is_empty() {
        return None;
    }
    let held = op
        .args
        .iter()
        .filter_map(|one| match one {
            Arg::Held(held) => Some(held),
            _ => None,
        })
        .collect::<Vec<_>>();
    if held.len() != 1 || op.args.len() != 1 {
        return None;
    }
    if Some(held[0].width) != _width(op.defines[0], op) || !_full(held[0], whole) {
        return None;
    }
    Some(held[0].value)
}

/// The width this operation's result comes out at, or None.
pub(crate) fn _width(_value: Value, op: &Op) -> Option<u32> {
    op.results.iter().find_map(|one| match one {
        Arg::Held(held) => Some(held.width),
        _ => None,
    })
}

/// Python's `_computation` key tuple.
///
/// `name` is `op.floating if floating else op.name`: `(Some(rule), "")` or
/// `(None, name)`.  `operands` is `(unordered, named)`: Python's two-element
/// frozenset is `(true, ..)` sorted and deduplicated, its tuple `(false, ..)`.
/// A named value is `Held` of a `Value` carrying only the id Python keys on.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct _Computation {
    pub kind: Kind,
    pub name: (Option<crate::model::floating::Semantics>, String),
    pub operands: (bool, Vec<Arg>),
    pub results: Vec<u32>,
}

/// What this operation computes, or None where that is not only its operands.
pub(crate) fn _computation(op: &Op, stands: &IndexMap<u32, Value>, whole: &IndexMap<u32, u32>) -> Option<_Computation> {
    use std::collections::HashSet;

    let floating = op.floating.is_some()
        && matches!(op.kind, Kind::Fload | Kind::Fadd | Kind::Fsub | Kind::Fmul | Kind::Fdiv | Kind::Fsqrt);
    if (!(_PURE.contains(&op.kind) || op.kind == Kind::Load) && !floating)
        || !op.stores.is_empty()
        || !op.merges.is_empty()
        || op.barrier()
    {
        return None;
    }
    if op.defines.is_empty() || op.args.is_empty() {
        return None;
    }
    let cells = op
        .args
        .iter()
        .filter_map(|arg| match arg {
            Arg::Cell(cell) => Some(&cell.r#ref),
            _ => None,
        })
        .collect::<HashSet<_>>();
    if op.loads.iter().collect::<HashSet<_>>() != cells {
        return None;
    }
    let mut named: Vec<Arg> = Vec::new();
    for one in &op.args {
        match one {
            Arg::Held(held) => {
                if !_full(held, whole) {
                    return None;
                }
                let id = stands.get(&held.value.id).unwrap_or(&held.value).id;
                named.push(Arg::Held(Held { value: Value::new(id, 0), width: held.width }));
            }
            Arg::Const(_) | Arg::Symbol(_) | Arg::FrameAddress(_) => named.push(one.clone()),
            Arg::Cell(cell) => {
                let reference = mir::symbolic_ref(&cell.r#ref);
                if !mir::same_bytes(&reference, &reference) {
                    return None;
                }
                named.push(Arg::Cell(mir::Cell { r#ref: reference }));
            }
            _ => return None,
        }
    }
    let results = op
        .results
        .iter()
        .filter_map(|one| match one {
            Arg::Held(held) => Some(held.width),
            _ => None,
        })
        .collect();
    let unordered = named.len() == 2
        && matches!(op.kind, Kind::Add | Kind::Mul | Kind::And | Kind::Or | Kind::Xor | Kind::Eq | Kind::Ne);
    if unordered {
        // Any total order will do; `typed` and `within` never compare.
        let order = |arg: &Arg| match arg {
            Arg::Cell(cell) => format!("{:?}", mir::MemRef { typed: None, within: None, ..cell.r#ref.clone() }),
            other => format!("{other:?}"),
        };
        if order(&named[0]) > order(&named[1]) {
            named.swap(0, 1);
        }
        if named[0] == named[1] {
            named.pop();
        }
    }
    let name = if floating { (op.floating.clone(), String::new()) } else { (None, op.name.clone()) };
    Some(_Computation { kind: op.kind, name, operands: (unordered, named), results })
}

/// Whether the earlier operation has certainly run by the later one.
pub(crate) fn _reaches(
    at: usize,
    where_: usize,
    then: usize,
    index: usize,
    doms: &BTreeMap<i64, BTreeSet<i64>>,
    body: &MirBody,
    block: &MirBlock,
) -> bool {
    if at == then {
        return where_ < index;
    }
    doms.get(&block.at).is_some_and(|dominating| dominating.contains(&body.blocks[at].at))
}

/// Whether anything in the body uses this value.
pub(crate) fn _read(body: &MirBody, value: Value) -> bool {
    body.blocks.iter().flat_map(|block| &block.ops).any(|op| {
        op.uses.contains(&value) || op.args.iter().any(|one| matches!(one, Arg::Held(held) if held.value == value))
    })
}

/// A divide whose answers the divide before it already computed.
///
/// The second divide becomes a copy of the first's answer, only where
/// everything else it defined is dead.
pub(crate) fn reused_divides(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    found: Option<&std::sync::Arc<dyn std::any::Any + Send + Sync>>,
) -> Result<MirBody, String> {
    use crate::model::ir::Operation;

    let _ = found;
    let pairs_found = divided_twice(body, dgroup);
    if pairs_found.is_empty() {
        return Ok(body.clone());
    }
    let alive = live(body);
    let mut into: IndexMap<*const Op, Op> = IndexMap::new();
    for (_at, earlier, one) in pairs_found {
        if one.results.len() != 2 || earlier.results.len() != 2 {
            continue;
        }
        let (mut served, mut wanted) = (None, None);
        for (mine, theirs) in one.results.iter().zip(&earlier.results) {
            if let (Arg::Held(mine), Arg::Held(theirs)) = (mine, theirs) {
                if alive.contains(&mine.value) {
                    (served, wanted) = (Some(*theirs), Some(*mine));
                }
            }
        }
        let (Some(served), Some(wanted)) = (served, wanted) else {
            continue;
        };
        // One register is what a copy writes, so every other value the site
        // defined has to be dead.  They stay on `defines`: a definition ends
        // a live range, and a phi naming one still has to find it.
        if one.defines.iter().any(|value| *value != wanted.value && alive.contains(value)) {
            continue;
        }
        into.insert(
            std::ptr::from_ref(one),
            Op {
                kind: Kind::Copy,
                op: Some(mir::OpCode::Operation(Operation::Move)),
                name: "mov".to_string(),
                defines: one.defines.clone(),
                uses: vec![served.value],
                loads: Vec::new(),
                stores: Vec::new(),
                merges: OrderedMap::new(),
                args: vec![Arg::Held(served)],
                results: vec![Arg::Held(wanted)],
                source_backed: false,
                ..one.clone()
            },
        );
    }
    if into.is_empty() {
        return Ok(body.clone());
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| MirBlock {
            ops: block.ops.iter().map(|op| into.get(&std::ptr::from_ref(op)).unwrap_or(op).clone()).collect(),
            ..block.clone()
        })
        .collect();
    Ok(MirBody { blocks, ..body.clone() })
}

/// Each divide whose answers the divide before it already computed, as
/// (which block, first, second).
pub(crate) fn divided_twice<'a>(body: &'a MirBody, dgroup: &BTreeSet<i64>) -> Vec<(i64, &'a Op, &'a Op)> {
    let mut found = Vec::new();
    for block in &body.blocks {
        for (index, one) in block.ops.iter().enumerate() {
            if one.kind != Kind::Divmod {
                continue;
            }
            let Some(earlier) =
                block.ops[..index].iter().rev().find(|other| other.kind == Kind::Divmod && other.args == one.args)
            else {
                continue;
            };
            // Python's `list.index`: the first equal operation.
            let position = block.ops.iter().position(|other| other == earlier).expect("earlier is in the block");
            let between = &block.ops[position + 1..index];
            if !_undisturbed(one, earlier, between, dgroup) {
                continue;
            }
            found.push((block.at, earlier, one));
        }
    }
    found
}

/// Whether the second divide still reads what the first one read.
pub(crate) fn _undisturbed(one: &Op, _earlier: &Op, between: &[Op], _dgroup: &BTreeSet<i64>) -> bool {
    let cells = one
        .args
        .iter()
        .filter_map(|arg| match arg {
            Arg::Cell(cell) => Some(&cell.r#ref),
            _ => None,
        })
        .collect::<Vec<_>>();
    for other in between {
        if other.barrier() || matches!(other.kind, Kind::Call | Kind::Escape) {
            return false;
        }
        if cells.iter().any(|reference| {
            other.stores.iter().any(|wrote| {
                crate::analysis::regions::overlapping(reference, wrote, None, None, None).unwrap_or(true)
            })
        }) {
            return false;
        }
    }
    true
}

/// Anything standing inside a call's argument run, moved ahead of it.
pub(crate) fn placed(body: &MirBody, dgroup: &BTreeSet<i64>, calls: &IndexMap<i64, String>) -> Result<MirBody, String> {
    let mut out = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut ops = block.ops.clone();
        for index in (0..ops.len()).rev() {
            if ops[index].kind != Kind::Call {
                continue;
            }
            let Some((first, standing)) = _argument_run(&ops, index, dgroup, calls) else {
                continue;
            };
            let kept = ops
                .iter()
                .enumerate()
                .filter(|(at, _)| !standing.contains(at))
                .map(|(_, one)| one.clone())
                .collect::<Vec<_>>();
            let ahead = standing.iter().map(|at| ops[*at].clone()).collect::<Vec<_>>();
            ops = kept[..first].iter().cloned().chain(ahead).chain(kept[first..].iter().cloned()).collect();
            changed = true;
        }
        out.push(MirBlock { ops, ..block.clone() });
    }
    Ok(if changed { MirBody { blocks: out, ..body.clone() } } else { body.clone() })
}

/// (where the run starts, which of its operations do not belong to it).
///
/// Walked back from the call, taking pushes and anything that may pass
/// them.  None where no push was reached, or where nothing stands among them.
pub(crate) fn _argument_run(
    ops: &[Op],
    call: usize,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
) -> Option<(usize, BTreeSet<usize>)> {
    let (mut first, mut standing, mut pushes) = (call, BTreeSet::new(), 0);
    let mut at = call;
    while at > 0 {
        at -= 1;
        let one = &ops[at];
        if one.kind == Kind::Arg {
            pushes += 1;
            first = at;
            continue;
        }
        if !_may_pass(one, &ops[at + 1..call], dgroup, calls) {
            break;
        }
        standing.insert(at);
        first = at;
    }
    if pushes == 0 || standing.is_empty() {
        return None;
    }
    // Only what stands among the pushes.  Anything collected before the
    // first one is not in the run and has no reason to move.
    let lowest = (first..call).find(|x| ops[*x].kind == Kind::Arg).expect("a push was reached");
    let standing = standing.into_iter().filter(|x| *x > lowest).collect::<BTreeSet<_>>();
    if standing.is_empty() {
        None
    } else {
        Some((lowest, standing))
    }
}

/// Whether this operation can move ahead of the run standing after it.
pub(crate) fn _may_pass(one: &Op, run: &[Op], _dgroup: &BTreeSet<i64>, calls: &IndexMap<i64, String>) -> bool {
    if one.barrier() || _OBSERVED.contains(&one.kind) || calls.contains_key(&one.at) {
        return false;
    }
    let made = run.iter().flat_map(|other| &other.defines).collect::<BTreeSet<_>>();
    if one.uses.iter().any(|used| made.contains(used)) {
        return false;
    }
    let wrote = one.defines.iter().collect::<BTreeSet<_>>();
    if run.iter().flat_map(|other| &other.uses).any(|used| wrote.contains(used)) {
        return false;
    }
    // A push moves the stack pointer, so `mov bp,sp` may not pass one.
    let (written, read) = mir::unheld(one);
    for other in run {
        let (theirs_written, theirs_read) = mir::unheld(other);
        if _meets(written.as_ref(), theirs_read.as_ref()) || _meets(read.as_ref(), theirs_written.as_ref()) {
            return false;
        }
    }
    for reference in one.loads.iter().chain(&one.stores) {
        for other in run {
            for theirs in other.loads.iter().chain(&other.stores) {
                if crate::analysis::regions::overlapping(reference, theirs, None, None, None).unwrap_or(true) {
                    return false;
                }
            }
        }
    }
    true
}

/// Whether two sets share anything, None being everything.
pub(crate) fn _meets(one: Option<&BTreeSet<String>>, other: Option<&BTreeSet<String>>) -> bool {
    match (one, other) {
        (Some(one), Some(other)) => !one.is_disjoint(other),
        _ => one.is_none_or(|set| !set.is_empty()) && other.is_none_or(|set| !set.is_empty()),
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
// ==== END E ====

// ==== BEGIN C2: transform.py 2564-2869 (agent C) ====
// ==== END C2 ====

// ==== BEGIN F: transform.py 2870-3297 (primary) ====
// ==== END F ====

#[cfg(test)]
#[path = "transform_tests.rs"]
mod transform_tests;
