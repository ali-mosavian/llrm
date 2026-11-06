//! DEVIATE UP1 (owner x86-m32; exit 2026-11-20): a full unroll the pin's
//! threshold allows is made only where the clocks it saves repay the bytes it
//! adds. The pin's bar counts instructions and never asks what a copy of the
//! body costs in code (x_ld_max: the 8-trip inner loop, copied 8 times,
//! 232 -> 755 bytes at m32 -O2 for a few clocks).

use llrm_mir::module::{Linkage, ValueDef, ValueId};

use super::*;
use crate::loopidioms::{trip_clocks, trip_cost};

/// How many times a loop of unknown trip count is assumed to run, as the
/// pin's branch probabilities do (`LBH_TAKEN_WEIGHT` 124 : `LBH_NONTAKEN_WEIGHT`
/// 4, a back edge taken 31 times in 32).
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
    // context of each site.
    let inlinable = !run.options.target_unrolling
        && run.this.is_some_and(|this| matches!(run.globals[this.0 as usize].linkage, Linkage::Internal | Linkage::Private));
    if inlinable || rate <= 0 || run.options.opt_size || run.options.min_size || run.options.opt_level < 2 {
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
    if !run.first.contains(&lp.get_header()) {
        return true;
    }
    if encloses_known_trips(run, se, analyzed, lp, up.full_unroll_max_count) {
        return true;
    }
    // The same for a loop around loops that all run few trips: the pin unrolls
    // the nest at its early pass, whole, and what that frees (the inner loops,
    // the arrays they index) is cleaned after it (nbody -O2 triangular nest:
    // +8 B for -23,816 instructions, priced +267 B). The bound is the pin's own
    // for what it analyzes, `-unroll-max-iteration-count-to-analyze`.
    if bounds_few_trips(run, se, analyzed, lp, up.max_iterations_count_to_analyze) {
        return true;
    }
    let clocks = crate::profit::costs(run.outer);
    let bytes = run.outer.target().byte_costs();
    let blocks = lp.get_blocks();
    let body_bytes = trip_cost(run.context, run.layout, run.function, run.callees, blocks.iter(), &bytes, true);
    let body_clocks = trip_clocks(run.context, run.layout, run.function, run.callees, blocks.iter(), &clocks);
    // What the copies need not run: the instructions the analysis simplifies
    // (the counter, the exit test, folded loads), at the body's average clocks
    // a unit; where it cannot analyze (a loop around loops), the back edge of
    // every trip, and the operands the copies make constant (below).
    let analysis = analyze_loop_unroll_cost(
        run,
        se,
        analyzed,
        lp,
        trips,
        eph,
        u32::MAX,
        trips,
        None,
        true,
    );
    // The code the copies add, at the body's average bytes a unit: what the
    // analysis folds away is not emitted (byval_struct: a 41-trip loop of
    // constants is two instructions, not 41 copies).
    let rolled_units = i64::from(uce.rolled_loop_size().max(1));
    let unrolled_units = analysis
        .as_ref()
        .map_or(uce.unrolled_loop_size(up, trips) as i64, |cost| i64::from(cost.unrolled_cost));
    let added = match analysis.as_ref() {
        Some(cost) if cost.through_call => (cost.unrolled_bytes - body_bytes).max(0),
        _ => ((unrolled_units - rolled_units).max(0) * body_bytes) / rolled_units,
    };
    let (nested_uses, known) = constant_operand_uses(run, analyzed, lp);
    // The accesses of a local array the copies make constant-indexed: the
    // array is promoted to registers, so they cost no clocks and no bytes
    // (frameless: `local[i]` filled and read by two 12-trip loops).
    let (mut promoted, mut promoted_runs, ints, floats, rmw) = promoted_accesses(run, analyzed, lp, &known);
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
    if fits_registers || x87_room.is_some() {
        let machine = run.outer.target();
        let float_room = x87_room.unwrap_or_else(|| {
            if machine.float_registers() > 0 { machine.float_registers() } else { machine.registers() }
        });
        let ints_over = fits_registers && ints as i64 * i64::from(trips) > machine.registers();
        if ints_over || floats as i64 * i64::from(trips) > float_room {
            promoted = 0;
            promoted_runs = 0;
        }
    }
    let added = (added - i64::from(trips) * promoted * bytes.load).max(0);
    let added = (added - merged_store_credit(run, se, analyzed, lp, trips)).max(0);
    let (saved_units, constant_clocks) = match analysis {
        Some(cost) => (i64::from(cost.rolled_dynamic_cost.saturating_sub(cost.unrolled_cost)), 0),
        None => (i64::from(trips) * i64::from(up.be_insns), i64::from(trips) * nested_uses * clocks.r#move),
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
    let runs = ASSUMED_TRIPS.pow((lp.get_loop_depth() - 1).min(2));
    let saved = (saved_units * body_clocks / rolled_units + constant_clocks) * runs;
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
            float_file: run.outer.target().float_registers(),
            rate,
        }
        .line(),
    );
    unroll_pays(saved, added, rate)
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
    pub float_file: i64,
    pub rate: i64,
}

impl Priced {
    pub(super) fn line(&self) -> String {
        format!(
            "priced unroll: {} trips, saves {} clk, adds {} B (body {} B; units {} of {}, promoted {}, ints {}, floats {}, rmw {}), float file {}, rate {}",
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
            self.float_file,
            self.rate
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
                    && matches!(local_array_at(run, one.operands[0], &known), Some(Base::Table));
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
                ) || matches!(one.opcode, Opcode::Call(_))
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

/// The loads and stores in the blocks of `lp` (and, as many as they run, those of
/// the loops inside it) of a local array that does not
/// escape, at an address computed from the counters of `lp` (`known`) alone: in
/// a full unroll each is at a constant offset and the array is promoted.
fn promoted_accesses(
    run: &Run,
    analyzed: &Analyzed,
    lp: Loop,
    known: &IdSet<ValueId>,
) -> (i64, i64, usize, usize, usize) {
    let function = &*run.function;
    let (mut found, mut runs) = (0, 0);
    let (mut ints, mut floats): (IdSet<ValueId>, IdSet<ValueId>) = (IdSet::new(), IdSet::new());
    // Float arrays the loop both reads and writes: after a full unroll their elements
    // are forwarded from one trip of the loops around to the next, one live value each.
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
            if free {
                found += 1;
                runs += ASSUMED_TRIPS.pow(nested.min(2));
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
                        if matches!(one.opcode, Opcode::Store { .. }) { written.insert(root) } else { loaded.insert(root) };
                        if loaded.contains(&root) && written.contains(&root) {
                            rmw.insert(root);
                        }
                    } else {
                        ints.insert(root);
                    }
                }
            }
        }
    }
    (found, runs, ints.len(), floats.len(), rmw.len())
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
            if stored { entry.1 = true } else { entry.0 = true }
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
/// backend merges them into wider stores and `stp` pairs (`MergeConsecutiveStores`;
/// lru -O3: the model said +28 B, the code shrank 12). A stream of `trips`
/// stores of `w` bytes becomes `ceil(trips * w / 16)` instructions of four
/// bytes; a constant that is neither zero nor all ones costs one more to build.
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
            let plain = matches!(run.context.get(value).kind, ConstantKind::Int(number) if number & mask == 0 || number & mask == mask);
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
                    GlobalKind::Variable(variable) if variable.constant && variable.initializer.is_some() && indexed => {
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
                        let counted = one.operands[1..].iter().all(|index| match index {
                            Operand::Value(index) => known.contains(index),
                            Operand::Constant(_) => true,
                            Operand::Block(_) => false,
                        });
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
