//! A loop with a test of its counter against an invariant in its body becomes
//! two loops, one for the trips before the test flips and one for those after,
//! neither with the test.
//!
//! # How it works
//! A candidate is a loop `induction::counted` proves to count up by one,
//! tested before each trip with `<` (signed or unsigned), that has one latch,
//! one outside predecessor (the entry), no exit but its header's, and at most
//! 128 operations. In its body (not the header) a block ends in a `br` on an
//! `icmp` of the counter and a value that is not defined in the loop and whose
//! definition dominates the entry. The counter only rises and does not wrap,
//! so the test holds for a prefix of the trips and fails for the rest
//! (`i < c`, `i <= C`), or the reverse (`i >= c`, `i > C`). `<=` and `>` are
//! taken for constants below the type's largest, as `i < C + 1`.
//!
//! The pass works on a copy of the function with the loops closed
//! ([`crate::lcssa`]). [`crate::loopclone`]'s `peeled` makes one clone of the
//! loop, which runs first. The entry computes `min(c, n)`, `n` being the
//! loop's bound, and the clone's header tests the counter against it, leaving
//! for the original loop's header instead of the exit. The original loop
//! tests `n` as before and takes up with the clone's counters. In each loop
//! the branch goes the one way it does in that part of the trips.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   en["<b>entry</b>"]:::mid
//!   h["<b>H: i &lt; n</b>"]:::mid
//!   b["<b>B: i &lt; c ? X : Y</b>"]:::mid
//!   ex["<b>exit</b>"]:::mid
//!   en --> h
//!   h --> b
//!   b --> h
//!   h --> ex
//! ```
//!
//! *Before*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   en["<b>entry</b><br/><small>m = min(c, n)</small>"]:::mid
//!   h1["<b>H': i &lt; m</b>"]:::mid
//!   x["<b>X</b>"]:::mid
//!   h["<b>H: i &lt; n</b>"]:::mid
//!   y["<b>Y</b>"]:::mid
//!   ex["<b>exit</b>"]:::mid
//!   en --> h1
//!   h1 --> x
//!   x --> h1
//!   h1 --> h
//!   h --> y
//!   y --> h
//!   h --> ex
//! ```
//!
//! *After*
//!
//! The split is kept without a price: each loop has fewer operations a trip
//! than the one it came from, and the trips are shared out between them. The
//! growth is bounded: a loop of more than 128 operations is left, and each
//! loop is split once a run.
//!
//! # Example
//! Before:
//! ```mir
//! define void @f(ptr %a, i16 %n, i16 %k) {
//! b0:
//!   br label %b1
//!
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
//!   %more = icmp slt i16 %i, %n
//!   br i1 %more, label %b2, label %b5
//!
//! b2:
//!   %early = icmp slt i16 %i, %k
//!   br i1 %early, label %b3, label %b4
//!   ; ... b3 stores 1, b4 steps %i
//! }
//! ```
//! After:
//! ```mir
//! b0:
//!   %0 = icmp slt i16 %k, %n
//!   %1 = select i1 %0, i16 %k, i16 %n
//!   br label %b6
//!
//! b6:
//!   %i1 = phi i16 [ 0, %b0 ], [ %next1, %b8 ]
//!   %more1 = icmp slt i16 %i1, %1
//!   br i1 %more1, label %b7, label %b9
//!   ; ... the loop with the store alone
//! b1:
//!   %i = phi i16 [ %i1, %b9 ], [ %next, %b4 ]
//!   %more = icmp slt i16 %i, %n
//!   ; ... the loop without it
//! ```
//!
//! From unit test `an_early_test_of_the_counter_splits_the_loop` in
//! `loopsplit_tests.rs`.
//!
//! # Runs
//! Once per body, after unswitching and before unroll-jam, at -O3 and -Omax
//! (`-fsplit-loops`). The halves go through the scalar rounds again.
//!
//! # Needs
//! [`crate::lcssa`], [`crate::loopclone`], `induction`'s counted proofs.
//!
//! # Keeps
//! Nothing when it splits: the function is replaced by the candidate.
//!
//! # See also
//! Implements gcc's `tree-ssa-loop-split.cc` (`split_loop`, `-fsplit-loops`).
//! LLVM has no scalar counterpart (IRCE splits for range checks, #1194).
//! [`crate::unswitch`] does the same for a test that does not change.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::{self, Loop};
use llrm_analysis::induction::{self, AffineOperand, CountedLoop, Reach};
use llrm_analysis::manager::Registers;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};

