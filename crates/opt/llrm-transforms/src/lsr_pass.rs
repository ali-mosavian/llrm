//! LLVM's loop strength reduction as a pass (`LoopStrengthReduce`,
//! `ReduceLoopStrength` and `LSRInstance::LSRInstance`;
//! LoopStrengthReduce.cpp at `d1106deb71cc`, l.6307-6430 and 7187-7265).
//!
//! # How it works
//! Each innermost loop of simplify form, innermost first, is taken once: its IV
//! users are found ([`crate::ivusers`]), they become uses and fixups with a
//! first formula ([`crate::lsr_collect`]), the formulae are generated and
//! narrowed ([`crate::lsr_generate`], [`crate::lsr_narrow`]), and the solver
//! picks one per use ([`crate::lsr_solve`]). If it picks, the code is expanded
//! ([`crate::lsr_implement`]) and what the replaced operands leave unread goes.
//! The costs are the target's ([`llrm_mir::tti::Tti`]).
//!
//! # Example
//! A local array filled and summed in a loop, exit `i + 1 == n`, becomes a
//! pointer phi stepping by 4 and a counter from `%n - 1` to 0; the interpreter
//! agrees on every input.
//!
//! # Runs
//! In the pipeline's strength slot, after `fixednarrow`.
//!
//! # Limits
//! DEVIATE: not yet ported: the debug value salvage. `DeleteDeadPHIs` reads
//! the header's phis only. `replaceCongruentIVs` runs on every loop in
//! simplify form, as `ReduceLoopStrength` does. A loop of more than 200 IV
//! users is left (`MaxIVUsers`).

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::{ScevSession, Scevs};
use llrm_analysis::scev::expander::{Env as ExpandEnv, Expander};
use llrm_analysis::scev::expr::{Scev, Type};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::tti::Tti;

use crate::ivusers::{Env, IvUsers};
use crate::lsr_collect::Collection;
use crate::lsr_implement::{Implement, Plan, split_phi_edges};
use crate::lsr_prepare::{self, ExitEdit, ShadowEdit};

/// Whether `operand` is computed with a multiply the target prices as worth
/// reducing in an outer loop (`Tti::reduces_outer_multiply`), through adds,
/// subtracts and casts to `depth` levels.
fn computed_by_priced_multiply(
    tti: &dyn Tti,
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    operand: Operand,
    depth: u32,
) -> bool {
    let Operand::Value(value) = operand else { return false };
    let ValueDef::Instruction(inst) = function.value(value).def else { return false };
    let inst = function.instruction(inst);
    match inst.opcode {
        Opcode::Binary(BinaryOp::Mul) => {
            let constant = match inst.operands.get(1) {
                Some(&Operand::Constant(constant)) => match context.get(constant).kind {
                    ConstantKind::Int(bits) => Some(bits),
                    _ => None,
                },
                _ => None,
            };
            tti.reduces_outer_multiply(constant)
        }
        // An index of a GEP is multiplied by the size of the element it steps
        // over: `g[i]` over rows of 54 bytes is `i * 54`.
        Opcode::GetElementPtr { source } => {
            let size = layout.alloc_size(&context.types, source);
            let scaled = size > 1
                && matches!(inst.operands.get(1), Some(Operand::Value(_)))
                && tti.reduces_outer_multiply(Some(u128::from(size)));
            scaled
                || depth > 0
                    && inst
                        .operands
                        .iter()
                        .any(|&one| computed_by_priced_multiply(tti, context, layout, function, one, depth - 1))
        }
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) | Opcode::Cast(_) if depth > 0 => {
            inst.operands.iter().any(|&one| computed_by_priced_multiply(tti, context, layout, function, one, depth - 1))
        }
        _ => false,
    }
}

