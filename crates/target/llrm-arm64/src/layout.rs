//! Block layout after register assignment: the blocks that only branch are
//! threaded through, unreachable blocks dropped, the blocks placed so a branch
//! falls through where it can, and the branches that then jump to the next
//! block removed (LLVM's `BranchFolder` and `MachineBlockPlacement`,
//! simplified: no frequencies, so a chain follows the first successor that is
//! not placed).

use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::placement::{Graph, Layout, Odds, place};
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

/// The branch probabilities by `(from, to)` label number
/// (`MachineBranchProbabilityInfo`): the IR's, then what the selector kept
/// for the edges its merged conditions made.
pub fn edge_odds(
    module: &llrm_mir::module::Module,
    function: &llrm_mir::module::Function,
    layout: &llrm_mir::datalayout::DataLayout,
    machine: &crate::mfunc::MFunc,
) -> Odds {
    use llrm_mir::blockfrequency::BranchProbability;
    let bpi = llrm_analysis::bpi::BranchProbabilityInfo::of(
        llrm_analysis::bpi::Module {
            context: &module.context,
            declarations: &module.globals,
            metadata: &module.metadata,
            layout,
        },
        function,
    );
    // A block's place is found by its id: the selector's, kept beside each
    // block through the passes that insert blocks, or the IR's layout order
    // among the blocks that are not an edge block.
    let position: HashMap<u32, u32> = if machine.block_ids.is_empty() {
        let edges: HashSet<&str> = machine.split_edges.iter().map(|(_, _, edge)| edge.as_str()).collect();
        let places = (0..machine.blocks.len()).filter(|&at| !edges.contains(machine.blocks[at].label.as_str()));
        function.layout().iter().map(|block| block.0).zip(places.map(|at| at as u32)).collect()
    } else {
        machine
            .blocks
            .iter()
            .enumerate()
            .filter_map(|(at, block)| machine.block_ids.get(&block.label).map(|&id| (id, at as u32)))
            .collect()
    };
    let mut odds = Odds::default();
    for &block in function.layout() {
        let Some(&from) = position.get(&block.0) else { continue };
        for (index, to) in llrm_mir::loopinfo::children(function, block).enumerate() {
            let Some(&to) = position.get(&to.0) else { continue };
            let probability = bpi.get_edge_probability(function, block, index);
            let entry = odds.entry((from, to)).or_insert((BranchProbability::get_zero(), index));
            entry.0 = entry.0 + probability;
        }
    }
    let mut rank: HashMap<u32, usize> = HashMap::default();
    let mut kept: HashSet<(u32, u32)> = HashSet::default();
    for &(from, to, numerator) in &machine.edge_probs {
        let (Some(&from), Some(&to)) = (position.get(&from), position.get(&to)) else { continue };
        let index = rank.entry(from).or_insert(0);
        let probability = BranchProbability::new(numerator, 1 << 31);
        // The first time an edge is named it replaces the IR's; a second
        // naming (both arms to one block) adds.
        if kept.insert((from, to)) {
            odds.insert((from, to), (probability, *index));
        } else if let Some(entry) = odds.get_mut(&(from, to)) {
            entry.0 = entry.0 + probability;
        }
        *index += 1;
    }
    // The odds of an edge `lower_phis` split are the new block's: the
    // source's edge to it has the probability of the edge it replaced, and its
    // own edge is certain (`ReplaceUsesOfBlockWith`,
    // MachineBasicBlock.cpp#L1263; `NMBB->addSuccessor`, #L1276).
    let place = |label: &str| machine.blocks.iter().position(|block| block.label == label).map(|at| at as u32);
    for (from, to, edge) in &machine.split_edges {
        let (Some(from), Some(to), Some(edge)) = (place(from), place(to), place(edge)) else { continue };
        if let Some(replaced) = odds.remove(&(from, to)) {
            odds.insert((from, edge), replaced);
        }
        odds.insert((edge, to), (BranchProbability::get_one(), 0));
    }
    odds
}

/// The number a block's label ends in (`.Lf.3`): where the selector put the
/// block, which `fold` calls its id. A block a pass made (`.e1`, `.t1`) has
/// none.
pub(crate) fn label_number(label: &str) -> Option<u32> {
    label.rsplit('.').next().and_then(|n| n.parse().ok())
}

/// [`edge_odds`] keyed as `fold` reads it, by label number: the table keyed
/// by position goes wrong once a block is inserted before another, as
/// `lower_phis` does beside each source. A block made on an edge has no number,
/// so the edge it replaced is stated again between the blocks that have one.
pub fn odds_by_label_number(
    odds: &Odds,
    machine: &crate::mfunc::MFunc,
) -> Odds {
    let number = |at: u32| label_number(&machine.blocks[at as usize].label);
    let mut out = Odds::default();
    for (&(from, to), &entry) in odds {
        if let (Some(from), Some(to)) = (number(from), number(to)) {
            out.insert((from, to), entry);
        }
    }
    for (from, to, edge) in &machine.split_edges {
        let place = |label: &str| machine.blocks.iter().position(|block| block.label == label).map(|at| at as u32);
        let (Some(from_at), Some(to_at), Some(edge_at)) = (place(from), place(to), place(edge)) else { continue };
        if let (Some(from), Some(to), Some(&entry)) = (number(from_at), number(to_at), odds.get(&(from_at, edge_at))) {
            out.insert((from, to), entry);
        }
    }
    out
}

