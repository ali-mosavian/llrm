//! Loop fusion: two adjacent loops of the same trips become one, so the data
//! is walked once and the loop's own work is paid once.
//!
//! # How it works
//! A candidate is a pair of loops with nothing between them but code that can
//! run before the first: the first's one exit leads, through blocks of one way
//! in and one way out, to the second's preheader (`Direct`); or both are
//! entered behind the same test, the first's skipping to the second's guard
//! (`Guarded`), and the first's exit leads to that guard. Each loop has one
//! latch and one exit, taken from its header (tested before each trip) or its
//! latch (tested after), both the same; and `induction::same_trips` proves one
//! count for both (LLVM's `haveIdenticalTripCounts`).
//!
//! The pair is fused when
//! - nothing the first loop computes is read outside it: the second loop and
//!   the code between do not read it, and the loop's values do not leave;
//! - the code between the loops (and the second's guard) has no effect and
//!   cannot trap, and its operands are available before the first loop;
//! - no body has work that may trap, and where the loops are tested before each
//!   trip, no header has work but what can run once more or not at all (a
//!   header runs one time more than its body);
//! - the loops have one parent loop, and a `Guarded` pair is tested after each
//!   trip and reached by no other way than the two guards;
//! - for a `Guarded` pair, the first's test proves the second's one way and its
//!   negation the other (`guards::on_edge`, `decides`);
//! - the accesses of both loops are affine in their counters
//!   ([`llrm_analysis::access`]), on one root or on objects that cannot be the
//!   same, and `dependence::fusable` finds no pair the second loop would reach
//!   before the first (LLVM's `dependencesAllowFusion`).
//!
//! The second loop's phis move to the first's header and both bodies run in
//! one trip:
//!
//! ```text
//!   tested before each trip              tested after each trip
//!   H0: test; B0 .. L0 -> H0             H0 .. L0: test -> H0 | X0
//!   H1: test; B1 .. L1 -> H1             H1 .. L1: test -> H1 | X1
//!   becomes                              becomes
//!   H0: test; B0 .. L0; B1 .. L1 -> H0   H0 .. L0 -> H1 .. L1: test -> H0 | X1
//! ```
//!
//! The setup between them moves ahead of the first loop. The copy is a
//! candidate, as in [`crate::unswitch`]: [`crate::candidate`] runs the
//! pipeline over it and keeps it when it has fewer loops, no more operations
//! and no larger price, spills included. At most `MAX_TRIALS` candidates are
//! tried per body and the loops are at most `MAX_LOOP_OPS` operations
//! together, so the work stays linear in the body.
//!
//! `LLRM_DEBUG=fuse` prints each pair fused, and why each other adjacent pair
//! was not.
//!
//! # Example
//! Before:
//! ```mir
//! b1:
//!   %i = phi i16 [ 0, %b0 ], [ %i.next, %b2 ]
//!   %more = icmp slt i16 %i, 8
//!   br i1 %more, label %b2, label %b3
//!
//! b2:
//!   ; a[i] = i
//!   %i.next = add i16 %i, 1
//!   br label %b1
//!
//! b3:
//!   br label %b4
//!
//! b4:
//!   %j = phi i16 [ 0, %b3 ], [ %j.next, %b5 ]
//!   %again = icmp slt i16 %j, 8
//!   br i1 %again, label %b5, label %b6
//!
//! b5:
//!   ; b[j] = a[j] + 1
//!   %j.next = add i16 %j, 1
//!   br label %b4
//! ```
//! After:
//! ```mir
//! b1:
//!   %j = phi i16 [ 0, %b0 ], [ %j.next, %b5 ]
//!   %i = phi i16 [ 0, %b0 ], [ %i.next, %b5 ]
//!   %more = icmp slt i16 %i, 8
//!   br i1 %more, label %b2, label %b3
//!
//! b2:
//!   ; a[i] = i
//!   %i.next = add i16 %i, 1
//!   br label %b5
//!
//! b5:
//!   ; b[j] = a[j] + 1
//!   %j.next = add i16 %j, 1
//!   br label %b1
//! ```
//! `a[j]` is read in the trip after `a[i]` is written, so one loop does the
//! work of two.
//!
//! From unit test `two_loops_over_one_range_become_one` in `loopfuse_tests.rs`.
//!
//! # Runs
//! After `unswitch` and `split`, before unroll-jam, at -Omax only
//! (`-floop-fusion`): no default pipeline has it, as LLVM's `LoopFuse` is not
//! in `-O3` and gcc has no such pass outside Graphite. Not run where one pass
//! is selected by name. The fused loop goes through the scalar rounds again.
//!
//! # Needs
//! [`crate::candidate`], [`crate::profit`], `induction` (`same_trips`),
//! [`llrm_analysis::access`] and [`llrm_analysis::dependence`] (`fusable`),
//! `guards`.
//!
//! # Keeps
//! Nothing when it fuses: the function is replaced by the candidate.
//!
//! # See also
//! LLVM `LoopFuse.cpp` (`collectFusionCandidates`, `dependencesAllowFusion`,
//! `performFusion`); gcc fuses only in Graphite (`-floop-nest-optimize`).
//! [`crate::unroll_jam`] fuses the copies of one outer loop's inner loop.

