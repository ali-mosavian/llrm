//! Which condition of a loop to unswitch non-trivially, and whether it is
//! cheap enough: `collectUnswitchCandidates`,
//! `findBestNonTrivialUnswitchCandidate` and `CalculateUnswitchCostMultiplier`.
//! Reads the function; changes nothing.
//!
//! # How it works
//! A candidate is a terminator of the loop's own blocks (not an inner loop's)
//! with the loop-invariant values to unswitch on: a branch or `select` on one
//! invariant condition, a branch on an `and`/`or` graph with invariant inputs,
//! or a switch on an invariant. Each is costed by the loop code that stays in
//! one copy only (`computeDomSubtreeCost`), scaled by the copies it adds and
//! by a multiplier that grows with the candidates left, the sibling loops and
//! the size of the parent loop. The cheapest is taken if its cost is below
//! `unswitch-threshold`.

use llrm_mir::codemetrics::collect_ephemeral_values;
use llrm_mir::dense::IdMap;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::tti::CostKind;

use super::{Analyzed, Run, collect_homogenous_inst_graph_loop_invariants, def_of, logical, skip_trivial_select};

/// `-unswitch-threshold`.
const UNSWITCH_THRESHOLD: i64 = 50;

/// The threshold the target's tuning table asks for, else the pin's.
fn unswitch_threshold(run: &Run) -> i64 {
    run.tti.unswitch_threshold_override().unwrap_or(UNSWITCH_THRESHOLD)
}
/// `-unswitch-siblings-toplevel-div`.
const UNSWITCH_SIBLINGS_TOPLEVEL_DIV: i64 = 2;
/// `-unswitch-parent-blocks-div`.
const UNSWITCH_PARENT_BLOCKS_DIV: i64 = 8;
/// `-unswitch-num-initial-unscaled-candidates`.
const UNSWITCH_NUM_INITIAL_UNSCALED_CANDIDATES: i64 = 8;

/// `NonTrivialUnswitchCandidate`.
#[derive(Clone, Debug)]
pub(super) struct Candidate {
    pub terminator: InstId,
    pub invariants: Vec<Operand>,
}

/// The unique successors of `block`'s terminator, in operand order.
pub(super) fn successors(
    run: &Run,
    block: BlockId,
) -> Vec<BlockId> {
    run.function.successors(block).into_iter().collect()
}

/// `collectUnswitchCandidates`, without guards (MIR has none), partially
/// invariant conditions (no MemorySSA) or injected conditions (SU9).
pub(super) fn collect(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let add_for = |run: &Run, inst: InstId, cond: Operand, candidates: &mut Vec<Candidate>| {
        let cond = skip_trivial_select(run, cond);
        if matches!(cond, Operand::Constant(_)) {
            return;
        }
        if lp.is_loop_invariant(run.function, cond) {
            candidates.push(Candidate { terminator: inst, invariants: vec![cond] });
            return;
        }
        let Some(root) = def_of(run, cond) else { return };
        if logical(run, root, true).is_some() || logical(run, root, false).is_some() {
            let invariants = collect_homogenous_inst_graph_loop_invariants(run, lp, root);
            if !invariants.is_empty() {
                candidates.push(Candidate { terminator: inst, invariants });
            }
        }
    };
    for &block in lp.get_blocks() {
        if analyzed.loops.get_loop_for(block).map(|one| one.get_header()) != Some(lp.get_header()) {
            continue;
        }
        for inst in run.function.iter_block(block) {
            let end = run.function.instruction(inst);
            // Do not unswitch vector selects and logical and/or selects.
            if end.opcode == Opcode::Select
                && run.context.types.int_bits(operand_type(run, end.operands[0])) == Some(1)
                && run.context.types.int_bits(end.ty) != Some(1)
            {
                add_for(run, inst, end.operands[0], &mut candidates);
            }
        }
        let Some(term) = run.function.terminator(block) else { continue };
        let end = run.function.instruction(term);
        if end.opcode == Opcode::Switch {
            // We can only consider fully loop-invariant switch conditions as
            // we need to completely eliminate the switch after unswitching.
            let cond = end.operands[0];
            if !matches!(cond, Operand::Constant(_))
                && lp.is_loop_invariant(run.function, cond)
                && successors(run, block).len() != 1
            {
                candidates.push(Candidate { terminator: term, invariants: vec![cond] });
            }
            continue;
        }
        if end.opcode != Opcode::Br || end.operands.len() != 3 || end.operands[1] == end.operands[2] {
            continue;
        }
        add_for(run, term, end.operands[0], &mut candidates);
    }
    candidates
}