use crate::counting::Seeds;
use crate::lcssa::{self, arms, from_arms, operations};
use crate::{edges, loopclone, transform};

/// Operations of a loop that is copied.
const MAX_LOOP_OPS: usize = 128;

pub struct Split;

impl FunctionPass for Split {
    fn name(&self) -> &'static str {
        "split"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match optimized(unit, analyses) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("split: {error}"),
        }
    }
}

/// Every loop with a split test, once each; whether any was.
pub fn optimized(
    unit: &mut Unit,
    analyses: &Analyses,
) -> Result<bool, String> {
    let mut split = BTreeSet::<i64>::new();
    while let Some((candidate, latch)) = _candidate(unit.context, unit.layout, unit.function, analyses, &split)? {
        llrm_support::debug!("split", "split the loop with latch b{latch}");
        split.insert(latch);
        *unit.function = candidate;
    }
    Ok(!split.is_empty())
}

/// What a body block's test of the counter says of the trips.
struct Test {
    block: i64,
    /// The test holds for the first trips, and not for the rest.
    holds_first: bool,
    /// What the counter is below where the test holds.
    limit: AffineOperand,
}

/// `function` closed and the first loop not in `skip` with a counter test
/// split, with that loop's latch.
fn _candidate(
    context: &mut Context,
    layout: &DataLayout,
    function: &Function,
    analyses: &Analyses,
    skip: &BTreeSet<i64>,
) -> Result<Option<(Function, i64)>, String> {
    if !_tested(function) {
        return Ok(None);
    }
    let mut closed = function.clone();
    lcssa::closed(&mut closed)?;
    let facts = analyses.fresh().get::<Registers>(context, layout, &closed);
    let shape = cfg::Shape::of(&closed);
    let predecessors = loops::predecessors(&cfg::graph(&closed));
    let owners = closed
        .walk()
        .filter_map(|(block, inst)| closed.instruction(inst).result.map(|value| (value, cfg::id(block))))
        .collect::<BTreeMap<_, _>>();
    for loop_ in shape.loops.clone() {
        let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else {
            continue;
        };
        let outside =
            predecessors[&loop_.header].iter().copied().filter(|at| !loop_.body.contains(at)).collect::<Vec<_>>();
        if skip.contains(&latch)
            || outside.len() != 1
            || loop_.body.iter().map(|&at| operations(&closed, cfg::block(at)).len()).sum::<usize>() > MAX_LOOP_OPS
        {
            continue;
        }
        let entry = outside[0];
        let unit =
            memory::Unit::within(context, layout, &closed, analyses.outer()).with_registers(&facts).with_shape(&shape);
        let leaves =
            |block: i64| closed.successors(cfg::block(block)).iter().any(|&to| !loop_.body.contains(&cfg::id(to)));
        if loop_.body.iter().any(|&block| block != loop_.header && leaves(block)) {
            continue;
        }
        for proof in induction::counted(&unit, &loop_, Some(&facts), false) {
            if !_splittable(&closed, &loop_, &proof) {
                continue;
            }
            let invariant = |operand: Operand| match operand {
                Operand::Value(value) => owners.get(&value).is_none_or(|&owner| {
                    !loop_.body.contains(&owner) && shape.dominance.dominates(function, owner, entry)
                }),
                _ => true,
            };
            let tests = loop_
                .body
                .iter()
                .filter(|&&block| block != loop_.header)
                .filter_map(|&block| _test(&unit, &closed, &proof, block, &invariant));
            for test in tests {
                let mut copied = closed.clone();
                let Some(labels) = loopclone::peeled(&mut copied, &loop_, 1)? else {
                    continue;
                };
                _split(context, &closed, &mut copied, &loop_, &proof, entry, &test, &labels[0])?;
                cfg::_unreachable(context, &mut copied);
                transform::_trivial_phis(&mut copied)?;
                return Ok(Some((copied, latch)));
            }
        }
    }
    Ok(None)
}

