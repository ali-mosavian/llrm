//! A branch in a loop's header on a loop-invariant condition, one side of
//! which leaves the loop, is decided in the preheader instead of on every
//! trip.
//!
//! # How it works
//! The loop needs one preheader that jumps only to the header. The header's
//! terminator is a conditional `br` on a value defined outside the loop, with
//! one target outside the loop (the exit) and one inside. Whichever way the
//! branch goes on the first trip it goes on every trip, so the preheader takes
//! that branch and the header falls through to the loop body.
//!
//! Nothing is copied. Three conditions keep the move sound:
//! - What the header computes before its branch is phis and work that can be
//!   speculated, because it is skipped where the loop is never entered.
//! - A value the loop defines is used outside it only by the exit's phis.
//! - The exit's phis take their value on the new edge from a value defined
//!   outside the loop, or from a header phi's value on entry. Any other value
//!   the loop computes is not there yet, and keeps the branch.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b>"]:::mid
//!   h["<b>head</b><br/><small>br z, out, test</small>"]:::mid
//!   t["<b>test</b><br/><small>the body</small>"]:::mid
//!   o["<b>out</b><br/><small>phi [acc, head]</small>"]:::mid
//!   pre --> h
//!   h -->|"z"| o
//!   h -->|"not z"| t
//!   t --> h
//! ```
//!
//! *Before: the header tests z on every trip*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pre["<b>pre</b><br/><small>br z, out, head</small>"]:::mid
//!   h["<b>head</b><br/><small>br test</small>"]:::mid
//!   t["<b>test</b><br/><small>the body</small>"]:::mid
//!   o["<b>out</b><br/><small>phi [acc_entry, pre]</small>"]:::mid
//!   pre -->|"z"| o
//!   pre -->|"not z"| h
//!   h --> t
//!   t --> h
//! ```
//!
//! *After: the preheader tests z once*
//!
//! Tail recursion makes such loops: `paths(n, m)` tests `n == 0` on every trip
//! of the loop that tests `m == 0`. A condition inside the body, or a branch
//! that stays in the loop on both sides, is [`crate::simpleloopunswitch`]'s,
//! which copies the loop.
//!
//! A rotated loop, one whose header jumps back to itself, is left alone (TU1).
//!
//! The pass repeats while a loop changes.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @pipeline.body(ptr nocapture readonly %0, i16 %1, i16 %2) ... {
//!   ; ...
//!   %9 = load i16, ptr %4, !tbaa !9
//!   %10 = icmp eq i16 %9, 0
//!   %11 = zext i1 %10 to i16
//!   %12 = load i16, ptr %5, !tbaa !9
//!   %13 = load ptr, ptr %3, !tbaa !21
//!   br label %b2
//!
//! b2:
//!   br i1 %10, label %b4, label %27
//!   ; ...
//! ```
//! After:
//! ```mir
//! define ... @pipeline.body(ptr nocapture readonly %0, i16 %1, i16 %2) ... {
//!   ; ...
//!   %9 = load i16, ptr %4, !tbaa !9
//!   %10 = icmp eq i16 %9, 0
//!   %11 = zext i1 %10 to i16
//!   %12 = load i16, ptr %5, !tbaa !9
//!   %13 = load ptr, ptr %3, !tbaa !21
//!   br i1 %10, label %b2, label %27
//!
//! b2:
//!   br label %b4
//!   ; ...
//! ```
//! `%10` is `z == 0`, defined outside the loop, and `b2` is the loop's header.
//! The preheader now takes the branch to the exit `27` or into the loop, and
//! `b2` goes straight to the body `b4`.
//!
//! From `--dump` of `scan` (`while (z == 0) { if (i >= n) break; s += a[i];
//! i++; }`) at -O2 -m16, stages `hoist` and `trivialunswitch` (frame slots and
//! the loop body left out).
//!
//! # Runs
//! In the function pipeline's fixed point, after `loopmotion` and before
//! `gvn`. At every level above -O0; there is no flag for it alone.
//!
//! # Needs
//! The loop shape ([`crate::loopsimplify`] gives the preheader) and callees'
//! effects, to tell work that can be speculated.
//!
//! # Keeps
//! Nothing: a change invalidates every analysis.
//!
//! # See also
//! LLVM's SimpleLoopUnswitch, trivial form (`unswitchTrivialBranch`). It
//! follows a chain of blocks that each continue unconditionally; this pass
//! looks only at the header.

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::Context;
use llrm_mir::footprint::Footprint;
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

    /// A branch is moved out of a loop where its condition is invariant: the
    /// condition's values and the memory they load, and the loop's shape.
    fn reads(&self) -> Footprint {
        Footprint::CONTROL | Footprint::MEMORY | Footprint::STRUCTURE
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let mut changed = false;
        while unswitched(unit.context, outer.callees(), unit.function) {
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
    for loop_ in &cfg::loops_of(function) {
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
    /// Whether the exit is the branch's first target, the one taken when the
    /// condition holds.
    exit_if_true: bool,
    condition: Operand,
}

fn planned(
    context: &Context,
    callees: &Callees,
    function: &Function,
    loop_: &Loop,
) -> Option<Plan> {
    let header = cfg::block(loop_.header);
    let outside: Vec<BlockId> =
        function.predecessors(header).into_iter().filter(|one| !loop_.body.contains(&cfg::id(*one))).collect();
    let [preheader] = outside[..] else { return None };
    if function.successors(preheader) != [header] {
        return None;
    }
    // TU1 A rotated loop, whose header tests the exit after its body, is
    // left alone: SimpleLoopUnswitch's trivial path (the translation that
    // replaces this pass) decides it from the exiting block.
    if function.successors(header).contains(&header) {
        return None;
    }
    let branch = function.terminator(header)?;
    let instruction = function.instruction(branch);
    let [condition, Operand::Block(first), Operand::Block(second)] = instruction.operands[..] else { return None };
    let Operand::Value(value) = condition else { return None };
    let defined_in =
        |block: BlockId| function.iter_block(block).any(|one| function.instruction(one).result == Some(value));
    if loop_.body.iter().any(|&at| defined_in(cfg::block(at))) {
        return None;
    }
    let (exit_if_true, exit, stays) =
        match (loop_.body.contains(&cfg::id(first)), loop_.body.contains(&cfg::id(second))) {
            (false, true) => (true, first, second),
            (true, false) => (false, second, first),
            _ => return None,
        };
    // What the header does before the branch is skipped where the loop is never
    // entered: it does nothing but phis and work that cannot trap, and its
    // results leave only through the exit's phis.
    for inst in function.iter_block(header) {
        if inst == branch || function.instruction(inst).opcode == Opcode::Phi {
            continue;
        }
        if !memory::speculatable(context, callees, function, inst) {
            return None;
        }
    }
    // Each value a loop block defines is used outside the loop only by the
    // exit's phis, which the new edge can supply.
    let in_loop = |value| match function.value(value).def {
        ValueDef::Instruction(def) => function.parent(def).is_some_and(|block| loop_.body.contains(&cfg::id(block))),
        _ => false,
    };
    for &at in &loop_.body {
        for inst in function.iter_block(cfg::block(at)) {
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
            // A header phi has its value on entry; anything else the loop
            // computes is not there yet.
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
    // The exit's phis take the header's value on the new edge from the
    // preheader: a header phi's value on entry.
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
