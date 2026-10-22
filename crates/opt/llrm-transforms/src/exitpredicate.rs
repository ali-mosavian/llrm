//! A read-only loop's exits are rewritten to tests on the loop's trip count
//! made before it. LLVM's `predicateLoopExits` (`IndVarSimplify.cpp`).
//!
//! # How it works
//! For each loop in LoopSimplify form, innermost first, whose exact backedge
//! count is known and safe to expand, and whose exits all dominate the latch:
//! the leading exits (in dominance order) that end in a conditional branch on a
//! non-constant, leave to a block with no phis, and have a known, expandable
//! count are predicated, when the loop writes nothing another thread could see
//! and has no implicit exit.
//!
//! The exit's test becomes a constant where its count is the loop's, and else
//! `exit count != backedge count` (`==` where the loop is left on true),
//! compared in the wider of the two types. A loop with stores to its own
//! memory is predicated only where every exit leaves to a block that traps.
//!
//! # Example
//! A loop with exits at counts `a` and `b`, backedge count `b`: the exit at `a`
//! tests `a != b` before the loop's first trip, the exit at `b` is `false`.
//!
//! # Used by
//! Not wired in: [`crate::indvars`] will run [`predicated`] after
//! [`crate::exitopt`], as `IndVarSimplify::run` does.
//!
//! # See also
//! `docs/architecture/scev-port.md`.

use llrm_analysis::effects::Declarations;
use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::expr::Ty;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, Operand};
use llrm_mir::opcode::{CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Declared, Outer};

use crate::lftr::branch_of;

/// `LLRM_NO_SCEV_PREDICATE`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_PREDICATE")
}

/// What one run reads and edits.
struct Predicate<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    declared: &'a mut Declared,
    outer: &'a Outer,
    se: &'a mut ScalarEvolution,
    env: Env<'a>,
    rewriter: &'a mut Expander,
}

/// `IndVarSimplify::run`'s `predicateLoopExits` over every loop of
/// `function`, innermost first; whether anything changed. Instructions left
/// dead are `dead`'s.
pub fn predicated(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    predicated_in(&mut session, context, layout, function, declared, outer)
}

/// [`predicated`] over `session`, whose expressions are valid for `function`.
pub fn predicated_in(
    session: &mut ScevSession,
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
) -> bool {
    let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
    let headers: Vec<BlockId> = {
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        crate::loopfilter::retain(&mut headers);
        headers
    };
    // `Rewriter.disableCanonicalMode()`.
    let mut rewriter = Expander::new("indvars");
    rewriter.set_canonical_mode(false);
    let mut run = Predicate { context, layout, function, declared, outer, se, env, rewriter: &mut rewriter };
    let mut changed = false;
    for header in headers {
        let simple = {
            let cx = run.env.cx(&*run.context, &*run.function);
            cx.loops.get_loop_for(header).is_some_and(|found| found.is_loop_simplify_form(cx.function))
        };
        if simple {
            changed |= run.predicate_loop_exits(header);
        }
    }
    changed
}