#[cfg(test)]
thread_local! {
    /// How many sessions (dominator tree, loops and assumptions) the pass made.
    pub static SESSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `MaxIVUsers`.
const MAX_IV_USERS: usize = 200;

/// LLVM's loop strength reduction. `size`: the compile is for size, which the
/// exit-value rewrite after it reads (`exitvalues` DEVIATE 15).
/// `bytes_strict`: code bytes outrank clocks (-O1, -Os), which DEVIATE LG3's
/// keep rule reads: a base added outside the loop counts against the
/// instructions it saves once, not once a trip.
pub struct LoopStrengthReduce {
    pub size: bool,
    pub bytes_strict: bool,
}

impl FunctionPass for LoopStrengthReduce {
    fn name(&self) -> &'static str {
        "lsr"
    }

    /// True: the pin's `LoopStrengthReducePass::run` returns
    /// `getLoopPassPreservedAnalyses()`, which does not preserve GlobalsAA. A
    /// pointer induction variable over a global array makes its address
    /// escape, so the module's `GlobalsModRef` is worked out again after `lsr`.
    fn adds_memory_operations(&self) -> bool {
        true
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        // The expressions of the passes before, as the pin keeps one SE for
        // the loop passes; handed back below where the CFG is as it was.
        // Beyond the pin: a counter only some trips step gets a pointer first,
        // where the target's address takes no scale and asks for it.
        if !self.size
            && tti.mirrors_conditional_counters()
            && crate::mirror::mirrored(
                unit.context,
                unit.layout,
                unit.function,
                &outer,
                &outer.target().address_forms(),
            )
        {
            analyses.invalidate(unit.function, &PreservedAnalyses::none());
        }
        let held = analyses.get::<Scevs>(unit.context, unit.layout, unit.function);
        let (changed, scev) = reduced_held(
            unit.context,
            unit.layout,
            unit.function,
            unit.declared,
            &outer,
            &*tti,
            false,
            self.size,
            self.bytes_strict,
            Some(held.take()),
        );
        let kept = scev.is_some();
        if let Some(scev) = scev {
            held.put(scev);
        }
        match (changed, kept) {
            (false, _) => PreservedAnalyses::all(),
            (true, true) => PreservedAnalyses::none().preserve::<Scevs>(),
            (true, false) => PreservedAnalyses::none(),
        }
    }
}

/// The `llvm.assume` calls of the blocks.
fn assume_calls(
    context: &Context,
    outer: &Outer,
    function: &Function,
    blocks: &[BlockId],
) -> Vec<InstId> {
    blocks
        .iter()
        .flat_map(|&block| function.iter_block(block))
        .filter(|&inst| {
            llrm_mir::memory::callee(context, function, inst)
                .and_then(|callee| outer.globals.get(callee.0 as usize))
                .is_some_and(|global| global.name.as_deref() == Some("llvm.assume"))
        })
        .collect()
}

/// What the constructor of `LSRInstance` makes of a loop.
enum Outcome {
    /// The loop is left as it is, or this is its plan.
    Planned(Option<Plan>),
    /// Exit tests to edit first; the loop is then analysed again.
    Edit(Vec<ExitEdit>, IvUsers),
    /// An integer variable's conversion to float, made a variable of its own.
    Shadow(ShadowEdit),
}

/// `DominatorTree::findNearestCommonDominator(Instruction *, Instruction *)`.
fn nearest_common_dominator(
    cx: &llrm_analysis::scev::Cx,
    a: InstId,
    b: InstId,
) -> InstId {
    let (block_a, block_b) = (cx.function.parent(a).expect("placed"), cx.function.parent(b).expect("placed"));
    if block_a == block_b {
        return if cx.tree.instruction_dominates(cx.function, a, b) { a } else { b };
    }
    let common = cx.tree.find_nearest_common_dominator(cx.function, block_a, block_b).expect("a common dominator");
    if common == block_a {
        a
    } else if common == block_b {
        b
    } else {
        cx.function.terminator(common).expect("a terminator")
    }
}

