//! How large a completely peeled loop will be, before anything is cloned.
//!
//! Port of `qbopt/analysis/peelsize.py`. GCC's `tree_estimate_loop_size` and
//! `estimated_unrolled_size` (tree-ssa-loop-ivcanon.cc): an operation whose
//! operands are all constant once the iteration is fixed folds away in every
//! copy; the rest is copied once per iteration.

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::analysis::consts::Known;
use crate::analysis::induction::{self, AffineOperand};
use crate::analysis::loops::Loop;
use crate::model::mir::{Arg, Kind, MirBody, Op, Value};
use crate::model::passes::Where;

const _OPAQUE: [Kind; 5] = [Kind::Call, Kind::Opaque, Kind::Escape, Kind::Arg, Kind::Result];

/// Python's signature tuple: each part rendered once, compared whole.
pub(crate) type Signature = Vec<String>;

/// Whether a `count`-fold copy of `loop` can pass the target's peel budget.
pub(crate) fn admitted(
    body: &MirBody,
    loop_: &Loop,
    count: &BigInt,
    facts: &IndexMap<Value, Known>,
    r#where: &Where,
) -> bool {
    let (size, folded) = _sizes(body, loop_, facts);
    let copied = count * BigInt::from(size - folded);
    if copied <= BigInt::from(size) {
        return true;
    }
    if !r#where.options.grows {
        return false;
    }
    let limits = &r#where.options;
    if limits.max_unroll_iterations != 0 && *count > BigInt::from(limits.max_unroll_iterations) {
        return false;
    }
    // GCC credits a third of what is left as likely to fold after all.
    limits.max_unrolled_operations == 0
        || &copied - induction::floor_div(&copied, &BigInt::from(3)) <= BigInt::from(limits.max_unrolled_operations)
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
        .flat_map(|block| block.ops.iter())
        .filter(|op| op.kind != Kind::Nothing)
        .collect::<Vec<_>>();
    // `id(op)`: the position in `ops`.
    let mut folded = BTreeSet::<usize>::new();
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
        .flat_map(|block| block.phis.iter())
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

/// The loop as a candidate sees it, with incidental value numbering removed.
///
/// Two rounds of the fixed point renumber every value; the same loop, with
/// the same constants reaching it, is the same candidate and gets the same
/// answer. A constant newly reaching it -- after its outer loop is peeled --
/// makes it a different one.
pub(crate) fn signature(body: &MirBody, loop_: &Loop, count: &BigInt, facts: &IndexMap<Value, Known>) -> Signature {
    let mut names = HashMap::<u32, usize>::new();

    let mut value = |one: &Value| -> (usize, Option<(BigInt, u32)>) {
        let next = names.len();
        let name = *names.entry(one.id).or_insert(next);
        (name, facts.get(one).map(|fact| (fact.n.clone(), fact.width)))
    };

    let arg = |one: &Arg, value: &mut dyn FnMut(&Value) -> (usize, Option<(BigInt, u32)>)| -> String {
        match one {
            Arg::Held(held) => format!("{:?}", (value(&held.value), held.width)),
            Arg::Cell(cell) => {
                let reference = &cell.r#ref;
                let reached = [reference.base, reference.segment]
                    .iter()
                    .map(|part| part.as_ref().map(&mut *value))
                    .collect::<Vec<_>>();
                format!("{:?}", (reference.addr, reference.width, reached))
            }
            _ => format!("{one:?}"),
        }
    };

    let mut parts: Signature = vec![count.to_string()];
    for block in &body.blocks {
        if !loop_.body.contains(&block.at) {
            continue;
        }
        let phis = block
            .phis
            .iter()
            .map(|phi| {
                let result = value(&phi.result);
                let mut incoming = phi.incoming.values().map(&mut value).collect::<Vec<_>>();
                incoming.sort();
                (result, incoming)
            })
            .collect::<Vec<_>>();
        parts.push(format!("{phis:?}"));
        for op in &block.ops {
            if op.kind == Kind::Nothing {
                continue;
            }
            let args = op.args.iter().map(|one| arg(one, &mut value)).collect::<Vec<_>>();
            let results = op.results.iter().map(|one| arg(one, &mut value)).collect::<Vec<_>>();
            parts.push(format!("{:?}", (op.kind, args, results)));
        }
    }
    parts
}
