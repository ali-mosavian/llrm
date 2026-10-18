//! `SCEVExpander`'s surface, as signatures (S6 translates the bodies): what
//! LoopIdiom and LSR call. Every body is `todo!()`.
//!
//! The expander edits the function and interns constants while it reads SCEVs,
//! and a [`Cx`] borrows both. So the expander takes `&mut Context` and
//! `&mut Function` and the rest as an [`Env`], and builds a [`Cx`] over
//! `&*context` and `&*function` for each SCEV query (`Env::cx`). Expansion adds
//! instructions inside existing blocks, so the loops and the dominator tree in
//! the `Env` stay true, as the references LLVM's `SE` holds do.

use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dominators::DominatorTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{Function, GlobalValue, InstId, MetadataNode, Operand};
use llrm_mir::valuetracking::AssumptionCache;

use super::expr::{Scev, Ty};
use super::{Cx, ScalarEvolution};

/// A [`Cx`] without its function.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub layout: &'a DataLayout,
    pub loops: &'a LoopInfo,
    pub tree: &'a DominatorTree,
    pub assumptions: &'a AssumptionCache,
}

impl<'a> Env<'a> {
    /// The [`Cx`] over `function` as it stands now.
    pub fn cx<'b>(
        &self,
        context: &'b Context,
        function: &'b Function,
    ) -> Cx<'b>
    where
        'a: 'b,
    {
        Cx {
            context,
            globals: self.globals,
            metadata: self.metadata,
            function,
            layout: self.layout,
            loops: self.loops,
            tree: self.tree,
            assumptions: self.assumptions,
        }
    }
}

/// `SCEVExpander`.
pub struct Expander {
    name: &'static str,
    insert_point: Option<InstId>,
}

impl Expander {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L192
    /// `SCEVExpander(SE, Name)`. `PreserveLCSSA` is always true.
    pub fn new(name: &'static str) -> Self {
        Self { name, insert_point: None }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L407
    /// `setInsertPoint`: new instructions go before `ip`.
    pub fn set_insert_point(
        &mut self,
        ip: InstId,
    ) {
        self.insert_point = Some(ip);
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L418
    /// `clearInsertPoint`.
    pub fn clear_insert_point(&mut self) {
        self.insert_point = None;
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L313
    /// `isSafeToExpand`.
    pub fn is_safe_to_expand(
        &self,
        _cx: &Cx,
        _se: &mut ScalarEvolution,
        _s: Scev,
    ) -> bool {
        todo!("S6")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L335
    /// `expandCodeFor(SH, Ty, I)`, and `expandCodeFor(SH, Ty)` at the insert
    /// point; `ty` is the type of the result, or none for the SCEV's own.
    pub fn expand_code_for(
        &mut self,
        _env: &Env,
        _context: &mut Context,
        _function: &mut Function,
        _se: &mut ScalarEvolution,
        _s: Scev,
        _ty: Option<Ty>,
    ) -> Operand {
        todo!("S6")
    }

    /// The name given at `new`, the prefix of the instructions it makes.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L587
/// `SCEVExpanderCleaner`. LLVM runs `cleanup` in the destructor; it needs the
/// function, so here it is called: `cleanup(function)` removes the
/// instructions expanded since `new`, `mark_result_used` keeps them.
pub struct ExpanderCleaner<'e> {
    _expander: &'e mut Expander,
    result_used: bool,
}

impl<'e> ExpanderCleaner<'e> {
    pub fn new(expander: &'e mut Expander) -> Self {
        Self { _expander: expander, result_used: false }
    }

    /// `markResultUsed`.
    pub fn mark_result_used(&mut self) {
        self.result_used = true;
    }

    /// `cleanup`.
    pub fn cleanup(
        self,
        _function: &mut Function,
    ) {
        if self.result_used {
            return;
        }
        todo!("S6")
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1617
    /// `applyLoopGuards(Expr, L)`. S5 (guards).
    pub fn apply_loop_guards(
        &mut self,
        _cx: &Cx,
        _expr: Scev,
        _l: llrm_mir::module::BlockId,
    ) -> Scev {
        todo!("S5")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1158
    /// `forgetLoop(L)`: drops what is cached for the loop and its users.
    pub fn forget_loop(
        &mut self,
        _cx: &Cx,
        _l: llrm_mir::module::BlockId,
    ) {
        todo!("S3")
    }
}
