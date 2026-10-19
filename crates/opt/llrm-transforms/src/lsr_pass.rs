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
//! Instead of [`crate::lsr`] with `LLRM_EXP_LSR`.
//!
//! # Limits
//! DEVIATE: not yet ported: IV chains, `OptimizeShadowIV`,
//! `OptimizeLoopTermCond`, `replaceCongruentIVs`, `DeleteDeadPHIs` after the
//! rewrite (what the later `dead` pass removes stays until it), and the debug
//! value salvage. A loop of more than 200 IV users is left (`MaxIVUsers`).

use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::ScevSession;
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

/// `MaxIVUsers`.
const MAX_IV_USERS: usize = 200;

/// Whether `LLRM_EXP_LSR` asks for this pass in place of [`crate::lsr`].
pub fn enabled() -> bool {
    llrm_support::env_flag!("LLRM_EXP_LSR")
}

/// LLVM's loop strength reduction.
pub struct LoopStrengthReduce;

impl FunctionPass for LoopStrengthReduce {
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
        let Some(tti) = outer.target().tti() else { return PreservedAnalyses::all() };
        if reduced(unit.context, unit.layout, unit.function, unit.declared, &outer, &*tti) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
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
                .is_some_and(|callee| outer.globals[callee.0 as usize].name.as_deref() == Some("llvm.assume"))
        })
        .collect()
}

/// What the constructor of `LSRInstance` makes of a loop.
enum Outcome {
    /// The loop is left as it is, or this is its plan.
    Planned(Option<Plan>),
    /// Exit tests to edit first; the loop is then analysed again.
    Edit(Vec<ExitEdit>),
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
    let mut users =
        IvUsers::of(se, &cx, header, &Env { safe_to_speculate: &safe_to_speculate, ephemeral: &is_ephemeral });
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
                return Outcome::Edit(edits);
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
    let headers: Vec<BlockId> = {
        let tree = llrm_mir::domtree::DomTree::of(function);
        let loops = llrm_mir::loopinfo::LoopInfo::analyze(function, || &tree);
        let mut headers: Vec<BlockId> = loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        headers
    };
    let mut changed = false;
    for header in headers {
        // The first look may find exit tests to edit, and the plan may split
        // edges: each is followed by an analysis of the loop as it is then.
        let mut marked: Option<Vec<InstId>> = None;
        for _ in 0..6 {
            let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
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
                stress_chains,
            ) {
                Outcome::Planned(None) => break,
                Outcome::Edit(edits) => {
                    marked = Some(lsr_prepare::apply(function, &context.types, header, &edits));
                    changed = true;
                    continue;
                }
                Outcome::Shadow(edit) => {
                    lsr_prepare::apply_shadow(function, context, &edit);
                    changed = true;
                    continue;
                }
                Outcome::Planned(Some(plan)) => plan,
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
                continue;
            }
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
                iv_inc_insert_pos: plan.iv_inc_insert_pos,
                dead: Vec::new(),
                changed: false,
            };
            run.implement_solution(&plan);
            let dead = std::mem::take(&mut run.dead);
            run.move_increments();
            let rewrote = run.changed;
            changed |= rewrote;
            // `DeleteDeadPHIs`: the phis nothing reads any more, those of a
            // cycle included.
            let mut dead = dead;
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
    changed
}
