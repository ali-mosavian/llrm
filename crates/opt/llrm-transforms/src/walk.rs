//! The module passes that LLVM runs between the early function pipeline and
//! the function simplification (`ipsccp`, `deadargelim`), so that a body they
//! change goes through the Simplify list once and not again afterwards.
//!
//! # How it works
//! `ModuleIpo` is a program pass placed after `Phase::Early`. It applies the
//! edits of the interprocedural step that used to run after the whole list
//! and send each body through the Simplify list again (`ipa-deadargs.`). The
//! bodies it changes are reported to their module's analyses; no pipeline
//! runs here.
//!
//! `Walk` is a program pass placed after `Phase::Cleanup`, in place of the
//! module-wide Simplify stage. Callees' components first, it splices the
//! calls `inlinecost::sites` accepts into each function, then runs the
//! Simplify list over it, as LLVM's CGSCC walk does: a loop around a call is
//! unrolled after the call is inlined (keybind `_action_for`, 8 inlined copies
//! of `_text_eq`, 325 instructions against 55).
//!
//! # Deviations
//! - W1: `deadargelim` runs before the Simplify list, LLVM's after the CGSCC
//!   walk (`PassBuilderPipelines.cpp` l.1335). Fact: nothing here cleans a body
//!   after the walk (LLVM's later module optimization pipeline does), and the
//!   argument a dropped parameter leaves behind is dead code only the list
//!   removes. Issue: it sees bodies simplified by Early only. Exit: a post-walk
//!   cleanup stage, then `deadargelim` after the walk.
//! - W2a: the walk is per function in callee-first order, not per SCC with the
//!   inliner's own updates; the clocks inliner still runs afterwards;
//!   `inlinecost::sites` stands for `decide` until it lands. Exit: W2.
//! - W2: `ipa-cp` clones with the clocks' cost model (`ipacp`), not
//!   `FunctionSpecialization`; exit W5.
//!
//! # Example
//! `@f` is called with an argument its second parameter never reads; it is
//! dropped before the Simplify list, which then runs once over `@f` and
//! `@top`:
//!
//! ```mir
//! define internal i16 @f(i16 %a, i16 %unused) noinline {
//! b0:
//!   %r = add i16 %a, 1
//!   ret i16 %r
//! }
//!
//! define i16 @top(i16 %x) {
//! b0:
//!   %r = call i16 @f(i16 %x, i16 12)
//!   ret i16 %r
//! }
//! ```
//!
//! From unit test
//! `test_a_body_changed_by_an_interprocedural_stage_does_not_rerun_the_list` in
//! `walk_tests.rs`.
//!
//! # Runs
//! Once per compile, after the Early stage.
//!
//! # See also
//! LLVM `PassBuilderPipelines.cpp` (`buildModuleSimplificationPipeline`:
//! `IPSCCPPass`, `GlobalOptPass`, `DeadArgumentEliminationPass` before the
//! CGSCC walk), `DeadArgumentElimination.cpp`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::manager::GlobalsAA;
use llrm_mir::context::GlobalId;
use llrm_mir::module::Module;
use llrm_mir::passes::ModuleAnalyses;
use llrm_mir::passes::PreservedAnalyses;
use llrm_mir::program::{Program, ProgramAnalyses, ProgramPass};

use crate::interprocedural::{
    Liveness, calls_of, declared, function_mut, managers, private, procedures, reporting, reporting_marked, roots,
    roots_here, vouched,
};

/// What the interprocedural step does to a body before the Simplify list:
/// `ipsccp` ([`crate::ipsccp::run_ipsccp`]), a function called with a constant
/// is copied for it (`ipa-cp`, the clone at -O3), and a parameter nothing reads
/// is not passed (`ipa-deadargs`).
pub struct ModuleIpo {
    pub inline: crate::inline::Threshold,
    /// The clones made here count against the interprocedural step's.
    pub cloning: crate::ipacp::Shared,
}

impl ProgramPass for ModuleIpo {
    fn name(&self) -> &'static str {
        "ipo-early"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let count = program.modules.len();
        let mut procedures: Vec<Vec<GlobalId>> = (0..count).map(|at| procedures(program, at)).collect();
        let mut private: Vec<BTreeSet<GlobalId>> = (0..count).map(|at| private(program, at)).collect();
        let costs = program.target.size_costs();
        let mut modules = managers(program, analyses);
        let mut changed = false;
        for (at, id) in crate::ipsccp::run_ipsccp_with(program, &mut modules, self.inline.single) {
            modules[at].changed(id);
            changed = true;
        }
        // `CalledValuePropagationPass`, which follows IPSCCP.
        for at in 0..count {
            for id in crate::calledvaluepropagation::propagated(&mut program.modules[at]) {
                modules[at].changed(id);
                changed = true;
            }
        }
        // `GlobalOptPass`, which LLVM runs here: a variable is split into the
        // pieces its loads read, and one a body writes before it reads is that
        // body's stack slot, while the loads are still there to be seen.
        for at in 0..count {
            let ids = reporting(&mut program.modules[at], &mut modules[at], |module| {
                crate::globalsra::split(module, &program.layout, program.target.spaces())
            });
            changed |= !ids.is_empty();
            for id in ids {
                modules[at].changed(id);
            }
        }
        let mut cloning = self.cloning.borrow_mut();
        // gcc finishes ipa-cp before ipa-inline (and the pin's specializer
        // solves again after each round): a clone's constants reach the calls
        // in its body, which are cloned in turn, so a chain such as
        // `place(row + 1)` is whole before the walk inlines one link into the
        // next. Off without `-fipa-cp-clone`: `cloned` makes nothing then.
        loop {
            let mut cloned_now = false;
            for at in 0..count {
                let (made, before) = reporting_marked(&mut program.modules[at], &mut modules[at], |module| {
                    crate::ipacp::cloned(
                        module,
                        &program.layout,
                        &procedures[at],
                        &private[at],
                        &costs,
                        self.inline.cp_clone.unwrap_or(false),
                        crate::ipacp::Tuning {
                            hint_clones: self.inline.cp_hint_clones,
                            peel_limited: self.inline.cp_peel_limited,
                        },
                        &mut cloning,
                    )
                });
                vouched(&program.modules[at], made.edited.iter().copied().filter(|id| !made.unsound.contains(id)), &before);
                for &id in &made.added {
                    procedures[at].push(id);
                    private[at].insert(id);
                }
                for &id in made.added.iter().chain(&made.edited) {
                    llrm_mir::memory::unsplit(
                        &mut function_mut(&mut program.modules[at], id).1.attrs,
                        llrm_mir::memory::Moved::OutOfArguments,
                    );
                    modules[at].changed(id);
                    changed = true;
                    cloned_now = true;
                }
            }
            if !cloned_now {
                break;
            }
            for (at, id) in crate::ipsccp::run_ipsccp_with(program, &mut modules, self.inline.single) {
                modules[at].changed(id);
            }
        }
        for one in &mut modules {
            one.invalidate_bodies(&PreservedAnalyses::none());
        }
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
        if changed {
            analyses.invalidate();
        }
        Ok(())
    }
}

/// `DevirtSCCRepeatedPass<4>` (PassBuilderPipelines.cpp): the inliner runs over
/// a body at most this many times while its simplification makes calls direct.
const DEVIRT_REPEATS: usize = 4;

/// LLVM's CGSCC walk: callees' components first, each function's calls decided
/// and spliced (`inlinecost::sites`), then the Simplify list over it, so that a
/// loop is unrolled (`unroll@753`) with the calls in it already inlined.
pub struct Walk {
    pub inline: crate::inline::Threshold,
    /// -O2 and -O3: the target's byte price at speed counts.
    pub speed_rate: bool,
    /// The Simplify list over one body, as the interprocedural step reruns it.
    pub simplify: Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str)>,
    /// The interprocedural step's byte-for-clock rate (None: bytes), which
    /// prices an ArgumentPromotion.
    pub promote_rate: Option<i64>,
    /// The Simplify list reruns over a body no call was spliced into. Off in
    /// the second walk (DEVIATE 6502-SZ1): there the rerun stays only where the
    /// body gets smaller, for Reassociate ranks by the layout and reorders a
    /// settled chain at no gain (call_keeps +26 B).
    pub rerun_unchanged: bool,
}

/// DEVIATE REC-1: the recursion a program is given. `-finline-recursive` asks
/// for gcc's (copies of a function in itself to depth 3, any size); a target
/// that states bounds (`Tti::recursive_inlining`) takes those by default when
/// optimizing for speed (-O2 and -O3: gcc inlines no function at -O1) and
/// inlining is on (`-fno-inline-functions` asks for no copies of a body, a
/// recursive one included). `-fno-inline-recursive` or neither: the pin's
/// inliner, which copies no recursive callee (`IsRecursiveCall`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Recursion {
    pub depth: usize,
    pub callee: i64,
}

impl Recursion {
    pub fn of(
        flag: Option<bool>,
        single: bool,
        inlining: bool,
        o3: bool,
        tti: Option<&dyn llrm_mir::tti::Tti>,
    ) -> Option<Self> {
        // `-fno-inline-recursive` is off whatever the target says.
        if flag == Some(false) {
            return None;
        }
        match tti.and_then(|tti| tti.recursive_inlining()) {
            Some((depth, callee)) if !single && inlining => {
                let depth = tti
                    .and_then(|tti| tti.recursive_inlining_depth())
                    .map_or(depth, |(o2, o3_depth)| if o3 { o3_depth } else { o2 });
                Some(Self { depth: depth as usize, callee })
            }
            _ => (flag == Some(true)).then_some(Self { depth: crate::inline::MAX_INLINE_RECURSIVE_DEPTH, callee: i64::MAX }),
        }
    }
}

/// Whether `id` has a pointer parameter: the only kind a promotion takes.
fn has_pointer_parameter(
    module: &Module,
    id: GlobalId,
) -> bool {
    module
        .global(id)
        .function()
        .is_some_and(
            |function| function
                .parameters()
                .iter()
                .any(
                    |&parameter| matches!(
                        module.context.types.get(function.value(parameter).ty),
                        llrm_mir::types::Type::Pointer(_)
                    ),
                ),
        )
}