/// DEVIATE IU7: the loop is planned twice when the exit tests are edited: the
/// first look (`marked` none) finds the edits, the second builds `IvUsers`
/// again over the edited loop and marks the tests with `transformToPostInc`.
/// The pin keeps one `IVUsers` through `OptimizeLoopTermCond`. Reason: the edit
/// changes the IR that `IvUsers` was built over. Cost: d_faces -O2, 38
/// constructions against the pin's 32, about 0.5 M of 16.8 M instructions (3%).
/// Exit: reuse `IvUsers` across the IR edit. Owner w-5. Review 2026-11-19.
/// What `LSRInstance`'s constructor decides for the loop with header `header`:
/// the plan, or none where the loop is left as it is. `marked` are the exit
/// tests already edited to take the post-incremented value; none until the
/// first look has found edits to make.
#[allow(clippy::too_many_arguments)]
/// Function flag: LSR chose end-biased formulae (DEVIATE LG3) that differ from
/// the pin's solution somewhere in the function.
pub const END_BIASED_FIRED: &str = "llrm-end-biased";
/// Function flag, set by a driver that builds the function both ways: LSR makes
/// no end-biased formulae for it.
pub const NO_END_BIASED: &str = "llrm-no-end-biased";

fn plan_loop(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    outer: &Outer,
    tti: &dyn Tti,
    se: &mut ScalarEvolution,
    env: &ExpandEnv,
    header: BlockId,
    marked: Option<&[InstId]>,
    carried: Option<IvUsers>,
    shadowed: bool,
    stress_chains: bool,
    size: bool,
    bytes_strict: bool,
) -> Outcome {
    let planned = |plan| Outcome::Planned(plan);
    let cx = env.cx(context, function);
    let Some(lp) = cx.loops.get_loop_for(header) else { return planned(None) };
    // Loop simplify form, or stay out of trouble.
    if lp.get_loop_preheader(function).is_none()
        || lp.get_loop_latch(function).is_none()
        || !lp.has_dedicated_exits(function)
    {
        return planned(None);
    }
    let mut rewriter = Expander::new("lsr");
    rewriter.set_canonical_mode(false);
    rewriter.enable_lsr_mode();

    let assumes = assume_calls(context, outer, function, lp.get_blocks());
    let effects_free = |inst: InstId| llrm_mir::memory::only_value(context, outer.callees(), function, inst);
    let ephemeral =
        llrm_mir::codemetrics::collect_ephemeral_values(function, &effects_free, &assumes, &|block| lp.contains(block));
    let safe_to_speculate =
        |inst: InstId| crate::licm::hoist::is_safe_to_speculatively_execute_in(context, layout, function, outer, inst);
    let is_ephemeral = |inst: InstId| ephemeral.contains(&inst);
    let env_users = Env {
        safe_to_speculate: &safe_to_speculate,
        ephemeral: &is_ephemeral,
        native_widths: tti.lsr_native_integer_widths(),
        use_list_order: tti.lsr_users_in_use_list_order(),
        phi_operand_uses: tti.lsr_phi_operand_uses(),
    };
    let mut users = match carried {
        Some(users) => {
            users.assert_current(se, &cx, &env_users);
            users
        }
        None => IvUsers::of(se, &cx, header, &env_users),
    };
    // No interesting work, or too much analysis to model it.
    if users.uses.is_empty() || users.uses.len() > MAX_IV_USERS {
        return planned(None);
    }
    // The exit tests: the first look finds what to edit, the second marks the
    // tests it made.
    let mut iv_inc_insert_pos =
        function.terminator(lp.get_loop_latch(function).expect("a latch")).expect("a terminator");
    match marked {
        None => {
            // `OptimizeShadowIV` makes one variable and breaks (DEVIATE LS-SH:
            // more where floats live on the x87 stack).
            if let Some(shadow) = lsr_prepare::analyze_shadow(se, &cx, tti, header, &users, shadowed) {
                return Outcome::Shadow(shadow);
            }
            let edits = lsr_prepare::analyze(se, &cx, tti, header, &users);
            if edits.iter().any(|edit| edit.max.is_some() || edit.post_inc) {
                return Outcome::Edit(edits, users);
            }
        }
        Some(conds) => {
            for &cond in conds {
                users.transform_to_post_inc(cond, header);
                iv_inc_insert_pos = nearest_common_dominator(&cx, iv_inc_insert_pos, cond);
            }
        }
    }

    // Skip nested loops until we can model them better with formulae: the
    // shadow variables and the exit tests above are made on every loop, as the
    // pin's constructor does before it looks at the nesting.
    // DEVIATE LSR-OUTER (owner x87, exit 2026-11-27; the pin skips every loop
    // that has a subloop): where the target prices it
    // (`Tti::reduces_outer_multiply`: the multiply's clocks against a
    // counter held across the inner loop) and not at -Os, an outer loop
    // that computes an affine value with a multiply is reduced as
    // an inner loop is, so `i * 300` feeding the inner loop becomes a counter
    // stepped by 300. gcc's ivopts does this on every loop (named behaviour
    // only, its code is not translated). x_cordic -O1: 200 `imul` x 19
    // clocks.
    if !lp.is_innermost()
        && !(!size
            && users.uses.iter().any(|one| computed_by_priced_multiply(tti, context, layout, function, one.operand, 3)))
    {
        return planned(None);
    }

    // Chains are looked for where the number of registers is what costs.
    let chains = if tti.is_num_regs_major_cost_of_lsr() {
        crate::lsr_chain::collect_chains(se, &cx, header, &users, &is_ephemeral, stress_chains)
    } else {
        Default::default()
    };
    let safe_to_expand = |se: &mut ScalarEvolution, s: Scev| rewriter.is_safe_to_expand(&cx, se, s);
    // DEVIATE LU13: the loop calls and the target's call takes every register.
    let across_call = tti.lsr_call_clobbers_registers() && crate::lsr_collect::loop_calls(&cx, tti, header);
    // The loop's register units do not change between the two builds.
    let units = std::cell::Cell::new(None);
    let build = |se: &mut ScalarEvolution| {
        let mut lsr = Collection::new(&cx, tti, header, &safe_to_expand, false, iv_inc_insert_pos);
        lsr.other_register_units = match units.get() {
            Some(counted) => counted,
            None => {
                let counted = crate::lsr_collect::loop_register_units(se, &cx, tti, header);
                units.set(Some(counted));
                counted
            }
        };
        lsr.size = size;
        lsr.bytes_counted = size || bytes_strict;
        lsr.across_call = across_call;
        llrm_support::debug!("lsr", "  other registers: {}", lsr.other_register_units);
        lsr.chain_uses = chains.inc_set.clone();
        lsr.collect_interesting_types_and_factors(se, &users);
        lsr.collect_fixups_and_initial_formulae(se, &users);
        lsr.collect_loop_invariant_fixups_and_formulae(se);
        lsr
    };
    let counts =
        |lsr: &Collection| lsr.uses.iter().map(|one| one.formulae.len().to_string()).collect::<Vec<_>>().join(" ");
    let narrowed = |se: &mut ScalarEvolution, lsr: &mut Collection| {
        llrm_support::debug!("lsr", "  formulae per use: {}", counts(lsr));
        lsr.filter_out_undesirable_dedicated_registers(se);
        llrm_support::debug!("lsr", "  after filtering: {}", counts(lsr));
        lsr.narrow_search_space_using_heuristics(se);
        llrm_support::debug!("lsr", "  after narrowing: {}", counts(lsr));
    };
    let mut lsr = build(se);
    if lsr.uses.is_empty() {
        return planned(None);
    }
    // The formulae for each use, and the search space made small enough.
    lsr.generate_all_reuse_formulae(se);
    // The pin's space as generated, for the second solve below.
    let mut pin = lsr.fork();
    let pin_counts = counts(&lsr);
    // DEVIATE LG3's formulae (beyond the pin) join the space before its
    // narrowing, as the formulae they share registers with do.
    // `llrm-no-end-biased`: the driver builds this function without them.
    if !llrm_mir::memory::has(&function.attrs, NO_END_BIASED) {
        lsr.generate_end_biased(se);
    }
    let beyond = counts(&lsr) != pin_counts;
    let mut differs = false;
    narrowed(se, &mut lsr);
    let (mut solution, cost) = lsr.solve_with_cost(se);
    if beyond {
        // The registers of those formulae can cut the pin's solution out of
        // the search (a use must reuse the registers already chosen), as they
        // did queens -O2 (+57859 clocks, 236848 against 170823 at -Os): the
        // pin's space is solved too, and its solution kept where it costs
        // less than the one with them, or where theirs adds a base to the
        // counter in an address the pin's does not, on a target that
        // cannot fold it: they save a compare, which that add outweighs
        // (wasm's copy epilog, jpeg_fixed `emit`), or where theirs saves
        // neither a counter nor a register: the pin's formula already shares
        // the exit test's, and a hoisted end base is only setup (wasm sieve,
        // whose stores merge into one, -O2 507 B against the pin's 500).
        narrowed(se, &mut pin);
        let (pin_solution, pin_cost) = pin.solve_with_cost(se);
        // A target with no register-plus-register address (wasm) pays an add
        // for each base added to the counter, and the code it adds outside the
        // loop must not outweigh the instructions it saves in it (nbody -O1:
        // four `base + 32` against one compare).
        let folds_base_and_counter = tti.is_legal_addressing_mode(false, 0, true, 1);
        let spent = cost.c.insns + cost.c.setup_cost;
        let saved = pin_cost.c.insns + pin_cost.c.setup_cost;
        // DEVIATE LG3: the `spent` against `saved` test above, over the trips
        // the loop is known to run, with the symbol bases the end-biased
        // solution adds to the pin's priced as the target's setup says
        // (`setup_cost` counts each as one instruction). Where bytes outrank
        // clocks (-O1, -Os) the loop is counted once, whatever its trips: an
        // instruction saved in it is one instruction of code, as a base is.
        let symbol_setup = tti.lsr_symbol_base_setup_insns();
        let symbols_outweigh_saving = symbol_setup != 0 && {
            let new_bases = symbol_bases(se, &lsr, &solution);
            let pin_bases = symbol_bases(se, &pin, &pin_solution);
            let added = new_bases.iter().filter(|base| !pin_bases.contains(base)).count() as u32;
            let trips = if bytes_strict { 1 } else { u64::from(se.get_small_constant_max_trip_count(&cx, header)) };
            let total = |insns: u32, setup: u32| trips * u64::from(insns) + u64::from(setup);
            trips != 0
                && total(cost.c.insns, cost.c.setup_cost) + u64::from(added * (symbol_setup - 1))
                    > total(pin_cost.c.insns, pin_cost.c.setup_cost)
        };
        // DEVIATE LG3, where bytes outrank clocks (-Os, -O1): the end-biased
        // solution is refused when the target's byte table prices it larger
        // than the pin's. At -O1 clocks are the other axis: it stays when it
        // saves loop instructions (x_radix -O1: a compare saved, 1578 clk
        // for 37 B), and is refused when it adds any.
        let bytes_larger = cost.size_bytes > pin_cost.size_bytes;
        let saves_insns = cost.c.insns < pin_cost.c.insns;
        let bytes_outweigh_saving = bytes_strict
            && tti.lsr_size_model().is_some()
            && if size { bytes_larger } else { cost.c.insns > pin_cost.c.insns || (bytes_larger && !saves_insns) };
        let kept = !pin_solution.is_empty()
            && (solution.is_empty()
                || symbols_outweigh_saving
                || bytes_outweigh_saving
                || pin_cost.is_less(tti, &cost)
                || (!folds_base_and_counter
                    && (cost.c.num_base_adds > pin_cost.c.num_base_adds
                        || spent > saved
                        || !(cost.c.add_rec_cost < pin_cost.c.add_rec_cost || cost.c.num_regs < pin_cost.c.num_regs))));
        llrm_support::debug!(
            "lsr",
            "  pin's space solved: kept {kept} (bytes {} against the pin's {}; size_bytes {} against {}; insns {} against {})",
            cost.bytes,
            pin_cost.bytes,
            cost.size_bytes,
            pin_cost.size_bytes,
            cost.c.insns,
            pin_cost.c.insns
        );
        if kept {
            lsr = pin;
            solution = pin_solution;
        } else {
            differs = !pin_solution.is_empty();
        }
    }
    if solution.is_empty() {
        return planned(None);
    }
    let mut plan = lsr.into_plan(solution);
    plan.end_biased_differs = differs;
    plan.chains = chains.chains;
    planned(Some(plan))
}

