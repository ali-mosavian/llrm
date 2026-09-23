//! Whether a loop is worth copying out completely, decided before anything is cloned.
//!
//! Port of `qbopt/analysis/peelsize.py`. The decision is GCC's
//! `try_unroll_loop_completely` (gcc/tree-ssa-loop-ivcanon.cc). The size it is
//! given is LLVM's `analyzeLoopUnrollCost`
//! (llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp): each iteration is run over
//! the values it knows, what folds is free, and only the successors a folded
//! branch leaves are followed. That replaces GCC's `tree_estimate_loop_size`
//! guess, which credits a third of what is left as likely to fold.

use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::analysis::consts::{self, Cells, Known};
use crate::analysis::induction::{self, AffineOperand};
use crate::analysis::loops::Loop;
use crate::model::mir::{Arg, Kind, MirBlock, MirBody, Op, Value};
use crate::model::passes::Where;

const _OPAQUE: [Kind; 5] = [Kind::Call, Kind::Opaque, Kind::Escape, Kind::Arg, Kind::Result];

/// GCC's `--param max-peel-branches`: undecided branches a copied sequence may hold.
const MAX_PEEL_BRANCHES: i64 = 16;

/// LLVM's `-unroll-max-percent-threshold-boost`: how far saved work may raise the budget.
const MAX_PERCENT_THRESHOLD_BOOST: i64 = 400;

/// Whether copying `loop` out `count` times pays: GCC's `try_unroll_loop_completely`.
///
/// Past `max-completely-peel-times` iterations nothing is copied, however small the
/// copy would settle: building it is the cost (deedlines' empty 16384-trip loops became
/// 360K operations before they folded). A copy no larger than the loop always pays.
/// Otherwise GCC refuses growth under -Os, with a call on the path (little is left
/// to fold), past `max-peel-branches` undecided branches, and past
/// `max-completely-peeled-insns` operations -- a budget raised, as LLVM's
/// `shouldFullUnroll` raises it, by the share of the rolled work the copy no longer
/// does (`getFullUnrollBoostingFactor`). A loop holding another is copied only when
/// that shrinks it, as GCC does for outer loops.
pub(crate) fn admitted(
    body: &MirBody,
    loop_: &Loop,
    count: &BigInt,
    facts: &IndexMap<Value, Known>,
    r#where: &Where,
) -> bool {
    let limits = &r#where.options;
    if limits.max_unroll_iterations != 0 && *count > BigInt::from(limits.max_unroll_iterations) {
        crate::debug!("unroll", "loop b{} x{count}: refused: max-completely-peel-times", loop_.header);
        return false;
    }
    let (size, folded) = _sizes(body, loop_, facts);
    let blocks = body
        .blocks
        .iter()
        .filter(|block| loop_.body.contains(&block.at))
        .map(|block| (block.at, block))
        .collect::<BTreeMap<i64, &MirBlock>>();
    let (Some(order), Some(count)) = (_ordered(&blocks, loop_.header), count.to_i64()) else {
        let shrinks = count * BigInt::from(size - folded) <= BigInt::from(size);
        crate::debug!("unroll", "loop b{} x{count}: holds a loop, {size} ops, {}", loop_.header, if shrinks { "shrinks" } else { "refused: not innermost and code would grow" });
        return shrinks;
    };
    let budget = if limits.max_unrolled_operations == 0 { i64::MAX } else { limits.max_unrolled_operations };
    let limit = budget.saturating_mul(MAX_PERCENT_THRESHOLD_BOOST) / 100;
    let Some(unrolled) = unrolled(&blocks, &order, loop_, count, facts, r#where, limit.max(size)) else {
        crate::debug!("unroll", "loop b{} x{count}: {size} ops, refused: over {} ops unrolled", loop_.header, limit.max(size));
        return false;
    };
    let boost = _boost(&unrolled);
    // GCC's reasons, in its order.
    let refusal = if unrolled.size <= size {
        None
    } else if !limits.grows {
        Some("size would grow")
    } else if unrolled.calls {
        Some("contains call and code would grow")
    } else if unrolled.branches > MAX_PEEL_BRANCHES {
        Some("max-peel-branches")
    } else if unrolled.size > budget.saturating_mul(boost) / 100 {
        Some("max-completely-peeled-insns")
    } else {
        None
    };
    crate::debug!(
        "unroll",
        "loop b{} x{count}: {size} ops -> {} unrolled ({} rolled, boost {boost}%, {} branches{}), {}",
        loop_.header,
        unrolled.size,
        unrolled.rolled,
        unrolled.branches,
        if unrolled.calls { ", calls" } else { "" },
        refusal.map_or("admitted".to_owned(), |why| format!("refused: {why}"))
    );
    refusal.is_none()
}

/// LLVM's `getFullUnrollBoostingFactor`: the rolled work per unrolled operation, in
/// percent, capped.
fn _boost(unrolled: &Unrolled) -> i64 {
    if unrolled.size == 0 {
        return MAX_PERCENT_THRESHOLD_BOOST;
    }
    (100 * unrolled.rolled / unrolled.size).min(MAX_PERCENT_THRESHOLD_BOOST)
}

/// A complete copy's size, simulated rather than guessed.
struct Unrolled {
    /// Operations no iteration folds, over every iteration.
    size: i64,
    /// Conditional branches no iteration decides, over every iteration.
    branches: i64,
    /// Whether some iteration's path calls out.
    calls: bool,
    /// Operations the rolled loop executes over every iteration: LLVM's `RolledDynamicCost`.
    rolled: i64,
}

