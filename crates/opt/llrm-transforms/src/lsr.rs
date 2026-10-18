//! Strength reduction: replaces a loop's induction variables, and the address
//! arithmetic built from them, with the cheapest set of counters the target
//! can address, so the loop carries few registers and no multiply (LLVM's
//! LoopStrengthReduce, gcc's ivopts).
//!
//! # How it works
//! Every read of a counter, or of a value affine in one, is one
//! `induction::Recurrence`, `pointer + start + step * trip`, with `start` and
//! `step` forms in invariant symbols (`Scev`, modulo the width).
//! `induction::users` finds each recurrence and how it is read: as an
//! address, compared with an invariant, or otherwise. A recurrence times a
//! recurrence is not one.
//!
//! Loops are taken innermost first (smallest body first), each once. For one
//! loop the pass needs a preheader, one latch, and a latch arm for every
//! counter; else the loop is left. A loop of more groups of uses than
//! `Bounds::groups` (gcc's `iv-max-considered-uses`) is left too. Three steps
//! come before the choice:
//! - a post-tested latch compare of a counter with a constant reads the
//!   counter's step instead, with the bound a step on, so one register serves
//!   for two (LLVM's `OptimizeLoopTermCond`);
//! - `crate::mirror` makes the pointer for a counter only some trips step,
//!   where the target's addresses take no scale (not at `-Os`);
//! - the loop is then planned again from the new body.
//!
//! The candidates are recurrences the loop could carry in a register:
//! - each use's own, and its start with the symbols split every way between
//!   counter and base (up to three symbols), with and without the constant for
//!   a compare or an exit, and without it for an address, which takes the
//!   constant as its displacement (LLVM strips the immediate from the base
//!   register, `LSRInstance::GenerateConstantOffsets`);
//! - the loop's existing counters;
//! - one counter per scale of an address form whose index is wider than the
//!   use;
//! - each step counted to zero at the exit, `-trips * step`, where the loop has
//!   a counted exit.
//!
//! A use is realized from a candidate `r` as `base + k * r + c` (`Fit`): the
//! address form that takes it, or the shift, multiply and add that compute
//! it. A value read after the loop may come from the trip the candidate
//! counted, or from the known count. An equality with an invariant takes the
//! candidate itself and moves the bias into the invariant. The counted exit
//! either keeps its compare or tests one candidate for its last value
//! (`Exit`), which costs nothing where that is zero. A counter read only by
//! its own step and the exit is then dead and `dead` removes it, so a loop
//! counting `i` from 0 to `n` keeps one counter, counted to zero.
//!
//! ```text
//!  for (i = 0; i < n; i++) s += a[i] * b[i]      (a, b: 16-bit elements)
//!
//!  before                          after, one counter counted to zero
//!  pre:                            pre:  c0 = -n; ea = a + 2n; eb = b + 2n
//!  l1: i = phi [0,pre] [i+1,l2]    l1:   c = phi [c0,pre] [c+1,l2]
//!      if i < n  -> l2 else exit         if c != 0 -> l2 else exit
//!  l2: o  = i * 2                  l2:   pa = ea + c*2   ; the address
//!      pa = a + o                          pb = eb + c*2   ; form takes *2
//!      pb = b + o
//! ```
//!
//! A candidate may test the exit only where it cannot come round to its final
//! value on an earlier trip. Its modular period is `2^w / gcd(step, 2^w)`;
//! the loop's most trips must not exceed it (nor equal it, where the test is
//! before a trip). The compare becomes an equality or inequality against the
//! candidate's last value. Only a pre-tested unit step has a symbolic
//! count, so a counter stepping by more keeps its compare unless the count is
//! a number.
//!
//! ```text
//!  i = 0..10 step 1, p = &a[i]            p = a, end = a + 10*step
//!  loop: *p = ...; p += step; i++    ==>  loop: *p = ...; p += step
//!        exit if i == 10                        exit if p == end
//! ```
//!
//! The price of a set of candidates (`Problem::total_of`) is the sum of
//! - an add per trip for each counter's step (a pointer's step is
//!   `profit::advance`; a counter wider than the native index pays the
//!   operand-size prefix);
//! - each use's cheapest realization, at its block frequency;
//! - building starts, steps and invariants in the preheader;
//! - a copy for each address that puts two values in the wrong register class;
//! - the spill traffic of every value past the target's registers
//!   (`crate::spill`, the one spill model `gvn` shares).
//!
//! Frequencies are `Products`: trips multiplied, ten where unproven (#203),
//! a branch's cold arm less. Prices, registers and address forms are the
//! target's (`OperationCosts`, `Machine`, `AddressForm`): the pass prices, it
//! does not know the machine. `kept_across` says how many registers a call to
//! a named callee keeps, from its contract, and `multiply_by` what a multiply
//! by a constant costs, from the backend's shift and add chains. A spilled
//! counter costs a memory update a trip, an invariant a load per read.
//!
//! The search is gcc's `find_optimal_iv_set`: from the loop's own counters
//! and from a greedy start (each use's cheapest candidate, added until every
//! use fits), add, remove or swap one candidate while the price falls, a tie
//! going to fewer counters; the cheaper end wins. Past
//! `Bounds::all_candidates` candidates, replacing is tried once, only where
//! no add or remove helps, and not for a candidate serving more than
//! `Bounds::always_prune` uses or exactly one. The loop is rewritten only
//! when the set beats the counters it has.
//!
//! A loop with another way out was never counted before, so its rewrite is
//! kept only if the whole function's price (`profit::motion_price`) does
//! not rise; at `-Os` it is left as it is.
//!
//! A symbolic count holds only once the loop is entered, so a loop tested
//! for its last value behind one is entered at its body behind a skip guard
//! (`counting::skip_guard`, `rotate`), unless a branch over the entry already
//! proves the first trip (`induction::guards`, as LLVM's
//! `isLoopEntryGuardedByCond`).
//!
//! In a space where a pointer carries into its selector, an integer cannot
//! index a pair: the use advances the pointer, and pays what an advance
//! costs there (`profit::advance`). `crate::window` splits the huge pointers
//! this chooses.
//!
//! # Example
//! Before:
//! ```mir
//! define ... @"_sum@3"(ptr nocapture readonly %0, i16 %1) ... {
//! b1:
//!   br label %b2
//!
//! b2:
//!   %2 = phi i16 [ 0, %b1 ], [ %10, %b4 ]
//!   %3 = phi i16 [ 0, %b1 ], [ %9, %b4 ]
//!   %4 = icmp slt i16 %2, %1
//!   br i1 %4, label %b4, label %b3
//!
//! b3:
//!   %5 = phi i16 [ %3, %b2 ]
//!   ret i16 %5
//!
//! b4:
//!   %6 = mul i16 %2, 2
//!   %7 = getelementptr inbounds i8, ptr %0, i16 %6
//!   %8 = load i16, ptr %7, !tbaa !9
//!   %9 = add nsw i16 %3, %8
//!   %10 = add nsw i16 %2, 1
//!   br label %b2
//! }
//! ```
//! After:
//! ```mir
//! define ... @"_sum@3"(ptr nocapture readonly %0, i16 %1) ... {
//! b1:
//!   %2 = shl i16 %1, 1
//!   %3 = sub i16 0, %2
//!   %4 = getelementptr i8, ptr %0, i16 %2
//!   %5 = icmp sle i16 %1, 0
//!   br i1 %5, label %b3, label %12
//!
//! b3:
//!   %6 = phi i16 [ 0, %b1 ], [ %10, %13 ]
//!   ret i16 %6
//!
//! b4:
//!   %7 = phi i16 [ %10, %b4 ], [ 0, %12 ]
//!   %lsr.iv1 = phi i16 [ %lsr.iv.next, %b4 ], [ %3, %12 ]
//!   %8 = getelementptr i8, ptr %4, i16 %lsr.iv1
//!   %9 = load i16, ptr %8, !tbaa !9
//!   %10 = add nsw i16 %7, %9
//!   %lsr.iv.next = add i16 %lsr.iv1, 2
//!   %11 = icmp ne i16 %lsr.iv.next, 0
//!   br i1 %11, label %b4, label %13
//!   ; ...
//! }
//! ```
//! The counters `i` and `2 * i` are one: `%lsr.iv1` runs from `-2n` to zero,
//! the address is `%4 + %lsr.iv1` with `%4 = a + 2n`, and the loop ends when
//! `%lsr.iv1` reaches zero.
//!
//! From `--dump` of `sum` (`for (i = 0; i < n; i++) s += a[i];`, `int *a`) at
//! -O2 -m16, stages `fixednarrow` and `lsr`.
//!
//! # Runs
//! Once per body, after the interprocedural step and `fixednarrow`, before
//! `algebraic::Differences` and `window`: nothing after it chooses a loop's
//! counters. It is in `-O1` and up with `-fstrength-reduce`; at `-Omax` the
//! search has no `Bounds` (gcc's values apply otherwise). See
//! [`crate::pipeline`] and `docs/reference/levels.md`.
//!
//! # Needs
//! `induction` (recurrences, counted exits, guards), `Registers`, `Pressure`
//! (`crate::spill`), `Products`, [`crate::counting`] and [`crate::expand`]
//! to build starts and guards, [`crate::rotate`] to enter at the body.
//!
//! # Keeps
//! Nothing: a changed loop invalidates every analysis.
//!
//! # Limits
//! A loop tested on a value other than a counter plus a constant (a scaled
//! compare) keeps a counter for the compare. Past the target's registers the
//! pressure estimate is coarse. Which value is the base and which the index
//! of a two-register address is the backend's choice, and it decides some
//! outcomes.
//!
//! # See also
//! [`crate::indvars`] keeps the exit values, exit sinking and `rewound`: which
//! counters the loop keeps is this pass's. LLVM:
//! [`LoopStrengthReduce.cpp`](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp).
//! gcc:
//! [`tree-ssa-loop-ivopts.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/tree-ssa-loop-ivopts.cc).
//! `LLRM_DEBUG=lsr` prints each loop's uses, candidates and costs.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, CountedLoop, IvUse, Recurrence, Scev, UseKind, Users};
use llrm_analysis::manager::Registers;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::{cfg, liveness, memory};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Analysis, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_mir::target::{AddressForm, Machine, OperationCosts};
use llrm_mir::types::{Type, TypeId};
use llrm_support::hash::HashMap;
use num_traits::ToPrimitive;

use crate::counting::{self, Seeds};
use crate::expand::{self, Expander};
use crate::spill::{self, Room, Traffic};
use crate::{dead, profit, rotate};

/// `size`: code size outranks speed, and a loop that has another way out is
/// left as it is.
#[derive(Default)]
pub struct Lsr {
    pub size: bool,
    /// gcc's ivopts limits (`Bounds::GCC`); -Omax searches without them.
    pub bounds: Bounds,
}

impl FunctionPass for Lsr {
    fn name(&self) -> &'static str {
        "lsr"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if reduced(unit, analyses, &outer, self.size, self.bounds) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

#[cfg(test)]
thread_local! {
    /// `total` asked and `total_of` worked out, for the test that a set is
    /// priced once.
    pub static TOTALS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };

}

/// What the target says a loop's choice may cost.
struct Target<'a> {
    machine: &'a dyn Machine,
    costs: OperationCosts,
    room: Room,
    forms: Vec<AddressForm>,
    bounds: Bounds,
}

/// gcc's ivopts parameters (tree-ssa-loop-ivopts.cc), and what each does past
/// its bound.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    /// `iv-max-considered-uses`: a loop of more groups of uses than this is
    /// left as it is.
    pub groups: usize,
    /// `iv-consider-all-candidates-bound`: below this many candidates the
    /// search is whole. gcc prices a use from the important candidates and
    /// its own only past it; here that left the cheapest shared counter out and
    /// cost more (QCport weapons.c).
    pub all_candidates: usize,
    /// `iv-always-prune-cand-set-bound`: a candidate serving more uses than
    /// this is not replaced, and replacing is tried only where no one
    /// candidate added or removed lowers the cost, once (`iv_ca_replace`).
    pub always_prune: usize,
}

impl Default for Bounds {
    fn default() -> Self {
        Self::NONE
    }
}

impl Bounds {
    pub const GCC: Self = Self { groups: 250, all_candidates: 40, always_prune: 10 };
    pub const NONE: Self = Self { groups: usize::MAX, all_candidates: usize::MAX, always_prune: usize::MAX };
}

/// What may take a register in a loop: a value it has, or one a choice
/// adds -- a counter, an invariant, a symbolic step, a product, a value
/// built from a counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Resident {
    Value(ValueId),
    Counter(usize),
    Held(usize),
    Step(usize),
    Product(usize, i64, i64),
    Rebuilt(usize),
}

/// What `Resident`s are held in while spill prices are worked out: a bit each
/// for those named by a number, the products (named by three) in a list. A hash
/// set of them was 13% of lsr on d_faces.
#[derive(Default)]
struct Residents {
    named: llrm_mir::dense::IdSet<Packed>,
    products: Vec<(usize, i64, i64)>,
}

#[derive(Clone, Copy)]
struct Packed(usize);

impl llrm_mir::dense::Dense for Packed {
    fn index(self) -> usize {
        self.0
    }
    fn at(index: usize) -> Self {
        Self(index)
    }
}

impl Residents {
    /// The bit of a resident named by a number, its kind in the low three bits.
    fn packed(one: &Resident) -> Option<Packed> {
        Some(Packed(match *one {
            Resident::Value(value) => value.0 as usize * 8,
            Resident::Counter(at) => at * 8 + 1,
            Resident::Held(at) => at * 8 + 2,
            Resident::Step(at) => at * 8 + 3,
            Resident::Rebuilt(at) => at * 8 + 4,
            Resident::Product(..) => return None,
        }))
    }
}

impl spill::Spilled<Resident> for Residents {
    fn has(
        &self,
        cell: &Resident,
    ) -> bool {
        match (Self::packed(cell), cell) {
            (Some(at), _) => self.named.contains(&at),
            (None, Resident::Product(one, k, block)) => self.products.contains(&(*one, *k, *block)),
            _ => unreachable!("a product has no number"),
        }
    }
    fn add(
        &mut self,
        cell: Resident,
    ) {
        match (Self::packed(&cell), cell) {
            (Some(at), _) => {
                self.named.insert(at);
            }
            (None, Resident::Product(one, k, block)) => self.products.push((one, k, block)),
            _ => unreachable!("a product has no number"),
        }
    }
}

/// The whole function's block frequencies as lsr's prices read them: a loop's
/// trips multiplied, ten where unproven (#203), a branch's cold arm less. A
/// manager analysis, so a loop lsr looks at and leaves alone does not find them
/// again (16 loops of a nest: a fifth of lsr), and a loop it changes does not
/// either unless the change reaches what they read.
pub struct Products;

/// `Products`' result: the frequencies, and the trips they were worked out
/// under.
#[derive(Debug, PartialEq)]
pub struct Frequencies {
    pub by_block: Option<std::collections::BTreeMap<i64, i64>>,
    trips: llrm_support::hash::IndexMap<i64, i64>,
    /// The odds of each edge `by_block` was worked out from.
    taken: std::collections::BTreeMap<(i64, i64), f64>,
    /// The blocks and their edges these were worked out on.
    edges: Vec<(BlockId, Vec<BlockId>)>,
    /// The cold blocks of the function: a function of its edges and calls,
    /// which `update` leaves as they were, so not found again for each loop
    /// lsr changes (a nest of N loops scanned the function N times).
    cold: std::collections::BTreeSet<i64>,
}

