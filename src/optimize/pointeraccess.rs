//! Expose the independently consumed words of packed far-pointer accesses.
//!
//! Direct port of `qbopt/optimize/pointeraccess.py`.

use std::rc::Rc;
use std::collections::BTreeMap;

use crate::analysis::ssa;
use crate::model::mir::{
    self, Arg, Cell, Const, Held, Kind, MemRef, MirBody, Op, OpCode, Synth, Value,
};
use crate::objectfile::module::{Addr, Space};
use crate::support::pyset::PySet;

/// Whether `ref` is the canonical supported packed dereference.
pub(crate) fn _packed(r#ref: &MemRef) -> bool {
    r#ref.pointer
        && r#ref.base.is_some()
        && r#ref.base_width == 4
        && r#ref.addr.is_none()
        && r#ref.segment.is_none()
}

/// One packed reference with its already-named address components.
pub(crate) fn _reference(r#ref: &MemRef, pieces: &BTreeMap<Value, (Value, Value)>) -> MemRef {
    if !_packed(r#ref) {
        return r#ref.clone();
    }
    let (low, high) = pieces[&r#ref.base.expect("packed")];
    MemRef {
        addr: Some(Addr::new(Space::Far, 0)),
        base: Some(low),
        segment: Some(high),
        space: Some(Space::Far),
        base_width: 2,
        pointer: false,
        ..r#ref.clone()
    }
}

/// Name each packed access's offset and selector as word SSA values.
pub(crate) fn split(body: Rc<MirBody>) -> Rc<MirBody> {
    if !body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .any(|op| op.loads.iter().chain(&op.stores).any(_packed))
    {
        return body;
    }

    let values = ssa::values(&body).collect::<Vec<_>>();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);

    let mut fresh = |at: i64| {
        serial += 1;
        variable += 1;
        Value {
            variable,
            version: 1,
            ..Value::new(serial, at)
        }
    };

    let extract = |source: Value, result: Value, offset: i64, at: i64| Op {
        kind: Kind::Extract,
        args: vec![
            Arg::Held(Held {
                value: source,
                width: 4,
            }),
            Arg::Const(Const::new(offset, 4)),
        ],
        results: vec![Arg::Held(Held {
            value: result,
            width: 2,
        })],
        ..Op::new(
            at,
            OpCode::Synth(Synth::HalfToLow),
            "extract",
            vec![result],
            vec![source],
        )
    };

    let mut changed = false;
    let mut blocks = Vec::new();
    for block in &body.blocks {
        // Reuse within a block immediately.  Dominating instances in other
        // blocks are ordinary redundant expressions for GVN to eliminate.
        let mut pieces = BTreeMap::<Value, (Value, Value)>::new();
        let mut operations = Vec::new();
        for op in &block.ops {
            let mut references = Vec::<MemRef>::new();
            let candidates = op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Cell(cell) => Some(&cell.r#ref),
                    _ => None,
                })
                .chain(op.results.iter().filter_map(|result| match result {
                    Arg::Cell(cell) => Some(&cell.r#ref),
                    _ => None,
                }))
                .chain(&op.loads)
                .chain(&op.stores)
                .chain(op.memory_values.iter().map(|(r#ref, _known)| r#ref));
            for r#ref in candidates {
                if _packed(r#ref) && !references.contains(r#ref) {
                    references.push(r#ref.clone());
                }
            }
            if references.is_empty() {
                operations.push(op.clone());
                continue;
            }

            let mut made = Vec::new();
            for r#ref in &references {
                let base = r#ref.base.expect("packed");
                if pieces.contains_key(&base) {
                    continue;
                }
                let (low, high) = (fresh(op.at), fresh(op.at));
                pieces.insert(base, (low, high));
                made.extend([extract(base, low, 0, op.at), extract(base, high, 16, op.at)]);
            }

            let direct = op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(held) => Some(held.value),
                    _ => None,
                })
                .chain(op.exits.iter().copied())
                .chain(op.merges.keys().copied())
                .collect::<Vec<_>>();
            let packed_values = references
                .iter()
                .filter_map(|r#ref| r#ref.base)
                .collect::<PySet<Value>>();
            let mut uses = Vec::new();
            for value in &op.uses {
                if !packed_values.contains(value) || direct.contains(value) {
                    uses.push(*value);
                }
            }
            for value in packed_values.iter() {
                let (low, high) = pieces[value];
                uses.extend([low, high]);
            }
            let mut unique = Vec::new();
            for value in uses {
                if !unique.contains(&value) {
                    unique.push(value);
                }
            }

            operations.extend(made);
            operations.push(Op {
                uses: unique,
                args: op
                    .args
                    .iter()
                    .map(|arg| match arg {
                        Arg::Cell(cell) => Arg::Cell(Cell {
                            r#ref: _reference(&cell.r#ref, &pieces),
                        }),
                        other => other.clone(),
                    })
                    .collect(),
                results: op
                    .results
                    .iter()
                    .map(|result| match result {
                        Arg::Cell(cell) => Arg::Cell(Cell {
                            r#ref: _reference(&cell.r#ref, &pieces),
                        }),
                        other => other.clone(),
                    })
                    .collect(),
                loads: op
                    .loads
                    .iter()
                    .map(|r#ref| _reference(r#ref, &pieces))
                    .collect(),
                stores: op
                    .stores
                    .iter()
                    .map(|r#ref| _reference(r#ref, &pieces))
                    .collect(),
                memory_values: op
                    .memory_values
                    .iter()
                    .map(|(r#ref, known)| (_reference(r#ref, &pieces), known.clone()))
                    .collect(),
                ..op.clone()
            });
            changed = true;
        }
        blocks.push(block.with_ops(operations));
    }

    if changed {
        Rc::new(MirBody { blocks, ..MirBody::clone(&body) })
    } else {
        body
    }
}
