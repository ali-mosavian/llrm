//! Store a whole value instead of separately storing its extracted words.
//!
//! Direct port of `qbopt/optimize/wholestores.py`.

use std::collections::BTreeMap;

use crate::model::ir::Operation;
use crate::model::mir::{
    self, Arg, Cell, Kind, MemRef, MirBlock, MirBody, Op, OpCode, OrderedMap, Value,
};

pub(crate) fn _word(op: &Op) -> bool {
    op.kind == Kind::Store
        && !op.barrier()
        && op.defines.is_empty()
        && op.loads.is_empty()
        && op.args.len() == 1
        && op.stores.len() == 1
        && matches!(&op.args[0], Arg::Held(held) if held.width == 2)
        && op.stores[0].width == 2
        && op.stores[0].addr.is_some()
}

pub(crate) fn joined(body: &MirBody) -> MirBody {
    let definitions = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect::<BTreeMap<Value, &Op>>();
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut ops: Vec<Op> = Vec::new();
        for op in &block.ops {
            if let Some(low) = ops.last() {
                if _word(low) && _word(op) {
                    let r#ref = &low.stores[0];
                    let moved = MemRef {
                        addr: r#ref.addr.map(|addr| addr.plus(2)),
                        ..r#ref.clone()
                    };
                    if moved == op.stores[0] {
                        if let Some(whole) =
                            mir::extracted_whole(&op.args[0], &low.args[0], &definitions)
                        {
                            let r#ref = MemRef {
                                width: 4,
                                ..r#ref.clone()
                            };
                            let mut uses = Vec::new();
                            for value in [Some(whole.value), r#ref.base, r#ref.segment]
                                .into_iter()
                                .flatten()
                            {
                                if !uses.contains(&value) {
                                    uses.push(value);
                                }
                            }
                            let mut absorbed = Vec::new();
                            for one in low.absorbed.iter().chain(&op.absorbed) {
                                if !absorbed.contains(one) {
                                    absorbed.push(*one);
                                }
                            }
                            let replaced = Op {
                                op: Some(OpCode::Operation(Operation::Move)),
                                name: "mov".to_owned(),
                                args: vec![Arg::Held(whole)],
                                results: vec![Arg::Cell(Cell {
                                    r#ref: r#ref.clone(),
                                })],
                                stores: vec![r#ref],
                                uses,
                                merges: OrderedMap::new(),
                                source_backed: false,
                                raised: None,
                                absorbed,
                                ..low.clone()
                            };
                            *ops.last_mut().expect("low") = replaced;
                            continue;
                        }
                    }
                }
            }
            ops.push(op.clone());
        }
        blocks.push(block.with_ops(ops));
    }
    body.with_blocks(blocks)
}