thread_local! {
    static PRODUCTS_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread worked the frequencies out from scratch, for a
/// test that a loop changed does not make the next loop's start over.
pub fn products_runs() -> usize {
    PRODUCTS_RUNS.with(std::cell::Cell::get)
}

impl Analysis for Products {
    type Result = Frequencies;
    const NAME: &'static str = "loop-products";
    const INCREMENTAL: bool = true;

    fn run(
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self::Result {
        PRODUCTS_RUNS.with(|runs| runs.set(runs.get() + 1));
        let registers = analyses.get::<Registers>(context, layout, function);
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(context, layout, function);
        let outer = std::rc::Rc::clone(analyses.outer());
        // The trips are the manager's proofs, renewed for the loops a change
        // reached, not worked out again for every loop.
        let counted = analyses.get::<llrm_analysis::manager::Counted>(context, layout, function);
        let view = memory::Unit::within(context, layout, function, &outer)
            .with_registers(&registers)
            .with_shape(&shape)
            .with_counted(&counted);
        let trips = profit::proven_trips(&view);
        let (by_block, taken) =
            profit::_products_with_odds_in(view.context, view.metadata, &outer.globals, function, &shape, Some(&trips))
                .map_or((None, Default::default()), |(by_block, taken)| (Some(by_block), taken));
        let cold = llrm_analysis::branchprob::cold_blocks(context, &outer.globals, function);
        Frequencies { by_block, trips, taken, edges: _graph(function), cold }
    }

    /// They are worked out from the blocks and their edges, the loops, the
    /// trips proven and the odds of each branch's edges, which read a call in
    /// a block, the weights it declares and the compare it tests with what
    /// that reads. A change that leaves these as they were (a counter another
    /// counter was chosen for, an address strength-reduced) leaves them: only
    /// the branches that read what it touched are weighed again.
    fn update(
        previous: &Self::Result,
        changes: &[llrm_mir::module::Change],
        context: &Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Option<Self::Result> {
        if _graph(function) != previous.edges {
            return None;
        }
        let reach = function.reach_of(changes)?;
        if reach.cfg {
            return None;
        }
        if previous.by_block.is_none() || changes.iter().any(|one| _calls(function, one)) {
            return None;
        }
        let registers = analyses.get::<Registers>(context, layout, function);
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(context, layout, function);
        let outer = std::rc::Rc::clone(analyses.outer());
        let counted = analyses.get::<llrm_analysis::manager::Counted>(context, layout, function);
        let view = memory::Unit::within(context, layout, function, &outer)
            .with_registers(&registers)
            .with_shape(&shape)
            .with_counted(&counted);
        let trips = profit::proven_trips(&view);
        if trips != previous.trips {
            return None;
        }
        let weighed = reach.conditions.iter().collect::<Vec<_>>();
        let by_header = profit::counted_by_header(&shape, Some(&trips))?;
        let taken = llrm_analysis::branchprob::taken_cold(
            view.context,
            view.metadata,
            &outer.globals,
            function,
            &shape,
            &by_header,
            &previous.cold,
            &weighed,
        );
        let wanted = weighed.iter().map(|&at| cfg::id(at)).collect::<BTreeSet<_>>();
        let same = previous.taken.iter().filter(|((from, _), _)| wanted.contains(from)).eq(taken.iter());
        same.then(|| Frequencies {
            by_block: previous.by_block.clone(),
            trips,
            taken: previous.taken.clone(),
            edges: previous.edges.clone(),
            cold: previous.cold.clone(),
        })
    }
}

/// Whether `change` touched a call: what a branch's weights read of a block.
fn _calls(
    function: &Function,
    change: &llrm_mir::module::Change,
) -> bool {
    use llrm_mir::module::Change;
    let inst = match *change {
        Change::Inserted { inst, .. }
        | Change::Erased { inst, .. }
        | Change::Moved { inst, .. }
        | Change::Rewritten(inst) => inst,
        Change::Cloned { to, .. } => to,
        // What an operand was is told before the `Rewritten` that decides.
        Change::Operand { .. } | Change::Operands { .. } | Change::Substituted { .. } => return false,
        Change::BlockCreated(_) | Change::BlockErased(_) | Change::Parameters => return true,
    };
    matches!(
        function.instruction(inst).opcode,
        Opcode::Call(_) | Opcode::Invoke(_)
    )
}

/// What the function as it stands gives every loop, each worked out when first
/// asked: the instructions' traffic, the frame objects, the far views.
#[derive(Default)]
struct Whole {
    traffic: Option<std::rc::Rc<spill::TrafficBase>>,
    frames: Option<std::rc::Rc<IdSet<ValueId>>>,
    views: Option<std::rc::Rc<IdSet<ValueId>>>,
}

/// Each loop's counters chosen, innermost first; what the changes left, none
/// where nothing changed.
pub fn reduced(
    unit: &mut Unit,
    analyses: &mut Analyses,
    outer: &Outer,
    size: bool,
    bounds: Bounds,
) -> bool {
    let target = Target {
        machine: outer.target(),
        costs: profit::costs(outer),
        room: profit::registers(outer),
        forms: outer.target().address_forms(),
        bounds,
    };
    let mut done = BTreeSet::<i64>::new();
    let mut changed = false;
    // Whether the function is known to hold no dead code: after a loop's dead
    // code is gone.
    let mut clean = false;
    // What is true of the function as it stands, worked out once: each loop
    // reads from it (and takes its own instructions out of the traffic).
    let mut whole = Whole::default();
    // What is known of the function is the manager's, kept until a loop is
    // changed (and then it is `applied` that declares nothing kept).
    loop {
        let plan = {
            let facts = analyses.get::<Registers>(unit.context, unit.layout, unit.function);
            let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
            let view = memory::Unit::within(unit.context, unit.layout, unit.function, outer)
                .with_registers(&facts)
                .with_shape(&shape);
            let mut loops = shape.loops.clone();
            loops.sort_by_key(|one| (one.body.len(), one.header));
            let Some(loop_) = loops.into_iter().find(|one| !done.contains(&one.header)) else { break };
            done.insert(loop_.header);
            if let Some(rewrite) = _post_incremented(&view, &loop_).or_else(|| _stepped_before_test(&view, &loop_)) {
                drop(facts);
                _read_after_step(unit, rewrite);
                clean = false;
                analyses.invalidate(unit.function, &PreservedAnalyses::none());
                whole = Whole::default();
                changed = true;
                done.remove(&loop_.header);
                continue;
            }
            // A counter some trips step has no candidate (it is no function of
            // the trip): where the target's address takes no scale, the pointer
            // that moves with it is made first, and the loop chosen again.
            if !size && let Some(plan) = crate::mirror::found(&view, &loop_, &target.forms) {
                drop(facts);
                crate::mirror::applied(unit.context, unit.function, &plan);
                dead::dead(unit.context, outer.callees(), unit.function);
                analyses.invalidate(unit.function, &PreservedAnalyses::none());
                whole = Whole::default();
                changed = true;
                done.remove(&loop_.header);
                continue;
            }
            let pressure = analyses.get::<spill::Pressure>(unit.context, unit.layout, unit.function);
            let (context, layout, function) = (&*unit.context, unit.layout, &*unit.function);
            let mut products = || analyses.get::<Products>(context, layout, function);
            _plan(&view, outer, &loop_, &target, &pressure, &mut products, &mut whole)
        };
        let Some(plan) = plan else { continue };
        // A loop with another way out was never counted before: its choice is
        // held to the function's whole price, work and spill, as
        // hoist's is, where the loop's own model has been wrong about the
        // registers a call leaves it. Where size outranks speed it
        // stays as it is: the setup is code, and the trips that repay it are
        // an estimate.
        let leaves = plan.exit.as_ref().is_some_and(|(exit, _)| exit.proof.leaves);
        if leaves && size {
            continue;
        }
        let saved = leaves.then(|| (unit.function.clone(), _function_price(unit, &*analyses, outer, &target)));
        let graph = _graph(unit.function);
        let before = unit.function.mark();
        if let Some(first) = _applied(unit, &plan) {
            done.insert(cfg::id(first));
        }
        analyses.invalidate(unit.function, &_kept_graph(unit.function, &graph));
        whole = Whole::default();
        if let Some((before, Some(kept))) = saved {
            dead::dead(unit.context, outer.callees(), unit.function);
            let moved_price = _function_price(unit, &*analyses, outer, &target);
            llrm_support::debug!("lsr", "leaving loop: kept {kept}, moved {moved_price:?}");
            if moved_price.is_some_and(|moved| moved > kept) {
                *unit.function = before;
                continue;
            }
        }
        // What the loop no longer reads is no use for the loop around it. Once
        // a loop is changed the function holds no dead code, so what
        // can have died is what the edit stopped reading; the first is
        // found in the whole.
        match unit.function.touched_since(before).filter(|_| clean) {
            Some(touched) => {
                let checking = llrm_support::env_flag!("LLRM_CHECK_DEAD_FROM");
                let full = checking.then(|| {
                    let mut copy = unit.function.clone();
                    dead::dead(unit.context, outer.callees(), &mut copy);
                    llrm_mir::print::body(unit.context, &copy)
                });
                dead::dead_from(unit.context, outer.callees(), unit.function, touched.named.iter());
                if let Some(full) = full {
                    assert_eq!(
                        full,
                        llrm_mir::print::body(unit.context, unit.function),
                        "LLRM_CHECK_DEAD_FROM: the dead code found from the edit differs from the whole function's"
                    );
                }
            }
            None => {
                dead::dead(unit.context, outer.callees(), unit.function);
            }
        }
        clean = true;
        changed = true;
    }
    changed
}

/// The blocks of `function` and where each goes.
fn _graph(function: &Function) -> Vec<(BlockId, Vec<BlockId>)> {
    function.layout().iter().map(|&block| (block, function.successors(block).into())).collect()
}

/// What an edit that left the blocks and their edges as `before` had them
/// leaves true: the analyses that read the graph and nothing else. A loop
/// counted onto another counter changes no edge, and a nest of N loops derived
/// the dominance and loops of the whole function N times.
fn _kept_graph(
    function: &Function,
    before: &[(BlockId, Vec<BlockId>)],
) -> PreservedAnalyses {
    if _graph(function) == before {
        PreservedAnalyses::none().preserve::<cfg::Shape>().preserve::<Dominators>().preserve::<Loops>()
    } else {
        PreservedAnalyses::none()
    }
}

/// What the function costs as it stands: `profit::motion_price` at the
/// frequencies lsr's own prices use.
fn _function_price(
    unit: &Unit,
    analyses: &Analyses,
    outer: &Outer,
    target: &Target,
) -> Option<i64> {
    let mut fresh = analyses.fresh();
    let facts = fresh.get::<Registers>(unit.context, unit.layout, unit.function);
    let shape = fresh.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
    let view =
        memory::Unit::within(unit.context, unit.layout, unit.function, outer).with_registers(&facts).with_shape(&shape);
    let frequencies = profit::_loop_products_by_branch_in(
        view.context,
        view.metadata,
        &outer.globals,
        unit.function,
        &shape,
        Some(&profit::proven_trips(&view)),
    )?;
    profit::motion_price(unit.context, unit.layout, outer, unit.function, &target.costs, target.room, &frequencies)
}

/// A recurrence the loop could carry, and the phi that already does.
#[derive(Clone, Debug)]
struct Candidate {
    of: Recurrence,
    /// A pointer's type; an integer is its width's.
    pointer: Option<TypeId>,
    existing: Option<ValueId>,
}

/// An invariant a realization keeps in a register: a pointer plus a sum.
type Key = (Option<Operand>, Scev);

/// A register an address reads.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Reg {
    Iv(usize),
    /// A candidate scaled, once in a block.
    Product(usize, i64, i64),
    Held(usize),
}

/// A use read as `base + rest + k * candidate + constant`.
#[derive(Clone, Debug)]
struct Fit {
    k: BigInt,
    base: Option<Operand>,
    rest: Scev,
    constant: BigInt,
    /// The candidate's stepped value, read in the latch.
    next: bool,
    /// After the loop, the trip itself: the candidate's distance from its
    /// start, taken down where it counts down, shifted right by `shift` and
    /// times `inverse`, the inverse of its step's odd part modulo the bits
    /// left. `k` then multiplies the trip.
    trip: Option<(u32, bool, BigInt)>,
    /// An equality with an invariant, which takes the candidate itself and
    /// the invariant less the rest, times `k`, one or minus one.
    folded: Option<Scev>,
}

/// What realizing a use from one candidate costs each time it runs, and
/// what it holds.
#[derive(Clone, Debug)]
struct Price {
    cost: i64,
    /// Invariants held in registers, by their index in `Problem::keys`.
    held: Vec<usize>,
    /// The registers an address in the native form reads.
    address: Vec<Reg>,
    /// An address in a wider form.
    wide: bool,
    /// `k * candidate` computed in the loop, in `cost`: one serves every
    /// use of it in the block, whose index is the last.
    product: Option<(i64, i64, i64)>,
}

/// A use, where its realization goes and how often that runs.
#[derive(Clone, Debug)]
struct Site {
    one: IvUse,
    at: Place,
    frequency: i64,
    inside: bool,
    /// The counted exit's compare, which may test a candidate's last value
    /// instead.
    exit: bool,
    /// Its value, where it is read as the loop leaves on a known count.
    known: Option<Scev>,
}

/// Where a realization is placed.
#[derive(Clone, Copy, Debug)]
enum Place {
    Before(InstId),
    End(BlockId),
    /// After the phis of an exit block, for the exit phi it replaces.
    Exit(BlockId, InstId),
}

/// How the exit may be tested.
#[derive(Clone, Debug)]
struct Exit {
    proof: CountedLoop,
    trips: Scev,
    most: BigInt,
    /// A symbolic count needs a guard, and the loop entered at its body.
    guarded: bool,
    /// The test is, or is made, the loop's last block: a loop with other exits
    /// is entered at its body only by a guard here.
    fused: bool,
}

/// One loop's choice, before anything changes.
struct Plan {
    loop_: Loop,
    preheader: BlockId,
    header: BlockId,
    latch: BlockId,
    candidates: Vec<Candidate>,
    /// Each use, its candidate and its fit.
    uses: Vec<(Site, usize, Fit)>,
    /// The candidate whose last value ends the loop, and that value.
    exit: Option<(Exit, usize)>,
}

/// Everything priced in one loop.
struct Problem<'a> {
    target: &'a Target<'a>,
    candidates: Vec<Candidate>,
    /// What each candidate's step costs a trip.
    steps: Vec<i64>,
    sites: Vec<Site>,
    /// Each site's fit and price from each candidate.
    fits: Vec<Vec<Option<(Fit, Price)>>>,
    exit: Option<Exit>,
    /// The exit's price from each candidate, testing its last value.
    exits: Vec<Option<(i64, Option<usize>)>>,
    keys: Vec<Key>,
    /// What the loop keeps in registers before each of its instructions,
    /// and across each call, whatever is chosen: its blocks as a sweep takes
    /// them.
    run: spill::Run,
    /// The blocks of `run`, in order, and each one's place in it.
    order: Vec<i64>,
    at: BTreeMap<i64, usize>,
    #[cfg(test)]
    fixed: BTreeMap<i64, Vec<spill::Site>>,
    /// The spill traffic of what `run` keeps, priced for the values asked.
    traffic: spill::TrafficLess,
    function: &'a Function,
    words: Box<dyn Fn(ValueId) -> i64 + 'a>,
    /// `total` of each set asked: the search asks the same set again from the
    /// other start and from each step's neighbours.
    totals: std::cell::RefCell<llrm_support::hash::HashMap<BTreeSet<usize>, Option<i64>>>,
    /// Where each site's value is live, at the points of `run`: the half-open
    /// stretches of each block's points, in order.
    lives: Vec<BTreeMap<i64, Vec<(usize, usize)>>>,
    /// The same point by point, for the test that holds the sweep to it.
    #[cfg(test)]
    alive: Vec<BTreeMap<i64, Vec<bool>>>,
    latch: i64,
    header: i64,
    /// How often the preheader runs, where starts and invariants are built.
    entry: i64,
    /// How often each block runs a trip.
    frequencies: BTreeMap<i64, i64>,
    /// Values the loop holds anyway: a key of one alone is no new register.
    live: BTreeSet<ValueId>,
    /// Frame objects: their addresses are the frame's register and a
    /// displacement.
    frames: std::rc::Rc<IdSet<ValueId>>,
    /// Far views of a segment: held in a segment register, not a general one.
    views: std::rc::Rc<IdSet<ValueId>>,
}

fn _preheader(
    function: &Function,
    loop_: &Loop,
) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let outside = function
        .predecessors(header)
        .into_iter()
        .filter(|&one| !loop_.body.contains(&cfg::id(one)))
        .collect::<Vec<_>>();
    match outside[..] {
        [one] if function.successors(one) == [header] => Some(one),
        _ => None,
    }
}

