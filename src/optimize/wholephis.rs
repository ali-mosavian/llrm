//! Join corresponding word phis using proven whole values on every edge.
//!
//! Direct port of `qbopt/optimize/wholephis.py`.

use std::rc::Rc;
use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use num_bigint::BigInt;

use crate::analysis::{consts, loops, ssa};
use crate::model::ir::Operation;
use crate::model::mir::{
    self, Arg, Const, Held, Kind, MirBlock, MirBody, Op, OpCode, OrderedMap, Phi, Synth, Value,
};

pub(crate) fn _source(arg: &Arg, definitions: &BTreeMap<Value, &Op>) -> Arg {
    let mut arg = arg.clone();
    let mut seen = BTreeSet::new();
    loop {
        let Arg::Held(held) = &arg else { break };
        if !seen.insert(held.value) {
            break;
        }
        let Some(op) = definitions.get(&held.value) else {
            break;
        };
        if op.kind != Kind::Copy
            || !op.loads.is_empty()
            || !op.stores.is_empty()
            || op.barrier()
            || op.results != [arg.clone()]
            || op.args.len() != 1
            || !matches!(&op.args[0], Arg::Held(source) if source.width == held.width)
        {
            break;
        }
        arg = op.args[0].clone();
    }
    arg
}

pub(crate) fn joined(body: &Rc<MirBody>) -> Rc<MirBody> {
    let definitions = body
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .flat_map(|op| op.defines.iter().map(move |value| (*value, op)))
        .collect::<BTreeMap<Value, &Op>>();
    let facts = consts::known(body, None, None, None, None);
    let predecessors = loops::predecessors(&body.blocks);
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.at, block))
        .collect::<BTreeMap<i64, &MirBlock>>();
    let values = ssa::values(body).collect::<Vec<_>>();
    let mut serial = values.iter().map(|value| value.id).max().unwrap_or(0);
    let mut variable = values.iter().map(|value| value.variable).max().unwrap_or(0);
    let mut additions = IndexMap::<i64, Vec<Op>>::new();
    let mut replacements = BTreeMap::<(usize, usize), Op>::new();
    let mut new_phis = IndexMap::<i64, Vec<Phi>>::new();
    let mut extracts = IndexMap::<i64, Vec<Op>>::new();
    let mut removed = BTreeSet::<Value>::new();

    let mut fresh = |at: i64, variable: u32| {
        serial += 1;
        Value {
            variable,
            version: serial,
            ..Value::new(serial, at)
        }
    };

    for (block_index, block) in body.blocks.iter().enumerate() {
        let mut phis = block
            .phis
            .iter()
            .map(|phi| (phi.result, phi))
            .collect::<IndexMap<Value, &Phi>>();
        for (op_index, op) in block.ops.iter().enumerate() {
            if op.kind != Kind::Concat
                || !op.loads.is_empty()
                || !op.stores.is_empty()
                || op.barrier()
                || op.args.len() != 2
                || op.results.len() != 1
                || !matches!(&op.results[0], Arg::Held(held) if held.width == 4 && op.defines == [held.value])
            {
                continue;
            }
            let args = op
                .args
                .iter()
                .map(|arg| _source(arg, &definitions))
                .collect::<Vec<_>>();
            if args
                .iter()
                .any(|arg| !matches!(arg, Arg::Held(held) if held.width == 2 && phis.contains_key(&held.value)))
            {
                continue;
            }
            let held = |arg: &Arg| match arg {
                Arg::Held(held) => held.value,
                _ => unreachable!("checked above"),
            };
            let (high, low) = (phis[&held(&args[0])], phis[&held(&args[1])]);
            let preceding = predecessors.get(&block.at).cloned().unwrap_or_default();
            if high.incoming.is_empty()
                || high.incoming.keys().collect::<BTreeSet<_>>()
                    != low.incoming.keys().collect::<BTreeSet<_>>()
                || high.incoming.keys().copied().collect::<BTreeSet<_>>() != preceding
            {
                continue;
            }
            let mut sources = IndexMap::<i64, Arg>::new();
            for (at, upper) in high.incoming.iter() {
                let lower = low.incoming.get(at).expect("same keys");
                let whole = mir::extracted_whole(
                    &Arg::Held(Held {
                        value: *upper,
                        width: 2,
                    }),
                    &Arg::Held(Held {
                        value: *lower,
                        width: 2,
                    }),
                    &definitions,
                );
                let whole = match whole {
                    Some(whole) => Arg::Held(whole),
                    None => {
                        let (Some(upper_fact), Some(lower_fact)) =
                            (facts.get(upper), facts.get(lower))
                        else {
                            break;
                        };
                        if upper_fact.width != 2 || lower_fact.width != 2 {
                            break;
                        }
                        let mask = BigInt::from(0xffff);
                        Arg::Const(Const::new(
                            ((&upper_fact.n & &mask) << 16) | (&lower_fact.n & &mask),
                            4,
                        ))
                    }
                };
                if !blocks.get(at).is_some_and(|one| !one.ops.is_empty()) {
                    break;
                }
                sources.insert(*at, whole);
            }
            if sources.len() != high.incoming.len() {
                continue;
            }
            variable += 1;
            let mut incoming = OrderedMap::new();
            for (at, source) in &sources {
                let position = blocks[at].ops.last().expect("nonempty").at;
                let value = fresh(position, variable);
                incoming.insert(*at, value);
                additions.entry(*at).or_default().push(Op {
                    kind: Kind::Copy,
                    args: vec![source.clone()],
                    results: vec![Arg::Held(Held { value, width: 4 })],
                    ..Op::new(
                        position,
                        OpCode::Operation(Operation::Move),
                        "mov",
                        vec![value],
                        match source {
                            Arg::Held(held) => vec![held.value],
                            _ => vec![],
                        },
                    )
                });
            }
            let result = fresh(block.at, variable);
            new_phis
                .entry(block.at)
                .or_default()
                .push(Phi { result, incoming });
            for (phi, offset) in [(high, 16), (low, 0)] {
                removed.insert(phi.result);
                phis.shift_remove(&phi.result);
                extracts.entry(block.at).or_default().push(Op {
                    kind: Kind::Extract,
                    args: vec![
                        Arg::Held(Held {
                            value: result,
                            width: 4,
                        }),
                        Arg::Const(Const::new(offset, 4)),
                    ],
                    results: vec![Arg::Held(Held {
                        value: phi.result,
                        width: 2,
                    })],
                    ..Op::new(
                        block.at,
                        OpCode::Synth(Synth::HalfToLow),
                        "extract",
                        vec![phi.result],
                        vec![result],
                    )
                });
            }
            replacements.insert(
                (block_index, op_index),
                Op {
                    kind: Kind::Copy,
                    args: vec![Arg::Held(Held {
                        value: result,
                        width: 4,
                    })],
                    uses: vec![result],
                    merges: OrderedMap::new(),
                    source_backed: false,
                    raised: None,
                    ..op.clone()
                },
            );
        }
    }
    if replacements.is_empty() {
        return body.clone();
    }
    let mut changed = Vec::new();
    for (block_index, block) in body.blocks.iter().enumerate() {
        let mut ops = block
            .ops
            .iter()
            .enumerate()
            .map(|(op_index, op)| {
                replacements
                    .get(&(block_index, op_index))
                    .unwrap_or(op)
                    .clone()
            })
            .collect::<Vec<_>>();
        let position =
            ops.len()
                - usize::from(ops.last().is_some_and(|last| {
                    matches!(last.kind, Kind::Branch | Kind::Jump | Kind::Return)
                }));
        if let Some(added) = additions.get(&block.at) {
            ops.splice(position..position, added.iter().cloned());
        }
        if let Some(added) = extracts.get(&block.at) {
            ops.splice(0..0, added.iter().cloned());
        }
        let phis = block
            .phis
            .iter()
            .filter(|phi| !removed.contains(&phi.result))
            .cloned()
            .chain(new_phis.get(&block.at).into_iter().flatten().cloned())
            .collect();
        changed.push(MirBlock {
            ops,
            phis,
            ..block.clone()
        });
    }
    Rc::new(MirBody {
        blocks: changed,
        ..MirBody::clone(body)
    })
}
