//! Selective whole-module MIR inlining.
//!
//! Inlining is a CFG operation, not a call peephole: split the caller at the
//! call, clone the callee's blocks, bind formal parameter loads to the actual
//! SSA values, and join every return back to the continuation.  The ordinary
//! body pipeline then simplifies the result.
//!
//! The initial policy covered leaf procedures called once.  It also admits a
//! straight-line private leaf at every direct call site when the target-priced
//! call work exceeds the semantic work duplicated by cloning.  In both cases
//! the ordinary body pipeline simplifies the result; MIR chooses from semantic
//! costs and never sees opcodes or registers.
//!
//! Direct port of `qbopt/optimize/inline.py`.


use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;

use crate::analysis::ssa;
use crate::model::mir::{self, Arg, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};
use crate::optimize::edges;
use crate::support::pyrepr;

/// Python's `Counter[str]`: read with a 0 default.
pub(crate) type Counter = IndexMap<String, i64>;

const _FORBIDDEN: [Kind; 20] = [
    Kind::Call,
    Kind::Arg,
    Kind::Escape,
    Kind::Opaque,
    Kind::Fill,
    Kind::Div,
    Kind::Rem,
    Kind::Divmod,
    Kind::Udivmod,
    Kind::Fadd,
    Kind::Fsub,
    Kind::Fmul,
    Kind::Fdiv,
    Kind::Fneg,
    Kind::Fabs,
    Kind::Fsqrt,
    Kind::Fload,
    Kind::Fstore,
    Kind::Fcompare,
    Kind::Fcheck,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Candidate {
    pub body: MirBody,
    pub parameters: Vec<MemRef>,
}

/// Python's `getattr(arg, "width", 0)`: a cell or opaque has no width.
macro_rules! width_of {
    ($arg:expr) => {
        match $arg {
            Arg::Held(one) => one.width,
            Arg::Const(one) => one.width,
            Arg::Symbol(one) => one.width,
            Arg::FrameAddress(one) => one.width,
            Arg::FrameSelector(one) => one.width,
            Arg::Cell(_) | Arg::Opaque(_) => 0,
        }
    };
}

/// Private pure leaves worth moving into their direct callers.
///
/// A single-use body disappears after expansion, so it only has to fit the
/// normal CFG budget.  A repeated body duplicates its semantic work once per
/// additional caller.  Admit that only for a straight-line leaf, and only
/// when the profile's total direct-call cost is greater than the duplicate
/// work.  This lets a short arithmetic helper disappear at every site while
/// keeping branchy or code-growing helpers out of the allocator's region.
pub(crate) fn candidates(
    bodies: &IndexMap<String, MirBody>,
    parameters: &IndexMap<String, Vec<MemRef>>,
    calls: &Counter,
    private: &BTreeSet<String>,
    pure: &BTreeSet<String>,
    call_cost: i64,
) -> IndexMap<String, Candidate> {
    let budget = 6.max(24.min(call_cost.div_euclid(2)));
    let mut out = IndexMap::new();
    for name in private.intersection(pure) {
        let body = &bodies[name];
        let parms = &parameters[name];
        let semantic = body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|op| ![Kind::Nothing, Kind::Jump, Kind::Return].contains(&op.kind))
            .count() as i64;
        let count = calls.get(name).copied().unwrap_or(0);
        let repeated = semantic * (count - 1);
        let profitable = count == 1 || (_straight(body) && repeated < count * call_cost);
        if count != 0 && semantic <= budget && profitable && _leaf(body, parms) {
            out.insert(name.clone(), Candidate { body: body.clone(), parameters: parms.clone() });
        }
    }
    out
}

/// Private pure leaves worth cloning at one constant direct-call site.
///
/// Whole-body parameter specialization needs every caller to agree.  This
/// narrower policy instead admits a call whose known actual exposes local
/// SCCP after the normal MIR clone.  The original body remains for dynamic
/// callers, so no source-level calling convention or OMF symbol changes.
/// As with repeated-leaf inlining, the target profile must price the call
/// above the cloned semantic work.
pub(crate) fn constant_sites(
    bodies: &IndexMap<String, MirBody>,
    parameters: &IndexMap<String, Vec<MemRef>>,
    calls: &IndexMap<i64, String>,
    constants: &IndexMap<i64, Vec<Option<Const>>>,
    private: &BTreeSet<String>,
    pure: &BTreeSet<String>,
    call_cost: i64,
) -> IndexMap<i64, Candidate> {
    let budget = 6.max(24.min(call_cost.div_euclid(2)));
    let mut out = IndexMap::new();
    for (at, name) in calls {
        let known = constants.get(at).map_or(&[][..], Vec::as_slice);
        if !known.iter().any(Option::is_some) {
            continue;
        }
        if !private.contains(name) || !pure.contains(name) || !bodies.contains_key(name) {
            continue;
        }
        let (body, parms) = (&bodies[name], &parameters[name]);
        let semantic = body
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .filter(|op| ![Kind::Nothing, Kind::Jump, Kind::Return].contains(&op.kind))
            .count() as i64;
        if semantic < call_cost && semantic <= budget && _leaf(body, parms) {
            out.insert(*at, Candidate { body: body.clone(), parameters: parms.clone() });
        }
    }
    out
}

