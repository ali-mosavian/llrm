//! Block layout after register assignment: the blocks that only branch are
//! threaded through, unreachable blocks dropped, the blocks placed so a branch
//! falls through where it can, and the branches that then jump to the next
//! block removed (LLVM's `BranchFolder` and `MachineBlockPlacement`,
//! simplified: no frequencies, so a chain follows the first successor that is
//! not placed).

use std::collections::{HashMap, HashSet};

use crate::frame::Lowered;
use crate::mc::inst::{Inst, Target};

pub(crate) fn label_of(inst: &Inst) -> Option<&str> {
    match inst {
        Inst::Branch { link: false, target: Target::Label(label) }
        | Inst::BranchCond { target: Target::Label(label), .. }
        | Inst::CompareBranch { target: Target::Label(label), .. }
        | Inst::TestBranch { target: Target::Label(label), .. } => Some(label),
        _ => None,
    }
}

fn retarget(
    inst: &mut Inst,
    forward: &HashMap<String, String>,
) {
    let target = match inst {
        Inst::Branch { link: false, target }
        | Inst::BranchCond { target, .. }
        | Inst::CompareBranch { target, .. }
        | Inst::TestBranch { target, .. } => target,
        _ => return,
    };
    if let Target::Label(label) = target
        && let Some(found) = forward.get(label.as_str())
    {
        *label = found.clone();
    }
}

/// Where a block that holds one unconditional branch leads, following chains.
fn forwards(blocks: &[Lowered]) -> HashMap<String, String> {
    let direct: HashMap<&str, &str> = blocks
        .iter()
        .filter_map(|block| match block.insts.as_slice() {
            [Inst::Branch { link: false, target: Target::Label(label) }] => {
                Some((block.label.as_str(), label.as_str()))
            }
            _ => None,
        })
        .collect();
    let mut out = HashMap::new();
    for (&from, &to) in &direct {
        // A chain, with a bound against a loop of empty blocks.
        let mut end = to;
        for _ in 0..direct.len() {
            match direct.get(end) {
                Some(next) if *next != from => end = next,
                _ => break,
            }
        }
        out.insert(from.to_owned(), end.to_owned());
    }
    out
}

pub fn fold(mut blocks: Vec<Lowered>) -> Vec<Lowered> {
    if blocks.is_empty() {
        return blocks;
    }
    // A branch to a block that only returns is the return (`TailDuplicator`'s
    // return blocks).
    let returns: HashSet<String> = blocks
        .iter()
        .filter(|block| matches!(block.insts.as_slice(), [Inst::Ret { .. }]))
        .map(|block| block.label.clone())
        .collect();
    for at in 0..blocks.len() {
        let ret = match blocks[at].insts.last() {
            Some(Inst::Branch { link: false, target: Target::Label(label) }) if returns.contains(label) => {
                blocks.iter().find(|block| &block.label == label).and_then(|block| block.insts.first().cloned())
            }
            _ => None,
        };
        if let Some(ret) = ret {
            *blocks[at].insts.last_mut().expect("a branch") = ret;
        }
    }
    // A fall-through becomes a branch, so that blocks can move.
    for at in 0..blocks.len() - 1 {
        let ends = matches!(
            blocks[at].insts.last(),
            Some(Inst::Branch { link: false, .. } | Inst::Ret { .. } | Inst::Brk { .. })
        );
        if !ends {
            let next = blocks[at + 1].label.clone();
            blocks[at].insts.push(Inst::Branch { link: false, target: Target::Label(next) });
        }
    }
    // The entry is never forwarded away.
    let entry = blocks[0].label.clone();
    let mut forward = forwards(&blocks);
    forward.remove(&entry);
    for block in &mut blocks {
        for inst in &mut block.insts {
            retarget(inst, &forward);
        }
    }
    // Reachable blocks, placed by following fall-through chains.
    let index: HashMap<&str, usize> = blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let successors = |at: usize| -> Vec<usize> {
        let block = &blocks[at];
        let out: Vec<usize> =
            block.insts.iter().filter_map(label_of).filter_map(|label| index.get(label).copied()).collect();
        let _ = at;
        out
    };
    let mut placed: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut pending: Vec<usize> = vec![0];
    while let Some(start) = pending.pop() {
        let mut current = start;
        loop {
            if !seen.insert(current) {
                break;
            }
            placed.push(current);
            let next = successors(current);
            // Follow the successor that is the fall-through (the last branch's
            // target, or the next block), push the rest to be placed later.
            let mut chain = None;
            for &candidate in next.iter().rev() {
                if !seen.contains(&candidate) && chain.is_none() {
                    chain = Some(candidate);
                } else if !seen.contains(&candidate) {
                    pending.push(candidate);
                }
            }
            match chain {
                Some(candidate) => current = candidate,
                None => break,
            }
        }
    }
    let mut taken: Vec<Option<Lowered>> = blocks.into_iter().map(Some).collect();
    let mut ordered: Vec<Lowered> = placed.into_iter().map(|at| taken[at].take().expect("placed once")).collect();
    // A branch to the next block falls through; a conditional branch over an
    // unconditional one is the inverse conditional.
    for at in 0..ordered.len() {
        let next = ordered.get(at + 1).map(|block| block.label.clone());
        let insts = &mut ordered[at].insts;
        if let (Some(Inst::Branch { link: false, target: Target::Label(label) }), Some(next)) = (insts.last(), &next)
            && label == next
        {
            insts.pop();
        }
        if insts.len() >= 2
            && let (
                Some(Inst::BranchCond { cond, target: Target::Label(over) }),
                Some(Inst::Branch { link: false, target: Target::Label(go) }),
            ) = (insts.get(insts.len() - 2), insts.last())
            && Some(over) == next.as_ref()
        {
            let (cond, go) = (cond.inverted(), go.clone());
            insts.truncate(insts.len() - 2);
            insts.push(Inst::BranchCond { cond, target: Target::Label(go) });
        }
    }
    ordered
}
