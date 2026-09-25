//! Specialize exact finite recurrences to one checked final iteration.
//!
//! Direct port of `qbopt/optimize/floatloop.py`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use crate::support::hash::IndexMap;
use num_bigint::BigInt;

use crate::analysis::{consts, floatfacts, induction, loops, regions, ssa};
use crate::model::mir::{self, Arg, Cell, Const, Held, Kind, MemRef, MirBlock, MirBody, Op, Value};
use crate::optimize::{strength, transform};

/// Direct port of `qbopt/optimize/floatloop.py:specialized`.
pub fn specialized(
    body: &Rc<MirBody>,
    dgroup: &BTreeSet<i64>,
    calls: &IndexMap<i64, String>,
) -> Result<Rc<MirBody>, String> {
    let proofs = floatfacts::loop_exits(body, dgroup, calls)
        .into_iter()
        .filter(|proof| proof.count > BigInt::from(1))
        .map(|proof| (proof.header, proof))
        .collect::<IndexMap<_, _>>();
    if proofs.is_empty() {
        return Ok(body.clone());
    }
    let blocks = body.blocks.iter().map(|block| (block.at, block)).collect::<IndexMap<_, _>>();
    let predecessors = loops::predecessors(&body.blocks);
    let facts = consts::known(body, Some(dgroup), Some(calls), None, None);
    let memory = floatfacts::cells(body, dgroup, calls);
    for loop_ in loops::loops(&body.blocks, Some(body.entry)) {
        let Some(proof) = proofs.get(&loop_.header) else {
            continue;
        };
        let header = blocks[&loop_.header];
        // Python's `next(iter(loop.latches))` reads a set; the lowest latch
        // stands in for CPython's order.
        let latch = blocks[loop_.latches.iter().next().expect("a loop has a latch")];
        // Python's `exit_at, = [...]` raises unless exactly one matches.
        let exit_at = match header.succ.iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>()[..] {
            [at] => at,
            [] => return Err("not enough values to unpack (expected 1, got 0)".to_string()),
            _ => return Err("too many values to unpack (expected 1)".to_string()),
        };
        if predecessors[&exit_at] != BTreeSet::from([header.at]) || !blocks[&exit_at].phis.is_empty() {
            continue;
        }
        let counters = induction::basics(body, &loop_);
        let live = transform::live(body);
        let phis = header.phis.iter().filter(|phi| live.contains(&phi.result)).collect::<Vec<_>>();
        if phis.len() != 1 || !counters.contains_key(&phis[0].result.id) {
            continue;
        }
        let phi = phis[0];
        let counter = counters.get(&phi.result.id).expect("a counter");
        let width = counter.start.width();
        let start = induction::_signed(&counter.start.as_arg(), &facts, width);
        let step = induction::_signed(&counter.step.as_arg(), &facts, width);
        let (Some(start), Some(step)) = (start, step) else {
            continue;
        };
        let active = latch.ops.iter().filter(|op| op.kind != Kind::Nothing).collect::<Vec<_>>();
        if active.is_empty() || active[0].kind != Kind::Fload || active[0].floating.is_none() {
            continue;
        }
        let checkpoint = active[0];
        if checkpoint.loads.iter().any(|read| {
            proof
                .stores
                .iter()
                .any(|(written, _)| regions::overlapping(read, written, None, None, None).unwrap_or(true))
        }) {
            continue;
        }
        let update = *phi.incoming.get(&latch.at).expect("the latch edge");
        if active[1..].iter().any(|op| {
            op.floating.is_none()
                && !floatfacts::checkpoint(op)
                && (!op.defines.contains(&update) || !op.loads.is_empty() || !op.stores.is_empty())
        }) {
            continue;
        }
        if active.iter().any(|op| op.floating.is_some() && op.uses.contains(&phi.result)) {
            continue;
        }
        let counted = vec![Arg::Held(Held { value: phi.result, width })];
        if header.ops.iter().any(|op| !op.stores.is_empty() && op.args != counted) {
            continue;
        }
        let internal = [header, latch]
            .iter()
            .flat_map(|block| block.ops.iter().flat_map(|op| op.defines.iter().copied()))
            .collect::<BTreeSet<_>>();
        let leaving = transform::_leaving(body);
        if internal.iter().chain([&phi.result]).any(|value| leaving.contains(value)) {
            continue;
        }
        let outside = body.blocks.iter().filter(|block| !loop_.body.contains(&block.at)).collect::<Vec<_>>();
        if outside
            .iter()
            .any(|block| block.ops.iter().any(|op| op.uses.iter().any(|value| internal.contains(value))))
        {
            continue;
        }
        if outside.iter().any(|block| {
            block.phis.iter().any(|join| {
                join.incoming
                    .values()
                    .any(|value| internal.contains(value) || *value == phi.result)
            })
        }) {
            continue;
        }
        let entry_at = match predecessors[&header.at]
            .iter()
            .copied()
            .filter(|at| !loop_.body.contains(at))
            .collect::<Vec<_>>()[..]
        {
            [at] => at,
            [] => return Err("not enough values to unpack (expected 1, got 0)".to_string()),
            _ => return Err("too many values to unpack (expected 1)".to_string()),
        };
        let entry = blocks[&entry_at];
        let last = entry.ops.len().checked_sub(1).expect("the entry has an operation");
        let mut asked = consts::memory_queries(body, &facts, dgroup);
        let initial = consts::_kills(
            (*memory[&(entry_at, last)]).clone(),
            &entry.ops[last],
            &facts,
            dgroup,
            calls,
            None,
            None,
            false,
            Some(&mut asked),
        );
        let before_last = floatfacts::repeated(
            &latch.ops,
            &(&proof.count - BigInt::from(1)),
            &initial,
            dgroup,
            Some(&facts),
            Some(&mut asked),
        );
        let Some(before_last) = before_last else {
            continue;
        };
        let mut seeds = Vec::new();
        let mut broke = false;
        for (reference, _) in &proof.stores {
            if !_carried(reference, &latch.ops, dgroup) {
                continue;
            }
            let Some(fact) = consts::_cell(&before_last, reference) else {
                broke = true;
                break;
            };
            let owner = latch
                .ops
                .iter()
                .find(|op| op.stores.contains(reference))
                .expect("a store owns the reference");
            let mut seed = _store(owner, reference, Const::new(fact.n.clone(), fact.width));
            seed.at = checkpoint.at;
            seed.absorbed = Vec::new();
            seeds.push(seed);
        }
        if !broke {
            let final_ = Const::new(consts::masked(&(start + step * &proof.count), width), width);
            return _rewritten(body, header, latch, exit_at, checkpoint, seeds, phi.result, final_).map(Rc::new);
        }
    }
    Ok(body.clone())
}

