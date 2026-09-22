//! Port of `qbopt/abi/nativecalls.py`: the interface of a native object's
//! own near procedures, from the returns its bodies reach.

use std::collections::BTreeSet;

use iced_x86::{Code, FlowControl};

use crate::abi::runtime::{self, Contract, Reg};
use crate::frontend::blocks::{Block, local_call_target};
use crate::frontend::declen::Insn;
use crate::frontend::extent::Partition;
use crate::objectfile::module::Module;
use crate::support::hash::IndexMap;

pub fn interfaces(
    module: &Module,
    partition: &Partition,
    blocks: &[Block],
    external: &IndexMap<i64, Contract>,
) -> IndexMap<i64, Contract> {
    let cleanup = cleanups(
        module,
        partition,
        blocks,
        &external.iter().filter_map(|(&at, rule)| rule.cleanup.map(|cleanup| (at, cleanup))).collect(),
    );
    let inputs: BTreeSet<Reg> = BTreeSet::from([Reg::Ax, Reg::Bx, Reg::Cx, Reg::Dx, Reg::Si, Reg::Di]);
    let mut result = external.clone();
    for block in blocks {
        for insn in &block.insns {
            let target = local_call_target(module, insn);
            let Some(target) = target else {
                continue;
            };
            let Some(&size) = cleanup.get(&(insn.at as i64)) else {
                continue;
            };
            result.insert(
                insn.at as i64,
                Contract {
                    inputs: Some(inputs.clone()),
                    cleanup: Some(size),
                    evidence: "Native C/Pascal input ABI assumed; cleanup from reachable local returns \
                               and a verified PUSH CS near-to-far adapter when required. \
                               No memory, preservation, termination or floating-stack claims."
                        .to_owned(),
                    ..runtime::contract(Some(&format!("native:{target:04x}")))
                },
            );
        }
    }
    result
}

pub fn cleanups(
    module: &Module,
    partition: &Partition,
    blocks: &[Block],
    external: &IndexMap<i64, i64>,
) -> IndexMap<i64, i64> {
    if !partition.unexplained.is_empty() || !partition.conflicts.is_empty() {
        return IndexMap::default();
    }
    let mut local: IndexMap<usize, (i64, bool)> = IndexMap::default();
    for body in &partition.bodies {
        let owned: Vec<&Block> = blocks
            .iter()
            .filter(|block| body.ranges.iter().any(|&(lo, hi)| lo <= block.at && block.at < hi))
            .collect();
        let starts: BTreeSet<usize> = owned.iter().map(|block| block.at).collect();
        if owned.iter().any(|block| block.succ.iter().any(|target| !starts.contains(target))) {
            continue;
        }
        let returns: Vec<&Insn> = owned
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|insn| insn.flow() == FlowControl::Return)
            .collect();
        let near = [Code::Retnw, Code::Retnw_imm16];
        let far = [Code::Retfw, Code::Retfw_imm16];
        if returns.is_empty() || returns.iter().any(|insn| !near.contains(&insn.code()) && !far.contains(&insn.code()))
        {
            continue;
        }
        let kinds: BTreeSet<bool> = returns.iter().map(|insn| far.contains(&insn.code())).collect();
        if kinds.len() != 1 {
            continue;
        }
        let returns_far = *kinds.first().unwrap();
        let address = if returns_far { 4 } else { 2 };
        let sizes: BTreeSet<i64> =
            returns.iter().map(|insn| insn.insn.stack_pointer_increment() as i64 - address).collect();
        if sizes.len() == 1 {
            local.insert(body.seed, (*sizes.first().unwrap(), returns_far));
        }
    }
    let mut previous: IndexMap<usize, &Insn> = IndexMap::default();
    for block in blocks {
        for pair in block.insns.windows(2) {
            if pair[0].end() == pair[1].at {
                previous.insert(pair[1].at, &pair[0]);
            }
        }
    }
    let mut result: IndexMap<i64, i64> = IndexMap::default();
    for block in blocks {
        for insn in &block.insns {
            if insn.flow() != FlowControl::Call {
                continue;
            }
            let target = local_call_target(module, insn);
            if let Some(&(size, returns_far)) = target.and_then(|target| local.get(&target)) {
                let adapter = previous.get(&insn.at);
                if !returns_far || adapter.is_some_and(|adapter| adapter.code() == Code::Pushw_CS) {
                    result.insert(insn.at as i64, size);
                }
            } else if module.calls.contains_key(&(insn.at as i64)) && external.contains_key(&(insn.at as i64)) {
                result.insert(insn.at as i64, external[&(insn.at as i64)]);
            }
        }
    }
    result
}

/// Visible stack bytes recovered by a call, including a synthetic CS.
///
/// A near CALL followed by a far return needs `push cs` immediately before
/// it. The callee's semantic cleanup remains its argument count; physical
/// frame validation additionally credits the far return for consuming that
/// explicit segment word.
pub fn stack_recovery(
    module: &Module,
    partition: &Partition,
    blocks: &[Block],
    cleanup: &IndexMap<i64, i64>,
) -> IndexMap<i64, i64> {
    let semantic = cleanups(module, partition, blocks, cleanup);
    let mut previous: IndexMap<usize, &Insn> = IndexMap::default();
    for block in blocks {
        for pair in block.insns.windows(2) {
            if pair[0].end() == pair[1].at {
                previous.insert(pair[1].at, &pair[0]);
            }
        }
    }
    let mut far_targets: BTreeSet<usize> = BTreeSet::new();
    for body in &partition.bodies {
        let owned: Vec<&Block> = blocks
            .iter()
            .filter(|block| body.ranges.iter().any(|&(lo, hi)| lo <= block.at && block.at < hi))
            .collect();
        let returns: Vec<&Insn> = owned
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|insn| insn.flow() == FlowControl::Return)
            .collect();
        if !returns.is_empty() && returns.iter().all(|insn| matches!(insn.code(), Code::Retfw | Code::Retfw_imm16)) {
            far_targets.insert(body.seed);
        }
    }
    let mut out = IndexMap::default();
    for block in blocks {
        for insn in &block.insns {
            let at = insn.at as i64;
            let Some(&size) = semantic.get(&at) else {
                continue;
            };
            let synthetic = local_call_target(module, insn).is_some_and(|target| far_targets.contains(&target))
                && previous.get(&insn.at).is_some_and(|before| before.code() == Code::Pushw_CS);
            out.insert(at, size + if synthetic { 2 } else { 0 });
        }
    }
    out
}

#[cfg(test)]
#[path = "nativecalls_tests.rs"]
mod tests;
