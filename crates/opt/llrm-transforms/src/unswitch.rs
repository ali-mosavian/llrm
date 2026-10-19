//! A loop with an invariant branch in its body becomes two loops, one per
//! way the condition goes, chosen by one test before them.
//!
//! # How it works
//! A candidate is a loop with one block outside it entering the header
//! (the entry), at most 128 operations, and in its body (not the header) a
//! block ending in a conditional `br` on an `icmp` whose operands are not
//! defined in the loop and whose definitions dominate the entry. The
//! first such branch of the first such loop is taken. The comparison is pure
//! by construction: an `icmp` reads only values.
//!
//! The pass works on a copy of the function with the loops closed
//! ([`crate::lcssa`]). [`crate::loopclone`]'s `peeled` makes one clone of the
//! loop; the entry gets a copy of the comparison and a `br` on it to the
//! clone's header (condition holds) or the original header (it does not). In
//! the clone the branch is replaced by its true arm; in the original by its
//! false arm. The clone's latch goes back to the clone's header, so each
//! version is a loop of its own, and the arm each no longer takes is
//! unreachable and removed.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   en["<b>entry</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   b["<b>B ? X : Y</b>"]:::mid
//!   x["<b>X</b>"]:::mid
//!   y["<b>Y</b>"]:::mid
//!   en --> h
//!   h --> b
//!   b --> h
//!   b --> x
//!   b --> y
//! ```
//!
//! *Before*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   en["<b>entry</b><br/><small>%c = icmp ..<br/>br %c, H', H</small>"]:::mid
//!   h1["<b>H'</b>"]:::mid
//!   b1["<b>B'</b>"]:::mid
//!   x1["<b>X'</b>"]:::mid
//!   h["<b>H</b>"]:::mid
//!   b["<b>B</b>"]:::mid
//!   y["<b>Y</b>"]:::mid
//!   en --> h1
//!   en --> h
//!   h1 --> b1
//!   b1 --> h1
//!   b1 --> x1
//!   h --> b
//!   b --> h
//!   b --> y
//! ```
//!
//! *After: entry tests %c = icmp ..*
//!
//! The copy is a candidate, not yet a result. It is run through the same
//! pipeline with unswitching and unroll-and-jam off (`Unswitch::passes`),
//! then judged against the original by [`crate::candidate`], on the work
//! alone: kept only if it has fewer loops, no more operations and no larger
//! price. Otherwise the original stays untouched.
//!
//! # Example
//! Before:
//! ```mir
//! define i16 @f(i16 %n, i16 %k) {
//! b0:
//!   br label %b1
//!
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
//!   %sum = phi i16 [ 0, %b0 ], [ %total, %b4 ]
//!   %more = icmp ult i16 %i, %n
//!   br i1 %more, label %b2, label %b5
//!
//! b2:
//!   %flag = icmp eq i16 %k, 0
//!   br i1 %flag, label %b3, label %b4
//!
//! b3:
//!   %bumped = add i16 %sum, %i
//!   br label %b4
//!   ; ...
//! }
//! ```
//! After:
//! ```mir
//! define i16 @f(i16 %n, i16 %k) {
//! b0:
//!   %0 = icmp eq i16 %k, 0
//!   br i1 %0, label %12, label %11
//!
//! b1:
//!   %i = phi i16 [ 0, %11 ], [ %next, %b4 ]
//!   ; ...
//! b2:
//!   %flag = icmp eq i16 %k, 0
//!   br label %b4
//!   ; ...
//! 1:
//!   %2 = phi i16 [ 0, %12 ], [ %10, %9 ]
//!   %3 = phi i16 [ 0, %12 ], [ %8, %9 ]
//!   ; ...
//! 5:
//!   %6 = icmp eq i16 %k, 0
//!   br label %7
//!
//! 7:
//!   %8 = add i16 %3, %2
//!   br label %9
//!   ; ...
//! }
//! ```
//! The test of `%k` is made once, in `b0`. `b1` to `b4` are the loop for `k !=
//! 0`, with the arm that adds gone; `1` to `9` are the copy for `k == 0`, whose
//! branch is now the add alone. The pipeline then keeps this candidate only if
//! it has fewer loops and no higher price.
//!
//! From unit test `test_an_invariant_branch_specializes_the_loop` in
//! `unswitch_tests.rs`, the candidate `specialized` makes, before it is
//! re-optimized.
//!
//! # Runs
//! Once per body, after the fixed point and the peel rounds settle and before
//! unroll-jam. -O3 and -Omax: `-funswitch-loops`.
//!
//! # Needs
//! [`crate::lcssa`], [`crate::loopclone`], [`crate::candidate`], and the
//! target's operation costs.
//!
//! # Keeps
//! Nothing when it unswitches: the function is replaced by the candidate.
//!
//! # See also
//! A branch in the header that leaves the loop is
//! [`crate::trivialunswitch`]'s: nothing is copied. gcc's
//! `tree-ssa-loop-unswitch.cc`; LLVM's SimpleLoopUnswitch, non-trivial form.