/// The fewest instructions a shared tail has, its jump included
/// (`BranchFolder`'s `-tail-merge-size`: 3, of which the jump is the pin's
/// extra one, so two others).
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L103>
const TAIL_MERGE_SIZE: usize = 3;

/// The most blocks compared at once (`-tail-merge-threshold`).
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L97>
const TAIL_MERGE_THRESHOLD: usize = 150;

/// Whether `inst` is an instruction of the tail: not a directive.
fn counts(inst: &Inst) -> bool {
    !matches!(inst, Inst::Cfi(_) | Inst::Label(_))
}

/// How many instructions at the end of `one` and `other` are the same, and
/// the tail holds no label.
fn common_tail(
    one: &[Inst],
    other: &[Inst],
) -> usize {
    let mut length = 0;
    while length < one.len()
        && length < other.len()
        && one[one.len() - 1 - length] == other[other.len() - 1 - length]
        && !matches!(one[one.len() - 1 - length], Inst::Label(_))
    {
        length += 1;
    }
    length
}

/// Blocks that end alike share the end they have in common
/// (`BranchFolder::TryTailMergeBlocks`,
/// [`ProfitableToMerge`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L633)):
/// of the blocks that end in the same jump or return, the pair with the
/// longest common tail of at least `TAIL_MERGE_SIZE` instructions makes it one
/// block, and the blocks branch to it. The epilogue of a function with two
/// exits was written twice (`rectwo` -O2: +16 B), and `jpeg` -Os wrote the
/// same calls on four paths.
///
/// DEVIATE: the pin also merges a shorter tail where a block is only the
/// tail and the other can fall into it, and two instructions at -Os; and it
/// takes the pairs by the hash of their ends, not by the last instruction.
/// Exit: 11-20. Tried 11-05: taking both on the 148-cell corpus changed no
/// cell, so they stay out until a cell needs them.
fn merge_return_tails(blocks: &mut Vec<Lowered>) {
    // The block that is only a return, where there is one: a block that
    // returns, or jumps there, ends the same way.
    let bare = |blocks: &[Lowered]| -> Option<(String, Inst)> {
        blocks.iter().find_map(|block| match block.insts.as_slice() {
            [ret @ Inst::Ret { .. }] => Some((block.label.clone(), ret.clone())),
            _ => None,
        })
    };
    let ends_alike = |blocks: &[Lowered], at: usize, bare: &Option<(String, Inst)>| -> Option<Vec<Inst>> {
        let mut insts = blocks[at].insts.clone();
        match (insts.last(), bare) {
            (Some(Inst::Ret { .. }), None) => Some(insts),
            (Some(last), Some((label, ret)))
                if last == ret
                    || matches!(last, Inst::Branch { link: false, target: Target::Label(to) } if to == label) =>
            {
                if blocks[at].label == *label || insts.len() < 2 {
                    return None;
                }
                *insts.last_mut()? = Inst::Branch { link: false, target: Target::Label(label.clone()) };
                Some(insts)
            }
            // A jump (to a block, or a tail call): the blocks that jump to the
            // same place end alike.
            (Some(Inst::Branch { link: false, .. }), _) => Some(insts),
            _ => None,
        }
    };
    loop {
        let bare_block = bare(blocks);
        let ending: Vec<(usize, Vec<Inst>)> = (0..blocks.len())
            .filter_map(|at| ends_alike(blocks, at, &bare_block).map(|insts| (at, insts)))
            .collect();
        // Blocks are compared with those that end in the same instruction.
        let mut groups: HashMap<String, Vec<usize>> = HashMap::default();
        for (at, (_, insts)) in ending.iter().enumerate() {
            if let Some(last) = insts.last() {
                groups.entry(format!("{last:?}")).or_default().push(at);
            }
        }
        let mut best: Option<(usize, usize, usize)> = None;
        for group in groups.values().filter(|group| group.len() > 1 && group.len() <= TAIL_MERGE_THRESHOLD) {
            for (a, &x) in group.iter().enumerate() {
                for &y in &group[a + 1..] {
                    let ((one, one_insts), (other, other_insts)) = (&ending[x], &ending[y]);
                    let length = common_tail(one_insts, other_insts);
                    let real = one_insts[one_insts.len() - length..].iter().filter(|i| counts(i)).count();
                    // Two blocks that are only a jump: `forwards` merges those.
                    let whole = length == 1
                        && blocks[*one].insts.len() == 1
                        && blocks[*other].insts.len() == 1;
                    let found = (*one.min(other), *one.max(other), length);
                    if real >= TAIL_MERGE_SIZE
                        && !whole
                        && best.is_none_or(|(a, b, longest)| length > longest || (length == longest && (found.0, found.1) < (a, b)))
                    {
                        best = Some(found);
                    }
                }
            }
        }
        let Some((one, other, length)) = best else { return };
        for at in [one, other] {
            if let Some(insts) = ends_alike(blocks, at, &bare_block) {
                blocks[at].insts = insts;
            }
        }
        // The tail is a block of its own, unless one of the two is only the tail.
        let (keep, branch) = if length == blocks[other].insts.len() {
            (other, one)
        } else if length == blocks[one].insts.len() {
            (one, other)
        } else {
            let at = blocks[other].insts.len() - length;
            let label = {
                let mut number = 0;
                loop {
                    let label = format!("{}.t{number}", blocks[other].label);
                    if !blocks.iter().any(|block| block.label == label) {
                        break label;
                    }
                    number += 1;
                }
            };
            let tail = Lowered {
                label,
                insts: blocks[other].insts.split_off(at),
                locs: blocks[other].locs.split_off(at),
            };
            let label = tail.label.clone();
            blocks[other].insts.push(Inst::Branch { link: false, target: Target::Label(label) });
            blocks[other].locs.push(None);
            blocks.insert(other + 1, tail);
            (other + 1, one)
        };
        let label = blocks[keep].label.clone();
        let at = blocks[branch].insts.len() - length;
        blocks[branch].insts.truncate(at);
        blocks[branch].locs.truncate(at);
        blocks[branch].insts.push(Inst::Branch { link: false, target: Target::Label(label) });
        blocks[branch].locs.push(None);
    }
}