use std::collections::BTreeSet;

use llrm_analysis::access::{self, Apart, Reference, Roots};
use llrm_analysis::cfg::Canonical;
use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, CountedLoop};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, dependence, guards, memory};
use llrm_mir::context::Context;
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};
use num_traits::ToPrimitive;

use crate::candidate::{self, Baseline, Gain, Pricing};
use crate::lcssa::{arms, from_arms, operations};
use crate::loopshape;
use crate::profit::{self, OperationCosts};
use crate::{edges, transform};

/// Operations of the two loops together: the fused loop is of that size.
const MAX_LOOP_OPS: usize = 128;
/// Candidates run through the pipeline per body.
const MAX_TRIALS: usize = 4;
/// Blocks between two loops, or from a guard to its loop.
const MAX_BETWEEN: usize = 4;
/// Accesses of one loop that `dependence` compares with the other's.
const MAX_ACCESSES: usize = 32;

pub struct Fuse {
    /// What a fused candidate goes through before it is judged.
    pub passes: Vec<Box<dyn FunctionPass>>,
}

impl FunctionPass for Fuse {
    fn name(&self) -> &'static str {
        "fuse"
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let costs = profit::costs(&outer);
        let passes = &mut self.passes;
        let module = std::rc::Rc::clone(&outer);
        let mut reoptimize = |candidate: &mut Unit| {
            for pass in passes.iter_mut() {
                pass.run(candidate, &mut Analyses::new(std::rc::Rc::clone(&module)));
            }
        };
        match optimized(unit, analyses, &outer, &costs, &mut reoptimize) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("fuse: {error}"),
        }
    }
}

/// Every pair of loops this can fuse, up to the trial budget; whether any was.
pub fn optimized(
    unit: &mut Unit,
    analyses: &mut Analyses,
    outer: &Outer,
    costs: &OperationCosts,
    reoptimize: &mut dyn FnMut(&mut Unit),
) -> Result<bool, String> {
    let mut refused = BTreeSet::new();
    let mut changed = false;
    for _ in 0..MAX_TRIALS {
        let Some(fusion) = found(unit, analyses, &refused) else { break };
        let key = fusion.key();
        let mut copy = unit.function.clone();
        if let Err(error) = fused(unit.context, &mut copy, &fusion) {
            llrm_support::debug!("fuse", "b{} then b{}: not fused, {error}", key.0, key.1);
            refused.insert(key);
            continue;
        }
        if candidate::adopted(
            unit,
            outer,
            costs,
            copy,
            reoptimize,
            Pricing::WithSpills,
            Gain::FewerLoops,
            Baseline::Kept,
        ) {
            llrm_support::debug!("fuse", "fused the loops with headers b{}, b{}", key.0, key.1);
            changed = true;
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
        } else {
            llrm_support::debug!("fuse", "b{} then b{}: not fused, the candidate is not better", key.0, key.1);
            refused.insert(key);
        }
    }
    Ok(changed)
}