impl Predicate<'_> {
    fn contains(
        &self,
        header: BlockId,
        block: BlockId,
    ) -> bool {
        let cx = self.env.cx(&*self.context, &*self.function);
        cx.loops.get_loop_for(header).expect("a loop").contains(block)
    }

    /// The branch's two targets.
    fn successors(
        &self,
        exiting: BlockId,
    ) -> (BlockId, BlockId) {
        let (term, _) = branch_of(self.function, exiting).expect("a conditional branch");
        let operands = &self.function.instruction(term).operands;
        match (operands[1], operands[2]) {
            (Operand::Block(first), Operand::Block(second)) => (first, second),
            _ => unreachable!("a conditional branch has block targets"),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1831
    /// `crashingBBWithoutEffect`.
    fn crashing_bb_without_effect(
        &self,
        block: BlockId,
    ) -> bool {
        let declarations: &Declarations = &self.outer.globals;
        self.function
            .iter_block(block)
            .all(
                |inst| {
                    // TODO (LLVM): for now this is overly restrictive, to make
                    // sure nothing in this BB can depend on the loop body. It's
                    // not enough to check for !I.mayHaveSideEffects(), because
                    // e.g. a load does not have a side effect, but we could
                    // have %a = load ptr, ptr %ptr
                    // %b = load i32, ptr %a
                    // Now if the loop stored a non-nullptr to %a, we could
                    // cause a nullptr dereference by skipping over loop
                    // iterations.
                    let opcode = &self.function.instruction(inst).opcode;
                    if matches!(opcode, Opcode::Call(_) | Opcode::Invoke(_))
                        && llrm_analysis::effects::only_accesses_inaccessible_memory(
                            self.context,
                            declarations,
                            self.function,
                            inst,
                        )
                    {
                        return true;
                    }
                    matches!(opcode, Opcode::Unreachable)
                },
            )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1849
    /// `predicateLoopExits` for the loop headed by `header`.
    fn predicate_loop_exits(
        &mut self,
        header: BlockId,
    ) -> bool {
        // Note: ExactBTC is the exact backedge taken count *iff* the loop
        // exits through *explicit* control flow. We have to eliminate the
        // possibility of implicit exits (see below) before we know it's truly
        // exact.
        let (exact_btc, safe) = {
            let cx = self.env.cx(&*self.context, &*self.function);
            let exact_btc = self.se.get_backedge_taken_count(&cx, header, ExitCountKind::Exact);
            let safe = exact_btc != self.se.get_could_not_compute()
                && self.rewriter.is_safe_to_expand(&cx, self.se, exact_btc);
            (exact_btc, safe)
        };
        if !safe {
            return false;
        }

        let (latch, preheader, mut exiting_blocks) = {
            let cx = self.env.cx(&*self.context, &*self.function);
            let l = cx.loops.get_loop_for(header).expect("a loop");
            (
                l.get_loop_latch(cx.function).expect("a latch"),
                l.get_loop_preheader(cx.function).expect("a preheader"),
                l.get_exiting_blocks(cx.function),
            )
        };

        // Make sure all exits dominate the latch. This means there is a linear
        // chain of exits. We check this before sorting so we have a total
        // order.
        {
            let cx = self.env.cx(&*self.context, &*self.function);
            if exiting_blocks.iter().any(|&exiting| !cx.tree.dominates(exiting, latch)) {
                return false;
            }
            // If we have any exits which can't be predicated themselves, than
            // we can't predicate any exit which isn't guaranteed to execute
            // before it. Consider two exits (a) and (b) which would both exit
            // on the same iteration. If we can predicate (b), but not (a), and
            // (a) preceeds (b) along some path, then we could convert a loop
            // from exiting through (a) to one exiting through (b). Note that
            // this problem exists only for exits with the same exit count, and
            // we could be more aggressive when exit counts are known inequal.
            exiting_blocks.sort_by(|&a, &b| {
                if a == b {
                    std::cmp::Ordering::Equal
                } else if cx.tree.properly_dominates(a, b) {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            });
        }

        // Given our sorted total order, we know that exit[j] must be evaluated
        // after all exit[i] such j > i.
        if let Some(bad) = (0..exiting_blocks.len()).find(|&i| self.bad_exit(header, exiting_blocks[i])) {
            exiting_blocks.truncate(bad);
        }

        if exiting_blocks.is_empty() {
            return false;
        }

        // At this point, ExitingBlocks consists of only those blocks which are
        // predicatable. Given that, we know we have at least one exit we can
        // predicate if the loop is doesn't have side effects and doesn't have
        // any implicit exits (because then our exact BTC isn't actually
        // exact).
        let declarations: &Declarations = &self.outer.globals;
        let mut has_thread_local_side_effects = false;
        let blocks: Vec<BlockId> = {
            let cx = self.env.cx(&*self.context, &*self.function);
            cx.loops.get_loop_for(header).expect("a loop").get_blocks().to_vec()
        };
        for block in blocks {
            for inst in self.function.iter_block(block) {
                // TODO (LLVM): isGuaranteedToTransfer
                if crate::local::may_have_side_effects(
                    self.context,
                    self.outer.callees(),
                    declarations,
                    self.function,
                    inst,
                ) {
                    has_thread_local_side_effects = true;
                    // Simple stores cannot be observed by other threads. If
                    // HasThreadLocalSideEffects is set, we check
                    // crashingBBWithoutEffect to make sure that the crashing
                    // BB cannot observe them either.
                    if !matches!(
                        self.function.instruction(inst).opcode,
                        Opcode::Store { volatile: false, .. }
                    ) {
                        return false;
                    }
                }
                // The token check of LLVM has no counterpart: llrm has no
                // token type.
            }
        }

        let mut changed = false;
        // Finally, do the actual predication for all predicatable blocks. A
        // couple of notes here:
        // 1) We don't bother to constant fold dominated exits with identical
        //    exit counts; that's simply a form of CSE/equality propagation and
        //    we leave it for dedicated passes.
        // 2) We insert the comparison at the branch. Hoisting introduces
        //    additional legality constraints and we leave that to dedicated
        //    logic. We want to predicate even if we can't insert a loop
        //    invariant expression as peeling or unrolling will likely reduce
        //    the cost of the otherwise loop varying check.
        let pre_term = self.function.terminator(preheader).expect("a terminator");
        self.rewriter.set_insert_point(pre_term);
        // Lazily generated if needed.
        let mut exact_btcv: Option<Operand> = None;
        for exiting in exiting_blocks {
            let exit_count = {
                let cx = self.env.cx(&*self.context, &*self.function);
                self.se.get_exit_count(&cx, header, exiting, ExitCountKind::Exact)
            };
            let (term, old_cond) = branch_of(self.function, exiting).expect("a conditional branch");
            let (first, second) = self.successors(exiting);
            if has_thread_local_side_effects {
                // Exit BB which have one branch back into the loop and another
                // one to a trap can still be optimized, because local side
                // effects cannot be observed in the exit case (the trap). We
                // could be smarter about this, but for now lets pattern match
                // common cases that directly trap.
                let unreachable = [first, second]
                    .into_iter()
                    .rev()
                    .find(
                        |&succ| self.function
                            .terminator(succ)
                            .is_some_and(|t| self.function.instruction(t).opcode == Opcode::Unreachable),
                    );
                if unreachable.is_none_or(|block| !self.crashing_bb_without_effect(block)) {
                    return changed;
                }
            }
            let new_cond = if exit_count == exact_btc {
                let stays = self.contains(header, first);
                let i1 = self.context.types.int(1);
                Operand::Constant(self.context.int(i1, i128::from(!stays)))
            } else {
                let mut ecv = self
                    .rewriter
                    .expand_code_for(&self.env, self.context, self.declared, self.function, self.se, exit_count, None);
                if exact_btcv.is_none() {
                    exact_btcv = Some(self.rewriter.expand_code_for(
                        &self.env,
                        self.context,
                        self.declared,
                        self.function,
                        self.se,
                        exact_btc,
                        None,
                    ));
                }
                let mut rhs = exact_btcv.expect("expanded");
                let (ecv_ty, rhs_ty) = (self.se.type_of(exit_count), self.se.type_of(exact_btc));
                if ecv_ty != rhs_ty {
                    let wider = {
                        let cx = self.env.cx(&*self.context, &*self.function);
                        self.se.get_wider_type(&cx, ecv_ty, rhs_ty)
                    };
                    let Ty::Int(bits) = wider else { unreachable!("an integer count") };
                    let wide = self.context.types.int(bits);
                    ecv = self.zext(ecv, wide, pre_term);
                    rhs = self.zext(rhs, wide, pre_term);
                }
                let pred = if self.contains(header, first) { IntPredicate::Ne } else { IntPredicate::Eq };
                let i1 = self.context.types.int(1);
                let cmp =
                    self.function.create_instruction(Opcode::ICmp(pred), i1, vec![ecv, rhs], Flags::default(), None);
                self.function.insert(cmp, Position::Before(pre_term)).expect("a placed branch");
                Operand::Value(self.function.instruction(cmp).result.expect("a compare value"))
            };
            let _ = old_cond;
            self.function.set_operand(term, 0, new_cond);
            changed = true;
        }
        changed
    }

    /// `B.CreateZExt` before `before`; a constant is folded.
    fn zext(
        &mut self,
        v: Operand,
        to: llrm_mir::types::TypeId,
        before: llrm_mir::module::InstId,
    ) -> Operand {
        if matches!(v, Operand::Constant(_))
            && let Some(folded) =
                llrm_mir::simplify::simplify_cast_operand(self.context, self.layout, self.function, CastOp::ZExt, v, to)
        {
            return folded;
        }
        let inst = self.function.create_instruction(Opcode::Cast(CastOp::ZExt), to, vec![v], Flags::default(), None);
        self.function.insert(inst, Position::Before(before)).expect("a placed branch");
        Operand::Value(self.function.instruction(inst).result.expect("a cast value"))
    }

    /// `BadExit`: an exit that can't be predicated.
    fn bad_exit(
        &mut self,
        header: BlockId,
        exiting: BlockId,
    ) -> bool {
        // If our exiting block exits multiple loops, we can only rewrite the
        // innermost one. Otherwise, we're changing how many times the
        // innermost loop runs before it exits.
        let innermost = {
            let cx = self.env.cx(&*self.context, &*self.function);
            cx.loops.get_loop_for(exiting) == cx.loops.get_loop_for(header)
        };
        if !innermost {
            return true;
        }
        // Can't rewrite non-branch yet.
        let Some((_, cond)) = branch_of(self.function, exiting) else { return true };
        // If already constant, nothing to do.
        if matches!(cond, Operand::Constant(_)) {
            return true;
        }
        // If the exit block has phis, we need to be able to compute the values
        // within the loop which contains them. This assumes trivially lcssa
        // phis have already been removed; TODO (LLVM): generalize
        let (first, second) = self.successors(exiting);
        let exit_block = if self.contains(header, first) { second } else { first };
        if self
            .function
            .iter_block(exit_block)
            .next()
            .is_some_and(|inst| self.function.instruction(inst).opcode == Opcode::Phi)
        {
            return true;
        }
        let cx = self.env.cx(&*self.context, &*self.function);
        let exit_count = self.se.get_exit_count(&cx, header, exiting, ExitCountKind::Exact);
        exit_count == self.se.get_could_not_compute() || !self.rewriter.is_safe_to_expand(&cx, self.se, exit_count)
    }
}

#[cfg(test)]
#[path = "exitpredicate_tests.rs"]
mod tests;