/// What an instruction reads or writes that the hoisting tracks: a register,
/// or the condition flags.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Item {
    Register(crate::mc::visit::Reg),
    Flags,
}

/// Whether an instruction is one `isSafeToMove` allows above a branch (a
/// register-to-register operation with no other effect), and whether it sets
/// the flags. `None`: not one.
fn movable(inst: &Inst) -> Option<bool> {
    use crate::mc::inst::Logic;
    match inst {
        Inst::AddSubImm { op, .. } | Inst::AddSubShifted { op, .. } | Inst::AddSubExtended { op, .. } => {
            Some(op.set_flags)
        }
        Inst::LogicImm { op, .. } | Inst::LogicShifted { op, .. } => Some(*op == Logic::Ands),
        Inst::MoveWide { .. }
        | Inst::BitfieldMove { .. }
        | Inst::Extract { .. }
        | Inst::Divide { .. }
        | Inst::OneSource { .. }
        | Inst::ThreeSource { .. }
        | Inst::AddPageOffset { .. } => Some(false),
        _ => None,
    }
}

/// The items `inst` writes and reads, for an instruction `movable` knows. The
/// zero register a flag-setting compare writes is no register; an operation
/// that writes the stack pointer is not hoisted.
fn effects(inst: &Inst) -> Option<(Vec<Item>, Vec<Item>)> {
    let sets_flags = movable(inst)?;
    let (defs, uses) = inst.defs_and_uses();
    let zero = crate::mc::visit::Reg::Gpr(crate::mc::inst::Gpr::R31);
    if !sets_flags && defs.contains(&zero) {
        return None;
    }
    let mut written: Vec<Item> = defs.into_iter().filter(|one| !(sets_flags && *one == zero)).map(Item::Register).collect();
    if sets_flags {
        written.push(Item::Flags);
    }
    // Register 31 in the source of these forms is the zero register, not the
    // stack pointer a pre-indexed store writes: `mov x11, xzr` reads nothing.
    let zero_sources = matches!(
        inst,
        Inst::LogicShifted { .. }
            | Inst::LogicImm { .. }
            | Inst::AddSubShifted { .. }
            | Inst::Divide { .. }
            | Inst::OneSource { .. }
            | Inst::ThreeSource { .. }
            | Inst::Extract { .. }
            | Inst::BitfieldMove { .. }
    );
    let read = uses.into_iter().filter(|one| !(zero_sources && *one == zero)).map(Item::Register).collect();
    Some((written, read))
}

/// DEVIATE (2026-11-08, owner w15, exit 2026-11-30): the pin hoists only the
/// leading run both ways of a branch share; this also lifts an instruction both
/// begin with later (x86's HC1), so that the run reaches it. recchop -Os kept
/// 62 instructions for want of it. Off: the pin's rule alone.
const LIFT_NON_LEADING: bool = true;

