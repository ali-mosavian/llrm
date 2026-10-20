//! Unroll-and-jam: the outer loop of a two-loop nest is unrolled by N and the
//! N copies of the inner loop are fused into one, so what the outer
//! iterations share is used by all N in one pass of the inner loop.
//!
//! # How it works
//! A nest (`loopnest`) is a loop that holds exactly one other loop and nothing
//! else besides pure work. The outer header tests and has one way into the
//! loop. A chain of single-successor blocks `pre` leads from it to the inner
//! header, and a chain `post` from the inner exit to the outer latch; no other
//! block is in the outer loop. Work in `pre`, `post` and the outer header is
//! pure (no memory, no division), and `pre`, `post` have no phis. The inner
//! loop has one latch and leaves only from its header, into a block only it
//! enters.
//!
//! The inner loop must run the same in every outer iteration: its test and the
//! starts and steps of its counters read only values the nest does not
//! change, and nothing it computes is read outside it. Its memory accesses
//! (non-volatile loads and stores; no call, `invoke` or `alloca`) must each
//! be an affine function of the two counters over an identified object. They
//! go to `llrm_analysis::dependence::factor` with `max_unroll` (4, gcc's
//! `unroll-jam-max-unroll`) and `min_percent` (1, `unroll-jam-min-percent`),
//! which bounds N by the dependence distances and refuses a nest where fewer
//! than `min_percent` percent of the accesses go away. The outer loop needs an
//! exact trip count (ScalarEvolution's `getSmallConstantTripCount`); N is the
//! largest factor from 2 up to the bound that divides it.
//!
//! The outer loop is unrolled by `loopclone::chained` into N bodies. In each
//! copy the work between the loops moves ahead of the first inner loop, the
//! copy's inner counters are replaced by the first's (one iteration space),
//! the copy's inner test is dropped, and the first inner loop's body runs on
//! into the copy's body before the latch jumps back.
//!
//! ```text
//!   before (x2 copies)               after (N = 2)
//!   outer:  pre_0                    outer: pre_0, pre_1, post_0
//!     inner_0: body_0 ->|              inner: body_0 ; body_1 -> inner
//!     post_0            |              post_1
//!   outer:  pre_1       |
//!     inner_1: body_1 ->|            (inner_1's entry goes straight to its
//!     post_1                          exit; its header is erased)
//! ```
//!
//! Left alone, narrower than gcc: an outer count N does not divide (gcc runs
//! an epilogue), loads between the loops, a rotated (guarded) inner loop, and
//! an access whose address is not an affine function of the two counters
//! over an identified object.
//!
//! `LLRM_DEBUG=unroll-jam` prints `jammed a nest` for each nest fused.
//!
//! # Example
//! Before:
//! ```mir
//! b3:
//!   %j = phi i16 [ 0, %b2 ], [ %j.next, %b3.body ]
//!   %more = icmp slt i16 %j, 8
//!   br i1 %more, label %b3.body, label %b4
//!
//! b3.body:
//!   %o = mul i16 %j, 2
//!   %pp = getelementptr i8, ptr @p, i16 %o
//!   %pv = getelementptr i8, ptr @v, i16 %o
//!   %a = load i16, ptr %pp
//!   %b = load i16, ptr %pv
//!   %s = add i16 %a, %b
//!   %s2 = add i16 %s, %bias
//!   store i16 %s2, ptr %pp
//!   %j.next = add i16 %j, 1
//!   br label %b3
//!
//! b4:
//!   %t.next = add i16 %t, 1
//!   br label %b1
//! ```
//! After:
//! ```mir
//! b3:
//!   %j = phi i16 [ 0, %b2 ], [ %j.next, %3 ]
//!   %more = icmp slt i16 %j, 8
//!   br i1 %more, label %b3.body, label %b4
//!
//! b3.body:
//!   %o = mul i16 %j, 2
//!   ; ...
//!   %a = load i16, ptr %pp
//!   %b = load i16, ptr %pv
//!   ; ...
//!   store i16 %s2, ptr %pp
//!   %j.next = add i16 %j, 1
//!   br label %3
//!
//! 3:
//!   %4 = icmp slt i16 %j, 8
//!   %5 = mul i16 %j, 2
//!   ; ...
//!   %8 = load i16, ptr %6
//!   %9 = load i16, ptr %7
//!   ; ...
//!   store i16 %11, ptr %6
//!   %12 = add i16 %j, 1
//!   br label %b3
//! ```
//! Two sweeps of `p[j] += v[j] + bias` run in one pass over `j`: `b3.body` is
//! the sweep of outer trip `t`, block `3` that of `t + 1`, and the outer loop
//! now steps by two.
//!
//! From unit test `sweeps_of_the_same_cells_are_made_two_at_a_time` in
//! `unroll_jam_tests.rs`, six sweeps over eight cells; the blocks of the nest
//! are shown.
//!
//! # Runs
//! Once per body, after `unswitch`, so an invariant guard is out of the
//! nest. If it fused a nest, `sroa` and the fixed point run again over the
//! result (the fused stores merge there). -O3 and -Omax:
//! `-floop-unroll-and-jam`. Not run where one pass is selected by name. See
//! `docs/reference/levels.md`.
//!
//! The pin's default pipeline never jams: `enable-unroll-and-jam` is false
//! ([`PassBuilderPipelines.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L222)),
//! so this pass is gcc's, not a translation. Were it enabled, the pin runs
//! SimplifyCFG with hoisting and sinking (L1438) before it (L1474); llrm runs
//! the jam first, because the matcher needs the nest as it was before that
//! rewrite. Exit: the matcher accepts the L1438 form; no date.
//!
//! # Needs
//! `induction` (trip counts), `dependence`, `Registers` (`llrm_analysis`);
//! [`crate::loopclone`] (`chained`).
//!
//! # Keeps
//! Nothing: a change invalidates every analysis.
//!
//! # See also
//! gcc's `pass_loop_jam`
//! ([`gimple-loop-jam.cc`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/gimple-loop-jam.cc)),
//! LLVM's `LoopUnrollAndJamPass`.

