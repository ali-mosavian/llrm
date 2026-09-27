//! Port of `qbopt/frontend/raising_division.py`: a sign-extended word
//! dividend recognised as one scalar signed division.

use std::collections::BTreeMap;

use num_bigint::BigInt;

use crate::model::ir::Operation;
use crate::model::mir::{Arg, Held, Kind, MirBlock, Op, OpCode, RaisedBody, Value};

pub fn scalar(body: RaisedBody) -> RaisedBody {
    let mut definitions: BTreeMap<Value, &Op> = BTreeMap::new();
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.defines {
                definitions.insert(*value, op);
            }
        }
    }

    let raised = |op: &Op| -> Op {
        if op.kind != Kind::Div || op.name != "idiv" || op.args.len() != 3 || op.results.len() != 2 {
            return op.clone();
        }
        let (Arg::Held(high), Arg::Held(low), Arg::Held(divisor)) = (&op.args[0], &op.args[1], &op.args[2]) else {
            return op.clone();
        };
        if !op.args.iter().chain(&op.results).all(|arg| matches!(arg, Arg::Held(held) if held.width == 2)) {
            return op.clone();
        }
        if !op.loads.is_empty() || !op.stores.is_empty() || !_extends(high, low, &definitions) {
            return op.clone();
        }
        // `idiv r16` ties dx:ax to the pair it writes; the lowering builds a
        // fresh high from `cwd`, so only a tie to the two halves is known.
        if !op.merges.keys().all(|was| *was == high.value || *was == low.value) {
            return op.clone();
        }
        let remaining = [low.value, divisor.value];
        let mut made = op.clone();
        made.kind = Kind::Divmod;
        made.args = vec![Arg::Held(*low), Arg::Held(*divisor)];
        made.merges = op.merges.iter().filter(|(was, _)| **was != high.value).map(|(was, now)| (*was, *now)).collect();
        made.uses = op.uses.iter().copied().filter(|value| *value != high.value || remaining.contains(value)).collect();
        made
    };

    let blocks: Vec<MirBlock> = body
        .blocks
        .iter()
        .map(|block| MirBlock { ops: block.ops.iter().map(raised).collect(), ..block.clone() })
        .collect();
    body.with_blocks(blocks)
}

/// Whether `high` is the top word of the sign extension of `low`: BC's
/// `cwd`, or a `movsx` dword whose halves the divide reads by extract.
pub fn _extends(high: &Held, low: &Held, definitions: &BTreeMap<Value, &Op>) -> bool {
    let Some(extension) = definitions.get(&high.value) else {
        return false;
    };
    if extension.op == Some(OpCode::Operation(Operation::Extend))
        && extension.name == "cwd"
        && extension.args == [Arg::Held(*low)]
        && extension.results == [Arg::Held(*high)]
    {
        return true;
    }
    if extension.kind != Kind::Extract
        || extension.results != [Arg::Held(*high)]
        || extension.args.len() != 2
        || !extension.loads.is_empty()
        || !extension.stores.is_empty()
        || extension.barrier()
    {
        return false;
    }
    let (Arg::Held(whole_arg), Arg::Const(offset)) = (&extension.args[0], &extension.args[1]) else {
        return false;
    };
    if whole_arg.width != 4 || offset.n != BigInt::from(16) {
        return false;
    }
    let Some(whole) = definitions.get(&whole_arg.value) else {
        return false;
    };
    whole.kind == Kind::SignExtend
        && whole.args == [Arg::Held(*low)]
        && whole.results == [Arg::Held(*whole_arg)]
        && whole.loads.is_empty()
        && whole.stores.is_empty()
        && !whole.barrier()
}

#[cfg(test)]
#[path = "raising_division_tests.rs"]
mod tests;