use std::collections::BTreeMap;

use llrm_analysis::cfg;
use llrm_analysis::graph::loops::{self, Loop};
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};

use crate::candidate::{self, Baseline, Gain, Pricing};
use crate::lcssa::{self, arms, from_arms, operations};
use crate::profit::{self, OperationCosts};
use crate::{edges, loopclone, transform};

pub struct Unswitch {
    /// What a specialized candidate goes through before it is judged.
    pub passes: Vec<Box<dyn FunctionPass>>,
}

impl FunctionPass for Unswitch {
    fn name(&self) -> &'static str {
        "unswitch"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let costs = profit::costs(&outer);
        let passes = &mut self.passes;
        let mut reoptimize = |candidate: &mut Unit| {
            for pass in passes.iter_mut() {
                pass.run(candidate, &mut analyses.fresh());
            }
        };
        match optimized(unit, &outer, &costs, &mut reoptimize) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("unswitch: {error}"),
        }
    }
}

/// `unit`'s function specialized and re-optimized, kept only when that
/// removed a loop without growing the function or its price; whether it was.
/// `outer` is what the prices' analyses read of the module.
pub fn optimized(
    unit: &mut Unit,
    outer: &Outer,
    costs: &OperationCosts,
    reoptimize: &mut dyn FnMut(&mut Unit),
) -> Result<bool, String> {
    let Some(candidate) = specialized(unit.context, unit.function)? else {
        return Ok(false);
    };
    Ok(candidate::adopted(unit, outer, costs, candidate, reoptimize, Pricing::Work, Gain::FewerLoops, Baseline::Kept))
}

/// `function` with its first loop that has a pure invariant condition
/// specialized into a copy for each way the condition goes, or `None`.
pub fn specialized(
    context: &Context,
    function: &Function,
) -> Result<Option<Function>, String> {
    let mut closed = function.clone();
    lcssa::closed(&mut closed)?;
    let mut owners = BTreeMap::new();
    for (block, inst) in closed.walk() {
        if let Some(value) = closed.instruction(inst).result {
            owners.insert(value, cfg::id(block));
        }
    }
    let graph = cfg::graph(&closed);
    let cfg::Shape { dominance, loops: found, .. } = cfg::Shape::of(&closed);
    let predecessors = loops::predecessors(&graph);
    for loop_ in found {
        let outside =
            predecessors[&loop_.header].iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>();
        if outside.len() != 1
            || loop_.body.iter().map(|&at| operations(&closed, cfg::block(at)).len()).sum::<usize>() > 128
        {
            continue;
        }
        let entry = outside[0];
        for block in &graph {
            let at = cfg::block(block.at);
            if !loop_.body.contains(&block.at)
                || block.at == loop_.header
                || block.succ.len() != 2
                || operations(&closed, at).is_empty()
            {
                continue;
            }
            let branch = closed.terminator(at).expect("a terminated block");
            let Some(compare) = transform::_comparison(&closed, at, branch) else {
                continue;
            };
            if closed.instruction(compare).operands.iter().any(|operand| match operand {
                Operand::Value(value) => owners
                    .get(value)
                    .is_some_and(|&owner| loop_.body.contains(&owner) || !dominance.dominates(function, owner, entry)),
                _ => false,
            }) {
                continue;
            }
            let mut copied = closed.clone();
            let Some(labels) = loopclone::peeled(&mut copied, &loop_, 1)? else {
                continue;
            };
            _specialized(&closed, &mut copied, &loop_, entry, block.at, compare, &labels[0])?;
            llrm_analysis::cfg::_unreachable(context, &mut copied);
            transform::_trivial_phis(&mut copied)?;
            return Ok(Some(copied));
        }
    }
    Ok(None)
}