/// How the first loop's exit reaches the second.
#[derive(Clone, Debug)]
enum Link {
    /// Through `between`, blocks of one way in and one way out from the exit
    /// to the second's preheader.
    Direct { between: Vec<BlockId> },
    /// The first loop is entered by `first_guard` through `entered`, whose
    /// other arm is `second_guard`, which the first's exit reaches through
    /// empty blocks. The guard enters the second loop through `entering`, to
    /// its preheader, or skips to `skip`.
    Guarded { first_guard: BlockId, entered: BlockId, second_guard: BlockId, entering: Vec<BlockId>, skip: BlockId },
}

/// Two loops that can be fused, and what was found to prove it.
struct Fusion {
    first: Canonical,
    second: Canonical,
    link: Link,
    /// Tested before each trip (in the header) or after (in the latch).
    pretested: bool,
    /// The second's setup, moved ahead of the first loop.
    setup: Vec<InstId>,
}

impl Fusion {
    fn key(&self) -> (i64, i64) {
        (self.first.loop_.header, self.second.loop_.header)
    }
}

/// The first pair of loops not in `refused` that can be fused, if any.
fn found(
    unit: &mut Unit,
    analyses: &mut Analyses,
    refused: &BTreeSet<(i64, i64)>,
) -> Option<Fusion> {
    let function: &Function = unit.function;
    let shape = cfg::Shape::of(function);
    if shape.loops.len() < 2 {
        return None;
    }
    let near = Adjacent { function, loops: &shape.loops };
    let pairs = shape
        .loops
        .iter()
        .filter_map(|loop_| Canonical::of(function, loop_))
        .filter_map(|first| {
            let (second, link) = near.following(&first)?;
            let kept = !refused.contains(&(first.loop_.header, second.loop_.header))
                && near.parent(&first.loop_) == near.parent(&second.loop_);
            kept.then_some((first, second, link))
        })
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        return None;
    }
    let facts = analyses.get::<Registers>(unit.context, unit.layout, function);
    let outer = std::rc::Rc::clone(analyses.outer());
    let within =
        memory::Unit::within(unit.context, unit.layout, function, &outer).with_registers(&facts).with_shape(&shape);
    let view = View { function, context: unit.context, outer: &outer, shape: &shape, within: &within };
    pairs.into_iter().find_map(|(first, second, link)| view.legal(first, second, link))
}

/// Where one loop follows another, from the blocks alone.
struct Adjacent<'a> {
    function: &'a Function,
    loops: &'a [Loop],
}

