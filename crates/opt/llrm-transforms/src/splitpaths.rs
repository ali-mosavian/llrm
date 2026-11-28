// SPDX-License-Identifier: GPL-3.0-or-later
//! Path splitting: gcc's
//! [`gimple-ssa-split-paths.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/gimple-ssa-split-paths.cc)
//! (`pass_split_paths`, -O3), as the rewrite of MIR before isel.
//!
//!
//! # How it works
//! A loop's latch `J` that is the join of an if-then-else diamond (`d` ends in
//! a conditional, each arm of `J` is `d` or a block that only `d` reaches and
//! that only reaches `J`) is copied into the arm that has a block of its own
//! and merged into it. The phis of `J` take the arm's value in the copy, so
//! what read them (an address, an index, the loop's test) is computed from a
//! constant or a value the arm already has, and `J` and its copy are two
//! latches.
//!
//! `is_feasible_trace` decides: the join must be small (under 15 statements),
//! must not look like an if-conversion candidate (two arms of one statement
//! each that feed the same phi), and must have a phi that something in the join
//! uses (a use that is only a compare does not count), or a header phi with an
//! unchanged path, or an equality the dominating branch decides.
//!
//! | # | DEVIATE |
//! |---|---|
//! | SP-1 | gcc 15.2 treats a join of arms of at most one statement each, and at most three phis, as an if-conversion candidate (`poor_ifcvt_pred`) and does not split it. gcc 13.4, which the bars use, takes only two arms of exactly one assignment each that feed the same phi. Here the narrow form: an empty arm (`side = c ? 1 : 2`) is split, as gcc 13 does (phi_slot_unknown_arm: gcc 15 does not split, 13 does, 205,539 against 207,458 Ir). Owner llrm-target-x86-m64-b; exit when the bars use gcc 15. |
//! | SP-2 | Done on MIR before isel, not on GIMPLE: the copy is merged into its arm at once (gcc merges after `cleanup_cfg`), and the values the join defined that are read beyond it are repaired with an `SsaUpdater`. Owner llrm-target-x86-m64-b; exit 2026-11-30. |
//! | SP-3 | Run only where the target says so (`Tti::split_paths_at_o3`) and at -O3, as gcc's `-fsplit-paths` is on at -O3: a target is measured before it takes it. Owner llrm-target-x86-m64-b; exit 2026-11-30. |
//!
//! # Example
//! Before, `%j` joins the arms of `%d`, the loop's latch:
//!
//! ```mir
//! d:
//!   br i1 %c, label %j, label %arm
//! arm:
//!   br label %j
//! j:
//!   %side = phi i32 [ 1, %d ], [ 2, %arm ]
//!   %next = add i32 %i, %side
//!   %again = icmp ne i32 %next, 0
//!   br i1 %again, label %d, label %out
//! ```
//!
//! After, `arm` holds a copy of `j` with `%side` as 2:
//!
//! ```mir
//! arm:
//!   %next.2 = add i32 %i, 2
//!   %again.2 = icmp ne i32 %next.2, 0
//!   br i1 %again.2, label %d, label %out
//! j:
//!   %next = add i32 %i, 1
//!   ...
//! ```
//!
//! From unit test `test_a_join_of_an_empty_arm_is_copied_into_it` in
//! `splitpaths_tests.rs`.
//!
//! # Runs
//! Once per body, at -O3 on a target that asks, after `cgpaddr` and before
//! `ree`. With `wanted("splitpaths")`.
//!
//! # Needs
//! Dominators and loops.
//!
//! # Keeps
//! Nothing.
//!
//! # See also
//! Tests: `llrm-transforms/src/splitpaths_tests.rs`. Levels:
//! `docs/reference/levels.md`.

use std::collections::BTreeSet;

use llrm_analysis::ssaupdater::SsaUpdater;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::module::{BlockId, Function, InstId, Operand, Use};
use llrm_mir::opcode::{Attribute, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};

/// gcc's `--param max-jump-thread-duplication-stmts`.
const MAX_JUMP_THREAD_DUPLICATION_STMTS: usize = 15;

/// `pass_split_paths`.
pub struct SplitPaths;