/// Whether cloning the body duplicates no control-flow structure.
fn _straight(body: &MirBody) -> bool {
    body.blocks.len() == 1 && body.blocks[0].phis.is_empty() && body.blocks[0].succ.is_empty()
}

fn _parameter(op: &Op, parameters: &[MemRef]) -> Option<usize> {
    if op.kind != Kind::Load
        || op.loads.len() != 1
        || op.results.len() != 1
        || !matches!(op.results[0], Arg::Held(_))
    {
        return None;
    }
    let found = parameters
        .iter()
        .enumerate()
        .filter(|(_, reference)| mir::same_bytes(&op.loads[0], reference))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if found.len() == 1 { Some(found[0]) } else { None }
}

/// Whether a pure body's remaining memory is only its formal values.
fn _leaf(body: &MirBody, parameters: &[MemRef]) -> bool {
    if !body.sealed || body.blocks.is_empty() {
        return false;
    }
    let mut returned = Vec::new();
    for block in &body.blocks {
        for op in &block.ops {
            if op.barrier() || _FORBIDDEN.contains(&op.kind) || !op.stores.is_empty() || op.array.is_some() {
                return false;
            }
            if !op.loads.is_empty() && _parameter(op, parameters).is_none() {
                return false;
            }
            if op.kind == Kind::Return {
                returned.push(op.args.iter().map(|arg| width_of!(arg)).collect::<Vec<_>>());
            }
        }
    }
    !returned.is_empty()
        && returned.iter().collect::<BTreeSet<_>>().len() == 1
        && returned[0].iter().all(|width| *width != 0)
}

/// Surviving direct call counts, never stale entries in a source side table.
pub(crate) fn call_counts(
    bodies: &IndexMap<String, MirBody>,
    calls: &IndexMap<String, IndexMap<i64, String>>,
) -> Counter {
    let mut counts = Counter::new();
    for (name, body) in bodies {
        let named = &calls[name];
        for op in body.blocks.iter().flat_map(|block| &block.ops) {
            if op.kind == Kind::Call && named.contains_key(&op.at) {
                *counts.entry(named[&op.at].clone()).or_insert(0) += 1;
            }
        }
    }
    counts
}

/// Inline the first legal call site in `body`, or return it unchanged.
pub(crate) fn expanded(
    body: &MirBody,
    calls: &IndexMap<i64, String>,
    arguments: &IndexMap<i64, BTreeSet<i64>>,
    available: &IndexMap<String, Candidate>,
    constant: Option<&IndexMap<i64, Candidate>>,
) -> Result<MirBody, String> {
    let empty = IndexMap::new();
    let constant = constant.unwrap_or(&empty);
    let mut used = BTreeSet::new();
    for block in &body.blocks {
        used.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
        used.extend(block.ops.iter().flat_map(mir::consumed));
    }
    for block in &body.blocks {
        for (index, call) in block.ops.iter().enumerate() {
            let candidate = constant
                .get(&call.at)
                .or_else(|| available.get(calls.get(&call.at).map_or("", String::as_str)));
            let Some(candidate) = candidate else {
                continue;
            };
            if call.kind != Kind::Call || !arguments.contains_key(&call.at) {
                continue;
            }
            let made = _at(body, block, index, call, &arguments[&call.at], candidate, &used)?;
            if let Some(made) = made {
                let problems = mir::verify(&made);
                if !problems.is_empty() {
                    return Err(format!(
                        "MIR inlining broke SSA: {}",
                        pyrepr::list(&problems[..problems.len().min(3)])
                    ));
                }
                return Ok(made);
            }
        }
    }
    Ok(body.clone())
}