impl Adjacent<'_> {
    /// The header of the smallest loop around `loop_`, if one is.
    fn parent(
        &self,
        loop_: &Loop,
    ) -> Option<i64> {
        self.loops
            .iter()
            .filter(|one| one.header != loop_.header && one.body.is_superset(&loop_.body))
            .min_by_key(|one| one.body.len())
            .map(|one| one.header)
    }

    /// The loop of the body whose preheader is `block`.
    fn loop_after(
        &self,
        block: BlockId,
    ) -> Option<Canonical> {
        let [header] = self.function.successors(block)[..] else { return None };
        let found = self.loops.iter().find(|one| one.header == cfg::id(header))?;
        Canonical::of(self.function, found).filter(|one| one.preheader == block)
    }

    /// The loop after `first`, and how its exit gets there.
    fn following(
        &self,
        first: &Canonical,
    ) -> Option<(Canonical, Link)> {
        let function = self.function;
        let mut between = vec![first.exit];
        for _ in 0..MAX_BETWEEN {
            let at = *between.last()?;
            if let Some(second) = self.loop_after(at) {
                let link = Link::Direct { between: chain(function, first.exit, at)? };
                return (second.header != first.header).then_some((second, link));
            }
            if function.successors(at).len() == 2 {
                between.pop();
                let between = chain(function, first.exit, *between.last()?)?;
                return self.guarded(first, between, at);
            }
            let [next] = function.successors(at)[..] else { return None };
            between.push(next);
        }
        None
    }

    /// `guard` is the second loop's: `first` is entered behind a test whose
    /// other arm is `guard`, and the first's exit reaches `guard` through
    /// `between`.
    fn guarded(
        &self,
        first: &Canonical,
        between: Vec<BlockId>,
        guard: BlockId,
    ) -> Option<(Canonical, Link)> {
        let function = self.function;
        // The test the first loop is entered behind, through blocks of one way
        // in and one way out.
        let (first_guard, entered) = {
            let mut back = first.preheader;
            let mut found = None;
            for _ in 0..MAX_BETWEEN {
                let [from] = function.predecessors(back)[..] else { break };
                if function.successors(from).len() == 2 {
                    found = Some((from, back));
                    break;
                }
                back = from;
            }
            found?
        };
        let mut guards: Vec<BlockId> = function.predecessors(guard).into();
        guards.sort();
        let mut expected = vec![*between.last()?, first_guard];
        expected.sort();
        if guards != expected
            || !function.successors(first_guard).contains(&guard)
            || between.iter().any(|&at| function.block(at).instructions().len() != 1)
        {
            return None;
        }
        let [yes, no] = function.successors(guard)[..] else { return None };
        for (arm, skip) in [(yes, no), (no, yes)] {
            let mut blocks = vec![arm];
            for _ in 0..MAX_BETWEEN {
                let at = *blocks.last()?;
                if let Some(second) = self.loop_after(at) {
                    let entering = chain(function, arm, at)?;
                    let link = Link::Guarded { first_guard, entered, second_guard: guard, entering, skip };
                    return Some((second, link));
                }
                let [next] = function.successors(at)[..] else { break };
                blocks.push(next);
            }
        }
        None
    }
}

/// What a pair is asked of.
struct View<'a> {
    function: &'a Function,
    context: &'a Context,
    outer: &'a Outer,
    shape: &'a cfg::Shape,
    within: &'a memory::Unit<'a>,
}

/// The blocks from `from` to `to`, each but `to` with one way in and one way
/// on, and `to` entered by the last alone.
fn chain(
    function: &Function,
    from: BlockId,
    to: BlockId,
) -> Option<Vec<BlockId>> {
    let mut blocks = vec![from];
    while *blocks.last()? != to {
        let at = *blocks.last()?;
        let [next] = function.successors(at)[..] else { return None };
        if function.predecessors(at).len() != 1 || blocks.len() >= MAX_BETWEEN {
            return None;
        }
        blocks.push(next);
    }
    (function.predecessors(to).len() == 1).then_some(blocks)
}

