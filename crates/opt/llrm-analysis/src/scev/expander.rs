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
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, MetadataNode, Operand};
use llrm_mir::passes::Declared;
use llrm_mir::valuetracking::AssumptionCache;
use llrm_support::hash::HashMap;

use super::expr::{Scev, Ty};
use super::{Cx, ScalarEvolution};

/// A [`Cx`] without its function.
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub layout: &'a DataLayout,
    pub loops: &'a LoopInfo,
    pub tree: &'a DomTree,
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
    pub(super) insert_point: Option<InstId>,
    /// `InsertedValues`: what this expander made, in order.
    pub(super) inserted: Vec<InstId>,
    /// `InsertedExpressions`: a SCEV expanded at an insert point.
    pub(super) expressions: HashMap<(Scev, InstId), Operand>,
    /// `RelevantLoops`.
    pub(super) relevant_loops: HashMap<Scev, Option<BlockId>>,
}

impl Expander {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L192
    /// `SCEVExpander(SE, Name)`. `PreserveLCSSA` is always true.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            insert_point: None,
            inserted: Vec::new(),
            expressions: HashMap::default(),
            relevant_loops: HashMap::default(),
        }
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

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L2412
    /// `isSafeToExpand(S, SE, CanonicalMode = true)`: no divisor that may be
    /// zero, and every add recurrence has a preheader to insert into (or is
    /// affine, in canonical mode).
    pub fn is_safe_to_expand(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        s: Scev,
    ) -> bool {
        use super::expr::Kind;
        let mut seen = std::collections::HashSet::new();
        let mut work = vec![s];
        while let Some(s) = work.pop() {
            if !seen.insert(s) {
                continue;
            }
            match se.node(s).kind.clone() {
                Kind::UDiv(l, r) => {
                    if !se.is_known_non_zero(cx, r.scev) {
                        return false;
                    }
                    work.extend([l.scev, r.scev]);
                }
                Kind::AddRec(ops, header) => {
                    let preheader = cx.loops.get_loop_for(header).and_then(|l| l.get_loop_preheader(cx.function));
                    if preheader.is_none() && ops.len() != 2 {
                        return false;
                    }
                    work.extend(ops.iter().map(|op| op.scev));
                }
                Kind::Constant(_) | Kind::Unknown(_) | Kind::CouldNotCompute => {}
                _ => work.extend(se.operands(s).iter().map(|op| op.scev)),
            }
        }
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L2428
    /// `isSafeToExpandAt(S, InsertionPoint, SE)`: safe, and its value is
    /// available at the insertion point.
    pub fn is_safe_to_expand_at(
        &self,
        cx: &Cx,
        se: &mut ScalarEvolution,
        s: Scev,
        insertion_point: InstId,
    ) -> bool {
        if !self.is_safe_to_expand(cx, se, s) {
            return false;
        }
        let Some(block) = cx.function.parent(insertion_point) else { return false };
        if se.properly_dominates(cx, s, block) {
            return true;
        }
        if se.dominates_block(cx, s, block) {
            if cx.function.terminator(block) == Some(insertion_point) {
                return true;
            }
            if let super::expr::Kind::Unknown(llrm_mir::module::Operand::Value(v)) = se.node(s).kind {
                if matches!(
                    cx.function.value(v).def,
                    llrm_mir::module::ValueDef::Instruction(i) if i == insertion_point
                ) {
                    return true;
                }
            }
        }
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L335
    /// `expandCodeFor(SH, Ty, I)`, and `expandCodeFor(SH, Ty)` at the insert
    /// point; `ty` is the type of the result, or none for the SCEV's own.
    pub fn expand_code_for(
        &mut self,
        env: &Env,
        context: &mut Context,
        declared: &mut Declared,
        function: &mut Function,
        se: &mut ScalarEvolution,
        s: Scev,
        ty: Option<Ty>,
    ) -> Operand {
        let mut run = super::expand::Run { x: self, env: *env, context, declared, function, se };
        run.expand_code_for(s, ty)
    }

    /// The name given at `new`, the prefix of the instructions it makes.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L587
/// `SCEVExpanderCleaner`. LLVM runs `cleanup` in the destructor; it needs the
/// function, so here it is called: `cleanup(expander, function)` removes the
/// instructions expanded since `new`, `mark_result_used` keeps them.
pub struct ExpanderCleaner {
    result_used: bool,
}

impl ExpanderCleaner {
    /// `SCEVExpanderCleaner(Expander)`: it holds no borrow of the expander,
    /// which is used between `new` and `cleanup`.
    pub fn new(_expander: &Expander) -> Self {
        Self { result_used: false }
    }

    /// `markResultUsed`.
    pub fn mark_result_used(&mut self) {
        self.result_used = true;
    }

    /// `cleanup`.
    pub fn cleanup(
        self,
        _expander: &mut Expander,
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
        cx: &Cx,
        expr: Scev,
        l: llrm_mir::module::BlockId,
    ) -> Scev {
        let guards = super::guards::LoopGuards::collect(self, cx, l);
        self.apply_loop_guards_with(cx, expr, &guards)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ScalarEvolution.h#L1158
    /// `forgetLoop(L)`: drops what is cached for the loop and its users.
    pub fn forget_loop(
        &mut self,
        _cx: &Cx,
        l: llrm_mir::module::BlockId,
    ) {
        // Drop any stored trip count value (and the loop's properties); the
        // other half, the expressions of the loop's phis, is S3.
        self.trips.forget(l);
    }
}
