//! ScalarEvolution as entries of llrm-mir's analysis manager, LLVM's
//! `ScalarEvolutionAnalysis` and `AssumptionAnalysis`.
//!
//! # How it works
//! [`Scevs`] holds one [`ScalarEvolution`] per function, made empty and filled
//! by the queries that follow; a pass that changes the function and does not
//! name it in the `PreservedAnalyses` drops it, as the manager drops any entry,
//! and so drops every expression with it. [`with_scev`] gathers what a
//! [`Cx`] borrows (the loops, dominator tree and assumptions the manager
//! holds too) and runs a query closure.
//!
//! The result compares equal to any other: it is a cache of what was asked,
//! like `cfg::Reaching`, and the manager's replay check compares a kept result
//! with a fresh one.
//!
//! # Used by
//! `decide` (`_implied`).

use std::cell::RefCell;
use std::rc::Rc;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Function;
use llrm_mir::passes::{Analyses, Analysis, Dominators, Loops};
use llrm_mir::valuetracking::AssumptionCache;

use super::{Cx, ScalarEvolution};

/// The expressions of a function, filled by the queries.
pub struct ScevCache(RefCell<ScalarEvolution>);

impl PartialEq for ScevCache {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl std::fmt::Debug for ScevCache {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str("ScalarEvolution")
    }
}

impl ScevCache {
    /// The expressions out of the entry, which is left empty: a pass that
    /// keeps them current while it edits holds them for its run and `put`s
    /// them back before it says it kept `Scevs`.
    pub fn take(&self) -> ScalarEvolution {
        std::mem::replace(&mut *self.0.borrow_mut(), ScalarEvolution::new())
    }

    /// `scev` as the entry's expressions again.
    pub fn put(
        &self,
        scev: ScalarEvolution,
    ) {
        *self.0.borrow_mut() = scev;
    }
}

/// LLVM's `ScalarEvolutionAnalysis`.
pub struct Scevs;

impl Analysis for Scevs {
    type Result = ScevCache;
    const NAME: &'static str = "scev";

    fn run(
        _: &llrm_mir::context::Context,
        _: &DataLayout,
        _: &Function,
        _: &mut Analyses,
    ) -> ScevCache {
        ScevCache(RefCell::new(ScalarEvolution::new()))
    }