impl<'a> View<'a> {
    /// Whether `first` and `second` can be fused: the `Fusion` that says how.
    fn legal(
        &self,
        first: Canonical,
        second: Canonical,
        link: Link,
    ) -> Option<Fusion> {
        let function = self.function;
        let ops = |one: &Canonical| {
            one.loop_.body.iter().map(|&at| operations(function, cfg::block(at)).len()).sum::<usize>()
        };
        let pair = (first.loop_.header, second.loop_.header);
        let refused = |reason: &str| -> Option<Fusion> {
            llrm_support::debug!("fuse", "b{} then b{}: not fused, {reason}", pair.0, pair.1);
            None
        };
        if ops(&first) + ops(&second) > MAX_LOOP_OPS {
            return refused("the loops are too big");
        }
        // One count for both, tested at the same place.
        let controls = |proof: &CountedLoop, one: &Canonical| loopshape::controls(function, proof, one);
        let proofs = |one: &Canonical| induction::counted(self.within, &one.loop_, None, false);
        let second_proofs = proofs(&second);
        let Some((proof, _)) = proofs(&first).into_iter().find_map(|one| {
            let other = second_proofs.iter().find(|other| induction::same_trips(&one, other))?;
            (controls(&one, &first) && controls(other, &second)).then(|| (one.clone(), other.clone()))
        }) else {
            return refused("no one trip count");
        };
        let pretested = !proof.posttested;
        // A header tested before each trip runs once more than the body, and
        // the second guard's branch is kept there: fused, neither is as it was.
        if pretested && matches!(link, Link::Guarded { .. }) {
            return refused("a guarded pair tested before each trip");
        }
        if !self.harmless(&first, &second, pretested) {
            return refused("work in a loop that may trap or touch memory out of order");
        }
        let Some(()) = self.closed_off(&first, &second, pretested) else {
            return refused("a value of the first loop is read outside it");
        };
        let Some(setup) = self.setup(&first, &second, &link) else {
            return refused("the code between the loops cannot move");
        };
        if let Link::Guarded { .. } = link {
            let Some(()) = self.proves_guard(&link) else {
                return refused("the first test does not prove the second");
            };
        }
        let trips = proof.count.as_ref().and_then(|count| count.to_i64());
        let Some(()) = self.memory_allows(&first, &second, trips) else {
            return refused("the memory accesses may depend");
        };
        Some(Fusion { first, second, link, pretested, setup })
    }

    /// Neither body has work that may trap, so the loops' order of faults is
    /// not asked of; and where the loops are tested before each trip, neither
    /// header has anything but work that can run an extra time or not at all.
    fn harmless(
        &self,
        first: &Canonical,
        second: &Canonical,
        pretested: bool,
    ) -> bool {
        let (function, context, outer) = (self.function, self.context, self.outer);
        [first, second]
            .into_iter()
            .all(
                |one| loopshape::quiet(context, outer, function, one)
                    && (!pretested || loopshape::header_quiet(context, outer, function, one)),
            )
    }

    /// Nothing the first loop computes is read outside it, and what the second
    /// computes in a header that is no longer run on the way out is not read
    /// there; the second's body is entered from its header alone.
    fn closed_off(
        &self,
        first: &Canonical,
        second: &Canonical,
        pretested: bool,
    ) -> Option<()> {
        let function = self.function;
        let read_outside = |one: &Canonical, inst: InstId| loopshape::read_outside(function, one, inst);
        let all = |one: &Canonical| {
            one.loop_
                .body
                .iter()
                .flat_map(|&at| function.block(cfg::block(at)).instructions().iter().copied())
                .collect::<Vec<_>>()
        };
        if all(first).into_iter().any(|inst| read_outside(first, inst)) {
            return None;
        }
        if pretested {
            let header_work = function
                .block(second.header)
                .instructions()
                .iter()
                .copied()
                .filter(|&inst| function.instruction(inst).opcode != Opcode::Phi);
            if header_work.into_iter().any(|inst| read_outside(second, inst)) {
                return None;
            }
            let [body] =
                function.successors(second.header).into_iter().filter(|to| second.holds(*to)).collect::<Vec<_>>()[..]
            else {
                return None;
            };
            if function.predecessors(body) != [second.header] || function.successors(first.latch) != [first.header] {
                return None;
            }
        }
        Some(())
    }

