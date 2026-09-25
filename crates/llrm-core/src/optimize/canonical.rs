//! Canonical forms, as LLVM's InstCombine puts them: compares with the
//! constant on the right, no neutral terms.
//!
//! Port of `qbopt/optimize/canonical.py`. Folding makes `1 sub v` whenever a
//! compare's left operand becomes known, and no machine compares an
//! immediate against a register in that order. The compare is swapped and
//! every test reading its flags mirrored; a reader that is not a test gets
//! the constant as a copied value instead.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;

use crate::analysis::{consts, ssa};
use crate::model::ir::Operation;
use crate::model::mir::{self, Arg, Held, Kind, MirBody, Op, OpCode, Value};

pub fn compares(body: Rc<MirBody>) -> Rc<MirBody> {
    let mut readers = IndexMap::<Value, Vec<&Op>>::default();
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
        blocks.push(block.with_ops(ops));
    }
    Rc::new(MirBody { blocks, ..MirBody::clone(&body) })
}

fn _constant_left(op: &Op) -> bool {
    op.kind == Kind::Sub
        && op.results.is_empty()
        && op.defines.len() == 1
        && op.args.len() == 2
        && matches!(op.args[0], Arg::Const(_))
}

/// `x + 0`, `x - 0` and `x * 1` are `x`, and a test `x <=u 0` is `x == 0`.
///
/// A rewrite states what it computes from a proof in full -- rotation's
/// trip count is `bound - start + inclusive` for any start -- and the
/// neutral terms go here, so no rewrite folds its own.
pub fn identities(body: Rc<MirBody>) -> Rc<MirBody> {
    let mut read = BTreeSet::<Value>::new();
    for block in &body.blocks {
        read.extend(block.ops.iter().flat_map(|op| op.uses.iter().copied()));
        read.extend(block.phis.iter().flat_map(|phi| phi.incoming.values().copied()));
    }
    let mut swap = BTreeMap::<u32, Value>::new();
    // `id(op)`: the block and operation index.
    let mut copies = BTreeMap::<(usize, usize), Op>::new();
    let mut zero_tests = BTreeSet::<Value>::new();
    for (b, block) in body.blocks.iter().enumerate() {
        for (i, op) in block.ops.iter().enumerate() {
            if _zero_test(op) {
                zero_tests.insert(op.defines[0]);
            }
            let Some(kept) = _neutral(op) else { continue };
            if op.defines.iter().any(|value| value.flags && read.contains(value)) {
                continue;
            }
            let Arg::Held(result) = &op.results[0] else { unreachable!("_neutral checked a held result") };
            if let Arg::Held(held) = &kept {
                swap.insert(result.value.id, held.value);
            } else {
                let mut copy = op.clone();
                copy.kind = Kind::Copy;
                copy.defines = vec![result.value];
                copy.uses = Vec::new();
                copy.args = vec![kept];
                copy.source_backed = false;
                copy.raised = None;
                copy.symbol = Some(false);
                copies.insert((b, i), copy);
            }
        }
    }
    let renamed_test = |op: &Op| {
        op.kind == Kind::Branch
            && matches!(op.test, Some(Kind::BelowEq | Kind::Above))
            && op.uses.iter().any(|value| zero_tests.contains(value))
    };
    let renamed = body.blocks.iter().flat_map(|block| &block.ops).any(|op| renamed_test(op));
    if swap.is_empty() && copies.is_empty() && !renamed {
        return body;
    }
    let mut blocks = Vec::new();
    for (b, block) in body.blocks.iter().enumerate() {
        let mut ops = Vec::new();
        for (i, op) in block.ops.iter().enumerate() {
            if let Some(Arg::Held(result)) = op.results.first() {
                if swap.contains_key(&result.value.id) {
                    if op.id.is_some() || op.source_backed || !op.absorbed.is_empty() {
                        ops.push(mir::cleared(op)); // its bytes stay owned
                    }
                    continue;
                }
            }
            let mut op = ssa::substituted(copies.get(&(b, i)).unwrap_or(op), &swap).expect("an acyclic substitution");
            if renamed_test(&op) {
                // Nothing is below zero.
                op.test = Some(if op.test == Some(Kind::BelowEq) { Kind::Eq } else { Kind::Ne });
            }
            ops.push(op);
        }
        let mut block = block.clone();
        for phi in &mut block.phis {
            phi.incoming = phi
                .incoming
                .iter()
                .map(|(at, value)| (*at, ssa::provider(*value, &swap).expect("an acyclic substitution")))
                .collect();
        }
        block.ops = ops;
        blocks.push(block);
    }
    Rc::new(MirBody { blocks, ..MirBody::clone(&body) })
}

/// The operand a pure `x + 0`, `x - 0` or `x * 1` passes through unchanged.
fn _neutral(op: &Op) -> Option<Arg> {
    if op.args.len() != 2
        || op.results.len() != 1
        || !matches!(op.results[0], Arg::Held(_))
        || !op.loads.is_empty()
        || !op.stores.is_empty()
        || !op.merges.is_empty()
        || op.barrier()
        || op.floating.is_some()
    {
        return None;
    }
    let Arg::Held(result) = &op.results[0] else { return None };
    let width = result.width;
    let identity = match op.kind {
        Kind::Add | Kind::Sub => 0,
        Kind::Mul => 1,
        _ => return None,
    };
    let (left, right) = (&op.args[0], &op.args[1]);
    let pairs = [(left, right), (right, left)];
    let commutes = op.kind != Kind::Sub;
    for (kept, other) in &pairs[..1 + usize::from(commutes)] {
        let Arg::Const(other) = other else { continue };
        if consts::masked(&other.n, width) != identity.into() {
            continue;
        }
        match kept {
            Arg::Held(held) if held.width == width => return Some((*kept).clone()),
            Arg::Const(constant) if constant.width == width => return Some((*kept).clone()),
            _ => {}
        }
    }
    None
}

fn _zero_test(op: &Op) -> bool {
    op.kind == Kind::Sub
        && op.results.is_empty()
        && op.defines.len() == 1
        && op.args.len() == 2
        && matches!(&op.args[1], Arg::Const(constant) if consts::masked(&constant.n, constant.width) == 0.into())
}

#[cfg(test)]
#[path = "canonical_tests.rs"]
mod tests;