/// The order in which the two ways' instructions put the ones both have, equal
/// and free of anything ahead of them, first (after the leading run): a
/// register-only instruction that reads and writes nothing an earlier one of
/// its block touches, with no call before it. The tail both end with stays.
fn lifted_order(
    first: &[Inst],
    second: &[Inst],
) -> (Vec<usize>, Vec<usize>) {
    let tail = first.iter().rev().zip(second.iter().rev()).take_while(|(a, b)| a == b).count();
    let free = |insts: &[Inst]| -> Vec<usize> {
        let open = insts.len().saturating_sub(tail);
        let (mut read, mut written): (HashSet<Item>, HashSet<Item>) = (HashSet::default(), HashSet::default());
        let mut found = Vec::new();
        for (at, inst) in insts.iter().enumerate().take(open) {
            if matches!(inst, Inst::Branch { link: true, .. } | Inst::BranchReg { link: true, .. }) {
                break;
            }
            let (defs, uses) = inst.defs_and_uses();
            let (defs, uses): (Vec<Item>, Vec<Item>) =
                (defs.into_iter().map(Item::Register).collect(), uses.into_iter().map(Item::Register).collect());
            if movable(inst) == Some(false) {
                if let Some((w, r)) = effects(inst) {
                    if w.iter().all(|one| !read.contains(one) && !written.contains(one))
                        && r.iter().all(|one| !written.contains(one))
                    {
                        found.push(at);
                    }
                }
            }
            read.extend(uses);
            written.extend(defs);
            if matches!(inst, Inst::BranchCond { .. } | Inst::CompareBranch { .. } | Inst::TestBranch { .. }) {
                break;
            }
        }
        found
    };
    let (ours, theirs) = (free(first), free(second));
    let mut taken = vec![false; theirs.len()];
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for &at in &ours {
        let hit = (0..theirs.len()).find(|&slot| !taken[slot] && first[at] == second[theirs[slot]]);
        if let Some(slot) = hit {
            taken[slot] = true;
            pairs.push((at, theirs[slot]));
        }
    }
    let front = |len: usize, picks: Vec<usize>| -> Vec<usize> {
        let mut order = picks.clone();
        order.extend((0..len).filter(|at| !picks.contains(at)));
        order
    };
    (front(first.len(), pairs.iter().map(|p| p.0).collect()), front(second.len(), pairs.iter().map(|p| p.1).collect()))
}

/// The pin's `HoistCommonCodeInSuccs`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L2043>):
/// the instructions two successors of a conditional branch both begin with go
/// above the branch, when the block is the only way into either. They go before
/// the compare that sets up the branch (`findHoistingInsertPosAndDeps`, L1946).
/// Every instruction is hoisted only if it reads nothing the branch or the
/// compare writes and writes nothing they read or write.
fn hoist_common_code(
    blocks: &mut [Lowered],
    tables: &[Table],
) -> bool {
    let mut entering: HashMap<String, usize> = HashMap::default();
    for block in blocks.iter() {
        for inst in &block.insts {
            if let Some(label) = label_of(inst) {
                *entering.entry(label.to_owned()).or_default() += 1;
            }
        }
    }
    for entry in tables.iter().flat_map(|table| table.entries.iter()) {
        *entering.entry(entry.clone()).or_default() += 1;
    }
    if let Some(first) = blocks.first() {
        *entering.entry(first.label.clone()).or_default() += 1;
    }
    let index: HashMap<String, usize> = blocks.iter().enumerate().map(|(at, block)| (block.label.clone(), at)).collect();
    let mut changed = false;
    for at in 0..blocks.len() {
        changed |= hoist_in_successors(blocks, at, &entering, &index);
    }
    changed
}