    /// The instructions between the loops, in order, each of which has no
    /// effect, cannot trap, takes only values made before the first loop and
    /// is read only where the first loop's preheader dominates.
    fn setup(
        &self,
        first: &Canonical,
        second: &Canonical,
        link: &Link,
    ) -> Option<Vec<InstId>> {
        let function = self.function;
        let blocks = match link {
            Link::Direct { between } => between.clone(),
            Link::Guarded { second_guard, entering, .. } => {
                std::iter::once(*second_guard).chain(entering.iter().copied()).collect()
            }
        };
        let mut listed = Vec::new();
        for &block in &blocks {
            for &inst in function.block(block).instructions() {
                let op = function.instruction(inst);
                if op.opcode == Opcode::Phi {
                    return None;
                }
                if !op.opcode.is_terminator() {
                    if !llrm_mir::memory::speculatable(self.context, self.outer.callees(), function, inst) {
                        return None;
                    }
                    listed.push(inst);
                }
            }
        }
        let before = cfg::id(first.preheader);
        let mut made = IdSet::new();
        let available = |operand: Operand, made: &IdSet<InstId>| match operand {
            Operand::Value(value) => match function.value(value).def {
                ValueDef::Argument(_) => true,
                ValueDef::Instruction(def) => {
                    made.contains(&def)
                        || function
                            .parent(def)
                            .is_some_and(|block| self.shape.dominance.dominates(self.function, cfg::id(block), before))
                }
            },
            _ => true,
        };
        for &inst in &listed {
            if !function.instruction(inst).operands.iter().all(|&operand| available(operand, &made)) {
                return None;
            }
            made.insert(inst);
        }
        // The second loop's phis start from values available there too.
        for phi in phis_of(function, second.header) {
            let starts = arms(function, phi).into_iter().filter(|&(_, from)| from == second.preheader);
            if !starts.into_iter().all(|(value, _)| available(value, &made)) {
                return None;
            }
        }
        // Where it is read, the first loop's preheader dominates; the second
        // guard's own branch is not read after the fusion.
        let guard_branch = match link {
            Link::Guarded { second_guard, .. } => function.terminator(*second_guard),
            Link::Direct { .. } => None,
        };
        for &inst in &listed {
            let Some(result) = function.instruction(inst).result else { continue };
            for used in function.users(result).iter().filter(|used| Some(used.user) != guard_branch) {
                let from = match function.instruction(used.user).opcode {
                    Opcode::Phi => arms(function, used.user)
                        .into_iter()
                        .find(|&(value, _)| value == Operand::Value(result))
                        .map(|(_, from)| from)?,
                    _ => function.parent(used.user)?,
                };
                if !made.contains(&used.user) && !self.shape.dominance.dominates(self.function, before, cfg::id(from)) {
                    return None;
                }
            }
        }
        Some(listed)
    }

    /// The first loop's entry test proves the second's the same way, and its
    /// skipping proves the second's skipping.
    fn proves_guard(
        &self,
        link: &Link,
    ) -> Option<()> {
        let Link::Guarded { first_guard, entered, second_guard, entering, .. } = link else { return None };
        let function = self.function;
        let branch = function.terminator(*second_guard)?;
        let [Operand::Value(condition), Operand::Block(yes), _] = function.instruction(branch).operands[..] else {
            return None;
        };
        let ran = guards::on_edge(self.within, *first_guard, *entered);
        let skipped = guards::on_edge(self.within, *first_guard, *second_guard);
        let enters_on_true = yes == entering[0];
        (guards::decides(self.within, &ran, condition) == Some(enters_on_true)
            && guards::decides(self.within, &skipped, condition) == Some(!enters_on_true))
        .then_some(())
    }

    /// No access of the second loop reaches bytes at an iteration before the
    /// iteration of the first loop's access to them.
    fn memory_allows(
        &self,
        first: &Canonical,
        second: &Canonical,
        trips: Option<i64>,
    ) -> Option<()> {
        let mut roots = Roots::default();
        let mut list = |one: &Canonical| {
            access::references(self.within, &[&one.loop_], &one.loop_.body, &one.loop_.body, &mut roots)
                .filter(|found| found.len() <= MAX_ACCESSES)
        };
        let (earlier, later) = (list(first)?, list(second)?);
        let apart = Apart::new(self.within);
        for x in &earlier {
            for y in &later {
                if (x.access.write || y.access.write) && apart.unknown(x, y) {
                    return None;
                }
            }
        }
        let accesses = |found: Vec<Reference>| found.into_iter().map(|one| one.access).collect::<Vec<_>>();
        dependence::fusable(&accesses(earlier), &accesses(later), trips).then_some(())
    }
}

