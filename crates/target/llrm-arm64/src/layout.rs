//! Block layout after register assignment: the blocks that only branch are
//! threaded through, unreachable blocks dropped, the blocks placed so a branch
//! falls through where it can, and the branches that then jump to the next
//! block removed (LLVM's `BranchFolder` and `MachineBlockPlacement`,
//! simplified: no frequencies, so a chain follows the first successor that is
//! not placed).

use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::placement::{Graph, Odds, place};
use llrm_support::hash::{HashMap, HashSet};

use crate::frame::Lowered;
use crate::jumptable::Table;
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
    let mut out = HashMap::default();
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

/// The conditional branch `branch` that jumps over the jump to `go` into the
/// block `next`, as the inverse branch to `go` (`reverseBranchCondition`).
fn inverse_over(
    branch: &Inst,
    go: &str,
    next: Option<&str>,
) -> Option<Inst> {
    let to = Target::Label(go.to_owned());
    let over = |target: &Target| {
        matches!(
            (target, next),
            (Target::Label(label), Some(next)) if label == next
        )
    };
    match branch {
        Inst::BranchCond { cond, target } if over(target) => {
            Some(Inst::BranchCond { cond: cond.inverted(), target: to })
        }
        Inst::CompareBranch { nonzero, width, rt, target } if over(target) => {
            Some(Inst::CompareBranch { nonzero: !nonzero, width: *width, rt: *rt, target: to })
        }
        Inst::TestBranch { nonzero, rt, bit, target } if over(target) => {
            Some(Inst::TestBranch { nonzero: !nonzero, rt: *rt, bit: *bit, target: to })
        }
        _ => None,
    }
}

/// The IR's branch probabilities by `(from, to)` layout position, for
/// [`fold`] (`MachineBranchProbabilityInfo`).
pub fn edge_odds(
    module: &llrm_mir::module::Module,
    function: &llrm_mir::module::Function,
    layout: &llrm_mir::datalayout::DataLayout,
) -> Odds {
    let bpi = llrm_analysis::bpi::BranchProbabilityInfo::of(
        llrm_analysis::bpi::Module {
            context: &module.context,
            declarations: &module.globals,
            metadata: &module.metadata,
            layout,
        },
        function,
    );
    let position: HashMap<_, _> = function.layout().iter().enumerate().map(|(at, &block)| (block, at as u32)).collect();
    let mut odds = Odds::default();
    for (from, &block) in function.layout().iter().enumerate() {
        for (index, to) in llrm_mir::loopinfo::children(function, block).enumerate() {
            let probability = bpi.get_edge_probability(function, block, index);
            let entry = odds
                .entry((from as u32, position[&to]))
                .or_insert((llrm_mir::blockfrequency::BranchProbability::get_zero(), index));
            entry.0 = entry.0 + probability;
        }
    }
    odds
}

