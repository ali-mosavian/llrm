//! Compares with the constant on the right, as LLVM's InstCombine puts them.
//!
//! Port of `qbopt/optimize/canonical.py`. Folding makes `1 sub v` whenever a
//! compare's left operand becomes known, and no machine compares an
//! immediate against a register in that order. The compare is swapped and
//! every test reading its flags mirrored; a reader that is not a test gets
//! the constant as a copied value instead.

use std::collections::BTreeSet;

use indexmap::IndexMap;

use crate::analysis::ssa;
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Held, Kind, MirBody, Op, OpCode, Value};

pub(crate) fn compares(body: MirBody) -> MirBody {
    let mut readers = IndexMap::<Value, Vec<&Op>>::new();
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.uses {
                readers.entry(*value).or_default().push(op);
            }
        }
    }
    let mut swapped = BTreeSet::<Value>::new();
    // `id(op)`: the block and operation index.
    let mut copied = BTreeSet::<(usize, usize)>::new();
    for (b, block) in body.blocks.iter().enumerate() {
        for (i, op) in block.ops.iter().enumerate() {
            if !_constant_left(op) {
                continue;
            }
            let tests = readers.get(&op.defines[0]).map_or(&[][..], Vec::as_slice).iter().all(|one| {
                one.kind == Kind::Branch && one.test.is_some_and(|test| mir::MIRRORED(test).is_some())
            });
            if tests && !matches!(op.args[1], Arg::Const(_)) {
                swapped.insert(op.defines[0]);
            } else {
                copied.insert((b, i));
            }
        }
    }
    if swapped.is_empty() && copied.is_empty() {
        return body;
    }

    let values = ssa::values(&body).collect::<Vec<_>>();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut blocks = Vec::new();
    for (b, block) in body.blocks.iter().enumerate() {
        let mut ops = Vec::new();
        for (i, op) in block.ops.iter().enumerate() {
            if _constant_left(op) && swapped.contains(&op.defines[0]) {
                let mut changed = op.clone();
                changed.args = vec![op.args[1].clone(), op.args[0].clone()];
                ops.push(changed);
            } else if copied.contains(&(b, i)) {
                serial += 1;
                variable += 1;
                let held = Value { variable, version: 1, ..Value::new(serial, op.at) };
                let Arg::Const(constant) = &op.args[0] else {
                    unreachable!("a constant left operand");
                };
                let mut copy = Op::new(op.at, OpCode::Operation(Operation::Move), "", vec![held], Vec::new());
                copy.kind = Kind::Copy;
                copy.args = vec![Arg::Const(constant.clone())];
                copy.results = vec![Arg::Held(Held { value: held, width: constant.width })];
                ops.push(copy);
                let mut changed = op.clone();
                changed.uses = std::iter::once(held).chain(op.uses.iter().copied()).collect();
                changed.args = std::iter::once(Arg::Held(Held { value: held, width: constant.width }))
                    .chain(op.args[1..].iter().cloned())
                    .collect();
                ops.push(changed);
            } else if op.kind == Kind::Branch && op.uses.iter().any(|value| swapped.contains(value)) {
                let mut changed = op.clone();
                changed.test = op.test.map(|test| mir::MIRRORED(test).expect("KeyError: a mirrored test"));
                ops.push(changed);
            } else {
                ops.push(op.clone());
            }
        }
        let mut block = block.clone();
        block.ops = ops;
        blocks.push(block);
    }
    MirBody { blocks, ..body }
}

fn _constant_left(op: &Op) -> bool {
    op.kind == Kind::Sub
        && op.results.is_empty()
        && op.defines.len() == 1
        && op.args.len() == 2
        && matches!(op.args[0], Arg::Const(_))
}