use std::collections::BTreeSet;

use llrm_analysis::dependence;
use llrm_analysis::manager::Registers;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::{cfg, memory};
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};

use crate::lcssa::{arms, from_arms};
use crate::loopclone::chained;
use crate::loopnest::{Nest, accesses, inner_is_invariant, nest_of, phis, work};

/// gcc's `unroll-jam-max-unroll` and `unroll-jam-min-percent`
/// ([`params.opt:1146`](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/params.opt#L1146)).
pub const MAX_UNROLL: u32 = 4;
pub const MIN_PERCENT: u32 = 1;

pub struct UnrollJam {
    pub max_unroll: u32,
    pub min_percent: u32,
}

impl Default for UnrollJam {
    fn default() -> Self {
        Self { max_unroll: MAX_UNROLL, min_percent: MIN_PERCENT }
    }
}

impl FunctionPass for UnrollJam {
    fn name(&self) -> &'static str {
        "unroll-jam"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        match jammed(unit, analyses, self) {
            Ok(true) => PreservedAnalyses::none(),
            Ok(false) => PreservedAnalyses::all(),
            Err(error) => panic!("unroll-jam: {error}"),
        }
    }
}

/// Every nest this can jam, once each; whether any was.
pub fn jammed(
    unit: &mut passes::Unit,
    analyses: &Analyses,
    params: &UnrollJam,
) -> Result<bool, String> {
    let mut changed = false;
    let mut refused = BTreeSet::new();
    while one(unit, analyses, params, &mut refused)? {
        llrm_support::debug!("unroll-jam", "jammed a nest");
        changed = true;
    }
    Ok(changed)
}

/// The first nest this jams, if any; whether there was one.
fn one(
    unit: &mut passes::Unit,
    analyses: &Analyses,
    params: &UnrollJam,
    refused: &mut BTreeSet<(i64, i64)>,
) -> Result<bool, String> {
    let graph = cfg::graph(unit.function);
    let shape = cfg::Shape::of(unit.function);
    let mut chosen = None;
    // Computed for the first nest there is: most functions have none.
    let mut facts = None;
    let mut scevs = None;
    for inner in &shape.loops {
        if shape.loops.iter().any(|other| other.header != inner.header && inner.body.is_superset(&other.body)) {
            continue;
        }
        let Some(outer) = shape
            .loops
            .iter()
            .filter(|other| other.header != inner.header && other.body.is_superset(&inner.body))
            .min_by_key(|other| other.body.len())
        else {
            continue;
        };
        if refused.contains(&(outer.header, inner.header)) {
            continue;
        }
        let Some(nest) = nest_of(unit.function, &graph, outer, inner) else { continue };
        if !inner_is_invariant(unit.function, &nest) {
            refused.insert((outer.header, inner.header));
            continue;
        }
        let facts =
            facts.get_or_insert_with(|| analyses.fresh().get::<Registers>(unit.context, unit.layout, unit.function));
        let memory_unit = memory::Unit::within(unit.context, unit.layout, unit.function, analyses.outer())
            .with_registers(facts)
            .with_shape(&shape);
        let session = scevs.get_or_insert_with(|| {
            ScevSession::within(unit.context, &analyses.outer().globals, &analyses.outer().metadata, unit.function)
        });
        let planned = (|| {
            let trips = crate::profit::scev_trips(session, &memory_unit, outer)?;
            let inner_trips = crate::profit::scev_trips(session, &memory_unit, inner);
            let list = accesses(&memory_unit, &nest)?;
            if list.is_empty() {
                return None;
            }
            let plan = dependence::factor(&list, params.max_unroll, params.min_percent, inner_trips);
            (2..=plan.unroll).rev().find(|n| trips % i64::from(*n) == 0 && trips / i64::from(*n) >= 1)
        })();
        match planned {
            Some(factor) => {
                chosen = Some((nest, factor as usize));
                break;
            }
            None => {
                refused.insert((outer.header, inner.header));
            }
        }
    }
    let Some((nest, factor)) = chosen else { return Ok(false) };
    let key = (nest.outer.header, nest.inner.header);
    let applied = apply(unit.function, &nest, factor)?;
    if !applied {
        refused.insert(key);
    }
    Ok(applied)
}

