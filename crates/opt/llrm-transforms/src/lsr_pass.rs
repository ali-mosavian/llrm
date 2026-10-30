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
use llrm_analysis::scev::expr::Scev;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{BlockId, Function, InstId};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::tti::Tti;

use crate::ivusers::{Env, IvUsers};
use crate::lsr_collect::Collection;
use crate::lsr_implement::{Implement, Plan, split_phi_edges};
use crate::lsr_prepare::{self, ExitEdit, ShadowEdit};

#[cfg(test)]
thread_local! {
    /// How many sessions (dominator tree, loops and assumptions) the pass made.
    pub static SESSIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `MaxIVUsers`.
const MAX_IV_USERS: usize = 200;

/// LLVM's loop strength reduction. `size`: the compile is for size, which the
/// exit-value rewrite after it reads (`exitvalues` DEVIATE 15).
pub struct LoopStrengthReduce {
    pub size: bool,
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
            && crate::mirror::mirrored(unit.context, unit.layout, unit.function, &outer, &outer.target().address_forms())
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
    stress_chains: bool,
) -> Outcome {
    let planned = |plan| Outcome::Planned(plan);
    let cx = env.cx(context, function);
    let Some(lp) = cx.loops.get_loop_for(header) else { return planned(None) };
    // Loop simplify form, or stay out of trouble; and the innermost loops
    // only: the outer ones wait for a model of them.
    if lp.get_loop_preheader(function).is_none()
        || lp.get_loop_latch(function).is_none()
        || !lp.has_dedicated_exits(function)
        || !lp.is_innermost()
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
    let env_users = Env { safe_to_speculate: &safe_to_speculate, ephemeral: &is_ephemeral };
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
            if let Some(shadow) = lsr_prepare::analyze_shadow(se, &cx, header, &users) {
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

    // Chains are looked for where the number of registers is what costs.
    let chains = if tti.is_num_regs_major_cost_of_lsr() {
        crate::lsr_chain::collect_chains(se, &cx, header, &users, &is_ephemeral, stress_chains)
    } else {
        Default::default()
    };
    let safe_to_expand = |se: &mut ScalarEvolution, s: Scev| rewriter.is_safe_to_expand(&cx, se, s);
    let mut lsr = Collection::new(&cx, tti, header, &safe_to_expand, false, iv_inc_insert_pos);
    lsr.chain_uses = chains.inc_set.clone();
    lsr.collect_interesting_types_and_factors(se, &users);
    lsr.collect_fixups_and_initial_formulae(se, &users);
    lsr.collect_loop_invariant_fixups_and_formulae(se);
    if lsr.uses.is_empty() {
        return planned(None);
    }
    // The formulae for each use, and the search space made small enough.
    lsr.generate_all_reuse_formulae(se);
    lsr.filter_out_undesirable_dedicated_registers(se);
    lsr.narrow_search_space_using_heuristics(se);
    let solution = lsr.solve(se);
    if solution.is_empty() {
        return planned(None);
    }
    let mut plan = lsr.into_plan(solution);
    plan.chains = chains.chains;
    planned(Some(plan))
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
    reduced_held(context, layout, function, declared, outer, tti, stress_chains, false, None).0
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
    // Where the function stood when the expressions were last made current.
    let mut mark = function.mark();
    let headers: Vec<BlockId> = {
        let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        headers
    };
    for header in headers {
        llrm_mir::counts::bump(llrm_mir::counts::Event::LsrLoops);
        // The first look may find exit tests to edit, and the plan may split
        // edges: each is followed by an analysis of the loop as it is then.
        let mut marked: Option<Vec<InstId>> = None;
        // The users of the first look, kept through the edit of the exit tests.
        let mut carried: Option<IvUsers> = None;
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
                    stress_chains,
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
                        changed = true;
                        continue;
                    }
                    Outcome::Planned(Some(plan)) => {
                        llrm_mir::counts::bump(llrm_mir::counts::Event::LsrSolutions);
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
                        changed: false,
                    };
                    run.implement_solution(&plan);
                    let dead = std::mem::take(&mut run.dead);
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
    if !stable {
        return (changed, None);
    }
    session.forget_since_keeping(context, layout, &outer.globals, &outer.metadata, function, mark, true);
    (changed, Some(session.into_scev()))
}