pub fn fold(
    mut blocks: Vec<Lowered>,
    tables: &mut [Table],
    odds: &Odds,
    optsize: bool,
) -> Vec<Lowered> {
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
            Some(Inst::Branch { link: false, target: Target::Label(label) }) if returns.contains(label) => blocks
                .iter()
                .find(|block| &block.label == label)
                .and_then(|block| Some((block.insts.first().cloned()?, block.locs.first().copied().flatten()))),
            _ => None,
        };
        // The copy keeps the return's own source place, not the branch's.
        if let Some((ret, place)) = ret {
            *blocks[at].insts.last_mut().expect("a branch") = ret;
            *blocks[at].locs.last_mut().expect("a place per instruction") = place;
        }
    }
    // A fall-through becomes a branch, so that blocks can move.
    for at in 0..blocks.len() - 1 {
        let ends = matches!(
            blocks[at].insts.last(),
            Some(
                Inst::Branch { link: false, .. }
                    | Inst::BranchReg { link: false, .. }
                    | Inst::Ret { .. }
                    | Inst::Brk { .. }
            )
        );
        if !ends {
            let next = blocks[at + 1].label.clone();
            blocks[at].insts.push(Inst::Branch { link: false, target: Target::Label(next) });
            blocks[at].locs.push(None);
        }
    }
    // The entry is never forwarded away.
    let entry = blocks[0].label.clone();
    let mut forward = forwards(&blocks);
    // Blocks that only return the same way are one block a conditional branch
    // can reach (`BranchFolder`).
    let mut first_return: Vec<(&Inst, &str)> = Vec::new();
    for block in &blocks {
        if let [ret @ Inst::Ret { .. }] = block.insts.as_slice() {
            match first_return.iter().find(|(known, _)| *known == ret) {
                Some((_, kept)) => {
                    forward.insert(block.label.clone(), (*kept).to_owned());
                }
                None => first_return.push((ret, block.label.as_str())),
            }
        }
    }
    forward.remove(&entry);
    for block in &mut blocks {
        for inst in &mut block.insts {
            retarget(inst, &forward);
        }
    }
    for entry in tables.iter_mut().flat_map(|table| table.entries.iter_mut()) {
        if let Some(found) = forward.get(entry.as_str()) {
            *entry = found.clone();
        }
    }
    let table_of: HashMap<&str, &Table> = tables.iter().map(|table| (table.base.as_str(), table)).collect();
    // Reachable blocks, in `MachineBlockPlacement`'s order.
    let index: HashMap<&str, usize> = blocks.iter().enumerate().map(|(at, block)| (block.label.as_str(), at)).collect();
    let successors = |at: usize| -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for inst in &blocks[at].insts {
            match inst {
                // The jump through a table goes to every entry.
                Inst::Label(base) => {
                    if let Some(table) = table_of.get(base.as_str()) {
                        out.extend(table.entries.iter().filter_map(|label| index.get(label.as_str()).copied()));
                    }
                }
                _ => out.extend(label_of(inst).and_then(|label| index.get(label).copied())),
            }
        }
        out
    };
    let mut reachable: Vec<usize> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::default();
    let mut pending: Vec<usize> = vec![0];
    seen.insert(0);
    while let Some(at) = pending.pop() {
        reachable.push(at);
        for next in successors(at) {
            if seen.insert(next) {
                pending.push(next);
            }
        }
    }
    reachable.sort_unstable();
    let dense: HashMap<usize, usize> = reachable.iter().enumerate().map(|(new, &old)| (old, new)).collect();
    let edges: Vec<Vec<usize>> =
        reachable.iter().map(|&old| successors(old).into_iter().map(|next| dense[&next]).collect()).collect();
    // The IR's block number is the label's last field; a block a pass made
    // has none the odds know.
    let numbered = |old: usize| blocks[old].label.rsplit('.').next().is_some_and(|n| n.parse::<u32>().is_ok());
    let ids: Vec<u32> = reachable
        .iter()
        .enumerate()
        .map(|(new, &old)| {
            blocks[old].label.rsplit('.').next().and_then(|n| n.parse().ok()).unwrap_or(u32::MAX - new as u32)
        })
        .collect();
    // A block made to split a critical edge takes the odds of the edge it
    // splits (the pin carries them through the split).
    let mut odds = odds.clone();
    for (new, &old) in reachable.iter().enumerate() {
        let (&[to], false) = (edges[new].as_slice(), numbered(old)) else { continue };
        let from: Vec<usize> = (0..edges.len()).filter(|&one| edges[one].contains(&new)).collect();
        if let [from] = from[..]
            && let Some(&split) = odds.get(&(ids[from], ids[to]))
        {
            odds.insert((ids[from], ids[new]), split);
        }
    }
    // DEVIATE: an entry that a branch returns to (the pin's IR never has one;
    // block forwarding can make one) keeps the layout order, as frequencies
    // need an entry that is no loop header. Exit: 11-12.
    let placed: Vec<usize> = if edges.iter().any(|next| next.contains(&0)) {
        reachable.clone()
    } else {
        let cfg = Cfg::from_successors(edges);
        let graph = Graph::from_cfg(&cfg, &ids, vec![false; ids.len()], &odds, optsize);
        place(&graph).into_iter().map(|new| reachable[new]).collect()
    };
    let mut taken: Vec<Option<Lowered>> = blocks.into_iter().map(Some).collect();
    let mut ordered: Vec<Lowered> = placed.into_iter().map(|at| taken[at].take().expect("placed once")).collect();
    // A branch to the next block falls through; a conditional branch over an
    // unconditional one is the inverse conditional (`cbz`/`tbz` too).
    for at in 0..ordered.len() {
        let next = ordered.get(at + 1).map(|block| block.label.clone());
        let Lowered { insts, locs, .. } = &mut ordered[at];
        if let (Some(Inst::Branch { link: false, target: Target::Label(label) }), Some(next)) = (insts.last(), &next)
            && label == next
        {
            insts.pop();
            locs.pop();
        }
        if insts.len() >= 2
            && let Some(Inst::Branch { link: false, target: Target::Label(go) }) = insts.last()
            && let Some(inverse) = inverse_over(&insts[insts.len() - 2], go, next.as_deref())
        {
            insts.truncate(insts.len() - 2);
            let place = locs.get(locs.len() - 2).copied().flatten();
            locs.truncate(locs.len() - 2);
            insts.push(inverse);
            locs.push(place);
        }
    }
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mc::inst::{Gpr, Width};

    fn block(
        label: &str,
        insts: Vec<Inst>,
    ) -> Lowered {
        let locs = vec![None; insts.len()];
        Lowered { label: label.into(), insts, locs }
    }

    fn cbz(
        reg: u32,
        label: &str,
    ) -> Inst {
        Inst::CompareBranch { nonzero: false, width: Width::W, rt: Gpr(reg), target: Target::Label(label.into()) }
    }

    /// Two early exits each went to a `ret` block of their own, three `ret`s
    /// where the pin has one (`rectwo`: +8 B).
    #[test]
    fn conditional_branches_to_identical_return_blocks_reach_one() {
        let ret = || Inst::Ret { rn: Gpr(30) };
        let blocks = vec![
            block("entry", vec![cbz(0, "a")]),
            block("mid", vec![cbz(1, "b")]),
            block("body", vec![Inst::Branch { link: false, target: Target::Label("c".into()) }]),
            block("a", vec![ret()]),
            block("b", vec![ret()]),
            block("c", vec![ret()]),
        ];
        let out = fold(blocks, &mut [], &Odds::default(), false);
        let rets = out.iter().flat_map(|block| &block.insts).filter(|inst| matches!(inst, Inst::Ret { .. })).count();
        let targets: Vec<_> = out
            .iter()
            .flat_map(|block| &block.insts)
            .filter_map(|inst| match inst {
                Inst::CompareBranch { target, .. } => Some(target.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(targets.len(), 2, "{out:?}");
        assert_eq!(targets[0], targets[1], "{out:?}");
        assert!(rets <= 2, "{rets} returns: {out:?}");
    }
}
