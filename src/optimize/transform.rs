//! Port of `qbopt/optimize/transform.py`: MIR transforms, a body in, an
//! optimised body out.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::support::hash::IndexMap;

use crate::analysis::loops::{self as loopy, Loop};
use crate::analysis::occurrence::OpOccurrence;
use crate::analysis::ssa::{provider as _provider, substituted as _substituted};
use crate::model::mir::{self, Arg, Held, Kind, MirBlock, MirBody, Op, OrderedMap, Phi, Value};

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

/// One operation where two computed the same thing from the same values.
pub(crate) fn subexpressions(body: &Rc<MirBody>, dgroup: &BTreeSet<i64>, avoid_store_crossing: bool) -> Result<Rc<MirBody>, String> {
    use crate::analysis::{floatbounds, floatfacts, occurrence};

    let doms = loopy::dominators(&body.blocks, Some(body.entry));
    let order: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    let whole = _widths(body);
    let demanded = halves(body);

    // Float facts are asked only by an exact Fstore or a float op met twice.
    let exact = std::cell::OnceCell::new();
    let exact = || {
        exact.get_or_init(|| {
            if body.blocks.iter().any(|block| block.ops.iter().any(|op| op.floating.is_some())) {
                floatfacts::known(body, dgroup, &IndexMap::default(), None)
            } else {
                IndexMap::default()
            }
        })
    };
    let bounded = std::cell::OnceCell::new();

    let mut seen: IndexMap<_Computation, Vec<(usize, usize, Op)>> = IndexMap::default();
    // What a name numbers as -- copies included.  `standing` mirrors it for
    // `_provider`, which takes an ordered map.
    let mut stands: IndexMap<u32, Value> = IndexMap::default();
    let mut standing: BTreeMap<u32, Value> = BTreeMap::new();
    let mut swap: BTreeMap<u32, Value> = BTreeMap::new(); // what a name is rewritten to -- only what folded
    let mut gone: BTreeSet<OpOccurrence> = BTreeSet::new();
    let mut floating_gone: BTreeSet<OpOccurrence> = BTreeSet::new();
    for (occurrence, block, op) in occurrence::operations(body) {
        let index = occurrence.operation_index();
        let here = order[&block.at];
        if let Some(stored) = _exact_stored_load(op, exact) {
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
            && !_reusable_float_path(body, body.blocks[at].at, where_, block.at, index, exact(), match bounded.get() {
                Some(bounded) => bounded,
                None => {
                    let found = floatbounds::exact(body, exact(), dgroup)?;
                    bounded.get_or_init(|| found)
                }
            })
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
    let mut body = MirBody::clone(body);
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
        blocks.push(MirBlock { phis, ..block.with_ops(ops) });
    }
    Ok(Rc::new(MirBody { blocks, ..body }))
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
pub(crate) fn _exact_stored_load<'a>(
    op: &Op,
    facts: impl FnOnce() -> &'a IndexMap<Value, crate::analysis::floatfacts::Finite>,
) -> Option<Op> {
    use crate::model::floating::{Format, Precision, Rounding, Semantics};

    let floating = op.floating.as_ref()?;
    if op.kind != Kind::Fstore
        || *floating.inputs != [Format::Extended80]
        || !matches!(floating.result, Format::Binary32 | Format::Binary64)
        || op.stores.len() != 1
        || !op.loads.is_empty()
        || !_exact_floating(op, facts())
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
        .map(|(block_index, block)| block.with_ops(block
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
                .collect()))
        .collect();
    body.with_blocks(blocks)
}

/// The width each value was defined at, keyed by value id.
///
/// A half of one is `Held(value, 2)` and so is the other half, so anything
/// narrow is refused rather than told apart.
pub(crate) fn _widths(body: &MirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
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
    use crate::support::hash::HashSet;

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
                named.push(Arg::Cell(mir::Cell { r#ref: reference.into_owned() }));
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
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    found: Option<&Rc<crate::objectfile::module::Module>>,
) -> Result<Rc<MirBody>, String> {
    use crate::model::ir::Operation;

    let _ = found;
    let pairs_found = divided_twice(body, dgroup);
    if pairs_found.is_empty() {
        return Ok(body.clone());
    }
    let alive = live(body);
    let mut into: IndexMap<*const Op, Op> = IndexMap::default();
    // A divide made a copy computes neither answer any more; one after it
    // reads them from the divide that copy was served by.
    let mut answers: IndexMap<mir::Value, mir::Held> = IndexMap::default();
    for (_at, earlier, one) in pairs_found {
        if one.results.len() != 2 || earlier.results.len() != 2 {
            continue;
        }
        let (mut served, mut wanted) = (None, None);
        for (mine, theirs) in one.results.iter().zip(&earlier.results) {
            if let (Arg::Held(mine), Arg::Held(theirs)) = (mine, theirs) {
                if alive.contains(&mine.value) {
                    (served, wanted) = (Some(*answers.get(&theirs.value).unwrap_or(theirs)), Some(*mine));
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
        for (mine, theirs) in one.results.iter().zip(&earlier.results) {
            if let (Arg::Held(mine), Arg::Held(theirs)) = (mine, theirs) {
                let leader = *answers.get(&theirs.value).unwrap_or(theirs);
                answers.insert(mine.value, leader);
            }
        }
    }
    if into.is_empty() {
        return Ok(body.clone());
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| block.with_ops(block.ops.iter().map(|op| into.get(&std::ptr::from_ref(op)).unwrap_or(op).clone()).collect()))
        .collect();
    Ok(Rc::new(body.with_blocks(blocks)))
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
pub(crate) fn placed(body: &Rc<MirBody>, dgroup: &BTreeSet<i64>, calls: &IndexMap<i64, String>) -> Result<Rc<MirBody>, String> {
    // Copied on the first move, so an unchanged body comes back as itself.
    let mut out: Option<MirBody> = None;
    for number in 0..body.blocks.len() {
        for index in (0..body.blocks[number].ops.len()).rev() {
            let ops = &out.as_ref().unwrap_or(body).blocks[number].ops;
            if ops[index].kind != Kind::Call {
                continue;
            }
            let Some((first, standing)) = _argument_run(ops, index, dgroup, calls) else {
                continue;
            };
            let kept = (0..ops.len()).filter(|at| !standing.contains(at)).collect::<Vec<_>>();
            let order = kept[..first].iter().chain(&standing).chain(&kept[first..]).copied().collect::<Vec<_>>();
            let block = &mut out.get_or_insert_with(|| MirBody::clone(body)).blocks[number];
            let mut ops = std::mem::take(&mut block.ops).into_iter().map(Some).collect::<Vec<_>>();
            block.ops = order.into_iter().map(|at| ops[at].take().expect("each once")).collect();
        }
    }
    Ok(out.map_or_else(|| Rc::clone(body), Rc::new))
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

/// Every store overwritten, or never observable, before anything read it, removed.
pub(crate) fn without_dead_stores(
    body: &Rc<MirBody>,
    _dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    private: Option<&dyn Fn(&mir::MemRef) -> bool>,
    bounds: Option<&IndexMap<(crate::objectfile::module::Space, i64), Vec<i64>>>,
    handles_errors: bool,
) -> Result<Rc<MirBody>, String> {
    let gone: BTreeSet<*const Op> =
        crate::analysis::avail::dead_stores(body, None, calls, private, bounds, handles_errors)
            .into_iter()
            .map(|op| op as *const Op)
            .collect();
    if gone.is_empty() {
        return Ok(body.clone());
    }
    let mut out = MirBody::clone(body);
    out.blocks = body
        .blocks
        .iter()
        .map(|one| {
            let mut block = one.clone();
            block.ops = _without(&one.ops, |op| gone.contains(&(op as *const Op)));
            block
        })
        .collect();
    Ok(Rc::new(out))
}

// A root register at the width an operand reads it. ir.ROOT maps the narrow
// name to the wide one; this is the way back, and only for the general
// registers -- a segment register has no narrower form and is never a
// provider here.

/// Replace known memory operands with SSA values, extending their uses.
pub(crate) fn forwarded(
    body: &Rc<MirBody>,
    _dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    avoid_store_crossing: bool,
) -> Result<Rc<MirBody>, String> {
    use crate::analysis::avail::{self, Holder};

    let want: BTreeSet<i64> =
        body.blocks.iter().flat_map(|block| &block.ops).filter(|op| !op.loads.is_empty()).map(|op| op.at).collect();
    if want.is_empty() {
        return Ok(body.clone());
    }
    let mut served: IndexMap<*const Op, Holder> = IndexMap::default();
    for one in avail::forwardable(body, None, calls, &want) {
        if let Some(op) = one.op {
            served.insert(op as *const Op, one.value);
        }
    }
    if avoid_store_crossing {
        let mut locations: IndexMap<*const Op, (i64, usize)> = IndexMap::default();
        let mut definitions: IndexMap<Value, (i64, usize)> = IndexMap::default();
        let mut by_at: IndexMap<i64, &MirBlock> = IndexMap::default();
        let mut op_by_id: IndexMap<*const Op, &Op> = IndexMap::default();
        for block in &body.blocks {
            by_at.insert(block.at, block);
            for (index, op) in block.ops.iter().enumerate() {
                locations.insert(op as *const Op, (block.at, index));
                for value in &op.defines {
                    definitions.insert(*value, (block.at, index));
                }
                op_by_id.insert(op as *const Op, op);
            }
        }
        let predecessors = loopy::predecessors(&body.blocks);

        let blocks_reaching = |destination: i64| -> BTreeSet<i64> {
            let mut reached = BTreeSet::from([destination]);
            let mut work = vec![destination];
            while let Some(at) = work.pop() {
                let new: Vec<i64> = predecessors[&at].difference(&reached).copied().collect();
                reached.extend(new.iter().copied());
                work.extend(new);
            }
            reached
        };

        // Python's slice `ops[low:high]`: clamped, empty when inverted.
        let stores_in = |block: &MirBlock, low: usize, high: usize| -> bool {
            let high = high.min(block.ops.len());
            low < high && block.ops[low..high].iter().any(|one| !one.stores.is_empty())
        };

        let crosses_store = |op: &Op, holder: &Holder| -> bool {
            let Holder::Value(holder) = holder else {
                return false;
            };
            let source = definitions.get(holder).copied();
            let destination = locations[&(op as *const Op)];
            let Some(source) = source else {
                return true;
            };
            if source.0 == destination.0 {
                return source.1 >= destination.1 || stores_in(by_at[&source.0], source.1 + 1, destination.1);
            }
            let reaching = blocks_reaching(destination.0);
            if !reaching.contains(&source.0) {
                return true;
            }
            let mut seen: BTreeSet<i64> = BTreeSet::new();
            let mut work = vec![source.0];
            let mut arrived = false;
            while let Some(at) = work.pop() {
                if seen.contains(&at) || !reaching.contains(&at) {
                    continue;
                }
                seen.insert(at);
                let block = by_at[&at];
                let low = if at == source.0 { source.1 + 1 } else { 0 };
                let high = if at == destination.0 { destination.1 } else { block.ops.len() };
                if stores_in(block, low, high) {
                    return true;
                }
                if at == destination.0 {
                    arrived = true;
                } else {
                    work.extend(block.succ.iter().copied());
                }
            }
            !arrived
        };

        served = served
            .into_iter()
            .filter(|(identity, holder)| !crosses_store(op_by_id[identity], holder))
            .collect();
    }
    if served.is_empty() {
        return Ok(body.clone());
    }

    let mut out = Vec::new();
    for block in &body.blocks {
        let mut ops: Vec<Op> = Vec::new();
        for op in &block.ops {
            let mut holder = served.get(&(op as *const Op));
            if matches!(holder, Some(Holder::Const(_) | Holder::Symbol(_))) && op.kind != Kind::Load {
                holder = None;
            }
            let args = holder.and_then(|holder| _served(op, holder));
            let (Some(holder), Some(args)) = (holder, args) else {
                ops.push(op.clone());
                continue;
            };
            let mut next = op.clone();
            next.args = args;
            next.loads = Vec::new();
            match holder {
                Holder::Const(_) | Holder::Symbol(_) => {
                    // Only a load, which becomes the constant itself. An
                    // arithmetic operand is a machine question this is not
                    // allowed to answer: `idiv [x]` has no immediate form.
                    // A constant is not a value either, so `uses` does not grow.
                    next.kind = Kind::Copy;
                    next.op = Some(mir::OpCode::Operation(crate::model::ir::Operation::Move));
                    next.name = "mov".to_owned();
                    next.source_backed = false;
                    next.raised = None;
                    next.symbol = Some(false);
                }
                Holder::Value(value) if op.kind == Kind::Load => {
                    // A load served by a value is a copy of it. Left a load, the
                    // counter's `mov ax,[x]` hid PLASMA's x from induction.
                    next.kind = Kind::Copy;
                    next.op = Some(mir::OpCode::Operation(crate::model::ir::Operation::Move));
                    next.name = "mov".to_owned();
                    next.uses.push(*value);
                    next.source_backed = false;
                    next.raised = None;
                    next.symbol = Some(false);
                }
                Holder::Value(value) => next.uses.push(*value),
            }
            ops.push(next);
        }
        out.push(block.with_ops(ops));
    }
    let mut result = MirBody::clone(body);
    result.blocks = out;
    Ok(Rc::new(result))
}

/// `op`'s one memory source read from whatever holds `holder` instead.
///
/// A value, not a register: which one holds it is the allocator's answer.
pub(crate) fn _served(op: &Op, holder: &crate::analysis::avail::Holder) -> Option<Vec<Arg>> {
    use crate::analysis::avail::Holder;

    if op.floating.is_some() {
        return None;
    }
    let cells: Vec<usize> =
        op.args.iter().enumerate().filter(|(_, one)| matches!(one, Arg::Cell(_))).map(|(index, _)| index).collect();
    if cells.len() != 1 {
        return None;
    }
    let at = cells[0];
    let Arg::Cell(cell) = &op.args[at] else { unreachable!("a cell") };
    if op.results.contains(&op.args[at]) {
        // An update in place: the read is the write's own operand, and serving
        // it turns one instruction into a load, the operation and a store.
        return None;
    }
    let replacement = match holder {
        Holder::Const(one) => {
            if one.width != cell.r#ref.width {
                return None;
            }
            Arg::Const(one.clone())
        }
        Holder::Symbol(one) => {
            if one.width != cell.r#ref.width {
                return None;
            }
            Arg::Symbol(*one)
        }
        Holder::Value(value) => Arg::Held(Held { value: *value, width: cell.r#ref.width }),
    };
    Some(op.args.iter().enumerate().map(|(index, one)| if index == at { replacement.clone() } else { one.clone() }).collect())
}
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

/// Every value some instruction reads, rather than merely preserves.
///
/// Transitive through phis, and grown from what is definitely read so a
/// cycle cannot talk itself into being effective.
pub(crate) fn _effective(body: &MirBody, calls: &IndexMap<i64, String>) -> BTreeSet<Value> {
    let _ = calls;
    let mut wanted: BTreeSet<Value> = BTreeSet::new();
    let mut carrying: IndexMap<Value, BTreeSet<Value>> = IndexMap::default();
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
    intervals: Option<&crate::support::hash::HashMap<usize, &BTreeMap<Value, crate::analysis::ranges::Interval>>>,
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

type _HalvesReuse = crate::support::hash::HashMap<usize, (Rc<MirBody>, BTreeSet<(Value, u8)>)>;

thread_local! {
    /// Python's `_halves_reuse` context variable.  Holding the body keeps its
    /// address from being recycled, as Python's holding keeps its `id`.
    #[allow(non_upper_case_globals)]
    static _halves_reuse: std::cell::RefCell<Option<_HalvesReuse>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
thread_local! {
    /// Half-liveness fixed points solved, for the tests that pin reuse to Python's.
    pub(crate) static HALVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Share half-liveness for immutable states in one transaction.
pub(crate) fn _reusing_halves<T>(inside: impl FnOnce() -> T) -> T {
    let token = _halves_reuse.with(|reuse| reuse.replace(Some(crate::support::hash::HashMap::default())));
    let result = inside();
    _halves_reuse.with(|reuse| *reuse.borrow_mut() = token);
    result
}
/// One step of `halves`' fixed point, on dense value indices.
enum _HalfStep {
    /// Add the halves in the mask.
    Add(u32, u8),
    /// Add HIGH of the second where the first's HIGH is read.
    Carry(u32, u32),
}

/// `halves`' fixed point compiled once per body: the same set operations in
/// the same order, on a mask per value index instead of a set of pairs.
#[derive(Default)]
struct _HalfSet {
    index: crate::support::hash::HashMap<Value, u32>,
    values: Vec<Value>,
    masks: Vec<u8>,
    count: usize,
}

impl _HalfSet {
    fn of(&mut self, one: Value) -> u32 {
        *self.index.entry(one).or_insert_with(|| {
            self.values.push(one);
            self.masks.push(0);
            u32::try_from(self.values.len() - 1).expect("value index fits u32")
        })
    }

    fn add(&mut self, one: u32, mask: u8) {
        let held = &mut self.masks[one as usize];
        let new = mask & !*held;
        *held |= new;
        self.count += new.count_ones() as usize;
    }

    fn has(&self, one: u32, half: u8) -> bool {
        self.masks[one as usize] & (1 << half) != 0
    }
}

/// Which half of which value something reads, to a fixed point.
///
/// Both halves of everything reaching an exit are live.
pub(crate) fn halves(body: &Rc<MirBody>) -> BTreeSet<(Value, u8)> {
    let key = Rc::as_ptr(body) as usize;
    let saved = _halves_reuse.with(|reuse| {
        reuse
            .borrow()
            .as_ref()
            .and_then(|reused| reused.get(&key))
            .filter(|saved| Rc::ptr_eq(&saved.0, body))
            .map(|saved| saved.1.clone())
    });
    if let Some(saved) = saved {
        if !crate::support::checking_caches() {
            return saved;
        }
        assert!(saved == _halved(body), "transform.halves: a cache hit disagrees with its recomputation");
        return saved;
    }
    let out = _halved(body);
    _halves_reuse.with(|reuse| {
        if let Some(reused) = reuse.borrow_mut().as_mut() {
            reused.insert(key, (Rc::clone(body), out.clone()));
        }
    });
    out
}

fn _halved(body: &MirBody) -> BTreeSet<(Value, u8)> {
    #[cfg(test)]
    HALVED.with(|halved| halved.set(halved.get() + 1));

    const BOTH: u8 = 1 << LOW | 1 << HIGH;
    let mut out = _HalfSet::default();
    for value in _leaving(body) {
        let one = out.of(value);
        out.add(one, BOTH);
    }

    // Each operation as (kept, its defines, its steps); each phi as its
    // result and incoming values. Ranges index `defines`, `steps`, `incoming`.
    let mut defines: Vec<u32> = Vec::new();
    let mut steps: Vec<_HalfStep> = Vec::new();
    let mut incoming: Vec<u32> = Vec::new();
    let mut blocks = Vec::with_capacity(body.blocks.len());
    let mut read: Vec<(Value, u32)> = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::with_capacity(block.ops.len());
        for op in &block.ops {
            let defined = defines.len()..defines.len() + op.defines.len();
            for one in &op.defines {
                let one = out.of(*one);
                defines.push(one);
            }
            // The widest each value is read at by this operation.
            read.clear();
            let mut note = |one: Value, width: u32| match read.iter_mut().find(|(seen, _)| *seen == one) {
                Some((_, widest)) => *widest = (*widest).max(width),
                None => read.push((one, width)),
            };
            for one in &op.args {
                if let Arg::Held(held) = one {
                    note(held.value, held.width);
                }
            }
            for reference in op.loads.iter().chain(&op.stores) {
                if let Some(base) = reference.base {
                    note(base, reference.base_width);
                }
            }
            let width = |one: &Value| read.iter().find(|(seen, _)| seen == one).map(|(_, widest)| *widest);
            let first = steps.len();
            let described = op.kind != Kind::Opaque && !op.barrier();
            for one in &op.uses {
                let index = out.of(*one);
                if let Some(into) = op.merges.get(one) {
                    // Read for the half it is merged into, and only if
                    // something reads that half of the result.
                    let into = out.of(*into);
                    steps.push(_HalfStep::Carry(into, index));
                    if width(one).is_none() {
                        continue;
                    }
                }
                // Where nothing written down says how much of it is read, both.
                let mask = match width(one).filter(|_| described) {
                    Some(widest) if widest < 4 => 1 << LOW,
                    _ => BOTH,
                };
                steps.push(_HalfStep::Add(index, mask));
            }
            for reference in op.loads.iter().chain(&op.stores) {
                for one in [reference.base, reference.segment].into_iter().flatten() {
                    let wide = Some(one) == reference.segment || reference.base_width >= 4;
                    let index = out.of(one);
                    steps.push(_HalfStep::Add(index, if wide { BOTH } else { 1 << LOW }));
                }
            }
            ops.push((_kept(op), defined, first..steps.len()));
        }
        let mut phis = Vec::with_capacity(block.phis.len());
        for phi in &block.phis {
            let result = out.of(phi.result);
            let first = incoming.len();
            for one in phi.incoming.values() {
                let one = out.of(*one);
                incoming.push(one);
            }
            phis.push((result, first..incoming.len()));
        }
        blocks.push((ops, phis));
    }

    let mut changing = true;
    while changing {
        let before = out.count;
        for (ops, phis) in &blocks {
            for (kept, defined, stepped) in ops {
                if !kept && !defines[defined.clone()].iter().any(|one| out.masks[*one as usize] != 0) {
                    continue;
                }
                for step in &steps[stepped.clone()] {
                    match *step {
                        _HalfStep::Add(one, mask) => out.add(one, mask),
                        _HalfStep::Carry(into, one) => {
                            if out.has(into, HIGH) {
                                out.add(one, 1 << HIGH);
                            }
                        }
                    }
                }
            }
            for (result, arms) in phis {
                for half in [LOW, HIGH] {
                    if out.has(*result, half) {
                        for one in &incoming[arms.clone()] {
                            out.add(*one, 1 << half);
                        }
                    }
                }
            }
        }
        changing = out.count != before;
    }
    let out: BTreeSet<(Value, u8)> = out
        .values
        .iter()
        .zip(&out.masks)
        .flat_map(|(one, mask)| {
            [LOW, HIGH].into_iter().filter(move |half| mask & (1 << half) != 0).map(move |half| (*one, half))
        })
        .collect();
    out
}

/// Values some half of which something reads.
///
/// Direct port of `qbopt/optimize/transform.py:live`.
pub(crate) fn live(body: &Rc<MirBody>) -> std::collections::BTreeSet<crate::model::mir::Value> {
    halves(body).into_iter().map(|(one, _)| one).collect()
}

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
    held: &crate::analysis::consts::HeldCells,
    nonnull: Option<&dyn Fn(Value) -> bool>,
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
        .map(|one| consts::_operand(compare, one, facts, held.get(&(block.at, index)).map(|here| &**here)))
        .collect::<Vec<_>>();
    if parts.iter().any(Option::is_none) {
        if !matches!(op.test, Some(Kind::Eq | Kind::Ne)) {
            return None;
        }
        let nonnull = nonnull?;
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
        if !nonnull(pointer) {
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
    held: &crate::analysis::consts::HeldCells,
    nonnull: Option<&dyn Fn(Value) -> bool>,
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
    if let Some(answer) = _outcome(block, last, facts, held, nonnull) {
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
pub(crate) fn _threaded(body: &Rc<MirBody>) -> Result<Rc<MirBody>, String> {
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
        blocks.push(MirBlock { succ: successors, ..block.with_ops(ops) });
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
    Ok(if changed { Rc::new(_unreachable(&body.with_blocks(converged))) } else { body.clone() })
}

/// A branch on two numbers, resolved.
///
/// Taken becomes an unconditional jump and not-taken becomes an inert owner.
pub(crate) fn decided(
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
) -> Result<Rc<MirBody>, String> {
    use crate::analysis::{alias, constant_cycles, consts, ranges};

    let body = _threaded(body)?;
    let facts = consts::known(&body, Some(dgroup), Some(calls), None, None);
    let held = consts::shared_cells(&body, dgroup, calls, Some(&facts), None, None, None, None);
    // Points-to only for a branch that compares a pointer with zero, as LLVM
    // asks isKnownNonZero of one value rather than solving every pointer.
    let pointers = std::cell::OnceCell::new();
    let defining = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect::<crate::support::hash::HashMap<_, _>>();
    let pointing = alias::may_point(&body);
    let nonnull = |value: Value| {
        pointing.contains(&value)
            && alias::nonnull_by_definition(&body, defining.get(&value).copied(), value).unwrap_or_else(|| {
            pointers.get_or_init(|| alias::pointers(&body)).as_ref().is_ok_and(|facts| facts.nonnull(value))
        })
    };
    let successors = |block: &MirBlock,
                      values: &IndexMap<Value, consts::Known>,
                      states: &IndexMap<Value, constant_cycles::State>| {
        _executable_successors(block, values, states, &held, Some(&nonnull))
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
            out.push(MirBlock { succ: vec![target], ..block.with_ops(ops) });
            changed = true;
            continue;
        }
        let mut answer = _outcome(block, last, &facts, &held, Some(&nonnull));
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
            out.push(MirBlock { succ: vec![target], ..block.with_ops(ops) });
        } else {
            let kept = _absorb(&block.ops, &BTreeSet::from([last.at]));
            if kept == block.ops {
                out.push(block.clone());
                continue;
            }
            out.push(MirBlock { succ: block.succ.iter().copied().filter(|at| *at != target).collect(), ..block.with_ops(kept) });
        }
    }
    if !changed {
        return Ok(body);
    }
    // Remove dead edges without losing the unreachable blocks' byte ownership.
    Ok(Rc::new(_trivial_phis(&_unreachable(&MirBody { blocks: out, ..MirBody::clone(&body) }))?))
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
                crate::model::mir::MirBlock { succ: Vec::new(), phis: Vec::new(), ..block.with_ops(block.ops.iter().map(_empty_operation).collect()) }
            }
        })
        .collect();
    body.with_blocks(kept)
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
            out.push(MirBlock { phis, ..block.with_ops(ops) });
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
pub(crate) fn dead(body: &Rc<MirBody>) -> Result<Rc<MirBody>, String> {
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
        out.push(block.with_ops(ops));
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
    Ok(Rc::new(body.with_blocks(out
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
            .collect())))
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
    body: &Rc<MirBody>,
    facts: &IndexMap<Value, crate::analysis::consts::Known>,
    wanted: &BTreeSet<Value>,
) -> Result<Rc<MirBody>, String> {
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
                let mut numbers: IndexMap<i64, num_bigint::BigInt> = IndexMap::default();
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
                    changed[&parent_at] = parent.with_ops(ops);
                }

                changed[&block.at].phis.push(Phi { result: target, incoming });
                changed[&operation_block.at].ops[index] = _empty_operation(op);
                return Ok(Rc::new(body.with_blocks(body.blocks.iter().map(|one| changed[&one.at].clone()).collect())));
            }
        }
    }
    Ok(body.clone())
}

/// An operation whose result is a number, replaced by that number.
pub(crate) fn folded(body: &Rc<MirBody>, dgroup: &BTreeSet<i64>, calls: &IndexMap<i64, String>) -> Result<Rc<MirBody>, String> {
    use crate::analysis::{consts, floatfacts};
    use crate::optimize::floatfold;

    let edges = floatfacts::exit_cells(body, dgroup, calls);
    let facts = consts::known(body, Some(dgroup), Some(calls), Some(&edges), None);
    let floating_facts = if body.blocks.iter().any(|block| block.ops.iter().any(|op| op.floating.is_some())) {
        floatfacts::known(body, dgroup, calls, None)
    } else {
        IndexMap::default()
    };
    let conversions = floatfacts::converted(body, dgroup, calls, Some(&floating_facts));
    let mut argument_facts = facts.clone();
    argument_facts.extend(conversions.iter().map(|(value, fact)| (*value, fact.clone())));
    let memory = if body
        .blocks
        .iter()
        .any(|block| block.ops.iter().any(|op| !op.loads.is_empty() || op.kind == Kind::Divmod))
    {
        consts::shared_cells(body, dgroup, calls, Some(&facts), None, Some(&edges), None, None)
    } else {
        IndexMap::default()
    };
    let symbols = _symbol_copies(body);
    if facts.is_empty() && memory.is_empty() && argument_facts.is_empty() && symbols.is_empty() {
        return Ok(body.clone());
    }

    // Live, not merely mentioned: see live()'s own note on hotlop's dx.
    let wanted = live(body);

    let nothing = consts::Cells::default();
    let mut out = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut ops = Vec::new();
        for (index, op) in block.ops.iter().enumerate() {
            let here = memory.get(&(block.at, index)).map(|here| &**here).unwrap_or(&nothing);
            if let Some(numbers) = consts::division(op, &facts, here) {
                let replacements = _folded_division(op, numbers, &wanted);
                changed |= replacements.as_slice() != std::slice::from_ref(op);
                ops.extend(replacements);
                continue;
            }
            let updated = _constant_update(op, &facts, here, &wanted);
            let made = _constant_operands(
                &_folded_op(&updated, &facts, &wanted),
                if op.kind == Kind::Arg { &argument_facts } else { &facts },
                Some(here),
                Some(&symbols),
            );
            let made = _constant_based(made, &facts);
            // Python's `made is not op`: every helper returns its input or a rewrite.
            changed = changed || made != *op;
            ops.push(made);
        }
        out.push(block.with_ops(ops));
    }

    let result = if changed { Rc::new(body.with_blocks(out)) } else { body.clone() };
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

/// A fill's value and count, where they are numbers: a held count was priced as unknown.
pub(crate) fn _constant_fill(op: &Op, facts: &IndexMap<Value, crate::analysis::consts::Known>) -> Op {
    use crate::analysis::consts;
    use crate::model::mir::Const;

    let known = |arg: &Arg| -> Arg {
        if let Arg::Held(held) = arg {
            if let Some(fact) = facts.get(&held.value).filter(|fact| fact.width >= held.width) {
                return Arg::Const(Const::new(consts::masked(&fact.n, held.width), held.width));
            }
        }
        arg.clone()
    };
    let args = op.args.iter().take(2).map(known).chain(op.args.iter().skip(2).cloned()).collect::<Vec<_>>();
    if args == op.args {
        return op.clone();
    }
    let kept = args
        .iter()
        .filter_map(|arg| if let Arg::Held(held) = arg { Some(held.value) } else { None })
        .chain(op.merges.keys().copied())
        .collect::<std::collections::BTreeSet<_>>();
    let mut result = op.clone();
    result.uses = op.uses.iter().copied().filter(|value| kept.contains(value)).collect();
    result.args = args;
    result.raised = None;
    result
}

/// A near cell reached through a proven constant, as the fixed cell it is.
///
/// Full unrolling leaves `L[k]` with `k` a number; kept based, each copy paid
/// a register load of `k` to address one fixed byte.
pub(crate) fn _constant_based(op: Op, facts: &IndexMap<Value, crate::analysis::consts::Known>) -> Op {
    use crate::objectfile::module::Space;
    use num_bigint::BigInt;
    use num_traits::ToPrimitive;

    let fixed = |r#ref: &mir::MemRef| -> mir::MemRef {
        let (Some(base), Some(addr)) = (r#ref.base, r#ref.addr) else {
            return r#ref.clone();
        };
        let fact = facts.get(&base);
        if r#ref.segment.is_some()
            || !matches!(addr.space, Space::Frame | Space::Segment)
            || r#ref.base_width != 2
            || r#ref.symbolic.is_some()
            || r#ref.allocation.is_some()
            || fact.is_none_or(|fact| fact.width < r#ref.base_width)
        {
            return r#ref.clone();
        }
        let fact = fact.expect("checked above");
        let mut disp = ((BigInt::from(addr.disp) + &fact.n) & BigInt::from(0xFFFF)).to_i64().expect("a word");
        if addr.space == Space::Frame {
            disp = (disp ^ 0x8000) - 0x8000;
        }
        mir::MemRef {
            addr: Some(crate::objectfile::module::Addr { disp, base: iced_x86::Register::None, ..addr }),
            base: None,
            ..r#ref.clone()
        }
    };

    let mut refs: Vec<(mir::MemRef, mir::MemRef)> = Vec::new();
    // A dict: an equal key keeps its first spelling and takes the last value.
    let mut add = |r#ref: &mir::MemRef| match refs.iter_mut().find(|(old, _)| old == r#ref) {
        Some(found) => found.1 = fixed(r#ref),
        None => refs.push((r#ref.clone(), fixed(r#ref))),
    };
    op.loads.iter().chain(&op.stores).for_each(&mut add);
    for one in op.args.iter().chain(&op.results) {
        if let Arg::Cell(cell) = one {
            add(&cell.r#ref);
        }
    }
    if refs.iter().all(|(old, new)| new == old) {
        return op;
    }
    let mapped = |r#ref: &mir::MemRef| refs.iter().find(|(old, _)| old == r#ref).expect("every ref is mapped").1.clone();
    let cell = |one: &Arg| match one {
        Arg::Cell(cell) => Arg::Cell(mir::Cell { r#ref: mapped(&cell.r#ref) }),
        other => other.clone(),
    };

    let mut kept: BTreeSet<Value> = op
        .args
        .iter()
        .chain(&op.results)
        .filter_map(|one| match one {
            Arg::Held(held) => Some(held.value),
            _ => None,
        })
        .collect();
    kept.extend(refs.iter().flat_map(|(_, new)| [new.base, new.segment]).flatten());
    let dropped: BTreeSet<Value> = refs
        .iter()
        .filter(|(old, new)| new != old)
        .filter_map(|(old, _)| old.base)
        .filter(|value| !kept.contains(value) && !op.merges.contains_key(value))
        .collect();
    Op {
        loads: op.loads.iter().map(mapped).collect(),
        stores: op.stores.iter().map(mapped).collect(),
        args: op.args.iter().map(cell).collect(),
        results: op.results.iter().map(cell).collect(),
        uses: op.uses.iter().copied().filter(|value| !dropped.contains(value)).collect(),
        source_backed: false,
        raised: None,
        ..op.clone()
    }
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

    let no_memory = consts::Cells::default();
    let no_symbols = _SymbolCopies::default();
    let memory = memory.unwrap_or(&no_memory);
    let symbols = symbols.unwrap_or(&no_symbols);
    if op.kind == Kind::Arg {
        return _constant_argument(op, facts, memory, Some(symbols));
    }
    if op.kind == Kind::Fill {
        return _constant_fill(op, facts);
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
    let mut instead: IndexMap<Value, Value> = IndexMap::default();
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
pub(crate) fn hoisted(
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    bounds: Option<&IndexMap<(crate::objectfile::module::Space, i64), Vec<i64>>>,
) -> Result<Rc<MirBody>, String> {
    use crate::support::hash::HashMap;

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
    let mut moved: IndexMap<i64, Vec<Op>> = IndexMap::default();
    let mut gone: BTreeSet<usize> = BTreeSet::new();
    let mut placing: IndexMap<i64, usize> = IndexMap::default();

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
        out.push(block.with_ops(ops));
    }

    // What crossed the loop edge is its own variable now, so re-deriving SSA
    // cannot join it to the counter that shared its register.
    let mut moved_out = MirBody::clone(body);
    moved_out.blocks = out;
    if !crossed.is_empty() {
        moved_out = _reparented(&moved_out, &crossed);
    }
    Ok(Rc::new(moved_out))
}

// Every transform, as the one thing a transform is. The functions above stay
// because they are what each class does and are what the tests name; what
// changes is that the pipeline can only reach them through `transform`.
pub(crate) struct Fold {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Fold {
    fn class_name(&self) -> &'static str {
        "Fold"
    }

    fn name(&self) -> &str {
        "fold"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        use crate::optimize::canonical;
        Ok(canonical::identities(canonical::compares(folded(&body, &self.r#where.dgroup, &self.r#where.named())?)))
    }
}

pub(crate) struct Decide {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Decide {
    fn class_name(&self) -> &'static str {
        "Decide"
    }

    fn name(&self) -> &str {
        "decide"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        crate::optimize::cfg::merged(&decided(&body, &self.r#where.dgroup, &self.r#where.named())?)
            .map_err(|error| error.to_string())
    }
}

pub(crate) struct Dead;

impl crate::model::passes::MIRTransform for Dead {
    fn class_name(&self) -> &'static str {
        "Dead"
    }

    fn name(&self) -> &str {
        "dead"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        dead(&body)
    }
}

/// Specialize a proven strict-FP recurrence before LICM changes its shape.
pub(crate) struct FloatLoop {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for FloatLoop {
    fn class_name(&self) -> &'static str {
        "FloatLoop"
    }

    fn name(&self) -> &str {
        "floatloop"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        crate::optimize::floatloop::specialized(&body, &self.r#where.dgroup, &self.r#where.named())
    }
}

pub(crate) struct Hoist {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Hoist {
    fn class_name(&self) -> &'static str {
        "Hoist"
    }

    fn name(&self) -> &str {
        "hoist"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        let body = hoisted(&body, &self.r#where.dgroup, &self.r#where.named(), self.r#where.bounds.as_ref())?;
        crate::optimize::loopmotion::sunk_stores(
            &body,
            &self.r#where.dgroup,
            self.r#where.bounds.as_ref(),
            _handles_errors(&self.r#where),
        )
    }
}

pub(crate) struct DropStores {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for DropStores {
    fn class_name(&self) -> &'static str {
        "DropStores"
    }

    fn name(&self) -> &str {
        "drop_stores"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        let private =
            crate::analysis::observers::private(&body, self.r#where.found.as_ref(), self.r#where.blocks.as_ref())?;
        without_dead_stores(
            &body,
            &self.r#where.dgroup,
            &self.r#where.named(),
            private.as_deref(),
            self.r#where.bounds.as_ref(),
            _handles_errors(&self.r#where),
        )
    }
}

pub(crate) fn _handles_errors(r#where: &crate::model::passes::Where) -> bool {
    use crate::abi::runtime;

    let contracts = r#where.named().values().map(|name| runtime::contract(Some(name))).collect::<Vec<_>>();
    runtime::handles_errors(&contracts)
}

/// The single value-reuse pass: scalar GVN and memory-aware PRE.
pub(crate) struct Gvn {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Gvn {
    fn class_name(&self) -> &'static str {
        "Gvn"
    }

    fn name(&self) -> &str {
        "gvn"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        crate::optimize::gvn::optimized(&body, &self.r#where)
    }
}

pub(crate) struct Place {
    pub r#where: crate::model::passes::Where,
}

impl crate::model::passes::MIRTransform for Place {
    fn class_name(&self) -> &'static str {
        "Place"
    }

    fn name(&self) -> &str {
        "place"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        placed(&body, &self.r#where.dgroup, &self.r#where.named())
    }
}

pub(crate) struct Algebraic;

impl crate::model::passes::MIRTransform for Algebraic {
    fn class_name(&self) -> &'static str {
        "Algebraic"
    }

    fn name(&self) -> &str {
        "algebraic"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        let demanded = halves(&body);
        crate::optimize::algebraic::simplified(
            &body,
            &demanded.iter().map(|(value, _)| *value).collect(),
            &demanded.iter().filter(|(_, part)| *part == HIGH).map(|(value, _)| *value).collect(),
        )
        .map(Rc::new)
    }
}

/// Expose packed far addresses as independently allocatable MIR values.
pub(crate) struct SplitPointers;

impl crate::model::passes::MIRTransform for SplitPointers {
    fn class_name(&self) -> &'static str {
        "SplitPointers"
    }

    fn name(&self) -> &str {
        "split_pointers"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        Ok(crate::optimize::pointeraccess::split(body))
    }
}

/// Canonicalize indirect objects from current SSA pointer facts.
pub(crate) struct PointerProvenance;

impl crate::model::passes::MIRTransform for PointerProvenance {
    fn class_name(&self) -> &'static str {
        "PointerProvenance"
    }

    fn name(&self) -> &str {
        "provenance"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        crate::analysis::alias::annotated(&body).map(Rc::new)
    }
}

/// The passes, in order, that `wanted` leaves on.
///
/// A pass that is off is not in it, rather than in it and skipped, so what
/// runs is what this returns.
pub(crate) fn pipeline(
    r#where: &crate::model::passes::Where,
    wanted: &IndexMap<&str, bool>,
) -> Vec<Box<dyn crate::model::passes::MIRTransform>> {
    use crate::optimize::{fill, lcssa, loopsimplify, peel, promote, strength, unroll};

    let every: Vec<Box<dyn crate::model::passes::MIRTransform>> = vec![
        // Pointer identity is a solved program fact: resolve it before a
        // packed pointer becomes independent offset and selector values.
        Box::new(PointerProvenance),
        Box::new(SplitPointers),
        // Aggregate/object leaves become ordinary SSA before any scalar or
        // CFG pass asks what is constant, redundant, or loop invariant.
        Box::new(promote::Sroa::new(r#where.clone())),
        Box::new(Fold { r#where: r#where.clone() }),
        Box::new(Decide { r#where: r#where.clone() }),
        Box::new(loopsimplify::LoopSimplify),
        Box::new(lcssa::LoopClosedSSA),
        // Strict floating recurrences must retain their original iteration
        // order; LICM may move invariant x87 preparation out afterwards.
        Box::new(FloatLoop { r#where: r#where.clone() }),
        Box::new(Hoist { r#where: r#where.clone() }),
        Box::new(DropStores { r#where: r#where.clone() }),
        Box::new(Gvn { r#where: r#where.clone() }),
        // Ordinary scalar write-through promotion remains after memory GVN.
        Box::new(promote::Promote::new(r#where.clone())),
        Box::new(strength::Strength::new(r#where.clone())),
        Box::new(Algebraic),
        Box::new(Dead),
        Box::new(Place { r#where: r#where.clone() }),
        Box::new(unroll::Unroll::new(r#where.clone())),
        Box::new(peel::Peel::new(r#where.clone())),
        Box::new(fill::Fill),
    ];
    every.into_iter().filter(|one| wanted.get(one.name()).copied().unwrap_or(true)).collect()
}

// The order, from the pipeline itself rather than beside it.
pub(crate) static PASSES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    pipeline(&crate::model::passes::Where::default(), &IndexMap::default())
        .iter()
        .map(|one| one.name().to_owned())
        .collect()
});
// What runs with no caller asking otherwise: a pass may exist, be correct,
// and be off because it costs.
#[allow(dead_code)]
pub(crate) static PASSES_ON: std::sync::LazyLock<Vec<String>> =
    std::sync::LazyLock::new(|| PASSES.iter().filter(|one| *one != "strength").cloned().collect());

/// `applied`'s keyword arguments, with Python's defaults.
pub(crate) struct Applied<'a> {
    pub blocks: Option<Rc<Vec<crate::frontends::bc::blocks::Block>>>,
    pub found: Option<Rc<crate::objectfile::module::Module>>,
    pub options: crate::model::passes::Options,
    pub only: Option<String>,
    pub registers: Option<i64>,
    pub call_registers: i64,
    pub index_scales: Option<BTreeSet<i64>>,
    pub address_forms: Option<Vec<crate::model::passes::AddressForm>>,
    pub costs: Option<crate::model::passes::OperationCosts>,
    pub watch: Option<&'a mut dyn FnMut(&str, &MirBody)>,
}

impl Default for Applied<'_> {
    fn default() -> Self {
        Self {
            blocks: None,
            found: None,
            options: crate::model::passes::O2(),
            only: None,
            registers: None,
            call_registers: 0,
            index_scales: None,
            address_forms: None,
            costs: None,
            watch: None,
        }
    }
}

/// Every transform this module has, or the one `only` names.
pub(crate) fn applied(
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    options: Applied<'_>,
) -> Result<Rc<MirBody>, String> {
    crate::analysis::consts::reusing(|| crate::analysis::manager::scoped(|| _reusing_halves(|| _applied(body, dgroup, calls, options))))
}

/// The closure state `applied`'s nested `scalarized`, `fixed` and
/// share; they recurse through unroll.
struct _Transaction<'w, 'a> {
    r#where: &'w crate::model::passes::Where,
    boundary: std::cell::RefCell<Vec<Box<dyn crate::model::passes::MIRTransform>>>,
    passes: std::cell::RefCell<Vec<Box<dyn crate::model::passes::MIRTransform>>>,
    unrollers: std::cell::RefCell<Vec<Box<dyn crate::model::passes::MIRTransform>>>,
    only: bool,
    watch: std::cell::RefCell<Option<&'a mut dyn FnMut(&str, &MirBody)>>,
}

impl _Transaction<'_, '_> {
    fn watching(&self) -> bool {
        self.watch.borrow().is_some()
    }

    fn watch(&self, stage: &str, state: &MirBody) {
        if let Some(watch) = self.watch.borrow_mut().as_mut() {
            watch(stage, state);
        }
    }

    fn scalarized(&self, state: Rc<MirBody>, stage: &str) -> Result<Rc<MirBody>, String> {
        let mut state = state;
        let mut boundary = self.boundary.borrow_mut();
        for one in boundary.iter_mut() {
            let name = one.name().to_owned();
            state = crate::support::debug::timed(&name, || one.transform(state))?;
            self.watch(&format!("{stage}-{}", one.name()), &state);
        }
        Ok(state)
    }

    fn fixed(&self, state: Rc<MirBody>, consider_unroll: bool, prefix: &str) -> Result<Rc<MirBody>, String> {
        // A monotone chain may expose one simplification per operation.
        // Scale with the body and separately reject a repeated state, so an
        // oscillator fails immediately instead of consuming that allowance.
        let size = state.blocks.iter().map(|block| 1 + block.phis.len() + block.ops.len()).sum::<usize>();
        let limit = std::cmp::max(16, size + 1);
        let mut state = state;
        let mut history = vec![Rc::clone(&state)];
        crate::debug!("fixed", "{prefix}start: {size} ops, at most {limit} rounds");
        // The body each pass last left unchanged. Passes are pure, so a pass
        // handed that same body again would change nothing: a round after the
        // last change skips every pass that already saw it.
        let mut settled: Vec<Option<Rc<MirBody>>> = vec![None; self.passes.borrow().len()];
        let mut unroll_settled: Option<Rc<MirBody>> = None;
        let mut holding = !self.only && self.passes.borrow().iter().any(|one| one.after_settling());
        for iteration in 0..limit {
            let before = Rc::clone(&state);
            let started = std::time::Instant::now();
            let mut changed = Vec::new();
            {
                let mut passes = self.passes.borrow_mut();
                for (one, settled) in passes.iter_mut().zip(&mut settled) {
                    if holding && one.after_settling() {
                        continue;
                    }
                    if !settled.as_ref().is_some_and(|body| Rc::ptr_eq(body, &state)) {
                        let name = one.name().to_owned();
                        let input = Rc::clone(&state);
                        state = crate::support::debug::timed(&name, || one.transform(state))?;
                        if Rc::ptr_eq(&input, &state) || *input == *state {
                            state = Rc::clone(&input);
                            *settled = Some(input);
                        } else {
                            *settled = None;
                            changed.push(name);
                        }
                    }
                    self.watch(&format!("{prefix}r{:02}-{}", iteration + 1, one.name()), &state);
                }
            }
            // Ask at the original pipeline boundary: fully converging the
            // scalar passes first destroys matmul's exact counted-loop shape.
            if consider_unroll && !self.unrollers.borrow().is_empty() && !unroll_settled.as_ref().is_some_and(|body| Rc::ptr_eq(body, &state)) {
                let mut watch = |stage: &str, candidate: &MirBody| self.watch(&format!("{prefix}{stage}"), candidate);
                let watching = self.watching();
                let unrolled = crate::support::debug::timed("unroll", || {
                    crate::optimize::unroll::optimized(&state, self.r#where, if watching { Some(&mut watch) } else { None })
                })?;
                // A copy's constant indices are new exact leaves, so it crosses the
                // structural boundary before the scalar passes settle it.
                if Rc::ptr_eq(&unrolled, &state) {
                    unroll_settled = Some(Rc::clone(&state));
                } else {
                    changed.push("unroll".to_owned());
                    state = self.scalarized(unrolled, &format!("{prefix}unrolled"))?;
                }
            }
            crate::debug!(
                "fixed",
                "{prefix}round {}: {} ops, {:.0} ms, changed by {}",
                iteration + 1,
                state.blocks.iter().map(|block| 1 + block.phis.len() + block.ops.len()).sum::<usize>(),
                started.elapsed().as_secs_f64() * 1e3,
                if changed.is_empty() { "nothing".to_owned() } else { changed.join(" ") }
            );
            if holding && state == before {
                holding = false;
                continue;
            }
            if self.only || state == before {
                // A structural candidate can make its last cloned region
                // unreachable on the same round that reaches the scalar fixed
                // point, so normalize the public boundary itself.
                return Ok(Rc::new(_unreachable(&state)));
            }
            if history.iter().any(|previous| state == *previous) {
                return Err(format!("MIR optimization did not converge: cycle after {} rounds", iteration + 1));
            }
            history.push(Rc::clone(&state));
        }
        Err(format!("MIR optimization did not converge after {limit} size-scaled rounds"))
    }
}

fn _applied(
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    options: Applied<'_>,
) -> Result<Rc<MirBody>, String> {
    let Applied {
        blocks,
        found,
        options,
        only,
        registers,
        call_registers,
        index_scales,
        address_forms,
        costs,
        watch,
    } = options;
    // Every pass can be turned off, which is how a miscompile is bisected.
    let wanted = IndexMap::from_iter([
        ("lcssa", options.lcssa),
        ("floatloop", options.floatloop),
        ("fold", options.fold),
        ("decide", options.decide),
        ("dead", options.dead),
        ("hoist", options.hoist),
        ("gvn", options.forward && options.drop_loads),
        ("drop_stores", options.drop_stores),
        ("sroa", options.promote),
        ("promote", options.promote),
        ("strength", options.strength),
        ("unroll", options.unroll),
        ("peel", options.peel),
        ("fill", options.fill),
    ]);
    let r#where = crate::model::passes::Where {
        dgroup: dgroup.clone(),
        calls: Some(calls.clone()),
        bounds: found.as_deref().map(crate::objectfile::module::landmarks),
        blocks,
        found,
        registers: registers.unwrap_or(mir::TRACKED.len() as i64),
        call_registers,
        // Existing direct MIR callers retain native medium-model addressing.
        index_scales: index_scales.unwrap_or_else(|| BTreeSet::from([1])),
        address_forms: address_forms.unwrap_or_default(),
        costs: costs.unwrap_or_default(),
        options: options.clone(),
    };
    // Public debugging selectors from before value reuse became one pass.
    let only = match only.as_deref() {
        Some("forward" | "drop_loads" | "reuse" | "cse") => Some("gvn".to_owned()),
        _ => only,
    };
    let passes = pipeline(&r#where, &wanted)
        .into_iter()
        .filter(|one| only.as_deref().is_none_or(|only| one.name() == only))
        .collect::<Vec<_>>();
    // Pointer decomposition and SROA establish the scalar memory shape at
    // structural boundaries; they are not members of the scalar fixed point.
    let structural = ["PointerProvenance", "SplitPointers", "Sroa"];
    let (boundary, passes): (Vec<_>, Vec<_>) =
        passes.into_iter().partition(|one| structural.contains(&one.class_name()));
    let (mut unrollers, passes): (Vec<_>, Vec<_>) =
        passes.into_iter().partition(|one| one.class_name() == "Unroll");
    let (mut peelers, passes): (Vec<_>, Vec<_>) = passes.into_iter().partition(|one| one.class_name() == "Peel");

    let has_unrollers = !unrollers.is_empty();
    let has_boundary = !boundary.is_empty();
    let transaction = _Transaction {
        r#where: &r#where,
        boundary: std::cell::RefCell::new(boundary),
        passes: std::cell::RefCell::new(passes),
        unrollers: std::cell::RefCell::new(Vec::new()),
        only: only.is_some(),
        watch: std::cell::RefCell::new(watch),
    };

    let mut body = transaction.scalarized(Rc::clone(body), "r01")?;
    if only.is_some() && has_boundary {
        return Ok(Rc::new(_unreachable(&body)));
    }
    if only.is_some() && has_unrollers {
        body = unrollers[0].transform(body)?;
        transaction.watch("r01-unroll", &body);
        return Ok(Rc::new(_unreachable(&body)));
    }
    if only.is_some() && !peelers.is_empty() {
        body = peelers[0].transform(body)?;
        transaction.watch("r01-peel", &body);
        return Ok(Rc::new(_unreachable(&body)));
    }
    *transaction.unrollers.borrow_mut() = std::mem::take(&mut unrollers);

    body = transaction.fixed(body, has_unrollers, "")?;
    if !peelers.is_empty() {
        let mut watch = |stage: &str, candidate: &MirBody| transaction.watch(stage, candidate);
        let watching = transaction.watching();
        let peeled = crate::support::debug::timed("peel", || crate::optimize::peel::optimized(
            &body,
            &r#where,
            if watching { Some(&mut watch) } else { None },
        ))?;
        if !Rc::ptr_eq(&peeled, &body) {
            body = transaction.fixed(transaction.scalarized(peeled, "peeled")?, has_unrollers, "peeled-")?;
        }
    }
    if options.unswitch {
        let mut watch = |stage: &str, candidate: &MirBody| transaction.watch(stage, candidate);
        let watching = transaction.watching();
        body = crate::optimize::unswitch::optimized(
            &body,
            dgroup,
            calls,
            crate::optimize::unswitch::Optimized {
                registers: Some(r#where.registers),
                call_registers: r#where.call_registers,
                index_scales: Some(r#where.index_scales.clone()),
                address_forms: Some(r#where.address_forms.clone()),
                costs: Some(r#where.costs.clone()),
                options: options.clone(),
                watch: if watching { Some(&mut watch) } else { None },
            },
        )?;
    }
    Ok(Rc::new(_unreachable(&body)))
}
#[cfg(test)]
#[path = "transform_tests.rs"]
mod transform_tests;
