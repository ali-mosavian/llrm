//! Static branch probabilities and block frequencies, as LLVM's
//! `BranchProbabilityInfo` and `BlockFrequencyInfo` estimate them when no
//! profile says otherwise.
//!
//! # How it works
//! `estimated` gives every edge a probability and every block a frequency. A
//! block with one successor takes 1. A block with several takes the first of
//! heuristics 1 to 5 that tells its successors apart; where none does, a
//! two-way branch combines the votes of 6 to 8 as GCC does (below), and a
//! wider one takes the first of them (`Heuristic` records which one fired):
//!
//! 1. `Declared`: `!prof` `branch_weights` on the terminator, one per named
//!    target, summed per distinct successor. Weights that do not name every
//!    target, or sum to zero, say nothing.
//! 2. `Invoke`: an invoke's unwind edge is taken 1 in 2^20.
//! 3. `Unreachable`: a successor every path of which ends in `unreachable`, a
//!    `noreturn` call or a `cold` call (`noreturn::cold`) is all but never
//!    taken.
//! 4. `Loop`: in a loop, staying is 124 to 4 for leaving (`LOOP`, LLVM's
//!    weights).
//! 5. `Counted`: a branch on a compare of the counters of an enclosing loop
//!    whose trip count induction proved (`trips`, at most 4096). The counters
//!    (header phis from a constant start) are stepped trip by trip, once for
//!    the loop, and a value the compares read is worked out once at each trip;
//!    the edge is taken on the share of trips the compare holds on, clamped
//!    away from 0 and 1.
//! 6. `Pointer`, `Zero`, `Float`, from the compare feeding a `br`: `p == q` on
//!    pointers fails (20:12), as do `x == 0`, `x == -1`, `x < 0` and `x <= 0`
//!    on integers, and `x == y` on floats; `isnan` (`uno`) is 1 in 2^20. Truth
//!    values (0 or all ones, 0 or 1, a compare, or a call whose `range` says
//!    so), one-bit tests (`x & 2^n`) and the sign of a three-way compare are no
//!    quantity and say nothing.
//! 7. `Call` (GCC, from Ball and Larus; LLVM has no such heuristic): a
//!    successor that calls, where the other does not, is not taken (67:33).
//! 8. `Return` (GCC): a successor that returns, where the other does not, is
//!    not taken (66:34); where it only returns a constant, 98% not taken if
//!    negative, 71% if null, 65% if another constant (`PRED_NEGATIVE_RETURN`,
//!    `PRED_NULL_RETURN`, `PRED_CONST_RETURN`).
//! 9. `Even`: equal weights.
//!
//! The votes of 6 to 8 are not first-match in GCC (`PRED_FLAG_FIRST_MATCH` is
//! set on 1 to 5): `combine_predictions_for_bb` (predict.cc L1213, GCC 13.4)
//! folds the probability each gives the first successor from 1/2 as
//! Dempster-Shafer, `d = c*p + (1-c)*(1-p); c = c*p/d` (`combined`), so a zero
//! test (62.5%) and a call on the same arm (33%) leave that arm at 45%. A call
//! to the function itself is no vote where the other way returns at once (the
//! base case of a recursion: the call is the work).
//!
//! Frequencies are relative to the entry's 1 (`propagated`). Blocks go in
//! reverse postorder and a block's frequency is the sum over its non-back edges
//! of predecessor frequency times edge probability, times its loop's scale if
//! it is a header. A loop's scale is 1 / (1 - p), `p` the probability of coming
//! back round, capped at 4096 (`LOOP_SCALE`, LLVM's cap). Inner loops are
//! scaled first. With proven trips the scale is the trips (or trips + 1 when
//! the only exit is a test at a non-latch header), and the exit test takes `1 /
//! (trips + 1 - tested)` of the mass, so the trips stay.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   entry["<b>entry</b><br/><small>frequency 1</small>"]:::mid
//!   h["<b>H</b><br/><small>scale 1/(1-0.969) = 32</small>"]:::mid
//!   body["<b>body</b>"]:::mid
//!   exit["<b>exit</b><br/><small>frequency 1<br/>(entering once)</small>"]:::mid
//!   entry --> h
//!   h --> body
//!   body -->|"stay 124/128"| h
//!   h --> exit
//! ```
//!
//! *Frequencies around a loop with a header scale of 32*
//!
//! The propagation takes edge probabilities as a function, so the backend uses
//! it too (`propagated_edges`, `Cycle`) and MIR and LIR estimates agree.
//!
//! # Example
//! A one-block loop with no proven trip count:
//!
//! ```mir
//! define i16 @f(i16 %n) {
//! entry:
//!   br label %head
//! head:
//!   %i = phi i16 [ 0, %entry ], [ %j, %head ]
//!   %j = add i16 %i, 1
//!   %c = icmp slt i16 %j, %n
//!   br i1 %c, label %head, label %out
//! out:
//!   ret i16 %j
//! }
//! ```
//!
//! | Fact | Value |
//! | --- | --- |
//! | heuristic at `head` | `Loop` |
//! | probability `head -> head` | 124/128 |
//! | frequency of `head` | 32 |
//! | frequency of `out` | 1 |
//!
//! From unit test `test_a_loop_stays_in_31_of_32_and_its_header_runs_32_times`
//! in `branchprob_tests.rs`.
//!
//! # Provides
//! `Odds { taken, frequency, by }` from `estimated`, `propagated` and
//! `propagated_edges`, and the constants `LOOP` and `LIKELY` (0.8, the edge
//! probability above which machine block placement trades the shorter layout
//! for it). Consumers: `profit` and `rotate` in `llrm-transforms`; the
//! backend's `isel`, `jumps` and `analysis/frequency` (grep `branchprob::`).
//!
//! # Needs
//! [`crate::cfg::Shape`] for loops, [`crate::noreturn`] (`terminal_sites`,
//! `cold`), [`crate::effects::Declarations`] and the trip counts the caller
//! takes from [`crate::induction`].
//!
//! # See also
//! LLVM `BranchProbabilityInfo.cpp`, `BlockFrequencyInfo`; GCC `predict.def`.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_mir::facts::Facts;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, MetadataNode, MetadataOperand, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::types::Type;
use llrm_mir::{ConstantKind, Context};
use llrm_support::hash::HashMap;

