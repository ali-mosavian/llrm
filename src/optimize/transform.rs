//! Shared MIR loop-transform helpers.
//!
//! Direct ports of the small helpers in `qbopt/optimize/transform.py`.

use crate::analysis::loops::Loop;
use crate::model::mir::MirBody;

/// Direct port of `qbopt.optimize.transform:_preheader`.
///
/// The one block entering `loop_` from outside it, if exactly one source
/// block occurrence does.  This deliberately walks `body.blocks` rather
/// than a predecessor map: Python preserves both source order and duplicate
/// block occurrences in the list it counts.
pub(crate) fn preheader(body: &MirBody, loop_: &Loop) -> Option<i64> {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::analysis::loops::Loop;
    use crate::model::mir::{MirBlock, MirBody};

    use super::preheader;

    fn block(at: i64, succ: Vec<i64>) -> MirBlock {
        MirBlock::new(at, Vec::new(), Vec::new(), succ)
    }

    fn loop_(header: i64, body: &[i64]) -> Loop {
        Loop {
            header,
            latches: BTreeSet::new(),
            body: body.iter().copied().collect(),
        }
    }

    #[test]
    fn preheader_returns_one_outside_predecessor_with_an_inside_latch() {
        let body = MirBody::new(
            10,
            vec![
                block(10, vec![20]),
                block(20, vec![20]),
                block(30, vec![20]),
            ],
        );

        assert_eq!(preheader(&body, &loop_(20, &[20, 30])), Some(10));
    }

    #[test]
    fn preheader_refuses_zero_or_two_outside_predecessor_occurrences() {
        let no_entry = MirBody::new(20, vec![block(20, vec![20])]);
        assert_eq!(preheader(&no_entry, &loop_(20, &[20])), None);

        let two_entries = MirBody::new(
            10,
            vec![
                block(10, vec![20]),
                block(11, vec![20]),
                block(20, vec![20]),
            ],
        );
        assert_eq!(preheader(&two_entries, &loop_(20, &[20])), None);
    }

    #[test]
    fn preheader_counts_duplicate_outside_block_occurrences() {
        let body = MirBody::new(
            10,
            vec![block(10, vec![20]), block(10, vec![20]), block(20, vec![])],
        );

        assert_eq!(preheader(&body, &loop_(20, &[20])), None);
    }
}

// ---- early port (agent F) ----

/// Keep opaque source ownership, but no computation or memory effect.
///
/// Direct port of `qbopt/optimize/transform.py:_empty_operation`.
pub(crate) fn _empty_operation(op: &crate::model::mir::Op) -> crate::model::mir::Op {
    use crate::model::mir::{Kind, OpCode, OrderedMap};
    let mut result = op.clone();
    result.op = Some(OpCode::nothing());
    result.name.clear();
    result.kind = Kind::Nothing;
    result.defines.clear();
    result.uses.clear();
    result.array = None;
    result.memory_values.clear();
    result.floating = None;
    result.floating_origin = None;
    result.args.clear();
    result.results.clear();
    result.loads.clear();
    result.stores.clear();
    result.merges = OrderedMap::new();
    result.source_backed = false;
    result.raised = None;
    result.target = None;
    result.cases.clear();
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

/// Direct port of `qbopt/optimize/transform.py:LOW`.
pub(crate) const LOW: u8 = 0;
/// Direct port of `qbopt/optimize/transform.py:HIGH`.
pub(crate) const HIGH: u8 = 1;

/// Which half of which value something reads, to a fixed point.
///
/// Direct port of `qbopt/optimize/transform.py:halves`, without the
/// `_reusing_halves` memo, which is not ported here.
pub(crate) fn halves(body: &MirBody) -> std::collections::BTreeSet<(crate::model::mir::Value, u8)> {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::model::mir::{Arg, Kind, Op, Value};

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
    out
}

/// Values some half of which something reads.
///
/// Direct port of `qbopt/optimize/transform.py:live`.
pub(crate) fn live(body: &MirBody) -> std::collections::BTreeSet<crate::model::mir::Value> {
    halves(body).into_iter().map(|(one, _)| one).collect()
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

/// Direct port of `qbopt.optimize.transform:_unreachable`.
pub(crate) fn _unreachable(body: &MirBody) -> MirBody {
    use std::collections::{BTreeMap, BTreeSet};
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<_, _>>();
    let mut reached = BTreeSet::new();
    let mut pending = vec![body.entry];
    while let Some(at) = pending.pop() {
        if reached.contains(&at) || !blocks.contains_key(&at) {
            continue;
        }
        reached.insert(at);
        pending.extend(blocks[&at].succ.iter().copied());
    }
    let mut result = body.clone();
    result.blocks = body
        .blocks
        .iter()
        .filter(|block| reached.contains(&block.at) || !block.ops.is_empty())
        .map(|block| {
            if reached.contains(&block.at) {
                block.clone()
            } else {
                let mut normalized = block.clone();
                normalized.succ = Vec::new();
                normalized.phis = Vec::new();
                normalized.ops = block.ops.iter().map(_empty_operation).collect();
                normalized
            }
        })
        .collect();
    result
}

/// Direct port of `qbopt.optimize.transform:_trivial_phis`.
pub(crate) fn _trivial_phis(
    body: &MirBody,
) -> Result<MirBody, crate::analysis::ssa::SubstitutionError> {
    use crate::analysis::ssa::{provider as _provider, substituted as _substituted};
    use crate::model::mir::{OrderedMap, Phi, Value};
    use std::collections::{BTreeMap, BTreeSet};

    let predecessors = crate::analysis::loops::predecessors(&body.blocks);
    let mut swaps = BTreeMap::<u32, Value>::new();
    let mut body = body.clone();
    loop {
        let mut changed = false;
        let mut out = Vec::new();
        for block in &body.blocks {
            let mut phis = Vec::new();
            for phi in &block.phis {
                let mut incoming = OrderedMap::new();
                for (at, value) in phi.incoming.iter() {
                    if predecessors[&block.at].contains(at) {
                        incoming.insert(*at, _provider(*value, &swaps)?);
                    }
                }
                let mut values = incoming.values().copied().collect::<BTreeSet<_>>();
                values.remove(&phi.result);
                if values.len() == 1 {
                    let only = *values.iter().next().expect("one remaining value");
                    swaps.insert(phi.result.id, only);
                    changed = true;
                } else {
                    phis.push(Phi {
                        result: phi.result,
                        incoming,
                    });
                }
            }
            let mut replaced = block.clone();
            replaced.phis = phis;
            replaced.ops = block
                .ops
                .iter()
                .map(|op| _substituted(op, &swaps))
                .collect::<Result<_, _>>()?;
            out.push(replaced);
        }
        body.blocks = out;
        if !changed {
            return Ok(body);
        }
    }
}