    /// `ScalarEvolution::verify`: the result equals every other, so the
    /// expressions it holds are checked against the function as it stands.
    fn consistent(
        old: &ScevCache,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> bool {
        let loops = analyses.get::<Loops>(context, layout, function);
        let tree = analyses.get::<Dominators>(context, layout, function);
        let assumptions = analyses.get::<Assumes>(context, layout, function);
        let outer = Rc::clone(analyses.outer());
        let cx = Cx {
            context,
            globals: &outer.globals,
            metadata: &outer.metadata,
            function,
            layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let held = old.0.borrow();
        held.verify(&cx).is_ok() && held.verify_same(&cx).is_ok()
    }
}

/// LLVM's `AssumptionAnalysis`: the `llvm.assume` calls of the function.
pub struct Assumes;

impl Analysis for Assumes {
    type Result = AssumptionCache;
    const NAME: &'static str = "assumes";

    fn run(
        context: &llrm_mir::context::Context,
        _: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> AssumptionCache {
        let outer = analyses.outer();
        AssumptionCache::of(context, &outer.globals, &outer.metadata, function)
    }
}

/// What a query reads, held by `Rc` so that it borrows nothing of the manager
/// while a pass reads other analyses.
pub struct ScevView {
    outer: Rc<llrm_mir::passes::Outer>,
    scevs: Rc<ScevCache>,
    loops: Rc<LoopInfo>,
    tree: Rc<DomTree>,
    assumptions: Rc<AssumptionCache>,
}

impl ScevView {
    pub fn of(
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        function: &Function,
        analyses: &mut Analyses,
    ) -> Self {
        Self {
            outer: Rc::clone(analyses.outer()),
            scevs: analyses.get::<Scevs>(context, layout, function),
            loops: analyses.get::<Loops>(context, layout, function),
            tree: analyses.get::<Dominators>(context, layout, function),
            assumptions: analyses.get::<Assumes>(context, layout, function),
        }
    }

    /// Runs `query` with `function`'s expressions and the context they are
    /// read in. The closure must not ask for the expressions again.
    pub fn ask<R>(
        &self,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        function: &Function,
        query: impl FnOnce(&mut ScalarEvolution, &Cx) -> R,
    ) -> R {
        let cx = Cx {
            context,
            globals: &self.outer.globals,
            metadata: &self.outer.metadata,
            function,
            layout,
            loops: &self.loops,
            tree: &self.tree,
            assumptions: &self.assumptions,
        };
        query(&mut self.scevs.0.borrow_mut(), &cx)
    }
}

/// A [`ScevView`] made at the first query: a pass that rarely asks pays for
/// the loops and dominator tree it would otherwise ask the manager for at
/// the start.
pub struct LazyScevView<'a> {
    analyses: std::cell::RefCell<&'a mut Analyses>,
    view: std::cell::OnceCell<ScevView>,
}

impl<'a> LazyScevView<'a> {
    pub fn new(analyses: &'a mut Analyses) -> Self {
        Self { analyses: std::cell::RefCell::new(analyses), view: std::cell::OnceCell::new() }
    }

    /// [`ScevView::ask`], the view made on the first call.
    pub fn ask<R>(
        &self,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        function: &Function,
        query: impl FnOnce(&mut ScalarEvolution, &Cx) -> R,
    ) -> R {
        let view = self.view.get_or_init(|| ScevView::of(context, layout, function, &mut self.analyses.borrow_mut()));
        view.ask(context, layout, function, query)
    }
}

/// One function's expressions and the context to read them in, for a pass
/// that has no analysis manager (`exitfold`): made once, asked many times,
/// valid while the function is not changed.
pub struct ScevSession {
    tree: DomTree,
    loops: LoopInfo,
    assumptions: AssumptionCache,
    scev: ScalarEvolution,
}

impl ScevSession {
    pub fn of(
        context: &llrm_mir::context::Context,
        function: &Function,
        outer: &llrm_mir::passes::Outer,
    ) -> Self {
        Self::within(context, &outer.globals, &outer.metadata, function)
    }

    /// [`ScevSession::of`] for a caller that holds the module's globals and
    /// metadata, not its `Outer` (a memory `Unit`).
    pub fn within(
        context: &llrm_mir::context::Context,
        globals: &[llrm_mir::module::GlobalValue],
        metadata: &[llrm_mir::module::MetadataNode],
        function: &Function,
    ) -> Self {
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::of(context, globals, metadata, function);
        Self { tree, loops, assumptions, scev: ScalarEvolution::new() }
    }

    /// `scev` as the session's expressions, for a session that carries on the
    /// work of an earlier pass (`Scevs`).
    pub fn adopt(
        &mut self,
        scev: ScalarEvolution,
    ) {
        self.scev = scev;
    }

    /// The expressions, to hand back to the manager.
    pub fn into_scev(self) -> ScalarEvolution {
        self.scev
    }

    /// The expressions dropped: after the function changed in ways that leave
    /// the CFG, and so the tree and loops, as they were.
    pub fn forget_expressions(&mut self) {
        self.scev = ScalarEvolution::new();
    }

    /// The expressions the edits since `mark` made stale dropped, the others
    /// kept: after edits that leave the CFG as it was. All are dropped where
    /// the log from `mark` is gone or the edits changed the CFG.
    pub fn forget_since(
        &mut self,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        globals: &[llrm_mir::module::GlobalValue],
        metadata: &[llrm_mir::module::MetadataNode],
        function: &Function,
        mark: llrm_mir::module::Mark,
    ) {
        self.forget_since_keeping(context, layout, globals, metadata, function, mark, false);
    }

    /// [`Self::forget_since`], where `keep_counts` the trip counts and the
    /// expressions of values whose operands were replaced stay
    /// (`forget_touched_keeping`).
    #[allow(clippy::too_many_arguments)]
    pub fn forget_since_keeping(
        &mut self,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        globals: &[llrm_mir::module::GlobalValue],
        metadata: &[llrm_mir::module::MetadataNode],
        function: &Function,
        mark: llrm_mir::module::Mark,
        keep_counts: bool,
    ) {
        match function.touched_since(mark) {
            Some(touched)
                if touched.cfg.is_empty() && touched.blocks_created.is_empty() && touched.blocks_erased.is_empty() =>
            {
                self.ask_within(context, layout, globals, metadata, function, |se, cx| {
                    se.forget_touched_keeping(cx, &touched, keep_counts)
                });
            }
            _ => self.forget_expressions(),
        }
    }

    /// `ScalarEvolution::verify` over the function as it is now: the first
    /// kept expression that a new one would not give, said.
    pub fn verify(
        &mut self,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        globals: &[llrm_mir::module::GlobalValue],
        metadata: &[llrm_mir::module::MetadataNode],
        function: &Function,
    ) -> Result<(), String> {
        self.ask_within(context, layout, globals, metadata, function, |se, cx| se.verify(cx))
    }

    /// Runs `query` with the expressions and the context they are read in.
    pub fn ask<R>(
        &mut self,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        function: &Function,
        outer: &llrm_mir::passes::Outer,
        query: impl FnOnce(&mut ScalarEvolution, &Cx) -> R,
    ) -> R {
        self.ask_within(context, layout, &outer.globals, &outer.metadata, function, query)
    }

    /// The expressions and the [`Env`] to expand them in, for a pass that
    /// edits the function between queries (the CFG stays as it was).
    pub fn split<'a>(
        &'a mut self,
        layout: &'a DataLayout,
        globals: &'a [llrm_mir::module::GlobalValue],
        metadata: &'a [llrm_mir::module::MetadataNode],
    ) -> (&'a mut ScalarEvolution, super::expander::Env<'a>) {
        let env = super::expander::Env {
            globals,
            metadata,
            layout,
            loops: &self.loops,
            tree: &self.tree,
            assumptions: &self.assumptions,
        };
        (&mut self.scev, env)
    }

    /// [`ScevSession::ask`] for a caller that holds the globals and metadata.
    pub fn ask_within<R>(
        &mut self,
        context: &llrm_mir::context::Context,
        layout: &DataLayout,
        globals: &[llrm_mir::module::GlobalValue],
        metadata: &[llrm_mir::module::MetadataNode],
        function: &Function,
        query: impl FnOnce(&mut ScalarEvolution, &Cx) -> R,
    ) -> R {
        let cx = Cx {
            context,
            globals,
            metadata,
            function,
            layout,
            loops: &self.loops,
            tree: &self.tree,
            assumptions: &self.assumptions,
        };
        query(&mut self.scev, &cx)
    }
}