fn _plan(
    view: &memory::Unit,
    outer: &Outer,
    loop_: &Loop,
    target: &Target,
    pressure: &spill::Pressure,
    products: &mut dyn FnMut() -> std::rc::Rc<Frequencies>,
    whole: &mut Whole,
) -> Option<Plan> {
    let function = view.function;
    let preheader = _preheader(function, loop_)?;
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let counters = induction::basics(view, loop_);
    let users = induction::users(view, loop_, &counters);
    if users.counters.is_empty()
        || users.counters.iter().any(|&one| _latch_arm(function, one, cfg::block(latch)).is_none())
    {
        return None;
    }
    // lsr's prices were fitted to trips multiplied, ten where unproven (#203),
    // and a branch's cold arm less.
    let held = products();
    let frequencies = held.by_block.as_ref()?;
    let frequency = |block: BlockId| frequencies.get(&cfg::id(block)).copied().unwrap_or(1);
    let exit = _exit(view, loop_, &users);
    let nested = view
        .shape()
        .loops
        .iter()
        .filter(|one| one.header != loop_.header && loop_.body.contains(&one.header))
        .cloned()
        .collect::<Vec<_>>();
    let mut sites = Vec::new();
    for one in &users.uses {
        let (at, inside) = _site(function, loop_, &nested, one)?;
        let block = match at {
            Place::Before(inst) => function.parent(inst)?,
            Place::End(block) | Place::Exit(block, _) => block,
        };
        let exit = exit.as_ref().is_some_and(|exit| exit.proof.compare == one.user);
        sites.push(Site { one: one.clone(), at, frequency: frequency(block), inside, exit, known: None });
    }
    // Exit phis that read one recurrence are realized once after the loop;
    // one reading several is realized on each way out.
    let mut exits = BTreeMap::<InstId, Vec<usize>>::new();
    for (index, site) in sites.iter().enumerate() {
        if let Place::Exit(_, phi) = site.at {
            exits.entry(phi).or_default().push(index);
        }
    }
    for (phi, indexes) in &exits {
        let operands = function.instruction(*phi).operands.chunks(2).count();
        let same = indexes.iter().all(|&index| sites[index].one.of == sites[indexes[0]].one.of);
        if !same || indexes.len() != operands {
            for &index in indexes {
                let from = match function.instruction(*phi).operands[sites[index].one.index + 1] {
                    Operand::Block(from) => from,
                    _ => return None,
                };
                sites[index].at = Place::End(from);
                sites[index].frequency = frequency(from);
                sites[index].inside = true;
            }
        } else {
            for &index in &indexes[1..] {
                sites[index].frequency = 0;
            }
        }
    }
    // Read as the loop leaves its header on a known count, a value is its
    // start plus its step each trip.
    if let Some(exit) = exit.as_ref().filter(|exit| !exit.proof.posttested && exit.trips.terms.is_empty()) {
        for site in &mut sites {
            let Place::Exit(block, phi) = site.at else { continue };
            let from_header = function
                .instruction(phi)
                .operands
                .chunks(2)
                .all(|pair| pair[1] == Operand::Block(cfg::block(loop_.header)));
            if block == cfg::block(exit.proof.exit) && from_header {
                let of = _normal(view, &site.one);
                site.known = of.step.product(&exit.trips.truncated(of.width())).map(|walked| of.start.plus(&walked));
            }
        }
    }
    // gcc leaves a loop of more groups of uses than `iv-max-considered-uses` as
    // it is; a group is the uses of one step and base that differ in a
    // constant.
    let groups = sites
        .iter()
        .map(|site| {
            let of = _normal(view, &site.one);
            (of.pointer.is_some(), of.step, of.start.symbolic())
        })
        .collect::<BTreeSet<_>>();
    if groups.len() > target.bounds.groups {
        return None;
    }
    let candidates = _candidates(view, target, &users, &sites, exit.as_ref());
    let web_values = users.values.keys().copied().collect::<BTreeSet<_>>();
    let live = _live_anyway(function, pressure.found(), loop_, &users, exit.as_ref());
    let cells = pressure.cells();
    let run = _run(view, outer, loop_, target.room, pressure, &web_values, &users, exit.as_ref(), &live);
    #[cfg(test)]
    let fixed = _fixed(view, outer, loop_, target.room, pressure, &web_values, &users, exit.as_ref(), &live);
    let order: Vec<i64> = loop_.body.iter().copied().collect();
    let at = order.iter().enumerate().map(|(position, &block)| (block, position)).collect();
    // The web's reads are the uses the choice replaces; each use adds its own
    // back.
    let base = whole.traffic.get_or_insert_with(|| {
        std::rc::Rc::new(spill::TrafficBase::of(function, &frequencies, &|value| cells.get(&value).copied(), &|_| true))
    });
    let traffic = base.without(function, &frequencies, &|value| cells.get(&value).copied(), users.web.iter().copied());
    let alive = _alive(function, pressure, loop_, &sites);
    let lives =
        alive.iter().map(|site| site.iter().map(|(&block, points)| (block, _stretches(points))).collect()).collect();
    let mut keys = Vec::new();
    let latch_block = cfg::block(latch);
    // The most backedges: the counted exit's, or what an in-bounds access
    // allows.
    let most = exit.as_ref().map(|exit| exit.most.clone()).or_else(|| induction::inbounds_backedges(view, loop_));
    let frames = std::rc::Rc::clone(whole.frames.get_or_insert_with(|| std::rc::Rc::new(_frames(view.function))));
    let fits = sites
        .iter()
        .map(|site| {
            candidates
                .iter()
                .enumerate()
                .map(|(index, one)| {
                    _priced(view, target, &frames, site, index, one, latch_block, most.as_ref(), &mut keys)
                })
                .collect()
        })
        .collect();
    let exits = candidates
        .iter()
        .map(|one| {
            exit.as_ref()
                .filter(|exit| _steps_before_test(function, &exit.proof, one) && _comparable(view, one))
                .and_then(|exit| _exit_price(target, exit, one, &mut keys))
        })
        .collect();
    let steps = candidates.iter().map(|one| _step(view, target, one)).collect();
    let problem = Problem {
        target,
        candidates,
        steps,
        sites,
        fits,
        exit,
        exits,
        keys,
        run,
        order,
        at,
        #[cfg(test)]
        fixed,
        traffic,
        function,
        words: Box::new(|value| spill::words(view.context, view.layout, function, value)),
        totals: Default::default(),
        lives,
        #[cfg(test)]
        alive,
        latch: frequency(cfg::block(latch)),
        header: frequency(cfg::block(loop_.header)),
        entry: frequency(preheader),
        frequencies: frequencies.clone(),
        live,
        frames,
        views: std::rc::Rc::clone(whole.views.get_or_insert_with(|| std::rc::Rc::new(_views(view)))),
    };
    let current = problem
        .candidates
        .iter()
        .enumerate()
        .filter(|(_, one)| one.existing.is_some())
        .map(|(index, _)| index)
        .collect::<BTreeSet<_>>();
    let before = problem.total_of(&current, true);
    let chosen = problem.solved(&current);
    let after = chosen.as_ref().and_then(|chosen| problem.total(chosen));
    llrm_support::debug!(
        "lsr",
        "loop b{}: {} uses, {} candidates, exit {}; {:?} costs {:?}, {:?} costs {:?}",
        loop_.header,
        problem.sites.len(),
        problem.candidates.len(),
        problem.exit.as_ref().map_or("none".to_owned(), |exit| format!("{:?} trips", exit.trips)),
        current,
        before,
        chosen,
        after
    );
    if llrm_support::debug::enabled("lsr") {
        for (label, set) in [("standing", Some(current.clone())), ("chosen", chosen.clone())] {
            let Some(set) = set else { continue };
            for index in 0..problem.sites.len() {
                if let Some((Some((one, fit, price)), _)) = problem.choice(&set, index) {
                    llrm_support::debug!(
                        "lsr",
                        "  {label} site {index}: from {one}, cost {} x{}, k {}, held {:?}, product {:?}",
                        price.cost,
                        problem.sites[index].frequency,
                        fit.k,
                        price.held,
                        price.product
                    );
                } else {
                    llrm_support::debug!("lsr", "  {label} site {index}: exit-tested");
                }
            }
        }
        for (at, site) in problem.sites.iter().enumerate() {
            let op = function.instruction(site.one.user);
            llrm_support::debug!(
                "lsr",
                "  site {at}: {:?} operand {} of {:?} {:?}, {:?} + {:?}*t, x{}",
                site.one.kind,
                site.one.index,
                op.opcode,
                op.result,
                site.one.of.start,
                site.one.of.step.known(),
                site.frequency
            );
        }
        for (index, one) in problem.candidates.iter().enumerate() {
            let alone = BTreeSet::from([index]);
            let sites = (0..problem.sites.len())
                .map(|at| problem.fits[at][index].as_ref().map(|(_, price)| price.cost))
                .collect::<Vec<_>>();
            llrm_support::debug!(
                "lsr",
                "  {index}: {:?} + {:?}*t ptr {:?} existing {:?}: alone {:?}, exit {:?}, sites {:?}",
                one.of.start,
                one.of.step.known(),
                one.of.pointer,
                one.existing,
                problem.total(&alone),
                problem.exits[index],
                sites
            );
        }
    }
    let (chosen, after) = (chosen?, after?);
    if before.is_some_and(|before| after >= before) {
        return None;
    }
    let (uses, exit) = problem.assigned(&chosen)?;
    let kept = chosen.iter().copied().collect::<Vec<_>>();
    let renumber = |index: usize| kept.iter().position(|&one| one == index).expect("a chosen candidate");
    let Problem { candidates, exit: exit_proof, .. } = problem;
    let uses = uses.into_iter().map(|(site, index, fit)| (site, renumber(index), fit)).collect::<Vec<_>>();
    Some(Plan {
        loop_: loop_.clone(),
        preheader,
        header: cfg::block(loop_.header),
        latch: cfg::block(latch),
        candidates: kept.iter().map(|&index| candidates[index].clone()).collect(),
        uses,
        exit: exit.and_then(|index| Some((exit_proof?, renumber(index)))),
    })
}

/// Where a use's realization goes, and whether that is inside the loop:
/// before the user, at the end of the block a phi reads it from, out of
/// any loop inside this one, or after the phis of an exit.
fn _site(
    function: &Function,
    loop_: &Loop,
    nested: &[Loop],
    one: &IvUse,
) -> Option<(Place, bool)> {
    let op = function.instruction(one.user);
    let block = function.parent(one.user)?;
    let inside = |block: BlockId| loop_.body.contains(&cfg::id(block));
    if op.opcode == Opcode::Phi {
        let Operand::Block(from) = op.operands[one.index + 1] else { return None };
        if inside(block) {
            return Some((_hoisted(function, nested, Place::End(from)), true));
        }
        // An exit phi: every way into its block leaves the loop.
        if inside(from) && function.predecessors(block).iter().all(|&pred| inside(pred)) {
            return Some((Place::Exit(block, one.user), false));
        }
        return None;
    }
    // A reader after the loop that no exit phi carries is dominated by what
    // it reads, so reads it from the trip that left, as the header's
    // counters then are.
    Some((_hoisted(function, nested, Place::Before(one.user)), inside(block)))
}

/// `at`, moved out of every loop inside this one to its preheader.
fn _hoisted(
    function: &Function,
    nested: &[Loop],
    at: Place,
) -> Place {
    let block = match at {
        Place::Before(inst) => function.parent(inst).expect("a placed user"),
        Place::End(block) | Place::Exit(block, _) => block,
    };
    let outermost = nested.iter().filter(|one| one.body.contains(&cfg::id(block))).max_by_key(|one| one.body.len());
    match outermost.and_then(|one| _preheader(function, one)) {
        Some(preheader) => Place::End(preheader),
        None => at,
    }
}

/// A latch test of the counter before its step, which the step is made for
/// anyway: the compare to rewrite, the step it reads instead and the bound a
/// step on. LLVM's `OptimizeLoopTermCond` makes the same of a compare whose
/// operand is the counter, where the step is live: the test reads the stepped
/// counter and the counter itself is dead after the step, one register for two.
struct PostIncrement {
    compare: InstId,
    counter: ValueId,
    step: ValueId,
    ty: TypeId,
    bound: i128,
}

fn _post_incremented(
    view: &memory::Unit,
    loop_: &Loop,
) -> Option<PostIncrement> {
    let function = view.function;
    // Cheap first: a latch branch on a compare of a header phi with a constant.
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    let branch = function.terminator(cfg::block(latch))?;
    let Some(&Operand::Value(condition)) = function.instruction(branch).operands.first() else { return None };
    let ValueDef::Instruction(compare) = function.value(condition).def else { return None };
    if !matches!(function.instruction(compare).opcode, Opcode::ICmp(_)) {
        return None;
    }
    let operands = &function.instruction(compare).operands;
    let ((Operand::Value(read), Operand::Constant(_)) | (Operand::Constant(_), Operand::Value(read))) =
        (operands[0], operands[1])
    else {
        return None;
    };
    let ValueDef::Instruction(phi) = function.value(read).def else { return None };
    if function.instruction(phi).opcode != Opcode::Phi || function.parent(phi) != Some(cfg::block(loop_.header)) {
        return None;
    }
    let Some(Operand::Value(step)) = _latch_arm(function, read, cfg::block(latch)) else { return None };
    let proofs = induction::counted_leaving(view, loop_, None, true);
    let [proof] = &proofs[..] else { return None };
    if !proof.shifted || !proof.posttested || proof.compare != compare {
        return None;
    }
    let counter = read;
    let ty = function.value(counter).ty;
    let width = proof.width();
    let moved = match &proof.bound {
        induction::AffineOperand::Const(known) => {
            let modulus = BigInt::from(1) << width;
            let low = ((&known.n % &modulus) + &modulus) % &modulus;
            let signed = if low >= (BigInt::from(1) << (width - 1)) { low - modulus } else { low };
            signed.to_i128()?
        }
        induction::AffineOperand::Value(..) => return None,
    };
    Some(PostIncrement { compare: proof.compare, counter, step, ty, bound: moved })
}

