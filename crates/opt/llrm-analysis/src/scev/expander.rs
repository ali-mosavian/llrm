//! `SCEVExpander`'s surface, as signatures (S6 translates the bodies): what
//! LoopIdiom and LSR call.
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
    /// `inserted` as a set, for `isInsertedInstruction`.
    pub(super) inserted_set: llrm_mir::dense::IdSet<InstId>,
    /// `OrigFlags`: the flags an instruction had before the expander changed
    /// them (`rememberFlags`), put back by a cleanup.
    pub(super) orig_flags: llrm_mir::dense::IdMap<InstId, llrm_mir::opcode::Flags>,
    /// `InsertedExpressions`: a SCEV expanded at an insert point.
    pub(super) expressions: HashMap<(Scev, InstId), Operand>,
    /// `RelevantLoops`.
    pub(super) relevant_loops: HashMap<Scev, Option<BlockId>>,
    /// `CanonicalMode`: add recurrences are written over one `{0,+,1}` phi.
    pub(super) canonical_mode: bool,
    /// `LSRMode`.
    pub(super) lsr_mode: bool,
    /// `PostIncLoops`, as the headers.
    pub(super) post_inc_loops: Vec<BlockId>,
    /// `IVIncInsertLoop` (its header) and `IVIncInsertPos`.
    pub(super) iv_inc_insert: Option<(BlockId, InstId)>,
    /// `InsertedIVs`.
    pub(super) inserted_ivs: Vec<InstId>,
    /// `ReusedValues`: found, not made.
    pub(super) reused: llrm_mir::dense::IdSet<InstId>,
}

impl Expander {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Transforms/Utils/ScalarEvolutionExpander.h#L192
    /// `SCEVExpander(SE, Name)`. `PreserveLCSSA` is always true.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            insert_point: None,
            inserted: Vec::new(),
            inserted_set: Default::default(),
            orig_flags: Default::default(),
            expressions: HashMap::default(),
            relevant_loops: HashMap::default(),
            canonical_mode: true,
            lsr_mode: false,
            post_inc_loops: Vec::new(),
            iv_inc_insert: None,
            inserted_ivs: Vec::new(),
            reused: Default::default(),
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

    /// `setCanonicalMode(false)` for LSR: each add recurrence gets its own phi.
    pub fn set_canonical_mode(
        &mut self,
        on: bool,
    ) {
        self.canonical_mode = on;
    }

    /// `enableLSRMode`.
    pub fn enable_lsr_mode(&mut self) {
        self.lsr_mode = true;
    }

    /// `setIVIncInsertPos(L, Pos)`: the increments of `l`'s phis go before
    /// `pos`.
    pub fn set_iv_inc_insert_pos(
        &mut self,
        l: BlockId,
        pos: InstId,
    ) {
        self.iv_inc_insert = Some((l, pos));
    }

    /// `setPostInc(L)`.
    pub fn set_post_inc(
        &mut self,
        l: BlockId,
    ) {
        if !self.post_inc_loops.contains(&l) {
            self.post_inc_loops.push(l);
        }
    }

    /// `clearPostInc`.
    pub fn clear_post_inc(&mut self) {
        self.post_inc_loops.clear();
    }

    /// `rememberInstruction`.
    pub(super) fn remember(
        &mut self,
        inst: InstId,
    ) {
        if self.inserted_set.insert(inst) {
            self.inserted.push(inst);
        }
    }

