//! Port of `qbopt/optimize/gvn.py`: global value numbering and partial
//! redundancy elimination.
//!
//! One pass owns reuse, whether the value came from arithmetic or memory.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;

use crate::analysis::loops::{self, Loop};
use crate::analysis::ssa;
use crate::model::mir::{Arg, Held, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};
use crate::model::passes::Where;
use crate::optimize::{floatfold, loadjoins, profit, transform};

/// Number values, reuse dominating providers, and complete join PRE.
pub(crate) fn optimized(body: &MirBody, where_: &Where) -> Result<MirBody, String> {
    let mut avoid_store_crossing = false;
    if where_.registers != 0 {
        let pressure = profit::spill_risk(body, &where_.costs, where_.registers, None);
        let cheap_secondary =
            where_.address_forms.iter().any(|form| form.secondary && form.before_spill(&where_.costs));
        avoid_store_crossing = pressure.is_some_and(|pressure| pressure > 0) && !cheap_secondary;
    }
    let body = transform::forwarded(body, &where_.dgroup, &where_.named(), avoid_store_crossing)?;
    let body = transform::reused_divides(&body, &where_.dgroup, where_.found.as_ref())?;
    let canonical = transform::subexpressions(&body, &where_.dgroup, avoid_store_crossing)?;
    // PRE may add work to a previously missing path.  Do that only after
    // local numbering has stabilized.
    let combined = joined(&canonical, canonical == body)?;
    let loaded = loadjoins::reused(&combined, None, combined == canonical)?;
    Ok(floatfold::checks(&loaded))
}

