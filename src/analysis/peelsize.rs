//! How large a completely peeled loop will be, before anything is cloned.
//!
//! GCC's `tree_estimate_loop_size` and `estimated_unrolled_size`
//! (tree-ssa-loop-ivcanon.cc): an operation whose operands are all constant once
//! the iteration is fixed -- constants, counters with a constant start and step,
//! and what those compute -- folds away in every copy; the rest is copied once
//! per iteration. Unroll and peel both ask this first. Building and optimizing a
//! candidate only to reject it cost modern nbody 14 times its compile time.
//!
//! Direct port of `qbopt/analysis/peelsize.py`.
// Unroll and peel are its only callers; their sync lands separately.
#![allow(dead_code)]

use std::collections::BTreeSet;

use indexmap::IndexMap;
use num_bigint::BigInt;

use super::consts::Known;
use super::induction::{self, AffineOperand};
use super::loops::Loop;
use crate::model::mir::{Arg, Kind, MirBody, Op, Value};
use crate::model::passes::Where;
use crate::support::pyrepr::Repr;

const _OPAQUE: [Kind; 5] = [Kind::Call, Kind::Opaque, Kind::Escape, Kind::Arg, Kind::Result];

/// Whether a `count`-fold copy of `loop` can pass the target's peel budget.
pub(crate) fn admitted(body: &MirBody, loop_: &Loop, count: i64, facts: &IndexMap<Value, Known>, r#where: &Where) -> bool {
    let (size, folded) = _sizes(body, loop_, facts);
    let copied = count * (size - folded);
    if copied <= size {
        return true;
    }
    if !r#where.options.grows {
        return false;
    }
    let limits = &r#where.options;
    if limits.max_unroll_iterations != 0 && count > limits.max_unroll_iterations {
        return false;
    }
    // GCC credits a third of what is left as likely to fold after all.
    limits.max_unrolled_operations == 0 || copied - copied.div_euclid(3) <= limits.max_unrolled_operations
}

/// The loop's operations, and how many of them fold once the iteration is fixed.
fn _sizes(body: &MirBody, loop_: &Loop, facts: &IndexMap<Value, Known>) -> (i64, i64) {
    let inside = body.blocks.iter().filter(|block| loop_.body.contains(&block.at)).collect::<Vec<_>>();
    let mut known = induction::basics(body, loop_)
        .values()
        .filter(|one| _constant(&one.start, facts) && _constant(&one.step, facts))
        .map(|one| one.value)
        .collect::<BTreeSet<u32>>();
    known.extend(facts.keys().map(|value| value.id));
    let ops = inside
        .iter()
        .flat_map(|block| &block.ops)
        .filter(|op| op.kind != Kind::Nothing)
        .collect::<Vec<_>>();
    // `id(op)`: each operation's position in `ops`.
    let mut folded = BTreeSet::new();
    let mut changed = true;
    while changed {
        changed = false;
        for (index, op) in ops.iter().enumerate() {
            if folded.contains(&index) || !_pure(op) {
                continue;
            }
            if op.uses.iter().all(|value| known.contains(&value.id)) {
                folded.insert(index);
                known.extend(op.defines.iter().map(|value| value.id));
                changed = true;
            }
        }
    }
    let phis = inside.iter().map(|block| block.phis.len()).sum::<usize>();
    let folded_phis = inside
        .iter()
        .flat_map(|block| &block.phis)
        .filter(|phi| known.contains(&phi.result.id))
        .count();
    ((ops.len() + phis) as i64, (folded.len() + folded_phis) as i64)
}

fn _constant(arg: &AffineOperand, facts: &IndexMap<Value, Known>) -> bool {
    match arg {
        AffineOperand::Const(_) => true,
        AffineOperand::Held(held) => facts.contains_key(&held.value),
    }
}

fn _pure(op: &Op) -> bool {
    !(!op.loads.is_empty() || !op.stores.is_empty() || op.barrier() || _OPAQUE.contains(&op.kind))
}

/// One value as `signature` names it: its renumbering, and its constant if known.
pub(crate) type Named = (usize, Option<(BigInt, u32)>);

/// One operand as `signature` sees it.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Operand {
    Held(Named, u32),
    Cell(String, u32, (Option<Named>, Option<Named>)),
    Other(String),
}

/// One element of the tuple `signature` returns.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Part {
    Count(i64),
    Phis(Vec<(Named, Vec<Named>)>),
    Op(Kind, Vec<Operand>, Vec<Operand>),
}

/// The loop as a candidate sees it, with incidental value numbering removed.
///
/// Two rounds of the fixed point renumber every value; the same loop, with
/// the same constants reaching it, is the same candidate and gets the same
/// answer. A constant newly reaching it -- after its outer loop is peeled --
/// makes it a different one.
pub(crate) fn signature(body: &MirBody, loop_: &Loop, count: i64, facts: &IndexMap<Value, Known>) -> Vec<Part> {
    let mut names: IndexMap<u32, usize> = IndexMap::new();

    let mut value = |one: &Value| -> Named {
        let next = names.len();
        let name = *names.entry(one.id).or_insert(next);
        (name, facts.get(one).map(|fact| (fact.n.clone(), fact.width)))
    };

    let mut parts = vec![Part::Count(count)];
    for block in &body.blocks {
        if !loop_.body.contains(&block.at) {
            continue;
        }
        let mut phis = Vec::new();
        for phi in &block.phis {
            let result = value(&phi.result);
            let mut incoming = phi.incoming.values().map(&mut value).collect::<Vec<_>>();
            incoming.sort();
            phis.push((result, incoming));
        }
        parts.push(Part::Phis(phis));
        for op in block.ops.iter().filter(|op| op.kind != Kind::Nothing) {
            let mut arg = |one: &Arg| match one {
                Arg::Held(held) => Operand::Held(value(&held.value), held.width),
                Arg::Cell(cell) => {
                    let reference = &cell.r#ref;
                    let reached = (reference.base.as_ref().map(&mut value), reference.segment.as_ref().map(&mut value));
                    Operand::Cell(reference.addr.repr(), reference.width, reached)
                }
                other => Operand::Other(other.repr()),
            };
            let args = op.args.iter().map(&mut arg).collect();
            let results = op.results.iter().map(&mut arg).collect();
            parts.push(Part::Op(op.kind, args, results));
        }
    }
    parts
}
