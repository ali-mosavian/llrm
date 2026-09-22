//! Port of `qbopt/optimize/unswitch.py`: specialize a loop around a pure
//! invariant condition, entirely in MIR.
//!
//! Python's `ValueError`s are the `Err` text.
//!
//! Every test in `tests/test_unswitch.py` is skipped: each needs the corpus
//! or a monkeypatched pipeline.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa;
use crate::model::mir::{self, Arg, Held, Kind, MirBlock, MirBody, Op, Value};
use crate::model::passes::{AddressForm, OperationCosts, Options};
use crate::optimize::{edges, lcssa, loopclone, profit, transform};

/// `optimized`'s keyword arguments, with Python's defaults.
pub(crate) struct Optimized<'a> {
    pub registers: Option<i64>,
    pub call_registers: i64,
    pub index_scales: Option<BTreeSet<i64>>,
    pub address_forms: Option<Vec<AddressForm>>,
    pub costs: Option<OperationCosts>,
    pub options: Options,
    pub watch: Option<&'a mut dyn FnMut(&str, &MirBody)>,
}

pub(crate) fn optimized(
    body: &MirBody,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
    options: Optimized<'_>,
) -> Result<MirBody, String> {
    let candidate = specialized(body)?;
    // Python's `candidate is body`: `specialized` returns its input unchanged.
    if candidate == *body {
        return Ok(body.clone());
    }
    let Optimized {
        registers,
        call_registers,
        index_scales,
        address_forms,
        costs,
        options,
        watch,
    } = options;
    let mut stages = vec![("unswitch".to_owned(), candidate.clone())];
    let prices = costs.clone().unwrap_or_default();
    let result = {
        let mut collect = |name: &str, state: &MirBody| stages.push((name.to_owned(), state.clone()));
        transform::applied(
            &candidate,
            dgroup,
            calls,
            transform::Applied {
                options: Options { unswitch: false, ..options },
                registers,
                call_registers,
                index_scales,
                address_forms,
                costs,
                watch: Some(&mut collect),
                ..Default::default()
            },
        )?
    };

    let size = |state: &MirBody| {
        state.blocks.iter().flat_map(|block| &block.ops).filter(|op| op.kind != Kind::Nothing).count()
    };
    let (before, after) = (profit::weighted(body, &prices, None), profit::weighted(&result, &prices, None));
    let worse = match (before, after) {
        (Some(before), Some(after)) => after > before,
        _ => true,
    };
    if loops::loops(&result.blocks, Some(result.entry)).len() >= loops::loops(&body.blocks, Some(body.entry)).len()
        || size(&result) > size(body)
        || worse
    {
        return Ok(body.clone());
    }
    if let Some(watch) = watch {
        for (name, state) in &stages {
            watch(name, state);
        }
    }
    Ok(result)
}

pub(crate) fn specialized(body: &MirBody) -> Result<MirBody, String> {
    let closed = lcssa::closed(body)?;
    let mut owners: IndexMap<Value, i64> = IndexMap::default();
    for block in &closed.blocks {
        let values = block.phis.iter().map(|phi| phi.result).chain(block.ops.iter().flat_map(|op| op.defines.iter().copied()));
        for value in values {
            owners.insert(value, block.at);
        }
    }
    let dominators = loops::dominators(&closed.blocks, Some(closed.entry));
    let predecessors = loops::predecessors(&closed.blocks);
    for loop_ in loops::loops(&closed.blocks, Some(closed.entry)) {
        let outside = predecessors[&loop_.header].difference(&loop_.body).copied().collect::<Vec<_>>();
        if outside.len() != 1
            || loop_.body.iter().map(|at| closed.block(*at).expect("AttributeError").ops.len()).sum::<usize>() > 128
        {
            continue;
        }
        let entry = outside[0];
        for block in &closed.blocks {
            if !loop_.body.contains(&block.at) || block.at == loop_.header || block.succ.len() != 2 || block.ops.is_empty() {
                continue;
            }
            let branch = block.ops.last().expect("nonempty");
            let Some((_, compare)) = transform::_comparison(block, branch) else {
                continue;
            };
            if !branch.loads.is_empty()
                || !branch.stores.is_empty()
                || branch.barrier()
                || branch.floating.is_some()
                || branch.stack.is_some_and(|stack| stack != 0)
                || !branch.defines.is_empty()
                || !branch.results.is_empty()
                || mir::partial(branch)
                || !compare.loads.is_empty()
                || !compare.stores.is_empty()
                || compare.barrier()
                || compare.floating.is_some()
                || mir::partial(compare)
                || compare.stack.is_some_and(|stack| stack != 0)
                || !compare.args.iter().all(|arg| matches!(arg, Arg::Held(_) | Arg::Const(_)))
                || compare.uses.iter().any(|value| {
                    owners.get(value).is_some_and(|owner| loop_.body.contains(owner))
                        || owners.get(value).is_some_and(|owner| !dominators[&entry].contains(owner))
                })
            {
                continue;
            }
            let Some(copied) = loopclone::peeled(&closed, &loop_, 1)? else {
                continue;
            };
            let candidate = _specialized(&closed, &copied, &loop_, entry, block, compare, branch)?;
            return transform::_trivial_phis(&transform::_unreachable(&candidate));
        }
    }
    Ok(body.clone())
}