fn operand_type(
    run: &Run,
    operand: Operand,
) -> llrm_mir::types::TypeId {
    run.function.operand_type(run.context, operand).expect("a typed operand")
}

/// `computeDomSubtreeCost`: the cost of the dominator subtree of `block`
/// restricted to the blocks in `costs`, memoized. Walked without recursion: a
/// chain of blocks is as deep as it is long.
fn dom_subtree_cost(
    analyzed: &Analyzed,
    block: BlockId,
    costs: &IdMap<BlockId, i64>,
    memo: &mut IdMap<BlockId, i64>,
) -> i64 {
    if !costs.contains_key(&block) {
        return 0;
    }
    if let Some(&done) = memo.get(&block) {
        return done;
    }
    // Post-order over the subtree: a node is summed once its children are.
    let mut stack = vec![(block, false)];
    while let Some((at, children_done)) = stack.pop() {
        if memo.contains_key(&at) {
            continue;
        }
        let children: Vec<BlockId> =
            analyzed.tree.children(Some(at)).into_iter().filter(|child| costs.contains_key(child)).collect();
        if children_done {
            let sum = children.iter().map(|child| memo[child]).sum::<i64>() + costs[&at];
            memo.insert(at, sum);
        } else {
            stack.push((at, true));
            stack.extend(children.into_iter().filter(|child| !memo.contains_key(child)).map(|child| (child, false)));
        }
    }
    memo[&block]
}

fn log2(value: i64) -> i64 {
    i64::from(value.max(1).ilog2())
}

/// `CalculateUnswitchCostMultiplier`.
fn cost_multiplier(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
    candidate: &Candidate,
    candidates: &[Candidate],
) -> i64 {
    let tree = &analyzed.tree;
    let latch = lp.get_loop_latch(run.function).expect("a loop in simplified form has a latch");
    let cond_block = run.function.parent(candidate.terminator).expect("a placed terminator");
    let successors_in_loop = |block: BlockId| successors(run, block).into_iter().filter(|&to| lp.contains(to)).count();
    // Guards and other exiting conditions do not contribute to exponential
    // explosion as soon as they dominate the latch (otherwise there might be
    // another path to the latch remaining that does not allow to eliminate the
    // loop copy on unswitch).
    if tree.dominates(cond_block, latch)
        && run.function.instruction(candidate.terminator).opcode.is_terminator()
        && successors_in_loop(cond_block) <= 1
    {
        return 1;
    }

    // When unswitching conditions in nested loops, the basic block size of the
    // outer loop should not be altered.
    let parent = lp.get_parent_loop();
    let parent_loop_size_multiplier =
        parent.map_or(1, |one| (one.get_num_blocks() as i64 / UNSWITCH_PARENT_BLOCKS_DIV).max(1));
    let siblings_count = match parent {
        Some(one) => one.get_sub_loops().count(),
        None => analyzed.loops.top_level_loops().count(),
    } as i64;

    // Count amount of clones that all the candidates might cause during
    // unswitching. Branch/select counts as 1, switch counts as log2 of its
    // cases.
    let mut unswitched_clones = 0;
    for other in candidates {
        let block = run.function.parent(other.terminator).expect("a placed terminator");
        let skip_exiting_successors = tree.dominates(block, latch);
        if run.function.instruction(other.terminator).opcode == Opcode::Select {
            unswitched_clones += 1;
            continue;
        }
        let non_exiting = successors(run, block).into_iter().filter(|&to| !skip_exiting_successors || lp.contains(to));
        unswitched_clones += log2(non_exiting.count() as i64);
    }

    // Ignore up to the "unscaled candidates" number of unswitch candidates
    // when calculating the power-of-two scaling of the cost.
    let clones_power = (unswitched_clones - UNSWITCH_NUM_INITIAL_UNSCALED_CANDIDATES).max(0);

    // Allowing top-level loops to spread a bit more than nested ones.
    let siblings_multiplier =
        (if parent.is_some() { siblings_count } else { siblings_count / UNSWITCH_SIBLINGS_TOPLEVEL_DIV }).max(1);
    // Saturating at the threshold keeps the product from overflowing.
    let threshold = unswitch_threshold(run);
    if clones_power > log2(threshold) || siblings_multiplier > threshold || parent_loop_size_multiplier > threshold {
        threshold
    } else {
        (siblings_multiplier * (1 << clones_power)).min(threshold)
    }
}

