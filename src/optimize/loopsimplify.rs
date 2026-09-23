//! Port of `qbopt/optimize/loopsimplify.py`.
//!
//! Python's `is` on an unchanged body becomes `==`: a grouping always adds a
//! block, so a changed body never equals its input.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::{loops, ssa};
use crate::model::ir::Operation;
use crate::model::mir::{Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Value};
use crate::model::passes::MIRTransform;
use crate::optimize::edges;

pub(crate) struct LoopSimplify;

impl MIRTransform for LoopSimplify {
    fn class_name(&self) -> &'static str {
        "LoopSimplify"
    }

    fn name(&self) -> &str {
        "loopsimplify"
    }

    fn transform(&mut self, body: Rc<MirBody>) -> Result<Rc<MirBody>, String> {
        Ok(simplified(&body))
    }
}

pub(crate) fn grouped(body: &Rc<MirBody>, target: i64, sources: &BTreeSet<i64>) -> Rc<MirBody> {
    let destination = body.block(target);
    let predecessors = loops::predecessors(&body.blocks);
    let Some(destination) = destination else {
        return body.clone();
    };
    if sources.is_empty() || !sources.is_subset(&predecessors[&target]) || target == body.entry {
        return body.clone();
    }
    let parents = body.blocks.iter().filter(|block| sources.contains(&block.at));
    for parent in parents {
        let Some(last) = parent.ops.last() else {
            return body.clone();
        };
        if last.kind == Kind::Branch {
            if !edges::conditional(parent, target) {
                return body.clone();
            }
        } else if last.kind == Kind::Jump {
            if parent.succ != [target] || last.target != Some(target) {
                return body.clone();
            }
        } else if parent.succ != [target]
            || matches!(last.kind, Kind::Call | Kind::Return | Kind::Opaque | Kind::Switch | Kind::Escape)
            || last.barrier()
        {
            return body.clone();
        }
    }
    if destination.phis.iter().any(|phi| {
        phi.incoming.keys().copied().collect::<BTreeSet<_>>() != predecessors[&target] || phi.result.flags
    }) {
        return body.clone();
    }
    let label = edges::fresh(body);
    let mut serial = ssa::values(body).map(|value| i64::from(value.id)).max().unwrap_or(-1) + 1;
    let mut versions: BTreeMap<u32, u32> = BTreeMap::new();
    for value in ssa::values(body) {
        let known = versions.get(&value.variable).copied().unwrap_or(0);
        versions.insert(value.variable, known.max(value.version));
    }
    let mut bridge_phis = Vec::new();
    let mut target_phis = Vec::new();
    for phi in &destination.phis {
        let incoming = phi
            .incoming
            .iter()
            .filter(|(at, _)| sources.contains(at))
            .map(|(&at, &value)| (at, value))
            .collect::<OrderedMap<_, _>>();
        let values = incoming.values().copied().collect::<BTreeSet<_>>();
        let result = if values.len() == 1 {
            *values.iter().next().expect("one value")
        } else {
            let variable = phi.result.variable;
            let version = versions.get(&variable).copied().unwrap_or(0) + 1;
            versions.insert(variable, version);
            let result = Value { id: serial as u32, at: label, flags: false, variable, version };
            serial += 1;
            bridge_phis.push(Phi { result, incoming });
            result
        };
        let mut kept = phi
            .incoming
            .iter()
            .filter(|(at, _)| !sources.contains(at))
            .map(|(&at, &value)| (at, value))
            .collect::<OrderedMap<_, _>>();
        kept.insert(label, result);
        target_phis.push(Phi { incoming: kept, ..phi.clone() });
    }
    let mut jump = Op::new(label, OpCode::Operation(Operation::Jump), "", vec![], vec![]);
    jump.kind = Kind::Jump;
    jump.target = Some(target);
    jump.symbol = Some(false);
    let bridge = MirBlock::new(label, bridge_phis, vec![jump], vec![target]);
    let mut changed = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        if sources.contains(&block.at) {
            block.succ = block.succ.iter().map(|&at| if at == target { label } else { at }).collect();
            let last = block.ops.last_mut().expect("checked above");
            if last.target == Some(target) {
                last.target = Some(label);
            }
        }
        if block.at == target {
            block.phis = target_phis.clone();
        }
        changed.push(block);
    }
    changed.push(bridge);
    Rc::new(body.with_blocks(changed))
}

pub(crate) fn simplified(body: &Rc<MirBody>) -> Rc<MirBody> {
    let mut body = body.clone();
    if !loops::irreducible(&body.blocks, Some(body.entry)).is_empty() {
        return body;
    }
    for original in loops::loops(&body.blocks, Some(body.entry)) {
        let original = loops::loops(&body.blocks, Some(body.entry))
            .into_iter()
            .find(|loop_| loop_.header == original.header)
            .expect("StopIteration");
        let mut candidate = body.clone();
        let predecessors = loops::predecessors(&candidate.blocks);
        let outside = predecessors[&original.header].difference(&original.body).copied().collect::<BTreeSet<_>>();
        if outside.is_empty() {
            continue;
        }
        let Some(parent) = candidate.block(*outside.iter().next().expect("nonempty")) else {
            continue;
        };
        if outside.len() != 1 || parent.succ != [original.header] {
            candidate = grouped(&candidate, original.header, &outside);
            if Rc::ptr_eq(&candidate, &body) {
                continue;
            }
        }
        if original.latches.len() != 1 {
            let changed = grouped(&candidate, original.header, &original.latches);
            if Rc::ptr_eq(&changed, &candidate) {
                continue;
            }
            candidate = changed;
        }
        let current = loops::loops(&candidate.blocks, Some(candidate.entry))
            .into_iter()
            .find(|loop_| loop_.header == original.header)
            .expect("StopIteration");
        let predecessors = loops::predecessors(&candidate.blocks);
        let exits = candidate
            .blocks
            .iter()
            .filter(|block| current.body.contains(&block.at))
            .flat_map(|block| block.succ.iter().copied())
            .filter(|at| !current.body.contains(at))
            .collect::<BTreeSet<_>>();
        let empty = BTreeSet::new();
        let mut broke = false;
        for target in exits {
            let reaching = predecessors.get(&target).unwrap_or(&empty);
            let sources = reaching.intersection(&current.body).copied().collect::<BTreeSet<_>>();
            if !reaching.is_subset(&current.body) {
                let changed = grouped(&candidate, target, &sources);
                if Rc::ptr_eq(&changed, &candidate) {
                    broke = true;
                    break;
                }
                candidate = changed;
            }
        }
        if !broke {
            body = candidate;
        }
    }
    body
}

#[cfg(test)]
#[path = "loopsimplify_tests.rs"]
mod tests;