fn hoist_in_successors(
    blocks: &mut [Lowered],
    at: usize,
    entering: &HashMap<String, usize>,
    index: &HashMap<String, usize>,
) -> bool {
    let insts = &blocks[at].insts;
    let [.., branch, Inst::Branch { link: false, target: Target::Label(other) }] = insts.as_slice() else {
        return false;
    };
    let Some(taken) = label_of(branch) else { return false };
    let mut uses: HashSet<Item> = HashSet::default();
    match branch {
        Inst::BranchCond { .. } => {
            uses.insert(Item::Flags);
        }
        Inst::CompareBranch { rt, .. } | Inst::TestBranch { rt, .. } => {
            uses.insert(Item::Register(crate::mc::visit::Reg::Gpr(*rt)));
        }
        _ => return false,
    }
    let (taken, other) = (taken.to_owned(), other.clone());
    if taken == other || taken == blocks[at].label || other == blocks[at].label {
        return false;
    }
    // Only this block enters either way.
    if entering.get(&taken) != Some(&1) || entering.get(&other) != Some(&1) {
        return false;
    }
    let (Some(&one), Some(&two)) = (index.get(&taken), index.get(&other)) else { return false };
    if one == at || two == at {
        return false;
    }
    // Where the instructions go: before the branch, or before the compare that
    // sets it up.
    let mut place = insts.len() - 2;
    let mut defs: HashSet<Item> = HashSet::default();
    if place > 0 {
        let before = &insts[place - 1];
        let (written, read) = match movable(before) {
            Some(_) => effects(before).unwrap_or_default(),
            None => {
                let (written, read) = before.defs_and_uses();
                (
                    written.into_iter().map(Item::Register).collect(),
                    read.into_iter().map(Item::Register).collect::<Vec<_>>(),
                )
            }
        };
        let sets_up = written.iter().any(|one| uses.contains(one));
        // A compare that is not a plain operation could set the flags in ways
        // not tracked: leave the block alone.
        let unknown_flags = movable(before).is_none() && uses.contains(&Item::Flags);
        if unknown_flags || (sets_up && movable(before).is_none()) {
            return false;
        }
        if sets_up {
            for one in &written {
                uses.remove(one);
                defs.insert(*one);
            }
            uses.extend(read);
            place -= 1;
        }
    }
    let leading = |first: &[Inst], second: &[Inst]| -> usize {
        let mut active: HashSet<Item> = HashSet::default();
        let mut run = 0;
        while let (Some(a), Some(b)) = (first.get(run), second.get(run)) {
            if a != b || movable(a) != Some(false) {
                break;
            }
            let Some((written, read)) = effects(a) else { break };
            if written.iter().any(|one| uses.contains(one) || defs.contains(one))
                || read.iter().any(|one| !active.contains(one) && defs.contains(one))
            {
                break;
            }
            active.extend(written);
            run += 1;
        }
        run
    };
    let mut run = leading(&blocks[one].insts, &blocks[two].insts);
    if LIFT_NON_LEADING {
        let (order_one, order_two) = lifted_order(&blocks[one].insts, &blocks[two].insts);
        let reorder = |insts: &[Inst], order: &[usize]| -> Vec<Inst> { order.iter().map(|&at| insts[at].clone()).collect() };
        let lifted = leading(&reorder(&blocks[one].insts, &order_one), &reorder(&blocks[two].insts, &order_two));
        if lifted > run {
            for (block, order) in [(one, order_one), (two, order_two)] {
                let locs: Vec<_> = order.iter().map(|&at| blocks[block].locs[at]).collect();
                blocks[block].insts = reorder(&blocks[block].insts, &order);
                blocks[block].locs = locs;
            }
            run = lifted;
        }
    }
    if run == 0 {
        return false;
    }
    let moved: Vec<(Inst, Option<(u32, u32)>)> = blocks[one]
        .insts
        .drain(..run)
        .zip(blocks[one].locs.drain(..run))
        .collect();
    blocks[two].insts.drain(..run);
    blocks[two].locs.drain(..run);
    for (offset, (inst, loc)) in moved.into_iter().enumerate() {
        blocks[at].insts.insert(place + offset, inst);
        blocks[at].locs.insert(place + offset, loc);
    }
    true
}

