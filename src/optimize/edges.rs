//! Port of `qbopt/optimize/edges.py`: place semantic operations on
//! conditional edges, not on either arm.

use crate::model::ir::Operation;
use crate::model::mir::{Kind, MirBlock, MirBody, Op, OpCode};

pub fn explicit(block: &MirBlock, target: i64) -> bool {
    conditional(block, target) && block.ops.last().and_then(|op| op.target) == Some(target)
}

pub fn conditional(block: &MirBlock, target: i64) -> bool {
    block.succ.len() == 2
        && block.succ.contains(&target)
        && block.ops.last().is_some_and(|last| {
            last.kind == Kind::Branch && last.target.is_some_and(|at| block.succ.contains(&at))
        })
}

pub fn fresh(body: &MirBody) -> i64 {
    ((body.entry + 1) << 32).max(body.blocks.iter().map(|block| block.at).max().expect("max() arg is an empty sequence"))
        + 1
}

pub fn split(body: &MirBody, source: i64, target: i64, label: i64, ops: Vec<Op>) -> Result<MirBody, String> {
    let parent = body.block(source);
    if parent.is_none_or(|parent| !conditional(parent, target)) || body.block(label).is_some() {
        return Err("edge split does not identify a fresh conditional edge".into());
    }
    let mut jump = Op::new(label, OpCode::Operation(Operation::Jump), "", vec![], vec![]);
    jump.kind = Kind::Jump;
    jump.target = Some(target);
    jump.symbol = Some(false);
    let mut bridge_ops = ops;
    bridge_ops.push(jump);
    let bridge = MirBlock::new(label, vec![], bridge_ops, vec![target]);
    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        if block.at == source {
            block.succ = block.succ.iter().map(|&at| if at == target { label } else { at }).collect();
            let last = block.ops.last_mut().expect("a conditional edge ends in a branch");
            if last.target == Some(target) {
                last.target = Some(label);
            }
        }
        if block.at == target {
            for phi in &mut block.phis {
                phi.incoming = phi
                    .incoming
                    .iter()
                    .map(|(&at, &value)| (if at == source { label } else { at }, value))
                    .collect();
            }
        }
        blocks.push(block);
    }
    blocks.push(bridge);
    Ok(MirBody { blocks, ..body.clone() })
}
