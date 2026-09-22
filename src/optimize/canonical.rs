//! Port of `qbopt/optimize/canonical.py`.
//!
//! Compares with the constant on the right, as LLVM's InstCombine puts them.
//!
//! Folding makes `1 sub v` whenever a compare's left operand becomes known, and
//! no machine compares an immediate against a register in that order. The
//! compare is swapped and every test reading its flags mirrored; a reader that
//! is not a test gets the constant as a copied value instead.

// ---- early port (agent C) ----

use std::collections::{BTreeSet, HashMap};

use crate::analysis::ssa;
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Kind, MirBlock, MirBody, Op, OpCode, Value};

pub fn compares(body: &MirBody) -> MirBody {
    let mut readers: HashMap<Value, Vec<&Op>> = HashMap::new();
    for block in &body.blocks {
        for op in &block.ops {
            for value in &op.uses {
                readers.entry(*value).or_default().push(op);
            }
        }
    }
    let mut swapped: BTreeSet<Value> = BTreeSet::new();
    // Python's `id(op)`: an operation's position in the body.
    let mut copied: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (b, block) in body.blocks.iter().enumerate() {
        for (o, op) in block.ops.iter().enumerate() {
            if !_constant_left(op) {
                continue;
            }
            let tests = readers.get(&op.defines[0]).map_or(true, |reading| {
                reading.iter().all(|one| {
                    one.kind == Kind::Branch && one.test.is_some_and(|test| mir::MIRRORED.contains_key(&test))
                })
            });
            if tests && !matches!(op.args[1], Arg::Const(_)) {
                swapped.insert(op.defines[0]);
            } else {
                copied.insert((b, o));
            }
        }
    }
    if swapped.is_empty() && copied.is_empty() {
        return body.clone();
    }

    let values: Vec<Value> = ssa::values(body).collect();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut blocks = Vec::new();
    for (b, block) in body.blocks.iter().enumerate() {
        let mut ops = Vec::new();
        for (o, op) in block.ops.iter().enumerate() {
            if _constant_left(op) && swapped.contains(&op.defines[0]) {
                ops.push(Op { args: vec![op.args[1].clone(), op.args[0].clone()], ..op.clone() });
            } else if copied.contains(&(b, o)) {
                serial += 1;
                variable += 1;
                let held = Value { variable, version: 1, ..Value::new(serial, op.at) };
                let Arg::Const(constant) = &op.args[0] else {
                    unreachable!("_constant_left")
                };
                let width = constant.width;
                ops.push(Op {
                    kind: Kind::Copy,
                    args: vec![op.args[0].clone()],
                    results: vec![Arg::Held(mir::Held { value: held, width })],
                    ..Op::new(op.at, Some(OpCode::Operation(Operation::Move)), "", vec![held], Vec::new())
                });
                ops.push(Op {
                    uses: std::iter::once(held).chain(op.uses.iter().copied()).collect(),
                    args: std::iter::once(Arg::Held(mir::Held { value: held, width }))
                        .chain(op.args[1..].iter().cloned())
                        .collect(),
                    ..op.clone()
                });
            } else if op.kind == Kind::Branch && op.uses.iter().any(|value| swapped.contains(value)) {
                ops.push(Op { test: op.test.map(|test| mir::MIRRORED[&test]), ..op.clone() });
            } else {
                ops.push(op.clone());
            }
        }
        blocks.push(MirBlock { ops, ..block.clone() });
    }
    MirBody { blocks, ..body.clone() }
}

fn _constant_left(op: &Op) -> bool {
    op.kind == Kind::Sub
        && op.results.is_empty()
        && op.defines.len() == 1
        && op.args.len() == 2
        && matches!(op.args[0], Arg::Const(_))
}
