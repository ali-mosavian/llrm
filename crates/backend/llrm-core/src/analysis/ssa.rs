//! Port of `qbopt/analysis/ssa.py`.
//!
//! `use_index` returns snapshot-local [`OpOccurrence`] keys where Python
//! returns operation objects, whose identity distinguishes equal operations.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::model::mir::{
    MirBody, Value,
    consumed as operation_consumed,
};

/// Drop phis nothing needs, including cycles only other dead phis read.
///
/// Direct port of `qbopt.analysis.ssa:pruned_phis`.
pub fn pruned_phis(body: &Rc<MirBody>, roots: &BTreeSet<Value>) -> Rc<MirBody> {
    let mut needed: BTreeSet<Value> = roots | &crate::model::mir::exposed(body);
    for op in body.blocks.iter().flat_map(|block| &block.ops) {
        needed.extend(operation_consumed(op));
        for reference in op.loads.iter().chain(&op.stores) {
            needed.extend(reference.base.iter().chain(&reference.segment).copied());
        }
    }
    let phis: BTreeMap<Value, &crate::model::mir::Phi> =
        body.blocks.iter().flat_map(|block| &block.phis).map(|phi| (phi.result, phi)).collect();
    let mut pending: Vec<Value> = needed.iter().filter(|value| phis.contains_key(value)).copied().collect();
    while let Some(next) = pending.pop() {
        let incoming: BTreeSet<Value> =
            phis[&next].incoming.values().filter(|value| !needed.contains(value)).copied().collect();
        needed.extend(incoming.iter().copied());
        pending.extend(incoming.into_iter().filter(|value| phis.contains_key(value)));
    }
    let removed: BTreeSet<Value> = phis.keys().filter(|value| !needed.contains(value)).copied().collect();
    if removed.is_empty() {
        return body.clone();
    }
    let mut body = MirBody::clone(body);
    for block in &mut body.blocks {
        block.phis.retain(|phi| !removed.contains(&phi.result));
        for op in &mut block.ops {
            op.uses.retain(|value| !removed.contains(value));
            op.merges = op.merges.iter().filter(|(source, _)| !removed.contains(source)).map(|(&s, &t)| (s, t)).collect();
        }
    }
    Rc::new(body)
}

/// Every value mentioned by a body, in its source declaration order.
///
/// Direct port of `qbopt.analysis.ssa:values`.
pub fn values(body: &MirBody) -> impl Iterator<Item = Value> + '_ {
    body.blocks.iter().flat_map(|block| {
        block
            .ops
            .iter()
            .flat_map(|op| op.defines.iter().chain(&op.uses).chain(&op.exits).copied())
            .chain(
                block.phis.iter().flat_map(|phi| {
                    std::iter::once(phi.result).chain(phi.incoming.values().copied())
                }),
            )
    })
}