/// Python's nested `fresh`, whose `nonlocal` counters are passed explicitly.
fn fresh(
    next_id: &mut u32,
    next_variable: &mut u32,
    versions: &mut BTreeMap<u32, u32>,
    at: i64,
    flags: bool,
    variable: Option<u32>,
) -> Value {
    let variable = match variable {
        Some(variable) => variable,
        None => {
            let variable = *next_variable;
            *next_variable += 1;
            variable
        }
    };
    let version = versions.entry(variable).or_insert(0);
    *version += 1;
    let value = Value { id: *next_id, at, flags, variable, version: *version };
    *next_id += 1;
    value
}

#[allow(clippy::too_many_arguments)]
fn _at(
    body: &MirBody,
    caller: &MirBlock,
    call_index: usize,
    call: &Op,
    argument_sites: &BTreeSet<i64>,
    candidate: &Candidate,
    used: &BTreeSet<Value>,
) -> Result<Option<MirBody>, String> {
    let (callee, parameters) = (&candidate.body, &candidate.parameters);
    let selected = caller.ops[..call_index]
        .iter()
        .enumerate()
        .filter(|(_, op)| op.kind == Kind::Arg && argument_sites.contains(&op.at))
        .collect::<Vec<_>>();
    // cdecl pushes the last source argument first.
    let actuals = selected
        .iter()
        .rev()
        .filter(|(_, op)| op.args.len() == 1)
        .map(|(_, op)| op.args[0].clone())
        .collect::<Vec<_>>();
    if selected.len() != parameters.len() || actuals.len() != parameters.len() {
        return Ok(None);
    }

    let returns = callee
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind == Kind::Return)
        .collect::<Vec<_>>();
    if returns.is_empty() || returns.iter().map(|op| op.args.len()).collect::<BTreeSet<_>>().len() != 1 {
        return Ok(None);
    }
    let arity = returns[0].args.len();
    let results = call
        .results
        .iter()
        .filter_map(|result| match result {
            Arg::Held(result) => Some(*result),
            _ => None,
        })
        .collect::<Vec<_>>();
    if arity > results.len() {
        return Ok(None);
    }
    if results[arity..].iter().any(|result| used.contains(&result.value)) {
        return Ok(None);
    }
    if returns.iter().any(|returned| {
        returned.args.iter().zip(&results[..arity]).any(|(arg, result)| match arg {
            Arg::Held(arg) => arg.width != result.width,
            _ => true,
        })
    }) {
        return Ok(None);
    }
    let semantic = results[..arity].iter().map(|result| result.value).collect::<BTreeSet<_>>();
    if call.defines.iter().any(|value| used.contains(value) && !semantic.contains(value)) {
        return Ok(None);
    }

    let values = ssa::values(body).collect::<Vec<_>>();
    let callee_values = ssa::values(callee).collect::<Vec<_>>();
    // The two independently raised bodies both number values from one.  A
    // materialized actual must consequently be fresh against the callee's
    // *source* IDs too: substitution keys are IDs, and colliding with one
    // would make a constant binding look like an unrelated cloned definition.
    let mut next_id = values.iter().chain(&callee_values).map(|value| value.id).max().unwrap_or(0) + 1;
    let mut next_variable = values.iter().chain(&callee_values).map(|value| value.variable).max().unwrap_or(0) + 1;
    let mut versions = BTreeMap::<u32, u32>::new();
    for value in &values {
        let version = versions.entry(value.variable).or_insert(0);
        *version = (*version).max(value.version);
    }

    let selected_indices = selected.iter().map(|(index, _)| *index).collect::<BTreeSet<_>>();
    let pre_ops = caller.ops[..call_index]
        .iter()
        .enumerate()
        .filter(|(index, _)| !selected_indices.contains(index))
        .map(|(_, op)| op.clone())
        .collect::<Vec<_>>();
    let mut materialized = Vec::new();
    let mut parameter_values = BTreeMap::<usize, Value>::new();
    let mut parameter_widths = BTreeMap::<usize, u32>::new();
    let callee_ids = callee_values.iter().map(|value| value.id).collect::<BTreeSet<_>>();
    for callee_block in &callee.blocks {
        for op in &callee_block.ops {
            if let Some(number) = _parameter(op, parameters) {
                parameter_widths.insert(number, width_of!(&op.results[0]));
            }
        }
    }
    if (0..parameters.len()).any(|number| !parameter_widths.contains_key(&number)) {
        // An unused formal needs no binding, but its argument setup is still removable.
        for number in 0..parameters.len() {
            if !parameter_widths.contains_key(&number) {
                parameter_widths.insert(number, parameters[number].width);
            }
        }
    }

    let widths = (0..parameters.len()).map(|number| parameter_widths[&number]).collect::<Vec<_>>();
    for (number, (actual, width)) in actuals.iter().zip(widths).enumerate() {
        let actual_width = width_of!(actual);
        if actual_width < width || matches!(actual, Arg::Cell(_) | Arg::Opaque(_)) {
            return Ok(None);
        }
        if let Arg::Held(held) = actual {
            if !callee_ids.contains(&held.value.id) {
                parameter_values.insert(number, held.value);
                continue;
            }
        }
        let actual = match actual {
            Arg::Const(constant) if constant.width != width => Arg::Const(Const::new(constant.n.clone(), width)),
            _ => actual.clone(),
        };
        let value = fresh(&mut next_id, &mut next_variable, &mut versions, call.at, false, None);
        parameter_values.insert(number, value);
        materialized.push(_copy(call.at, &actual, Held { value, width }, value));
    }

    let mut variable_map = BTreeMap::<u32, u32>::new();
    let mut swap = BTreeMap::<u32, Value>::new();
    for callee_block in &callee.blocks {
        for op in &callee_block.ops {
            if let Some(number) = _parameter(op, parameters) {
                let Arg::Held(result) = &op.results[0] else { unreachable!() };
                swap.insert(result.value.id, parameter_values[&number]);
            }
        }
    }
    let formal_values = swap.keys().copied().collect::<BTreeSet<_>>();
    for value in &callee_values {
        if swap.contains_key(&value.id) {
            continue;
        }
        let variable = *variable_map.entry(value.variable).or_insert(next_variable);
        if variable == next_variable {
            next_variable += 1;
        }
        let made = fresh(&mut next_id, &mut next_variable, &mut versions, call.at, value.flags, Some(variable));
        swap.insert(value.id, made);
    }

    let first_label = edges::fresh(body);
    let labels = callee
        .blocks
        .iter()
        .enumerate()
        .map(|(number, block)| (block.at, first_label + number as i64))
        .collect::<BTreeMap<_, _>>();
    let continuation = first_label + callee.blocks.len() as i64;
    let mut return_edges = Vec::<(i64, Vec<Held>)>::new();
    let mut cloned = Vec::new();
    for callee_block in &callee.blocks {
        let phis = callee_block
            .phis
            .iter()
            .map(|phi| Phi {
                result: swap[&phi.result.id],
                incoming: phi
                    .incoming
                    .iter()
                    .map(|(source, value)| (labels[source], swap[&value.id]))
                    .collect::<OrderedMap<_, _>>(),
            })
            .collect::<Vec<_>>();
        let mut ops = Vec::new();
        let mut returned = None;
        for op in &callee_block.ops {
            if _parameter(op, parameters).is_some() {
                continue;
            }
            let read = ssa::substituted(op, &swap).map_err(|error| error.to_string())?;
            if op.kind == Kind::Return {
                let mut held = Vec::new();
                for arg in &read.args {
                    let Arg::Held(arg) = arg else {
                        return Ok(None);
                    };
                    held.push(*arg);
                }
                returned = Some(held);
                continue;
            }
            let mut made = read.clone();
            made.at = call.at;
            made.defines = op.defines.iter().map(|value| swap[&value.id]).collect();
            made.results = read
                .results
                .iter()
                .map(|result| match result {
                    Arg::Held(result) => Arg::Held(Held { value: swap[&result.value.id], width: result.width }),
                    _ => result.clone(),
                })
                .collect();
            made.source_backed = false;
            made.id = None;
            made.raised = None;
            made.absorbed = Vec::new();
            made.symbol = Some(false);
            made.target = op.target.map(|target| labels.get(&target).copied().unwrap_or(target));
            made.cases = op
                .cases
                .iter()
                .map(|(number, target)| (*number, labels.get(target).copied().unwrap_or(*target)))
                .collect();
            ops.push(made);
        }
        let mut succ = callee_block
            .succ
            .iter()
            .map(|target| labels.get(target).copied().unwrap_or(*target))
            .collect::<Vec<_>>();
        if let Some(returned) = returned {
            if !succ.is_empty() {
                return Ok(None);
            }
            return_edges.push((labels[&callee_block.at], returned));
            succ = vec![continuation];
            ops.push(_jump(call.at, continuation));
        }
        cloned.push(MirBlock { cold: callee_block.cold, ..MirBlock::new(labels[&callee_block.at], phis, ops, succ) });
    }

    if return_edges.is_empty() {
        return Ok(None);
    }
    let mut result_phis = Vec::new();
    if return_edges.len() == 1 {
        let (return_at, returned) = &return_edges[0];
        let position = cloned.iter().position(|one| one.at == *return_at).unwrap();
        let copies = returned
            .iter()
            .zip(&results[..arity])
            .map(|(value, result)| _copy(call.at, &Arg::Held(*value), *result, result.value))
            .collect::<Vec<_>>();
        let clone = &mut cloned[position];
        let last = clone.ops.pop().unwrap();
        clone.ops.extend(copies);
        clone.ops.push(last);
    } else {
        let mut by_block = return_edges.iter().map(|(at, _)| (*at, Vec::new())).collect::<BTreeMap<_, _>>();
        let mut incoming = (0..arity).map(|_| OrderedMap::new()).collect::<Vec<_>>();
        for (at, returned) in &return_edges {
            for (number, (value, result)) in returned.iter().zip(&results[..arity]).enumerate() {
                let edge_value = fresh(
                    &mut next_id,
                    &mut next_variable,
                    &mut versions,
                    call.at,
                    false,
                    Some(result.value.variable),
                );
                by_block.get_mut(at).unwrap().push(_copy(
                    call.at,
                    &Arg::Held(*value),
                    Held { value: edge_value, width: result.width },
                    edge_value,
                ));
                incoming[number].insert(*at, edge_value);
            }
        }
        for (number, result) in results[..arity].iter().enumerate() {
            result_phis.push(Phi { result: result.value, incoming: incoming[number].clone() });
        }
        for block in &mut cloned {
            if let Some(copies) = by_block.get(&block.at) {
                let last = block.ops.pop().unwrap();
                block.ops.extend(copies.iter().cloned());
                block.ops.push(last);
            }
        }
    }

    let mut pre = caller.clone();
    pre.ops = pre_ops;
    pre.ops.extend(materialized);
    pre.ops.push(_jump(call.at, labels[&callee.entry]));
    pre.succ = vec![labels[&callee.entry]];
    let after = caller.ops[call_index + 1..].to_vec();
    let continued = MirBlock::new(continuation, result_phis, after, caller.succ.clone());
    let mut blocks = Vec::new();
    for block in &body.blocks {
        if block.at == caller.at {
            blocks.push(pre.clone());
            blocks.extend(cloned.iter().cloned());
            blocks.push(continued.clone());
            continue;
        }
        let mut block = block.clone();
        if caller.succ.contains(&block.at) {
            for phi in &mut block.phis {
                phi.incoming = phi
                    .incoming
                    .iter()
                    .map(|(at, value)| (if *at == caller.at { continuation } else { *at }, *value))
                    .collect();
            }
        }
        blocks.push(block);
    }

    let mut pointer_values = body.pointer_values.clone();
    pointer_values.extend(
        callee
            .pointer_values
            .iter()
            .filter(|value| swap.contains_key(&value.id))
            .map(|value| swap[&value.id]),
    );
    let mut pointer_seeds = body.pointer_seeds.clone();
    for (value, seed) in callee.pointer_seeds.iter() {
        if swap.contains_key(&value.id)
            && !formal_values.contains(&value.id)
            && !pointer_seeds.contains_key(&swap[&value.id])
        {
            pointer_seeds.insert(swap[&value.id], seed.clone());
        }
    }
    let mut integer_ranges = body.integer_ranges.clone();
    for (value, interval) in callee.integer_ranges.iter() {
        if swap.contains_key(&value.id)
            && !formal_values.contains(&value.id)
            && !integer_ranges.contains_key(&swap[&value.id])
        {
            integer_ranges.insert(swap[&value.id], interval.clone());
        }
    }
    let mut made = body.clone();
    made.blocks = blocks;
    made.cloned = true;
    made.pointer_values = pointer_values;
    made.pointer_seeds = pointer_seeds;
    made.integer_ranges = integer_ranges;
    Ok(Some(made))
}

fn _copy(at: i64, source: &Arg, result: Held, value: Value) -> Op {
    let uses = match source {
        Arg::Held(source) => vec![source.value],
        _ => Vec::new(),
    };
    let mut made = Op::new(at, OpCode::nothing(), "", vec![value], uses);
    made.kind = Kind::Copy;
    made.args = vec![source.clone()];
    made.results = vec![Arg::Held(result)];
    made.symbol = Some(matches!(source, Arg::Symbol(_)));
    made.reads_complete = true;
    made
}

fn _jump(at: i64, target: i64) -> Op {
    let mut made = Op::new(at, OpCode::nothing(), "", vec![], vec![]);
    made.kind = Kind::Jump;
    made.target = Some(target);
    made.symbol = Some(false);
    made.reads_complete = true;
    made
}

#[cfg(test)]
#[path = "inline_tests.rs"]
mod tests;