/// `findBestNonTrivialUnswitchCandidate` and the threshold test of
/// `unswitchBestCondition`: the cheapest candidate, if its cost is below the
/// threshold.
pub(super) fn best(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
    candidates: &[Candidate],
) -> Option<Candidate> {
    // Given that unswitching these terminators will require duplicating parts
    // of the loop, model that cost: ephemeral values do not count, and each
    // block's cost is cached across the subsets of the loop considered.
    let assumes: Vec<InstId> = analyzed
        .assumptions
        .assumes
        .iter()
        .copied()
        .filter(|&assume| run.function.parent(assume).is_some_and(|block| lp.contains(block)))
        .collect();
    let eph = {
        let function = &*run.function;
        let effects_free = |inst: InstId| llrm_mir::memory::only_value(run.context, run.callees, function, inst);
        collect_ephemeral_values(function, &effects_free, &assumes, &|block| lp.contains(block))
    };
    let minimum_size = run
        .function
        .attrs
        .iter()
        .any(
            |attr| matches!(
                attr,
                llrm_mir::opcode::Attribute::Flag(flag) if flag == "minsize"
            ),
        );
    let kind = if minimum_size { CostKind::CodeSize } else { CostKind::SizeAndLatency };
    let mut costs: IdMap<BlockId, i64> = IdMap::new();
    let mut loop_cost = 0;
    for &block in lp.get_blocks() {
        let mut cost = 0;
        for inst in run.function.iter_block(block) {
            if eph.contains(&inst) {
                continue;
            }
            cost += instruction_cost(run.tti, run.context, run.layout, run.globals, run.function, inst, kind).value;
        }
        loop_cost += cost;
        costs.insert(block, cost);
    }

    // The non-duplicated cost of unswitching a terminator.
    let mut memo: IdMap<BlockId, i64> = IdMap::new();
    let mut unswitched_cost = |candidate: &Candidate, full_unswitch: bool| -> i64 {
        // Unswitching selects unswitches the entire loop.
        if run.function.instruction(candidate.terminator).opcode == Opcode::Select {
            return loop_cost;
        }
        let block = run.function.parent(candidate.terminator).expect("a placed terminator");
        let operands = run.function.instruction(candidate.terminator).operands.clone();
        let mut cost = 0;
        let unique = successors(run, block);
        for &succ in &unique {
            // If this is a partial unswitch candidate, then it must be a
            // conditional branch with a condition of either `or` or `and`,
            // their select forms. In that case, one of the successors is
            // necessarily duplicated, so don't even try to remove its cost.
            if !full_unswitch {
                let cond = skip_trivial_select(run, operands[0]);
                let root = def_of(run, cond).expect("a partial candidate's condition is an instruction");
                let Operand::Block(on_true) = operands[1] else { unreachable!("a branch's target") };
                let Operand::Block(on_false) = operands[2] else { unreachable!("a branch's target") };
                if logical(run, root, true).is_some() {
                    if succ == on_false {
                        continue;
                    }
                } else if logical(run, root, false).is_some() && succ == on_true {
                    continue;
                }
            }
            // This successor's domtree will not need to be duplicated after
            // unswitching if the edge to the successor dominates it (and thus
            // the entire tree): no other path leads into this subtree, so it
            // ends up live in only one clone of the loop.
            let preds = run.function.predecessors(succ);
            if preds.len() == 1 || preds.iter().all(|&pred| pred == block || analyzed.tree.dominates(succ, pred)) {
                cost += dom_subtree_cost(analyzed, succ, &costs, &mut memo);
            }
        }
        // Scale by the number of unique successors minus one: there is
        // already at least one copy of the entire loop.
        (loop_cost - cost) * (unique.len() as i64 - 1)
    };

    let mut best: Option<(i64, &Candidate)> = None;
    for candidate in candidates {
        let term = run.function.instruction(candidate.terminator);
        let full_unswitch = term.opcode != Opcode::Br
            || (candidate.invariants.len() == 1
                && candidate.invariants[0] == skip_trivial_select(run, term.operands[0]));
        let multiplier = cost_multiplier(run, analyzed, lp, candidate, candidates);
        let cost = unswitched_cost(candidate, full_unswitch) * multiplier;
        llrm_support::debug!(
            "simpleloopunswitch",
            "candidate in b{}: cost {cost} (multiplier {multiplier}, loop cost {loop_cost})",
            run.function.parent(candidate.terminator).map_or(0, |block| block.0)
        );
        if best.is_none_or(|(lowest, _)| cost < lowest) {
            best = Some((cost, candidate));
        }
    }
    let (cost, candidate) = best?;
    llrm_support::debug!("simpleloopunswitch", "lowest cost found: {cost}");
    (cost < unswitch_threshold(run)).then(|| candidate.clone())
}
