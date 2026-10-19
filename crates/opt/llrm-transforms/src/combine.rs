//! One worklist over a function's instructions: each is visited, simplified to
//! a value it equals when its operands say so, and what read it is visited
//! again.
//!
//! A translation of LLVM's InstCombine driver,
//! [`InstructionCombining.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp)
//! at d1106deb71cc: `prepareWorklist` (:6031) seeds it, `run` (:5780) drains
//! it, `eraseInstFromFunction` (InstCombineInternal.h:504) re-queues what lost
//! a user, over
//! [`InstructionWorklist`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/Transforms/Utils/InstructionWorklist.h).
//! `visit` is [`llrm_mir::simplify::simplify_instruction_in`].
//!
//! MIRROR: the seed in reverse reverse-post-order after folding and deleting
//! the dead as it walks; the live-successor handling of a constant branch (the
//! phi inputs of a dead edge become poison); the pop, delete-if-dead, visit,
//! replace-all-uses, `pushUsersToWorkList`, erase sequence.
//!
//! DEVIATE, with the reason: code sinking (`tryToSinkInstruction`) is not made
//! (no counterpart in llrm yet: its cost is a number the combine page owes);
//! blocks no live edge reaches keep their instructions (`decide` removes
//! them; LLVM's `removeAllNonTerminatorAndEHPadInstructions` is a CFG edit
//! this pass does not make); a call is never trivially dead here (`dead` knows
//! what a call does not write); `visit` is `simplify_instruction_in` alone: the
//! rewrites of InstCombine's `visit*` functions are `algebraic`'s, to move in
//! as they are translated (the visit map in the combine page).
//!
//! # Used by
//! `fold` (its number step).

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::simplify::simplify_instruction_in;
use llrm_mir::valuetracking::FunctionAnalyses;

use crate::instworklist::InstWorklist;

/// `handleUseCountDecrement`: an operand that lost a user, and the one user it
/// has left, are looked at again.
fn handle_use_count_decrement(
    worklist: &mut InstWorklist,
    function: &Function,
    operand: Operand,
) {
    let Some(inst) = defining(function, operand) else { return };
    worklist.add(inst);
    if let Some(result) = function.instruction(inst).result
        && let Some(only) = function.users(result).only()
    {
        worklist.add(only.user);
    }
}

/// The instruction in the body that defines `operand`.
fn defining(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    function.parent(inst).map(|_| inst)
}

/// `isInstructionTriviallyDead`: nothing reads it and running it does nothing
/// else.
fn is_trivially_dead(
    function: &Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let Some(result) = instruction.result else { return false };
    if !function.users(result).is_empty() {
        return false;
    }
    matches!(
        instruction.opcode,
        Opcode::Binary(_)
            | Opcode::Cast(_)
            | Opcode::ICmp(_)
            | Opcode::FCmp(_)
            | Opcode::Select
            | Opcode::Phi
            | Opcode::GetElementPtr { .. }
            | Opcode::Alloca { .. }
            | Opcode::FNeg
            | Opcode::Freeze
            | Opcode::ExtractValue(_)
            | Opcode::InsertValue(_)
            | Opcode::Load { volatile: false, .. }
    )
}

/// `eraseInstFromFunction`: nothing reads `inst`; its operands are looked at
/// again.
fn erase(
    function: &mut Function,
    worklist: &mut InstWorklist,
    inst: InstId,
) {
    let operands = function.instruction(inst).operands.to_vec();
    worklist.remove(inst);
    function.erase(inst).expect("nothing reads it");
    for operand in operands {
        handle_use_count_decrement(worklist, function, operand);
    }
}

/// `InstCombinerImpl::prepareWorklist` (:6031): the blocks in reverse
/// post-order, each live one's instructions folded or deleted where that is
/// trivial, a constant branch's dead edge cut from the phis it feeds, then the
/// rest pushed so the first is popped first. Whether anything changed.
fn prepare(
    env: &Env,
    context: &mut Context,
    function: &mut Function,
    worklist: &mut InstWorklist,
) -> bool {
    let mut changed = false;
    let Some(entry) = function.entry() else { return false };
    let tree = env.analyses.dominator_tree();
    let graph = llrm_analysis::cfg::graph(function);
    let order = llrm_support::graph::reverse_postorder(&graph, llrm_analysis::cfg::id(entry));
    let mut dead_edges = std::collections::BTreeSet::<(BlockId, BlockId)>::new();
    let mut seeded: Vec<InstId> = Vec::new();
    for at in order {
        let block = llrm_analysis::cfg::block(at);
        if !tree.is_reachable(block) {
            continue;
        }
        // A block no live edge enters (every way in is dead, or comes from a
        // block it dominates) is skipped, its successors' edges from it
        // dead.
        if block != entry {
            let predecessors = function.predecessors(block);
            if predecessors.iter().all(|&pred| dead_edges.contains(&(pred, block)) || tree.dominates(block, pred)) {
                for succ in function.successors(block) {
                    handle_only_live_successor(context, function, &mut dead_edges, &mut changed, block, None, succ);
                }
                continue;
            }
        }
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            if function.parent(inst).is_none() {
                continue;
            }
            // ConstantProp an instruction that is trivially constant.
            let instruction = function.instruction(inst);
            let first_constant =
                instruction.operands.is_empty() || matches!(instruction.operands.first(), Some(Operand::Constant(_)));
            if let Some(result) = instruction.result
                && !function.users(result).is_empty()
                && first_constant
                && let Some(Operand::Constant(found)) =
                    simplify_instruction_in(context, env.globals, env.layout, Some(&env.analyses), function, inst)
            {
                function.replace_value(result, Operand::Constant(found));
                if is_trivially_dead(function, inst) {
                    let _ = function.erase(inst);
                }
                changed = true;
                continue;
            }
            seeded.push(inst);
        }
        // A branch on a number has one live successor.
        if let Some(branch) = function.terminator(block)
            && function.instruction(branch).opcode == Opcode::Br
            && let [Operand::Constant(condition), Operand::Block(taken), Operand::Block(other)] =
                function.instruction(branch).operands[..]
            && let Some(bits) = number(context, condition)
        {
            let (live, dead) = if bits & 1 == 1 { (taken, other) } else { (other, taken) };
            handle_only_live_successor(context, function, &mut dead_edges, &mut changed, block, Some(live), dead);
        }
    }
    // Pushed in reverse so the first instruction is the first popped, the dead
    // ones deleted as they are met.
    for inst in seeded.into_iter().rev() {
        if function.parent(inst).is_none() {
            continue;
        }
        if is_trivially_dead(function, inst) {
            let _ = function.erase(inst);
            changed = true;
            continue;
        }
        worklist.push(inst);
    }
    changed
}

