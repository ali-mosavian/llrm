//! DEVIATE UP1 (owner x86-m32; exit 2026-11-20): a full unroll the pin's
//! threshold allows is made only where the clocks it saves repay the bytes it
//! adds. The pin's bar counts instructions and never asks what a copy of the
//! body costs in code (x_ld_max: the 8-trip inner loop, copied 8 times,
//! 232 -> 755 bytes at m32 -O2 for a few clocks).
//!
//! The full-unroll gate against the pin's `shouldFullUnroll`
//! (LoopUnrollPass.cpp:904), audited 2026-11-08 (w2): (1) the gate itself,
//! above, run on both success paths of `should_full_unroll`, with bypasses (an
//! inlinable internal function, a copy of this run, the nest, loops of few
//! trips); (2) the cost analysis is passed `u32::MAX` and the trips, where the
//! pin passes `Threshold * MaxPercentThresholdBoost / 100` and
//! `MaxIterationsCountToAnalyze`; (3) UNROLL-O1 (owner x86), (4) UNROLL-TRIAL
//! (owner 6502, exit 2026-12-05), (5) U1, the optsize few-trip bar (arm64
//! states none), (6) U3, the float-stack bar (owner m32, exit 2026-11-20).

use llrm_mir::dense::IdMap;
use llrm_mir::module::{Linkage, ValueDef, ValueId};
use llrm_mir::opcode::BinaryOp;

use super::*;
use crate::loopidioms::{trip_clocks, trip_cost};

/// How many times a loop of unknown trip count is assumed to run, as the
/// pin's branch probabilities do (`LBH_TAKEN_WEIGHT` 124 :
/// `LBH_NONTAKEN_WEIGHT` 4, a back edge taken 31 times in 32).
const ASSUMED_TRIPS: i64 = 32;

/// Whether `saved` clocks repay `added` bytes at `rate` thousandths of a clock
/// a byte: the one profit rule of a priced unroll, shared by every target.
pub(crate) fn unroll_pays(
    saved: i64,
    added: i64,
    rate: i64,
) -> bool {
    saved * 1000 >= added * rate
}

/// Whether the full unroll of `lp` by `trips` stays, at the target's rate
/// (`Tti::unroll_milliclocks_per_byte`). Only at -O2/-O3: -Os and -Oz hold to
/// the pin's bytes.
pub(super) fn full_unroll_pays(
    run: &mut Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    eph: &IdSet<InstId>,
    trips: u32,
    uce: &UnrollCostEstimator,
    up: &UnrollingPreferences,
) -> bool {
    let rate = run.tti.unroll_milliclocks_per_byte();
    // The early LoopFullUnrollPass unrolls a callee before the inliner sees it
    // (the pin's order): a veto in a function the inliner can take leaves a
    // small rolled helper that is then inlined, and unrolled, at every site
    // (jpeg_fixed -O2 +530 B). Such a loop is priced in the late pass, in the
    // context of each site. Where nothing follows to inline the function
    // (`-fno-inline-functions` with called-once off, or `noinline`) there is
    // no late pass for it: the loop is priced here (huffman_decode -O3: 17
    // calls of getbit unrolled unpriced, +915 B over gcc).
    let inlinable = !run.options.target_unrolling
        && run.options.inliner_follows
        && llrm_mir::facts::Facts::of(&run.function.attrs).inline() != Some(llrm_mir::facts::Inlining::Never)
        && run
            .this
            .is_some_and(|this| matches!(
                run.globals[this.0 as usize].linkage,
                Linkage::Internal | Linkage::Private
            ));
    if inlinable || rate <= 0 || run.options.opt_size || run.options.min_size || run.options.opt_level < 2 {
        why(run, lp, &format!("unpriced unroll: inlinable {inlinable}, rate {rate}"));
        return true;
    }
    // A loop of known trips in a nest of loops of known trips is a step to
    // unrolling the whole nest, whose counters are then all constants: what the
    // outer loops make constant (a table's element, a divisor) folds in the
    // copies and this loop's own analysis cannot see it (tests/run
    // int64_division: 7 x 6 divisions fold to constants; vetoed, 14 -> 198
    // instructions). The pin's threshold decides it.
    // A loop an unroll of this run made is part of a nest whose outer loop was
    // decided: the copies of the inner loops follow it (nbody -O2: the 4-trip
    // outer loop unrolls, its 3-, 2- and 1-trip inner copies were each priced
    // alone and kept rolled, +38,016).
    // Only where nests are priced: at -O3 the pin prices these copies (matmul
    // -O3 700 B; unpriced 1,208 B).
    let carried = run.decided.contains(&lp.get_header())
        && run.tti.unroll_prices_nest_loops_without_outer_counter()
        && run.options.opt_level < 3;
    if !run.first.contains(&lp.get_header()) || carried {
        why(run, lp, "unpriced unroll: a copy of this run");
        return true;
    }
    // Unless nothing in this loop depends on a counter of the loops around it:
    // then the nest's unroll folds nothing here and the loop is priced as any
    // other (crc -O2: the bit loop reads the running value and constants only,
    // 64 B added for 232 instructions saved).
    // -O3 is speed: the pin's decision holds there. A byte rate priced at -O3
    // (250 down to 3 clocks a byte) trades matmul 672 -> 5,009 instructions (or
    // 857 at 6) for 700 -> 292 B, and no rate keeps shellsort and jpeg at the
    // pin's bytes: DEVIATE ARM64-NEST-O3 (owner w15, exit 2026-11-30) leaves
    // the rows slower than gcc and clang at the pin's value.
    let prices_nest = run.tti.unroll_prices_nest_loops_without_outer_counter() && run.options.opt_level < 3;
    // DEVIATE ARM64-NEST-O3-INNER (owner arm64, exit 2026-11-24): at -O3 the
    // pin decides a loop inside a loop of known trips by its size alone, as it
    // decides the nest (below). Priced, the inner loop was vetoed while the
    // loop around it was unrolled, and its copies were then unrolled unpriced
    // ("a copy of this run"): fpbench -O3 grew 2,300 B (the pin's order leaves
    // the outer loop rolled around inner loops it unrolled: 1,412 B).
    if run.tti.unroll_pin_decides_inner_of_known_trips()
        && run.options.opt_level >= 3
        && lp.is_innermost()
        && lp.get_parent_loop().is_some_and(|parent| {
            let cx = llrm_analysis::scev::Cx {
                context: &*run.context,
                globals: run.globals,
                metadata: run.metadata,
                function: &*run.function,
                layout: run.layout,
                loops: &analyzed.loops,
                tree: &analyzed.tree,
                assumptions: &analyzed.assumptions,
            };
            let trips = se.get_small_constant_trip_count(&cx, parent.get_header());
            trips != 0 && trips <= up.full_unroll_max_count
        })
    {
        why(run, lp, "unpriced unroll: inside a loop of known trips");
        return true;
    }
    if encloses_known_trips(run, se, analyzed, lp, up.full_unroll_max_count)
        && (!prices_nest || reads_outer_counter(run, se, analyzed, lp, up))
    {
        why(run, lp, "unpriced unroll: the nest bypass");
        return true;
    }
    // The same for a loop around loops that all run few trips: the pin unrolls
    // the nest at its early pass, whole, and what that frees (the inner loops,
    // the arrays they index) is cleaned after it (nbody -O2 triangular nest:
    // +8 B for -23,816 instructions, priced +267 B). The bound is the pin's own
    // for what it analyzes, `-unroll-max-iteration-count-to-analyze`.
    if bounds_few_trips(run, se, analyzed, lp, up.max_iterations_count_to_analyze) {
        why(run, lp, "unpriced unroll: inner loops of few trips");
        return true;
    }
    let analysis = analyze_loop_unroll_cost(run, se, analyzed, lp, trips, eph, u32::MAX, trips, None, true);
    priced_verdict(&*run, se, analyzed, lp, up, trips, uce, analysis, None, prices_nest, prices_nest, rate)
}