/// Some loop has a block, not its header, that branches on a compare: the
/// cheap look before the function is copied and closed.
fn _tested(function: &Function) -> bool {
    let shape = cfg::Shape::of(function);
    shape
        .loops
        .iter()
        .any(
            |loop_| loop_.body.iter().filter(|&&block| block != loop_.header).any(|&block| {
                let at = cfg::block(block);
                function.terminator(at).is_some_and(|branch| transform::_comparison(function, at, branch).is_some())
            }),
        )
}

/// `proof` counts up by one, tested by `<` before each trip, in the header,
/// and nothing else leaves.
fn _splittable(
    function: &Function,
    loop_: &Loop,
    proof: &CountedLoop,
) -> bool {
    matches!(proof.test, IntPredicate::Slt | IntPredicate::Ult)
        && matches!(proof.reach, Reach::Distance)
        && proof.step == BigInt::from(1)
        && !proof.posttested
        && !proof.stepped
        && !proof.shifted
        && !proof.leaves
        && function.parent(proof.branch).map(cfg::id) == Some(loop_.header)
}

/// `block`'s branch as a test of `proof`'s counter that flips once.
fn _test(
    unit: &memory::Unit,
    function: &Function,
    proof: &CountedLoop,
    block: i64,
    invariant: &dyn Fn(Operand) -> bool,
) -> Option<Test> {
    let at = cfg::block(block);
    let branch = function.terminator(at)?;
    let compare = transform::_comparison(function, at, branch)?;
    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else {
        return None;
    };
    let counter = Operand::Value(proof.counter.value);
    let [left, right] = function.instruction(compare).operands[..] else {
        return None;
    };
    let (predicate, other) = match (left == counter, right == counter) {
        (true, false) => (predicate, right),
        (false, true) => (predicate.swapped(), left),
        _ => return None,
    };
    let width = proof.width();
    if !invariant(other) || unit.int_bits(other) != Some(width) {
        return None;
    }
    let signed = matches!(
        predicate,
        IntPredicate::Slt | IntPredicate::Sge | IntPredicate::Sle | IntPredicate::Sgt
    );
    let (holds_first, inclusive) = match predicate {
        IntPredicate::Slt | IntPredicate::Ult => (true, false),
        IntPredicate::Sge | IntPredicate::Uge => (false, false),
        IntPredicate::Sle | IntPredicate::Ule => (true, true),
        IntPredicate::Sgt | IntPredicate::Ugt => (false, true),
        _ => return None,
    };
    if signed != (proof.test == IntPredicate::Slt) {
        return None;
    }
    let limit = match inclusive {
        false => induction::term(unit, other)?,
        true => {
            let largest = (BigInt::from(1) << if signed { width - 1 } else { width }) - 1;
            let bound = BigInt::from(unit.int_constant(other)?);
            (bound != largest).then(|| AffineOperand::constant(bound + 1, width))?
        }
    };
    Some(Test { block, holds_first, limit })
}