pub(crate) fn _specialized(
    body: &MirBody,
    copied: &MirBody,
    loop_: &Loop,
    entry: i64,
    selected: &MirBlock,
    compare: &Op,
    branch: &Op,
) -> Result<MirBody, String> {
    let originals = body.blocks.iter().filter(|block| loop_.body.contains(&block.at)).collect::<Vec<_>>();
    let old_labels = body.blocks.iter().map(|block| block.at).collect::<BTreeSet<_>>();
    let duplicates = copied.blocks.iter().filter(|block| !old_labels.contains(&block.at)).collect::<Vec<_>>();
    let labels = originals
        .iter()
        .zip(&duplicates)
        .map(|(original, duplicate)| (original.at, duplicate.at))
        .collect::<BTreeMap<_, _>>();
    let latch = *loop_.latches.iter().next().expect("one latch");
    let (cloned_header, cloned_latch) = (labels[&loop_.header], labels[&latch]);
    let values = ssa::values(copied).collect::<Vec<_>>();
    let next_id = values.iter().map(|value| value.id).max().expect("max() arg is an empty sequence") + 1;
    let next_variable = values.iter().map(|value| value.variable).max().expect("max() arg is an empty sequence") + 1;
    let definitions = compare
        .defines
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let index = index as u32;
            (value.id, Value { id: next_id + index, variable: next_variable + index, version: 1, ..*value })
        })
        .collect::<BTreeMap<_, _>>();
    let parent = body.block(entry).expect("AttributeError");
    let last = parent.ops.last();
    let replaces_jump = last.is_some_and(|last| last.kind == Kind::Jump);
    let anchor = last.map_or(entry, |last| last.at);
    let mut guard = compare.clone();
    guard.at = anchor;
    guard.defines = compare.defines.iter().map(|value| definitions[&value.id]).collect();
    guard.results = compare
        .results
        .iter()
        .map(|result| match result {
            Arg::Held(held) => Arg::Held(Held { value: definitions[&held.value.id], width: held.width }),
            _ => panic!("replace() got an unexpected keyword argument 'value'"),
        })
        .collect();
    guard.source_backed = false;
    guard.raised = None;
    guard.absorbed = Vec::new();
    guard.id = None;
    guard.symbol = Some(false);
    let mut dispatch = ssa::substituted(branch, &definitions).map_err(|error| error.to_string())?;
    dispatch.at = anchor;
    dispatch.target = Some(cloned_header);
    dispatch.source_backed = false;
    dispatch.raised = None;
    dispatch.name = String::new();
    dispatch.id = None;
    dispatch.symbol = Some(false);
    dispatch.absorbed = if replaces_jump { last.expect("jump").absorbed.clone() } else { Vec::new() };
    let mut parent = parent.clone();
    if replaces_jump {
        parent.ops.pop();
    }
    parent.ops.push(guard);
    parent.ops.push(dispatch);
    parent.succ = vec![loop_.header, cloned_header];
    let mut changed = Vec::new();
    for block in &copied.blocks {
        let mut block = if block.at == entry {
            parent.clone()
        } else if block.at == loop_.header {
            body.block(loop_.header).expect("header").clone()
        } else if block.at == cloned_header {
            let residual = &copied.block(loop_.header).expect("header").phis;
            let phis = block
                .phis
                .iter()
                .zip(residual)
                .map(|(phi, residual)| {
                    let mut incoming = phi.incoming.clone();
                    incoming.insert(cloned_latch, *residual.incoming.get(&cloned_latch).expect("KeyError"));
                    mir::Phi { incoming, ..phi.clone() }
                })
                .collect();
            MirBlock { phis, ..block.clone() }
        } else {
            block.clone()
        };
        if block.at == cloned_latch {
            block.succ = vec![cloned_header];
            for op in &mut block.ops {
                if op.target == Some(loop_.header) {
                    op.target = Some(cloned_header);
                }
            }
        }
        if block.at == selected.at || block.at == labels[&selected.at] {
            let taken = block.ops.last().expect("IndexError").target;
            let destination = if block.at == labels[&selected.at] {
                taken
            } else {
                Some(*block.succ.iter().find(|at| Some(**at) != taken).expect("StopIteration"))
            };
            let mut jump = block.ops.last().expect("IndexError").clone();
            jump.kind = Kind::Jump;
            jump.target = destination;
            jump.test = None;
            jump.name = String::new();
            jump.uses = Vec::new();
            jump.args = Vec::new();
            jump.defines = Vec::new();
            jump.results = Vec::new();
            jump.raised = None;
            *block.ops.last_mut().expect("IndexError") = jump;
            block.succ = destination.into_iter().collect();
        }
        changed.push(block);
    }
    let mut result = copied.with_blocks(changed);
    for header in [loop_.header, cloned_header] {
        let label = edges::fresh(&result);
        result = edges::split(&result, entry, header, label, Vec::new())?;
    }
    for version in [loop_.body.clone(), labels.values().copied().collect::<BTreeSet<_>>()] {
        let exits = result
            .blocks
            .iter()
            .filter(|block| version.contains(&block.at))
            .flat_map(|block| block.succ.iter().map(move |&target| (block.at, target)))
            .filter(|(_, target)| !version.contains(target))
            .collect::<Vec<_>>();
        for (source, target) in exits {
            if edges::conditional(result.block(source).expect("AttributeError"), target) {
                let label = edges::fresh(&result);
                result = edges::split(&result, source, target, label, Vec::new())?;
            }
        }
    }
    Ok(result)
}