impl FunctionPass for SplitPaths {
    fn name(&self) -> &'static str {
        "splitpaths"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let wanted = analyses.outer().target().tti().is_some_and(|tti| tti.split_paths_at_o3());
        // `optimize_loop_for_speed_p`: not a function kept small.
        let for_size = unit
            .function
            .attrs
            .iter()
            .any(
                |attr| matches!(
                    attr,
                    Attribute::Flag(flag) if flag == "optsize" || flag == "minsize"
                ),
            );
        // gcc: at least two real blocks and a back edge.
        if !wanted || for_size || unit.function.layout().len() < 3 {
            return PreservedAnalyses::all();
        }
        let tree = analyses.get::<Dominators>(unit.context, unit.layout, unit.function);
        let loops = analyses.get::<Loops>(unit.context, unit.layout, unit.function);
        let mut plans = Vec::new();
        let mut taken = BTreeSet::new();
        for one in loops.get_loops_in_preorder().into_iter().rev() {
            let Some(latch) = one.get_loop_latch(&*unit.function) else { continue };
            let Some(plan) = find_block_to_duplicate_for_splitting_paths(unit.function, one.get_header(), latch) else {
                continue;
            };
            let blocks = [plan.join, plan.dominator, plan.arm, plan.other];
            if blocks.iter().any(|block| taken.contains(block)) || !is_feasible_trace(unit.function, &tree, &plan) {
                continue;
            }
            taken.extend(blocks);
            plans.push(plan);
        }
        if plans.is_empty() {
            return PreservedAnalyses::all();
        }
        for plan in plans {
            transform_duplicate(unit.context, unit.layout, unit.function, &plan);
        }
        PreservedAnalyses::none()
    }
}

/// The diamond `find_block_to_duplicate_for_splitting_paths` found: `join` is
/// the loop's latch, `dominator` ends in the conditional, `arm` is the
/// predecessor of `join` that has a block of its own.
struct Plan {
    header: BlockId,
    join: BlockId,
    dominator: BlockId,
    arm: BlockId,
    /// The other predecessor of `join`: `dominator` itself or a second arm.
    other: BlockId,
}

/// A block that ends in a two-way `br` to two different blocks.
fn ends_in_conditional(
    function: &Function,
    block: BlockId,
) -> bool {
    let Some(last) = function.terminator(block) else { return false };
    let last = function.instruction(last);
    last.opcode == Opcode::Br
        && matches!(
            last.operands[..],
            [_, Operand::Block(taken), Operand::Block(other)] if taken != other
        )
}

/// A block that ends in an unconditional `br`.
fn ends_in_jump(
    function: &Function,
    block: BlockId,
) -> bool {
    function
        .terminator(block)
        .is_some_and(
            |last| {
                let last = function.instruction(last);
                last.opcode == Opcode::Br && last.operands.len() == 1
            },
        )
}

/// `find_block_to_duplicate_for_splitting_paths`: the latch `join` of the loop,
/// when it joins an if-then-else.
fn find_block_to_duplicate_for_splitting_paths(
    function: &Function,
    header: BlockId,
    join: BlockId,
) -> Option<Plan> {
    // gcc's latch forwarder has one predecessor, `join`, whose conditional
    // leaves the loop or goes round.
    if join == header || !ends_in_conditional(function, join) {
        return None;
    }
    let preds: Vec<BlockId> = function.predecessors(join).iter().copied().collect();
    let [first, second] = preds[..] else { return None };
    if first == join || second == join {
        return None;
    }
    // The dominator ends in a conditional too, and each predecessor is it or a
    // block only it reaches.
    let only_pred = |block: BlockId| {
        let preds = function.predecessors(block);
        (preds.len() == 1).then(|| preds[0])
    };
    let candidates = [Some(first), Some(second), only_pred(first), only_pred(second)];
    for dominator in candidates.into_iter().flatten() {
        if dominator == join || !ends_in_conditional(function, dominator) {
            continue;
        }
        let reaches = |block: BlockId| {
            block == dominator
                || (only_pred(block) == Some(dominator)
                    && ends_in_jump(function, block)
                    && function.successors(block).len() == 1)
        };
        if !reaches(first) || !reaches(second) || (first == dominator && second == dominator) {
            continue;
        }
        // The path that is split is the predecessor with a block of its own.
        let (arm, other) = if first != dominator { (first, second) } else { (second, first) };
        return Some(Plan { header, join, dominator, arm, other });
    }
    None
}

/// The instructions of `block` after its phis.
fn body(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).filter(|&inst| function.instruction(inst).opcode != Opcode::Phi).collect()
}