impl ProgramPass for Walk {
    fn name(&self) -> &'static str {
        "walk"
    }

    fn run(
        &mut self,
        program: &mut Program,
        analyses: &mut ProgramAnalyses,
    ) -> Result<(), String> {
        let mut modules = managers(program, analyses);
        let roots = roots(program);
        let recursion = Recursion::of(
            self.inline.recursive,
            self.inline.single,
            self.inline.limit != 0 && self.speed_rate,
            self.inline.cp_clone.unwrap_or(false),
            program.target.tti().as_deref(),
        );
        let mut params = crate::inlinecost::InlineParams {
            allow_recursive_call: recursion.is_some(),
            recursive_callee_limit: recursion.map_or(i64::MAX, |one| one.callee),
            recursive_edge_growth: edge_growth_bar(program.target.tti().as_deref(), self.inline.limit >= 250),
            last_call_to_static: self.inline.last,
            o1_bound: !self.speed_rate,
            cold_site_ignores_sinks: program.target.tti().is_some_and(|tti| tti.cold_site_ignores_sinks()),
            ..crate::inlinecost::InlineParams::from_level(if self.inline.limit >= 250 { 3 } else { 2 })
        };
        // The target's tuning table.
        if let Some(threshold) = program.target.tti().and_then(|tti| tti.inline_threshold_override()) {
            params.default_threshold = threshold;
        }
        let size = self.inline.caller_size();
        for at in 0..program.modules.len() {
            let wanted: BTreeSet<GlobalId> = procedures(program, at).into_iter().collect();
            let private = private(program, at);
            let module = &mut program.modules[at];
            let mut recursive = crate::inline::recursive(module);
            let mut linear_recursive = crate::inline::linear_recursive(module, &recursive);
            let addressed = llrm_mir::callgraph::addressed(module);
            // The calls to each function as the splices leave them, as LLVM's
            // use lists do (`isSoleCallToLocalFunction`), and a callee with
            // none left is deleted there, as `removeDeadFunctions` does after
            // the inliner.
            let mut live = Liveness::new(module, private, &addressed, &roots_here(&roots, at));
            let components: Vec<Vec<GlobalId>> = llrm_mir::callgraph::CallGraph::new(module)
                .bottom_up_components()
                .into_iter()
                .map(|(members, _)| members.into_iter().filter(|id| wanted.contains(id)).collect())
                .collect();
            let remarks = llrm_support::env_flag!("LLRM_INLINE_REMARKS");
            let mut visited: BTreeSet<GlobalId> = BTreeSet::new();
            for members in components {
                for &id in &members {
                    // `DevirtSCCRepeatedPass<4>`: a call the simplification
                    // made direct is inlined by another run
                    // of the inliner over the body.
                    let mut repeats = 0;
                    // SZ2: the callees a trial rejected in this caller, with the credit
                    // that trial had; a later trial of the callee is skipped unless it
                    // has more (the group of all a callee's sites, which takes the
                    // callee with them, is not another try of one site).
                    let mut rejected: BTreeMap<GlobalId, i64> = BTreeMap::new();
                    loop {
                        let mut decided = remarks.then(Vec::new);
                        // `sites` prices every block's frequency before it
                        // looks at a call.
                        let calls_a_body = module
                            .global(id)
                            .function()
                            .is_some_and(
                                |caller| caller.walk().any(|(_, inst)| {
                                    llrm_mir::memory::callee(&module.context, caller, inst).is_some_and(|callee| {
                                        module.global(callee).function().is_some_and(|one| !one.layout().is_empty())
                                    })
                                }),
                            );
                        // SZ2: the sites the -Os bound alone refused, to try.
                        let mut trials = llrm_support::hash::SparseIdMap::default();
                        let tti = program.target.tti();
                        let constant = if !calls_a_body {
                            Default::default()
                        } else {
                            crate::inlinecost::sites_with_trials(
                                module,
                                &program.layout,
                                id,
                                live.references(),
                                &addressed,
                                &recursive,
                                &linear_recursive,
                                params,
                                size,
                                self.inline.limit == 0,
                                tti.as_deref(),
                                decided.as_mut(),
                                (tti.as_deref().is_some_and(|tti| {
                                    size != crate::inlinecost::CallerSize::Speed
                                        || (tti.inline_policy().by_rate > 0
                                            && (self.speed_rate || tti.speed_inline_rate_at_o1()))
                                        || (!self.speed_rate && tti.inline_trials_at_o1())
                                }))
                                .then_some(&mut trials),
                            )
                        };
                        let mut declared = declared(&mut modules[at], module);
                        let (context, function, metadata) =
                            crate::interprocedural::function_and_metadata_mut(module, id);
                        let was = calls_of(context, function);
                        // DEVIATE (recursion): a recursive callee inlined here
                        // has its own calls peeled in
                        // the copy (gcc's peeling, not
                        // at -Os).
                        let peeled: llrm_support::hash::IndexMap<GlobalId, crate::inline::Candidate> = constant
                            .values()
                            .filter(|one| one.self_calls.is_some() && !self.inline.single && recursion.is_some())
                            .filter_map(|one| Some((one.callee?, one.clone())))
                            .collect();
                        let known = modules[at].cached::<llrm_analysis::manager::GlobalsModRefAnalysis>();
                        let by = crate::inline::Caller {
                            layout: &program.layout,
                            effects: known.as_deref(),
                            recursive: recursive.contains(&id)
                                && !tti.as_deref().is_some_and(|tti| tti.inline_frames_into_recursive_callers()),
                            self_inlining: None,
                            peeling: !self.inline.single && recursion.is_some(),
                            depth: recursion.map_or(0, |one| one.depth),
                            callee: recursion.map_or(i64::MAX, |one| one.callee),
                            growth: crate::inline::MAX_INLINE_INSNS_RECURSIVE,
                        };
                        let spliced = crate::inline::expanded_all(
                            context,
                            function,
                            &by,
                            &peeled,
                            Some(&constant),
                            &mut declared,
                            metadata,
                        )? > 0;
                        // A recursive callee's peeled copies spliced here make
                        // this body one with self-copies too (`SELF_COPIED`).
                        if spliced && !peeled.is_empty() {
                            function_mut(module, id)
                                .1
                                .attrs
                                .push(llrm_mir::opcode::Attribute::Flag(SELF_COPIED.into()));
                        }
                        // DEVIATE (recursion): a tree-recursive function waits
                        // for its tail-call elimination
                        // until its copies are made
                        // (`SelfInline`, after ipa-cp has seen it whole). Only
                        // a body within the callee cap
                        // waits: one above it gets no
                        // copies, so its elimination stays at the pin's slot.
                        let tree = self_calls(module, &program.layout, id, self.inline.single);
                        if llrm_support::debug::enabled("recursion")
                            && let (Some(tti), true) = (tti.as_deref(), tree.is_some())
                        {
                            eprintln!(
                                "recursion: @{} gcc_insns {} call/total clocks {:?}",
                                module.global(id).name.owned().unwrap_or_default(),
                                crate::inline::gcc_work(module.global(id).function().expect("a function")),
                                self_call_share(module, &*program.target, tti, &program.layout, id)
                            );
                        }
                        let priced = tti.as_deref().and_then(|tti| tti.priced_self_inlining());
                        let fits = match (&tree, priced, tti.as_deref()) {
                            (Some(frequencies), Some(ceilings), Some(tti)) => {
                                priced_copy_depth(
                                    module,
                                    &program.layout,
                                    &*program.target,
                                    tti,
                                    id,
                                    frequencies,
                                    ceilings,
                                ) > 0
                            }
                            _ => recursion.is_some_and(|one| {
                                let share = tti.as_deref().and_then(|tti| tti.recursive_inlining_call_share());
                                let calls_dominate = share.is_none_or(|(num, den)| {
                                    tti.as_deref()
                                        .and_then(|tti| self_call_share(module, &*program.target, tti, &program.layout, id))
                                        .is_some_and(|(calls, total)| calls * den >= total * num)
                                });
                                calls_dominate
                                    && module
                                    .global(id)
                                    .function()
                                    .is_some_and(
                                        |body| crate::inline::gcc_work(body) <= one.callee
                                            && crate::inline::recursive_edge_growth(body)
                                                <= edge_growth_bar(tti.as_deref(), self.inline.limit >= 250),
                                    )
                            }),
                        };
                        if fits && recursion.is_some() && tree.is_some() {
                            function_mut(module, id)
                                .1
                                .attrs
                                .push(llrm_mir::opcode::Attribute::Flag(crate::tailcallelim::DEFERRED.into()));
                            // The flag is part of its declaration: the analyses that list declarations patch only what is told changed.
                            modules[at].changed(id);
                        }
                        for (call, callee, cost) in decided.iter().flatten() {
                            let name = |id: GlobalId| module.global(id).name.as_deref().unwrap_or("?");
                            let gone = module.global(id).function().is_some_and(|one| one.parent(*call).is_none());
                            eprintln!("{}", crate::inlinecost::remark(name(id), name(*callee), cost, gone));
                        }
                        if declared.place(module)? > 0 {
                            modules[at].invalidate(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                        }
                        if spliced {
                            modules[at].changed(id);
                            modules[at].invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                        }
                        // SZ2: a trial costs the Simplify list over the caller
                        // once more, so it is made for
                        // a caller of
                        // at most `TRIAL_CALLER` instructions.
                        let spliced_body = (!trials.is_empty()
                            && function_mut(module, id).1.walk().count() <= tti.as_deref().map_or(TRIAL_CALLER, |tti| tti.inline_trial_caller_limit()))
                            .then(|| function_mut(module, id).1.clone());
                        let mid = calls_of(&module.context, module.global(id).function().expect("a procedure"));
                        // The calls the splice brought go to the inliner's
                        // worklist, priced and spliced before the function
                        // passes run once (`InlinerPass::run`'s `Calls`).
                        // DEVIATE: bounded by `DEVIRT_REPEATS` rounds; the
                        // pin's worklist has no cap. Owner w2, exit 2026-11-12.
                        // The pin pushes every site the splice brings
                        // (`InlinedCallSites`, Inliner.cpp#L422-L444); a count
                        // against `was` misses one when this round's splices
                        // also took away calls to the same callee (`g` brings
                        // `f` as the earlier `f` sites go). So the calls of
                        // each callee spliced here are the brought ones.
                        let brought: Vec<GlobalId> = constant
                            .iter()
                            .filter(|(call, _)| module.global(id).function().is_some_and(|one| one.parent(**call).is_none()))
                            .filter_map(|(_, one)| module.global(one.callee?).function())
                            .flat_map(|body| calls_of(&module.context, body))
                            .map(|(callee, _)| callee)
                            .collect();
                        let worklisted = spliced
                            && trials.is_empty()
                            && repeats + 1 < DEVIRT_REPEATS
                            && mid.iter().any(|(callee, count)| {
                                let before = was.iter().find(|(seen, _)| seen == callee).map_or(0, |&(_, count)| count);
                                (before == 0
                                    || ((*count > before || brought.contains(callee)) && !recursive.contains(callee)))
                                    && module.global(*callee).function().is_some_and(|one| !one.layout().is_empty())
                            });
                        if worklisted {
                            live.spliced(module, &mut modules[at], was, mid);
                            if recursive.contains(&id) {
                                recursive = crate::inline::recursive(module);
                                linear_recursive = crate::inline::linear_recursive(module, &recursive);
                            }
                            repeats += 1;
                            continue;
                        }
                        if spliced || self.rerun_unchanged {
                            (self.simplify)(module, &mut modules[at], id, "walk.");
                        } else if let Some(tti) = tti.as_deref() {
                            let kept = function_mut(module, id).1.clone();
                            let before = sized(module, &program.layout, tti, id, false);
                            (self.simplify)(module, &mut modules[at], id, "walk.again.");
                            if sized(module, &program.layout, tti, id, false) + GUARD >= before {
                                *function_mut(module, id).1 = kept;
                                modules[at].changed(id);
                                modules[at]
                                    .invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                            }
                        }
                        if let (Some(before), Some(tti)) = (spliced_body, tti.as_deref()) {
                            // SZ2: each refused site is inlined into the body
                            // as it stood before
                            // the Simplify list, which runs again;
                            // it stays if the body is smaller. The callee stays
                            // either way.
                            let mut best = body_size(module, &program.layout, tti, id, size == crate::inlinecost::CallerSize::Speed);
                            let site_frequencies =
                                crate::inlinecost::call_frequencies(module, &program.layout, &before);
                            // The sites of a private callee that goes with its
                            // last call are tried
                            // together, for its body is saved too;
                            // any other site alone.
                            let mut by_callee: std::collections::BTreeMap<GlobalId, Vec<llrm_mir::module::InstId>> =
                                std::collections::BTreeMap::new();
                            for (call, candidate) in trials.iter() {
                                by_callee.entry(candidate.callee.expect("a callee")).or_default().push(*call);
                            }
                            let mut units: Vec<(Vec<llrm_mir::module::InstId>, i64)> = Vec::new();
                            for (callee, mut calls) in by_callee {
                                calls.sort();
                                let global = module.global(callee);
                                // The references left once this step's splice
                                // is counted: a
                                // site inlined beside the refused ones
                                // no longer keeps the callee alive.
                                let count = |calls: &[(GlobalId, i64)]| {
                                    calls.iter().filter(|(one, _)| *one == callee).map(|(_, n)| *n).sum::<i64>()
                                };
                                let left =
                                    live.references().get(&callee).copied().unwrap_or(0) - count(&was) + count(&mid);
                                let goes = crate::inlinecost::local_linkage(global.linkage)
                                    && !addressed.contains(&callee)
                                    && left == calls.len() as i64;
                                if goes {
                                    units.push((calls, body_size(module, &program.layout, tti, callee, size == crate::inlinecost::CallerSize::Speed)));
                                } else if tti.inline_trial_groups_sites() && calls.len() > 1 {
                                    units.push((calls, 0));
                                } else {
                                    units.extend(calls.into_iter().map(|one| (vec![one], 0)));
                                }
                            }
                            let mut accepted: Vec<llrm_mir::module::InstId> = Vec::new();
                            let mut hot_tried = 0;
                            for (calls, credit) in units {
                                // INL-HOT: a unit of sites the pin's threshold alone refused; a trial costs the
                                // Simplify list over the caller, so a caller tries at most HOT_TRIALS of them.
                                let hot = calls.iter().all(|one| trials[one].hot);
                                if hot && self.inline.limit >= 250 && !tti.inline_hot_trial_at_o3() {
                                    continue;
                                }
                                if hot {
                                    if hot_tried >= HOT_TRIALS {
                                        continue;
                                    }
                                    hot_tried += 1;
                                }
                                if tti.inline_trial_skips_rejected()
                                    && calls.iter().any(|one| trials[one].callee.is_some_and(|callee| rejected.get(&callee).is_some_and(|&tried| tried >= credit)))
                                {
                                    continue;
                                }
                                let kept = function_mut(module, id).1.clone();
                                let by_trips = tti.inline_trial_by_trips();
                                let kept_clocks = trial_clocks(by_trips, module, &program.layout, tti, &kept);
                                let mut chosen = accepted.clone();
                                chosen.extend(calls.iter().copied());
                                let available: llrm_support::hash::SparseIdMap<_, _> =
                                    chosen.iter().map(|one| (*one, trials[one].clone())).collect();
                                *function_mut(module, id).1 = before.clone();
                                let mut declared = crate::interprocedural::declared(&mut modules[at], module);
                                let (context, function, metadata) =
                                    crate::interprocedural::function_and_metadata_mut(module, id);
                                crate::inline::expanded_all(
                                    context,
                                    function,
                                    &by,
                                    &peeled,
                                    Some(&available),
                                    &mut declared,
                                    metadata,
                                )?;
                                if declared.place(module)? > 0 {
                                    modules[at].invalidate(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                                }
                                modules[at].changed(id);
                                modules[at]
                                    .invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                                // POC chain: the sites the trial's splice brought, priced by the
                                // ordinary rules, are inlined into the trial too (the pin inlines
                                // callee-first, so `foo` into `main` and `bar` into that copy meet
                                // the constants together); one round, so O(n).
                                if (size != crate::inlinecost::CallerSize::Speed || (!self.speed_rate && tti.inline_trials_at_o1()))
                                    && tti.opt_size_inline_chain_trial()
                                {
                                    let mut seen = Vec::new();
                                    let second = crate::inlinecost::sites_with_trials(
                                        module,
                                        &program.layout,
                                        id,
                                        live.references(),
                                        &addressed,
                                        &recursive,
                                        &linear_recursive,
                                        params,
                                        size,
                                        self.inline.limit == 0,
                                        Some(tti),
                                        Some(&mut seen),
                                        None,
                                    );
                                    if remarks {
                                        for (call, callee, cost) in &seen {
                                            let name = |g: GlobalId| module.global(g).name.as_deref().unwrap_or("?");
                                            eprintln!("chain: {}", crate::inlinecost::remark(name(id), name(*callee), cost, false));
                                            let _ = call;
                                        }
                                    }
                                    if !second.is_empty() {
                                        let mut declared = crate::interprocedural::declared(&mut modules[at], module);
                                        let (context, function, metadata) =
                                            crate::interprocedural::function_and_metadata_mut(module, id);
                                        crate::inline::expanded_all(
                                            context,
                                            function,
                                            &by,
                                            &peeled,
                                            Some(&second),
                                            &mut declared,
                                            metadata,
                                        )?;
                                        if declared.place(module)? > 0 {
                                            modules[at].invalidate(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                                        }
                                        modules[at].changed(id);
                                        modules[at]
                                            .invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                                                                            }
                                }
                                // The cleanup list has no loop passes: at speed a copy of a callee
                                // with a loop meets constants that unroll and forward away, which
                                // only the whole list prices (vla -O2: 8 sites, 644 B against -40).
                                // At size the unroll keeps only what shrinks the code: jpeg -Os
                                // measured +32 B with the whole list.
                                let light = tti.inline_trial_light_simplify()
                                    && !(size == crate::inlinecost::CallerSize::Speed && calls.iter().filter_map(|one| trials[one].callee).any(|callee| {
                                        module.global(callee).function().is_some_and(|body| !crate::inlinecost::loop_headers(body).is_empty())
                                    }));
                                (self.simplify)(module, &mut modules[at], id, if light { TRIAL_LIGHT } else { "walk.trial." });
                                let tried = body_size(module, &program.layout, tti, id, size == crate::inlinecost::CallerSize::Speed);
                                // At speed a target that prices bytes in clocks
                                // takes a trial that grows the code where the
                                // clocks it saves are worth the growth.
                                let rate = if self.speed_rate {
                                    tti.speed_milliclocks_per_byte().max(tti.inline_policy().by_rate)
                                } else {
                                    0
                                };
                                let (saved, growth) = if size == crate::inlinecost::CallerSize::Speed && rate > 0 {
                                    let costs = program.target.byte_costs();
                                    let now = module.global(id).function().expect("a body");
                                    let callees: Vec<&llrm_mir::module::Function> = calls
                                        .iter()
                                        .filter_map(|one| trials[one].callee)
                                        .filter_map(|callee| module.global(callee).function())
                                        .collect();
                                    let run: i64 = callees
                                        .iter()
                                        .map(|body| trial_clocks(by_trips, module, &program.layout, tti, body))
                                        .sum();
                                    let gone: i64 = if credit > 0 {
                                        let sites: Vec<GlobalId> =
                                            calls.iter().filter_map(|one| trials[one].callee).collect();
                                        dying_callees_bytes(module, &sites, |body| {
                                            function_bytes(module, &program.layout, &costs, body)
                                        })
                                    } else {
                                        0
                                    };
                                    // A site run per trip of a loop saves its
                                    // clocks that often; a cold one saves them
                                    // once.
                                    let trips = calls
                                        .iter()
                                        .map(|one| site_frequencies.get(one).copied().unwrap_or(1.0).max(1.0))
                                        .sum::<f64>()
                                        / calls.len() as f64;
                                    let saves: i64 = callees
                                        .iter()
                                        .map(|body| tti.callee_saved_clocks(llrm_analysis::liveness::pressure(body, None, None)))
                                        .sum();
                                    let now_clocks = trial_clocks(by_trips, module, &program.layout, tti, now);
                                    // By trips the caller's loops weigh the
                                    // copy among them: the callee's
                                    // run and saves are what the site's
                                    // frequency repeats.
                                    let saved = if by_trips {
                                        (kept_clocks as f64 + (run + saves) as f64 * trips - now_clocks as f64) as i64
                                    } else {
                                        ((kept_clocks + run + saves - now_clocks) as f64 * trips) as i64
                                    };
                                    (
                                        saved,
                                        function_bytes(module, &program.layout, &costs, now)
                                            - function_bytes(module, &program.layout, &costs, &kept)
                                            - gone,
                                    )
                                } else {
                                    // At size the units count a call by its
                                    // parameters, not the words and address
                                    // forms it
                                    // pushes: the target's byte prices decide
                                    // too, for a
                                    // callee that stays (the byte model
                                    // misprices a
                                    // body that goes with its last calls), by
                                    // an eighth of
                                    // the caller: the model priced a float
                                    // kernel's copy
                                    // 19 B under its 180 (+5 B built; qmove
                                    // -Os).
                                    let costs = program.target.trial_costs();
                                    let now = module.global(id).function().expect("a body");
                                    // DEVIATE (owner softfloat, exit 2026-11-20): the callee's bytes are summed per
                                    // site off the byte gate's targets, as the x86 model was fit with.
                                    let gone: i64 = if credit > 0 && (tti.opt_size_inline_byte_gate() || tti.opt_size_inline_trial_by_bytes()) {
                                        let sites: Vec<GlobalId> =
                                            calls.iter().filter_map(|one| trials[one].callee).collect();
                                        dying_callees_bytes(module, &sites, |body| {
                                            function_bytes(module, &program.layout, &costs, body)
                                        })
                                    } else if credit > 0 {
                                        calls
                                            .iter()
                                            .filter_map(|one| trials[one].callee)
                                            .filter_map(|callee| module.global(callee).function())
                                            .map(|body| function_bytes(module, &program.layout, &costs, body))
                                            .sum()
                                    } else {
                                        0
                                    };
                                    let growth = function_bytes(module, &program.layout, &costs, now)
                                        - function_bytes(module, &program.layout, &costs, &kept)
                                        - gone;
                                    // Bytes that tie to the byte are decided by the clocks the copies save.
                                    let saved = if growth == 0 && tti.opt_size_inline_trial_by_bytes() {
                                        let run: i64 = calls
                                            .iter()
                                            .filter_map(|one| trials[one].callee)
                                            .filter_map(|callee| module.global(callee).function())
                                            .map(|body| function_clocks(module, &program.layout, tti, body))
                                            .sum();
                                        kept_clocks + run - function_clocks(module, &program.layout, tti, now)
                                    } else {
                                        0
                                    };
                                    (saved, growth)
                                };
                                // DEVIATE INL-SPEED-OS (`Tti::inline_trial_speed_at_size`): the clocks the copies
                                // save at a hot site, weighed by its frequency, against a few bytes a site.
                                let speed_at_size = match tti.inline_trial_speed_at_size() {
                                    Some(least) if size != crate::inlinecost::CallerSize::Speed => {
                                        let trips = calls
                                            .iter()
                                            .map(|one| site_frequencies.get(one).copied().unwrap_or(1.0).max(1.0))
                                            .sum::<f64>()
                                            / calls.len() as f64;
                                        let now = module.global(id).function().expect("a body");
                                        let run: i64 = calls
                                            .iter()
                                            .filter_map(|one| trials[one].callee)
                                            .filter_map(|callee| module.global(callee).function())
                                            .map(|body| function_clocks(module, &program.layout, tti, body))
                                            .sum();
                                        let once = kept_clocks + run - function_clocks(module, &program.layout, tti, now);
                                        speed_keeps_at_size(least, trips, once, calls.len(), growth)
                                    }
                                    _ => false,
                                };
                                if remarks {
                                    eprintln!(
                                        "{} ({} instructions) SZ2 trial of {} call(s) to '{}': {} against {} (+{} for the callee, {} clocks saved, {} bytes{})",
                                        module.global(id).name.as_deref().unwrap_or("?"),
                                        module.global(id).function().map_or(0, |one| one.walk().count()),
                                        calls.len(),
                                        trials[&calls[0]].callee.map_or("?", |callee| module
                                            .global(callee)
                                            .name
                                            .as_deref()
                                            .unwrap_or("?")),
                                        tried,
                                        best,
                                        credit,
                                        saved,
                                        growth,
                                        if light { ", cleanup list" } else { "" }
                                    );
                                }
                                // At speed with a byte price only the sites of
                                // a callee that
                                // goes with them are tried, and the price
                                // decides: the
                                // module loses the callee, and the copies meet
                                // the constants
                                // (a stack bracket, a trip count) the cost
                                // model does not follow. One site of a callee
                                // that stays
                                // widens the caller's registers for its whole
                                // body (jpeg
                                // `decode` with `getbit` inlined: five more
                                // register
                                // pairs saved on every call, +441k
                                // instructions).
                                let priced = rate > 0 && size == crate::inlinecost::CallerSize::Speed;
                                let kept_bytes = function_bytes(module, &program.layout, &program.target.trial_costs(), &kept);
                                // DEVIATE OS-INLINE-TRIAL (`trial::Site`): a copy the byte model refuses within its
                                // error is the backend's to decide; the second compile inlines it.
                                let by_bytes = tti.opt_size_inline_trial_by_bytes() && !priced;
                                let site = by_bytes.then(|| crate::trial::Site {
                                    caller: module.global(id).name.as_deref().unwrap_or_default().to_owned(),
                                    callees: calls
                                        .iter()
                                        .filter_map(|one| trials[one].callee)
                                        .filter_map(|callee| module.global(callee).name.as_deref().map(str::to_owned))
                                        .collect(),
                                });
                                let ruled = speed_at_size || trial_accepted(tti.opt_size_inline_byte_gate(), tti.opt_size_inline_trial_by_bytes(), tti.inline_trial_keeps_shrinking_copy(), priced, hot || (priced && by_trips), tried, best, credit, saved, growth, rate, kept_bytes);
                                let forced = !ruled && site.as_ref().is_some_and(|site| crate::trial::inlines(&site.caller, &site.callees));
                                if ruled || forced {
                                    best = tried;
                                    accepted.extend(calls);
                                } else {
                                    if let Some(site) = site.filter(|_| {
                                        !crate::trial::inlining()
                                            && growth > 0
                                            && tti.opt_size_inline_trial_limits().is_some_and(|(band, _)| growth * 100 <= kept_bytes * band)
                                    }) {
                                        crate::trial::note_refused(site);
                                    }
                                    for callee in calls.iter().filter_map(|one| trials[one].callee) {
                                        let tried = rejected.entry(callee).or_insert(credit);
                                        *tried = (*tried).max(credit);
                                    }
                                    if remarks {
                                        eprintln!(
                                            "{} SZ2 trial rejected '{}'",
                                            module.global(id).name.as_deref().unwrap_or("?"),
                                            module.global(trials[&calls[0]].callee.expect("a callee")).name.as_deref().unwrap_or("?"),
                                        );
                                    }
                                    *function_mut(module, id).1 = kept;
                                    modules[at].changed(id);
                                    modules[at]
                                        .invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                                }
                            }
                        }
                        // The calls left after the splices and the
                        // simplification (tail-call
                        // elimination makes a recursive body a loop), as
                        // CGSCC updates its call graph.
                        let now = calls_of(&module.context, module.global(id).function().expect("a procedure"));
                        // A callee the splices or the simplification added,
                        // with a body: a parameter that
                        // was a function became the copy's callee, or the call
                        // was devirtualized (a table of
                        // functions made a switch of calls by
                        // `jumptableswitch`).
                        // Or more calls of a callee with a body than before the
                        // splices: the copies
                        // brought call sites that `InlineFunction` hands to the
                        // inliner's worklist.
                        let exposed = now.iter().any(|(callee, count)| {
                            let before = was.iter().find(|(seen, _)| seen == callee).map_or(0, |&(_, count)| count);
                            (before == 0 || (spliced && *count > before && !recursive.contains(callee)))
                                && module.global(*callee).function().is_some_and(|one| !one.layout().is_empty())
                        });
                        live.spliced(module, &mut modules[at], was, now.clone());
                        if recursive.contains(&id) {
                            recursive = crate::inline::recursive(module);
                            linear_recursive = crate::inline::linear_recursive(module, &recursive);
                        }
                        repeats += 1;
                        if repeats >= DEVIRT_REPEATS || !exposed {
                            break;
                        }
                    }
                }
                // `ArgumentPromotionPass` follows the inliner in the CGSCC
                // pipeline: once per component, before its callers are
                // visited, which then simplify once with the new arguments.
                // A function already visited is simplified again.
                // DEVIATE: the copy-out elision and the stamp that precede
                // the module-wide promotion run after the walk; what they
                // enable is promoted there. Owner w2, exit 2026-11-12.
                visited.extend(members.iter().copied());
                if !members.is_empty() && members.iter().any(|&id| has_pointer_parameter(module, id)) {
                    let costs =
                        if self.promote_rate.is_some() { program.target.size_costs() } else { program.target.costs() };
                    let clocks = program.target.costs();
                    let (priced, bytes) = match self.promote_rate {
                        Some(rate) if rate > 0 => (&clocks, false),
                        _ => (&costs, true),
                    };
                    for id in
                        crate::argpromotion::promoted_among(module, &program.layout, priced, bytes, Some(&members))
                    {
                        modules[at].changed(id);
                        function_mut(module, id)
                            .1
                            .attrs
                            .push(llrm_mir::opcode::Attribute::Flag(PROMOTED_IN_WALK.into()));
                        if visited.contains(&id) {
                            (self.simplify)(module, &mut modules[at], id, "promote.");
                        }
                    }
                }
                // `PostOrderFunctionAttrsPass`: once per component, after its
                // bodies are simplified.
                if !members.is_empty() {
                    let mut stated = false;
                    let callees = modules[at].get::<llrm_mir::passes::CalleeEffects>(module);
                    let stated_now = llrm_analysis::functionattrs::post_order_with(
                        module,
                        &program.layout,
                        program.target.spaces(),
                        &callees,
                        &members,
                    );
                    drop(callees);
                    for id in stated_now {
                        modules[at].changed(id);
                        stated = true;
                    }
                    // What it stated reaches the next body's callees.
                    if stated {
                        // Each body stated was reported to `changed`.
                        modules[at].invalidate_logged(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                    }
                }
            }
        }
        for (at, one) in modules.into_iter().enumerate() {
            analyses.carry(at, one.carried());
        }
        analyses.invalidate();
        Ok(())
    }
}

/// DEVIATE (recursion): the frequency of each call in `id`, if it is to be
/// given copies of its own body at its calls of itself, as gcc's
/// `recursive_inlining` does, to the limits of [`crate::inline`]. None where
/// it is not: it calls itself in fewer than two distinct ways (calls of a
/// function that reads and writes nothing, with the same arguments, are one
/// call: `hanoi(n-1, a, c, b) + hanoi(n-1, a, c, b)` is a loop once the
/// simplifier has merged them, 188 clocks, and 185482 copied before), the
/// calls it makes of itself are fewer than one per call (below), or the body
/// is sized for bytes (gcc does not inline recursively at -Os).
pub(crate) fn self_calls(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: GlobalId,
    for_size: bool,
) -> Option<llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, f64>> {
    self_calls_from(module, layout, id, for_size, 2)
}

/// `self_calls` with `fewest` distinct self calls: one for a body whose tail-call
/// elimination has already left a single call in a loop.
pub(crate) fn self_calls_from(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: GlobalId,
    for_size: bool,
    fewest: usize,
) -> Option<llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, f64>> {
    let function = module.global(id).function()?;
    let pure = {
        let effects = llrm_mir::memory::stated(&function.attrs);
        !effects.reads && !effects.writes
    };
    let mut distinct: Vec<(Vec<llrm_mir::module::Operand>, llrm_mir::module::InstId)> = Vec::new();
    for (_, inst) in function.walk() {
        if llrm_mir::memory::callee(&module.context, function, inst) != Some(id) {
            continue;
        }
        let operands = function.instruction(inst).operands.clone();
        if !(pure && distinct.iter().any(|(seen, _)| *seen == operands)) {
            distinct.push((operands, inst));
        }
    }
    if for_size || distinct.is_empty() {
        return None;
    }
    let frequencies = crate::inlinecost::call_frequencies(module, layout, function);
    // A tree: each call of the function makes one or more calls of itself
    // (the frequencies of its distinct self-calls over the entry's sum to 1
    // or more). Fewer, as in recpow's two calls in exclusive branches (0.625),
    // is a linear recursion, which tail-call elimination turns into a loop:
    // copying it peels it (+3.7% clocks at -O2).
    let calls: f64 = distinct.iter().map(|(_, inst)| frequencies.get(inst).copied().unwrap_or(0.0)).sum();
    if calls < 1.0 - 1e-9 {
        return None;
    }
    // One call is a tree only when a loop runs it more than once per entry
    // (queens `place`); `f(n - 1) + n` has frequency exactly 1.
    if distinct.len() < fewest && calls <= 1.0 + 1e-9 {
        return None;
    }
    Some(frequencies)
}

/// The gain is an estimate (`LOOP_TRIPS` assumed trips, the call's sheet
/// price), the growth is exact bytes: the first level is made when its gain is
/// this many times its growth in clocks (later ones, when it is one). Between
/// the loop-bodied rows (growth/gain 0.28 or more: quicksort, bintree, recchop)
/// and the call-bound ones (0.13 or less: fib, frames, rectwo).
const COPY_MARGIN: f64 = 4.0;

/// The clocks the calls of `id` to itself spend on the call, return and
/// argument words (`CallPrice`), and the clocks of its whole body (latency of
/// every instruction, the calls included), counted statically.
pub(crate) fn self_call_share(
    module: &Module,
    program_target: &dyn llrm_mir::target::Machine,
    tti: &dyn llrm_mir::tti::Tti,
    layout: &llrm_mir::datalayout::DataLayout,
    id: GlobalId,
) -> Option<(i64, i64)> {
    let body = module.global(id).function()?;
    let sites: Vec<llrm_mir::module::InstId> = body
        .walk()
        .filter_map(|(_, inst)| (llrm_mir::memory::callee(&module.context, body, inst) == Some(id)).then_some(inst))
        .collect();
    let &first = sites.first()?;
    let arguments = body.instruction(first).operands.len().saturating_sub(1);
    let bytes = program_target.byte_costs();
    let price = llrm_mir::callprice::CallPrice::new(
        &program_target.costs(),
        &bytes,
        arguments,
        tti.speed_milliclocks_per_byte(),
    );
    let total: i64 = body
        .walk()
        .map(
            |(_, inst)| llrm_mir::instructioncost::instruction_cost(
                tti,
                &module.context,
                layout,
                &module.globals,
                body,
                inst,
                llrm_mir::tti::CostKind::Latency,
            )
            .value,
        )
        .sum();
    Some((price.clocks * sites.len() as i64, total))
}

/// DEVIATE REC-1 (priced): the depth of copies of itself that `id` is given,
/// 0 for none. A level of copies saves the calls it removes, `c` clocks each
/// (the sheet's call, return and arguments), `LOOP_TRIPS` times for the trips
/// a recursion is assumed to make: with `f` calls of itself per call, a
/// body that held one level now holds `S = 1 + f + f^2 ...` and a call in `S`
/// is gone, so `d` levels save `trips * c * (1 - 1 / S_d)`. They add, for the
/// `k` calls of itself it has, `k^d` copies of the body less the call and the
/// return each replaces. A level is made while its own clocks, at
/// `speed_milliclocks_per_byte`, are worth its own bytes, up to the target's
/// ceilings (`Tti::priced_self_inlining`). The rule is the sheet's, not a
/// program's: it replaces the literals 14 and 2.
pub(crate) fn priced_copy_depth(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    program_target: &dyn llrm_mir::target::Machine,
    tti: &dyn llrm_mir::tti::Tti,
    id: GlobalId,
    frequencies: &llrm_support::hash::SparseIdMap<llrm_mir::module::InstId, f64>,
    ceilings: (u32, i64),
) -> usize {
    let rate = tti.speed_milliclocks_per_byte();
    let Some(body) = module.global(id).function() else { return 0 };
    if rate <= 0 || crate::inline::gcc_work(body) > ceilings.1 {
        return 0;
    }
    let sites: Vec<llrm_mir::module::InstId> = body
        .walk()
        .filter_map(|(_, inst)| (llrm_mir::memory::callee(&module.context, body, inst) == Some(id)).then_some(inst))
        .collect();
    let Some(&first) = sites.first() else { return 0 };
    let arguments = body.instruction(first).operands.len().saturating_sub(1);
    let calls: f64 = sites.iter().map(|one| frequencies.get(one).copied().unwrap_or(0.0)).sum();
    let bytes = program_target.byte_costs();
    let price = llrm_mir::callprice::CallPrice::new(&program_target.costs(), &bytes, arguments, rate);
    let overhead = price.clocks as f64;
    let net = (function_bytes(module, layout, &bytes, body) - price.bytes).max(0) as f64;
    let k = sites.len() as f64;
    let (mut seen, mut saved_before, mut grown_before) = (1.0f64, 0.0f64, 0.0f64);
    for level in 1..=ceilings.0 as usize {
        seen += calls.powi(level as i32);
        let saved = crate::inline::LOOP_TRIPS as f64 * overhead * (1.0 - 1.0 / seen);
        let grown = (1..=level).map(|one| k.powi(one as i32)).sum::<f64>() * net;
        let (gain, growth) = (saved - saved_before, grown - grown_before);
        let margin = if level == 1 { COPY_MARGIN } else { 1.0 };
        if !price.pays(gain, growth, margin) {
            return level - 1;
        }
        (saved_before, grown_before) = (saved, grown);
    }
    ceilings.0 as usize
}

/// The flag a body that had copies of itself made (D10), or took a recursive
/// callee's peeled copies, carries from then on:
/// the simplifier folds the chains of tests those copies make (`n - 1 == 0`,
/// `n - 2 == 0`, ...) through `algebraic::_offset_compared` whoever else reads
/// the sum, which the pin does only with one use.
/// DEVIATE REC-2: the growth a recursive edge may add at -O2 or -O3
/// (`Tti::recursive_edge_growth`); no bar where the target states none.
fn edge_growth_bar(
    tti: Option<&dyn llrm_mir::tti::Tti>,
    o3: bool,
) -> i64 {
    tti.and_then(|tti| tti.recursive_edge_growth()).map_or(i64::MAX, |(o2, o3_bar)| if o3 { o3_bar } else { o2 })
}

pub const SELF_COPIED: &str = "llrm-self-copied";

/// A body the walk promoted arguments of: the module-wide step after the walk
/// reads it to run IPSCCP again (SD443), as it does for its own promotions.
const PROMOTED_IN_WALK: &str = "llrm-promoted-in-walk";

/// Whether the walk promoted any body; the marks are removed.
pub(crate) fn take_walk_promotions(
    program: &mut Program,
    modules: &mut [ModuleAnalyses],
) -> bool {
    let mut found = false;
    for (at, module) in program.modules.iter_mut().enumerate() {
        for (index, global) in module.globals.iter_mut().enumerate() {
            if let llrm_mir::module::GlobalKind::Function(function) = &mut global.kind {
                let before = function.attrs.len();
                function.attrs.retain(|attr| !matches!(attr, llrm_mir::opcode::Attribute::Flag(flag) if flag == PROMOTED_IN_WALK));
                if function.attrs.len() != before {
                    found = true;
                    // The mark is part of the declaration the analyses list; the second IPSCCP restates only some bodies.
                    modules[at].changed(GlobalId(index as u32));
                }
            }
        }
    }
    found
}

/// DEVIATE (recursion): the copies of a tree-recursive function in itself, as
/// gcc's `recursive_inlining` makes them, to the limits of [`crate::inline`],
/// then its tail-call elimination, which the walk left out
/// (`tailcallelim::DEFERRED`). After both ipa-cp rounds, as gcc runs ipa-cp
/// before its inliner and `pass_tail_recursion` after it: the specializer
/// prices the callee whole (copies first made recchop -O3 decline its clones:
/// 6368 clocks against 6209) and the copies are of the tree (fib: 132006
/// clocks, 294986 copied after tail-call elimination made it a loop).
///
/// Ruling 2026-10-22 (oversight): stays a numbered DEVIATE, owner w10-instcombine, review
/// 2026-11-30. The rule is cost-based: the target's `gcc_insns` (150 here,
/// 450 for peeling), the call frequency over the entry's (10%), depth 3; no
/// shape of a program. Depth 3 and cap 450 kept. The priced row is recpow -O1:
/// compile 49 to 461 M, bytes 5547 against main's 293, clocks 454246 against
/// main's 504509. The rows still above main are open rows with owners
/// (docs/architecture/recursion-rows.md).
pub struct SelfInline {
    /// The Simplify list over one body.
    pub simplify: Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str)>,
    /// Sized for bytes: gcc does not inline recursively there.
    pub for_size: bool,
    /// -O2 and -O3: the target's byte price at speed counts.
    pub speed_rate: bool,
    /// `-f[no-]inline-recursive`; `None`: the target's default.
    pub flag: Option<bool>,
    /// -O3: gcc's `max-inline-insns-auto` is the larger bar.
    pub o3: bool,
    /// Inlining is on (`Threshold.limit != 0`).
    pub inlining: bool,
}

impl SelfInline {
    /// The marked functions of `program`, over `modules`' analyses.
    pub(crate) fn apply(
        &mut self,
        program: &mut Program,
        modules: &mut [ModuleAnalyses],
    ) -> Result<(), String> {
        let waiting = |function: &llrm_mir::module::Function| {
            function
                .attrs
                .iter()
                .any(
                    |one| matches!(
                        one,
                        llrm_mir::opcode::Attribute::Flag(flag) if flag == crate::tailcallelim::DEFERRED
                    ),
                )
        };
        let Some(recursion) =
            Recursion::of(self.flag, self.for_size, self.inlining && self.speed_rate, self.o3, program.target.tti().as_deref())
        else {
            return Ok(());
        };
        for at in 0..program.modules.len() {
            let module = &mut program.modules[at];
            let marked: Vec<GlobalId> =
                module.functions().filter(|(_, _, function)| waiting(function)).map(|(id, _, _)| id).collect();
            for id in marked {
                let mut copies = 0;
                // DEVIATE REC-2: where the target says so, the tail-call elimination comes first and
                // the copies are made of the loop it leaves (gcc's order).
                let tre_first = {
                    let (o2, o3) = program.target.tti().map_or((false, false), |tti| tti.recursive_tre_first());
                    if self.o3 { o3 } else { o2 }
                };
                if tre_first {
                    function_mut(module, id).1.attrs.retain(
                        |one| !matches!(one, llrm_mir::opcode::Attribute::Flag(flag) if flag == crate::tailcallelim::DEFERRED),
                    );
                    modules[at].changed(id);
                    modules[at].invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                    (self.simplify)(module, &mut modules[at], id, "tre-first.");
                }
                let frequencies = self_calls_from(module, &program.layout, id, self.for_size, if tre_first { 1 } else { 2 }).filter(|_| {
                    let bar = edge_growth_bar(program.target.tti().as_deref(), self.o3);
                    let body = module.global(id).function().expect("a marked function");
                    // The bar was fitted on the body as marked; the
                    // elimination's loop is larger (queens 22 -> 27).
                    tre_first || crate::inline::recursive_edge_growth(body) <= bar
                });
                if let Some(frequencies) = frequencies {
                    let tti = program.target.tti();
                    let (depth, callee) = match (
                        tti.as_deref().filter(|_| self.speed_rate).and_then(|tti| tti.priced_self_inlining()),
                        tti.as_deref(),
                    ) {
                        (Some(ceilings), Some(tti)) => (
                            priced_copy_depth(
                                module,
                                &program.layout,
                                &*program.target,
                                tti,
                                id,
                                &frequencies,
                                ceilings,
                            ),
                            ceilings.1,
                        ),
                        _ => (recursion.depth, recursion.callee),
                    };
                    let mut declared = declared(&mut modules[at], module);
                    let (context, function, metadata) = crate::interprocedural::function_and_metadata_mut(module, id);
                    let itself = crate::inline::Candidate {
                        body: std::rc::Rc::new(function.clone()),
                        frame: crate::inline::grown(context, &program.layout, function),
                        moved: false,
                        last_call: false,
                        callee: None,
                        self_calls: None,
                        self_sites: 0,
                        hot: false,
                    };
                    let available = [(id, itself)].into_iter().collect();
                    let known = modules[at].cached::<llrm_analysis::manager::GlobalsModRefAnalysis>();
                    let by = crate::inline::Caller {
                        layout: &program.layout,
                        effects: known.as_deref(),
                        recursive: true,
                        self_inlining: Some(&frequencies),
                        peeling: false,
                        depth,
                        callee,
                        growth: tti
                            .as_deref()
                            .and_then(|tti| tti.recursive_inlining_growth())
                            .map_or(
                                crate::inline::MAX_INLINE_INSNS_RECURSIVE,
                                |(o2, o3)| if self.o3 { o3 } else { o2 },
                            ),
                    };
                    copies =
                        crate::inline::expanded_all(context, function, &by, &available, None, &mut declared, metadata)?;
                    if declared.place(module)? > 0 {
                        modules[at].invalidate(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                    }
                }
                function_mut(module, id)
                    .1
                    .attrs
                    .retain(
                        |one| !matches!(
                            one,
                            llrm_mir::opcode::Attribute::Flag(flag) if flag == crate::tailcallelim::DEFERRED
                        ),
                    );
                if copies > 0 {
                    function_mut(module, id).1.attrs.push(llrm_mir::opcode::Attribute::Flag(SELF_COPIED.into()));
                }
                modules[at].changed(id);
                modules[at].invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                (self.simplify)(module, &mut modules[at], id, "self-inline.");
                // DEVIATE (recursion, D10 third part, docs/architecture/
                // recursion-rows.md): the pin runs its function pipeline once
                // (`NoRerun`, PassBuilderPipelines.cpp:1047) and has no
                // self-copies; gcc's FRE and PRE run again after the inliner.
                // The list runs GVN before jump threading and the copies' paths
                // join in the threading, so once, each copy repeats the calls
                // of the ones that dominate it (fib -O2: 154398 clocks once,
                // 14802 twice). Owner: w-6. Exit: this run goes when the
                // cleanup can run in
                // the pin's inline-then-cleanup order, or when that order gives
                // fib and x_fib at or below main (227171) without it; review
                // 2026-11-30, else a permanent numbered DEVIATE.
                if copies > 0 {
                    (self.simplify)(module, &mut modules[at], id, "self-inline-again.");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;

/// The stage name of a SZ2 trial's simplification when the target asks for the cleanup list.
pub const TRIAL_LIGHT: &str = "walk.trial.light.";

/// SZ2: the largest caller, in instructions, whose refused sites are tried
/// (`Tti::inline_trial_caller_limit` where a target states one).
const TRIAL_CALLER: usize = 300;

/// Whether both operands of a product are an extension of a value no wider than the word: the product the CPU makes in one
/// widening multiply where it has one (`muls.l` into a register pair), not a full double-word product.
fn extended_from_word(
    module: &Module,
    function: &llrm_mir::module::Function,
    operands: &[llrm_mir::module::Operand],
    word: u32,
) -> bool {
    use llrm_mir::module::{Operand, ValueDef};
    use llrm_mir::opcode::{CastOp, Opcode};
    use llrm_mir::tti::Ty;
    operands.iter().take(2).all(|&operand| {
        let Operand::Value(id) = operand else { return false };
        let ValueDef::Instruction(def) = function.value(id).def else { return false };
        let def = function.instruction(def);
        matches!(def.opcode, Opcode::Cast(CastOp::ZExt | CastOp::SExt))
            && def
                .operands
                .first()
                .and_then(|&source| function.operand_type(&module.context, source))
                .and_then(|ty| Ty::of(&module.context.types, ty))
                .is_some_and(|kind| matches!(kind, Ty::Int(bits) if bits <= word))
    })
}

/// The bytes of a fixed-point or saturating intrinsic as the operations it expands to, from the target's table:
/// `llvm.{s,u}mul.fix[.sat].iN` is two extensions, the product of two `N`-bit values in `2N` bits, two shifts to take
/// the scaled middle and, when saturating, two compares and two selects; `llvm.{s,u}shl.sat.iN` is a shift, the shift
/// back, a compare and a select. None: not one of these.
fn expanded_intrinsic(
    costs: &llrm_mir::target::OperationCosts,
    word: u32,
    name: &str,
) -> Option<i64> {
    let rest = name.strip_prefix("llvm.")?;
    let (kind, rest) = rest.split_once('.')?;
    let (op, rest) = (kind.get(1..)?, rest);
    if !matches!(kind.get(..1)?, "s" | "u") {
        return None;
    }
    let bits: u32 = rest.rsplit_once(".i")?.1.parse().ok()?;
    let saturating = rest.contains(".sat");
    let select = costs.branch + costs.r#move;
    match op {
        "mul" if rest.starts_with("fix") => {
            let product = if 2 * bits <= word {
                costs.multiply
            } else if bits <= word {
                costs.widening_product()
            } else {
                costs.wide_product()
            };
            let scale = 2 * costs.extend + product + 2 * costs.shift + costs.add;
            Some(if saturating { scale + 2 * (costs.add + select) } else { scale })
        }
        "shl" if rest.starts_with("sat") => Some(2 * costs.shift + costs.add + select),
        _ => None,
    }
}

/// The operations of `function` by class, width-weighted, as `name=count,...`:
/// what `function_bytes` prices, for the fit of a target's byte table against
/// the bytes written (`tools/6502/size_fit.py`).
pub fn operation_counts(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    function: &llrm_mir::module::Function,
    costs: &llrm_mir::target::OperationCosts,
) -> String {
    use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
    use llrm_mir::tti::Ty;
    let words = |ty: Option<llrm_mir::types::TypeId>| match ty.and_then(|ty| Ty::of(&module.context.types, ty)) {
        Some(Ty::Int(bits)) if costs.width_scaled => costs.words(bits),
        _ => 1,
    };
    let mut c = std::collections::BTreeMap::<&str, i64>::new();
    for (_, inst) in function.walk() {
        let operation = function.instruction(inst);
        let first = operation.operands.first().and_then(|&operand| function.operand_type(&module.context, operand));
        let result = operation.result.map(|value| function.value(value).ty);
        let constant_second = matches!(
            operation.operands.get(1),
            Some(llrm_mir::module::Operand::Constant(_))
        );
        let (class, n): (&str, i64) = match &operation.opcode {
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) if constant_second => ("addc", words(first)),
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => ("add", words(first)),
            Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) => ("logic", words(first)),
            Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) if constant_second => {
                ("shiftc", words(first))
            }
            Opcode::Binary(BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) => ("shiftv", words(first)),
            Opcode::Binary(BinaryOp::Mul) => ("mul", words(first)),
            Opcode::Binary(BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem) => ("div", words(first)),
            Opcode::Binary(_) => ("fp", 1),
            Opcode::Cast(
                CastOp::Trunc | CastOp::BitCast | CastOp::PtrToInt | CastOp::IntToPtr | CastOp::AddrSpaceCast,
            ) => ("castfree", 1),
            Opcode::Cast(CastOp::ZExt | CastOp::SExt) => ("ext", words(result)),
            Opcode::Cast(_) => ("castfp", 1),
            Opcode::Load { .. } | Opcode::Store { .. } => {
                let pointer = match operation.opcode {
                    Opcode::Load { .. } => operation.operands[0],
                    _ => operation.operands[1],
                };
                let (base, _) = llrm_mir::valuetracking::pointer_base_with_constant_offset(
                    &module.context,
                    layout,
                    function,
                    pointer,
                );
                let direct = match base {
                    llrm_mir::module::Operand::Constant(_) => true,
                    llrm_mir::module::Operand::Value(value) => {
                        matches!(
                            function.value(value).def,
                            llrm_mir::module::ValueDef::Instruction(i) if matches!(function.instruction(i).opcode, Opcode::Alloca { .. })
                        )
                    }
                    _ => false,
                };
                match (&operation.opcode, direct) {
                    (Opcode::Load { .. }, true) => ("loadd", words(result)),
                    (Opcode::Load { .. }, false) => ("loadi", words(result)),
                    (_, true) => ("stored", words(first)),
                    (_, false) => ("storei", words(first)),
                }
            }
            Opcode::GetElementPtr { .. }
                if operation.operands[1..].iter().all(|o| matches!(o, llrm_mir::module::Operand::Constant(_))) =>
            {
                ("gepc", 1)
            }
            Opcode::GetElementPtr { .. } => ("gep", 1),
            Opcode::ICmp(_) if constant_second => ("icmpc", words(first)),
            Opcode::ICmp(_) => ("icmp", words(first)),
            Opcode::FCmp(_) => ("fcmp", 1),
            Opcode::Select => ("select", words(result)),
            Opcode::Br => ("br", 1),
            Opcode::Switch => ("switch", (operation.operands.len() as i64 - 2) / 2 + 1),
            Opcode::Phi => ("phi", words(result)),
            Opcode::Call(_) | Opcode::Invoke(_) => {
                for &argument in operation.operands.iter().skip(1) {
                    let kind = if matches!(argument, llrm_mir::module::Operand::Constant(_)) {
                        "callargc"
                    } else {
                        "callargv"
                    };
                    *c.entry(kind).or_default() += words(function.operand_type(&module.context, argument));
                }
                ("call", 1)
            }
            Opcode::Ret => ("ret", 1),
            _ => ("other", 0),
        };
        *c.entry(class).or_default() += n;
    }
    c.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")
}

/// SZ2: the bytes `function` becomes, by the target's byte prices
/// (`Machine::size_costs`, the opcode table's encodings): each operation is
/// the instructions it lowers to, a dword where the word is smaller
/// carrying its operand-size prefix, a divide by a constant its multiply-high
/// sequence. The `CodeSize` units of the cost model count instructions of
/// the pin's x86, not bytes of a 16-bit encoding.
pub fn function_bytes(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    costs: &llrm_mir::target::OperationCosts,
    function: &llrm_mir::module::Function,
) -> i64 {
    use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
    use llrm_mir::tti::Ty;
    let word = layout.pointer(0).bits.max(16);
    // A target with a native width states its prices per native word (`OperationCosts::words`): an `i32` add on an
    // 8-bit machine is four byte adds. 1 where it states none.
    let words = |ty: Option<llrm_mir::types::TypeId>| match ty.and_then(|ty| Ty::of(&module.context.types, ty)) {
        Some(Ty::Int(bits)) if costs.width_scaled => costs.words(bits),
        _ => 1,
    };
    // The access names its object: a global, or a stack slot, through constant offsets only.
    let direct = |pointer| {
        let (base, _) =
            llrm_mir::valuetracking::pointer_base_with_constant_offset(&module.context, layout, function, pointer);
        match base {
            llrm_mir::module::Operand::Constant(_) => true,
            llrm_mir::module::Operand::Value(value) => matches!(
                function.value(value).def,
                llrm_mir::module::ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Alloca { .. })
            ),
            _ => false,
        }
    };
    function
        .walk()
        .map(|(_, inst)| {
            let operation = function.instruction(inst);
            let ty = operation.operands.first().and_then(|&operand| function.operand_type(&module.context, operand));
            let kind = ty.and_then(|ty| Ty::of(&module.context.types, ty));
            let wide = matches!(kind, Some(Ty::Int(bits)) if bits > word);
            let prefix = if wide { costs.prefix } else { 0 };
            let float = matches!(kind, Some(Ty::Float | Ty::Double | Ty::X87));
            let constant_divisor = operation
                .operands
                .get(1)
                .is_some_and(
                    |&operand| matches!(
                        operand,
                        llrm_mir::module::Operand::Constant(id) if matches!(module.context.get(id).kind, llrm_mir::context::ConstantKind::Int(_))
                    ),
                );
            let expanded = (costs.expanded_intrinsics && matches!(operation.opcode, Opcode::Call(_) | Opcode::Invoke(_)))
                .then(|| llrm_mir::memory::callee(&module.context, function, inst).and_then(|global| module.global(global).name.as_deref()))
                .flatten()
                .and_then(|name| expanded_intrinsic(costs, word, name));
            match &operation.opcode {
                Opcode::Binary(op) => match op {
                    BinaryOp::FAdd | BinaryOp::FSub => costs.float_add,
                    BinaryOp::FMul => costs.float_multiply,
                    BinaryOp::FDiv | BinaryOp::FRem => costs.float_divide,
                    BinaryOp::Mul => {
                        let product = if !wide {
                            costs.multiply
                        } else if extended_from_word(module, function, &operation.operands, word) {
                            costs.widening_product()
                        } else {
                            costs.wide_product()
                        };
                        product * words(ty) + prefix
                    }
                    BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem if constant_divisor => {
                        // The multiply-high sequence: the constant, the high
                        // multiply, two shifts and two adds (a remainder
                        // multiplies back and subtracts).
                        let rem = matches!(op, BinaryOp::URem | BinaryOp::SRem);
                        2 * costs.r#move
                            + costs.multiply_long
                            + 2 * costs.shift
                            + 2 * costs.add
                            + if rem { costs.multiply_long + costs.add } else { 0 }
                            + 4 * costs.prefix
                    }
                    BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem => {
                        (if wide { costs.divide_long } else { costs.divide }) * words(ty) + prefix
                    }
                    BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => costs.shift * words(ty) + prefix,
                    _ => costs.add * words(ty) + prefix,
                },
                Opcode::Cast(cast) => match cast {
                    CastOp::Trunc | CastOp::BitCast | CastOp::AddrSpaceCast | CastOp::PtrToInt | CastOp::IntToPtr => 0,
                    CastOp::FPToSI | CastOp::FPToUI | CastOp::SIToFP | CastOp::UIToFP => {
                        costs.float_store + costs.float_load
                    }
                    CastOp::FPTrunc | CastOp::FPExt | CastOp::FPRound => costs.float_store + costs.float_load,
                    CastOp::ZExt | CastOp::SExt => costs.extend,
                },
                Opcode::Load { .. } => {
                    let price =
                        if costs.direct_load > 0 && direct(operation.operands[0]) { costs.direct_load } else { costs.load };
                    (if float { costs.float_load } else { price * words(operation.result.map(|value| function.value(value).ty)) })
                        + prefix
                }
                Opcode::Store { .. } => {
                    let stored =
                        operation.operands.first().and_then(|&operand| function.operand_type(&module.context, operand));
                    let stored = stored.and_then(|ty| Ty::of(&module.context.types, ty));
                    let wide = matches!(stored, Some(Ty::Int(bits)) if bits > word);
                    (if matches!(stored, Some(Ty::Float | Ty::Double | Ty::X87)) {
                        costs.float_store
                    } else {
                        (if costs.direct_store > 0 && direct(operation.operands[1]) { costs.direct_store } else { costs.store })
                            * words(operation.operands.first().and_then(|&operand| function.operand_type(&module.context, operand)))
                    }) + if wide { costs.prefix } else { 0 }
                }
                Opcode::GetElementPtr { .. }
                    if costs.constant_offsets_fold
                        && operation.operands[1..].iter().all(|operand| matches!(operand, llrm_mir::module::Operand::Constant(_))) =>
                {
                    0
                }
                Opcode::GetElementPtr { .. } => costs.address,
                // A float compare is the target's own: fcom and the status word on x87, a libcall where the FPU is
                // software.
                Opcode::FCmp(_) => costs.add + costs.float_compare,
                Opcode::ICmp(_) => costs.add * words(ty) + prefix,
                Opcode::Select => costs.branch + costs.r#move,
                Opcode::Br if costs.branch_in_compare => 0,
                Opcode::Br => costs.branch,
                Opcode::Switch => costs.branch * 2,
                // `llvm.lifetime.*`, `llvm.assume` and the rest the cost model calls free emit no instruction: priced as
                // calls they were 60 B of 267 in stack_arrays_copy (object 94). A target opts in (`free_intrinsics`):
                // the others' inlining bars were fit with the markers priced as calls.
                Opcode::Call(_) | Opcode::Invoke(_)
                    if costs.free_intrinsics
                        && llrm_mir::memory::callee(&module.context, function, inst)
                        .and_then(|global| module.global(global).name.as_deref())
                        .is_some_and(llrm_mir::tti::is_free_intrinsic) =>
                {
                    0
                }
                // `llvm.smul.fix.sat` and the like expand to a widening multiply, shifts and compares: priced as calls they
                // were 12 B each in fixed_point, whose objects are 1.6 kB at the 68020 and 2.9 kB at the 68000. A target
                // opts in (`expanded_intrinsics`).
                Opcode::Call(_) | Opcode::Invoke(_) if expanded.is_some() => expanded.unwrap_or(0),
                Opcode::Call(_) | Opcode::Invoke(_) => {
                    costs.call
                        + costs.argument
                            * operation
                                .operands
                                .iter()
                                .skip(1)
                                .map(|&argument| words(function.operand_type(&module.context, argument)))
                                .sum::<i64>()
                }
                Opcode::Ret => costs.return_,
                Opcode::Phi => costs.r#move * words(operation.result.map(|value| function.value(value).ty)),
                Opcode::Alloca { .. } | Opcode::Freeze | Opcode::ExtractValue(_) | Opcode::InsertValue(_) => 0,
                _ => costs.add,
            }
        })
        .sum::<i64>()
        .saturating_add(costs.function)
        .saturating_mul(costs.function_scale)
        / 100
}

/// INL-HOT: the most trials of hot sites a caller tries.
const HOT_TRIALS: usize = 4;

/// SZ2: whether a trial's copies are kept. Priced (speed, a byte rate): the callee must go with them and the growth
/// must be worth the clocks saved. At size the instruction count decides (`tried <= best + credit`), but a callee that
/// goes with its last calls, accepted only by the credit for the callee (the copies alone count more), must also leave the bytes no larger: the count priced a float compare as an add, and
/// `canon` (a libcall per site on m68k) was inlined five times for +28 B at -Os.
#[allow(clippy::too_many_arguments)]
/// Bytes a site may add where INL-SPEED-OS keeps a trial for the clocks it saves.
const INLINE_SPEED_BYTES_PER_SITE: i64 = 4;

/// Instructions a call site runs that its copy does not: the call and the callee's return.
const INLINE_CALL_AND_RETURN: i64 = 2;

/// DEVIATE INL-SPEED-OS: whether an -Os trial is kept for its clocks: its sites run at least `least` times per
/// call of the caller, `once` is the clocks one run of the sites saves (the latency model prices a call as its
/// callee's body, so a copy that merely replaces it reads 0: it also drops the call and the return), and the copy
/// adds at most `INLINE_SPEED_BYTES_PER_SITE` bytes a site.
fn speed_keeps_at_size(
    least: i64,
    trips: f64,
    once: i64,
    sites: usize,
    growth: i64,
) -> bool {
    let once = once + INLINE_CALL_AND_RETURN * sites as i64;
    trips >= least as f64 && (once as f64 * trips) as i64 > 0 && growth <= INLINE_SPEED_BYTES_PER_SITE * sites as i64
}

fn trial_accepted(
    byte_gate: bool,
    by_bytes: bool,
    keeps_shrinking: bool,
    priced: bool,
    hot: bool,
    tried: i64,
    best: i64,
    credit: i64,
    saved: i64,
    growth: i64,
    rate: i64,
    kept_bytes: i64,
) -> bool {
    if priced {
        // A copy that leaves the module no larger and runs in fewer clocks needs no callee to go with it.
        // INL-HOT: a hot site needs no callee going with the copy; the rate alone decides.
        return (credit > 0 || hot || (keeps_shrinking && growth <= 0 && saved > 0)) && growth * rate <= saved * 1000;
    }
    if by_bytes {
        // Bytes decide; clocks only where they tie exactly.
        return growth < 0 || (growth == 0 && saved > 0);
    }
    (if byte_gate { tried <= best || (tried <= best + credit && growth <= 0) } else { tried <= best + credit })
        || (saved > 0 && growth * rate <= saved * 1000)
        || (rate == 0 && credit == 0 && -growth * 8 >= kept_bytes)
}

/// SZ2: the bytes the module loses when the callees of a trial's `sites` go
/// with them: each callee once, however many of its sites the trial inlines.
fn dying_callees_bytes(
    module: &Module,
    sites: &[GlobalId],
    bytes: impl Fn(&llrm_mir::module::Function) -> i64,
) -> i64 {
    let mut distinct = sites.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    distinct.into_iter().filter_map(|callee| module.global(callee).function()).map(|body| bytes(body)).sum()
}

/// SZ2: the clocks of `function` run once, by the target's latency model.
fn function_clocks(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    tti: &dyn llrm_mir::tti::Tti,
    function: &llrm_mir::module::Function,
) -> i64 {
    function
        .walk()
        .map(|(_, inst)| {
            llrm_mir::instructioncost::instruction_cost(
                tti,
                &module.context,
                layout,
                &module.globals,
                function,
                inst,
                llrm_mir::tti::CostKind::Latency,
            )
            .value
        })
        .sum()
}

/// Trips a loop is priced at where ScalarEvolution proves no fewer than
/// this: the conventional ten.
const TRIAL_TRIPS: f64 = 10.0;

/// SZ2 with `Tti::inline_trial_by_trips`: the clocks of `function` per entry,
/// each instruction repeated by the trips of the loops around it (ten, or the
/// fewer ScalarEvolution proves). The same weight on both sides of a trial: a
/// branch estimate weighs a loop of the callee and its copy differently (20
/// and 32 for the one loop of `recsum`), more than the clocks a copy saves a
/// trip. Else [`function_clocks`].
fn trial_clocks(
    by_trips: bool,
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    tti: &dyn llrm_mir::tti::Tti,
    function: &llrm_mir::module::Function,
) -> i64 {
    if !by_trips {
        return function_clocks(module, layout, tti, function);
    }
    let mut session = llrm_analysis::scev::analysis::ScevSession::within(
        &module.context,
        &module.globals,
        &module.metadata,
        function,
    );
    let weights: llrm_mir::dense::IdMap<llrm_mir::module::BlockId, f64> =
        session.ask_within(&module.context, layout, &module.globals, &module.metadata, function, |scev, cx| {
            function
                .layout()
                .iter()
                .map(|&block| {
                    let mut weight = 1.0;
                    let mut around = cx.loops.get_loop_for(block);
                    while let Some(found) = around {
                        let proven = f64::from(scev.get_small_constant_max_trip_count(cx, found.get_header()));
                        weight *= if proven > 0.0 { proven.min(TRIAL_TRIPS) } else { TRIAL_TRIPS };
                        around = found.get_parent_loop();
                    }
                    (block, weight)
                })
                .collect()
        });
    function
        .walk()
        .map(|(_, inst)| {
            let clocks = llrm_mir::instructioncost::instruction_cost(
                tti,
                &module.context,
                layout,
                &module.globals,
                function,
                inst,
                llrm_mir::tti::CostKind::Latency,
            )
            .value;
            clocks as f64 * function.parent(inst).and_then(|block| weights.get(&block).copied()).unwrap_or(1.0)
        })
        .sum::<f64>() as i64
}

/// Units a rerun of the Simplify list over a settled body must save to stay:
/// one is the wobble of the operand-sensitive estimate (recmany_o1 "saved" 1 by
/// an add's operand and kept a Reassociate reorder, +16 B). Of the 75 reruns
/// kept on the corpus, 14 save one unit and the rest more.
const GUARD: i64 = 1;

/// SZ2: the code size of body `id`, by the target's cost model for bytes (its
/// size view, in encoded bytes, where it has one).
fn body_size(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    tti: &dyn llrm_mir::tti::Tti,
    id: GlobalId,
    speed: bool,
) -> i64 {
    // Only the measured view, and only for a speed caller: the 6502 and m68k
    // views stay out of the trials, as at the tip.
    let view = tti.inline_size_view().filter(|_| speed && tti.inline_size_view_is_measured());
    sized(module, layout, view.as_deref().unwrap_or(tti), id, true)
}

/// [`body_size`] with or without its phis: a phi is a copy the allocator
/// coalesces, and the single-input ones LCSSA left are folded later anyway.
fn sized(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    tti: &dyn llrm_mir::tti::Tti,
    id: GlobalId,
    phis: bool,
) -> i64 {
    let function = module.global(id).function().expect("a body");
    let float =
        |ty: llrm_mir::types::TypeId| llrm_mir::tti::Ty::of(&module.context.types, ty).is_some_and(|ty| ty.is_float());
    function
        .walk()
        .map(|(_, inst)| {
            let operation = function.instruction(inst);
            if !phis && matches!(operation.opcode, llrm_mir::opcode::Opcode::Phi) {
                return 0;
            }
            let units = llrm_mir::instructioncost::instruction_cost(
                tti,
                &module.context,
                layout,
                &module.globals,
                function,
                inst,
                llrm_mir::tti::CostKind::CodeSize,
            )
            .value;
            let moves = matches!(
                operation.opcode,
                llrm_mir::opcode::Opcode::Load { .. }
                    | llrm_mir::opcode::Opcode::Store { .. }
                    | llrm_mir::opcode::Opcode::Call(_)
            );
            let touches = !moves
                && (float(operation.ty)
                    || operation
                        .operands
                        .iter()
                        .any(|&operand| function.operand_type(&module.context, operand).is_some_and(|ty| float(ty))));
            if touches { units * tti.float_code_size_percent() / 100 } else { units }
        })
        .sum()
}