/// The copies of each block a nest's unroll makes: the product of the trip
/// counts of the loops inside the nest's root that hold it.
pub(super) struct Projection {
    weight: IdMap<BlockId, i64>,
}

impl Projection {
    fn of(
        &self,
        block: BlockId,
    ) -> i64 {
        self.weight.get(&block).copied().unwrap_or(1)
    }
}

/// `root`'s nest as already unrolled inside it, when every loop in it runs a
/// known number of trips and the copies stay under a bound.
fn project_nest(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    root: Loop,
) -> Option<Projection> {
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut weight = IdMap::new();
    for &block in root.get_blocks() {
        let mut copies = 1i64;
        let mut around = analyzed.loops.get_loop_for(block);
        while let Some(one) = around {
            if one.get_header() == root.get_header() {
                break;
            }
            let trips = se.get_small_constant_trip_count(&cx, one.get_header());
            if trips == 0 {
                return None;
            }
            copies = copies.saturating_mul(i64::from(trips));
            if copies > 1 << 16 {
                return None;
            }
            around = one.get_parent_loop();
        }
        weight.insert(block, copies);
    }
    Some(Projection { weight })
}

/// The outermost loop above `lp`, itself included, whose chain down to `lp`
/// runs known trips.
pub(super) fn known_root<'a>(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop<'a>,
) -> Loop<'a> {
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut root = lp;
    while let Some(parent) = root.get_parent_loop() {
        if se.get_small_constant_trip_count(&cx, parent.get_header()) == 0 {
            break;
        }
        root = parent;
    }
    root
}

/// Whether the full unroll of the nest rooted at `known_root(lp)` pays, priced
/// with every loop inside unrolled; `None` where it has no known trips or
/// cannot be priced. `memory_aware` off ends the recursion through
/// `stays_in_memory`.
fn root_pays(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    up: &UnrollingPreferences,
    memory_aware: bool,
) -> Option<bool> {
    let root = known_root(run, se, analyzed, lp);
    // A nest of loops that all run few trips is the pin's, unpriced.
    if bounds_few_trips(run, se, analyzed, root, up.max_iterations_count_to_analyze) {
        return Some(true);
    }
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let trips = se.get_small_constant_trip_count(&cx, root.get_header());
    if trips == 0 {
        return None;
    }
    let projection = project_nest(run, se, analyzed, root)?;
    let eph = ephemeral_values(run, analyzed, root);
    let uce = cost_estimator(run, root, &eph, up.be_insns);
    if !uce.can_unroll() {
        return None;
    }
    let rate = run.tti.unroll_milliclocks_per_byte();
    let prices_nest = run.tti.unroll_prices_nest_loops_without_outer_counter();
    Some(priced_verdict(
        run,
        se,
        analyzed,
        root,
        up,
        trips,
        &uce,
        None,
        Some(&projection),
        prices_nest,
        memory_aware,
        rate,
    ))
}

/// Whether `lp` is a step of a nest of known trips whose whole unroll is under
/// `limit` units: the root's body at the copies the inner unrolls make
/// (`project_nest`), once more for each of its trips, as the pin's threshold
/// counts the outermost loop after the inner ones are gone. The loops DEVIATE
/// UNROLL-O1-TRIAL compiles again for: a loop that is no nest step goes alone,
/// and a nest the pin's own threshold refuses is not unrolled by the trial
/// either.
pub(super) fn nest_unroll_fits(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    up: &UnrollingPreferences,
    limit: u32,
) -> bool {
    let root = known_root(run, se, analyzed, lp);
    if root.get_header() == lp.get_header() {
        return false;
    }
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let trips = se.get_small_constant_trip_count(&cx, root.get_header());
    let Some(projection) = (trips != 0).then(|| project_nest(run, se, analyzed, root)).flatten() else { return false };
    let eph = ephemeral_values(run, analyzed, root);
    let uce = cost_estimator(run, root, &eph, up.be_insns);
    if !uce.can_unroll() {
        return false;
    }
    let bytes = run.outer.target().byte_costs();
    let blocks = root.get_blocks();
    let once = trip_cost(run.context, run.layout, run.function, run.callees, blocks.iter(), &bytes, true).max(1);
    let mut projected = 0;
    for block in blocks.iter() {
        projected += projection.of(*block)
            * trip_cost(run.context, run.layout, run.function, run.callees, std::iter::once(block), &bytes, true);
    }
    let units = (i64::from(uce.rolled_loop_size().max(1)) * projected.max(1) / once).max(1);
    let unrolled = ((units - i64::from(up.be_insns)).max(0)) * i64::from(trips) + i64::from(up.be_insns);
    unrolled < i64::from(limit)
}

