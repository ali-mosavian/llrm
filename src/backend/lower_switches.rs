//! Port of `qbopt/backend/lower_switches.py`.

use std::collections::BTreeSet;
use crate::support::hash::HashMap;

use num_bigint::BigInt;

use crate::analysis::{liveness, ssa};
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Kind, MirBlock, MirBody, Op, OpCode, Phi, Value};
use crate::optimize::edges;

fn width(arg: &Arg) -> Option<u32> {
    match arg {
        Arg::Held(one) => Some(one.width),
        Arg::Const(one) => Some(one.width),
        _ => None,
    }
}

pub fn expanded(body: &MirBody) -> Result<MirBody, String> {
    let is_switch = |op: &Op| op.kind == Kind::Switch;
    let switches: Vec<&MirBlock> =
        body.blocks.iter().filter(|block| block.ops.last().is_some_and(is_switch)).collect();
    if switches.is_empty() {
        if body.blocks.iter().any(|block| block.ops.iter().any(is_switch)) {
            return Err("switch must terminate its block".into());
        }
        return Ok(body.clone());
    }
    if body.blocks.iter().any(|block| block.ops[..block.ops.len().saturating_sub(1)].iter().any(is_switch)) {
        return Err("switch must terminate its block".into());
    }
    let live = liveness::live(body);
    let labels: BTreeSet<i64> = body.blocks.iter().map(|block| block.at).collect();
    for block in &switches {
        let op = block.ops.last().unwrap();
        let mut successors: BTreeSet<Option<i64>> = BTreeSet::from([op.target]);
        successors.extend(op.cases.iter().map(|&(_, target)| Some(target)));
        if op.args.len() != 1
            || !matches!(width(&op.args[0]), Some(1 | 2 | 4))
            || !op.defines.is_empty()
            || !op.results.is_empty()
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || !op.merges.is_empty()
            || op.barrier()
            || op.stack.is_some()
            || op.floating.is_some()
            || !op.target.is_some_and(|target| labels.contains(&target))
            || op.cases.iter().any(|(_, target)| !labels.contains(target))
            || block.succ.iter().map(|&at| Some(at)).collect::<BTreeSet<_>>() != successors
        {
            return Err("invalid semantic switch".into());
        }
        let mask = (1i64 << (8 * width(&op.args[0]).unwrap())) - 1;
        let normalized: Vec<i64> = op.cases.iter().map(|(value, _)| value & mask).collect();
        if normalized.iter().collect::<BTreeSet<_>>().len() != normalized.len() {
            return Err("duplicate switch case value".into());
        }
        if matches!(op.args[0], Arg::Held(_))
            && op.cases.iter().any(|&(_, target)| Some(target) != op.target)
            && live.live_out[&block.at].iter().any(|value| value.flags)
        {
            return Err("switch expansion crosses a live condition".into());
        }
    }
    let mut serial = ssa::values(body).map(|value| value.id).max().unwrap_or(0);
    let mut label = edges::fresh(body);
    let mut replacements: HashMap<i64, Vec<MirBlock>> = HashMap::default();
    let mut incoming: HashMap<(i64, i64), Vec<i64>> = HashMap::default();
    for block in &switches {
        let mut op = block.ops.last().unwrap().clone();
        let default = op.target.unwrap();
        op.cases.retain(|&(_, target)| target != default);
        let selector = op.args[0].clone();
        let selector_width = width(&selector).unwrap();
        if op.cases.is_empty() || matches!(selector, Arg::Const(_)) {
            let mut target = default;
            if let Arg::Const(selector) = &selector {
                let mask = (BigInt::from(1) << (8 * selector.width)) - 1;
                target = op
                    .cases
                    .iter()
                    .find(|&&(number, _)| BigInt::from(number) & &mask == &selector.n & &mask)
                    .map_or(target, |&(_, target)| target);
            }
            let mut jump = op.clone();
            jump.kind = Kind::Jump;
            jump.target = Some(target);
            jump.args = vec![];
            jump.uses = vec![];
            jump.cases = vec![];
            jump.name = String::new();
            jump.raised = None;
            let mut ops = block.ops[..block.ops.len() - 1].to_vec();
            ops.push(jump);
            replacements.insert(block.at, vec![MirBlock { ops, succ: vec![target], ..(*block).clone() }]);
            for &successor in &block.succ {
                incoming.insert((successor, block.at), if successor == target { vec![block.at] } else { vec![] });
            }
            continue;
        }
        let count = op.cases.len() as i64;
        let mut chain = vec![block.at];
        chain.extend(label..label + count - 1);
        label += count - 1;
        let mut rebuilt = Vec::new();
        for (index, &(number, target)) in op.cases.iter().enumerate() {
            let at = chain[index];
            let fallback = chain.get(index + 1).copied().unwrap_or(default);
            serial += 1;
            let condition = Value { flags: true, variable: serial, version: 1, ..Value::new(serial, op.at) };
            let uses = match &selector {
                Arg::Held(held) => vec![held.value],
                _ => vec![],
            };
            let mut compare = Op::new(op.at, OpCode::Operation(Operation::Compare), "cmp", vec![condition], uses);
            compare.kind = Kind::Sub;
            compare.args =
                vec![selector.clone(), Arg::Const(Const::new(number & ((1i64 << (8 * selector_width)) - 1), selector_width))];
            compare.symbol = Some(false);
            let mut branch = Op::new(op.at, OpCode::Operation(Operation::Branch), "", vec![], vec![condition]);
            branch.kind = Kind::Branch;
            branch.test = Some(Kind::Eq);
            branch.target = Some(target);
            branch.symbol = Some(false);
            if index == 0 {
                compare.absorbed = op.absorbed.clone();
                compare.id = op.id;
                compare.source_backed = op.source_backed;
            }
            let mut ops = if index == 0 { block.ops[..block.ops.len() - 1].to_vec() } else { vec![] };
            ops.push(compare);
            ops.push(branch);
            let mut succ = vec![target];
            if fallback != target {
                succ.push(fallback);
            }
            rebuilt.push(MirBlock {
                cold: block.cold,
                ..MirBlock::new(at, if index == 0 { block.phis.clone() } else { vec![] }, ops, succ)
            });
            incoming.entry((target, block.at)).or_default().push(at);
            if index + 1 == chain.len() {
                incoming.entry((fallback, block.at)).or_default().push(at);
            }
        }
        replacements.insert(block.at, rebuilt);
    }
    let mut blocks = Vec::new();
    for original in &body.blocks {
        for block in replacements.get(&original.at).cloned().unwrap_or_else(|| vec![original.clone()]) {
            let phis: Vec<Phi> = block
                .phis
                .iter()
                .map(|phi| Phi {
                    incoming: phi
                        .incoming
                        .iter()
                        .flat_map(|(&source, &value)| {
                            incoming
                                .get(&(block.at, source))
                                .cloned()
                                .unwrap_or_else(|| vec![source])
                                .into_iter()
                                .map(move |replacement| (replacement, value))
                        })
                        .collect(),
                    ..phi.clone()
                })
                .collect();
            blocks.push(MirBlock { phis, ..block });
        }
    }
    Ok(MirBody { blocks, cloned: true, ..body.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::loops;

    fn switched() -> MirBody {
        let selector = Value { variable: 1, version: 1, ..Value::new(1, 0) };
        let merged = Value { variable: 2, version: 1, ..Value::new(2, 20) };
        let mut op = Op::new(5, OpCode::Operation(Operation::Jump), "", vec![], vec![selector]);
        op.kind = Kind::Switch;
        op.args = vec![Arg::Held(crate::model::mir::Held { value: selector, width: 2 })];
        op.target = Some(30);
        op.cases = vec![(1, 20), (2, 20), (3, 30)];
        op.absorbed = vec![5];
        let phi = Phi { result: merged, incoming: [(0, selector)].into_iter().collect() };
        MirBody::new(
            0,
            vec![
                MirBlock::new(0, vec![], vec![op], vec![20, 30]),
                MirBlock::new(20, vec![phi], vec![], vec![]),
                MirBlock::new(30, vec![], vec![], vec![]),
            ],
        )
    }

    fn destination(body: &MirBody, number: i64) -> i64 {
        let mut block = body.block(body.entry).unwrap();
        while !block.ops.is_empty() {
            let [compare, branch] = &block.ops[block.ops.len() - 2..] else { unreachable!() };
            let Arg::Const(constant) = &compare.args[1] else { panic!("not a constant") };
            let selected = if BigInt::from(number) == constant.n || block.succ.len() == 1 {
                branch.target.unwrap()
            } else {
                *block.succ.iter().find(|&&at| Some(at) != branch.target).unwrap()
            };
            block = body.block(selected).unwrap();
        }
        block.at
    }

    #[test]
    fn test_switch_expansion_keeps_cases_default_and_shared_destination_phis() {
        let body = expanded(&switched()).unwrap();
        let got: Vec<i64> = [0, 1, 2, 3, 4, 255].iter().map(|&value| destination(&body, value)).collect();
        assert_eq!(got, [30, 20, 20, 30, 30, 30]);
        let predecessors = loops::predecessors(&body.blocks);
        let target = body.block(20).unwrap();
        assert_eq!(target.phis[0].incoming.keys().copied().collect::<BTreeSet<_>>(), predecessors[&20]);
        assert_eq!(target.phis[0].incoming.len(), 2);
        assert_eq!(body.blocks.iter().flat_map(|block| &block.ops).filter(|op| op.absorbed == [5]).count(), 1);
    }
}
