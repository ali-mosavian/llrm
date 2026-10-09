//! LLVM's SimpleLoopUnswitch, the trivial form (`unswitchTrivialBranch`): a branch on a loop-invariant
//! condition that leaves the loop on one side is decided before the loop, not on every trip.
//!
//! The branch, in the loop's header, goes to its preheader: whichever way it goes the first time it goes
//! every time, so the loop is entered only where it stays, and where it leaves the header's tests are
//! never made. Nothing is copied. Tail recursion makes such loops (`paths(n, m)` tests `n == 0` on every
//! trip of the loop that tests `m == 0`); unswitch.rs copies the loop for a condition inside its body and
//! leaves the header's alone.
//!
//! Where it differs from LLVM: only the loop's header holds the branch (LLVM follows a chain of blocks
//! that each continue unconditionally), and the exit's phis take their value on the new edge only from
//! a value defined outside the loop or a header phi's value on entry; any other value keeps the branch.

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::Context;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::edges;
use crate::lcssa::{arms, from_arms};

pub struct TrivialUnswitch;

impl FunctionPass for TrivialUnswitch {
    fn name(&self) -> &'static str {
        "trivialunswitch"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let callees = analyses.outer().callees().clone();
        let mut changed = false;
        while unswitched(unit.context, &callees, unit.function) {
            changed = true;
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// Whether one loop's header branch was moved to its preheader.
pub fn unswitched(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
) -> bool {
    let shape = cfg::Shape::of(function);
    for loop_ in &shape.loops {
        if let Some(plan) = planned(context, callees, function, loop_) {
            moved(function, plan);
            return true;
        }
    }
    false
}

struct Plan {
    header: BlockId,
    preheader: BlockId,
    exit: BlockId,
    stays: BlockId,
    /// Whether the exit is the branch's first target, the one taken when the condition holds.
    exit_if_true: bool,
    condition: Operand,
}

fn planned(
    context: &Context,
    callees: &Callees,
    function: &Function,
    loop_: &Loop,
) -> Option<Plan> {
    let graph = cfg::graph(function);
    let header = cfg::block(loop_.header);
    let outside: Vec<BlockId> =
        function.predecessors(header).into_iter().filter(|one| !loop_.body.contains(&cfg::id(*one))).collect();
    let [preheader] = outside[..] else { return None };
    if function.successors(preheader) != [header] {
        return None;
    }
    let branch = function.terminator(header)?;
    let instruction = function.instruction(branch);
    let [condition, Operand::Block(first), Operand::Block(second)] = instruction.operands[..] else { return None };
    let Operand::Value(value) = condition else { return None };
    let defined_in = |block: BlockId| {
        function.block(block).instructions().iter().any(|&one| function.instruction(one).result == Some(value))
    };
    if loop_.body.iter().any(|&at| defined_in(cfg::block(at))) {
        return None;
    }
    let (exit_if_true, exit, stays) =
        match (loop_.body.contains(&cfg::id(first)), loop_.body.contains(&cfg::id(second))) {
            (false, true) => (true, first, second),
            (true, false) => (false, second, first),
            _ => return None,
        };
    // What the header does before the branch is skipped where the loop is never entered: it does nothing but phis and
    // work that cannot trap, and its results leave only through the exit's phis.
    for &inst in function.block(header).instructions() {
        if inst == branch || function.instruction(inst).opcode == Opcode::Phi {
            continue;
        }
        if !memory::speculatable(context, callees, function, inst) {
            return None;
        }
    }
    let _ = graph;
    // Each value a loop block defines is used outside the loop only by the exit's phis, which the new edge can supply.
    let in_loop = |value| match function.value(value).def {
        ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| loop_.body.contains(&cfg::id(block))),
        _ => false,
    };
    for &at in &loop_.body {
        for &inst in function.block(cfg::block(at)).instructions() {
            let Some(result) = function.instruction(inst).result else { continue };
            for one in function.users(result) {
                let user = one.user;
                let owner = function.parent(user)?;
                if loop_.body.contains(&cfg::id(owner)) {
                    continue;
                }
                if owner != exit || function.instruction(user).opcode != Opcode::Phi {
                    return None;
                }
            }
        }
    }
    for phi in edges::phis(function, exit) {
        for (operand, from) in arms(function, phi) {
            if from != header {
                continue;
            }
            let Operand::Value(carried) = operand else { continue };
            if !in_loop(carried) {
                continue;
            }
            // A header phi has its value on entry; anything else the loop computes is not there yet.
            let own = matches!(
                function.value(carried).def,
                ValueDef::Instruction(def) if function.parent(def) == Some(header) && function.instruction(def).opcode == Opcode::Phi
            );
            if !own {
                return None;
            }
        }
    }
    Some(Plan { header, preheader, exit, stays, exit_if_true, condition })
}

fn moved(
    function: &mut Function,
    plan: Plan,
) {
    let Plan { header, preheader, exit, stays, exit_if_true, condition } = plan;
    // The exit's phis take the header's value on the new edge from the preheader: a header phi's value on entry.
    for phi in edges::phis(function, exit) {
        let mut incoming = arms(function, phi);
        let mut added = Vec::new();
        for &(operand, from) in &incoming {
            if from != header {
                continue;
            }
            let entry = match operand {
                Operand::Value(value) => match function.value(value).def {
                    ValueDef::Instruction(def)
                        if function.parent(def) == Some(header) && function.instruction(def).opcode == Opcode::Phi =>
                    {
                        arms(function, def)
                            .into_iter()
                            .find(|&(_, source)| source == preheader)
                            .map_or(operand, |(first, _)| first)
                    }
                    _ => operand,
                },
                other => other,
            };
            added.push((entry, preheader));
        }
        incoming.retain(|&(_, from)| from != header);
        incoming.extend(added);
        function.set_operands(phi, from_arms(&incoming));
    }
    // The header goes on to the loop; the preheader decides.
    let branch = function.terminator(header).expect("a terminated header");
    let void = function.instruction(branch).ty;
    let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(stays)], Default::default(), None);
    function.insert(jump, llrm_mir::edit::Position::Before(branch)).expect("a placed block");
    function.erase(branch).expect("the branch had no users");
    let last = function.terminator(preheader).expect("a terminated preheader");
    let targets = if exit_if_true { [exit, header] } else { [header, exit] };
    let decide = function.create_instruction(
        Opcode::Br,
        void,
        vec![condition, Operand::Block(targets[0]), Operand::Block(targets[1])],
        Default::default(),
        None,
    );
    function.insert(decide, llrm_mir::edit::Position::Before(last)).expect("a placed block");
    function.erase(last).expect("the jump had no users");
}

#[cfg(test)]
#[path = "trivialunswitch_tests.rs"]
mod tests;
