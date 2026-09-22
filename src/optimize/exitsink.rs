//! Direct port of `qbopt/optimize/exitsink.py`.
//!
//! Tests: `tests/test_ivshare.py:test_final_pointer_update_moves_to_the_exit`
//! is skipped; it builds its input through `wholeseg.emitted`, which is not
//! ported.

use std::rc::Rc;
use std::collections::BTreeMap;

use crate::analysis::liveness;
use crate::analysis::loops;
use crate::analysis::ssa::{self, SubstitutionError};
use crate::model::mir::{Arg, Held, Kind, MirBody, Op, OrderedMap, Phi, Value};
use crate::optimize::strength;
use crate::optimize::transform;

/// Direct port of `qbopt/optimize/exitsink.py:sunk`.  Python lets
/// `ssa.substituted` raise; Rust returns that refusal.
pub(crate) fn sunk(body: &Rc<MirBody>) -> Result<Rc<MirBody>, SubstitutionError> {
    if body.blocks.iter().flat_map(|block| &block.ops).any(|op| {
        (op.barrier() || op.kind == Kind::Opaque) && !op.reads_complete
    }) {
        return Ok(body.clone());
    }
    let mut definitions: BTreeMap<Value, (i64, &Op)> = BTreeMap::new();
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.defines {
                definitions.insert(*value, (block.at, op));
            }
        }
    }
    let live = transform::live(body);
    let live_in = liveness::live(body).live_in;
    let all_values = ssa::values(body).collect::<Vec<_>>();
    let mut serial = all_values.iter().map(|value| value.id).max().unwrap_or(0) + 1;
    let mut variable = all_values.iter().map(|value| value.variable).max().unwrap_or(0) + 1;
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        for (index, block) in body.blocks.iter().enumerate() {
            if loop_.body.contains(&block.at)
                || block.ops.is_empty()
                || live_in
                    .get(&block.at)
                    .is_some_and(|values| values.iter().any(|value| value.flags))
            {
                continue;
            }
            for (phi_index, phi) in block.phis.iter().enumerate() {
                if phi.incoming.len() != 1 {
                    continue;
                }
                let (predecessor, value) = phi.incoming.iter().next().map(|(at, value)| (*at, *value)).unwrap();
                if !loop_.body.contains(&predecessor) || !definitions.contains_key(&value) {
                    continue;
                }
                let (where_, op) = definitions[&value];
                if !loop_.body.contains(&where_)
                    || !matches!(op.kind, Kind::Add | Kind::Sub)
                    || op.barrier()
                    || !op.loads.is_empty()
                    || !op.stores.is_empty()
                    || op.stack.is_some()
                    || op.floating.is_some()
                    || op.results.len() != 1
                    || !matches!(op.results[0], Arg::Held(_))
                    || op.defines.iter().any(|other| *other != value && live.contains(other))
                {
                    continue;
                }
                if body
                    .blocks
                    .iter()
                    .any(|one| one.ops.iter().any(|other| other.uses.contains(&value)))
                {
                    continue;
                }
                if body.blocks.iter().any(|one| {
                    one.phis
                        .iter()
                        .any(|other| !std::ptr::eq(other, phi) && other.incoming.values().any(|each| *each == value))
                }) {
                    continue;
                }
                if op.args.iter().any(|arg| !matches!(arg, Arg::Held(_) | Arg::Const(_))) {
                    continue;
                }
                let mut exported: OrderedMap<Value, Value> = OrderedMap::new();
                for source in op.uses.iter().chain(op.merges.keys()) {
                    if !exported.contains_key(source) {
                        let mut fresh = Value::new(serial, block.at);
                        fresh.variable = variable;
                        exported.insert(*source, fresh);
                        serial += 1;
                        variable += 1;
                    }
                }
                let args = op
                    .args
                    .iter()
                    .map(|arg| match arg {
                        Arg::Held(held) => Arg::Held(Held {
                            value: *exported.get(&held.value).expect("exported"),
                            width: held.width,
                        }),
                        other => other.clone(),
                    })
                    .collect::<Vec<_>>();
                let mut moved = strength::_made(op.kind, "", value, args, block.at, op);
                moved.merges = op
                    .merges
                    .keys()
                    .map(|source| (*exported.get(source).expect("exported"), value))
                    .collect();
                moved.uses = exported.values().copied().collect();
                let exports = exported
                    .iter()
                    .map(|(source, result)| {
                        let mut export = Phi::new(*result);
                        export.incoming.insert(predecessor, *source);
                        export
                    })
                    .collect::<Vec<_>>();
                let mut changed = MirBody::clone(body);
                for (position, one) in body.blocks.iter().enumerate() {
                    if position == index {
                        let target = &mut changed.blocks[position];
                        target.phis = one
                            .phis
                            .iter()
                            .enumerate()
                            .filter(|(other, _)| *other != phi_index)
                            .map(|(_, other)| other.clone())
                            .chain(exports.iter().cloned())
                            .collect();
                        target.ops.insert(0, moved.clone());
                    } else {
                        changed.blocks[position].ops = one
                            .ops
                            .iter()
                            .map(|other| {
                                if std::ptr::eq(other, op) {
                                    transform::_empty_operation(other)
                                } else {
                                    other.clone()
                                }
                            })
                            .collect();
                    }
                }
                let swap = BTreeMap::from([(phi.result.id, value)]);
                for one in &mut changed.blocks {
                    one.ops = one
                        .ops
                        .iter()
                        .map(|other| ssa::substituted(other, &swap))
                        .collect::<Result<_, _>>()?;
                    for other in &mut one.phis {
                        other.incoming = other
                            .incoming
                            .iter()
                            .map(|(edge, source)| (*edge, if *source == phi.result { value } else { *source }))
                            .collect();
                    }
                }
                return Ok(Rc::new(changed));
            }
        }
    }
    Ok(body.clone())
}