/// The profit rule of a priced unroll of `lp` by `trips`, from the loop's
/// numbers: those of the rolled loop, or, with a `projection`, those of the
/// nest it roots as if its inner loops were already unrolled.
fn priced_verdict(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    up: &UnrollingPreferences,
    trips: u32,
    uce: &UnrollCostEstimator,
    analysis: Option<EstimatedUnrollCost>,
    projection: Option<&Projection>,
    prices_nest: bool,
    memory_aware: bool,
    rate: i64,
) -> bool {
    let clocks = crate::profit::costs(run.outer);
    let bytes = run.outer.target().byte_costs();
    let blocks = lp.get_blocks();
    let body_bytes = trip_cost(run.context, run.layout, run.function, run.callees, blocks.iter(), &bytes, true);
    let body_clocks = trip_clocks(run.context, run.layout, run.function, run.callees, blocks.iter(), &clocks);
    // A projected nest: every block counted once for each copy of it the inner
    // loops' unrolls make.
    let (body_bytes, body_clocks, grown) = match projection {
        None => (body_bytes, body_clocks, (1, 1)),
        Some(projection) => {
            let (mut projected_bytes, mut projected_clocks) = (0, 0);
            for block in blocks.iter() {
                let weight = projection.of(*block);
                let once = std::iter::once(block);
                projected_bytes +=
                    weight * trip_cost(run.context, run.layout, run.function, run.callees, once.clone(), &bytes, true);
                projected_clocks +=
                    weight * trip_clocks(run.context, run.layout, run.function, run.callees, once, &clocks);
            }
            (projected_bytes, projected_clocks, (projected_bytes.max(1), body_bytes.max(1)))
        }
    };
    // What the copies need not run: the instructions the analysis simplifies
    // (the counter, the exit test, folded loads), at the body's average clocks
    // a unit; where it cannot analyze (a loop around loops), the back edge of
    // every trip, and the operands the copies make constant (below).
    // The code the copies add, at the body's average bytes a unit: what the
    // analysis folds away is not emitted (byval_struct: a 41-trip loop of
    // constants is two instructions, not 41 copies).
    let rolled_units = (i64::from(uce.rolled_loop_size().max(1)) * grown.0 / grown.1).max(1);
    let unrolled_units = match (&analysis, projection) {
        (_, Some(_)) => ((rolled_units - i64::from(up.be_insns)).max(0)) * i64::from(trips) + i64::from(up.be_insns),
        (Some(cost), None) => i64::from(cost.unrolled_cost),
        (None, None) => uce.unrolled_loop_size(up, trips) as i64,
    };
    let added = match analysis.as_ref() {
        Some(cost) if cost.through_call => (cost.unrolled_bytes - body_bytes).max(0),
        _ => ((unrolled_units - rolled_units).max(0) * body_bytes) / rolled_units,
    };
    let (nested_uses, known) = constant_operand_uses(run, analyzed, lp);
    // The accesses of a local array the copies make constant-indexed: the
    // array is promoted to registers, so they cost no clocks and no bytes
    // (frameless: `local[i]` filled and read by two 12-trip loops).
    let (mut promoted, mut promoted_runs, ints, floats, forwarded) =
        promoted_accesses(run, se, analyzed, lp, &known, up, memory_aware, projection);
    let rmw = forwarded.len();
    // DEVIATE UP1-REG (owner m64, exit 2026-11-20): the promoted elements are
    // registers only while they fit: more than the file holds, and they are
    // stack slots again (fpbench -O2 m64: four 6-element float arrays, 24
    // values for 16 registers, were priced at 0 bytes and grew 815).
    //
    // DEVIATE UP1-X87 (owner x87, exit 2026-11-20): on an x87 target the floats
    // fit the stack (`x87_stack_slots`), not the integer file. nbody -O2 m32:
    // four 4-element arrays are 16 values for 8 slots; the nest was unrolled,
    // its values kept across the inner loops with 20 `fxch` and 11 `fst`
    // (381,454 clk; rolled 362,987, gcc 363,879).
    let fits_registers = run.tti.unroll_promotion_fits_registers();
    let x87_room = run.tti.x87_stack_slots().filter(|_| run.tti.unroll_promotion_fits_x87_stack()).map(i64::from);
    // The elements the copies hold past the stack's slots are stored and
    // loaded again every run of the loop (priced below).
    let spilled = x87_room.map_or(0, |room| x87_spilled(run, lp, &forwarded, trips, room));
    if fits_registers || x87_room.is_some() {
        let machine = run.outer.target();
        let float_room = x87_room.unwrap_or_else(|| {
            if machine.float_registers() > 0 { machine.float_registers() } else { machine.registers() }
        });
        let ints_over = fits_registers && ints as i64 * i64::from(trips) > machine.registers();
        let floats_over = if x87_room.is_some() { spilled > 0 } else { floats as i64 * i64::from(trips) > float_room };
        if ints_over || floats_over {
            promoted = 0;
            promoted_runs = 0;
        }
    }
    let added = (added - i64::from(trips) * promoted * bytes.load).max(0);
    let added = (added - merged_store_credit(run, se, analyzed, lp, trips)).max(0);
    let credited_operands =
        if analysis.is_none() && run.tti.unroll_credits_immediate_operands() { nested_uses } else { 0 };
    let (saved_units, constant_clocks) = match analysis {
        Some(cost) => (i64::from(cost.rolled_dynamic_cost.saturating_sub(cost.unrolled_cost)), 0),
        None => {
            let credited = if run.tti.unroll_credits_immediate_operands() { nested_uses } else { 0 };
            (i64::from(trips) * i64::from(up.be_insns), i64::from(trips) * credited * clocks.r#move)
        }
    };
    // The loop's control costs what it runs, never less than the body's average
    // unit: a taken back edge is several clocks (the i486's `jne` 3), the
    // counter's step and the compare one each (lru: a 3-trip loop saved 6
    // clocks by the average, 25 run).
    let control = (i64::from(trips) - 1) * clocks.branch + i64::from(trips) * 2 * clocks.add;
    let averaged = i64::from(trips) * (i64::from(up.be_insns) + 1) * body_clocks / rolled_units;
    let constant_clocks = constant_clocks + (control - averaged).max(0);
    let constant_clocks = constant_clocks + i64::from(trips) * promoted_runs * clocks.load;
    // The loop runs once for each trip of the loops around it, which the
    // bytes do not: the saving is counted over those runs (x_cordic: the
    // 12-trip loop inside a loop of `n` runs `n` times, 89 clocks each).
    let runs = if prices_nest {
        enclosing_runs(run, se, analyzed, lp)
    } else {
        if run.tti.unroll_assumes_unknown_loops_run() { ASSUMED_TRIPS.pow((lp.get_loop_depth() - 1).min(2)) } else { 1 }
    };
    // DEVIATE UP2-X87 (owner x87, exit 2026-11-20): an element past the stack
    // is stored and loaded again each run (nbody -O2 m32: the unroll priced
    // to save 44 clocks a run ran 92 more: 14 more stores, 17 more exchanges).
    let spill_clocks = spilled * (clocks.float_store + clocks.float_load);
    // DEVIATE THROUGHPUT (owner x86-m64, exit 2026-12-20): see `throughput`. A
    // target with a throughput model prices a trip at the larger of its
    // resource and chain bounds, and the unroll saves the resources it
    // removes only where the trip is bound by them. A trip that calls a
    // function the inliner takes next is outside the model: the copies make its
    // arguments constants and the callee folds on them, work the trip's own
    // instructions do not show (dot -O3 m64: the 40-trip loop around
    // `dot(a, b)` saved "0 clk", and rolled left the callee's 48 products
    // unfolded, +306 B and 87 -> 170 clocks a call). Such a trip is priced
    // serially. Only a target with the model asks: the other targets'
    // pricing is untouched.
    let sees_trip = |_: &_| {
        !blocks
            .iter()
            .any(
                |&block| run.function
                    .iter_block(block)
                    .any(
                        |inst| matches!(
                            run.function.instruction(inst).opcode,
                            Opcode::Call(_) | Opcode::Invoke(_)
                        )
                            && call_inlined_next(run, inst),
                    ),
            )
    };
    let (saved, bound) = match run.outer.target().throughput().filter(sees_trip) {
        None => ((saved_units * body_clocks / rolled_units + constant_clocks - spill_clocks) * runs, (0, 0)),
        Some(tp) => {
            let weight = |block: BlockId| projection.map_or(1, |projection| projection.of(block));
            let trip = throughput::trip(run, &blocks, tp, &weight);
            let (trips, be) = (i64::from(trips), i64::from(up.be_insns));
            let units = rolled_units.max(1);
            let removed: Vec<i64> = tp
                .resources
                .iter()
                .zip(&trip.load)
                .map(|(resource, load)| {
                    let costs = &resource.costs;
                    let unit = load / units;
                    let control = (trips - 1) * costs.branch + trips * 2 * costs.add;
                    let averaged = trips * (be + 1) * unit;
                    saved_units * unit
                        + (control - averaged).max(0)
                        + trips * promoted_runs * costs.load
                        + trips * credited_operands * costs.r#move
                })
                .collect();
            let milli = throughput::saved(tp, &trip, trips, &removed);
            (((milli + 500) / 1000 - spill_clocks) * runs, (throughput::resource_bound(tp, &trip.load), trip.chain))
        }
    };
    why(
        run,
        lp,
        &Priced {
            trips,
            saved,
            added,
            body_bytes,
            units: unrolled_units,
            rolled: rolled_units,
            promoted,
            ints,
            floats,
            rmw,
            spilled,
            float_file: run.outer.target().float_registers(),
            rate,
            saved_units,
            unit_clocks: body_clocks / rolled_units,
            constant_clocks,
            runs,
            resource: bound.0,
            chain: bound.1,
        }
        .line(),
    );
    unroll_pays(saved, added, rate)
}

/// How many times `lp` runs for one entry of its outermost loop: the product
/// of the trip counts of the loops around it, each its constant count where
/// the analysis has one, else `ASSUMED_TRIPS`, the pin's branch probability
/// (nbody_single -O2: the 6-trip loop inside the 1000-trip loop ran 32 times
/// by the assumption, 1000 by its count: 17,408 clocks saved against 72,250
/// the bytes cost, kept rolled, +236,000 instructions).
fn enclosing_runs(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
) -> i64 {
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let (mut runs, mut assumed): (i64, u32) = (1, 0);
    let mut around = lp.get_parent_loop();
    while let Some(parent) = around {
        let trips = se.get_small_constant_trip_count(&cx, parent.get_header());
        if trips != 0 {
            runs = runs.saturating_mul(i64::from(trips)).min(1 << 24);
        } else if run.tti.unroll_assumes_unknown_loops_run() && assumed < 2 {
            // As before: at most two unknown loops around count.
            assumed += 1;
            runs *= ASSUMED_TRIPS;
        }
        around = parent.get_parent_loop();
    }
    runs
}

/// What a priced unroll weighed, the line of `LLRM_DEBUG=unroll`: `floats` and
/// `ints` are the local arrays whose elements the copies make registers, and
/// `float_file` the target's stated float register file (0 where unstated).
pub(super) struct Priced {
    pub trips: u32,
    pub saved: i64,
    pub added: i64,
    pub body_bytes: i64,
    pub units: i64,
    pub rolled: i64,
    pub promoted: i64,
    pub ints: usize,
    pub floats: usize,
    pub rmw: usize,
    pub spilled: i64,
    pub float_file: i64,
    pub rate: i64,
    /// The terms of `saved`: units the analysis folds, the body's clocks a
    /// unit, the clocks the control and the promoted loads add, the runs.
    pub saved_units: i64,
    pub unit_clocks: i64,
    pub constant_clocks: i64,
    pub runs: i64,
    /// The trip's resource and chain bounds in milli-clocks (0 where the target
    /// states no throughput model).
    pub resource: i64,
    pub chain: i64,
}

impl Priced {
    pub(super) fn line(&self) -> String {
        format!(
            "priced unroll: {} trips, saves {} clk, adds {} B (body {} B; units {} of {}, promoted {}, ints {}, floats {}, rmw {}, spilled {}), float file {}, rate {}; saved units {} at {} clk, constant {} clk, runs {}; trip resource {} chain {} mclk",
            self.trips,
            self.saved,
            self.added,
            self.body_bytes,
            self.units,
            self.rolled,
            self.promoted,
            self.ints,
            self.floats,
            self.rmw,
            self.spilled,
            self.float_file,
            self.rate,
            self.saved_units,
            self.unit_clocks,
            self.constant_clocks,
            self.runs,
            self.resource,
            self.chain
        )
    }
}

/// The operands of the loops inside `lp` that each copy of its body makes a
/// constant, one for each time they run (a loop of unknown trips runs
/// `ASSUMED_TRIPS`): a value computed from the counters of `lp` alone is a
/// constant in a copy, and its readers take an immediate, not a register
/// (x_ld_clamp: `8 + r` a bound of the inner compare, the stack's `fxch`).
fn constant_operand_uses(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
) -> (i64, IdSet<ValueId>) {
    let function = &*run.function;
    let header = lp.get_header();
    let (Some(preheader), Some(latch)) = (lp.get_loop_preheader(function), lp.get_loop_latch(function)) else {
        return (0, IdSet::new());
    };
    let depth = lp.get_loop_depth();
    let own: Vec<InstId> = lp
        .get_blocks()
        .iter()
        .filter(|&&block| analyzed.loops.get_loop_depth(block) == depth)
        .flat_map(|&block| function.iter_block(block).collect::<Vec<_>>())
        .collect();
    let incoming = |inst: InstId, from: BlockId| {
        function.instruction(inst).operands.chunks(2).find(|pair| pair[1] == Operand::Block(from)).map(|pair| pair[0])
    };
    // The counters start at constants; keep those whose step is computed from
    // them alone.
    let mut counters: Vec<InstId> = function
        .iter_block(header)
        .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        .filter(|&inst| matches!(incoming(inst, preheader), Some(Operand::Constant(_))))
        .collect();
    let known = loop {
        let mut known: IdSet<ValueId> = counters.iter().filter_map(|&inst| function.instruction(inst).result).collect();
        let mut grew = true;
        while grew {
            grew = false;
            for &inst in &own {
                let one = function.instruction(inst);
                // An element of a constant table at the counters is a constant
                // in a copy.
                let element = matches!(one.opcode, Opcode::Load { volatile: false, .. })
                    && matches!(
                        local_array_at(run, one.operands[0], &known),
                        Some(Base::Table)
                    );
                let pure = element
                    || matches!(
                        one.opcode,
                        Opcode::FNeg
                            | Opcode::Binary(_)
                            | Opcode::Cast(_)
                            | Opcode::ICmp(_)
                            | Opcode::FCmp(_)
                            | Opcode::Select
                            | Opcode::GetElementPtr { .. }
                    )
                    || matches!(one.opcode, Opcode::Call(_))
                        && crate::licm::hoist::is_safe_to_speculatively_execute_in(
                            run.context,
                            run.layout,
                            function,
                            run.outer,
                            inst,
                        );
                let Some(result) = one.result else { continue };
                if pure
                    && !known.contains(&result)
                    && one.operands.iter().all(|operand| match operand {
                        Operand::Value(value) => known.contains(value),
                        Operand::Constant(_) => true,
                        Operand::Block(_) => false,
                    })
                {
                    known.insert(result);
                    grew = true;
                }
            }
        }
        let before = counters.len();
        counters.retain(|&inst| match incoming(inst, latch) {
            Some(Operand::Value(value)) => known.contains(&value),
            Some(Operand::Constant(_)) => true,
            _ => false,
        });
        if counters.len() == before {
            break known;
        }
    };
    let mut uses = 0;
    for &block in lp.get_blocks() {
        let nested = analyzed.loops.get_loop_depth(block).saturating_sub(depth);
        if nested == 0 {
            continue;
        }
        let weight = ASSUMED_TRIPS.pow(nested.min(2));
        for inst in function.iter_block(block) {
            let reads = function
                .instruction(inst)
                .operands
                .iter()
                .any(|operand| matches!(operand, Operand::Value(value) if known.contains(value)));
            if reads {
                uses += weight;
            }
        }
    }
    (uses, known)
}

/// The stack slots a full unroll of `lp` costs beyond what the code already
/// needs. DEVIATE UP2-X87 (owner x87, exit 2026-11-20): the arrays `lp` both
/// reads and writes (`forwarded`) leave one live value per trip from the loop
/// to their next read, and those values share the stack with whatever else is
/// live in the blocks between: the peak of the float values live there, plus
/// the copies' elements, against the slots the stack has (`room`) and against
/// the peak the rolled function already has. nbody -O2 m32: the update loop's
/// 2 arrays x 4 trips lie across the pair loops (peak 6 rolled, 14 unrolled,
/// 4,005 `fxch`); fpbench -O2 m32: 2 arrays x 6 trips lie across straight-line
/// code (peak 17 either way).
fn x87_spilled(
    run: &Run,
    lp: Loop,
    forwarded: &IdSet<ValueId>,
    trips: u32,
    room: i64,
) -> i64 {
    let function = &*run.function;
    let is_float = |value: ValueId| {
        llrm_mir::tti::Ty::of(&run.context.types, function.value(value).ty).is_some_and(|ty| ty.is_float())
    };
    let rolled = llrm_analysis::liveness::pressure_by_block(function, &|value| usize::from(is_float(value)));
    let peak_rolled = rolled.iter().map(|&(_, peak)| peak as i64).max().unwrap_or(0);
    let inside: IdSet<BlockId> = lp.get_blocks().iter().copied().collect();
    // The blocks that run after the loop: from its exits, round any loop
    // around it.
    let mut after: IdSet<BlockId> = IdSet::new();
    let mut work: Vec<BlockId> = lp.get_exit_blocks(function);
    while let Some(block) = work.pop() {
        if after.insert(block) {
            work.extend(function.successors(block));
        }
    }
    // The blocks from which an array is read again, found backwards from
    // those reads outside the loop.
    let reads_of = |root: ValueId| -> Vec<BlockId> {
        function
            .layout()
            .iter()
            .copied()
            .filter(|block| !inside.contains(block))
            .filter(|&block| {
                function.iter_block(block).any(|inst| {
                    let one = function.instruction(inst);
                    matches!(one.opcode, Opcode::Load { .. }) && alloca_of(function, one.operands[0]) == Some(root)
                })
            })
            .collect()
    };
    let mut held: Vec<IdSet<BlockId>> = Vec::new();
    for root in forwarded.iter() {
        let mut before: IdSet<BlockId> = IdSet::new();
        let mut work = reads_of(root);
        while let Some(block) = work.pop() {
            if before.insert(block) {
                work.extend(function.predecessors(block));
            }
        }
        let mut span: IdSet<BlockId> = after.iter().filter(|block| before.contains(block)).collect();
        span.extend(inside.iter());
        held.push(span);
    }
    let elements = i64::from(trips);
    let peak_unrolled = rolled
        .iter()
        .map(|&(block, peak)| peak as i64 + elements * held.iter().filter(|span| span.contains(&block)).count() as i64)
        .max()
        .unwrap_or(0);
    (peak_unrolled - peak_rolled.max(room)).max(0)
}

/// The alloca `pointer` addresses through geps, whatever their indices.
fn alloca_of(
    function: &Function,
    mut pointer: Operand,
) -> Option<ValueId> {
    loop {
        let Operand::Value(value) = pointer else { return None };
        let ValueDef::Instruction(def) = function.value(value).def else { return None };
        let one = function.instruction(def);
        match one.opcode {
            Opcode::Alloca { .. } => return Some(value),
            Opcode::GetElementPtr { .. } => pointer = one.operands[0],
            _ => return None,
        }
    }
}

/// The loads and stores in the blocks of `lp` (and, as many as they run, those
/// of the loops inside it) of a local array that does not
/// escape, at an address computed from the counters of `lp` (`known`) alone: in
/// a full unroll each is at a constant offset and the array is promoted.
fn promoted_accesses(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    known: &IdSet<ValueId>,
    up: &UnrollingPreferences,
    memory_aware: bool,
    projection: Option<&Projection>,
) -> (i64, i64, usize, usize, IdSet<ValueId>) {
    let function = &*run.function;
    let mut resident: std::collections::BTreeMap<u32, bool> = Default::default();
    let (mut found, mut runs) = (0, 0);
    let (mut ints, mut floats): (IdSet<ValueId>, IdSet<ValueId>) = (IdSet::new(), IdSet::new());
    // Float arrays the loop both reads and writes: after a full unroll their
    // elements are forwarded from one trip of the loops around to the next,
    // one live value each.
    let (mut loaded, mut written, mut rmw): (IdSet<ValueId>, IdSet<ValueId>, IdSet<ValueId>) =
        (IdSet::new(), IdSet::new(), IdSet::new());
    for &block in lp.get_blocks() {
        let nested = analyzed.loops.get_loop_depth(block).saturating_sub(lp.get_loop_depth());
        for inst in function.iter_block(block) {
            let one = function.instruction(inst);
            let pointer = match one.opcode {
                Opcode::Load { volatile: false, .. } => one.operands[0],
                Opcode::Store { volatile: false, .. } => one.operands[1],
                _ => continue,
            };
            let free = match local_array_at(run, pointer, known) {
                // A read of a constant table at the counter is the table's
                // element in a copy.
                Some(Base::Table) => matches!(one.opcode, Opcode::Load { .. }),
                Some(Base::Local(root)) => llrm_analysis::capturetracking::pointer_may_be_captured(
                    function,
                    run.globals,
                    run.context,
                    root,
                    llrm_analysis::capturetracking::CaptureComponents::PROVENANCE,
                    |cc| {
                        cc.intersect(llrm_analysis::capturetracking::CaptureComponents::PROVENANCE)
                            == llrm_analysis::capturetracking::CaptureComponents::PROVENANCE
                    },
                    llrm_analysis::capturetracking::DEFAULT_MAX_USES_TO_EXPLORE,
                )
                .without_ret
                .captures_nothing(),
                None => false,
            };
            // A store the array survives is still run: the array is promoted
            // only where every other access is a constant offset
            // too.
            let free = free
                && !(memory_aware
                    && matches!(one.opcode, Opcode::Store { .. })
                    && matches!(
                        local_array_at(run, pointer, known),
                        Some(Base::Local(root)) if *resident.entry(root.0).or_insert_with(|| stays_in_memory(run, se, analyzed, lp, root, up))
                    ));
            if free {
                let weight = projection.map_or(1, |projection| projection.of(block));
                found += weight;
                runs += projection.map_or(ASSUMED_TRIPS.pow(nested.min(2)), |_| weight);
                if let Some(Base::Local(root)) = local_array_at(run, pointer, known) {
                    let stored = match one.opcode {
                        Opcode::Store { .. } => function.operand_type(run.context, one.operands[0]),
                        _ => one.result.map(|result| function.value(result).ty),
                    };
                    let float = stored
                        .and_then(|ty| llrm_mir::tti::Ty::of(&run.context.types, ty))
                        .is_some_and(|ty| ty.is_float());
                    if float {
                        floats.insert(root);
                        if matches!(one.opcode, Opcode::Store { .. }) {
                            written.insert(root)
                        } else {
                            loaded.insert(root)
                        };
                        if loaded.contains(&root) && written.contains(&root) {
                            rmw.insert(root);
                        }
                    } else if !immediate_store(run, se, analyzed, one, known) {
                        ints.insert(root);
                    }
                }
            }
        }
    }
    (found, runs, ints.len(), floats.len(), rmw)
}

/// A store of an integer that is a constant in each copy (a constant, or a
/// value of the counters alone) and fits the target's store immediate
/// (`Tti::unroll_store_immediate_bits`; none: never): the reader takes it as an
/// immediate, so the element holds no register. UP1-REG counts the elements
/// that do: a float is loaded from the constant pool, an element the loop also
/// loads is a live value, a constant outside the immediate's range
/// is loaded into a register first.
fn immediate_store(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    inst: &llrm_mir::module::Instruction,
    known: &IdSet<ValueId>,
) -> bool {
    let bits = run.tti.unroll_store_immediate_bits();
    let operand = inst.operands[0];
    let constant = match operand {
        Operand::Constant(_) => true,
        Operand::Value(value) => known.contains(&value),
        Operand::Block(_) => false,
    };
    if bits == 0 || !matches!(inst.opcode, Opcode::Store { .. }) || !constant {
        return false;
    }
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    if !se.is_value_scevable(&cx, operand) {
        return false;
    }
    let scev = se.get_scev(&cx, operand);
    se.get_signed_range(&cx, scev).get_min_signed_bits() <= bits
}

/// Whether a local array keeps its memory after the full unroll of `lp`: some
/// access outside `lp` is addressed by a value the unroll leaves a variable,
/// one of a loop that stays rolled (it has no constant trip count, or its
/// nest, priced as a whole, is not unrolled). The
/// stores of `lp` into it are then still run (matmul -O2: the init nest fills
/// `a` and `b`, read by the rolled `k` loop at run-time indices; the credit
/// for 128 stores grew the function 660 B against gcc's 320).
fn stays_in_memory(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    root: ValueId,
    up: &UnrollingPreferences,
) -> bool {
    let function = &*run.function;
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut finishes: std::collections::BTreeMap<u32, bool> = Default::default();
    // Only a read that can run after the loop keeps the stores (nbody -O2: the
    // last update loop, after the step loop, is followed by reads of
    // constants).
    let mut after: IdSet<BlockId> = IdSet::new();
    let mut work: Vec<BlockId> = lp.get_exit_blocks(function);
    while let Some(block) = work.pop() {
        if after.insert(block) {
            work.extend(function.successors(block));
        }
    }
    for &block in function.layout() {
        if lp.get_blocks().contains(&block) || !after.contains(&block) {
            continue;
        }
        for inst in function.iter_block(block) {
            let one = function.instruction(inst);
            let pointer = match one.opcode {
                Opcode::Load { .. } => one.operands[0],
                _ => continue,
            };
            if alloca_of(function, pointer) != Some(root) {
                continue;
            }
            // The indices' sources: every loop whose counter feeds the address.
            let mut work: Vec<Operand> = Vec::new();
            let mut at = pointer;
            while let Operand::Value(value) = at {
                let ValueDef::Instruction(def) = function.value(value).def else { break };
                let gep = function.instruction(def);
                if !matches!(gep.opcode, Opcode::GetElementPtr { .. }) {
                    break;
                }
                work.extend(gep.operands[1..].iter().copied());
                at = gep.operands[0];
            }
            let mut seen: IdSet<ValueId> = IdSet::new();
            while let Some(operand) = work.pop() {
                let value = match operand {
                    Operand::Constant(_) => continue,
                    Operand::Block(_) => continue,
                    Operand::Value(value) => value,
                };
                if !seen.insert(value) {
                    continue;
                }
                let ValueDef::Instruction(def) = function.value(value).def else { return true };
                let source = function.instruction(def);
                match source.opcode {
                    Opcode::Phi => {
                        let header = function.parent(def);
                        let counter = header.and_then(|header| {
                            analyzed.loops.get_loop_for(header).filter(|one| one.get_header() == header)
                        });
                        if let Some(counter) = counter {
                            let id = counter.get_header().0;
                            // A loop an unroll of this run made is part of a
                            // nest decided already;
                            // else the nest it belongs to is priced as a whole.
                            let fits = *finishes
                                .entry(id)
                                .or_insert_with(
                                    || !run.first.contains(&counter.get_header())
                                        || run.decided.contains(&counter.get_header())
                                        || root_pays(run, se, analyzed, counter, up, false).unwrap_or(false),
                                );
                            if !fits {
                                return true;
                            }
                        }
                        work.extend(source.operands.iter().copied());
                    }
                    Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::Select => {
                        work.extend(source.operands.iter().copied());
                    }
                    _ => return true,
                }
            }
        }
    }
    false
}

/// The float values a full unroll of `lp` by `trips` keeps live across its
/// copies: each float array the loop both reads and writes (a local one or a
/// global, at an address of the loop's counters) is one value a trip, forwarded
/// from copy to copy; one both read and written at a fixed address, one value.
pub(super) fn forwarded_floats(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
    trips: u32,
) -> usize {
    let (_, known) = constant_operand_uses(run, analyzed, lp);
    let function = &*run.function;
    // (loaded, stored) per root: an alloca, or a global's id.
    let mut roots: std::collections::BTreeMap<(bool, u32), (bool, bool, bool)> = Default::default();
    for &block in lp.get_blocks() {
        for inst in function.iter_block(block) {
            let one = function.instruction(inst);
            let (pointer, stored) = match one.opcode {
                Opcode::Load { volatile: false, .. } => (one.operands[0], false),
                Opcode::Store { volatile: false, .. } => (one.operands[1], true),
                _ => continue,
            };
            let ty = if stored {
                function.operand_type(run.context, one.operands[0])
            } else {
                one.result.map(|result| function.value(result).ty)
            };
            if !ty.and_then(|ty| llrm_mir::tti::Ty::of(&run.context.types, ty)).is_some_and(|ty| ty.is_float()) {
                continue;
            }
            let Some((root, varying)) = array_root(run, pointer, &known) else { continue };
            let entry = roots.entry(root).or_default();
            if stored {
                entry.1 = true
            } else {
                entry.0 = true
            }
            entry.2 |= varying;
        }
    }
    roots
        .values()
        .filter(|(loaded, stored, _)| *loaded && *stored)
        .map(|(_, _, varying)| if *varying { trips as usize } else { 1 })
        .sum()
}

/// The alloca, or mutable global, `pointer` addresses through geps whose
/// indices are constants or in `known`, and whether an index varies.
fn array_root(
    run: &Run,
    mut pointer: Operand,
    known: &IdSet<ValueId>,
) -> Option<((bool, u32), bool)> {
    let function = &*run.function;
    let mut varying = false;
    loop {
        match pointer {
            Operand::Constant(id) => {
                let ConstantKind::Global(global) = run.context.get(id).kind else { return None };
                return match &run.globals[global.0 as usize].kind {
                    GlobalKind::Variable(variable) if !variable.constant => Some(((true, global.0), varying)),
                    _ => None,
                };
            }
            Operand::Value(value) => {
                let ValueDef::Instruction(def) = function.value(value).def else { return None };
                let one = function.instruction(def);
                match one.opcode {
                    Opcode::Alloca { .. } => return Some(((false, value.0), varying)),
                    Opcode::GetElementPtr { .. } => {
                        for index in &one.operands[1..] {
                            match index {
                                Operand::Value(index) if known.contains(index) => varying = true,
                                Operand::Constant(_) => {}
                                _ => return None,
                            }
                        }
                        pointer = one.operands[0];
                    }
                    _ => return None,
                }
            }
            Operand::Block(_) => return None,
        }
    }
}

/// The bytes the copies of a full unroll save by storing one constant at
/// consecutive addresses: the stores of `trips` trips are adjacent, and the
/// backend merges them into wider stores and `stp` pairs
/// (`MergeConsecutiveStores`; lru -O3: the model said +28 B, the code shrank
/// 12). A stream of `trips` stores of `w` bytes becomes `ceil(trips * w / 16)`
/// instructions of four bytes; a constant that is neither zero nor all ones
/// costs one more to build.
fn merged_store_credit(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    trips: u32,
) -> i64 {
    if trips < 2 {
        return 0;
    }
    let function = &*run.function;
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut credit = 0;
    for &block in lp.get_blocks() {
        if analyzed.loops.get_loop_depth(block) != lp.get_loop_depth() {
            continue;
        }
        for inst in function.iter_block(block) {
            let one = function.instruction(inst);
            let Opcode::Store { volatile: false, .. } = one.opcode else { continue };
            let Operand::Constant(value) = one.operands[0] else { continue };
            let Some(llrm_mir::tti::Ty::Int(bits)) = function
                .operand_type(run.context, one.operands[0])
                .and_then(|ty| llrm_mir::tti::Ty::of(&run.context.types, ty))
            else {
                continue;
            };
            if !matches!(bits, 8 | 16 | 32 | 64) || !se.is_value_scevable(&cx, one.operands[1]) {
                continue;
            }
            let width = i128::from(bits / 8);
            let address = se.get_scev(&cx, one.operands[1]);
            if se.addrec_header(address) != Some(lp.get_header()) || !se.is_affine(address) {
                continue;
            }
            let step = se.get_step_recurrence(&cx, address);
            if se.as_constant(step).map(|step| step.sext_value()) != Some(width) {
                continue;
            }
            let merged = (i64::from(trips) * i64::from(bits / 8) + 15) / 16;
            let mask = (1u128 << bits) - 1;
            let plain = matches!(
                run.context.get(value).kind,
                ConstantKind::Int(number) if number & mask == 0 || number & mask == mask
            );
            credit += (i64::from(trips) - merged) * 4 - if plain { 0 } else { 4 };
        }
    }
    credit.max(0)
}

/// What an address at the counters is an element of.
enum Base {
    Local(ValueId),
    Table,
}

/// The alloca, or constant global, `pointer` addresses through geps whose
/// indices are constants or in `known`.
fn local_array_at(
    run: &Run,
    mut pointer: Operand,
    known: &IdSet<ValueId>,
) -> Option<Base> {
    let function = &*run.function;
    let mut indexed = false;
    loop {
        match pointer {
            Operand::Constant(id) => {
                let ConstantKind::Global(global) = run.context.get(id).kind else { return None };
                return match &run.globals[global.0 as usize].kind {
                    GlobalKind::Variable(variable)
                        if variable.constant && variable.initializer.is_some() && indexed =>
                    {
                        Some(Base::Table)
                    }
                    _ => None,
                };
            }
            Operand::Value(value) => {
                let ValueDef::Instruction(def) = function.value(value).def else { return None };
                let one = function.instruction(def);
                match one.opcode {
                    Opcode::Alloca { .. } => return Some(Base::Local(value)),
                    Opcode::GetElementPtr { .. } => {
                        let varying = one.operands[1..].iter().any(|index| matches!(index, Operand::Value(_)));
                        let counted = one
                            .operands[1..]
                            .iter()
                            .all(
                                |index| match index {
                                    Operand::Value(index) => known.contains(index),
                                    Operand::Constant(_) => true,
                                    Operand::Block(_) => false,
                                },
                            );
                        if !counted {
                            return None;
                        }
                        indexed |= varying;
                        pointer = one.operands[0];
                    }
                    _ => return None,
                }
            }
            Operand::Block(_) => return None,
        }
    }
}

/// Whether `lp` is innermost and has an enclosing loop, and every loop around
/// it runs a known number of trips, at most `most`.
fn encloses_known_trips(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    most: u32,
) -> bool {
    if !lp.is_innermost() || lp.get_parent_loop().is_none() {
        return false;
    }
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut around = lp.get_parent_loop();
    while let Some(parent) = around {
        let trips = se.get_small_constant_trip_count(&cx, parent.get_header());
        if trips == 0 || trips > most {
            return false;
        }
        around = parent.get_parent_loop();
    }
    true
}

/// Whether unrolling the whole nest around `lp` folds real work: a value the
/// nest makes constant (every loop-carried value that starts known and is
/// stepped from known values) that is data, not an address: it is divided, a
/// float is made of it, it selects, it indexes a constant table, or it is
/// stored, passed or returned. An index sum that only addresses memory or ends
/// the loop does not (matmul -O2: `i * 8 + k`). tests/run int64_division: the
/// divisors are the outer counters; stft -O2: the bit-reverse of the counter
/// is stored.
fn reads_outer_counter(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    up: &UnrollingPreferences,
) -> bool {
    let function = &*run.function;
    let Some(outermost) = std::iter::successors(lp.get_parent_loop(), |parent| parent.get_parent_loop()).last() else {
        return false;
    };
    let blocks = outermost.get_blocks();
    let inside = lp.get_blocks();
    // Every loop's header phis, with the block they start from and the one
    // that steps them.
    let loops: Vec<Loop> = std::iter::once(lp)
        .chain(std::iter::successors(lp.get_parent_loop(), |parent| parent.get_parent_loop()))
        .collect();
    let mut seeds: Vec<(InstId, Operand, Operand)> = Vec::new();
    for one in &loops {
        let (Some(preheader), Some(latch)) = (one.get_loop_preheader(function), one.get_loop_latch(function)) else {
            return false;
        };
        for inst in
            function.iter_block(one.get_header()).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
        {
            let pairs: Vec<&[Operand]> = function.instruction(inst).operands.chunks(2).collect();
            let start = pairs.iter().find(|pair| pair[1] == Operand::Block(preheader)).map(|pair| pair[0]);
            let step = pairs.iter().find(|pair| pair[1] == Operand::Block(latch)).map(|pair| pair[0]);
            if let (Some(start), Some(step)) = (start, step) {
                seeds.push((inst, start, step));
            }
        }
    }
    let headers: Vec<InstId> = seeds.iter().map(|&(inst, ..)| inst).collect();
    let (known, real) = loop {
        let mut known: IdSet<ValueId> = IdSet::new();
        let mut real: Vec<ValueId> = Vec::new();
        let mut grew = true;
        while grew {
            grew = false;
            let all_known = |operands: &[Operand], known: &IdSet<ValueId>| {
                operands
                    .iter()
                    .all(
                        |&operand| match operand {
                            Operand::Constant(_) => true,
                            Operand::Value(value) => known.contains(&value),
                            Operand::Block(_) => true,
                        },
                    )
            };
            for &(inst, start, _) in &seeds {
                let Some(result) = function.instruction(inst).result else { continue };
                if !known.contains(&result) && all_known(&[start], &known) {
                    known.insert(result);
                    grew = true;
                }
            }
            for &block in blocks.iter() {
                for inst in function.iter_block(block) {
                    if headers.contains(&inst) {
                        continue;
                    }
                    let one = function.instruction(inst);
                    let Some(result) = one.result else { continue };
                    if known.contains(&result) {
                        continue;
                    }
                    let element = matches!(one.opcode, Opcode::Load { volatile: false, .. })
                        && matches!(
                            local_array_at(run, one.operands[0], &known),
                            Some(Base::Table)
                        );
                    let divides = matches!(
                        one.opcode,
                        Opcode::Binary(
                            BinaryOp::UDiv
                                | BinaryOp::SDiv
                                | BinaryOp::URem
                                | BinaryOp::SRem
                                | BinaryOp::FAdd
                                | BinaryOp::FSub
                                | BinaryOp::FMul
                                | BinaryOp::FDiv
                                | BinaryOp::FRem
                        ) | Opcode::Select
                            | Opcode::FCmp(_)
                    );
                    let pure = element
                        || divides
                        || matches!(
                            one.opcode,
                            Opcode::Binary(_)
                                | Opcode::Cast(_)
                                | Opcode::ICmp(_)
                                | Opcode::GetElementPtr { .. }
                                | Opcode::Phi
                        );
                    let has_value = one.operands.iter().any(|&operand| matches!(operand, Operand::Value(_)));
                    if pure && has_value && all_known(&one.operands, &known) {
                        known.insert(result);
                        if (element || divides) && inside.contains(&block) {
                            real.push(result);
                        }
                        grew = true;
                    }
                }
            }
        }
        // A loop-carried value is constant only if what steps it is.
        let before = seeds.len();
        seeds.retain(|&(_, _, step)| match step {
            Operand::Value(value) => known.contains(&value),
            _ => true,
        });
        if seeds.len() == before {
            break (known, real);
        }
        let _ = &headers;
    };
    // Which of the constants are data: read by something that is not
    // addressing or counting, or by a computation of such a value.
    let mut data: IdSet<ValueId> = IdSet::new();
    let mut resident: std::collections::BTreeMap<u32, bool> = Default::default();
    let mut grew = true;
    while grew {
        grew = false;
        for &block in blocks.iter() {
            for inst in function.iter_block(block) {
                let one = function.instruction(inst);
                let passes = matches!(
                    one.opcode,
                    Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::Phi | Opcode::GetElementPtr { .. }
                );
                // DEVIATE ARM64-UNROLL-PRICE (owner w15, exit 2026-11-30): a
                // load reads its address, which is addressing,
                // not data (tile -O2: `tile[(y + dy) & 63]
                // [(x + dx) & 63]` bypassed the pricing for 364 B and 2,155
                // instructions where the rolled loop is 68 B
                // and 4,181).
                let mut sink = !passes && !matches!(one.opcode, Opcode::Br | Opcode::Load { .. });
                // A constant passed to a call nothing will inline folds in no
                // copy: the call still runs and takes its
                // arguments in registers (huffman_decode
                // -O2 m64: `report(decode(t))` in a 12 x 4 nest, 850 B against
                // gcc's 212).
                sink &=
                    !matches!(one.opcode, Opcode::Call(_) | Opcode::Invoke(_)) || call_may_fold_arguments(run, inst);
                // DEVIATE ARM64-UNROLL-PRICE: nor a store into a local array
                // that stays in memory, which the copies still
                // run (matmul -O2: `a` and `b` filled by `(i +
                // j) % 3`, read by the rolled `k` loop).
                if sink
                    && matches!(one.opcode, Opcode::Store { .. })
                    && let Some(Base::Local(root)) = local_array_at(run, one.operands[1], &known)
                    && *resident
                        .entry(root.0)
                        .or_insert_with(|| stays_in_memory(run, se, analyzed, outermost, root, up))
                {
                    sink = false;
                }
                let reads_data = if passes { one.result.is_some_and(|result| data.contains(&result)) } else { sink };
                if !reads_data {
                    continue;
                }
                for &operand in &one.operands {
                    if let Operand::Value(value) = operand
                        && known.contains(&value)
                        && data.insert(value)
                    {
                        grew = true;
                    }
                }
            }
        }
    }
    // Data the copies of `lp` itself make constant.
    real.iter().any(|value| data.contains(value))
        || inside.iter().any(|&block| {
            function
                .iter_block(block)
                .any(
                    |inst| {
                        let one = function.instruction(inst);
                        one.opcode != Opcode::Phi && one.result.is_some_and(|result| data.contains(&result))
                    },
                )
        })
}

/// Whether the arguments of the call `inst` can fold once they are constants:
/// an intrinsic folds them, a function with a body does where the inliner still
/// runs and may take it, and any other callee (a declaration, a `noinline`
/// function, an indirect call's target) runs on its arguments as they come.
fn call_may_fold_arguments(
    run: &Run,
    inst: InstId,
) -> bool {
    let Some(id) = llrm_mir::memory::callee(run.context, run.function, inst) else { return false };
    // A callee declared after the pass's snapshot of the module is past
    // `run.globals`.
    let Some(global) = run.globals.get(id.0 as usize) else { return false };
    if global.name.as_deref().is_some_and(|name| name.starts_with("llvm.")) {
        return true;
    }
    call_inlined_next(run, inst)
}

/// Whether the call `inst` is to a function with a body that the inliner, still
/// to run, may take.
fn call_inlined_next(
    run: &Run,
    inst: InstId,
) -> bool {
    let Some(id) = llrm_mir::memory::callee(run.context, run.function, inst) else { return false };
    let Some(global) = run.globals.get(id.0 as usize) else { return false };
    let llrm_mir::module::GlobalKind::Function(callee) = &global.kind else { return false };
    // The late pass runs after the inliner took what it would take of this
    // function's calls.
    run.options.inliner_follows
        && !run.options.target_unrolling
        && run.callees.get(&id).is_some_and(|summary| summary.defined)
        && llrm_mir::facts::Facts::of(&callee.attrs).inline() != Some(llrm_mir::facts::Inlining::Never)
}

/// Whether `lp` holds a loop whose trip count follows a counter of `lp` (a
/// constant maximum, no constant count: a triangular nest's inner loop) and
/// every loop inside it runs at most `most` trips. A loop inside that runs a
/// constant number of trips whatever `lp` does is priced on its own.
fn bounds_few_trips(
    run: &Run,
    se: &mut ScalarEvolution,
    analyzed: &Analyzed,
    lp: Loop,
    most: u32,
) -> bool {
    if lp.is_innermost() {
        return false;
    }
    let cx = llrm_analysis::scev::Cx {
        context: &*run.context,
        globals: run.globals,
        metadata: run.metadata,
        function: &*run.function,
        layout: run.layout,
        loops: &analyzed.loops,
        tree: &analyzed.tree,
        assumptions: &analyzed.assumptions,
    };
    let mut follows = false;
    let mut inside: Vec<Loop> = lp.get_sub_loops().collect();
    while let Some(one) = inside.pop() {
        let trips = se.get_small_constant_max_trip_count(&cx, one.get_header());
        if trips == 0 || trips > most {
            return false;
        }
        follows |= se.get_small_constant_trip_count(&cx, one.get_header()) == 0;
        inside.extend(one.get_sub_loops());
    }
    follows
}