/// A test of the counter that leaves the loop, made after the step the
/// counter's other use is: the compare reads the step and the bound a step on,
/// so the counter is dead once stepped. gcc's ivopts takes a use after the
/// increment from the incremented candidate, whichever exit it is; here a
/// loop entered behind its copied tests leaves from its header, and the
/// stepped value and the counter were two registers (`lea cx, [si+1]; cmp
/// si, 31; ...; mov si, cx`). Where an ordered test would wrap the step's
/// flags say so, as `icmp (add nsw x, k), c` is `icmp x, c - k`.
fn _stepped_before_test(
    view: &memory::Unit,
    loop_: &Loop,
) -> Option<PostIncrement> {
    let function = view.function;
    let [latch] = loop_.latches.iter().copied().collect::<Vec<_>>()[..] else { return None };
    for &at in &loop_.body {
        let block = cfg::block(at);
        let branch = function.terminator(block)?;
        let instruction = function.instruction(branch);
        if instruction.operands.len() != 3
            || !function.successors(block).iter().any(|one| !loop_.body.contains(&cfg::id(*one)))
        {
            continue;
        }
        let Operand::Value(condition) = instruction.operands[0] else { continue };
        let ValueDef::Instruction(compare) = function.value(condition).def else { continue };
        let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { continue };
        let operands = &function.instruction(compare).operands;
        let ((Operand::Value(read), Operand::Constant(bound)) | (Operand::Constant(bound), Operand::Value(read))) =
            (operands[0], operands[1])
        else {
            continue;
        };
        let ValueDef::Instruction(phi) = function.value(read).def else { continue };
        if function.instruction(phi).opcode != Opcode::Phi || function.parent(phi) != Some(cfg::block(loop_.header)) {
            continue;
        }
        let Some(Operand::Value(step)) = _latch_arm(function, read, cfg::block(latch)) else { continue };
        let ValueDef::Instruction(made) = function.value(step).def else { continue };
        let stepper = function.instruction(made);
        let width = view.int_bits(Operand::Value(read))?;
        if width > 64 {
            continue;
        }
        let signed = |bits: u128| {
            let low = bits & ((1u128 << width) - 1);
            if low >> (width - 1) == 1 { low as i128 - (1i128 << width) } else { low as i128 }
        };
        let by = match (&stepper.opcode, &stepper.operands[..]) {
            (Opcode::Binary(BinaryOp::Add), [Operand::Value(one), k @ Operand::Constant(_)])
            | (Opcode::Binary(BinaryOp::Add), [k @ Operand::Constant(_), Operand::Value(one)])
                if *one == read =>
            {
                signed(view.int_constant(*k)?)
            }
            (Opcode::Binary(BinaryOp::Sub), [Operand::Value(one), k @ Operand::Constant(_)]) if *one == read => {
                -signed(view.int_constant(*k)?)
            }
            _ => continue,
        };
        // The step is made before the compare, on every path to it.
        let (stepped_in, compared_in) = (function.parent(made)?, block);
        let before = if stepped_in == compared_in {
            let all = function.block(block);
            all.iter().position(|one| one == made)? < all.iter().position(|one| one == compare)?
        } else {
            view.shape().dominance.dominates(function, cfg::id(stepped_in), at)
        };
        // The counter is read by the step, the compare and its phi alone.
        let readers = function.users(read);
        if !before || readers.iter().any(|one| one.user != made && one.user != compare && one.user != phi) {
            continue;
        }
        let flags = llrm_mir::facts::Facts::of_flags(stepper.flags);
        let ty = function.value(read).ty;
        let equality = matches!(predicate, IntPredicate::Eq | IntPredicate::Ne);
        let unsigned =
            matches!(
                predicate,
                IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge
            );
        if (!equality && !unsigned && !flags.no_signed_wrap()) || (unsigned && !flags.no_unsigned_wrap()) {
            continue;
        }
        let known = view.int_constant(Operand::Constant(bound))?;
        let (known, modulus) =
            (if unsigned { (known & ((1u128 << width) - 1)) as i128 } else { signed(known) }, 1i128 << width);
        let moved = known + by;
        let (low, high) = if unsigned { (0, modulus - 1) } else { (-(modulus >> 1), (modulus >> 1) - 1) };
        let moved = if equality { signed(moved.rem_euclid(modulus) as u128) } else { moved };
        if !equality && (moved < low || moved > high) {
            continue;
        }
        return Some(PostIncrement { compare, counter: read, step, ty, bound: moved });
    }
    None
}

/// `rewrite`: the compare reads the step and the bound a step on.
fn _read_after_step(
    unit: &mut Unit,
    rewrite: PostIncrement,
) {
    let constant = Operand::Constant(unit.context.int(rewrite.ty, rewrite.bound));
    let operands = unit.function.instruction(rewrite.compare).operands.clone();
    for (at, operand) in operands.iter().enumerate() {
        match operand {
            Operand::Value(value) if *value == rewrite.counter => {
                unit.function.set_operand(rewrite.compare, at, Operand::Value(rewrite.step));
            }
            Operand::Constant(_) => unit.function.set_operand(rewrite.compare, at, constant),
            _ => {}
        }
    }
}

/// The loop's counted exit, where its compare is read only by its branch.
fn _exit(
    view: &memory::Unit,
    loop_: &Loop,
    users: &Users,
) -> Option<Exit> {
    let function = view.function;
    // The count holds whenever the loop goes on: where another exit stops
    // the program, nothing after it reads what the counters were.
    let proofs = induction::counted_leaving(view, loop_, None, true);
    let [proof] = &proofs[..] else { return None };
    let result = function.instruction(proof.compare).result?;
    if function.users(result).iter().any(|one| one.user != proof.branch) || users.web.contains(&proof.compare) {
        return None;
    }
    if proof.posttested && !proof.stepped {
        return None;
    }
    let trips = proof.trips_linear()?;
    let most = proof.most()?;
    // A symbolic count needs a guard against no trip, unless a branch over
    // the loop's entry already proves it runs one.
    let guarded = !trips.terms.is_empty() && !proof.entry_guarded;
    if guarded && (proof.posttested || rotate::_shape(function, loop_).is_none()) {
        return None;
    }
    let fused = proof.posttested || guarded || !proof.leaves;
    Some(Exit { proof: proof.clone(), trips, most, guarded, fused })
}

/// Whether `candidate`'s step is made before `proof`'s test where the test
/// reads it: a new counter's step is put there; an existing one's stands.
fn _steps_before_test(
    function: &Function,
    proof: &CountedLoop,
    candidate: &Candidate,
) -> bool {
    let Some(phi) = candidate.existing.filter(|_| proof.posttested) else { return true };
    let Some(Operand::Value(step)) = _latch_arm(function, phi, cfg::block(proof.latch)) else { return false };
    let ValueDef::Instruction(made) = function.value(step).def else { return false };
    let (here, there) = (function.parent(made), function.parent(proof.branch));
    here == there
        && function.block(here.expect("a placed step")).iter().position(|one| one == made)
            < function.block(there.expect("a placed branch")).iter().position(|one| one == proof.branch)
        || here != there
            && here.is_some_and(|step| {
                function
                    .parent(proof.branch)
                    .is_some_and(
                        |test| cfg::Shape::of(function).dominance.dominates(function, cfg::id(step), cfg::id(test)),
                    )
            })
}

/// Values read inside the loop by something other than a recurrence or
/// the counted exit, or live after it.
fn _live_anyway(
    function: &Function,
    found: &liveness::Liveness,
    loop_: &Loop,
    users: &Users,
    exit: Option<&Exit>,
) -> BTreeSet<ValueId> {
    let mut live = BTreeSet::new();
    for &at in &loop_.body {
        for inst in function.block(cfg::block(at)).iter() {
            if users.web.contains(&inst) || exit.is_some_and(|exit| exit.proof.compare == inst) {
                continue;
            }
            live.extend(function.instruction(inst).operands.iter().filter_map(|one| match one {
                Operand::Value(value) => Some(*value),
                _ => None,
            }));
        }
    }
    for &at in &loop_.body {
        for succ in function.successors(cfg::block(at)) {
            if !loop_.body.contains(&cfg::id(succ)) {
                live.extend(found.live_in.get(&cfg::id(succ)).into_iter().flat_map(|set| set.iter()));
            }
        }
    }
    live
}

/// The symbols of every recurrence the loop reads.
fn _symbols(
    users: &Users,
    exit: Option<&Exit>,
) -> BTreeSet<ValueId> {
    let mut symbols = BTreeSet::new();
    for of in users.values.values() {
        symbols.extend(of.start.unknowns().chain(of.step.unknowns()));
        if let Some(Operand::Value(pointer)) = of.pointer {
            symbols.insert(pointer);
        }
    }
    if let Some(exit) = exit {
        symbols.extend(exit.trips.unknowns());
        if let induction::AffineOperand::Value(bound, _) = exit.proof.bound {
            symbols.insert(bound);
        }
    }
    symbols
}

/// What the loop keeps in registers besides its counters and the
/// invariants only its recurrences read, at each instruction.
#[allow(clippy::too_many_arguments)]
fn _run(
    view: &memory::Unit,
    outer: &Outer,
    loop_: &Loop,
    room: Room,
    pressure: &spill::Pressure,
    web: &BTreeSet<ValueId>,
    users: &Users,
    exit: Option<&Exit>,
    live: &BTreeSet<ValueId>,
) -> spill::Run {
    let function = view.function;
    let symbols = _symbols(users, exit);
    let across = |inst: InstId| spill::kept_across(outer, view.context, function, inst);
    let model = spill::View::over(pressure, view.context, view.layout, function, room, &across);
    let hide = |value: ValueId| web.contains(&value) || (symbols.contains(&value) && !live.contains(&value));
    let blocks = loop_.body.iter().map(|&at| cfg::block(at)).collect::<Vec<_>>();
    model.run(&blocks, &hide)
}

/// What the loop keeps in registers besides its counters and the
/// invariants only its recurrences read, at each instruction.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn _fixed(
    view: &memory::Unit,
    outer: &Outer,
    loop_: &Loop,
    room: Room,
    pressure: &spill::Pressure,
    web: &BTreeSet<ValueId>,
    users: &Users,
    exit: Option<&Exit>,
    live: &BTreeSet<ValueId>,
) -> BTreeMap<i64, Vec<spill::Site>> {
    let function = view.function;
    let symbols = _symbols(users, exit);
    let across = |inst: InstId| spill::kept_across(outer, view.context, function, inst);
    let model = spill::View::over(pressure, view.context, view.layout, function, room, &across);
    let hide = |value: ValueId| web.contains(&value) || (symbols.contains(&value) && !live.contains(&value));
    loop_.body.iter().map(|&at| (at, model.sites(cfg::block(at), &hide))).collect()
}

