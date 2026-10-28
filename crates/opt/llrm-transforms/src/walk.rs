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
//! Once per compile under `Options::llvm_order`, after the Early stage.
//!
//! # See also
//! LLVM `PassBuilderPipelines.cpp` (`buildModuleSimplificationPipeline`:
//! `IPSCCPPass`, `GlobalOptPass`, `DeadArgumentEliminationPass` before the
//! CGSCC walk), `DeadArgumentElimination.cpp`.

use std::collections::BTreeSet;

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
        for (at, id) in crate::ipsccp::run_ipsccp_with(program, &mut modules) {
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
        let mut cloning = crate::ipacp::Cloning::default();
        for at in 0..count {
            let (made, before) = reporting_marked(&mut program.modules[at], &mut modules[at], |module| {
                crate::ipacp::cloned(
                    module,
                    &program.layout,
                    &procedures[at],
                    &private[at],
                    &costs,
                    self.inline.cp_clone,
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
            }
        }
        for at in 0..count {
            let ((removed, unsound), before) =
                reporting_marked(&mut program.modules[at], &mut modules[at], crate::deadargs::removed_with);
            vouched(&program.modules[at], removed.iter().copied().filter(|id| !unsound.contains(id)), &before);
            changed |= !removed.is_empty();
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
    /// The Simplify list over one body, as the interprocedural step reruns it.
    pub simplify: Box<dyn FnMut(&mut Module, &mut ModuleAnalyses, GlobalId, &str)>,
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
        let params = crate::inlinecost::InlineParams {
            allow_recursive_call: self.inline.recursive,
            last_call_to_static: self.inline.last,
            ..crate::inlinecost::InlineParams::from_level(if self.inline.limit >= 250 { 3 } else { 2 })
        };
        let size = self.inline.caller_size();
        for at in 0..program.modules.len() {
            let wanted: BTreeSet<GlobalId> = procedures(program, at).into_iter().collect();
            let private = private(program, at);
            let module = &mut program.modules[at];
            let mut recursive = crate::inline::recursive(module);
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
            for members in components {
                for &id in &members {
                    // `DevirtSCCRepeatedPass<4>`: a call the simplification
                    // made direct is inlined by another run
                    // of the inliner over the body.
                    let mut repeats = 0;
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
                                params,
                                size,
                                self.inline.limit == 0,
                                tti.as_deref(),
                                decided.as_mut(),
                                (tti.is_some() && size != crate::inlinecost::CallerSize::Speed).then_some(&mut trials),
                            )
                        };
                        let mut declared = declared(&mut modules[at], module);
                        let (context, function) = function_mut(module, id);
                        let was = calls_of(context, function);
                        // DEVIATE (recursion): a recursive callee inlined here
                        // has its own calls peeled in
                        // the copy (gcc's peeling, not
                        // at -Os).
                        let peeled: llrm_support::hash::IndexMap<GlobalId, crate::inline::Candidate> = constant
                            .values()
                            .filter(|one| one.self_calls.is_some() && !self.inline.single && self.inline.recursive)
                            .filter_map(|one| Some((one.callee?, one.clone())))
                            .collect();
                        let by = crate::inline::Caller {
                            layout: &program.layout,
                            recursive: recursive.contains(&id),
                            self_inlining: None,
                            peeling: !self.inline.single && self.inline.recursive,
                        };
                        let spliced = crate::inline::expanded_all(
                            context,
                            function,
                            &by,
                            &peeled,
                            Some(&constant),
                            &mut declared,
                        )? > 0;
                        // A recursive callee's peeled copies spliced here make
                        // this body one with self-copies too (`SELF_COPIED`).
                        if spliced && !peeled.is_empty() {
                            function_mut(module, id)
                                .1
                                .attrs
                                .push(llrm_mir::opcode::Attribute::Flag(SELF_COPIED.to_owned()));
                        }
                        // DEVIATE (recursion): a tree-recursive function waits
                        // for its tail-call elimination
                        // until its copies are made
                        // (`SelfInline`, after ipa-cp has seen it whole).
                        if self.inline.recursive
                            && self_calls(module, &program.layout, id, self.inline.single).is_some()
                        {
                            function_mut(module, id)
                                .1
                                .attrs
                                .push(llrm_mir::opcode::Attribute::Flag(crate::tailcallelim::DEFERRED.to_owned()));
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
                            && function_mut(module, id).1.walk().count() <= TRIAL_CALLER)
                            .then(|| function_mut(module, id).1.clone());
                        let mid = calls_of(&module.context, module.global(id).function().expect("a procedure"));
                        (self.simplify)(module, &mut modules[at], id, "walk.");
                        if let (Some(before), Some(tti)) = (spliced_body, tti.as_deref()) {
                            // SZ2: each refused site is inlined into the body
                            // as it stood before
                            // the Simplify list, which runs again;
                            // it stays if the body is smaller. The callee stays
                            // either way.
                            let mut best = body_size(module, &program.layout, tti, id);
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
                                    units.push((calls, body_size(module, &program.layout, tti, callee)));
                                } else {
                                    units.extend(calls.into_iter().map(|one| (vec![one], 0)));
                                }
                            }
                            let mut accepted: Vec<llrm_mir::module::InstId> = Vec::new();
                            for (calls, credit) in units {
                                let kept = function_mut(module, id).1.clone();
                                let mut chosen = accepted.clone();
                                chosen.extend(calls.iter().copied());
                                let available: llrm_support::hash::SparseIdMap<_, _> =
                                    chosen.iter().map(|one| (*one, trials[one].clone())).collect();
                                *function_mut(module, id).1 = before.clone();
                                let mut declared = crate::interprocedural::declared(&mut modules[at], module);
                                let (context, function) = function_mut(module, id);
                                crate::inline::expanded_all(
                                    context,
                                    function,
                                    &by,
                                    &peeled,
                                    Some(&available),
                                    &mut declared,
                                )?;
                                if declared.place(module)? > 0 {
                                    modules[at].invalidate(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                                }
                                modules[at].changed(id);
                                modules[at]
                                    .invalidate_bodies(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
                                (self.simplify)(module, &mut modules[at], id, "walk.trial.");
                                let tried = body_size(module, &program.layout, tti, id);
                                if remarks {
                                    eprintln!(
                                        "{} ({} instructions) SZ2 trial of {} call(s) to '{}': {} against {} (+{} for the callee)",
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
                                        credit
                                    );
                                }
                                if tried <= best + credit {
                                    best = tried;
                                    accepted.extend(calls);
                                } else {
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
                        // A callee the splices or the simplification added, with a body: a parameter
                        // that was a function became the copy's callee, or the call was devirtualized
                        // (a table of functions made a switch of calls by `jumptableswitch`).
                        // Or more calls of a callee with a body than before the splices: the copies
                        // brought call sites that `InlineFunction` hands to the inliner's worklist.
                        let exposed = now.iter().any(|(callee, count)| {
                            let before = was.iter().find(|(seen, _)| seen == callee).map_or(0, |&(_, count)| count);
                            (before == 0 || (spliced && *count > before && !recursive.contains(callee)))
                                && module.global(*callee).function().is_some_and(|one| !one.layout().is_empty())
                        });
                        live.spliced(module, &mut modules[at], was, now.clone());
                        if recursive.contains(&id) {
                            recursive = crate::inline::recursive(module);
                        }
                        repeats += 1;
                        if repeats >= DEVIRT_REPEATS || !exposed {
                            break;
                        }
                    }
                }
                // `PostOrderFunctionAttrsPass`: once per component, after its
                // bodies are simplified.
                if !members.is_empty() {
                    let mut stated = false;
                    for id in llrm_analysis::functionattrs::post_order(
                        module,
                        &program.layout,
                        program.target.spaces(),
                        &members,
                    ) {
                        modules[at].changed(id);
                        stated = true;
                    }
                    // What it stated reaches the next body's callees.
                    if stated {
                        modules[at].invalidate(&PreservedAnalyses::none().preserve_module::<GlobalsAA>());
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
fn self_calls(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    id: GlobalId,
    for_size: bool,
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
    if for_size || distinct.len() < 2 {
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
    Some(frequencies)
}

/// The flag a body that had copies of itself made (D10), or took a recursive
/// callee's peeled copies, carries from then on:
/// the simplifier folds the chains of tests those copies make (`n - 1 == 0`,
/// `n - 2 == 0`, ...) through `algebraic::_offset_compared` whoever else reads
/// the sum, which the pin does only with one use.
pub const SELF_COPIED: &str = "llrm-self-copied";

/// DEVIATE (recursion): the copies of a tree-recursive function in itself, as
/// gcc's `recursive_inlining` makes them, to the limits of [`crate::inline`],
/// then its tail-call elimination, which the walk left out
/// (`tailcallelim::DEFERRED`). After both ipa-cp rounds, as gcc runs ipa-cp
/// before its inliner and `pass_tail_recursion` after it: the specializer
/// prices the callee whole (copies first made recchop -O3 decline its clones:
/// 6368 clocks against 6209) and the copies are of the tree (fib: 132006
/// clocks, 294986 copied after tail-call elimination made it a loop).
///
/// Ruling 2026-10-22 (oversight): stays a numbered DEVIATE, owner w-6, review
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
        for at in 0..program.modules.len() {
            let module = &mut program.modules[at];
            let marked: Vec<GlobalId> =
                module.functions().filter(|(_, _, function)| waiting(function)).map(|(id, _, _)| id).collect();
            for id in marked {
                let mut copies = 0;
                let frequencies = self_calls(module, &program.layout, id, self.for_size);
                if let Some(frequencies) = frequencies {
                    let mut declared = declared(&mut modules[at], module);
                    let (context, function) = function_mut(module, id);
                    let itself = crate::inline::Candidate {
                        body: std::rc::Rc::new(function.clone()),
                        frame: crate::inline::grown(context, &program.layout, function),
                        moved: false,
                        last_call: false,
                        callee: None,
                        self_calls: None,
                        self_sites: 0,
                    };
                    let available = [(id, itself)].into_iter().collect();
                    let by = crate::inline::Caller {
                        layout: &program.layout,
                        recursive: true,
                        self_inlining: Some(&frequencies),
                        peeling: false,
                    };
                    copies = crate::inline::expanded_all(context, function, &by, &available, None, &mut declared)?;
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
                    function_mut(module, id).1.attrs.push(llrm_mir::opcode::Attribute::Flag(SELF_COPIED.to_owned()));
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

/// SZ2: the largest caller, in instructions, whose refused sites are tried.
const TRIAL_CALLER: usize = 300;

/// SZ2: the code size of body `id`, by the target's cost model for bytes.
fn body_size(
    module: &Module,
    layout: &llrm_mir::datalayout::DataLayout,
    tti: &dyn llrm_mir::tti::Tti,
    id: GlobalId,
) -> i64 {
    let function = module.global(id).function().expect("a body");
    let float =
        |ty: llrm_mir::types::TypeId| llrm_mir::tti::Ty::of(&module.context.types, ty).is_some_and(|ty| ty.is_float());
    function
        .walk()
        .map(|(_, inst)| {
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
            let operation = function.instruction(inst);
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
