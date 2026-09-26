//! Port of `qbopt/frontend/raising_conditions.py`.
//!
//! Separate scalar condition reads from their value comparisons.

use crate::analysis::ssa;
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Held, Kind, Op, OpCode, RaisedBody, Value};
use crate::objectfile::module::Space;
use crate::support::hash::IndexSet;

pub fn loaded(body: RaisedBody) -> RaisedBody {
    let values: Vec<Value> = ssa::values(&body).collect();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops = Vec::new();
        for op in &block.ops {
            let cells: Vec<_> = op
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Cell(cell) => Some(cell),
                    _ => None,
                })
                .collect();
            let refused = op.op != Some(OpCode::Operation(Operation::Compare))
                || op.kind != Kind::Sub
                || !op.results.is_empty()
                || op.defines.len() != 1
                || !op.defines[0].flags
                || op.barrier()
                || op.floating.is_some()
                || op.stack.is_some()
                || !op.merges.is_empty()
                || !op.stores.is_empty()
                || op.args.len() != 2
                || cells.len() != 1
                || op.loads != [cells[0].r#ref.clone()]
                || !matches!(cells[0].r#ref.width, 2 | 4)
                || cells[0].r#ref.addr.is_none_or(|addr| !matches!(addr.space, Space::Segment | Space::Frame | Space::Literal))
                || op.args.iter().any(|arg| match arg {
                    Arg::Cell(cell) => cell.r#ref.width != cells[0].r#ref.width,
                    Arg::Held(held) => held.width != cells[0].r#ref.width,
                    Arg::Const(constant) => constant.width != cells[0].r#ref.width,
                    _ => true,
                });
            if refused {
                ops.push(op.clone());
                continue;
            }
            let cell = cells[0].clone();
            let r#ref = cell.r#ref.clone();
            serial += 1;
            variable += 1;
            let value = Value { id: serial, at: op.at, flags: false, variable, version: 1 };
            let held = Held { value, width: r#ref.width };
            let mut load = Op::new(
                op.at,
                OpCode::Operation(Operation::Move),
                "",
                vec![value],
                [r#ref.base, r#ref.segment].into_iter().flatten().collect(),
            );
            load.kind = Kind::Load;
            load.loads = vec![r#ref];
            load.args = vec![Arg::Cell(cell)];
            load.results = vec![Arg::Held(held)];
            load.source = Some(mir::next_id());
            load.symbol = Some(false);
            ops.push(load);
            let args: Vec<Arg> =
                op.args.iter().map(|arg| if matches!(arg, Arg::Cell(_)) { Arg::Held(held) } else { arg.clone() }).collect();
            let mut changed = op.clone();
            changed.uses = args
                .iter()
                .filter_map(|arg| match arg {
                    Arg::Held(held) => Some(held.value),
                    _ => None,
                })
                .collect::<IndexSet<_>>()
                .into_iter()
                .collect();
            changed.args = args;
            changed.loads = Vec::new();
            changed.raised = None;
            ops.push(mir::detached(changed));
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}