/// The half-open stretches of `true` in `points`.
fn _stretches(points: &[bool]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, &live) in points.iter().enumerate() {
        match (live, start) {
            (true, None) => start = Some(at),
            (false, Some(from)) => {
                found.push((from, at));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        found.push((from, points.len()));
    }
    found
}

/// Where each site's value is live in the loop, before each instruction.
fn _alive(
    function: &Function,
    pressure: &spill::Pressure,
    loop_: &Loop,
    sites: &[Site],
) -> Vec<BTreeMap<i64, Vec<bool>>> {
    // What is live before each instruction of a block is the block's, not the
    // site's: worked out once, not once a site (22 sites of a loop took a
    // fifth of lsr on d_faces).
    let live: Vec<_> = loop_
        .body
        .iter()
        .map(|&at| {
            #[cfg(test)]
            LIVE_POINTS.with(|count| count.set(count.get() + 1));
            (at, pressure.points(function, cfg::block(at)))
        })
        .collect();
    sites
        .iter()
        .map(|site| {
            live.iter()
                .map(|(at, points)| (*at, points.iter().map(|point| point.before.contains(&site.one.value)).collect()))
                .collect()
        })
        .collect()
}

#[cfg(test)]
thread_local! {
    /// Block liveness walks made by `_alive`, for a test that they do not grow
    /// with the sites.
    pub(crate) static LIVE_POINTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What a trip pays to advance `one`: an add, or a pointer's own advance.
fn _step(
    view: &memory::Unit,
    target: &Target,
    one: &Candidate,
) -> i64 {
    one.pointer.map_or(target.costs.add, |ty| {
        profit::advance(view.context, view.layout, ty, target.costs.add, one.of.step.known().is_some(), &target.costs)
    })
}

/// The candidates: each use's own recurrence, less its symbols and its
/// constant; the loop's counters; and each step counted to zero at the exit.
fn _candidates(
    view: &memory::Unit,
    target: &Target,
    users: &Users,
    sites: &[Site],
    exit: Option<&Exit>,
) -> Vec<Candidate> {
    let function = view.function;
    let pointer_type = |value: ValueId| {
        let ty = function.value(value).ty;
        matches!(view.context.types.get(ty), Type::Pointer(_)).then_some(ty)
    };
    let mut found = Vec::<Candidate>::new();
    let add = |found: &mut Vec<Candidate>, of: Recurrence, pointer: Option<TypeId>, existing: Option<ValueId>| {
        if of.step.is_zero() {
            return;
        }
        match found.iter_mut().find(|one| one.of == of) {
            Some(one) => one.existing = one.existing.or(existing),
            None => found.push(Candidate { of, pointer, existing }),
        }
    };
    for &counter in &users.counters {
        add(&mut found, users.values[&counter].clone(), pointer_type(counter), Some(counter));
    }
    let mut steps = BTreeSet::<Scev>::new();
    for site in sites {
        let of = _normal(view, &site.one);
        steps.insert(of.step.clone());
        let bare = Recurrence { pointer: None, start: of.start.clone(), step: of.step.clone() };
        // The start's symbols split every way between the counter and the
        // base, with its constant or without: LLVM's reassociation.
        let terms = of.start.terms.iter().collect::<Vec<_>>();
        if terms.len() <= _SPLIT_TERMS {
            for mask in 0..1_usize << terms.len() {
                let part = terms
                    .iter()
                    .enumerate()
                    .filter(|(bit, _)| mask >> bit & 1 == 1)
                    .map(|(_, (value, factor))| ((*value).clone(), (*factor).clone()))
                    .collect();
                let symbolic = Scev { constant: BigInt::from(0), terms: part, width: of.width() };
                add(&mut found, Recurrence { start: symbolic.clone(), ..bare.clone() }, None, None);
                // An address takes the constant as its displacement: a
                // candidate for each constant is a candidate
                // per copy of an unrolled body.
                if site.one.kind != UseKind::Address {
                    add(
                        &mut found,
                        Recurrence {
                            start: symbolic.plus(&Scev::constant(of.start.constant.clone(), of.width())),
                            ..bare.clone()
                        },
                        None,
                        None,
                    );
                }
            }
        } else {
            add(&mut found, Recurrence { start: Scev::constant(0, of.width()), ..bare.clone() }, None, None);
            add(
                &mut found,
                Recurrence { start: Scev::constant(of.start.constant.clone(), of.width()), ..bare.clone() },
                None,
                None,
            );
        }
        match of.pointer {
            Some(_) => {
                let pointer = pointer_type(site.one.value);
                if pointer.is_some() {
                    if site.one.kind != UseKind::Address {
                        add(&mut found, of.clone(), pointer, None);
                    }
                    add(&mut found, Recurrence { start: of.start.symbolic(), ..of.clone() }, pointer, None);
                }
            }
            None => add(&mut found, bare, None, None),
        }
    }
    // An address form whose index is wider takes a counter of its width,
    // stepping by the address's step over one of its scales.
    for site in sites.iter().filter(|site| site.one.kind == UseKind::Address) {
        let of = _normal(view, &site.one);
        let Some(bytes) = of.step.known() else { continue };
        for form in target.forms.iter().filter(|form| form.index_width * 8 > i64::from(of.width())) {
            let width = u32::try_from(form.index_width * 8).expect("an index width");
            for &scale in &form.scales {
                if &bytes % scale != BigInt::from(0) {
                    continue;
                }
                let step = Scev::constant(&bytes / scale, width);
                steps.insert(step.clone());
                add(&mut found, Recurrence { pointer: None, start: Scev::constant(0, width), step }, None, None);
            }
        }
    }
    // Each step, counted to zero at the exit.
    if let Some(exit) = exit {
        for step in &steps {
            // A symbolic count is of its own width only.
            if step.width != exit.trips.width && !exit.trips.terms.is_empty() {
                continue;
            }
            let trips = exit.trips.truncated(step.width).times(&BigInt::from(-1));
            let Some(start) = trips.product(step) else { continue };
            add(&mut found, Recurrence { pointer: None, start, step: step.clone() }, None, None);
        }
    }
    found
}

/// The most symbols of a start split between counter and base, every way.
const _SPLIT_TERMS: usize = 3;

/// The function's frame objects, whose addresses need no register of their own.
fn _frames(function: &Function) -> IdSet<ValueId> {
    function
        .walk()
        .filter(|&(_, inst)| matches!(function.instruction(inst).opcode, Opcode::Alloca { .. }))
        .filter_map(|(_, inst)| function.instruction(inst).result)
        .collect()
}

/// The function's far views of a segment, which the segment registers hold.
fn _views(view: &memory::Unit) -> IdSet<ValueId> {
    let function = view.function;
    function
        .walk()
        .filter_map(|(_, inst)| function.instruction(inst).result)
        .filter(|&value| spill::segment_view(view.context, view.layout, view.spaces(), function, value))
        .collect()
}

/// How many registers of an address cannot be where it needs them: each
/// address of two is a base and an index, a register is one or the other, and
/// the target has so many of each. The fewest, over every assignment.
fn _misplaced(
    address: &BTreeSet<Reg>,
    pairs: &BTreeSet<(Reg, Reg)>,
    bases: i64,
    indices: i64,
) -> i64 {
    let registers = address.iter().cloned().collect::<Vec<_>>();
    if registers.len() > 16 {
        return 0;
    }
    let at = |reg: &Reg| registers.iter().position(|one| one == reg).expect("an address register");
    let edges = pairs.iter().map(|(one, other)| (at(one), at(other))).collect::<Vec<_>>();
    (0u32..1 << registers.len())
        .map(|base| {
            let bases_used = i64::from(base.count_ones());
            let clashes = edges.iter().filter(|&&(one, other)| (base >> one & 1) == (base >> other & 1)).count() as i64;
            clashes + (bases_used - bases).max(0) + ((registers.len() as i64 - bases_used) - indices).max(0)
        })
        .min()
        .unwrap_or(0)
}

/// What building `key` before the loop costs: its scaled terms, the adds
/// that join its parts, and an address off its pointer.
fn _built(
    target: &Target,
    key: &Key,
) -> i64 {
    let costs = &target.costs;
    let (pointer, sum) = key;
    let mut cost = sum
        .terms
        .values()
        .map(|factor| _scaling(target, &BigInt::from(expand::signed(factor, sum.width).magnitude().clone())))
        .sum::<i64>();
    let parts = sum.terms.len()
        + usize::from(sum.constant != BigInt::from(0))
        + usize::from(pointer.is_some() && !sum.is_zero());
    if sum.terms.is_empty() && pointer.is_none() {
        return 0;
    }
    cost += parts.saturating_sub(1) as i64 * costs.add;
    cost
}

/// A use's recurrence in its own width: an address's in its index's.
fn _normal(
    view: &memory::Unit,
    one: &IvUse,
) -> Recurrence {
    match one.of.pointer {
        Some(pointer) => match view.space(pointer) {
            Some(space) => one.of.truncated(view.layout.pointer(space).index_bits),
            None => one.of.clone(),
        },
        None => one.of.clone(),
    }
}

/// `site` as `base + rest + k * candidate + constant`.
fn _fit(
    view: &memory::Unit,
    site: &Site,
    candidate: &Candidate,
    most: Option<&BigInt>,
) -> Option<Fit> {
    let of = _normal(view, &site.one);
    // A wider counter indexes an address by its low bits.
    let narrowed;
    let candidate =
        if candidate.of.width() > of.width() && site.one.kind == UseKind::Address && candidate.of.pointer.is_none() {
            narrowed = Candidate { of: candidate.of.truncated(of.width()), ..candidate.clone() };
            &narrowed
        } else {
            candidate
        };
    if of.width() != candidate.of.width() {
        return None;
    }
    if let Some(known) = &site.known {
        let constant = known.known().unwrap_or_else(|| expand::signed(&known.constant, known.width));
        return Some(Fit {
            k: BigInt::from(0),
            base: of.pointer,
            rest: known.symbolic(),
            constant,
            next: false,
            trip: None,
            folded: None,
        });
    }
    let Some(k) = of.step.over(&candidate.of.step) else { return _trip_fit(site, &of, candidate, most) };
    let (base, rest) = match (candidate.of.pointer, of.pointer) {
        (Some(one), Some(other)) if one == other && k == BigInt::from(1) => (None, of.start.minus(&candidate.of.start)),
        (Some(_), _) => return None,
        (None, pointer) => (pointer, of.start.minus(&candidate.of.start.times(&k))),
    };
    let mut constant = rest.known().unwrap_or_else(|| expand::signed(&rest.constant, rest.width));
    let mut rest = rest.symbolic();
    // What its reader cannot see need not be added: a multiple of the bits
    // it observes.
    let seen = BigInt::from(1) << site.one.demanded.min(rest.width);
    if base.is_none() && site.one.demanded < rest.width {
        rest.terms.retain(|_, factor| &*factor % &seen != BigInt::from(0));
        if &constant % &seen == BigInt::from(0) {
            constant = BigInt::from(0);
        }
    }
    Some(Fit { k, base, rest, constant, next: false, trip: None, folded: None })
}

/// After the loop, `site` from the trip the candidate has counted: its
/// distance from its start divided exactly by its step, which holds while
/// the loop's trips stay below the step's period.
fn _trip_fit(
    site: &Site,
    of: &Recurrence,
    candidate: &Candidate,
    most: Option<&BigInt>,
) -> Option<Fit> {
    let step = candidate.of.step.known()?;
    let k = of.step.known()?;
    if site.inside || candidate.of.pointer.is_some() || step == BigInt::from(0) {
        return None;
    }
    let magnitude = BigInt::from(step.magnitude().clone());
    let shift = u32::try_from(magnitude.trailing_zeros()?).ok()?;
    let bits = candidate.of.width().checked_sub(shift)?;
    if most? >= &(BigInt::from(1) << bits) {
        return None;
    }
    let inverse = _inverse(&(&magnitude >> shift), bits);
    let constant = of.start.known().unwrap_or_else(|| expand::signed(&of.start.constant, of.start.width));
    Some(Fit {
        k,
        base: of.pointer,
        rest: of.start.symbolic(),
        constant,
        next: false,
        trip: Some((shift, step < BigInt::from(0), inverse)),
        folded: None,
    })
}

/// The inverse of odd `n` modulo `2^bits`, by Newton's iteration.
fn _inverse(
    n: &BigInt,
    bits: u32,
) -> BigInt {
    let modulus = BigInt::from(1) << bits;
    let mut x = n.clone();
    for _ in 0..7 {
        x = (&x * (BigInt::from(2) - n * &x)) % &modulus;
        if x < BigInt::from(0) {
            x += &modulus;
        }
    }
    x
}

/// What `phi`, a counter, takes from `latch`.
fn _latch_arm(
    function: &Function,
    phi: ValueId,
    latch: BlockId,
) -> Option<Operand> {
    let ValueDef::Instruction(inst) = function.value(phi).def else { return None };
    function.instruction(inst).operands.chunks(2).find(|pair| pair[1] == Operand::Block(latch)).map(|pair| pair[0])
}

/// What `k * r` costs a trip: nothing, a negation, a shift or a multiply. On a
/// two-address target the candidate stays live, so a negation or a shift is
/// made in a copy of it, unless `lea` scales it.
fn _scaling(
    target: &Target,
    k: &BigInt,
) -> i64 {
    let costs = &target.costs;
    let magnitude = BigInt::from(k.magnitude().clone());
    let copy = if target.room.two_address && !_leas(target, k) { costs.r#move } else { 0 };
    if *k == BigInt::from(1) || *k == BigInt::from(0) {
        0
    } else if *k == BigInt::from(-1) {
        costs.add + copy
    } else if (&magnitude & (&magnitude - 1)) == BigInt::from(0) {
        costs.shift + copy + if *k < BigInt::from(0) { costs.add } else { 0 }
    } else {
        let multiply = magnitude.to_i64().map_or(costs.multiply, |factor| target.machine.multiply_by(factor));
        multiply + if *k < BigInt::from(0) { costs.add } else { 0 }
    }
}

/// What `k * r` costs where a use adds `rest`, and whether the use is `rest -
/// r`: one `sub` from a copy of `rest` (the copy is the use's, as `made`
/// charges it), with no negation of `r` and no product to hold. Priced as a
/// negation, a copy and an add, it made a counter that rebuilds `c - r` look
/// dearer than it is (#721: Nib -Os queens, 34% in the model, tied in the
/// code).
fn _scaled(
    target: &Target,
    k: &BigInt,
    added: bool,
) -> (i64, bool) {
    let subtracted = added && *k == BigInt::from(-1);
    (if subtracted { 0 } else { _scaling(target, k) }, subtracted)
}

/// Whether one `lea` makes `k * r + rest + constant`: where any register may be
/// a base and an index (`[bx+si]` is no `lea` of `ax`), and its scales hold
/// `k`.
fn _leas(
    target: &Target,
    k: &BigInt,
) -> bool {
    target
        .forms
        .iter()
        .any(
            |form| form.bases.is_none()
                && form.indices.is_none()
                && (*k == BigInt::from(1) || k.to_i64().is_some_and(|scale| scale > 1 && form.scales.contains(&scale))),
        )
}

/// `site`'s price from `candidate`.
/// `key`'s index in `keys`, added where new.
fn _interned(
    keys: &mut Vec<Key>,
    key: Key,
) -> usize {
    match keys.iter().position(|one| *one == key) {
        Some(index) => index,
        None => {
            keys.push(key);
            keys.len() - 1
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn _priced(
    view: &memory::Unit,
    target: &Target,
    frames: &IdSet<ValueId>,
    site: &Site,
    index: usize,
    candidate: &Candidate,
    latch: BlockId,
    most: Option<&BigInt>,
    keys: &mut Vec<Key>,
) -> Option<(Fit, Price)> {
    let mut fit = _fit(view, site, candidate, most)?;
    // The candidate plus its step, in the latch, is the step itself: a new
    // one is placed before its first reader, a counter's must be what is read.
    let in_latch = match site.at {
        Place::Before(inst) => view.function.parent(inst) == Some(latch),
        Place::End(block) => block == latch,
        Place::Exit(..) => false,
    };
    let stepped = fit.k == BigInt::from(1)
        && fit.rest.is_zero()
        && candidate.of.step.known().is_some_and(|step| step == fit.constant)
        && candidate.of.pointer.is_none()
        && site.one.of.pointer.is_none();
    let own = match candidate.existing {
        Some(existing) => _latch_arm(view.function, existing, latch) == Some(Operand::Value(site.one.value)),
        None => true,
    };
    // A counter's own step is read where it stands.
    if (in_latch || candidate.existing.is_some()) && stepped && own && site.one.kind != UseKind::Address {
        fit.next = true;
        // A step's own flags answer its equality with zero.
        let op = view.function.instruction(site.one.user);
        let zero = matches!(op.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne))
            && view.int_constant(op.operands[1 - site.one.index]) == Some(0);
        let cost = if site.one.kind == UseKind::Compare && !zero { target.costs.add } else { 0 };
        let mut held = Vec::new();
        if site.one.kind == UseKind::Compare
            && let Operand::Value(other) = view.function.instruction(site.one.user).operands[1 - site.one.index]
        {
            let width = view.int_bits(Operand::Value(other)).unwrap_or(fit.rest.width);
            held.push(_interned(keys, (None, Scev::of(&induction::AffineOperand::Value(other, width), width))));
        }
        return Some((fit, Price { cost, held, address: Vec::new(), wide: false, product: None }));
    }
    let costs = &target.costs;
    // A constant pointer is a displacement; any other base a register.
    let pointer_base = |fit: &Fit| {
        matches!(
            fit.base,
            Some(Operand::Value(value)) if !frames.contains(&value)
        ) || !fit.rest.is_zero()
    };
    let mut price = Price { cost: 0, held: Vec::new(), address: Vec::new(), wide: false, product: None };
    let block = cfg::id(match site.at {
        Place::Before(inst) => view.function.parent(inst)?,
        Place::End(block) | Place::Exit(block, _) => block,
    });
    let small = |k: &BigInt| k.to_string().parse::<i64>().unwrap_or(i64::MAX);
    match site.one.kind {
        UseKind::Address if candidate.of.pointer.is_some() => {
            price.address.push(Reg::Iv(index));
            if !fit.rest.is_zero() {
                let key = _interned(keys, (None, fit.rest.clone()));
                price.held.push(key);
                price.address.push(Reg::Held(key));
            }
        }
        UseKind::Address => {
            let native = target.forms.first()?;
            // An integer cannot index a pair in a carrying space: the use
            // advances the pointer by it, and pays what an advance
            // there costs over an address's.
            let advance = profit::advance(
                view.context,
                view.layout,
                view.function.value(site.one.value).ty,
                costs.address,
                false,
                costs,
            );
            price.cost += advance - costs.address;
            if pointer_base(&fit) {
                // A global is a displacement beside two registers; a frame
                // object takes BP for its own, and leaves one
                // register for all it adds.
                let held = fit.base.filter(|base| matches!(base, Operand::Value(_)));
                let key = _interned(keys, (held, fit.rest.clone()));
                price.held.push(key);
                price.address.push(Reg::Held(key));
            }
            if fit.k == BigInt::from(1) {
                price.address.push(Reg::Iv(index));
                price.cost += native.use_cost;
            } else if fit.k != BigInt::from(0) {
                let scale = fit.k.to_string().parse::<i64>().unwrap_or(0);
                let wider = candidate.of.width() > fit.rest.width;
                let nonnegative = wider
                    || candidate.of.start.known().is_some_and(|start| start >= BigInt::from(0))
                        && candidate.of.step.known().is_some_and(|step| step > BigInt::from(0));
                // A form that scales takes the index at its own width: a
                // narrower one, zero-extended, must not go negative, and
                // pays its extension each time it changes.
                let width = i64::from(candidate.of.width()) / 8;
                // A dword index is truncated to the word the address adds: the
                // selector scales it only where its range keeps it in one.
                let bounded = width < 4
                    || most.zip(candidate.of.step.known()).is_some_and(|(most, step)| {
                        let magnitude = |one: &BigInt| BigInt::from(one.magnitude().clone());
                        let start = candidate.of.start.known().map_or(BigInt::from(0), |start| magnitude(&start));
                        (most * magnitude(&step) + start) * BigInt::from(scale.unsigned_abs()) <= BigInt::from(i16::MAX)
                    });
                let scaled = target
                    .forms
                    .iter()
                    .filter(|form| {
                        form.scales.contains(&scale)
                            && (form.index_width == width && bounded || nonnegative && form.index_width > width)
                    })
                    .map(|form| form.use_cost + if form.index_width > width { form.extension_cost } else { 0 })
                    .min();
                let computed = _scaling(target, &fit.k) + native.use_cost;
                match scaled {
                    Some(cost) if cost < computed => {
                        price.cost += cost;
                        price.wide = true;
                        price.address.clear();
                    }
                    _ => {
                        price.cost += computed;
                        price.address.push(Reg::Product(index, small(&fit.k), block));
                        price.product = Some((small(&fit.k), _scaling(target, &fit.k), block));
                    }
                }
            }
        }
        UseKind::Compare
            if fit.trip.is_none()
                && fit.base.is_none()
                && candidate.of.pointer.is_none()
                && (fit.k == BigInt::from(1) || fit.k == BigInt::from(-1))
                && {
                    let op = view.function.instruction(site.one.user);
                    matches!(op.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne))
                } =>
        {
            // `k * r + rest == w` is `r == k * (w - rest)`: the invariant
            // absorbs the rest.
            let op = view.function.instruction(site.one.user);
            let other = op.operands[1 - site.one.index];
            let width = fit.rest.width;
            let invariant = match view.int_constant(other) {
                Some(bits) => Scev::constant(BigInt::from(bits), width),
                None => Scev::of(&induction::term(view, other)?, width),
            };
            let folded = invariant.minus(&fit.rest).minus(&Scev::constant(fit.constant.clone(), width)).times(&fit.k);
            if !folded.terms.is_empty() {
                price.held.push(_interned(keys, (None, folded.clone())));
            }
            price.cost += costs.add;
            fit.folded = Some(folded);
        }
        UseKind::Compare | UseKind::Basic => {
            if let Some((shift, _, inverse)) = &fit.trip {
                price.cost += costs.add
                    + if *shift != 0 { costs.shift } else { 0 }
                    + if *inverse != BigInt::from(1) { costs.multiply + costs.add } else { 0 };
            }
            let pointer = candidate.of.pointer.is_some();
            let symbolic = !fit.rest.is_zero() || matches!(fit.base, Some(Operand::Value(_)));
            let (scaling, subtracted) =
                _scaled(target, &fit.k, fit.trip.is_none() && (symbolic || fit.constant != BigInt::from(0)));
            price.cost += scaling;
            if fit.trip.is_none() && scaling != 0 {
                price.product = Some((small(&fit.k), scaling, block));
            }
            // A pointer made from an integer, in a carrying space, carries into
            // its selector. A two-address target makes a result in
            // its first operand's register: the candidate stays
            // live, as a counter does, so a form no address takes (a negation,
            // a general scale) is copied first. `lea` makes `cand *
            // k + rest + constant` in one, where its scales hold `k`.
            // (A scaled one is made in a copy already, which the add then makes
            // its result in.)
            let copy = if target.room.two_address && (subtracted || fit.k == BigInt::from(1) && !_leas(target, &fit.k))
            {
                costs.r#move
            } else {
                0
            };
            let made = |constant| {
                let ty = view.function.value(site.one.value).ty;
                copy + if pointer {
                    costs.add
                } else {
                    profit::advance(view.context, view.layout, ty, costs.add, constant, costs)
                }
            };
            if symbolic {
                let whole = fit.rest.plus(&Scev::constant(fit.constant.clone(), fit.rest.width));
                price.held.push(_interned(keys, (if pointer { None } else { fit.base }, whole)));
                price.cost += made(false);
            } else if fit.constant != BigInt::from(0) || fit.base.is_some() {
                price.cost += made(fit.k == BigInt::from(0));
            }
            if site.one.kind == UseKind::Compare {
                price.cost += costs.add;
                let op = view.function.instruction(site.one.user);
                if let Operand::Value(other) = op.operands[1 - site.one.index] {
                    let width = view.int_bits(Operand::Value(other)).unwrap_or(fit.rest.width);
                    price
                        .held
                        .push(_interned(keys, (None, Scev::of(&induction::AffineOperand::Value(other, width), width))));
                }
            }
        }
    }
    Some((fit, price))
}

/// Whether the exit can test `candidate` by one compare: not a pointer
/// wider than its offset, a far one, which the selector has no compare for.
fn _comparable(
    view: &memory::Unit,
    candidate: &Candidate,
) -> bool {
    candidate
        .pointer
        .is_none_or(
            |ty| match view.context.types.get(ty) {
                Type::Pointer(space) => {
                    let spec = view.layout.pointer(*space);
                    spec.bits <= spec.index_bits
                }
                _ => true,
            },
        )
}

/// The exit's price testing `candidate` for its last value: a compare,
/// none where that is zero, and a register where it is symbolic.
fn _exit_price(
    target: &Target,
    exit: &Exit,
    candidate: &Candidate,
    keys: &mut Vec<Key>,
) -> Option<(i64, Option<usize>)> {
    let step = candidate.of.step.known()?;
    if candidate.of.width() < exit.trips.width || candidate.of.step.width != candidate.of.width() {
        return None;
    }
    // A symbolic count is of its own width only.
    if candidate.of.width() != exit.trips.width && !exit.trips.terms.is_empty() {
        return None;
    }
    let modulus = BigInt::from(1) << candidate.of.width();
    let period = &modulus / induction::gcd(BigInt::from(step.magnitude().clone()), modulus.clone());
    // Tested before a trip, the value must not come round in `most`; tested
    // after one, as a guarded loop entered at its body is, it may just return.
    let after = exit.guarded || exit.proof.posttested;
    if exit.most > period || (!after && exit.most == period) {
        return None;
    }
    let end = _end(exit, candidate);
    // Free only where the step's flags are the test's: the backend fuses a zero
    // test with the step that precedes it along a single path: a test at
    // the latch, not one at the head of a loop that has other exits and no
    // guard to enter at its body.
    if end.is_zero() && candidate.of.pointer.is_none() && exit.fused {
        return Some((0, None));
    }
    let key =
        (!end.terms.is_empty() || candidate.of.pointer.is_some()).then(|| _interned(keys, (candidate.of.pointer, end)));
    Some((target.costs.add, key))
}

/// `candidate`'s value as the loop leaves: its start plus its step each trip.
fn _end(
    exit: &Exit,
    candidate: &Candidate,
) -> Scev {
    let trips = exit.trips.truncated(candidate.of.width());
    candidate.of.start.plus(&trips.product(&candidate.of.step).expect("a constant step"))
}

impl Problem<'_> {
    /// What holds invariant `key`: the loop's own value where it names one,
    /// else a new register.
    fn resident(
        &self,
        key: usize,
    ) -> Resident {
        let named = match &self.keys[key] {
            (None, sum) if sum.constant == BigInt::from(0) && sum.terms.len() == 1 => sum
                .terms
                .iter()
                .find(|(_, factor)| **factor == BigInt::from(1))
                .and_then(|(product, _)| product.single()),
            (Some(Operand::Value(value)), sum) if sum.is_zero() => Some(*value),
            _ => None,
        };
        match named {
            Some(value) if self.free(&self.keys[key]) => Resident::Value(value),
            _ => Resident::Held(key),
        }
    }

    /// What keeping `one` in memory costs, read as `reads` says.
    fn spill_price(
        &self,
        one: Resident,
        reads: &BTreeMap<Resident, i64>,
    ) -> i64 {
        let read = reads.get(&one).copied().unwrap_or(0);
        let traffic = match one {
            Resident::Value(value) => {
                let kept = self.traffic.of(self.function, &self.target.costs, &*self.words, value);
                Traffic { loads: kept.loads + read, ..kept }
            }
            // Stepped in place each trip; a new one's start stored on entry.
            Resident::Counter(at) => Traffic {
                stores: if self.candidates[at].existing.is_none() { self.entry } else { 0 },
                updates: self.latch,
                loads: read,
                rebuild: None,
            },
            Resident::Held(_) => Traffic { stores: self.entry, loads: read, ..Traffic::default() },
            Resident::Step(_) => Traffic { stores: self.entry, loads: self.latch, ..Traffic::default() },
            // The code a spilled product becomes: its register copy stored, the
            // multiply or shift made in the cell, and each reader's load.
            Resident::Product(_, _, block) => {
                let made = self.frequencies.get(&block).copied().unwrap_or(1);
                Traffic { stores: made, updates: made, loads: read, rebuild: None }
            }
            Resident::Rebuilt(at) => {
                Traffic { stores: self.sites[at].frequency, loads: self.sites[at].frequency, ..Traffic::default() }
            }
        };
        traffic.price(&self.target.costs)
    }

    /// Each site's cheapest fit among `set`; the exit's candidate where
    /// testing its last value is cheaper than keeping its compare.
    #[allow(clippy::type_complexity)]
    fn assigned(
        &self,
        set: &BTreeSet<usize>,
    ) -> Option<(Vec<(Site, usize, Fit)>, Option<usize>)> {
        let mut uses = Vec::new();
        let mut exit = None;
        for (index, site) in self.sites.iter().enumerate() {
            let (kept, tested) = self.choice(set, index)?;
            if let Some((one, _)) = tested {
                exit = Some(one);
                continue;
            }
            let (one, fit, _) = kept.expect("a fit where nothing else ends the loop");
            uses.push((site.clone(), one, fit.clone()));
        }
        Some((uses, exit))
    }

    /// Site `index`'s cheapest fit among `set`, or for the exit's compare
    /// the candidate whose last value it tests where that is cheaper.
    #[allow(clippy::type_complexity)]
    fn choice(
        &self,
        set: &BTreeSet<usize>,
        index: usize,
    ) -> Option<(Option<(usize, &Fit, &Price)>, Option<(usize, (i64, Option<usize>))>)> {
        let kept = set
            .iter()
            .filter_map(|&one| self.fits[index][one].as_ref().map(|(fit, price)| (one, fit, price)))
            .min_by_key(|(one, _, price)| (price.cost, price.held.len(), *one));
        if !self.sites[index].exit {
            return kept.map(|kept| (Some(kept), None));
        }
        let tested = set
            .iter()
            .filter_map(|&one| self.exits[one].clone().map(|price| (one, price)))
            .min_by_key(|(one, (cost, key))| (*cost, key.is_some(), *one));
        match (kept, tested) {
            (Some(kept), Some(tested))
                if (kept.2.cost, kept.2.held.len()) < (tested.1.0, usize::from(tested.1.1.is_some())) =>
            {
                Some((Some(kept), None))
            }
            (_, Some(tested)) => Some((None, Some(tested))),
            (Some(kept), None) => Some((Some(kept), None)),
            (None, None) => None,
        }
    }

    /// The cost of `set`, where every use has a fit in it.
    fn total(
        &self,
        set: &BTreeSet<usize>,
    ) -> Option<i64> {
        #[cfg(test)]
        TOTALS.with(|counts| counts.set((counts.get().0 + 1, counts.get().1)));
        if let Some(&known) = self.totals.borrow().get(set) {
            return known;
        }
        #[cfg(test)]
        TOTALS.with(|counts| counts.set((counts.get().0, counts.get().1 + 1)));
        let total = self.total_of(set, false);
        self.totals.borrow_mut().insert(set.clone(), total);
        total
    }

    /// What site `index` costs from a counter of `set` that is already
    /// there, extended: the extension is paid by the use, the rest is what the
    /// use needs of a counter of that value at its width.
    fn widened(
        &self,
        set: &BTreeSet<usize>,
        index: usize,
    ) -> Option<i64> {
        let site = &self.sites[index];
        set.iter()
            .filter(|&&one| {
                self.candidates[one].existing.is_some()
                    && self.candidates[one].of.pointer.is_none()
                    && self.candidates[one].of.width() < site.one.of.width()
            })
            .flat_map(|&one| {
                let have = &self.candidates[one].of;
                (0..self.candidates.len()).filter(move |&wide| {
                    let of = &self.candidates[wide].of;
                    of.pointer.is_none()
                        && of.width() == site.one.of.width()
                        && of.start.known().is_some()
                        && of.start.known() == have.start.known()
                        && of.step.known().is_some()
                        && of.step.known() == have.step.known()
                })
            })
            .filter_map(|wide| self.fits[index][wide].as_ref().map(|(_, price)| price.cost))
            .min()
    }

    /// `total`; with `standing`, a wider use no counter of `set` fits costs
    /// what the loop pays for it now, `widened`: the loop as it stands.
    fn total_of(
        &self,
        set: &BTreeSet<usize>,
        standing: bool,
    ) -> Option<i64> {
        if set.is_empty() {
            return None;
        }
        let costs = &self.target.costs;
        let mut cost = set.iter().map(|&one| self.steps[one]).sum::<i64>() * self.latch;
        // A counter wider than the native index pays the operand-size prefix a
        // step.
        let native = self.target.forms.first().map_or(i64::MAX, |form| form.index_width * 8);
        cost += set.iter().filter(|&&one| i64::from(self.candidates[one].of.width()) > native).count() as i64
            * costs.prefix
            * self.latch;
        let mut held = BTreeSet::<usize>::new();
        let mut built = BTreeSet::<usize>::new();
        let mut products = BTreeSet::<(usize, i64, i64)>::new();
        // Where in its block each product is read last.
        let mut last = BTreeMap::<(usize, i64, i64), usize>::new();
        let mut address = BTreeSet::<Reg>::new();
        let mut pairs = BTreeSet::<(Reg, Reg)>::new();
        // Values built from a counter rather than being one.
        let mut rebuilt = Vec::<usize>::new();
        // How often each counter and invariant is read a trip.
        let mut reads = BTreeMap::<Resident, i64>::new();
        // The counters whose step a use in the body reads, which puts the step
        // above the body, and the exit tests that cost nothing only beside it.
        let mut stepped_early = BTreeSet::<usize>::new();
        let mut free_exits = Vec::<(usize, i64)>::new();

        for (index, site) in self.sites.iter().enumerate() {
            let choice = match self.choice(set, index) {
                Some(choice) => choice,
                None if standing && self.widened(set, index).is_some() => {
                    cost += (costs.extend + self.widened(set, index)?) * site.frequency;
                    continue;
                }
                None => return None,
            };
            match choice {
                (_, Some((one, (exit, key)))) => {
                    cost += exit * site.frequency;
                    if exit == 0 {
                        free_exits.push((one, site.frequency));
                    }
                    *reads.entry(Resident::Counter(one)).or_default() += site.frequency;
                    if let Some(key) = key {
                        *reads.entry(self.resident(key)).or_default() += site.frequency;
                    }
                    held.extend(key);
                    built.extend(key);
                }
                (Some((one, fit, price)), None) => {
                    cost += price.cost * site.frequency;
                    if site.inside && fit.next && site.one.kind == UseKind::Basic {
                        stepped_early.insert(one);
                    }
                    if site.inside {
                        if fit.k != BigInt::from(0) {
                            *reads.entry(Resident::Counter(one)).or_default() += site.frequency;
                        }
                        for &key in &price.held {
                            *reads.entry(self.resident(key)).or_default() += site.frequency;
                        }
                    }
                    let product = fit.rest.is_zero() && fit.constant == BigInt::from(0) && fit.base.is_none();
                    if site.inside
                        && site.one.kind == UseKind::Basic
                        && !fit.next
                        && !(product && fit.k == BigInt::from(1))
                    {
                        rebuilt.push(index);
                    }
                    if let Some((k, scaling, block)) = price.product
                        && site.inside
                        && !(product && rebuilt.last() == Some(&index))
                    {
                        if !products.insert((one, k, block)) {
                            cost -= scaling * site.frequency;
                        }
                        let reader = match site.at {
                            Place::Before(inst) => Some(inst),
                            _ => None,
                        };
                        let in_run = self.at.get(&block).copied();
                        let at = in_run
                            .and_then(|run| self.run.insts(run).position(|inst| Some(inst) == reader))
                            .unwrap_or_else(|| in_run.map_or(0, |run| self.run.insts(run).count().saturating_sub(1)));
                        let slot = last.entry((one, k, block)).or_insert(at);
                        *slot = (*slot).max(at);
                        *reads.entry(Resident::Product(one, k, block)).or_default() += site.frequency;
                    }
                    built.extend(price.held.iter().copied());
                    if site.inside {
                        held.extend(price.held.iter().cloned());
                        address.extend(price.address.iter().cloned());
                        if let [one, other] = &price.address[..] {
                            pairs.insert((one.clone().min(other.clone()), one.clone().max(other.clone())));
                        }
                    }
                }
                (None, None) => return None,
            }
        }
        for (one, frequency) in free_exits {
            if stepped_early.contains(&one) {
                cost += costs.add * frequency;
            }
        }
        // Starts, symbolic steps and invariants, built once a way in.
        for &one in set {
            let candidate = &self.candidates[one];
            if candidate.existing.is_none() {
                cost += (_built(self.target, &(candidate.of.pointer, candidate.of.start.clone()))
                    + _built(self.target, &(None, candidate.of.step.clone())))
                    * self.entry;
            }
        }
        cost += built.iter().map(|&key| _built(self.target, &self.keys[key])).sum::<i64>() * self.entry;
        // Counters, their symbolic steps and new invariants live throughout;
        // a product the loop computes, from its block's top to its last
        // reader; a value built from a counter, where it is live.
        let throughout = set
            .iter()
            .map(|&one| Resident::Counter(one))
            .chain(
                set.iter()
                    .filter(|&&one| self.candidates[one].of.step.known().is_none())
                    .map(|&one| Resident::Step(one)),
            )
            .chain(
                held.iter()
                    .filter(|&&key| !self.free(&self.keys[key]) && !self.viewed(key))
                    .map(|&key| Resident::Held(key)),
            )
            .collect::<Vec<_>>();
        // A held far view of a segment is in a segment register, with those
        // live.
        let segmented = held
            .iter()
            .filter(|&&key| !self.free(&self.keys[key]) && self.viewed(key))
            .map(|&key| Resident::Held(key))
            .collect::<Vec<_>>();
        if self.target.room.priced() {
            let room = self.target.room;
            let price = |one: Resident| self.spill_price(one, &reads);
            // What the residents besides the cells are at each step, told as
            // what comes and what goes: a product from the top of
            // its block to its last reader, a rebuilt value where
            // it is live.
            let runs = self.order.len();
            let mut products: Vec<Vec<(usize, Resident)>> = vec![Vec::new(); runs];
            for (&(one, k, block), &read) in &last {
                if let Some(&run) = self.at.get(&block) {
                    products[run].push((read, Resident::Product(one, k, block)));
                }
            }
            let mut lives: Vec<Vec<(usize, bool, Resident)>> = vec![Vec::new(); runs];
            for &at in &rebuilt {
                for (block, stretches) in &self.lives[at] {
                    if let Some(&run) = self.at.get(block) {
                        for &(from, to) in stretches {
                            lives[run].push((from, true, Resident::Rebuilt(at)));
                            lives[run].push((to, false, Resident::Rebuilt(at)));
                        }
                    }
                }
            }
            for list in &mut products {
                list.sort_unstable();
            }
            for list in &mut lives {
                list.sort_by_key(|&(index, on, _)| (index, on));
            }
            let (mut first, mut present, mut reader, mut event) =
                (true, llrm_support::hash::HashSet::<Resident>::default(), 0usize, 0usize);
            let mut extra = |run: usize, index: usize, coming: &mut Vec<Resident>, going: &mut Vec<Resident>| {
                if std::mem::take(&mut first) {
                    coming.extend(throughout.iter().copied());
                }
                let (came, went) = (coming.len(), going.len());
                if index == 0 {
                    going.extend(present.drain());
                    (reader, event) = (0, 0);
                    coming.extend(products[run].iter().map(|&(_, product)| product));
                }
                while let Some(&(_, product)) = products[run].get(reader).filter(|(read, _)| *read < index) {
                    going.push(product);
                    reader += 1;
                }
                while let Some(&(_, on, one)) = lives[run].get(event).filter(|(at, ..)| *at <= index) {
                    if on {
                        coming.push(one);
                    } else {
                        going.push(one);
                    }
                    event += 1;
                }
                if index == 0 {
                    // A resident that lives on past a block's edge neither
                    // goes nor comes.
                    let (was, is): (llrm_support::hash::HashSet<Resident>, llrm_support::hash::HashSet<Resident>) =
                        (going[went..].iter().copied().collect(), coming[came..].iter().copied().collect());
                    present.extend(is.iter().copied());
                    let (mut gone, mut come) = (going.split_off(went), coming.split_off(came));
                    gone.retain(|one| !is.contains(one));
                    come.retain(|one| !was.contains(one));
                    going.extend(gone);
                    coming.extend(come);
                }
                present.extend(coming[came..].iter().copied());
                for one in &going[went..] {
                    present.remove(one);
                }
                // Held to the lists it replaced: at each step, what the loop
                // keeps live besides the cells.
                #[cfg(test)]
                {
                    let block = self.order[run];
                    let mut old: BTreeSet<Resident> = last
                        .iter()
                        .filter(|(product, read)| product.2 == block && **read >= index)
                        .map(|(&(one, k, block), _)| Resident::Product(one, k, block))
                        .collect();
                    old.extend(
                        rebuilt
                            .iter()
                            .filter(|&&at| self.alive[at].get(&block).is_some_and(|alive| alive[index]))
                            .map(|&at| Resident::Rebuilt(at)),
                    );
                    assert_eq!(present.iter().copied().collect::<BTreeSet<_>>(), old, "run {run}, step {index}");
                }
            };
            let swept = self.run.fit::<Resident, Residents>(room, &Resident::Value, &price, &segmented, &mut extra);
            #[cfg(test)]
            assert_eq!(
                swept,
                self.spilled_dense(&throughout, &segmented, &last, &rebuilt, &reads),
                "the sweep over the loop's blocks is not the forecast over its points"
            );
            cost += swept;
        }
        if let Some((bases, indices)) = self.target.forms.first().and_then(AddressForm::register_classes) {
            cost += _misplaced(&address, &pairs, bases, indices) * costs.r#move * self.header;
        }
        Some(cost)
    }

    /// What `total_of` spilled before the sweep: `spilled_in` over the loop's
    /// points, a list of residents at each. The sweep is held to it.
    #[cfg(test)]
    fn spilled_dense(
        &self,
        throughout: &[Resident],
        segmented: &[Resident],
        last: &BTreeMap<(usize, i64, i64), usize>,
        rebuilt: &[usize],
        reads: &BTreeMap<Resident, i64>,
    ) -> i64 {
        let points = self
            .fixed
            .iter()
            .flat_map(
                |(&block, sites)| {
                    let (throughout, last, rebuilt) = (&throughout, &last, &rebuilt);
                    sites
                        .iter()
                        .enumerate()
                        .flat_map(
                            move |(index, site)| {
                                let added = throughout
                                    .iter()
                                    .copied()
                                    .chain(
                                        last.iter()
                                            .filter(|(product, read)| product.2 == block && **read >= index)
                                            .map(|(&(one, k, block), _)| Resident::Product(one, k, block)),
                                    )
                                    .chain(
                                        rebuilt
                                            .iter()
                                            .filter(|&&at| self.alive[at].get(&block).is_some_and(|alive| alive[index]))
                                            .map(|&at| Resident::Rebuilt(at)),
                                    )
                                    .collect::<Vec<_>>();
                                std::iter::once(&site.before)
                                    .chain(&site.across)
                                    .map(
                                        move |point| spill::Point {
                                            registers: point.registers,
                                            residents: point
                                                .residents
                                                .iter()
                                                .map(|&value| Resident::Value(value))
                                                .chain(added.iter().copied())
                                                .collect(),
                                        },
                                    )
                            },
                        )
                },
            );
        let segment_points = self
            .fixed
            .values()
            .flatten()
            .map(
                |site| spill::Point {
                    registers: site.segments.registers,
                    residents: site
                        .segments
                        .residents
                        .iter()
                        .map(|&value| Resident::Value(value))
                        .chain(segmented.iter().copied())
                        .collect(),
                },
            );
        if self.target.room.priced() {
            return spill::spilled_in::<Resident, Residents>(points.chain(segment_points), |one| {
                self.spill_price(one, &reads)
            });
        }
        let points = self
            .fixed
            .iter()
            .flat_map(
                |(&block, sites)| {
                    let (throughout, last, rebuilt) = (&throughout, &last, &rebuilt);
                    sites
                        .iter()
                        .enumerate()
                        .flat_map(
                            move |(index, site)| {
                                let added = throughout
                                    .iter()
                                    .copied()
                                    .chain(
                                        last.iter()
                                            .filter(|(product, read)| product.2 == block && **read >= index)
                                            .map(|(&(one, k, block), _)| Resident::Product(one, k, block)),
                                    )
                                    .chain(
                                        rebuilt
                                            .iter()
                                            .filter(|&&at| self.alive[at].get(&block).is_some_and(|alive| alive[index]))
                                            .map(|&at| Resident::Rebuilt(at)),
                                    )
                                    .collect::<Vec<_>>();
                                std::iter::once(&site.before)
                                    .chain(&site.across)
                                    .map(
                                        move |point| spill::Point {
                                            registers: point.registers,
                                            residents: point
                                                .residents
                                                .iter()
                                                .map(|&value| Resident::Value(value))
                                                .chain(added.iter().copied())
                                                .collect(),
                                        },
                                    )
                            },
                        )
                },
            );
        let segment_points = self
            .fixed
            .values()
            .flatten()
            .map(
                |site| spill::Point {
                    registers: site.segments.registers,
                    residents: site
                        .segments
                        .residents
                        .iter()
                        .map(|&value| Resident::Value(value))
                        .chain(segmented.iter().copied())
                        .collect(),
                },
            );
        spill::spilled_in::<Resident, Residents>(points.chain(segment_points), |one| self.spill_price(one, reads))
    }
    /// Whether `key` is a far view of a segment: held in a segment register.
    fn viewed(
        &self,
        key: usize,
    ) -> bool {
        self.target.room.segments > 0
            && matches!(
                &self.keys[key],
                (Some(Operand::Value(base)), sum) if sum.is_zero() && self.views.contains(base)
            )
    }

    /// Whether `key` is a value the loop holds anyway.
    fn free(
        &self,
        key: &Key,
    ) -> bool {
        match key {
            (None, sum) => {
                sum.constant == BigInt::from(0)
                    && sum.terms.len() == 1
                    && sum.terms.iter().all(|(product, factor)| {
                        *factor == BigInt::from(1) && product.single().is_some_and(|value| self.live.contains(&value))
                    })
            }
            (Some(Operand::Value(value)), sum) => {
                sum.is_zero() && (self.live.contains(value) || self.frames.contains(value))
            }
            (Some(_), sum) => sum.is_zero(),
        }
    }

    /// The cheapest set found from `current` and from nothing.
    fn solved(
        &self,
        current: &BTreeSet<usize>,
    ) -> Option<BTreeSet<usize>> {
        let starts = [current.clone(), self.initial()?];
        starts
            .into_iter()
            .filter_map(|start| self.improved(start))
            .min_by_key(|set| (self.total(set).unwrap_or(i64::MAX), set.len(), set.clone()))
    }

    /// Each site's cheapest candidate alone, added until every site fits.
    fn initial(&self) -> Option<BTreeSet<usize>> {
        let mut set = BTreeSet::new();
        for (index, _) in self.sites.iter().enumerate() {
            if set.iter().any(|&one: &usize| self.fits[index][one].is_some()) {
                continue;
            }
            let best =
                (0..self.candidates.len()).filter(|&one| self.fits[index][one].is_some()).min_by_key(|&one| {
                    let mut with = set.clone();
                    with.insert(one);
                    (
                        self.fits.iter().filter(|fits| with.iter().all(|&one| fits[one].is_none())).count(),
                        self.fits[index][one].as_ref().map_or(i64::MAX, |(_, price)| price.cost),
                        one,
                    )
                })?;
            set.insert(best);
        }
        if set.is_empty() {
            set.insert((0..self.candidates.len()).find(|&one| self.exits[one].is_some())?);
        }
        Some(set)
    }

    /// `set` improved by adding, removing or swapping one candidate while
    /// any lowers the cost.
    fn improved(
        &self,
        mut set: BTreeSet<usize>,
    ) -> Option<BTreeSet<usize>> {
        let bounds = self.target.bounds;
        // Past `iv-consider-all-candidates-bound`, where gcc stops considering
        // every candidate for every use; below it the search is whole:
        // measured on the bench, replacing once and not beyond ten uses costs
        // 6% of crc's instructions.
        let bounded = self.candidates.len() > bounds.all_candidates;
        // Cheaper first, then fewer counters: a tie goes to fewer registers.
        let rank = |set: &BTreeSet<usize>| self.total(set).map(|total| (total, set.len()));
        let mut current = rank(&set).unwrap_or((i64::MAX, usize::MAX));
        // gcc replaces a candidate (`iv_ca_replace`) once, where no one added
        // or removed lowers the cost.
        let mut replace = true;
        loop {
            let mut best: Option<((i64, usize), BTreeSet<usize>)> = None;
            let consider = |best: &mut Option<((i64, usize), BTreeSet<usize>)>, with: BTreeSet<usize>| {
                if let Some(ranked) = rank(&with)
                    && ranked < current
                    && best.as_ref().is_none_or(|(least, one)| ranked < *least || (ranked == *least && with < *one))
                {
                    *best = Some((ranked, with));
                }
            };
            for one in 0..self.candidates.len() {
                let mut with = set.clone();
                if !with.insert(one) {
                    with.remove(&one);
                }
                consider(&mut best, with);
            }
            if !bounded || (replace && best.is_none()) {
                replace = false;
                let serving = if bounded { self.serving(&set) } else { Vec::new() };
                for &out in &set {
                    // A candidate of one use is another's to prune; one of many
                    // is too costly to place again.
                    if bounded && (serving[out] == 1 || serving[out] > bounds.always_prune) {
                        continue;
                    }
                    for into in (0..self.candidates.len()).filter(|one| !set.contains(one)) {
                        let mut with = set.clone();
                        with.remove(&out);
                        with.insert(into);
                        consider(&mut best, with);
                    }
                }
            }
            let Some((ranked, with)) = best else { break };
            current = ranked;
            set = with;
        }
        (current.0 != i64::MAX).then_some(set)
    }

    /// How many sites each candidate serves in `set`: gcc's `n_cand_uses`.
    fn serving(
        &self,
        set: &BTreeSet<usize>,
    ) -> Vec<usize> {
        let mut served = vec![0; self.candidates.len()];
        for index in 0..self.sites.len() {
            match self.choice(set, index) {
                Some((Some((one, ..)), _)) | Some((None, Some((one, _)))) => served[one] += 1,
                _ => {}
            }
        }
        served
    }
}

/// `plan` carried out; the block a guarded loop is now entered at.
fn _applied(
    unit: &mut Unit,
    plan: &Plan,
) -> Option<BlockId> {
    let (context, function) = (&mut *unit.context, &mut *unit.function);
    let entering = function.terminator(plan.preheader).expect("a preheader's branch");
    let mut expander = Expander::new(entering);
    let back = function.terminator(plan.latch).expect("a latch's branch");
    // Each candidate: a header phi from its start, stepped before the latch's
    // branch.
    let mut registers = Vec::new();
    let mut steps = Vec::new();
    let mut stepping = Vec::new();
    for one in &plan.candidates {
        if let Some(existing) = one.existing {
            registers.push(Operand::Value(existing));
            let ValueDef::Instruction(phi) = function.value(existing).def else { unreachable!("a counter is a phi") };
            let from_latch = function
                .instruction(phi)
                .operands
                .chunks(2)
                .find(|pair| pair[1] == Operand::Block(plan.latch))
                .map(|pair| pair[0])
                .expect("a latch arm");
            // A counter that takes another counter's next value and a constant
            // more steps from itself, so the other can go.
            let steps_itself = |next: ValueId| match function.value(next).def {
                ValueDef::Instruction(made) => {
                    let op = function.instruction(made);
                    match op.opcode {
                        Opcode::Binary(BinaryOp::Add) => op.operands.contains(&Operand::Value(existing)),
                        Opcode::Binary(BinaryOp::Sub) => op.operands[0] == Operand::Value(existing),
                        _ => false,
                    }
                }
                _ => false,
            };
            let own = matches!(from_latch, Operand::Value(next) if steps_itself(next));
            if !own && one.pointer.is_none() {
                let ty = function.value(existing).ty;
                let step = expander.int(context, function, &one.of.step);
                let next = function.create_instruction(
                    Opcode::Binary(BinaryOp::Add),
                    ty,
                    vec![Operand::Value(existing), step],
                    Flags::default(),
                    Some("lsr.iv.next"),
                );
                // Where the step it replaces stood, for what reads it there.
                let at = match from_latch {
                    Operand::Value(old) => match function.value(old).def {
                        ValueDef::Instruction(made) if function.parent(made) == Some(plan.latch) => {
                            Position::Before(made)
                        }
                        _ => Position::Before(back),
                    },
                    _ => Position::Before(back),
                };
                function.insert(next, at).expect("a latch");
                let next = Operand::Value(function.instruction(next).result.expect("a value"));
                let operands = function
                    .instruction(phi)
                    .operands
                    .chunks(2)
                    .flat_map(
                        |pair| if pair[1] == Operand::Block(plan.latch) { [next, pair[1]] } else { [pair[0], pair[1]] },
                    )
                    .collect();
                function.set_operands(phi, operands);
                steps.push(next);
                stepping.push(None);
                continue;
            }
            steps.push(from_latch);
            stepping.push(None);
            continue;
        }
        let ty = one.pointer.unwrap_or_else(|| context.types.int(one.of.width()));
        let start = expander.value(context, function, one.of.pointer, &one.of.start, ty);
        let step = expander.int(context, function, &one.of.step);
        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), Some("lsr.iv"));
        let first = function.block(plan.header).first().expect("a block with an instruction");
        function.insert(phi, Position::Before(first)).expect("a header");
        let value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
        let opcode = if one.of.pointer.is_some() {
            Opcode::GetElementPtr { source: context.types.int(8) }
        } else {
            Opcode::Binary(BinaryOp::Add)
        };
        let next = function.create_instruction(opcode, ty, vec![value, step], Flags::default(), Some("lsr.iv.next"));
        function.insert(next, Position::Before(back)).expect("a latch");
        stepping.push(Some(next));
        let next = Operand::Value(function.instruction(next).result.expect("a value"));
        function.set_operands(phi, vec![start, Operand::Block(plan.preheader), next, Operand::Block(plan.latch)]);
        registers.push(value);
        steps.push(next);
    }
    // Each use from its candidate. An exit phi takes the candidate as it
    // leaves, and what it carried is computed after the phis.
    let mut leaving = HashMap::<(BlockId, usize), Operand>::default();
    let mut products = HashMap::default();
    // Steps are placed first: an address after one reads the stepped value.
    let ordered =
        plan.uses.iter().filter(|(_, _, fit)| fit.next).chain(plan.uses.iter().filter(|(_, _, fit)| !fit.next));
    for (site, index, fit) in ordered {
        let candidate = &plan.candidates[*index];
        if let Place::Exit(_, phi) = site.at
            && function.is_erased(phi)
        {
            continue;
        }
        let (at, register) = match site.at {
            Place::Before(inst) => (Position::Before(inst), registers[*index]),
            Place::End(block) => {
                (Position::Before(function.terminator(block).expect("a terminated block")), registers[*index])
            }
            Place::Exit(block, _) => {
                let register = match leaving.get(&(block, *index)) {
                    Some(&one) => one,
                    None => {
                        let preds = function.predecessors(block);
                        let ty = function
                            .value(match registers[*index] {
                                Operand::Value(value) => value,
                                _ => unreachable!("a candidate is a value"),
                            })
                            .ty;
                        let phi = function.create_instruction(Opcode::Phi, ty, Vec::new(), Flags::default(), None);
                        let first = function.block(block).first().expect("a block with an instruction");
                        function.insert(phi, Position::Before(first)).expect("an exit");
                        function.set_operands(
                            phi,
                            preds.iter().flat_map(|&pred| [registers[*index], Operand::Block(pred)]).collect(),
                        );
                        let one = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
                        leaving.insert((block, *index), one);
                        one
                    }
                };
                let after = function
                    .block(block)
                    .iter()
                    .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
                    .expect("a terminator");
                (Position::Before(after), register)
            }
        };
        let ty = function.value(site.one.value).ty;
        let value = if fit.next {
            // The step goes before its first reader in the latch.
            if let (Some(step), Place::Before(reader)) = (stepping[*index], site.at) {
                let order = function.block(plan.latch);
                if order.iter().position(|one| one == reader) < order.iter().position(|one| one == step) {
                    function.move_to(step, Position::Before(reader)).expect("a placed reader");
                }
            }
            steps[*index]
        } else if fit.folded.is_some() {
            register
        } else if let Some(after) = _after_step(function, plan.latch, site, candidate, fit, steps[*index]) {
            _realized(context, function, &mut expander, &mut products, site, candidate, &after, steps[*index], ty, at)
        } else {
            _realized(context, function, &mut expander, &mut products, site, candidate, fit, register, ty, at)
        };
        match site.at {
            Place::Exit(_, phi) => {
                let result = function.instruction(phi).result.expect("a phi's value");
                function.replace_all_uses_with(result, value);
                function.set_operands(phi, Vec::new());
                function.erase(phi).expect("its uses were replaced");
            }
            _ => function.set_operand(site.one.user, site.one.index, value),
        }
        if let Some(folded) = &fit.folded {
            let other = expander.int(context, function, folded);
            function.set_operand(site.one.user, 1 - site.one.index, other);
        }
    }
    let (exit, index) = plan.exit.as_ref()?;
    let candidate = &plan.candidates[*index];
    let proof = &exit.proof;
    let end = _end(exit, candidate);
    let ty = function
        .value(match registers[*index] {
            Operand::Value(value) => value,
            _ => unreachable!("a candidate is a value"),
        })
        .ty;
    let end = expander.value(context, function, candidate.of.pointer, &end, ty);
    let tested = if proof.posttested { steps[*index] } else { registers[*index] };
    let branch = proof.branch;
    // A test in the body, not the latch, reads a step made before it; a step
    // already ahead of the branch stays, where its readers need it.
    if let (true, Some(next)) = (proof.posttested, stepping[*index]) {
        let block = function.parent(branch).expect("a placed branch");
        let order = function.block(block);
        let ahead = function.parent(next) == Some(block)
            && order.iter().position(|one| one == next) < order.iter().position(|one| one == branch);
        if !ahead {
            function.move_to(next, Position::Before(branch)).expect("a placed branch");
        }
    }
    let continues = matches!(
        function.instruction(branch).operands[1],
        Operand::Block(block) if plan.loop_.body.contains(&cfg::id(block))
    );
    let predicate = if continues { IntPredicate::Ne } else { IntPredicate::Eq };
    let bit = context.types.int(1);
    let test = function.create_instruction(Opcode::ICmp(predicate), bit, vec![tested, end], Flags::default(), None);
    function.insert(test, Position::Before(branch)).expect("a placed branch");
    function.set_operand(branch, 0, Operand::Value(function.instruction(test).result.expect("a value")));
    if !exit.guarded {
        return None;
    }
    // A symbolic count holds only once the loop is entered: a guard skips
    // it where it runs no trip, and the loop is entered at its body.
    let guard = counting::skip_guard(
        &mut Seeds { context: &mut *context, function: &mut *function, at: entering, width: proof.width() },
        proof,
    )
    .expect("a pre-tested loop");
    let shape = rotate::_shape(function, &plan.loop_).expect("a rotatable loop");
    rotate::_rotate(context, function, &shape, Some((Operand::Value(guard), true)), &[]).expect("a rotation");
    crate::cfg::merged(function);
    // The guard also reaches the exit: the loop leaves through its own block
    // again.
    crate::loopsimplify::simplified(function);
    Some(shape.first)
}

/// An address placed after its candidate's step in the latch, as a fit
/// of the stepped value: a step less in its constant. Reading the counter
/// there would keep it live beside its successor.
fn _after_step(
    function: &Function,
    latch: BlockId,
    site: &Site,
    candidate: &Candidate,
    fit: &Fit,
    stepped: Operand,
) -> Option<Fit> {
    let Place::Before(reader) = site.at else { return None };
    let Operand::Value(stepped) = stepped else { return None };
    let ValueDef::Instruction(step) = function.value(stepped).def else { return None };
    let order = function.block(latch);
    let (step_at, reader_at) = (order.iter().position(|one| one == step)?, order.iter().position(|one| one == reader)?);
    if site.one.kind != UseKind::Address || fit.trip.is_some() || fit.k == BigInt::from(0) || step_at > reader_at {
        return None;
    }
    let by = candidate.of.step.known()?;
    Some(Fit { constant: &fit.constant - &fit.k * by, ..fit.clone() })
}

/// `site`'s value from `register`, the candidate's, placed at `at`.
#[allow(clippy::too_many_arguments)]
fn _realized(
    context: &mut Context,
    function: &mut Function,
    expander: &mut Expander,
    products: &mut HashMap<(Operand, BigInt, BlockId), Operand>,
    site: &Site,
    candidate: &Candidate,
    fit: &Fit,
    register: Operand,
    ty: TypeId,
    at: Position,
) -> Operand {
    let width = fit.rest.width;
    let int = context.types.int(width);
    let i8 = context.types.int(8);
    let magnitude = BigInt::from(fit.k.magnitude().clone());
    let constant = Scev::constant(fit.constant.clone(), width);
    let (register, pointer) = match &fit.trip {
        Some((shift, down, inverse)) => {
            let (shift, down) = (*shift, *down);
            let mut trip = if candidate.of.start.is_zero() && !down {
                // The distance from a start of zero is the counter.
                register
            } else {
                let start = expander.int(context, function, &candidate.of.start);
                let distance = if down { vec![start, register] } else { vec![register, start] };
                expand::placed(context, function, Opcode::Binary(BinaryOp::Sub), int, distance, at)
            };
            if shift != 0 {
                let by = counting::constant(context, &BigInt::from(shift), width);
                trip = expand::placed(context, function, Opcode::Binary(BinaryOp::LShr), int, vec![trip, by], at);
            }
            if *inverse != BigInt::from(1) {
                let by = counting::constant(context, inverse, width);
                trip = expand::placed(context, function, Opcode::Binary(BinaryOp::Mul), int, vec![trip, by], at);
                if shift != 0 {
                    let mask = counting::constant(context, &((BigInt::from(1) << (width - shift)) - 1), width);
                    trip = expand::placed(context, function, Opcode::Binary(BinaryOp::And), int, vec![trip, mask], at);
                }
            }
            (trip, None)
        }
        None => (register, candidate.of.pointer),
    };
    // A product in the loop is made once in its block, at its top.
    let shared = match (&fit.trip, site.at) {
        (None, Place::Before(inst)) => function.parent(inst),
        (None, Place::End(block)) => Some(block),
        _ => None,
    };
    let scaled = |context: &mut Context,
                  function: &mut Function,
                  products: &mut HashMap<(Operand, BigInt, BlockId), Operand>|
     -> Option<Operand> {
        if fit.k == BigInt::from(0) {
            return None;
        }
        if fit.k == BigInt::from(1) {
            return Some(register);
        }
        if let Some(&made) = shared.and_then(|block| products.get(&(register, fit.k.clone(), block))) {
            return Some(made);
        }
        // At the block's top, or after the register where the block steps it.
        let at = match shared {
            Some(block) => {
                let order = function.block(block).instructions();
                let defined = match register {
                    Operand::Value(value) => match function.value(value).def {
                        ValueDef::Instruction(def) => order
                            .iter()
                            .position(|&one| one == def)
                            .filter(|_| function.instruction(def).opcode != Opcode::Phi),
                        _ => None,
                    },
                    _ => None,
                };
                let first = defined.map_or_else(
                    || {
                        order
                            .iter()
                            .position(|&one| function.instruction(one).opcode != Opcode::Phi)
                            .expect("a terminator")
                    },
                    |def| def + 1,
                );
                Position::Before(order[first])
            }
            None => at,
        };
        // An index is scaled at its register's width.
        let index = if fit.trip.is_none() { candidate.of.width() } else { width };
        let scaled = expand::scaled(context, function, register, &magnitude, index, at);
        let made = if fit.k < BigInt::from(0) {
            let zero = counting::constant(context, &BigInt::from(0), index);
            let wide = context.types.int(index);
            expand::placed(context, function, Opcode::Binary(BinaryOp::Sub), wide, vec![zero, scaled], at)
        } else {
            scaled
        };
        if let Some(block) = shared {
            products.insert((register, fit.k.clone(), block), made);
        }
        Some(made)
    };
    // A value no candidate steps, as one read after a loop of known count, is
    // its base and its sum.
    if fit.k == BigInt::from(0) && fit.trip.is_none() {
        let sum = fit.rest.plus(&constant);
        return match site.one.of.pointer {
            Some(_) => expander.value(context, function, fit.base, &sum, ty),
            None => expander.int(context, function, &sum),
        };
    }
    match (pointer, site.one.of.pointer) {
        // A pointer candidate: itself, offset by the rest and the constant.
        (Some(_), _) => {
            let mut value = register;
            if !fit.rest.is_zero() {
                let rest = expander.int(context, function, &fit.rest);
                value =
                    expand::placed(context, function, Opcode::GetElementPtr { source: i8 }, ty, vec![value, rest], at);
            }
            if fit.constant != BigInt::from(0) {
                let offset = counting::constant(context, &fit.constant, width);
                value = expand::placed(
                    context,
                    function,
                    Opcode::GetElementPtr { source: i8 },
                    ty,
                    vec![value, offset],
                    at,
                );
            }
            value
        }
        // An address or pointer from an integer candidate: its base indexed
        // by the scaled candidate, then the constant, which the address form
        // takes.
        (None, Some(_)) => {
            let base = expander.value(context, function, fit.base, &fit.rest, ty);
            let mut value = match scaled(context, function, products) {
                Some(index) => {
                    expand::placed(context, function, Opcode::GetElementPtr { source: i8 }, ty, vec![base, index], at)
                }
                None => base,
            };
            if fit.constant != BigInt::from(0) {
                let offset = counting::constant(context, &fit.constant, width);
                value = expand::placed(
                    context,
                    function,
                    Opcode::GetElementPtr { source: i8 },
                    ty,
                    vec![value, offset],
                    at,
                );
            }
            value
        }
        (None, None) => {
            let invariant = fit.rest.plus(&constant);
            let scaled = scaled(context, function, products);
            let value = match (scaled, invariant.is_zero()) {
                (Some(scaled), true) => scaled,
                (None, _) => expander.int(context, function, &invariant),
                (Some(scaled), false) => {
                    let invariant = if invariant.terms.is_empty() {
                        counting::constant(context, &constant.constant, width)
                    } else {
                        expander.int(context, function, &invariant)
                    };
                    expand::placed(context, function, Opcode::Binary(BinaryOp::Add), int, vec![scaled, invariant], at)
                }
            };
            value
        }
    }
}

#[cfg(test)]
#[path = "lsr_tests.rs"]
mod tests;