/// Direct port of `qbopt/optimize/floatloop.py:_carried`.  `dgroup` is
/// regions' `layout=None`, as in `floatfacts`.
fn _carried(reference: &MemRef, ops: &[Op], _dgroup: &BTreeSet<i64>) -> bool {
    for op in ops {
        if op
            .loads
            .iter()
            .any(|read| regions::overlapping(reference, read, None, None, None).unwrap_or(true))
        {
            return true;
        }
        if op.stores.iter().any(|written| mir::same_bytes(reference, written)) {
            return false;
        }
    }
    false
}

/// Direct port of `qbopt/optimize/floatloop.py:_store`.
fn _store(beside: &Op, reference: &MemRef, value: Const) -> Op {
    let mut op = Op::new(beside.at, beside.op, "", Vec::new(), Vec::new());
    op.loads = Vec::new();
    op.stores = vec![reference.clone()];
    op.kind = Kind::Store;
    op.args = vec![Arg::Const(value)];
    op.results = vec![Arg::Cell(Cell {
        r#ref: reference.clone(),
    })];
    op.id = beside.id;
    op.symbol = Some(true);
    op
}

/// Direct port of `qbopt/optimize/floatloop.py:_jump`.
/// Direct port of `qbopt/optimize/floatloop.py:_rewritten`.
#[allow(clippy::too_many_arguments)]
fn _rewritten(
    body: &MirBody,
    header: &MirBlock,
    latch: &MirBlock,
    exit_at: i64,
    checkpoint: &Op,
    seeds: Vec<Op>,
    counter: Value,
    final_: Const,
) -> Result<MirBody, String> {
    let exit_block = body.blocks.iter().find(|block| block.at == exit_at).expect("the exit block");
    let values = ssa::values(body).collect::<Vec<_>>();
    let mut result = Value::new(values.iter().map(|value| value.id).max().expect("a value") + 1, exit_at);
    result.variable = values.iter().map(|value| value.variable).max().expect("a value") + 1;
    let copy = strength::_made(Kind::Copy, "", result, vec![Arg::Const(final_.clone())], exit_at, &exit_block.ops[0]);
    let final_stores = header
        .ops
        .iter()
        .flat_map(|op| op.stores.iter().map(move |reference| (op, reference)))
        .map(|(op, reference)| {
            let mut store = _store(op, reference, final_.clone());
            store.at = exit_at;
            store.absorbed = Vec::new();
            store
        })
        .collect::<Vec<_>>();
    let dominators = loops::dominators(&body.blocks, Some(body.entry));
    let following = body
        .blocks
        .iter()
        .filter(|block| dominators.get(&block.at).is_some_and(|found| found.contains(&exit_at)))
        .map(|block| block.at)
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for block in &body.blocks {
        let mut block = block.clone();
        if block.at == header.at {
            let last = block.ops.pop().expect("the header ends in a branch");
            block.ops.push(mir::jump(&last, latch.at));
            block.succ = vec![latch.at];
        } else if block.at == latch.at {
            let mut ops = Vec::new();
            for op in &latch.ops {
                ops.push(op.clone());
                if std::ptr::eq(op, checkpoint) {
                    ops.extend(seeds.iter().cloned());
                }
            }
            let end = block.ops.last().expect("the latch has an operation").at;
            let mut jump = mir::jump(header.ops.last().expect("the header ends in a branch"), exit_at);
            jump.at = end;
            jump.absorbed = Vec::new();
            ops.push(jump);
            block.ops = ops;
            block.succ = vec![exit_at];
        } else if following.contains(&block.at) {
            let swap = BTreeMap::from([(counter.id, result)]);
            let ops = block
                .ops
                .iter()
                .map(|op| ssa::substituted(op, &swap))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            block.ops = if block.at == exit_at {
                std::iter::once(copy.clone()).chain(final_stores.iter().cloned()).chain(ops).collect()
            } else {
                ops
            };
        }
        out.push(block);
    }
    let mut rewritten = body.clone();
    rewritten.blocks = out;
    transform::_trivial_phis(&rewritten)
}

#[cfg(test)]
#[path = "floatloop_tests.rs"]
mod tests;