/// LLVM's `analyzeLoopUnrollCost`: run each of `count` iterations over the values it
/// knows and the memory it has written, count what does not fold, and follow only the
/// successors a folded branch leaves. `None` once more than `limit` operations remain,
/// where LLVM bails out too.
fn unrolled(
    blocks: &BTreeMap<i64, &MirBlock>,
    order: &[i64],
    loop_: &Loop,
    count: i64,
    facts: &IndexMap<Value, Known>,
    r#where: &Where,
    limit: i64,
) -> Option<Unrolled> {
    use crate::optimize::transform::{_comparison, _outcome, _switch_target};
    let latch = *loop_.latches.first()?;
    let calls = r#where.named();
    let mut out = Unrolled { size: 0, branches: 0, calls: false, rolled: 0 };
    let mut cells = Cells::default();
    let mut previous = facts.clone();
    for iteration in 0..count {
        let mut values = facts.clone();
        let mut carries = IndexMap::<Value, BigInt>::default();
        let mut came = BTreeMap::<i64, Vec<i64>>::from([(loop_.header, Vec::new())]);
        for at in order {
            let Some(from) = came.get(at).cloned() else {
                continue;
            };
            let block = blocks[at];
            for phi in &block.phis {
                // The header's value comes from before the loop, then from the last iteration.
                let incoming = if *at == loop_.header {
                    let source = if iteration == 0 {
                        phi.incoming.iter().find(|(pred, _)| !loop_.body.contains(*pred)).map(|(_, value)| *value)
                    } else {
                        phi.incoming.get(&latch).copied()
                    };
                    source.and_then(|value| previous.get(&value).cloned())
                } else if let [pred] = from.as_slice() {
                    phi.incoming.get(pred).and_then(|value| values.get(value).cloned())
                } else {
                    None
                };
                match incoming {
                    Some(known) => values.insert(phi.result, known),
                    None => values.shift_remove(&phi.result),
                };
            }
            let last = block.ops.last();
            let compare = last.and_then(|last| _comparison(block, last)).map(|(index, _)| index);
            let mut compared = IndexMap::default();
            for (index, op) in block.ops.iter().enumerate() {
                if Some(index) == compare {
                    compared.insert((block.at, index), cells.clone());
                }
                // A compare is free when the branch reading it is decided, and counted there if not.
                if op.kind != Kind::Nothing {
                    out.rolled += 1;
                }
                if matches!(op.kind, Kind::Nothing | Kind::Jump | Kind::Branch | Kind::Switch) || Some(index) == compare {
                    continue;
                }
                if let Some(carry) = consts::_carry(op, &values, &cells) {
                    for value in op.defines.iter().filter(|value| value.flags) {
                        carries.insert(*value, carry.clone());
                    }
                }
                let defined = consts::_defined(op);
                let result = defined
                    .filter(|_| _folds(op))
                    .and_then(|_| consts::_result(op, &values, Some(&cells), Some(&carries)));
                match (defined, result) {
                    (Some(value), Some(known)) => {
                        values.insert(value, known);
                    }
                    (defined, _) => {
                        if let Some(value) = defined {
                            values.shift_remove(&value);
                        }
                        out.size += 1;
                        out.calls |= op.kind == Kind::Call;
                    }
                }
                cells = consts::_kills(cells, op, &values, &r#where.dgroup, &calls, None, None, false, None);
            }
            let decided = match last {
                Some(last) if last.kind == Kind::Branch && block.succ.len() == 2 => {
                    _outcome(block, last, &values, &compared, None).map(|taken| {
                        let target = last.target.expect("a branch has a target");
                        block.succ.iter().copied().filter(|at| (*at == target) == taken).collect::<Vec<_>>()
                    })
                }
                Some(last) if last.kind == Kind::Switch => _switch_target(last, &values).map(|target| vec![target]),
                _ => Some(block.succ.clone()),
            };
            let successors = decided.unwrap_or_else(|| {
                out.branches += 1;
                out.size += 1 + i64::from(compare.is_some());
                block.succ.clone()
            });
            for successor in successors {
                if successor != loop_.header && loop_.body.contains(&successor) {
                    came.entry(successor).or_default().push(block.at);
                }
            }
            if out.size > limit {
                return None;
            }
        }
        previous = values;
    }
    Some(out)
}

/// The loop's blocks, each after every block reaching it inside one iteration; `None`
/// when the loop holds another.
fn _ordered(blocks: &BTreeMap<i64, &MirBlock>, header: i64) -> Option<Vec<i64>> {
    let inner = |at: &i64| *at != header && blocks.contains_key(at);
    let mut waiting = blocks.keys().map(|at| (*at, 0)).collect::<BTreeMap<i64, usize>>();
    for block in blocks.values() {
        for successor in block.succ.iter().filter(|at| inner(at)) {
            *waiting.get_mut(successor).expect("inside") += 1;
        }
    }
    let mut ready = vec![header];
    let mut order = Vec::new();
    while let Some(at) = ready.pop() {
        order.push(at);
        for successor in blocks[&at].succ.iter().filter(|at| inner(at)) {
            let left = waiting.get_mut(successor).expect("inside");
            *left -= 1;
            if *left == 0 {
                ready.push(*successor);
            }
        }
    }
    (order.len() == blocks.len()).then_some(order)
}

/// Whether an operation's result is a function of its inputs alone, memory it reads included.
fn _folds(op: &Op) -> bool {
    op.stores.is_empty() && !op.barrier() && !_OPAQUE.contains(&op.kind) && op.floating.is_none()
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
