//! Facts that cross direct procedure boundaries.
//!
//! The C raise deliberately gives every procedure an independent MIR body.  A
//! direct call is consequently opaque to ordinary SCCP even when every return in
//! the named body has the same value.  This module computes that fact over the
//! whole compilation unit and materialises it after the call.  The call itself
//! stays unless a separate purity proof says its effects are unobservable.
//!
//! Direct port of `qbopt/analysis/interprocedural.py`.

#![allow(dead_code)] // The cfront optimizer port is its first production caller.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;

use crate::abi::runtime::Contract;
use crate::analysis::{consts, noreturn as control, ssa};
use crate::model::mir::{self, Arg, Const, Held, Kind, MemRef, MirBody, Op, OpCode, Value};
use crate::objectfile::module::Space;
use crate::support::pyrepr;

pub(crate) type Returns = IndexMap<String, Vec<Const>>;
pub(crate) type Parameters = IndexMap<String, Vec<Option<Const>>>;

const _MAY_TRAP: [Kind; 5] = [Kind::Div, Kind::Rem, Kind::Divmod, Kind::Udivmod, Kind::FixedDiv];
const _FLOATING: [Kind; 11] = [
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

/// Parameter constants agreed by every direct call to a private body.
pub(crate) fn constant_parameters(
    procedures: &IndexMap<String, (&IndexMap<i64, String>, &IndexMap<i64, Vec<Option<Const>>>)>,
    eligible: &BTreeSet<String>,
) -> Parameters {
    let mut actuals = eligible.iter().map(|name| (name.clone(), Vec::new())).collect::<IndexMap<_, _>>();
    for (calls, constants) in procedures.values() {
        for (at, target) in calls.iter() {
            if let (Some(sites), Some(constant)) = (actuals.get_mut(target), constants.get(at)) {
                sites.push(constant.clone());
            }
        }
    }

    _agreed_parameters(&actuals)
}

/// Parameter constants proved at every *surviving* private call.
///
/// Source-time facts are enough for a literal call, but not for a call whose
/// actual becomes constant only after another private return is summarized.
/// Read the current MIR instead: SCCP owns the value fact, while the C call
/// contract supplies the exact ARG operations belonging to each call.  A
/// missing or malformed contract is an unknown call, rather than permission
/// to specialize a body that may still receive a different value.
pub(crate) fn current_parameter_constants(
    bodies: &IndexMap<String, MirBody>,
    calls: &IndexMap<String, IndexMap<i64, String>>,
    arguments: &IndexMap<String, IndexMap<i64, BTreeSet<i64>>>,
    parameters: &IndexMap<String, Vec<MemRef>>,
    eligible: &BTreeSet<String>,
) -> Parameters {
    let empty_calls = IndexMap::default();
    let empty_arguments = IndexMap::default();
    let mut actuals = eligible.iter().map(|name| (name.clone(), Vec::new())).collect::<IndexMap<_, _>>();
    for (owner, body) in bodies {
        let owner_calls = calls.get(owner).unwrap_or(&empty_calls);
        for (at, values) in
            current_call_constants(body, owner_calls, arguments.get(owner).unwrap_or(&empty_arguments), parameters)
        {
            if let Some(sites) = owner_calls.get(&at).and_then(|target| actuals.get_mut(target)) {
                sites.push(values);
            }
        }
    }

    _agreed_parameters(&actuals)
}

/// Current SCCP facts for every direct call with a known C contract.
///
/// The result is deliberately per-call instead of per-callee: a costed
/// inlining decision may use one constant call even when a second dynamic
/// call prevents whole-body parameter specialization.
pub(crate) fn current_call_constants(
    body: &MirBody,
    calls: &IndexMap<i64, String>,
    arguments: &IndexMap<i64, BTreeSet<i64>>,
    parameters: &IndexMap<String, Vec<MemRef>>,
) -> IndexMap<i64, Vec<Option<Const>>> {
    let facts = consts::known(body, None, None, None, None);
    let mut out = IndexMap::default();
    for block in &body.blocks {
        for (index, call) in block.ops.iter().enumerate() {
            if call.kind != Kind::Call {
                continue;
            }
            let Some(target) = calls.get(&call.at).filter(|target| parameters.contains_key(*target)) else {
                continue;
            };
            let width = parameters[target].len();
            let sites = arguments.get(&call.at);
            let selected = match sites {
                Some(sites) => block.ops[..index]
                    .iter()
                    .filter(|op| op.kind == Kind::Arg && sites.contains(&op.at) && op.args.len() == 1)
                    .collect::<Vec<_>>(),
                None => Vec::new(),
            };
            // cdecl lays down the final source argument first.
            let values = selected
                .iter()
                .rev()
                .map(|op| _constant_argument(&op.args[0], &facts))
                .collect::<Vec<_>>();
            out.insert(call.at, if values.len() == width { values } else { vec![None; width] });
        }
    }
    out
}

fn _constant_argument(argument: &Arg, facts: &IndexMap<Value, consts::Known>) -> Option<Const> {
    if let Arg::Const(argument) = argument {
        return Some(Const::new(consts::masked(&argument.n, argument.width), argument.width));
    }
    if let Arg::Held(argument) = argument {
        if let Some(fact) = facts.get(&argument.value).filter(|fact| fact.width >= argument.width) {
            return Some(Const::new(consts::masked(&fact.n, argument.width), argument.width));
        }
    }
    None
}

/// Facts shared by every call in an already-normalized actual map.
fn _agreed_parameters(actuals: &IndexMap<String, Vec<Vec<Option<Const>>>>) -> Parameters {
    let mut out = Parameters::default();
    for (name, sites) in actuals {
        if sites.is_empty() || sites.iter().map(Vec::len).collect::<BTreeSet<_>>().len() != 1 {
            continue;
        }
        let mut agreed = Vec::new();
        for index in 0..sites[0].len() {
            let values = sites.iter().map(|site| site[index].clone()).collect::<BTreeSet<_>>();
            agreed.push(if values.len() == 1 && !values.contains(&None) {
                values.into_iter().next().flatten()
            } else {
                None
            });
        }
        if agreed.iter().any(Option::is_some) {
            out.insert(name.clone(), agreed);
        }
    }
    out
}

/// Seed agreed parameter bytes at procedure entry for ordinary SCCP.
pub(crate) fn specialize_parameters(body: &MirBody, parameters: &[MemRef], constants: &[Option<Const>]) -> MirBody {
    let known = parameters
        .iter()
        .zip(constants)
        .filter_map(|(reference, constant)| match constant {
            Some(constant) if constant.width == reference.width => Some((reference.clone(), constant.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let added = known.into_iter().filter(|one| !body.initial.contains(one)).collect::<Vec<_>>();
    if added.is_empty() {
        return body.clone();
    }
    let mut made = body.clone();
    made.initial.extend(added);
    made
}

/// The common integer tuple produced by every return of each body.
///
/// Absence is the conservative answer for void, floating, mixed-width or
/// disagreeing returns.  Values are read from SCCP's fixed point, so copies,
/// promoted locals, phis and folded expressions need no special cases here.
pub(crate) fn constant_returns(bodies: &IndexMap<String, MirBody>) -> Returns {
    let mut out = Returns::default();
    for (name, body) in bodies {
        let facts = consts::known(body, None, None, None, None);
        let mut returned = Vec::new();
        let mut complete = true;
        for block in &body.blocks {
            for op in &block.ops {
                if op.kind != Kind::Return {
                    continue;
                }
                let mut values = Vec::new();
                for arg in &op.args {
                    match arg {
                        Arg::Const(arg) => values.push(Const::new(consts::masked(&arg.n, arg.width), arg.width)),
                        Arg::Held(arg)
                            if facts.get(&arg.value).is_some_and(|fact| fact.width >= arg.width) =>
                        {
                            values.push(Const::new(consts::masked(&facts[&arg.value].n, arg.width), arg.width));
                        }
                        _ => {
                            complete = false;
                            break;
                        }
                    }
                }
                if !complete || values.is_empty() {
                    break;
                }
                returned.push(values);
            }
            if !complete {
                break;
            }
        }
        if complete && !returned.is_empty() && returned.iter().collect::<BTreeSet<_>>().len() == 1 {
            out.insert(name.clone(), returned.swap_remove(0));
        }
    }
    out
}

/// Define a direct call's known result from its module-level summary.
///
/// Fresh values receive the physical call result.  Constant copies define
/// the original SSA names immediately afterwards, allowing the ordinary
/// body pipeline to propagate through phis and fold consumers.  Unmodelled
/// extra results (for example DX after a 16-bit C result in AX) stay intact.
pub(crate) fn propagate_returns(
    body: &MirBody,
    calls: &IndexMap<i64, String>,
    returns: &Returns,
    done: &BTreeSet<i64>,
) -> Result<(MirBody, BTreeSet<i64>), String> {
    let values = ssa::values(body).collect::<Vec<_>>();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut completed = done.clone();
    let mut changed = false;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let known = if op.kind == Kind::Call && !completed.contains(&op.at) {
                returns.get(calls.get(&op.at).map_or("", String::as_str))
            } else {
                None
            };
            let integer_results = op
                .results
                .iter()
                .filter_map(|result| match result {
                    Arg::Held(result) => Some(*result),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let Some(known) = known.filter(|known| !known.is_empty() && known.len() <= integer_results.len()) else {
                ops.push(op.clone());
                continue;
            };
            let pairs = known.iter().zip(&integer_results).collect::<Vec<_>>();
            if pairs.iter().any(|(constant, result)| constant.width != result.width) {
                ops.push(op.clone());
                continue;
            }

            let mut replacements = BTreeMap::new();
            let mut copies = Vec::new();
            for (constant, result) in pairs {
                serial += 1;
                variable += 1;
                let fresh = Value { id: serial, at: op.at, flags: false, variable, version: 1 };
                replacements.insert(result.value, fresh);
                let mut copy = Op::new(op.at, OpCode::nothing(), "", vec![result.value], vec![]);
                copy.kind = Kind::Copy;
                copy.args = vec![Arg::Const(constant.clone())];
                copy.results = vec![Arg::Held(*result)];
                copy.symbol = Some(false);
                copies.push(copy);
            }
            let mut made = op.clone();
            made.results = op
                .results
                .iter()
                .map(|result| match result {
                    Arg::Held(result) => Arg::Held(Held {
                        value: replacements.get(&result.value).copied().unwrap_or(result.value),
                        width: result.width,
                    }),
                    _ => result.clone(),
                })
                .collect();
            made.defines = op
                .defines
                .iter()
                .map(|value| replacements.get(value).copied().unwrap_or(*value))
                .collect();
            made.raised = None;
            ops.push(made);
            ops.extend(copies);
            completed.insert(op.at);
            changed = true;
        }
        let mut made = block.clone();
        made.ops = ops;
        blocks.push(made);
    }
    let made = if changed {
        let mut made = body.clone();
        made.blocks = blocks;
        made
    } else {
        body.clone()
    };
    let problems = mir::verify(&made);
    if !problems.is_empty() {
        return Err(format!(
            "interprocedural return propagation broke SSA: {}",
            pyrepr::list(&problems[..problems.len().min(3)])
        ));
    }
    Ok((made, completed))
}

/// Whether every CFG path ends in RETURN without revisiting a block.
fn _acyclic_returning(body: &MirBody) -> bool {
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();

    fn visit(
        at: i64,
        blocks: &BTreeMap<i64, &mir::MirBlock>,
        visiting: &mut BTreeSet<i64>,
        visited: &mut BTreeSet<i64>,
    ) -> bool {
        if visiting.contains(&at) || !blocks.contains_key(&at) {
            return false;
        }
        if visited.contains(&at) {
            return true;
        }
        visiting.insert(at);
        let block = blocks[&at];
        let okay = if !block.succ.is_empty() {
            block.succ.iter().all(|one| visit(*one, blocks, visiting, visited))
        } else {
            block.ops.last().is_some_and(|last| last.kind == Kind::Return)
        };
        visiting.remove(&at);
        if okay {
            visited.insert(at);
        }
        okay
    }

    visit(body.entry, &blocks, &mut visiting, &mut visited)
}

fn _local_effects(body: &MirBody, calls: &IndexMap<i64, String>, pure: &BTreeSet<String>) -> bool {
    if !_acyclic_returning(body) {
        return false;
    }
    for block in &body.blocks {
        for op in &block.ops {
            if op.barrier()
                || _MAY_TRAP.contains(&op.kind)
                || _FLOATING.contains(&op.kind)
                || [Kind::Escape, Kind::Opaque, Kind::Fill].contains(&op.kind)
            {
                return false;
            }
            if op.kind == Kind::Call {
                if !calls.get(&op.at).is_some_and(|target| pure.contains(target)) {
                    return false;
                }
                continue;
            }
            if op
                .loads
                .iter()
                .chain(&op.stores)
                .any(|reference| !matches!(reference.space, Some(Space::Frame | Space::Stack)))
            {
                return false;
            }
        }
    }
    true
}

/// Direct procedures with no observable effects and guaranteed return.
///
/// The least fixed point admits an acyclic call chain once all its callees
/// are admitted.  Recursive SCCs remain conservative because removing one
/// would otherwise remove possible nontermination.
pub(crate) fn pure_procedures(procedures: &IndexMap<String, (&MirBody, &IndexMap<i64, String>)>) -> BTreeSet<String> {
    let mut pure = BTreeSet::new();
    loop {
        let mut made = pure.clone();
        made.extend(
            procedures
                .iter()
                .filter(|(_, (body, calls))| _local_effects(body, calls, &pure))
                .map(|(name, _)| name.clone()),
        );
        if made == pure {
            return pure;
        }
        pure = made;
    }
}

/// Acyclic user bodies whose unused calls have no observable effect.
///
/// This is intentionally broader than `pure_procedures`: an ordinary,
/// direct read of this module's static data is not observable in C when its
/// result is unused.  It remains narrower than a general no-fault proof:
/// pointer-based, far/externally selected, volatile and floating reads stay
/// out, as do all non-frame writes.  Callers may use this fact only to erase
/// a dead result; it is not an inlining or alias-preservation permission.
pub(crate) fn readonly_procedures(
    procedures: &IndexMap<String, (&MirBody, &IndexMap<i64, String>)>,
) -> BTreeSet<String> {
    let mut readonly = BTreeSet::new();
    loop {
        let mut made = readonly.clone();
        made.extend(
            procedures
                .iter()
                .filter(|(_, (body, calls))| _readonly_effects(body, calls, &readonly))
                .map(|(name, _)| name.clone()),
        );
        if made == readonly {
            return readonly;
        }
        readonly = made;
    }
}

fn _readonly_effects(body: &MirBody, calls: &IndexMap<i64, String>, readonly: &BTreeSet<String>) -> bool {
    if !_acyclic_returning(body) {
        return false;
    }
    let local = [Space::Frame, Space::Stack];
    let is_local = |reference: &MemRef| reference.space.is_some_and(|space| local.contains(&space));
    for block in &body.blocks {
        for op in &block.ops {
            if op.barrier()
                || _MAY_TRAP.contains(&op.kind)
                || _FLOATING.contains(&op.kind)
                || [Kind::Escape, Kind::Opaque, Kind::Fill].contains(&op.kind)
            {
                return false;
            }
            if op.kind == Kind::Call {
                if !calls.get(&op.at).is_some_and(|target| readonly.contains(target)) {
                    return false;
                }
                continue;
            }
            if op.loads.iter().chain(&op.stores).any(|reference| reference.volatile) {
                return false;
            }
            // Internal frame writes disappear with the call.  Any write to a
            // nonlocal object remains observable, even if it is otherwise an
            // exact direct reference.
            if op.stores.iter().any(|reference| !is_local(reference)) {
                return false;
            }
            for reference in &op.loads {
                if is_local(reference) {
                    continue;
                }
                // A direct near static data reference is guaranteed to name
                // this module's mapped data.  Do not infer the same from an
                // arbitrary pointer, external selector or far access.
                if reference.space != Some(Space::Segment) || reference.base.is_some() || reference.segment.is_some() {
                    return false;
                }
            }
        }
    }
    true
}

/// Direct private procedures that cannot reach a normal return.
///
/// This is the named-body spelling of the shared MIR control proof used by
/// the object path.  Start with all private candidates and remove a body
/// only when a normal return remains reachable.  This greatest fixed point
/// proves a closed recursive SCC terminal when every member stops through a
/// member of that same SCC; an unknown, external, public, or returning edge
/// removes its owner instead of being assumed terminal.
pub(crate) fn noreturn_procedures(
    procedures: &IndexMap<String, (&MirBody, &IndexMap<i64, String>)>,
    eligible: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut proven = eligible
        .iter()
        .filter(|name| procedures.contains_key(*name))
        .cloned()
        .collect::<BTreeSet<_>>();
    loop {
        let found = procedures
            .iter()
            .filter(|(name, _)| eligible.contains(*name))
            .filter(|(_, (body, calls))| {
                control::_cannot_return(
                    body,
                    &calls
                        .iter()
                        .filter(|(_, target)| proven.contains(*target))
                        .map(|(at, _)| *at)
                        .collect(),
                )
            })
            .map(|(name, _)| name.clone())
            .collect::<BTreeSet<_>>();
        if found == proven {
            return proven;
        }
        proven = found;
    }
}

/// Apply the shared MIR terminal-call cleanup to named direct C calls.
pub(crate) fn terminal_calls(body: &MirBody, calls: &IndexMap<i64, String>, noreturn: &BTreeSet<String>) -> MirBody {
    let sites = calls
        .iter()
        .filter(|(_, target)| noreturn.contains(*target))
        .map(|(at, _)| *at)
        .collect::<BTreeSet<_>>();
    control::after_terminal_calls(body, &sites)
}

/// Associate each call with the exact stack ARG operations that feed it.
pub(crate) fn argument_sites(body: &MirBody, contracts: &IndexMap<i64, Contract>) -> IndexMap<i64, BTreeSet<i64>> {
    let mut out = IndexMap::default();
    for block in &body.blocks {
        for (index, op) in block.ops.iter().enumerate() {
            if op.kind != Kind::Call || !contracts.contains_key(&op.at) {
                continue;
            }
            let contract = &contracts[&op.at];
            let Some(cleanup) = contract.cleanup else {
                continue;
            };
            let needed = cleanup + contract.caller_cleanup;
            if needed == 0 {
                out.insert(op.at, BTreeSet::new());
                continue;
            }
            let mut found = BTreeSet::new();
            let mut total = 0;
            for prior in block.ops[..index].iter().rev() {
                if prior.kind == Kind::Call {
                    break;
                }
                if prior.kind != Kind::Arg || prior.args.len() != 1 {
                    continue;
                }
                // Python's `getattr(arg, "width", 0)`: a cell or opaque has none.
                let width = match &prior.args[0] {
                    Arg::Held(one) => i64::from(one.width),
                    Arg::Const(one) => i64::from(one.width),
                    Arg::Symbol(one) => i64::from(one.width),
                    Arg::FrameAddress(one) => i64::from(one.width),
                    Arg::FrameSelector(one) => i64::from(one.width),
                    Arg::Cell(_) | Arg::Opaque(_) => 0,
                };
                if width <= 0 {
                    break;
                }
                found.insert(prior.at);
                total += width;
                if total >= needed {
                    break;
                }
            }
            if total == needed {
                out.insert(op.at, found);
            }
        }
    }
    out
}

/// Remove effect-free calls whose result no operation still reads.
pub(crate) fn remove_dead_pure_calls(
    body: &MirBody,
    calls: &IndexMap<i64, String>,
    pure: &BTreeSet<String>,
    arguments: &IndexMap<i64, BTreeSet<i64>>,
) -> Result<MirBody, String> {
    let mut used = BTreeSet::new();
    for block in &body.blocks {
        used.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
        used.extend(block.ops.iter().flat_map(mir::consumed));
    }
    let removed = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| {
            op.kind == Kind::Call
                && calls.get(&op.at).is_some_and(|target| pure.contains(target))
                && !op.defines.iter().any(|value| used.contains(value))
                && arguments.contains_key(&op.at)
        })
        .map(|op| op.at)
        .collect::<BTreeSet<_>>();
    if removed.is_empty() {
        return Ok(body.clone());
    }
    let discarded_arguments = removed
        .iter()
        .flat_map(|at| arguments[at].iter().copied())
        .collect::<BTreeSet<_>>();
    let mut made = body.clone();
    for block in &mut made.blocks {
        block.ops.retain(|op| {
            !((op.kind == Kind::Call && removed.contains(&op.at))
                || (op.kind == Kind::Arg && discarded_arguments.contains(&op.at)))
        });
    }
    let problems = mir::verify(&made);
    if !problems.is_empty() {
        return Err(format!("pure call removal broke SSA: {}", pyrepr::list(&problems[..problems.len().min(3)])));
    }
    Ok(made)
}

#[cfg(test)]
#[path = "interprocedural_tests.rs"]
mod tests;