use crate::cfg::{self, Shape, id};
use crate::effects::Declarations;
use crate::noreturn;

/// Why a block's successors have the probabilities they do.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Heuristic {
    /// The program says: `!prof` branch weights.
    Declared,
    /// An invoke's unwind edge is all but never taken.
    Invoke,
    Unreachable,
    Loop,
    /// A compare of an enclosing counted loop's counters: the share of its
    /// trips it holds on.
    Counted,
    Pointer,
    Zero,
    Float,
    Call,
    Return,
    Even,
}

/// Every edge's probability and every block's frequency, by block id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Odds {
    pub taken: BTreeMap<(i64, i64), f64>,
    pub frequency: BTreeMap<i64, f64>,
    /// Which heuristic set each branching block's probabilities.
    pub by: BTreeMap<i64, Heuristic>,
}

impl Odds {
    /// The probability of `from`'s edge to `to`: certain for a lone successor.
    pub fn probability(
        &self,
        from: i64,
        to: i64,
    ) -> Option<f64> {
        self.taken.get(&(from, to)).copied()
    }
}

// LLVM's weights (BranchProbabilityInfo.cpp), and GCC's hit rates.
const UNREACHABLE: (f64, f64) = (1.0, ((1 << 20) - 1) as f64);
/// Staying in a loop to leaving it: LLVM's weights, and the guard a copied
/// header leaves before the loop.
pub const LOOP: (f64, f64) = (124.0, 4.0);
const OPCODE: (f64, f64) = (20.0, 12.0);
const ORDERED: (f64, f64) = ((1024 * 1024 - 1) as f64, 1.0);
const CALL: (f64, f64) = (67.0, 33.0);
const RETURN: (f64, f64) = (66.0, 34.0);
// GCC's predict.def: a path that returns a constant, rather than computing a
// result, is the exception.
const NEGATIVE_RETURN: (f64, f64) = (2.0, 98.0);
const NULL_RETURN: (f64, f64) = (29.0, 71.0);
const CONST_RETURN: (f64, f64) = (35.0, 65.0);
/// MachineBlockPlacement's `StaticLikelyProb`: how likely an edge must be
/// before placement trades the shorter layout for it.
pub const LIKELY: f64 = 0.8;
/// LLVM's cap on how many times a loop header runs per entry.
const LOOP_SCALE: f64 = 4096.0;

/// `function`'s odds. `declarations` are its module's globals; `trips` each
/// loop's header and the trips induction proves it, which stand in for the
/// heuristic's 31 in 32.
pub fn estimated(
    context: &Context,
    metadata: &[MetadataNode],
    declarations: &Declarations,
    function: &Function,
    shape: &Shape,
    trips: &BTreeMap<i64, i64>,
) -> Odds {
    let terminal = noreturn::terminal_sites(context, declarations, function, &BTreeSet::new());
    let cold = noreturn::cold(context, declarations, function, &terminal);
    let mut odds = Odds::default();
    let runs = Runs::default();
    for &block in function.layout() {
        let successors: Vec<i64> = function.successors(block).into_iter().map(id).collect();
        match successors.as_slice() {
            [] => {}
            [only] => {
                odds.taken.insert((id(block), *only), 1.0);
            }
            _ => {
                let (heuristic, weights) =
                    weighed(context, metadata, declarations, function, shape, trips, &cold, &runs, block, &successors);
                let total: f64 = weights.iter().sum();
                for (to, weight) in successors.iter().zip(&weights) {
                    *odds.taken.entry((id(block), *to)).or_default() += weight / total;
                }
                odds.by.insert(id(block), heuristic);
            }
        }
    }
    odds.frequency = frequencies(function, shape, &odds, trips);
    odds
}

/// The blocks `cold` says are cold, as `taken_from` finds them.
pub fn cold_blocks(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
) -> BTreeSet<i64> {
    let terminal = noreturn::terminal_sites(context, declarations, function, &BTreeSet::new());
    noreturn::cold(context, declarations, function, &terminal)
}

/// The probability of each edge out of `blocks`, as `estimated` gives them.
pub fn taken_from(
    context: &Context,
    metadata: &[MetadataNode],
    declarations: &Declarations,
    function: &Function,
    shape: &Shape,
    trips: &BTreeMap<i64, i64>,
    blocks: &[BlockId],
) -> BTreeMap<(i64, i64), f64> {
    let cold = cold_blocks(context, declarations, function);
    taken_cold(context, metadata, declarations, function, shape, trips, &cold, blocks)
}

/// `taken_from`, `cold` being `cold_blocks`' of `function`: a caller that asks
/// for a few blocks at a time keeps it, the scan being the function's size.
pub fn taken_cold(
    context: &Context,
    metadata: &[MetadataNode],
    declarations: &Declarations,
    function: &Function,
    shape: &Shape,
    trips: &BTreeMap<i64, i64>,
    cold: &BTreeSet<i64>,
    blocks: &[BlockId],
) -> BTreeMap<(i64, i64), f64> {
    let runs = Runs::default();
    let mut taken = BTreeMap::new();
    for &block in blocks {
        let successors: Vec<i64> = function.successors(block).into_iter().map(id).collect();
        match successors.as_slice() {
            [] => {}
            [only] => {
                taken.insert((id(block), *only), 1.0);
            }
            _ => {
                let (_, weights) =
                    weighed(context, metadata, declarations, function, shape, trips, cold, &runs, block, &successors);
                let total: f64 = weights.iter().sum();
                for (to, weight) in successors.iter().zip(&weights) {
                    *taken.entry((id(block), *to)).or_default() += weight / total;
                }
            }
        }
    }
    taken
}

/// What an invoke's normal edge weighs against its unwind edge's one.
const INVOKE_NORMAL: f64 = 1048575.0;

