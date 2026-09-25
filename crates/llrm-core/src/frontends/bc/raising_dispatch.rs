//! Port of `qbopt/frontend/raising_dispatch.py`.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::analysis::ssa;
use crate::frontends::bc::blocks::{Block, dispatch_targets};
use crate::frontends::bc::declen::Insn;
use crate::frontends::bc::raising_words;
use crate::model::ir::Operation;
use crate::model::mir::{Arg, Const, Kind, MirBlock, Op, OpCode, OrderedMap, RaisedBody, Value};
use crate::objectfile::module::{self, Family, Module};

pub fn raised(body: RaisedBody, found: &Module, machine_blocks: &[Block]) -> RaisedBody {
    if !matches!(module::family(&found.records), Family::Quickbasic | Family::Pds | Family::Vbdos) {
        return body;
    }
    if module::defines(&found.records, found.seg).contains("B$OGTA") {
        return body;
    }
    if body.blocks.iter().flat_map(|block| &block.ops).any(|op| {
        op.barrier() || op.kind == Kind::Opaque || op.loads.iter().chain(&op.stores).any(|r#ref| r#ref.segment.is_some())
    }) {
        return body;
    }
    let exits = raising_words::leaving(&body);
    let candidate = ssa::pruned_phis(&Rc::new(body.body.clone()), &exits);
    let mut readers: BTreeSet<Value> = exits.clone();
    readers.extend(candidate.blocks.iter().flat_map(|block| &block.ops).flat_map(|op| op.uses.iter().copied()));
    readers.extend(
        candidate.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.values().copied()),
    );
    let instructions: BTreeMap<i64, &Insn> =
        machine_blocks.iter().flat_map(|block| &block.insns).map(|insn| (insn.at as i64, insn)).collect();
    let labels: BTreeSet<i64> = candidate.blocks.iter().map(|block| block.at).collect();
    let mut serial = ssa::values(&candidate).map(|value| value.id).max().unwrap_or(0);
    let mut label = crate::optimize::edges::fresh(&candidate);
    let mut replacements: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    let mut blocks = Vec::new();
    let mut added = Vec::new();
    for block in &candidate.blocks {
        let op = block.ops.last();
        let Some(op) = op.filter(|op| {
            op.kind == Kind::Call
                && found.calls.get(&op.at).map(String::as_str) == Some("B$OGTA")
                && instructions.contains_key(&op.at)
                && op.args_known
                && op.args.len() == 1
                && match &op.args[0] {
                    Arg::Held(held) => held.width == 2,
                    Arg::Const(constant) => constant.width == 2,
                    _ => false,
                }
                && !op.defines.iter().any(|value| readers.contains(value))
        }) else {
            blocks.push(block.clone());
            continue;
        };
        let insn = instructions[&op.at];
        let Some(targets) = dispatch_targets(found, insn) else {
            blocks.push(block.clone());
            continue;
        };
        let default = insn.end() as i64 + 1 + 2 * targets.len() as i64;
        let mut expected: BTreeSet<i64> = targets.iter().copied().collect();
        expected.insert(default);
        if !labels.contains(&default) || block.succ.iter().copied().collect::<BTreeSet<i64>>() != expected {
            blocks.push(block.clone());
            continue;
        }
        let (error, normal) = (label, label + 1);
        label += 2;
        serial += 1;
        let condition = Value { id: serial, at: op.at, flags: true, variable: serial, version: 1 };
        let selector = op.args[0].clone();
        let uses = match &selector {
            Arg::Held(held) => vec![held.value],
            _ => Vec::new(),
        };
        let mut compare = Op::new(op.at, OpCode::Operation(Operation::Compare), "cmp", vec![condition], uses.clone());
        compare.kind = Kind::Sub;
        compare.args = vec![selector.clone(), Arg::Const(Const::new(255, 2))];
        compare.symbol = Some(false);
        let mut guard = Op::new(op.at, OpCode::Operation(Operation::Branch), "", vec![], vec![condition]);
        guard.kind = Kind::Branch;
        guard.test = Some(Kind::Above);
        guard.target = Some(error);
        guard.symbol = Some(false);
        let mut dispatch = Op::new(op.at, OpCode::Operation(Operation::Jump), "", vec![], uses);
        dispatch.kind = Kind::Switch;
        dispatch.args = vec![selector];
        dispatch.target = Some(default);
        dispatch.cases = targets.iter().enumerate().map(|(number, target)| (number as i64 + 1, *target)).collect();
        dispatch.symbol = Some(false);
        let mut ops = block.ops[..block.ops.len() - 1].to_vec();
        ops.extend([compare, guard]);
        let mut guarded = block.with_ops(ops);
        guarded.succ = vec![error, normal];
        blocks.push(guarded);
        added.push(MirBlock::new(error, vec![], vec![op.clone()], block.succ.clone()));
        added.push(MirBlock::new(normal, vec![], vec![dispatch], block.succ.clone()));
        replacements.insert(block.at, (error, normal));
    }
    if replacements.is_empty() {
        return body;
    }
    let blocks = blocks
        .into_iter()
        .chain(added)
        .map(|mut block| {
            for phi in &mut block.phis {
                let mut incoming = OrderedMap::new();
                for (source, value) in phi.incoming.iter() {
                    match replacements.get(source) {
                        Some((error, normal)) => {
                            incoming.insert(*error, *value);
                            incoming.insert(*normal, *value);
                        }
                        None => {
                            incoming.insert(*source, *value);
                        }
                    }
                }
                phi.incoming = incoming;
            }
            block
        })
        .collect();
    let mut result = RaisedBody { body: candidate.with_blocks(blocks), origin: body.origin.clone(), pins: body.pins.clone() };
    result.body_mut().cloned = true;
    result
}

#[cfg(test)]
#[path = "raising_dispatch_tests.rs"]
mod tests;