/// The statements of `block` as gcc counts them: its body without the jump that
/// falls into the next block (gcc has no statement for an unconditional edge).
fn statements(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    let mut all = body(function, block);
    if all.last().is_some_and(|&last| {
        let last = function.instruction(last);
        last.opcode == Opcode::Br && last.operands.len() == 1
    }) {
        all.pop();
    }
    all
}

fn phis_of(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function.iter_block(block).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect()
}

fn phi_arms(
    function: &Function,
    phi: InstId,
) -> Vec<(Operand, BlockId)> {
    function
        .instruction(phi)
        .operands
        .chunks(2)
        .map(|pair| match pair {
            [value, Operand::Block(from)] => (*value, *from),
            _ => unreachable!("a phi's arms are value and block"),
        })
        .collect()
}

fn set_phi_arms(
    function: &mut Function,
    phi: InstId,
    arms: &[(Operand, BlockId)],
) {
    function.set_operands(phi, arms.iter().flat_map(|(value, from)| [*value, Operand::Block(*from)]).collect());
}

/// `count_stmts_in_block`, of a predecessor only where it has one way out
/// (gcc's `num_stmts_in_pred`).
fn statements_of_arm(
    function: &Function,
    block: BlockId,
) -> usize {
    if function.successors(block).len() == 1 { statements(function, block).len() } else { 0 }
}

/// `poor_ifcvt_candidate_code`: a statement that does not if-convert to one
/// instruction.
fn poor_ifcvt_candidate(
    function: &Function,
    inst: InstId,
) -> bool {
    function.instruction(inst).opcode == Opcode::Select
}

/// What `GIMPLE_ASSIGN` is in MIR: an instruction that makes a value without a
/// side effect or a branch.
fn is_assignment(
    function: &Function,
    inst: InstId,
) -> bool {
    let opcode = &function.instruction(inst).opcode;
    function.instruction(inst).result.is_some()
        && !matches!(
            opcode,
            Opcode::Phi | Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Atomic(_) | Opcode::VaArg
        )
}

/// `is_feasible_trace`: whether duplicating the join is worth it.
fn is_feasible_trace(
    function: &Function,
    tree: &llrm_mir::domtree::DomTree,
    plan: &Plan,
) -> bool {
    let join = plan.join;
    let num_stmts_in_join = statements(function, join).len();
    if num_stmts_in_join >= MAX_JUMP_THREAD_DUPLICATION_STMTS {
        return false;
    }
    let (first, second) = (plan.arm, plan.other);
    // SP-1: the arms of one assignment each that feed the same phi are an
    // if-conversion candidate.
    if statements_of_arm(function, first) == 1 && statements_of_arm(function, second) == 1 {
        let (a, b) = (statements(function, first)[0], statements(function, second)[0]);
        if is_assignment(function, a)
            && is_assignment(function, b)
            && !poor_ifcvt_candidate(function, a)
            && !poor_ifcvt_candidate(function, b)
        {
            let (a, b) = (function.instruction(a).result, function.instruction(b).result);
            for phi in phis_of(function, join) {
                let arms = phi_arms(function, phi);
                let values: Vec<Option<_>> = arms
                    .iter()
                    .map(|(value, _)| match value {
                        Operand::Value(value) => Some(*value),
                        _ => None,
                    })
                    .collect();
                if let [Some(x), Some(y)] = values[..]
                    && ((Some(x) == a && Some(y) == b) || (Some(y) == a && Some(x) == b))
                {
                    return false;
                }
            }
        }
    }
    // A phi with a use that can be simplified in the copy, or that makes the
    // loop's own phi's unchanged path a threading opportunity.
    let header = Some(plan.header);
    let mut found_useful_phi = false;
    'phis: for phi in phis_of(function, join) {
        let Some(result) = function.instruction(phi).result else { continue };
        for used in function.users(result) {
            let user = function.instruction(used.user);
            let in_join = function.parent(used.user) == Some(join);
            if in_join && !matches!(user.opcode, Opcode::ICmp(_) | Opcode::FCmp(_)) {
                found_useful_phi = true;
                break 'phis;
            }
            if user.opcode == Opcode::Phi && function.parent(used.user) == header {
                let header_value = Operand::Value(user.result.expect("a phi's value"));
                let unchanged = phi_arms(function, phi).iter().any(|(value, _)| *value == header_value);
                if unchanged {
                    for second_use in function.users(user.result.expect("a phi's value")) {
                        let block = function.parent(second_use.user);
                        let Some(block) = block else { continue };
                        if block != join && tree.dominates(block, join) {
                            let decider = function.instruction(second_use.user);
                            if matches!(
                                decider.opcode,
                                Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)
                            ) {
                                found_useful_phi = true;
                            }
                            break;
                        }
                    }
                }
                if found_useful_phi {
                    break 'phis;
                }
            }
        }
    }
    // The exception: an equality the dominating branch decides, which holds in
    // the join.
    let mut found_cprop_opportunity = false;
    if let Some(last) = function.terminator(plan.dominator)
        && let Some(Operand::Value(condition)) = function.instruction(last).operands.first().copied()
        && let llrm_mir::module::ValueDef::Instruction(compare) = function.value(condition).def
        && matches!(
            function.instruction(compare).opcode,
            Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne)
        )
    {
        for operand in function.instruction(compare).operands.clone() {
            if let Operand::Value(value) = operand
                && function.users(value).iter().any(|one| function.parent(one.user) == Some(join))
            {
                found_cprop_opportunity = true;
                break;
            }
        }
    }
    found_useful_phi || found_cprop_opportunity
}