fn phis_of(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .collect()
}

/// The non-phi, non-terminator instructions of `block`.
fn work(
    function: &Function,
    block: BlockId,
) -> Vec<InstId> {
    function
        .block(block)
        .instructions()
        .iter()
        .copied()
        .filter(|&inst| {
            let opcode = &function.instruction(inst).opcode;
            *opcode != Opcode::Phi && !opcode.is_terminator()
        })
        .collect()
}

/// `fusion` made in `function`: the second loop's phis and body run in the
/// first loop's trips.
fn fused(
    context: &mut Context,
    function: &mut Function,
    fusion: &Fusion,
) -> Result<(), String> {
    let (first, second) = (&fusion.first, &fusion.second);
    let before = function.terminator(first.preheader).ok_or("a terminated preheader")?;
    for &inst in &fusion.setup {
        function.move_to(inst, Position::Before(before))?;
    }
    // The first loop's phis come back round from the second's latch, and the
    // second's are the fused loop's too.
    let top = *function.block(first.header).instructions().first().ok_or("a header")?;
    for phi in phis_of(function, first.header) {
        let moved = arms(function, phi)
            .into_iter()
            .map(|(value, from)| (value, if from == first.latch { second.latch } else { from }))
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&moved));
    }
    for phi in phis_of(function, second.header) {
        let moved = arms(function, phi)
            .into_iter()
            .map(|(value, from)| (value, if from == second.preheader { first.preheader } else { from }))
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&moved));
        function.move_to(phi, Position::Before(top))?;
    }
    let back = function.terminator(second.latch).ok_or("a terminated latch")?;
    edges::retarget(function, back, second.header, first.header);
    let latch = function.terminator(first.latch).ok_or("a terminated latch")?;
    if fusion.pretested {
        let body = function
            .successors(second.header)
            .into_iter()
            .find(|&to| second.holds(to))
            .ok_or("the second loop's body")?;
        // The second's header work runs ahead of its body; its test is the
        // first's.
        let ahead = function
            .block(body)
            .instructions()
            .iter()
            .copied()
            .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
            .ok_or("a terminated body")?;
        for inst in work(function, second.header) {
            function.move_to(inst, Position::Before(ahead))?;
        }
        for phi in phis_of(function, body) {
            let moved = arms(function, phi)
                .into_iter()
                .map(|(value, from)| (value, if from == second.header { first.latch } else { from }))
                .collect::<Vec<_>>();
            function.set_operands(phi, from_arms(&moved));
        }
        edges::retarget(function, latch, first.header, body);
        // The second loop is never entered: its preheader goes to its exit.
        let enter = function.terminator(second.preheader).ok_or("a terminated preheader")?;
        edges::retarget(function, enter, second.header, second.exit);
        for phi in phis_of(function, second.exit) {
            let moved = arms(function, phi)
                .into_iter()
                .map(|(value, from)| (value, if from == second.header { second.preheader } else { from }))
                .collect::<Vec<_>>();
            function.set_operands(phi, from_arms(&moved));
        }
        let test = function.terminator(second.header).ok_or("a terminated header")?;
        function.erase(test)?;
        function.erase_block(second.header)?;
    } else {
        // The first latch's test is the second's: it runs on into it.
        function.set_operands(latch, vec![Operand::Block(second.header)]);
        if let Link::Guarded { second_guard, skip, .. } = &fusion.link {
            let guard = function.terminator(*second_guard).ok_or("a terminated guard")?;
            function.set_operands(guard, vec![Operand::Block(*skip)]);
        }
    }
    llrm_analysis::cfg::_unreachable(context, function);
    transform::_trivial_phis(function)?;
    Ok(())
}

#[cfg(test)]
#[path = "loopfuse_tests.rs"]
mod tests;