/// `copied`, `body` with one iteration of `loop_` peeled into `labels`,
/// made a second loop: `entry` tests `compare` once and runs the copy when
/// it holds, the original when not, and in each `selected` goes one way.
pub fn _specialized(
    body: &Function,
    copied: &mut Function,
    loop_: &Loop,
    entry: i64,
    selected: i64,
    compare: InstId,
    labels: &BTreeMap<i64, BlockId>,
) -> Result<(), String> {
    let latch = *loop_.latches.first().expect("one latch");
    let header = cfg::block(loop_.header);
    let (cloned_header, cloned_latch) = (labels[&loop_.header], labels[&latch]);
    let parent = cfg::block(entry);

    let last = copied.terminator(parent).expect("a terminated block");
    let guard = copied.clone_instruction(compare);
    copied.insert(guard, Position::Before(last))?;
    let condition = Operand::Value(copied.instruction(guard).result.expect("a compare's value"));
    let operands = vec![condition, Operand::Block(cloned_header), Operand::Block(header)];
    let dispatch = copied.create_instruction(Opcode::Br, copied.instruction(last).ty, operands, Flags::default(), None);
    copied.insert(dispatch, Position::Before(last))?;
    copied.erase(last)?;

    // The original header reads the original latch again; the copy's header
    // takes the copy's latch in its place.
    for (phi, cloned) in edges::phis(copied, header).into_iter().zip(edges::phis(copied, cloned_header)) {
        let residual = arms(copied, phi).into_iter().find(|&(_, source)| source == cloned_latch).ok_or("KeyError")?;
        copied.set_operands(phi, body.instruction(phi).operands.clone());
        let mut incoming = arms(copied, cloned);
        incoming.push(residual);
        copied.set_operands(cloned, from_arms(&incoming));
    }
    let jump = copied.terminator(cloned_latch).expect("a terminated block");
    edges::retarget(copied, jump, header, cloned_header);

    for (at, taken) in [(cfg::block(selected), false), (labels[&selected], true)] {
        let branch = copied.terminator(at).ok_or("IndexError")?;
        let destination = copied.instruction(branch).operands[if taken { 1 } else { 2 }];
        copied.set_operands(branch, vec![destination]);
    }

    for target in [header, cloned_header] {
        edges::split(copied, parent, target, Vec::new())?;
    }
    for version in [loop_.body.iter().map(|&at| cfg::block(at)).collect::<Vec<_>>(), labels.values().copied().collect()]
    {
        let exits = version
            .iter()
            .flat_map(|&source| copied.successors(source).into_iter().map(move |target| (source, target)))
            .filter(|(_, target)| !version.contains(target))
            .collect::<Vec<_>>();
        for (source, target) in exits {
            if edges::conditional(copied, source, target) {
                edges::split(copied, source, target, Vec::new())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "unswitch_tests.rs"]
mod tests;