/// A constant integer condition's value.
fn number(
    context: &Context,
    id: llrm_mir::context::ConstantId,
) -> Option<u128> {
    match context.get(id).kind {
        llrm_mir::context::ConstantKind::Int(bits) => Some(bits),
        llrm_mir::context::ConstantKind::Null | llrm_mir::context::ConstantKind::Zero => Some(0),
        _ => None,
    }
}

/// `HandleOnlyLiveSuccessor` (:6046): `block` goes to `live` only; the edge to
/// `dead` is dead, and the inputs of `dead`'s phis that come over it are
/// poison.
fn handle_only_live_successor(
    context: &mut Context,
    function: &mut Function,
    dead_edges: &mut std::collections::BTreeSet<(BlockId, BlockId)>,
    changed: &mut bool,
    block: BlockId,
    live: Option<BlockId>,
    dead: BlockId,
) {
    if Some(dead) == live || !dead_edges.insert((block, dead)) {
        return;
    }
    for phi in function.iter_block(dead).collect::<Vec<_>>() {
        if function.instruction(phi).opcode != Opcode::Phi {
            break;
        }
        let ty = function.instruction(phi).ty;
        let poison = Operand::Constant(
            context.constant(llrm_mir::context::Constant { ty, kind: llrm_mir::context::ConstantKind::Poison }),
        );
        let arms: Vec<Operand> = function.instruction(phi).operands.to_vec();
        for (at, pair) in arms.chunks(2).enumerate() {
            if let [value, Operand::Block(from)] = *pair
                && from == block
                && !matches!(
                    value,
                    Operand::Constant(id) if context.get(id).kind == llrm_mir::context::ConstantKind::Poison
                )
            {
                function.set_operand(phi, at * 2, poison);
                *changed = true;
            }
        }
    }
}

/// `InstCombinerImpl::run` (:5780): drain the worklist. Whether anything
/// changed.
fn drain(
    env: &Env,
    context: &mut Context,
    function: &mut Function,
    worklist: &mut InstWorklist,
) -> bool {
    let mut changed = false;
    while !worklist.is_empty() {
        // Walk deferred instructions, deleting the dead among them (and so,
        // chain by chain, what only they read), and push the rest.
        while let Some(inst) = worklist.pop_deferred() {
            if function.parent(inst).is_none() {
                continue;
            }
            if is_trivially_dead(function, inst) {
                erase(function, worklist, inst);
                changed = true;
                continue;
            }
            worklist.push(inst);
        }
        let Some(inst) = worklist.remove_one() else { continue };
        if function.parent(inst).is_none() {
            continue;
        }
        // Check to see if we can DCE the instruction.
        if is_trivially_dead(function, inst) {
            erase(function, worklist, inst);
            changed = true;
            continue;
        }
        // Visit: the value it equals, if any.
        let Some(result) = function.instruction(inst).result else { continue };
        let Some(found) =
            simplify_instruction_in(context, env.globals, env.layout, Some(&env.analyses), function, inst)
        else {
            continue;
        };
        if found == Operand::Value(result) {
            continue;
        }
        // Everything uses the new value now; the users are looked at again.
        let users: Vec<InstId> = function.users(result).iter().map(|one| one.user).collect();
        function.replace_value(result, found);
        for user in users {
            worklist.push(user);
        }
        if let Some(new) = defining(function, found) {
            worklist.push(new);
        }
        erase(function, worklist, inst);
        changed = true;
    }
    changed
}

/// What `visit` reads besides the function.
struct Env<'a> {
    globals: &'a [GlobalValue],
    layout: &'a DataLayout,
    analyses: FunctionAnalyses,
}

/// One combine of `function`: the worklist seeded and drained. Whether anything
/// changed.
pub fn combine(
    context: &mut Context,
    globals: &[GlobalValue],
    metadata: &[MetadataNode],
    layout: &DataLayout,
    function: &mut Function,
) -> bool {
    let env = Env { globals, layout, analyses: FunctionAnalyses::of(context, globals, metadata, layout, function) };
    let mut worklist = InstWorklist::default();
    let mut changed = prepare(&env, context, function, &mut worklist);
    changed |= drain(&env, context, function, &mut worklist);
    changed
}

#[cfg(test)]
#[path = "combine_tests.rs"]
mod tests;