/// `transform_duplicate`: a copy of the join, merged into the arm `plan.arm`;
/// the phis of the join take the arm's values in the copy.
fn transform_duplicate(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    plan: &Plan,
) {
    let (join, arm) = (plan.join, plan.arm);
    let join_phis = phis_of(function, join);
    let mut map = ValueMap::default();
    for &phi in &join_phis {
        let result = function.instruction(phi).result.expect("a phi's value");
        let (value, _) = phi_arms(function, phi).into_iter().find(|(_, from)| *from == arm).expect("an arm of the phi");
        map.values.insert(result, value);
    }
    // The arm falls into the copy.
    let jump = function.terminator(arm).expect("the arm ends in a jump");
    function.erase(jump).expect("a jump has no readers");
    let mut copies = Vec::new();
    for inst in body(function, join) {
        let copy = function.clone_instruction(inst);
        function.insert(copy, Position::End(arm)).expect("a placed block");
        if let (Some(from), Some(to)) = (function.instruction(inst).result, function.instruction(copy).result) {
            map.values.insert(from, Operand::Value(to));
        }
        copies.push(copy);
    }
    for &copy in &copies {
        function.remap_instruction(copy, &map);
    }
    let remapped = |operand: Operand| match operand {
        Operand::Value(value) => map.values.get(&value).copied().unwrap_or(operand),
        other => other,
    };
    // The successors of the join are the copy's too: their phis hear from the
    // arm.
    for successor in function.successors(join) {
        for phi in phis_of(function, successor) {
            let mut arms = phi_arms(function, phi);
            let Some(&(value, _)) = arms.iter().find(|(_, from)| *from == join) else { continue };
            arms.push((remapped(value), arm));
            set_phi_arms(function, phi, &arms);
        }
    }
    // The join no longer hears from the arm.
    for &phi in &join_phis {
        let arms: Vec<_> = phi_arms(function, phi).into_iter().filter(|(_, from)| *from != arm).collect();
        set_phi_arms(function, phi, &arms);
    }
    // What the join made and is read beyond it is made twice now.
    let originals: Vec<_> = join_phis.iter().copied().chain(body(function, join)).collect();
    for inst in originals {
        let Some(result) = function.instruction(inst).result else { continue };
        let copy = match map.values.get(&result) {
            Some(&copy) => copy,
            None => continue,
        };
        let uses: Vec<Use> = function
            .users(result)
            .iter()
            .copied()
            .filter(|one| function.parent(one.user) != Some(join) && function.parent(one.user) != Some(arm))
            .collect();
        if uses.is_empty() {
            continue;
        }
        let mut updater = SsaUpdater::new(function.value(result).ty, None);
        updater.add_available_value(join, Operand::Value(result));
        updater.add_available_value(arm, copy);
        for one in uses {
            updater.rewrite_use(context, layout, function, one);
        }
    }
    // A phi with one arm left is its value.
    for &phi in &join_phis {
        let arms = phi_arms(function, phi);
        if let [(value, _)] = arms[..] {
            let result = function.instruction(phi).result.expect("a phi's value");
            function.replace_value(result, value);
            function.erase(phi).expect("no readers");
        }
    }
}

#[cfg(test)]
#[path = "splitpaths_tests.rs"]
mod tests;
