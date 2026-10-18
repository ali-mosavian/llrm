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
use llrm_mir::dominators::DominatorTree;
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
    tree: Rc<DominatorTree>,
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
    tree: DominatorTree,
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
        let tree = DominatorTree::new(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::of(context, &outer.globals, &outer.metadata, function);
        Self { tree, loops, assumptions, scev: ScalarEvolution::new() }
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
        let cx = Cx {
            context,
            globals: &outer.globals,
            metadata: &outer.metadata,
            function,
            layout,
            loops: &self.loops,
            tree: &self.tree,
            assumptions: &self.assumptions,
        };
        query(&mut self.scev, &cx)
    }
}