/// The distinct base registers of `solution`'s formulae that name a global's
/// address.
fn symbol_bases(
    se: &ScalarEvolution,
    lsr: &Collection,
    solution: &[usize],
) -> Vec<Scev> {
    fn names_global(
        se: &ScalarEvolution,
        lsr: &Collection,
        s: Scev,
        depth: u32,
    ) -> bool {
        match se.scev_type(s) {
            Type::Unknown => lsr.global_of(se, s).is_some(),
            _ if depth == 0 => false,
            Type::Add | Type::Mul | Type::AddRec => {
                se.operands(s).iter().any(|operand| names_global(se, lsr, operand.scev, depth - 1))
            }
            _ => false,
        }
    }
    let mut bases = Vec::new();
    for (lu, &chosen) in lsr.uses.iter().zip(solution) {
        for &base in &lu.formulae[chosen].base_regs {
            if !bases.contains(&base) && names_global(se, lsr, base, 4) {
                bases.push(base);
            }
        }
    }
    bases
}

/// `ReduceLoopStrength` over every loop of `function`, innermost first;
/// whether anything changed.
pub fn reduced(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    reduced_with(context, layout, function, declared, outer, tti, false)
}

/// [`reduced`], with every IV chain made if `stress_chains`
/// (`-lsr-stress-ivchain`): for tests of the chains' generation.
pub fn reduced_with(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    tti: &dyn Tti,
    stress_chains: bool,
) -> bool {
    reduced_held(context, layout, function, declared, outer, tti, stress_chains, false, false, None).0
}