/// `nest` with its outer loop unrolled by `factor` and the inner copies fused;
/// whether it could be.
fn apply(
    function: &mut Function,
    nest: &Nest,
    factor: usize,
) -> Result<bool, String> {
    let Some(copies) = chained(function, &nest.outer, factor - 1)? else { return Ok(false) };
    let (hi, li, ho) = (nest.inner_header(), nest.inner_latch(), nest.outer_header());
    let entry0 = nest.entry();
    let inner_first = |function: &Function, header: BlockId, inside: &dyn Fn(BlockId) -> bool| {
        function.successors(header).into_iter().find(|&to| inside(to)).expect("the body's first block")
    };
    let mut tail = li;
    let mut previous_post = nest.post.clone();
    for copy in &copies {
        let hi_k = copy[&cfg::id(hi)];
        let li_k = copy[&cfg::id(li)];
        let body_k: IdSet<BlockId> = nest.inner.body.iter().map(|at| copy[at]).collect();
        let entry_k = copy[&cfg::id(entry0)];
        let xi_k = copy[&cfg::id(nest.post[0])];
        // The work between the loops, ahead of the fused loop.
        let region: Vec<BlockId> = previous_post
            .iter()
            .copied()
            .chain([copy[&cfg::id(ho)]])
            .chain(nest.pre.iter().map(|block| copy[&cfg::id(*block)]))
            .collect();
        let before = function.terminator(entry0).expect("a terminated entry");
        for block in &region {
            for inst in work(function, *block) {
                function.move_to(inst, Position::Before(before))?;
            }
        }
        // The copy's counters are the first's: the same iteration space.
        let first_phis = phis(function, hi);
        let copy_phis = phis(function, hi_k);
        for (&mine, &theirs) in first_phis.iter().zip(&copy_phis) {
            let (kept, dropped) = (
                function.instruction(mine).result.expect("a phi's value"),
                function.instruction(theirs).result.expect("a phi's value"),
            );
            function.replace_all_uses_with(dropped, Operand::Value(kept));
            function.set_operands(theirs, Vec::new());
            function.erase(theirs)?;
        }
        // The copy's test is not made: its work runs ahead of the body.
        let first_k = inner_first(function, hi_k, &|to| body_k.contains(&to));
        let at = function.iter_block(first_k).find(|&inst| function.instruction(inst).opcode != Opcode::Phi);
        for inst in work(function, hi_k) {
            match at {
                Some(at) => function.move_to(inst, Position::Before(at))?,
                None => function.move_to(inst, Position::End(first_k))?,
            }
        }
        for phi in phis(function, first_k) {
            let incoming = arms(function, phi)
                .into_iter()
                .map(|(value, from)| (value, if from == hi_k { tail } else { from }))
                .collect::<Vec<_>>();
            function.set_operands(phi, from_arms(&incoming));
        }
        // The tail runs on into the copy's body, whose own tail loops back.
        let jump = function.terminator(tail).expect("a terminated tail");
        crate::edges::retarget(function, jump, hi, first_k);
        let back = function.terminator(li_k).expect("a terminated latch");
        crate::edges::retarget(function, back, hi_k, hi);
        // The copy's loop is never entered: its entry goes straight out.
        let entry_jump = function.terminator(entry_k).expect("a terminated entry");
        crate::edges::retarget(function, entry_jump, hi_k, xi_k);
        let test = function.terminator(hi_k).expect("a terminated header");
        function.erase(test)?;
        function.erase_block(hi_k)?;
        tail = li_k;
        previous_post = nest.post.iter().map(|block| copy[&cfg::id(*block)]).collect();
    }
    // The fused loop's counters take the last body's back edge.
    for phi in phis(function, hi) {
        let incoming = arms(function, phi)
            .into_iter()
            .map(|(value, from)| (value, if from == li { tail } else { from }))
            .collect::<Vec<_>>();
        function.set_operands(phi, from_arms(&incoming));
    }
    Ok(true)
}

#[cfg(test)]
#[path = "unroll_jam_tests.rs"]
mod tests;