/// `copied`, `body` with one iteration of `loop_` peeled into `labels`, made
/// two loops: the clone runs the trips below `test.limit` and hands its
/// counters to the original, which runs the rest.
#[allow(clippy::too_many_arguments)]
fn _split(
    context: &mut Context,
    body: &Function,
    copied: &mut Function,
    loop_: &Loop,
    proof: &CountedLoop,
    entry: i64,
    test: &Test,
    labels: &BTreeMap<i64, BlockId>,
) -> Result<(), String> {
    let latch = *loop_.latches.first().expect("one latch");
    let header = cfg::block(loop_.header);
    let (cloned_header, cloned_latch) = (labels[&loop_.header], labels[&latch]);
    let exit = cfg::block(proof.exit);
    let parent = cfg::block(entry);

    // The clone stops at the smaller of the loop's bound and the limit.
    let last = copied.terminator(parent).ok_or("a terminated block")?;
    let stop = _smaller(context, copied, last, proof, &test.limit);
    let position = body.block(header).iter().position(|one| one == proof.compare).ok_or("KeyError")?;
    let cloned_compare = copied.block(cloned_header).iter().nth(position).ok_or("IndexError")?;
    let counter = Operand::Value(proof.counter.value);
    let at = body.instruction(proof.compare).operands.iter().position(|&one| one != counter).ok_or("IndexError")?;
    let mut operands = copied.instruction(cloned_compare).operands.clone();
    operands[at] = stop;
    copied.set_operands(cloned_compare, operands);

    // The original loop is entered from the clone's header, with the clone's
    // counters; the clone goes round on what its own latch computes.
    for (phi, cloned) in edges::phis(copied, header).into_iter().zip(edges::phis(copied, cloned_header)) {
        let mut original = arms(copied, phi);
        let at = original.iter().position(|&(_, source)| source == cloned_latch).ok_or("KeyError")?;
        let residual = original[at];
        original[at] = (Operand::Value(copied.instruction(cloned).result.expect("a phi's value")), cloned_header);
        copied.set_operands(phi, from_arms(&original));
        let mut incoming = arms(copied, cloned);
        incoming.push(residual);
        copied.set_operands(cloned, from_arms(&incoming));
    }
    let jump = copied.terminator(cloned_latch).ok_or("a terminated block")?;
    edges::retarget(copied, jump, header, cloned_header);
    let leave = copied.terminator(cloned_header).ok_or("a terminated block")?;
    edges::retarget(copied, leave, exit, header);
    for phi in edges::phis(copied, exit) {
        let kept = arms(copied, phi).into_iter().filter(|&(_, source)| source != cloned_header).collect::<Vec<_>>();
        copied.set_operands(phi, from_arms(&kept));
    }

    for (at, taken) in [(cfg::block(test.block), !test.holds_first), (labels[&test.block], test.holds_first)] {
        let branch = copied.terminator(at).ok_or("IndexError")?;
        let destination = copied.instruction(branch).operands[if taken { 1 } else { 2 }];
        copied.set_operands(branch, vec![destination]);
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

/// `proof`'s bound or `limit`, whichever the counter reaches first, placed
/// before `at`.
fn _smaller(
    context: &mut Context,
    function: &mut Function,
    at: InstId,
    proof: &CountedLoop,
    limit: &AffineOperand,
) -> Operand {
    let mut seeds = Seeds { context, function, at, width: proof.width() };
    let (limit, bound) = (seeds.operand(limit), seeds.operand(&proof.bound));
    let flag_type = seeds.context.types.int(1);
    let value_type = seeds.context.types.int(proof.width());
    let below = seeds
        .function
        .create_instruction(Opcode::ICmp(proof.test), flag_type, vec![limit, bound], Flags::default(), None);
    seeds.function.insert(below, Position::Before(at)).expect("`at` is placed");
    let flag = Operand::Value(seeds.function.instruction(below).result.expect("a compare's value"));
    let pick =
        seeds.function.create_instruction(Opcode::Select, value_type, vec![flag, limit, bound], Flags::default(), None);
    seeds.function.insert(pick, Position::Before(at)).expect("`at` is placed");
    Operand::Value(seeds.function.instruction(pick).result.expect("a select's value"))
}

#[cfg(test)]
#[path = "loopsplit_tests.rs"]
mod tests;