/// The `!prof` `branch_weights` on `block`'s terminator, one per target it
/// names, in the order it names them (a `br`'s true target then its false, a
/// `switch`'s default then its cases), summed into each distinct successor:
/// cases that share a body weigh together, as LLVM's `calcMetadataWeights`
/// has it. Weights that do not name every target, or sum to nothing, say
/// nothing.
fn declared(
    context: &Context,
    metadata: &[MetadataNode],
    function: &Function,
    block: BlockId,
    successors: &[i64],
) -> Option<Vec<f64>> {
    let last = function.instruction(function.terminator(block)?);
    let (_, node) = last.metadata.iter().find(|(kind, _)| kind == "prof")?;
    let mut operands = metadata.get(node.0 as usize)?.operands.iter();
    let Some(MetadataOperand::String(name)) = operands.next() else { return None };
    if name != "branch_weights" {
        return None;
    }
    let weights = operands
        .map(|one| match one {
            MetadataOperand::Constant(at) => match context.get(*at).kind {
                ConstantKind::Int(bits) => Some(bits as f64),
                ConstantKind::Zero => Some(0.0),
                _ => None,
            },
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let targets = last
        .operands
        .iter()
        .filter_map(|one| if let Operand::Block(target) = one { Some(id(*target)) } else { None })
        .collect::<Vec<_>>();
    if weights.len() != targets.len() || weights.iter().sum::<f64>() <= 0.0 {
        return None;
    }
    Some(
        successors
            .iter()
            .map(|to| targets.iter().zip(&weights).filter(|(target, _)| *target == to).map(|(_, weight)| weight).sum())
            .collect(),
    )
}

/// The first heuristic that tells `block`'s successors apart, and their
/// weights; below the loop and trip heuristics, which first-match, the
/// predictors of a branch of two ways combined.
fn weighed(
    context: &Context,
    metadata: &[MetadataNode],
    declarations: &Declarations,
    function: &Function,
    shape: &Shape,
    trips: &BTreeMap<i64, i64>,
    cold: &BTreeSet<i64>,
    runs: &Runs,
    block: BlockId,
    successors: &[i64],
) -> (Heuristic, Vec<f64>) {
    if let Some(weights) = declared(context, metadata, function, block, successors) {
        return (Heuristic::Declared, weights);
    }
    if let Some(last) = function.terminator(block).map(|one| function.instruction(one))
        && matches!(last.opcode, Opcode::Invoke(_))
        && let [Operand::Block(_), Operand::Block(_)] =
            last.operands.iter().filter(|one| matches!(one, Operand::Block(_))).copied().collect::<Vec<_>>()[..]
        && successors.len() == 2
    {
        // LLVM's `calcInvokeHeuristics`: the unwind edge is taken 1 time in
        // 2^20.
        return (Heuristic::Invoke, vec![INVOKE_NORMAL, 1.0]);
    }
    let split = |favoured: &dyn Fn(i64) -> bool, (yes, no): (f64, f64)| -> Option<Vec<f64>> {
        let count = successors.iter().filter(|&&at| favoured(at)).count();
        if count == 0 || count == successors.len() {
            return None;
        }
        let rest = successors.len() - count;
        Some(successors.iter().map(|&at| if favoured(at) { yes / count as f64 } else { no / rest as f64 }).collect())
    };
    if let Some(weights) = split(&|at| !cold.contains(&at), UNREACHABLE.swap()) {
        return (Heuristic::Unreachable, weights);
    }
    if let Some(found) = shape.loops.iter().find(|one| one.body.contains(&id(block))) {
        if let Some(weights) = split(&|at| found.body.contains(&at), LOOP) {
            return (Heuristic::Loop, weights);
        }
    }
    if let Some(weights) = counted(context, function, shape, trips, runs, block, successors) {
        return (Heuristic::Counted, weights);
    }
    // The predictors left do not first-match: on a branch of two ways every one
    // that applies votes, and the votes are combined as GCC's
    // `combine_predictions_for_bb` does (Dempster-Shafer, predict.cc:1213).
    // Each vote is the probability of the first successor.
    let mut votes: Vec<(Heuristic, f64)> = Vec::new();
    if successors.len() == 2 {
        if let Some((heuristic, likely, nan)) = compared(context, declarations, function, block) {
            let weights = if nan { ORDERED } else { OPCODE };
            let (when_true, when_false) = if likely { weights } else { weights.swap() };
            votes.push((heuristic, when_true / (when_true + when_false)));
        }
    }
    let returns = |at: i64| {
        function.terminator(cfg::block(at)).is_some_and(|last| function.instruction(last).opcode == Opcode::Ret)
    };
    let called = |at: i64| {
        function
            .iter_block(cfg::block(at))
            .filter(|&inst| matches!(
                function.instruction(inst).opcode,
                Opcode::Call(_) | Opcode::Invoke(_)
            ))
            .collect::<Vec<_>>()
    };
    // A way that returns at once is the base case of a recursion that the
    // other way calls: that call is the work, not an exceptional case.
    let base = successors.len() == 2 && successors.iter().any(|&at| returns(at) && called(at).is_empty());
    let calls =
        |at: i64| called(at).into_iter().any(|inst| !(base && recursive(context, declarations, function, inst)));
    if let Some(weights) = split(&|at| !calls(at), CALL) {
        if successors.len() != 2 {
            return (Heuristic::Call, weights);
        }
        votes.push((Heuristic::Call, weights[0] / (weights[0] + weights[1])));
    }
    if successors.len() == 2 && successors.iter().filter(|&&at| returns(at)).count() == 1 {
        let (leaving, staying) = if returns(successors[0]) { (0, 1) } else { (1, 0) };
        // What the returning block returns, when it does nothing else: a
        // constant says it is an error or a flag.
        let odds = returned(context, function, cfg::block(successors[leaving])).unwrap_or(RETURN.swap());
        let mut weights = vec![0.0; 2];
        weights[leaving] = odds.0;
        weights[staying] = odds.1;
        votes.push((Heuristic::Return, weights[0] / (weights[0] + weights[1])));
    } else if successors.len() != 2 {
        if let Some(weights) = split(&|at| !returns(at), RETURN) {
            return (Heuristic::Return, weights);
        }
    }
    if let Some(&(heuristic, _)) = votes.first() {
        let combined = combined(votes.iter().map(|&(_, vote)| vote));
        return (heuristic, vec![combined, 1.0 - combined]);
    }
    (Heuristic::Even, vec![1.0; successors.len()])
}

/// The weights of `block`'s two successors where an enclosing loop with
/// `trips` proven decides its compare: the loop's counters (its header phis,
/// from constant starts) are run trip by trip, and the branch is taken as
/// often as the compare holds. Inner loops first; none that proves it, none.
fn counted(
    context: &Context,
    function: &Function,
    shape: &Shape,
    trips: &BTreeMap<i64, i64>,
    runs: &Runs,
    block: BlockId,
    successors: &[i64],
) -> Option<Vec<f64>> {
    let branch = function.instruction(function.terminator(block)?);
    let (Opcode::Br, [Operand::Value(condition), Operand::Block(yes), Operand::Block(_)]) =
        (&branch.opcode, branch.operands.as_slice())
    else {
        return None;
    };
    let ValueDef::Instruction(compare) = function.value(*condition).def else { return None };
    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { return None };
    let [left, right] = function.instruction(compare).operands.as_slice() else { return None };
    let mut around = shape.loops.iter().filter(|one| one.body.contains(&id(block))).collect::<Vec<_>>();
    around.sort_by_key(|one| one.body.len());
    let share = around.into_iter().filter(|one| one.header != id(block)).find_map(|one| {
        let count = trips.get(&one.header).copied().filter(|count| (1..=COUNTED_TRIPS).contains(count))?;
        let run = runs.of(one);
        let mut run = run.borrow_mut();
        let mut held = 0;
        for trip in 0..count as usize {
            run.at(context, function, one, trip)?;
            let (a, b) =
                (run.evaluated(context, function, trip, *left)?, run.evaluated(context, function, trip, *right)?);
            held += i64::from(compared_as(predicate, a, b));
        }
        // The state after the last trip is stepped to as well: a counter that
        // has no value there leaves the loop unrun.
        run.at(context, function, one, count as usize)?;
        Some(held as f64 / count as f64)
    })?;
    // Never all but certain: a block must stay ordered, not unreachable.
    let share = share.clamp(1.0 / COUNTED_TRIPS as f64, 1.0 - 1.0 / COUNTED_TRIPS as f64);
    let [first, second] = successors else { return None };
    let taken = if first == &id(*yes) { share } else { 1.0 - share };
    (first != second).then(|| vec![taken, 1.0 - taken])
}

/// What a loop's counters hold at each of its trips, `counted`'s run of the
/// loop: the same for every branch in the loop, so the trips are run for the
/// first branch that asks and no further than any asks. `broken` once a counter
/// has no value, or the loop no start.
#[derive(Default)]
struct Run {
    values: Vec<BTreeMap<llrm_mir::module::ValueId, Option<(u128, u32)>>>,
    broken: bool,
    /// What a value is at a trip, for the branches that compare the same
    /// value: found for the first, not run again for each.
    evaluations: HashMap<(usize, llrm_mir::module::ValueId), Option<(u128, u32)>>,
}

#[derive(Default)]
struct Runs(std::cell::RefCell<HashMap<i64, Rc<std::cell::RefCell<Run>>>>);

thread_local! {
    static RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static EVALUATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many values this thread has worked out at a trip for `counted`.
pub fn values_evaluated() -> usize {
    EVALUATIONS.with(std::cell::Cell::get)
}

/// How many loops this thread has started to run trip by trip for `counted`.
pub fn loops_run() -> usize {
    RUNS.with(std::cell::Cell::get)
}

impl Runs {
    fn of(
        &self,
        one: &crate::graph::loops::Loop,
    ) -> Rc<std::cell::RefCell<Run>> {
        Rc::clone(self.0.borrow_mut().entry(one.header).or_insert_with(|| {
            RUNS.with(|runs| runs.set(runs.get() + 1));
            Rc::default()
        }))
    }
}

impl Run {
    /// `operand` at the start of trip `trip` (`at` has run it), kept per value.
    fn evaluated(
        &mut self,
        context: &Context,
        function: &Function,
        trip: usize,
        operand: Operand,
    ) -> Option<(u128, u32)> {
        let Operand::Value(value) = operand else {
            return evaluated(context, function, self.values.get(trip)?, operand, 6);
        };
        if let Some(known) = self.evaluations.get(&(trip, value)) {
            return *known;
        }
        EVALUATIONS.with(|runs| runs.set(runs.get() + 1));
        let found = evaluated(context, function, self.values.get(trip)?, operand, 6);
        self.evaluations.insert((trip, value), found);
        found
    }

    /// The counters at the start of trip `trip` of `one` (`trip` = its trips is
    /// the state after the last), run as far as that.
    fn at(
        &mut self,
        context: &Context,
        function: &Function,
        one: &crate::graph::loops::Loop,
        trip: usize,
    ) -> Option<&BTreeMap<llrm_mir::module::ValueId, Option<(u128, u32)>>> {
        if self.broken {
            return None;
        }
        // A trip run for an earlier branch is found as it was, not from the
        // loop's phis again: every branch of the loop asks every trip.
        if trip < self.values.len() {
            return self.values.get(trip);
        }
        let phis = function
            .iter_block(cfg::block(one.header))
            .filter(|&inst| function.instruction(inst).opcode == Opcode::Phi)
            .collect::<Vec<_>>();
        let outside = |at: &Operand| matches!(at, Operand::Block(from) if !one.body.contains(&id(*from)));
        let incoming = |inst, inside: bool| {
            let pairs = function
                .instruction(inst)
                .operands
                .chunks(2)
                .filter(|pair| outside(&pair[1]) != inside)
                .map(|pair| pair[0])
                .collect::<Vec<_>>();
            (!pairs.is_empty() && pairs.iter().all(|one| *one == pairs[0])).then(|| pairs[0])
        };
        if self.values.is_empty() {
            let mut values = BTreeMap::new();
            for &inst in &phis {
                let Some(result) = function.instruction(inst).result else {
                    self.broken = true;
                    return None;
                };
                values.insert(
                    result,
                    incoming(inst, false).and_then(|start| evaluated(context, function, &BTreeMap::new(), start, 6)),
                );
            }
            self.values.push(values);
        }
        while self.values.len() <= trip {
            let values = self.values.last().expect("the start");
            let next = phis
                .iter()
                .map(|&inst| {
                    Some((
                        function.instruction(inst).result?,
                        incoming(inst, true).and_then(|step| evaluated(context, function, values, step, 6)),
                    ))
                })
                .collect::<Option<BTreeMap<_, _>>>();
            let Some(next) = next else {
                self.broken = true;
                return None;
            };
            self.values.push(next);
        }
        self.values.get(trip)
    }
}

/// The most trips `counted` will run.
const COUNTED_TRIPS: i64 = 4096;

/// An integer operand's bits and width, from the loop counters `values`
/// and constants, through the arithmetic a counter is stepped and offset by.
fn evaluated(
    context: &Context,
    function: &Function,
    values: &BTreeMap<llrm_mir::module::ValueId, Option<(u128, u32)>>,
    operand: Operand,
    depth: u32,
) -> Option<(u128, u32)> {
    let width = |one: &Operand| {
        function
            .operand_type(context, *one)
            .and_then(|ty| context.types.int_bits(ty))
            .filter(|bits| (1..=64).contains(bits))
    };
    let mask = |bits: u32| (1u128 << bits) - 1;
    match operand {
        Operand::Constant(at) => {
            let ConstantKind::Int(bits) = context.get(at).kind else { return None };
            let bits_wide = width(&operand)?;
            Some((bits & mask(bits_wide), bits_wide))
        }
        Operand::Value(value) => {
            if let Some(known) = values.get(&value) {
                return known.as_ref().map(|&(bits, wide)| (bits, wide));
            }
            let ValueDef::Instruction(inst) = function.value(value).def else { return None };
            let instruction = function.instruction(inst);
            let bits_wide = width(&operand)?;
            let operands = instruction.operands.as_slice();
            if depth == 0 {
                return None;
            }
            let signed = |(bits, wide): (u128, u32)| {
                if bits >> (wide - 1) & 1 == 1 { bits as i128 - (1i128 << wide) } else { bits as i128 }
            };
            let result = match (&instruction.opcode, operands) {
                (Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)), [a, b]) => {
                    let (a, b) = (
                        evaluated(context, function, values, *a, depth - 1)?,
                        evaluated(context, function, values, *b, depth - 1)?,
                    );
                    match op {
                        BinaryOp::Add => a.0.wrapping_add(b.0),
                        BinaryOp::Sub => a.0.wrapping_sub(b.0),
                        BinaryOp::Mul => a.0.wrapping_mul(b.0),
                        _ => a.0.checked_shl(u32::try_from(b.0).ok().filter(|&by| by < bits_wide)?)?,
                    }
                }
                (Opcode::Cast(CastOp::ZExt), [a]) => evaluated(context, function, values, *a, depth - 1)?.0,
                (Opcode::Cast(CastOp::Trunc), [a]) => evaluated(context, function, values, *a, depth - 1)?.0,
                (Opcode::Cast(CastOp::SExt), [a]) => {
                    signed(evaluated(context, function, values, *a, depth - 1)?) as u128
                }
                _ => return None,
            };
            Some((result & mask(bits_wide), bits_wide))
        }
        _ => None,
    }
}

/// `predicate` of two integers of one width.
fn compared_as(
    predicate: IntPredicate,
    (a, wide): (u128, u32),
    (b, _): (u128, u32),
) -> bool {
    let signed = |bits: u128| if bits >> (wide - 1) & 1 == 1 { bits as i128 - (1i128 << wide) } else { bits as i128 };
    match predicate {
        IntPredicate::Eq => a == b,
        IntPredicate::Ne => a != b,
        IntPredicate::Ugt => a > b,
        IntPredicate::Uge => a >= b,
        IntPredicate::Ult => a < b,
        IntPredicate::Ule => a <= b,
        IntPredicate::Sgt => signed(a) > signed(b),
        IntPredicate::Sge => signed(a) >= signed(b),
        IntPredicate::Slt => signed(a) < signed(b),
        IntPredicate::Sle => signed(a) <= signed(b),
    }
}

/// The odds of reaching a block that returns a constant and does nothing else:
/// GCC's `PRED_NEGATIVE_RETURN`, `PRED_NULL_RETURN` and `PRED_CONST_RETURN`.
/// None where it returns a computed value, or anything but the return.
fn returned(
    context: &Context,
    function: &Function,
    block: BlockId,
) -> Option<(f64, f64)> {
    let body = function.block(block);
    let (1, Some(only)) = (body.len(), body.first()) else { return None };
    let instruction = function.instruction(only);
    let (Opcode::Ret, [Operand::Constant(value)]) = (&instruction.opcode, instruction.operands.as_slice()) else {
        return None;
    };
    let constant = context.get(*value);
    match constant.kind {
        ConstantKind::Null => Some(NULL_RETURN),
        ConstantKind::Int(bits) => {
            let width = context.types.int_bits(constant.ty).filter(|&width| (1..=64).contains(&width))?;
            let negative = bits >> (width - 1) & 1 == 1;
            Some(if negative && width > 1 { NEGATIVE_RETURN } else { CONST_RETURN })
        }
        _ => None,
    }
}

/// Whether `inst` calls `function` itself: the work of the function, not the
/// report of an exceptional case the call heuristic assumes.
fn recursive(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    inst: InstId,
) -> bool {
    llrm_mir::memory::callee(context, function, inst).is_some_and(|callee| {
        matches!(
            declarations.get(callee.0 as usize).map(|global| &global.kind),
            Some(GlobalKind::Function(one)) if std::ptr::eq(&**one, function)
        )
    })
}

/// The probability of the first way, from the probabilities each predictor
/// gives it: Dempster-Shafer, as `combine_predictions_for_bb` folds them (from
/// one half, `p' = p * q / (p * q + (1 - p) * (1 - q))`; two certainties that
/// disagree leave one half).
fn combined(votes: impl Iterator<Item = f64>) -> f64 {
    votes.fold(0.5, |p, q| {
        let d = p * q + (1.0 - p) * (1.0 - q);
        if d == 0.0 { 0.5 } else { p * q / d }
    })
}

trait Swap {
    fn swap(self) -> Self;
}

impl Swap for (f64, f64) {
    fn swap(self) -> Self {
        (self.1, self.0)
    }
}

/// The compare deciding `block`'s branch, if a heuristic reads it: which
/// one, whether the branch's true edge is the likely one, and whether it
/// tests for NaN, which takes the extreme weights.
fn compared(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    block: BlockId,
) -> Option<(Heuristic, bool, bool)> {
    let branch = function.instruction(function.terminator(block)?);
    let (Opcode::Br, [Operand::Value(condition), ..]) = (&branch.opcode, branch.operands.as_slice()) else {
        return None;
    };
    let ValueDef::Instruction(inst) = function.value(*condition).def else { return None };
    let compare = function.instruction(inst);
    let [left, right] = compare.operands.as_slice() else { return None };
    match compare.opcode {
        Opcode::ICmp(predicate) => {
            let pointer = |one: &Operand| {
                function.operand_type(context, *one).is_some_and(|ty| matches!(context.types.get(ty), Type::Pointer(_)))
            };
            if pointer(left) || pointer(right) {
                return match predicate {
                    IntPredicate::Eq => Some((Heuristic::Pointer, false, false)),
                    IntPredicate::Ne => Some((Heuristic::Pointer, true, false)),
                    _ => None,
                };
            }
            let constant = |one: &Operand| match one {
                Operand::Constant(at) => match context.get(*at).kind {
                    ConstantKind::Int(bits) => {
                        let width = function.operand_type(context, *one).and_then(|ty| context.types.int_bits(ty))?;
                        let mask = if width >= 128 { u128::MAX } else { (1u128 << width) - 1 };
                        Some(if bits & mask == mask { -1 } else { i128::try_from(bits & mask).ok()? })
                    }
                    ConstantKind::Null | ConstantKind::Zero => Some(0),
                    _ => None,
                },
                _ => None,
            };
            // The compared value on the left: `0 == x` reads as `x == 0`.
            let (predicate, value, compared) = match (constant(left), constant(right)) {
                (_, Some(value)) => (predicate, value, left),
                (Some(value), None) => (predicate.swapped(), value, right),
                _ => return None,
            };
            // A flag is no quantity: BASIC's `IF a AND b` tests a 0/-1 truth
            // value against 0, `x AND 1` one bit; neither says how often.
            if flag(context, declarations, function, *compared) {
                return None;
            }
            // Nor is a three-way compare's sign: of its result only equality
            // with 0 says something, that the data are unlikely equal.
            if three_way(context, declarations, function, *compared)
                && !(value == 0 && matches!(predicate, IntPredicate::Eq | IntPredicate::Ne))
            {
                return None;
            }
            let likely = match (predicate, value) {
                (IntPredicate::Eq, 0 | -1) => false,
                (IntPredicate::Ne, 0 | -1) => true,
                (IntPredicate::Slt, 0 | 1) | (IntPredicate::Sle, -1 | 0) => false,
                (IntPredicate::Sgt, -1 | 0) | (IntPredicate::Sge, 0 | 1) => true,
                _ => return None,
            };
            Some((Heuristic::Zero, likely, false))
        }
        Opcode::FCmp(predicate) => match predicate {
            FloatPredicate::Oeq | FloatPredicate::Ueq => Some((Heuristic::Float, false, false)),
            FloatPredicate::One | FloatPredicate::Une => Some((Heuristic::Float, true, false)),
            FloatPredicate::Uno => Some((Heuristic::Float, false, true)),
            FloatPredicate::Ord => Some((Heuristic::Float, true, true)),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `operand` has at most one bit that can be set: a truth value,
/// or a one-bit mask. No known-bits analysis answers it, so these are the
/// two forms: the first BASIC's, the second LLVM's `(x & pow2)`.
fn flag(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    operand: Operand,
) -> bool {
    truth(context, declarations, function, operand, 4) || single_bit(context, function, operand)
}

/// Whether `operand` is `x & 2^n`, as LLVM's zero heuristic leaves
/// `(x & pow2) ==/!= 0` alone.
fn single_bit(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> bool {
    let Operand::Value(value) = operand else { return false };
    let ValueDef::Instruction(inst) = function.value(value).def else { return false };
    let instruction = function.instruction(inst);
    let Opcode::Binary(BinaryOp::And) = instruction.opcode else { return false };
    instruction
        .operands
        .iter()
        .any(
            |one| match one {
                Operand::Constant(at) => matches!(
                    context.get(*at).kind,
                    ConstantKind::Int(bits) if bits.is_power_of_two()
                ),
                _ => false,
            },
        )
}

/// Whether `operand` is the result of a call to a routine stated a
/// three-way compare, at the call or of the callee.
fn three_way(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    operand: Operand,
) -> bool {
    let Operand::Value(value) = operand else { return false };
    let ValueDef::Instruction(inst) = function.value(value).def else { return false };
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return false };
    let declared = llrm_mir::memory::callee(context, function, inst)
        .and_then(|one| declarations.get(one.0 as usize))
        .and_then(|one| one.function());
    Facts::of(&info.attrs).three_way_compare() || declared.is_some_and(|one| Facts::of(&one.attrs).three_way_compare())
}

/// Whether `operand` is provably 0 or all ones, or 0 or 1: a compare, one
/// widened, bitwise logic of those, `depth` operations deep, or the result of a
/// call whose `range` says so (a routine that returns a truth value as an
/// integer, as Nib's bool is a byte).
fn truth(
    context: &Context,
    declarations: &Declarations,
    function: &Function,
    operand: Operand,
    depth: u32,
) -> bool {
    let Operand::Value(value) = operand else { return false };
    let ValueDef::Instruction(inst) = function.value(value).def else { return false };
    let instruction = function.instruction(inst);
    match &instruction.opcode {
        Opcode::ICmp(_) | Opcode::FCmp(_) => true,
        Opcode::Cast(CastOp::SExt | CastOp::ZExt) => {
            instruction.operands.first().is_some_and(|one| truth(context, declarations, function, *one, depth))
        }
        Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) if depth > 0 => {
            instruction.operands.iter().all(|one| truth(context, declarations, function, *one, depth - 1))
        }
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let bits = |ty| context.types.int_bits(ty);
            let callee = llrm_mir::memory::callee(context, function, inst);
            // Stated, at the call or on the callee.
            let attributes: Vec<_> = info
                .return_attrs
                .iter()
                .chain(
                    callee
                        .and_then(|one| declarations.get(one.0 as usize))
                        .and_then(|one| one.function())
                        .iter()
                        .flat_map(|one| one.return_attrs.iter()),
                )
                .cloned()
                .collect();
            Facts::of_typed(&attributes, bits, true).range().is_some_and(|found| found.hi - found.lo <= 1)
        }
        _ => false,
    }
}

/// A natural loop as `propagated` reads it.
pub struct Cycle<'a> {
    pub header: i64,
    pub latches: &'a BTreeSet<i64>,
    pub body: &'a llrm_mir::forest::Body,
    /// The trips induction proves, in place of the heuristic's odds.
    pub trips: Option<i64>,
}

/// Each block's frequency, the entry's 1, loops scaled by their back edges.
fn frequencies(
    function: &Function,
    shape: &Shape,
    odds: &Odds,
    trips: &BTreeMap<i64, i64>,
) -> BTreeMap<i64, f64> {
    let order = reverse_postorder(function);
    let cycles: Vec<Cycle> = shape
        .loops
        .iter()
        .map(|one| Cycle {
            header: one.header,
            latches: &one.latches,
            body: &one.body,
            trips: trips.get(&one.header).copied(),
        })
        .collect();
    let predecessors = |at: i64| function.predecessors(cfg::block(at)).into_iter().map(id).collect::<Vec<_>>();
    let successors = |at: i64| function.successors(cfg::block(at)).into_iter().map(id).collect::<Vec<_>>();
    propagated(&order, &predecessors, &successors, &cycles, &|from, to| odds.probability(from, to).unwrap_or(0.0))
}

/// Frequencies over any CFG: `order` its reachable blocks in reverse
/// postorder, the entry first; `cycles` its natural loops, innermost first;
/// `edge` each edge's probability. A loop with proven `trips` stays in as
/// many times as they say: its exit test, where the loop has one exit or
/// where it is tested at the header or a latch, takes `1 / (trips + 1 -
/// tested)` out, `tested` being 1 when the test follows a trip and 0 when it
/// precedes one. Only here, so that MIR and LIR estimates agree.
pub fn propagated(
    order: &[i64],
    predecessors: &dyn Fn(i64) -> Vec<i64>,
    successors: &dyn Fn(i64) -> Vec<i64>,
    cycles: &[Cycle],
    given: &dyn Fn(i64, i64) -> f64,
) -> BTreeMap<i64, f64> {
    propagated_edges(order, predecessors, successors, cycles, given).0
}

/// `propagated`, and each edge's probability as the trips left it.
///
/// Worked over the blocks by place in `order`: each block's forward
/// predecessors and the edges they come by are found once, a loop's blocks are
/// marked with its own stamp, and an edge's plan is made at its first ask.
pub fn propagated_edges(
    order: &[i64],
    predecessors: &dyn Fn(i64) -> Vec<i64>,
    successors: &dyn Fn(i64) -> Vec<i64>,
    cycles: &[Cycle],
    given: &dyn Fn(i64, i64) -> f64,
) -> (BTreeMap<i64, f64>, BTreeMap<(i64, i64), f64>) {
    let count = order.len();
    let place: HashMap<i64, usize> = order.iter().enumerate().map(|(at, &block)| (block, at)).collect();
    let next: Vec<Vec<i64>> = order.iter().map(|&at| successors(at)).collect();
    // Each loop's header by place, and each header's loops.
    let mut headed: HashMap<i64, Vec<usize>> = HashMap::default();
    for (index, one) in cycles.iter().enumerate() {
        headed.entry(one.header).or_default().push(index);
    }
    // Each block's loops, inner first, and each loop's reachable blocks in
    // `order`: a loop is asked about from its own blocks, so no ask walks every
    // loop of the function.
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut inside_of: Vec<Vec<usize>> = vec![Vec::new(); cycles.len()];
    for (index, one) in cycles.iter().enumerate() {
        for at in one.body.iter() {
            if let Some(&at) = place.get(at) {
                around[at].push(index);
            }
        }
    }
    for at in 0..count {
        for &index in &around[at] {
            inside_of[index].push(at);
        }
    }
    // Each block's predecessors by place with the edge they come by, whether it
    // goes back to a loop's header from inside the loop, in the order the
    // closure gives them.
    struct Coming {
        from: usize,
        edge: usize,
        back: bool,
    }
    let mut coming: Vec<Vec<Coming>> = Vec::with_capacity(count);
    for (at, &block) in order.iter().enumerate() {
        let mut list = Vec::new();
        for from in predecessors(block) {
            let Some(&from) = place.get(&from) else { continue };
            let Some(edge) = next[from].iter().position(|to| *to == block) else { continue };
            let back = headed.get(&block).into_iter().flatten().any(|&index| cycles[index].body.contains(&order[from]));
            list.push(Coming { from, edge, back });
        }
        let _ = at;
        coming.push(list);
    }
    // How often a trip of each loop reaches its exiting blocks together, which
    // its exit test runs: more than once where one is in a loop nested in it,
    // and each visit then takes that much less of the exit, so that the trips
    // stay.
    let mut visits: Vec<f64> = vec![1.0; cycles.len()];
    let exiting: std::cell::RefCell<Vec<Option<usize>>> = std::cell::RefCell::new(vec![None; cycles.len()]);
    // Asked only of a loop whose trips are proven, and then once.
    let exiting_blocks = |index: usize| -> usize {
        *exiting.borrow_mut()[index].get_or_insert_with(|| {
            let one = &cycles[index];
            one.body
                .iter()
                .filter(|&&at| place.get(&at).is_some_and(|&at| next[at].iter().any(|to| !one.body.contains(to))))
                .count()
        })
    };
    // Which loop's trips fix an edge, and its share of its side's mass: what
    // the loops and the odds say, not what `visits` is by the time the edge is
    // asked about, so it is worked out once.
    struct Counted {
        cycle: usize,
        trips: f64,
        tested: f64,
        part: f64,
        inside: bool,
    }
    let plans: std::cell::RefCell<Vec<Vec<Option<Option<Rc<Counted>>>>>> =
        std::cell::RefCell::new(next.iter().map(|list| vec![None; list.len()]).collect());
    let plan = |from: usize, edge: usize| -> Option<Rc<Counted>> {
        if let Some(known) = &plans.borrow()[from][edge] {
            return known.clone();
        }
        let made = (|| {
            let to = next[from][edge];
            let list = &next[from];
            for &index in &around[from] {
                let one = &cycles[index];
                let inside = list.iter().filter(|at| one.body.contains(at)).count();
                let outside = list.len() - inside;
                if inside == 0 || outside == 0 {
                    continue;
                }
                // A loop without proven trips leaves the edge to the loops
                // around it, which may state them.
                let Some(trips) = one.trips else { continue };
                if exiting_blocks(index) != 1 && order[from] != one.header && !one.latches.contains(&order[from]) {
                    return None;
                }
                let tested = if order[from] == one.header && !one.latches.contains(&order[from]) { 0.0 } else { 1.0 };
                // The trips fix how often the loop is left, not which way: each
                // edge takes its side's mass by its own odds.
                let side: BTreeSet<i64> =
                    list.iter().copied().filter(|at| one.body.contains(at) == one.body.contains(&to)).collect();
                let weight: f64 = side.iter().map(|at| given(order[from], *at)).sum();
                let part = if weight > 0.0 { given(order[from], to) / weight } else { 1.0 / side.len() as f64 };
                return Some(Rc::new(Counted {
                    cycle: index,
                    trips: trips as f64,
                    tested,
                    part,
                    inside: one.body.contains(&to),
                }));
            }
            None
        })();
        plans.borrow_mut()[from][edge] = Some(made.clone());
        made
    };
    let edge = |from: usize, edge: usize, visits: &[f64]| -> f64 {
        match plan(from, edge) {
            Some(found) => {
                let visited = visits[found.cycle].max(1.0);
                let stay = 1.0 - (1.0 - (found.trips - found.tested) / (found.trips + 1.0 - found.tested)) / visited;
                if found.inside { stay * found.part } else { (1.0 - stay) * found.part }
            }
            None => given(order[from], next[from][edge]),
        }
    };
    // The edge from a block to another, by their places.
    let edge_between = |from: usize, to: i64, visits: &[f64]| -> f64 {
        match next[from].iter().position(|one| *one == to) {
            Some(at) => edge(from, at, visits),
            None => given(order[from], to),
        }
    };
    // Innermost first: an inner header's scale is known when its outer loop is
    // weighed.
    let mut scale: Vec<f64> = vec![1.0; count];
    let mut mass: Vec<f64> = vec![0.0; count];
    let mut stamp: Vec<usize> = vec![usize::MAX; count];
    for (index, found) in cycles.iter().enumerate() {
        let header = place.get(&found.header).copied();
        let weigh = |mass: &mut Vec<f64>, stamp: &mut Vec<usize>, rough: bool, visits: &[f64], scale: &[f64]| {
            let inside = &inside_of[index];
            #[cfg(test)]
            VISITS.with(|visits| visits.set(visits.get() + found.body.len()));
            for &at in inside {
                stamp[at] = index;
                mass[at] = 0.0;
            }
            for &at in inside {
                let entering: f64 = if Some(at) == header {
                    1.0
                } else {
                    coming[at]
                        .iter()
                        .filter(|one| stamp[one.from] == index && !one.back)
                        .map(|one| {
                            mass[one.from]
                                * if rough {
                                    given(order[one.from], order[at])
                                } else {
                                    edge(one.from, one.edge, visits)
                                }
                        })
                        .sum()
                };
                let inner = if Some(at) == header { 1.0 } else { scale[at] };
                mass[at] = entering * inner;
            }
        };
        if found.trips.is_some() {
            // By the odds alone: where the exits are visited, and how often.
            weigh(&mut mass, &mut stamp, true, &visits, &scale);
            let total: f64 = found
                .body
                .iter()
                .filter(|at| place.get(at).is_some_and(|&at| next[at].iter().any(|to| !found.body.contains(to))))
                .map(|at| mass[place[at]])
                .sum();
            visits[index] = total;
        }
        weigh(&mut mass, &mut stamp, false, &visits, &scale);
        let back: f64 = found
            .latches
            .iter()
            .map(|latch| match place.get(latch) {
                Some(&latch) => mass[latch] * edge_between(latch, found.header, &visits),
                None => 0.0,
            })
            .sum();
        let mut weighed_scale = (1.0 / (1.0 - back.min(1.0 - 1.0 / LOOP_SCALE))).min(LOOP_SCALE);
        // The proven trips are the scale, not what the odds around them add up
        // to: a sum within a hair of 1 is where a leak of a tenth of a
        // percent in a loop nested in this one reads as a sixth of the trips.
        if let Some(trips) = found.trips {
            let exiting: Vec<i64> = found
                .body
                .iter()
                .copied()
                .filter(|at| place.get(at).is_some_and(|&at| next[at].iter().any(|to| !found.body.contains(to))))
                .collect();
            if !exiting.is_empty() && exiting.iter().all(|at| found.latches.contains(at)) {
                weighed_scale = trips as f64;
            } else if exiting == [found.header] && !found.latches.contains(&found.header) {
                weighed_scale = trips as f64 + 1.0;
            }
        }
        if let Some(header) = header {
            scale[header] = weighed_scale;
        }
    }
    let mut frequency: Vec<f64> = vec![0.0; count];
    for at in 0..count {
        let entering: f64 = if at == 0 {
            1.0
        } else {
            coming[at]
                .iter()
                .filter(|one| !one.back)
                .map(|one| frequency[one.from] * edge(one.from, one.edge, &visits))
                .sum()
        };
        frequency[at] = entering * scale[at];
    }
    let mut edges = BTreeMap::new();
    for from in 0..count {
        for at in 0..next[from].len() {
            edges.insert((order[from], next[from][at]), edge(from, at, &visits));
        }
    }
    (order.iter().copied().zip(frequency).collect(), edges)
}

/// Blocks in reverse postorder from `entry`, by `successors`.
pub fn reverse_postorder_of(
    entry: i64,
    successors: &dyn Fn(i64) -> Vec<i64>,
) -> Vec<i64> {
    let mut seen = BTreeSet::from([entry]);
    let mut post = Vec::new();
    let mut stack = vec![(entry, 0usize)];
    while let Some((block, next)) = stack.pop() {
        let all = successors(block);
        if let Some(&to) = all.get(next) {
            stack.push((block, next + 1));
            if seen.insert(to) {
                stack.push((to, 0));
            }
        } else {
            post.push(block);
        }
    }
    post.reverse();
    post
}

/// `function`'s reachable blocks, each before its successors but back edges.
fn reverse_postorder(function: &Function) -> Vec<i64> {
    let Some(entry) = function.entry() else { return Vec::new() };
    reverse_postorder_of(id(entry), &|at| function.successors(cfg::block(at)).into_iter().map(id).collect())
}

// Blocks the loops' weighings looked at to find their own, for a test that a
// loop is weighed by its own blocks.
#[cfg(test)]
thread_local! {
    pub(crate) static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[path = "branchprob_tests.rs"]
mod tests;