pub fn fold(
    mut blocks: Vec<Lowered>,
    tables: &mut [Table],
    odds: &Odds,
    optsize: bool,
    exits: &[crate::mc::visit::Reg],
) -> Vec<Lowered> {
    if blocks.is_empty() {
        return blocks;
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
    merge_return_tails(&mut blocks);
    hoist_common_code(&mut blocks, tables);
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
    let numbered = |old: usize| label_number(&blocks[old].label).is_some();
    let ids: Vec<u32> = reachable
        .iter()
        .enumerate()
        .map(|(new, &old)| label_number(&blocks[old].label).unwrap_or(u32::MAX - new as u32))
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
    let mut likelihood: HashMap<(String, String), u32> = HashMap::default();
    let placed: Vec<usize> = if edges.iter().any(|next| next.contains(&0)) {
        reachable.clone()
    } else {
        let cfg = Cfg::from_successors(edges.clone());
        let graph = Graph::from_cfg(&cfg, &ids, vec![false; ids.len()], &odds, optsize);
        for (from, next) in edges.iter().enumerate() {
            for &to in next {
                likelihood.insert(
                    (blocks[reachable[from]].label.clone(), blocks[reachable[to]].label.clone()),
                    graph.probability(from, to).get_numerator(),
                );
            }
        }
        let pinned = place(&graph);
        // DEVIATE: the old first-successor chain is kept where placement
        // executes more jumps by the same frequencies; the pin's
        // placement ignores that cost (quicksort's swap path gains a
        // `b`). On a tie the pin's order stands. Exit: none while the pin's
        // does.
        let jumps = |order: &[usize]| -> u128 {
            order
                .iter()
                .enumerate()
                .map(|(at, &new)| {
                    let insts = &blocks[reachable[new]].insts;
                    let jump = match insts.last() {
                        Some(Inst::Branch { link: false, target: Target::Label(label) }) => index.get(label.as_str()),
                        _ => None,
                    };
                    let cond = insts
                        .len()
                        .checked_sub(2)
                        .and_then(|before| label_of(&insts[before]))
                        .filter(|_| jump.is_some())
                        .and_then(|label| index.get(label));
                    let next = order.get(at + 1).map(|&one| reachable[one]);
                    let falls = |target: Option<&usize>| target.is_some_and(|t| Some(*t) == next);
                    let costs = match (cond, jump) {
                        (_, None) => false,
                        (None, Some(_)) => !falls(jump),
                        (Some(_), Some(_)) => !falls(jump) && !falls(cond),
                    };
                    u128::from(costs) * u128::from(graph.frequency(new).get_frequency())
                })
                .sum()
        };
        // The chain the selector's order gives: follow the last unplaced
        // successor, push the rest.
        let identity: Vec<usize> = {
            let mut placed: Vec<usize> = Vec::new();
            let mut seen = vec![false; edges.len()];
            let mut pending: Vec<usize> = vec![0];
            while let Some(start) = pending.pop() {
                let mut current = start;
                while !std::mem::replace(&mut seen[current], true) {
                    placed.push(current);
                    let mut chain = None;
                    for &candidate in edges[current].iter().rev() {
                        if seen[candidate] {
                            continue;
                        }
                        if chain.is_none() {
                            chain = Some(candidate);
                        } else {
                            pending.push(candidate);
                        }
                    }
                    match chain {
                        Some(next) => current = next,
                        None => break,
                    }
                }
            }
            placed
        };
        let order = if jumps(&pinned) <= jumps(&identity) { pinned } else { identity };
        order.into_iter().map(|new| reachable[new]).collect()
    };
    let mut taken: Vec<Option<Lowered>> = blocks.into_iter().map(Some).collect();
    let mut ordered: Vec<Lowered> = placed.into_iter().map(|at| taken[at].take().expect("placed once")).collect();
    // The pin copies in layout order (`TailDuplicator`, then
    // `MachineCopyPropagation`): a jump to the block next to it stays a
    // fall-through.
    for (jumper, original) in crate::taildup::tail_duplicate(&mut ordered, optsize) {
        // The copy keeps the original's edge odds (`addSuccessor` with the
        // original's probabilities), which `optimizeBranches` reads below.
        let edges: Vec<(String, u32)> =
            likelihood.iter().filter(|((from, _), _)| *from == original).map(|((_, to), odds)| (to.clone(), *odds)).collect();
        for (to, odds) in edges {
            likelihood.entry((jumper.clone(), to)).or_insert(odds);
        }
    }
    crate::taildup::drop_unreferenced(&mut ordered, tables);
    crate::taildup::propagate_backward(&mut ordered, exits);
    // A branch to the next block falls through; a conditional branch over an
    // unconditional one is the inverse conditional (`cbz`/`tbz` too).
    for at in 0..ordered.len() {
        let next = ordered.get(at + 1).map(|block| block.label.clone());
        let Lowered { insts, locs, label: ordered_label } = &mut ordered[at];
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
        // `optimizeBranches`: a block that ends in a conditional branch and a
        // jump takes the likelier successor by the conditional branch.
        if insts.len() >= 2
            && let Some(Inst::Branch { link: false, target: Target::Label(unlikely) }) = insts.last()
            && let Some(taken) = label_of(&insts[insts.len() - 2])
            && !matches!(insts[insts.len() - 2], Inst::Branch { .. })
            && let (Some(&via_jump), Some(&via_branch)) = (
                likelihood.get(&(ordered_label.clone(), unlikely.clone())),
                likelihood.get(&(ordered_label.clone(), taken.to_owned())),
            )
            && via_jump > via_branch
            && let Some(flipped) = inverse_over(&insts[insts.len() - 2], unlikely, Some(taken))
        {
            let jump = Inst::Branch { link: false, target: Target::Label(taken.to_owned()) };
            let len = insts.len();
            insts[len - 2] = flipped;
            insts[len - 1] = jump;
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
    /// where the pin has one (`rectwo`: +8 B). The test named the label each
    /// branch reached, so #9346's layout tie (the pin's order) turned it red
    /// with the same bytes.
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
        let out = fold(blocks, &mut [], &Odds::default(), false, &[]);
        let insts = || out.iter().flat_map(|block| &block.insts);
        let rets = insts().filter(|inst| matches!(inst, Inst::Ret { .. })).count();
        let ret_blocks: Vec<&str> = out
            .iter()
            .filter(|block| matches!(block.insts.as_slice(), [Inst::Ret { .. }]))
            .map(|block| block.label.as_str())
            .collect();
        let targets: Vec<&str> = insts()
            .filter(|inst| matches!(inst, Inst::CompareBranch { .. }))
            .filter_map(|inst| label_of(inst))
            .collect();
        assert_eq!(targets.len(), 2, "{out:?}");
        // Which of the two returns each branch reaches follows the layout
        // order; what matters is that none goes through a jump.
        assert!(targets.iter().all(|label| ret_blocks.contains(label)), "{out:?}");
        assert_eq!(insts().filter(|inst| matches!(inst, Inst::Branch { .. })).count(), 0, "{out:?}");
        assert!(rets <= 2, "{rets} returns: {out:?}");
    }

    /// The loop's header test was copied into its latch, and the copy lost the
    /// odds of its edges: the latch read `cbz exit; b top`, a jump every
    /// iteration (recpow -O1: +7.7% instructions over the pin).
    #[test]
    fn a_copied_header_test_branches_back_to_the_likelier_loop_block() {
        use llrm_mir::blockfrequency::BranchProbability;
        let jump = |to: &str| Inst::Branch { link: false, target: Target::Label(to.into()) };
        let ret = || Inst::Ret { rn: Gpr(30) };
        let blocks = vec![
            block(".Lf.0", vec![jump(".Lf.1")]),
            block(".Lf.1", vec![cbz(1, ".Lf.4"), jump(".Lf.2")]),
            block(".Lf.2", vec![cbz(2, ".Lf.5"), jump(".Lf.3")]),
            block(".Lf.3", vec![Inst::Nop, jump(".Lf.1")]),
            block(".Lf.4", vec![ret()]),
            block(".Lf.5", vec![Inst::Nop, ret()]),
        ];
        let mut odds = Odds::default();
        let mut edge = |from: u32, to: u32, numerator: u32, index: usize| {
            odds.insert((from, to), (BranchProbability::new(numerator, 32), index));
        };
        edge(1, 4, 1, 0);
        edge(1, 2, 31, 1);
        edge(2, 5, 16, 0);
        edge(2, 3, 16, 1);
        let out = fold(blocks, &mut [], &odds, false, &[]);
        let latch = out.iter().find(|block| block.insts.contains(&Inst::Nop) && block.label == ".Lf.3").expect("latch");
        let ends = format!("{:?}", &latch.insts[latch.insts.len() - 2..]);
        assert!(ends.contains("nonzero: true"), "{ends}");
    }

    fn movz(
        reg: u32,
        value: u16,
    ) -> Inst {
        Inst::MoveWide { op: crate::mc::inst::Wide::Zero, width: Width::X, rd: Gpr(reg), imm16: value, lsl: 0 }
    }

    fn compare(
        left: u32,
        right: u32,
    ) -> Inst {
        use crate::mc::inst::{AddSub, Shift};
        Inst::AddSubShifted {
            op: AddSub { subtract: true, set_flags: true },
            width: Width::W,
            rd: Gpr::R31,
            rn: Gpr(left),
            rm: Gpr(right),
            shift: Shift::Lsl,
            amount: 0,
        }
    }

    fn go(to: &str) -> Inst {
        Inst::Branch { link: false, target: Target::Label(to.into()) }
    }

    fn if_hs(to: &str) -> Inst {
        Inst::BranchCond { cond: crate::mc::inst::Cond::Hs, target: Target::Label(to.into()) }
    }

    /// Both ways of a guard began with `mov x9, #7`, and the pin hoists a
    /// leading identical run above the branch and the compare that sets it up
    /// (`HoistCommonCodeInSuccs`); only what differs stays.
    #[test]
    fn a_leading_run_both_ways_begin_with_goes_above_the_compare() {
        let mut blocks = vec![
            block("g", vec![compare(0, 1), if_hs("t"), go("f")]),
            block("t", vec![movz(9, 7), go("j")]),
            block("f", vec![movz(9, 7), Inst::Nop, go("j")]),
            block("j", vec![Inst::Ret { rn: Gpr(30) }]),
        ];
        assert!(hoist_common_code(&mut blocks, &[]));
        assert_eq!(blocks[0].insts[0], movz(9, 7), "{blocks:?}");
        assert_eq!(blocks[1].insts, vec![go("j")], "{blocks:?}");
        assert_eq!(blocks[2].insts, vec![Inst::Nop, go("j")], "{blocks:?}");
        assert_eq!(blocks[0].locs.len(), blocks[0].insts.len());
    }

    /// recpow -Os: `mov x11, xzr` begins the loop's preheader behind the
    /// prologue's pre-indexed `stp` (which writes register 31, the stack
    /// pointer) and is the whole guard-bypass block. `xzr` was taken for that
    /// register: the move was held below the branch, an exit block of 8 B (gcc 4).
    #[test]
    fn a_move_from_the_zero_register_is_not_held_by_a_write_of_the_stack_pointer() {
        use crate::mc::inst::{Addr, Memory};
        let int = |reg| Memory::Int { width: Width::X, bytes: 8, sign: false, reg: Gpr(reg) };
        let prologue = Inst::StorePair { first: int(29), second: int(30), addr: Addr::Pre { base: Gpr(31), imm: -16 } };
        let zero_x11 = Inst::LogicShifted {
            op: crate::mc::inst::Logic::Orr,
            invert: false,
            width: Width::X,
            rd: Gpr(11),
            rn: Gpr(31),
            rm: Gpr(31),
            shift: crate::mc::inst::Shift::Lsl,
            amount: 0,
        };
        let mut blocks = vec![
            block("g", vec![compare(0, 1), if_hs("t"), go("f")]),
            block("t", vec![prologue, zero_x11.clone(), go("l")]),
            block("f", vec![zero_x11.clone(), go("j")]),
            block("l", vec![go("j")]),
            block("j", vec![Inst::Ret { rn: Gpr(30) }]),
        ];
        assert!(hoist_common_code(&mut blocks, &[]));
        assert_eq!(blocks[0].insts[0], zero_x11, "{blocks:?}");
    }

    /// Both ways held `mov x9, #7` but not first (recchop -Os kept 62
    /// instructions): it is independent of what comes before it, so it is
    /// lifted to the front and hoisted above the branch (HC1, DEVIATE).
    #[test]
    fn an_identical_instruction_not_leading_in_both_ways_is_hoisted() {
        let mut blocks = vec![
            block("g", vec![compare(0, 1), if_hs("t"), go("f")]),
            block("t", vec![movz(10, 1), movz(9, 7), movz(12, 3), go("j")]),
            block("f", vec![movz(11, 2), movz(9, 7), movz(13, 4), go("j")]),
            block("j", vec![Inst::Ret { rn: Gpr(30) }]),
        ];
        assert!(hoist_common_code(&mut blocks, &[]));
        assert_eq!(blocks[0].insts[0], movz(9, 7), "{blocks:?}");
        assert_eq!(blocks[1].insts, vec![movz(10, 1), movz(12, 3), go("j")], "{blocks:?}");
        assert_eq!(blocks[2].insts, vec![movz(11, 2), movz(13, 4), go("j")], "{blocks:?}");
    }

    /// An instruction that reads what an earlier one of its way writes is not
    /// lifted past it.
    #[test]
    fn an_instruction_that_depends_on_one_before_it_is_not_lifted() {
        let mut blocks = vec![
            block("g", vec![compare(0, 1), if_hs("t"), go("f")]),
            block("t", vec![movz(9, 1), movz(9, 7), movz(12, 3), go("j")]),
            block("f", vec![movz(11, 2), movz(9, 7), movz(13, 4), go("j")]),
            block("j", vec![Inst::Ret { rn: Gpr(30) }]),
        ];
        assert!(!hoist_common_code(&mut blocks, &[]));
    }

    /// The hoisted `mov x0, #7` would change what the compare reads: it stays
    /// in both ways.
    #[test]
    fn an_instruction_that_writes_what_the_compare_reads_stays_below_the_branch() {
        let mut blocks = vec![
            block("g", vec![compare(0, 1), if_hs("t"), go("f")]),
            block("t", vec![movz(0, 7), go("j")]),
            block("f", vec![movz(0, 7), Inst::Nop, go("j")]),
            block("j", vec![Inst::Ret { rn: Gpr(30) }]),
        ];
        assert!(!hoist_common_code(&mut blocks, &[]));
        assert_eq!(blocks[1].insts.len(), 2);
    }

    /// A way another block also enters is not the guard's alone: nothing goes
    /// above the branch (`TBB->pred_size() > 1`).
    #[test]
    fn a_way_with_another_entrance_keeps_its_instructions() {
        let mut blocks = vec![
            block("g", vec![compare(0, 1), if_hs("t"), go("f")]),
            block("t", vec![movz(9, 7), go("j")]),
            block("f", vec![movz(9, 7), go("j")]),
            block("other", vec![go("t")]),
            block("j", vec![Inst::Ret { rn: Gpr(30) }]),
        ];
        assert!(!hoist_common_code(&mut blocks, &[]));
    }

    /// The folding pass runs it: the guard of the loop below begins with the
    /// instruction both ways began with (recchop's `mov x24, #0` shape, when
    /// the ways begin with it).
    #[test]
    fn folding_hoists_what_both_ways_of_a_guard_begin_with() {
        let blocks = vec![
            block(".Lf.0", vec![compare(0, 1), if_hs(".Lf.2"), go(".Lf.1")]),
            block(".Lf.1", vec![movz(9, 7), Inst::Nop, go(".Lf.3")]),
            block(".Lf.2", vec![movz(9, 7), go(".Lf.3")]),
            block(".Lf.3", vec![Inst::Ret { rn: Gpr(30) }]),
        ];
        let out = fold(blocks, &mut [], &Odds::default(), false, &[]);
        let count = out.iter().flat_map(|block| &block.insts).filter(|inst| **inst == movz(9, 7)).count();
        assert_eq!(count, 1, "{out:?}");
        assert_eq!(out[0].insts[0], movz(9, 7), "{out:?}");
    }
}