    /// `isInsertedInstruction`: the expander made `inst` (or reused it as its
    /// own).
    pub fn is_inserted_instruction(
        &self,
        inst: &InstId,
    ) -> bool {
        self.inserted_set.contains(inst)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L848
    /// `hoistIVInc(IncV, InsertPos, RecomputePoisonFlags)`: move a simple IV
    /// increment (and the chain back to its phi) above `insert_pos`, where it
    /// can be reached from the uses there; false if it cannot be hoisted.
    pub fn hoist_iv_inc(
        &mut self,
        env: &Env,
        context: &mut Context,
        function: &mut Function,
        se: &mut ScalarEvolution,
        inc: InstId,
        insert_pos: InstId,
        recompute_poison_flags: bool,
    ) -> bool {
        let mut declared = Declared::default();
        let mut run = super::expand::Run { x: self, env: *env, context, declared: &mut declared, function, se };
        run.hoist_iv_inc(inc, insert_pos, recompute_poison_flags)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1909
    /// `replaceCongruentIVs(L, DT, DeadInsts, TTI)` for the loop headed by
    /// `header`: the number of phis eliminated; the dead ones are in `dead`.
    pub fn replace_congruent_ivs(
        &mut self,
        env: &Env,
        context: &mut Context,
        function: &mut Function,
        se: &mut ScalarEvolution,
        header: BlockId,
        dead: &mut Vec<InstId>,
        tti: Option<&dyn llrm_mir::tti::Tti>,
    ) -> u32 {
        let mut declared = Declared::default();
        let mut run = super::expand::Run { x: self, env: *env, context, declared: &mut declared, function, se };
        run.replace_congruent_ivs(header, dead, tti)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L901
    /// `canReuseFlagsFromOriginalIVInc(OrigPhi, WidePhi, OrigInc, WideInc)`:
    /// both increments are the same binary operation of their phi.
    pub fn can_reuse_flags_from_original_iv_inc(
        function: &Function,
        orig_phi: InstId,
        wide_phi: InstId,
        orig_inc: InstId,
        wide_inc: InstId,
    ) -> bool {
        let reads = |inc: InstId, phi: InstId| {
            let one = function.instruction(inc);
            let phi_value = function.instruction(phi).result.map(Operand::Value);
            matches!(one.opcode, llrm_mir::opcode::Opcode::Binary(_))
                && one.operands.len() == 2
                && phi_value.is_some_and(|v| one.operands.contains(&v))
        };
        reads(orig_inc, orig_phi)
            && reads(wide_inc, wide_phi)
            && function.instruction(orig_inc).opcode == function.instruction(wide_inc).opcode
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L174
    /// `eraseDeadInstructions(Root)`: every instruction the expander made that
    /// is trivially dead, but `root`, is erased, with the operands that die
    /// with it. `is_dead` is `isInstructionTriviallyDead`, which lives with
    /// the transforms.
    pub fn erase_dead_instructions(
        &mut self,
        function: &mut Function,
        root: Option<Operand>,
        is_dead: &dyn Fn(&Function, InstId) -> bool,
    ) {
        let def = |function: &Function, operand: Operand| match operand {
            Operand::Value(value) => match function.value(value).def {
                llrm_mir::module::ValueDef::Instruction(inst) => Some(inst),
                _ => None,
            },
            _ => None,
        };
        let root = root.and_then(|root| def(function, root));
        let mut work = self.inserted_instructions();
        let mut deleted = llrm_mir::dense::IdSet::<InstId>::new();
        while let Some(inst) = work.pop() {
            if deleted.contains(&inst)
                || Some(inst) == root
                || !self.inserted_set.contains(&inst)
                || function.is_erased(inst)
                || !is_dead(function, inst)
            {
                continue;
            }
            let operands = function.instruction(inst).operands.clone();
            work.extend(operands.iter().filter_map(|&operand| def(function, operand)));
            deleted.insert(inst);
            self.inserted_set.remove(&inst);
            function.set_operands(inst, Vec::new());
            function.erase(inst).expect("a dead instruction");
        }
        self.inserted.retain(|inst| !deleted.contains(inst));
        self.expressions.retain(|_, value| {
            !matches!(
                value,
                Operand::Value(v) if match function.value(*v).def { llrm_mir::module::ValueDef::Instruction(inst) => deleted.contains(&inst), _ => false, }
            )
        });
    }

    /// `getAllInsertedInstructions`: what the expander made, in order, less
    /// what it reused.
    pub fn inserted_instructions(&self) -> Vec<InstId> {
        self.inserted.iter().copied().filter(|inst| !self.reused.contains(inst)).collect()
    }

    /// `getInsertedIVs`.
    pub fn inserted_ivs(&self) -> &[InstId] {
        &self.inserted_ivs
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
        let mut seen = llrm_support::hash::HashSet::default();
        let mut work = vec![s];
        while let Some(s) = work.pop() {
            if !seen.insert(s) {
                continue;
            }
            match se.node(s).kind.clone() {
                Kind::UDiv(l, r) => {
                    if !se.is_known_non_zero(cx, r.scev) || !se.is_guaranteed_not_to_be_poison_scev(cx, r.scev) {
                        return false;
                    }
                    work.extend([l.scev, r.scev]);
                }
                Kind::AddRec(ops, header) => {
                    let preheader = cx.loops.get_loop_for(header).and_then(|l| l.get_loop_preheader(cx.function));
                    if preheader.is_none() && (!self.canonical_mode || ops.len() != 2) {
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

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L2610
    /// `cleanup`: unless the result is used, every instruction the expander
    /// inserted (not those it reused) is erased. They are used only by each
    /// other, which `erase_all` allows.
    pub fn cleanup(
        self,
        expander: &mut Expander,
        function: &mut Function,
    ) {
        if self.result_used {
            return;
        }
        // Restore the original poison flags.
        let originals: Vec<_> = expander.orig_flags.iter().map(|(inst, flags)| (inst, *flags)).collect();
        expander.orig_flags = Default::default();
        for (inst, flags) in originals {
            if !function.is_erased(inst) {
                function.set_flags(inst, flags);
            }
        }
        expander.inserted_set = Default::default();
        let reused = std::mem::take(&mut expander.reused);
        let mut inserted = std::mem::take(&mut expander.inserted);
        expander.expressions.clear();
        expander.relevant_loops.clear();
        expander.inserted_ivs.clear();
        let mut seen = llrm_mir::dense::IdSet::<InstId>::new();
        inserted.retain(|&inst| !reused.contains(&inst) && seen.insert(inst));
        inserted.reverse();
        function.erase_all(&inserted).expect("what the expander inserted is used only by itself");
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