/// [`reduced_with`], continuing the expressions `held` of the passes before;
/// they are handed back, current, where no edit changed the CFG.
#[allow(clippy::too_many_arguments)]
pub fn reduced_held(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
    tti: &dyn Tti,
    stress_chains: bool,
    size: bool,
    bytes_strict: bool,
    held: Option<ScalarEvolution>,
) -> (bool, Option<ScalarEvolution>) {
    // The pass requires `LoopSimplify` (`getAnalysisUsage`): an outer loop
    // entered by a conditional branch gets the preheader its invariants'
    // induction variables are made in.
    let mut changed = crate::loopsimplify::simplified(function);
    // The held expressions are read in the CFG they were made in.
    let mut stable = !changed;
    // The dominator tree, loops and assumptions of the function are made once:
    // an edit of the loop does not change the CFG. Only splitting an edge does;
    // then they are made again.
    let new_session = |context: &Context, function: &Function| {
        #[cfg(test)]
        SESSIONS.with(|made| made.set(made.get() + 1));
        ScevSession::within(context, &outer.globals, &outer.metadata, function)
    };
    let mut session = new_session(context, function);
    if let (Some(scev), true) = (held, stable) {
        session.adopt(scev);
    }
    // Beyond the pin: a remainder by a constant that the target keeps as a
    // divide is one instruction (the divide's own remainder); read as
    // `x - (x / c) * c` it splits into the address's other terms, and the
    // divide is paid beside a multiply and an induction variable per use.
    let divide_kept = remainders_kept_as_divides(context, function, outer, size);
    let mark_opaque = |session: &mut ScevSession, context: &Context, function: &Function| {
        for &value in &divide_kept {
            session.ask(context, layout, function, outer, |se, cx| {
                se.set_opaque(cx, llrm_mir::module::Operand::Value(value), true)
            });
        }
    };
    mark_opaque(&mut session, context, function);
    // Where the function stood when the expressions were last made current.
    let mut mark = function.mark();
    let headers: Vec<BlockId> = {
        let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
        // The order the code generator's legacy loop pass manager runs LSR in.
        env.loops.get_loops_in_legacy_visit_order().iter().map(|one| one.get_header()).collect()
    };
    for header in headers {
        llrm_mir::counts::bump(llrm_mir::counts::Event::LsrLoops);
        // The first look may find exit tests to edit, and the plan may split
        // edges: each is followed by an analysis of the loop as it is then.
        let mut marked: Option<Vec<InstId>> = None;
        // The users of the first look, kept through the edit of the exit tests.
        let mut carried: Option<IvUsers> = None;
        // The one shadow variable the loop gets.
        let mut shadowed = false;
        for _ in 0..6 {
            // What the edits since changed is no longer what the expressions
            // say; the rest of them stay, as they do in LLVM.
            session.forget_since_keeping(context, layout, &outer.globals, &outer.metadata, function, mark, true);
            mark = function.mark();
            let split;
            {
                let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
                let plan = match plan_loop(
                    context,
                    layout,
                    function,
                    outer,
                    tti,
                    se,
                    &env,
                    header,
                    marked.as_deref(),
                    carried.take(),
                    shadowed,
                    stress_chains,
                    size,
                    bytes_strict,
                ) {
                    Outcome::Planned(None) => break,
                    Outcome::Edit(edits, mut users) => {
                        llrm_mir::counts::bump(llrm_mir::counts::Event::LsrPrepared);
                        let applied = lsr_prepare::apply(function, &context.types, header, &edits);
                        marked = Some(applied.marked.clone());
                        // A moved or copied test is the same use; `OptimizeMax`
                        // makes a new compare, so its users are made again.
                        if edits.iter().all(|edit| edit.max.is_none()) {
                            let ok = applied.moved.iter().all(|&test| users.carry_exit_test(function, test, None))
                                && applied
                                    .copied
                                    .iter()
                                    .all(|&(old, copy)| users.carry_exit_test(function, old, Some(copy)));
                            carried = ok.then_some(users);
                        }
                        changed = true;
                        continue;
                    }
                    Outcome::Shadow(edit) => {
                        llrm_mir::counts::bump(llrm_mir::counts::Event::LsrPrepared);
                        lsr_prepare::apply_shadow(function, context, &edit);
                        shadowed = true;
                        changed = true;
                        continue;
                    }
                    Outcome::Planned(Some(plan)) => {
                        llrm_mir::counts::bump(llrm_mir::counts::Event::LsrSolutions);
                        if plan.end_biased_differs && !llrm_mir::memory::has(&function.attrs, END_BIASED_FIRED) {
                            function.attrs.push(llrm_mir::opcode::Attribute::Flag(END_BIASED_FIRED.into()));
                        }
                        plan
                    }
                };
                let mut lp_blocks = llrm_mir::dense::IdSet::<BlockId>::new();
                if let Some(lp) = env.loops.get_loop_for(header) {
                    for &block in lp.get_blocks() {
                        lp_blocks.insert(block);
                    }
                }
                if split_phi_edges(function, &plan, &|block| env.loops.is_loop_header(block), &|block| {
                    lp_blocks.contains(&block)
                }) {
                    changed = true;
                    split = true;
                } else {
                    // Expand and rewrite.
                    let mut rewriter = Expander::new("lsr");
                    rewriter.set_canonical_mode(false);
                    rewriter.enable_lsr_mode();
                    let mut run = Implement {
                        context,
                        function,
                        declared,
                        se,
                        env: &env,
                        rewriter: &mut rewriter,
                        tti,
                        l: header,
                        body: &lp_blocks,
                        iv_inc_insert_pos: plan.iv_inc_insert_pos,
                        dead: Vec::new(),
                        hoisted: Vec::new(),
                        changed: false,
                    };
                    run.implement_solution(&plan);
                    let dead = std::mem::take(&mut run.dead);
                    let hoisted = std::mem::take(&mut run.hoisted);
                    let rewrote = run.changed;
                    // `ImplementSolution`: the dead instructions go first, so
                    // the increments move over only the
                    // uses that are left.
                    crate::dead::dead_from(context, outer.callees(), function, dead);
                    let mut run = Implement {
                        context,
                        function,
                        declared,
                        se,
                        env: &env,
                        rewriter: &mut rewriter,
                        tti,
                        l: header,
                        body: &lp_blocks,
                        iv_inc_insert_pos: plan.iv_inc_insert_pos,
                        dead: Vec::new(),
                        hoisted,
                        changed: false,
                    };
                    run.move_increments();
                    changed |= rewrote | run.changed;
                    if rewrote | run.changed {
                        llrm_mir::counts::bump(llrm_mir::counts::Event::LsrRewritten);
                    }
                    let mut dead = Vec::new();
                    // `DeleteDeadPHIs`: the phis nothing reads any more, those
                    // of a cycle included.
                    dead.extend(
                        function
                            .iter_block(header)
                            .take_while(|&inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi)
                            .filter_map(|inst| function.instruction(inst).result),
                    );
                    crate::dead::dead_from(context, outer.callees(), function, dead);
                    break;
                }
            }
            if split {
                stable = false;
                session = new_session(context, function);
                mark = function.mark();
                mark_opaque(&mut session, context, function);
            }
        }
        // `Rewriter.replaceCongruentIVs(L, &DT, DeadInsts, &TTI)`: after the
        // loop's strength reduction, or none, a phi of the loop that an equal
        // or wider one makes (a narrower counter beside the one it truncates)
        // is that one, cast.
        session.forget_since_keeping(context, layout, &outer.globals, &outer.metadata, function, mark, true);
        mark = function.mark();
        let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
        let simple = {
            let cx = env.cx(&*context, &*function);
            cx.loops.get_loop_for(header).is_some_and(|found| found.is_loop_simplify_form(cx.function))
        };
        if simple {
            let mut rewriter = Expander::new("lsr");
            rewriter.set_canonical_mode(false);
            let mut folded_away = Vec::new();
            if rewriter.replace_congruent_ivs(&env, context, function, se, header, &mut folded_away, Some(tti)) > 0 {
                changed = true;
                let seeds: Vec<_> =
                    folded_away.into_iter().filter_map(|inst| function.instruction(inst).result).collect();
                crate::dead::dead_from(context, outer.callees(), function, seeds);
            }
        }
        // `rewriteLoopExitValues(..., UnusedIndVarInLoop, ...)`: LSR may have
        // removed every use of an induction variable from the loop; the exit
        // phi is the only one left, and the final value replaces it.
        let lcssa_exit = {
            let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
            env.loops
                .get_loop_for(header)
                .is_some_and(
                    |found| found.get_exit_block(function).is_some()
                        && crate::loopversioning::is_recursively_lcssa_form(function, env.loops, env.tree, found),
                )
        };
        if lcssa_exit && crate::exitvalues::enabled() {
            let mut rewriter = Expander::new("lsr");
            rewriter.set_canonical_mode(true);
            if crate::exitvalues::rewritten_unused_ivs(
                context,
                layout,
                function,
                declared,
                outer,
                &mut session,
                &mut rewriter,
                header,
                size,
            ) > 0
            {
                changed = true;
                session.forget_since_keeping(context, layout, &outer.globals, &outer.metadata, function, mark, true);
                mark = function.mark();
                // `DeleteDeadPHIs`.
                let phis: Vec<_> = function
                    .iter_block(header)
                    .take_while(|&inst| function.instruction(inst).opcode == llrm_mir::opcode::Opcode::Phi)
                    .filter_map(|inst| function.instruction(inst).result)
                    .collect();
                crate::dead::dead_from(context, outer.callees(), function, phis);
            }
        }
    }
    // The expressions made with those values read as unknown are not handed on.
    if !stable || !divide_kept.is_empty() {
        return (changed, None);
    }
    session.forget_since_keeping(context, layout, &outer.globals, &outer.metadata, function, mark, true);
    (changed, Some(session.into_scev()))
}

/// The results of the `urem` and `srem` by a constant that is not a power of
/// two, on a width where `keeps_division` says the target keeps the divide.
fn remainders_kept_as_divides(
    context: &Context,
    function: &Function,
    outer: &Outer,
    size: bool,
) -> Vec<llrm_mir::module::ValueId> {
    use llrm_mir::opcode::{BinaryOp, Opcode};
    let target = outer.target();
    function
        .walk()
        .map(|(_, inst)| function.instruction(inst))
        .filter(|one| matches!(one.opcode, Opcode::Binary(BinaryOp::URem | BinaryOp::SRem)))
        .filter(|one| {
            let Some(llrm_mir::module::Operand::Constant(divisor)) = one.operands.get(1).copied() else { return false };
            let llrm_mir::context::ConstantKind::Int(divisor) = context.get(divisor).kind else { return false };
            let Some(bits) = one.result.and_then(|value| context.types.int_bits(function.value(value).ty)) else {
                return false;
            };
            divisor > 1 && !divisor.is_power_of_two() && target.keeps_division(bits, size)
        })
        .filter_map(|one| one.result)
        .collect()
}
