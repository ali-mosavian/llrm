//! DEVIATE UP1 trial pricing (owner w15, exit 2026-11-20). The pin has no
//! price for a full unroll its threshold allows. Where the target prices bytes
//! in clocks, such a loop is unrolled on a copy of the function, the
//! Simplify list runs once over the copy as it does over an inliner trial
//! (`walk.trial`), and the copy's real bytes and clocks decide, not an estimate
//! of what the analysis folds.
//!
//! The clocks of a block are weighted by the trips of the two loops around it:
//! a known small trip count, else `ASSUMED_TRIPS`. The function's own entry
//! frequency is not seen (a rolled loop in a hot callee is weighted as in a
//! cold one).

use std::collections::HashMap;
use std::rc::Rc;

use llrm_analysis::scev::Cx;

use super::price::{ASSUMED_TRIPS, unroll_pays};
use super::*;
use crate::loopidioms::{trip_clocks, trip_cost};

/// Loops tried in one function run: each costs two Simplify lists.
const MAX_TRIALS: usize = 4;
/// Functions of more instructions are not tried.
const TRIAL_FUNCTION: usize = 1500;

/// The bytes and weighted clocks of the function `run` holds.
fn measure(run: &mut Run) -> (i64, i64) {
    let analyzed = Analyzed::of(run);
    let clocks = crate::profit::costs(run.outer);
    let bytes = run.outer.target().byte_costs();
    let mut se = ScalarEvolution::new();
    let cx = Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut trips: HashMap<BlockId, i64> = HashMap::new();
    let mut weighted = 0;
    for block in run.function.layout() {
        let mut weight = 1;
        let mut around = analyzed.loops.get_loop_for(*block);
        for _ in 0..2 {
            let Some(lp) = around else { break };
            let header = lp.get_header();
            weight *= *trips.entry(header).or_insert_with(|| match se.get_small_constant_trip_count(&cx, header) {
                0 => ASSUMED_TRIPS,
                known => i64::from(known),
            });
            around = lp.get_parent_loop();
        }
        weighted += weight
            * trip_clocks(run.context, run.layout, run.function, run.callees, std::iter::once(block), &clocks);
    }
    let size = trip_cost(run.context, run.layout, run.function, run.callees, run.function.layout().iter(), &bytes, true);
    (size, weighted)
}

impl LoopFullUnroll {
    /// The loops of `deferred` whose trial unroll pays, unrolled in `unit`: the
    /// headers each unroll made.
    pub(super) fn accepted(
        &self,
        unit: &mut Unit,
        outer: &Rc<llrm_mir::passes::Outer>,
        tti: &dyn Tti,
        deferred: &[BlockId],
        cleanup: &Rc<dyn Fn() -> Box<dyn FunctionPass>>,
    ) -> Vec<Vec<BlockId>> {
        let mut made = Vec::new();
        if deferred.is_empty() || unit.function.walk().count() > TRIAL_FUNCTION {
            return made;
        }
        let rate = tti.unroll_milliclocks_per_byte();
        let mut pass = cleanup();
        let mut base: Option<(i64, i64)> = None;
        for &header in deferred.iter().take(MAX_TRIALS) {
            let mut measured = |unit: &mut Unit, trial: Trial, unroll: bool| {
                let mut copy = unit.function.clone();
                let mut one = Unit {
                    context: &mut *unit.context,
                    layout: unit.layout,
                    function: &mut copy,
                    id: unit.id,
                    metadata: unit.metadata,
                    declared: &mut *unit.declared,
                };
                if unroll {
                    let mut run = self.run_with(&mut one, outer, tti, ScalarEvolution::new(), trial);
                    if !unroll_loops(&mut run) {
                        return None;
                    }
                }
                pass.run(&mut one, &mut Analyses::new(Rc::clone(outer)));
                let mut run = self.run_with(&mut one, outer, tti, ScalarEvolution::new(), Trial::Skip);
                Some(measure(&mut run))
            };
            let before = match base {
                Some(done) => done,
                None => *base.insert(measured(unit, Trial::Skip, false).expect("a measure")),
            };
            let Some(after) = measured(unit, Trial::Force(header), true) else { continue };
            let (saved, added) = (before.1 - after.1, after.0 - before.0);
            let pays = unroll_pays(saved, added, rate);
            if llrm_support::debug::enabled("unroll") {
                let name = unit.function.block(header).name.clone().unwrap_or_default();
                eprintln!(
                    "unroll: {header:?} %{name}: trial unroll: saves {saved} clk, adds {added} B (rate {rate}): {}",
                    if pays { "unrolled" } else { "kept rolled" }
                );
            }
            if !pays {
                continue;
            }
            let mut run = self.run_with(unit, outer, tti, ScalarEvolution::new(), Trial::Force(header));
            if unroll_loops(&mut run) {
                made.push(std::mem::take(&mut run.created));
                base = None;
            }
        }
        made
    }
}