/// Translate simultaneously: an incoming phi value belongs to the prior edge.
fn _on_edge(op: &Op, phis: &[Phi], predecessor: i64) -> Option<Op> {
    let incoming =
        phis.iter().map(|phi| (phi.result, phi.incoming.get(&predecessor).copied())).collect::<IndexMap<_, _>>();
    let mut args = Vec::new();
    for arg in &op.args {
        let mut arg = arg.clone();
        if let Arg::Held(held) = &arg {
            if let Some(value) = incoming.get(&held.value) {
                let value = (*value)?;
                arg = Arg::Held(Held { value, width: held.width });
            }
        }
        args.push(arg);
    }
    let uses = op
        .uses
        .iter()
        .map(|value| match incoming.get(value) {
            Some(value) => *value,
            None => Some(*value),
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Op { args, uses, ..op.clone() })
}

/// Insert only on an unconditional edge, with available scalar inputs.
fn _insertion(
    op: &Op,
    parent: &MirBlock,
    join: &MirBlock,
    prefix: &[Op],
    definitions: &IndexMap<Value, (i64, i64)>,
    dominators: &BTreeMap<i64, BTreeSet<i64>>,
    natural_loops: &[Loop],
) -> Option<usize> {
    let held = op
        .args
        .iter()
        .filter_map(|arg| match arg {
            Arg::Held(held) => Some(held.value),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if parent.succ != [join.at]
        || dominators[&parent.at].contains(&join.at)
        || natural_loops.iter().any(|loop_| loop_.body.contains(&parent.at) != loop_.body.contains(&join.at))
        || prefix.iter().any(|prior| prior.uses.iter().any(|value| value.flags))
        || op.args.iter().any(|arg| !matches!(arg, Arg::Held(_) | Arg::Const(_)))
        || op.uses.iter().any(|value| value.flags)
        || op.uses.iter().any(|value| !held.contains(value))
    {
        return None;
    }
    let mut cut = parent.ops.len();
    if cut != 0 && parent.ops[parent.ops.len() - 1].kind == Kind::Jump {
        cut -= 1;
    }
    if parent.ops.iter().any(|one| matches!(one.kind, Kind::Branch | Kind::Return | Kind::Escape)) {
        return None;
    }
    for arg in &op.args {
        let Arg::Held(arg) = arg else {
            continue;
        };
        let (at, index) = *definitions.get(&arg.value)?;
        if !dominators[&parent.at].contains(&at) || (at == parent.at && index >= cut as i64) {
            return None;
        }
    }
    Some(cut)
}

/// Eliminate scalar redundancy without adding execution to any path.
///
/// A phi combines independently dominating providers. Missing providers may
/// be inserted on unconditional incoming edges, but only when another edge
/// already supplies the result. Memory and floating expressions stay out.
pub(crate) fn joined(body: &MirBody, insert: bool) -> Result<MirBody, String> {
    let predecessors = loops::predecessors(&body.blocks);
    if !predecessors.values().any(|parents| parents.len() > 1) {
        return Ok(body.clone());
    }
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let natural_loops = loops::loops(&body.blocks, Some(body.entry));
    let widths = transform::_widths(body);
    let live = transform::live(body);
    let allowed = transform::_PURE
        .iter()
        .copied()
        .filter(|kind| !matches!(kind, Kind::Div | Kind::Rem | Kind::Convert | Kind::Copy))
        .collect::<BTreeSet<_>>();
    let no_stands = IndexMap::default();

    let key = |op: &Op| {
        let single = match &op.results[..] {
            [Arg::Held(result)] => Some(result.value),
            _ => None,
        };
        let result = single?;
        if !allowed.contains(&op.kind)
            || op.floating.is_some()
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.stack.is_some()
            || !op.merges.is_empty()
            || op.barrier()
            || op.defines.iter().any(|value| *value != result && !value.flags)
        {
            return None;
        }
        let expression = transform::_computation(op, &no_stands, &widths)?;
        Some((expression.kind, expression.operands, expression.results))
    };

    let mut expressions = IndexMap::<_, Vec<(i64, usize, Value)>>::default();
    let mut definitions = IndexMap::<Value, (i64, i64)>::default();
    let by_at = body.blocks.iter().map(|block| (block.at, block)).collect::<BTreeMap<_, _>>();
    let mut values = BTreeSet::<Value>::new();
    for block in &body.blocks {
        definitions.extend(block.phis.iter().map(|phi| (phi.result, (block.at, -1))));
        values.extend(block.phis.iter().flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.values().copied())));
        for (index, op) in block.ops.iter().enumerate() {
            definitions.extend(op.defines.iter().map(|value| (*value, (block.at, index as i64))));
            values.extend(op.defines.iter().chain(&op.uses).copied());
            if let Some(expression) = key(op) {
                let Arg::Held(result) = &op.results[0] else { unreachable!("key needs a held result") };
                expressions.entry(expression).or_default().push((block.at, index, result.value));
            }
        }
    }

    let mut changed = false;
    let mut fresh = values.iter().map(|value| value.id).max().unwrap_or(0) + 1;
    let mut replacements = BTreeMap::<u32, Value>::new();
    let mut insertions = IndexMap::<i64, Vec<(usize, Op)>>::default();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let parents = &predecessors[&block.at];
        if block.at == body.entry || parents.len() < 2 {
            blocks.push(block.clone());
            continue;
        }
        let (mut phis, mut ops) = (block.phis.clone(), Vec::new());
        for (index, op) in block.ops.iter().enumerate() {
            let expression = key(op);
            let mut incoming = OrderedMap::<i64, Value>::new();
            let mut missing = IndexMap::<i64, (usize, Op)>::default();
            if expression.is_some() && !op.defines.iter().any(|value| value.flags && live.contains(value)) {
                for &parent in parents {
                    let substituted = ssa::substituted(op, &replacements).map_err(|error| error.to_string())?;
                    let translated = _on_edge(&substituted, &phis, parent);
                    let edge_expression = translated.as_ref().and_then(&key);
                    let candidates = edge_expression
                        .as_ref()
                        .and_then(|edge| expressions.get(edge))
                        .into_iter()
                        .flatten()
                        .filter(|(at, _, _)| {
                            *at != block.at
                                && dominators[&parent].contains(at)
                                && !dominators[at].contains(&block.at)
                                && natural_loops
                                    .iter()
                                    .all(|loop_| !loop_.body.contains(at) || loop_.body.contains(&block.at))
                        })
                        .collect::<Vec<_>>();
                    if candidates.is_empty() {
                        if !insert {
                            break;
                        }
                        let cut = match (&edge_expression, &translated) {
                            (Some(_), Some(translated)) => _insertion(
                                translated,
                                by_at[&parent],
                                block,
                                &block.ops[..index],
                                &definitions,
                                &dominators,
                                &natural_loops,
                            ),
                            _ => None,
                        };
                        let Some(cut) = cut else {
                            break;
                        };
                        missing.insert(parent, (cut, translated.expect("an edge expression has a translation")));
                        continue;
                    }
                    // Python's `max` keeps the first of equal keys.
                    let mut best = candidates[0];
                    for item in &candidates[1..] {
                        if (dominators[&item.0].len(), item.1) > (dominators[&best.0].len(), best.1) {
                            best = item;
                        }
                    }
                    incoming.insert(parent, best.2);
                }
            }
            if incoming.is_empty() || incoming.len() + missing.len() != parents.len() {
                ops.push(op.clone());
                continue;
            }
            let Arg::Held(result_held) = &op.results[0] else { unreachable!("key needs a held result") };
            for (parent, (cut, translated)) in missing {
                let predecessor = by_at[&parent];
                // The predecessor owns this occurrence; its end may be the successor's branch label.
                let at = if predecessor.ops.is_empty() {
                    predecessor.at
                } else {
                    predecessor.ops[cut.min(predecessor.ops.len() - 1)].at
                };
                let value = Value::new(fresh, at);
                fresh += 1;
                let mut uses = Vec::new();
                for arg in &translated.args {
                    if let Arg::Held(held) = arg {
                        if !uses.contains(&held.value) {
                            uses.push(held.value);
                        }
                    }
                }
                let mut made = Op::new(at, op.op, "", vec![value], uses);
                made.kind = op.kind;
                made.args = translated.args.clone();
                made.results = vec![Arg::Held(Held { value, width: result_held.width })];
                insertions.entry(parent).or_default().push((cut, made));
                incoming.insert(parent, value);
            }
            let result = Value::new(fresh, block.at);
            fresh += 1;
            replacements.insert(result_held.value.id, result);
            phis.push(Phi { result, incoming });
            let mut erased = op.clone();
            erased.op = Some(OpCode::nothing());
            erased.kind = Kind::Nothing;
            erased.name = String::new();
            erased.args = Vec::new();
            erased.results = Vec::new();
            erased.defines = Vec::new();
            erased.uses = Vec::new();
            erased.source_backed = false;
            erased.raised = None;
            ops.push(erased);
            changed = true;
        }
        blocks.push(MirBlock { phis, ..block.with_ops(ops) });
    }
    if !changed {
        return Ok(body.clone());
    }
    for block in &mut blocks {
        let mut made = insertions.get(&block.at).cloned().unwrap_or_default();
        made.sort_by(|one, other| other.0.cmp(&one.0));
        for (cut, one) in made {
            block.ops.insert(cut, one);
        }
    }
    let mut out = Vec::new();
    for block in blocks {
        let phis = block
            .phis
            .iter()
            .map(|phi| {
                let incoming = phi
                    .incoming
                    .iter()
                    .map(|(at, value)| ssa::provider(*value, &replacements).map(|value| (*at, value)))
                    .collect::<Result<OrderedMap<_, _>, _>>()?;
                Ok(Phi { result: phi.result, incoming })
            })
            .collect::<Result<Vec<_>, ssa::SubstitutionError>>()
            .map_err(|error| error.to_string())?;
        let ops = block
            .ops
            .iter()
            .map(|op| ssa::substituted(op, &replacements))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        out.push(MirBlock { phis, ops, ..block });
    }
    Ok(body.with_blocks(out))
}
