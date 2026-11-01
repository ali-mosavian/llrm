//! A loop's exit test is rewritten to compare a counter with the value it has
//! when the loop's trip count is spent. LLVM's `linearFunctionTestReplace` and
//! its kin (`IndVarSimplify.cpp`).
//!
//! # How it works
//! For each loop, innermost first, and each exiting block that ends in a
//! conditional branch and belongs to the loop itself:
//!
//! - `needsLFTR`: the test is not already `==`/`!=` of a simple counter with a
//!   loop-invariant value.
//! - The block's exact exit count is known, is not zero, is cheap to expand
//!   (`isHighCostExpansion`, budget 4) and is safe to expand.
//! - `FindLoopCounter` picks a header phi that is a counter (a unit stride, or
//!   a constant one that cannot wrap: DEVIATE LFTR1) at least as wide as the
//!   count, with a concrete start, preferring one that counts from zero or is
//!   wider, and not one that only this test keeps alive.
//! - `genLoopLimit` writes the counter's value after the count (a post-inc
//!   value where the test is in the latch) before the branch, and the branch
//!   tests `counter != limit` (`==` where the loop is left on true). A counter
//!   wider than the count is compared by a truncation, or the limit is extended
//!   (`wide.trip.count`) where that equals it.
//!
//! The test it replaces is left to `dead`.
//!
//! DEVIATE LFTR1 (owner llrm-w12-lsr; exit: the pin's `isLoopCounter`
//! (IndVarSimplify.cpp#L913, `m_scev_One`) takes any constant step): a loop
//! that steps by two, `for (o = 0; o <= 3998; o += 2)`, kept `ult` for the
//! pin's LSR, which turns only equality tests into zero-ending counters; main's
//! earlier LSR made it `ne`. A step that cannot wrap reaches `start + count *
//! step` exactly, so `!=` holds on the trip the original test ended (gcc's
//! `iv_elimination_compare` relies on the same). Fail-first tests: strides
//! 2, 3, 4, 8 and -2 in `lftr_tests.rs`.
//!
//! # Example
//! `for (i = 0; i < n; i++)` with `n` an `i32`: the test `i slt n` becomes
//! `%exitcond = icmp ne i32 %i.next, %n` in the latch.
//!
//! # Used by
//! [`crate::indvars`] runs [`replaced`] after the exit steps, as
//! `IndVarSimplify::run` does.
//!
//! # See also
//! `linearFunctionTestReplace`, `FindLoopCounter`, `genLoopLimit`,
//! `needsLFTR`, `docs/architecture/scev-port.md`.

use llrm_analysis::occurrence::arms;
use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::backedge::ExitCountKind;
use llrm_analysis::scev::expand_cost::{ExpansionTti, MachineCosts};
use llrm_analysis::scev::expander::{Env, Expander};
use llrm_analysis::scev::expr::{Kind, Scev, Ty, Use};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::types::{Type as MirType, TypeId};
use llrm_support::hash::HashSet;

/// `SCEVCheapExpansionBudget`: its `cl::opt` default.
const CHEAP_EXPANSION_BUDGET: u32 = 4;

/// What one run reads and edits.
struct Lftr<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    declared: &'a mut Declared,
    outer: &'a Outer,
    se: &'a mut ScalarEvolution,
    env: Env<'a>,
    rewriter: &'a mut Expander,
    tti: &'a dyn ExpansionTti,
}

/// `LLRM_NO_SCEV_LFTR`, read once; set, the step is skipped.
pub fn enabled() -> bool {
    !llrm_support::env_flag!("LLRM_NO_SCEV_LFTR")
}

/// `IndVarSimplify::run`'s LFTR step over every loop of `function`, innermost
/// first; whether anything changed. Instructions left dead are `dead`'s.
pub fn replaced(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    declared: &mut Declared,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::within(context, &outer.globals, &outer.metadata, function);
    replaced_in(&mut session, context, layout, function, declared, outer)
}

/// [`replaced`] over `session`, whose expressions are valid for `function`.
pub fn replaced_in(
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
    let minimum_size = function.attrs.iter().any(|attr| matches!(attr, Attribute::Flag(flag) if flag == "minsize"));
    let tti = MachineCosts { tti: outer.target().tti(), minimum_size };
    // `Rewriter.disableCanonicalMode()`.
    let mut rewriter = Expander::new("indvars");
    rewriter.set_canonical_mode(false);
    let mut run = Lftr { context, layout, function, declared, outer, se, env, rewriter: &mut rewriter, tti: &tti };
    let mut changed = false;
    for header in headers {
        // `IndVarSimplify::run`: stay out of trouble without LoopSimplify
        // form.
        let simple = {
            let cx = run.env.cx(&*run.context, &*run.function);
            cx.loops.get_loop_for(header).is_some_and(|found| found.is_loop_simplify_form(cx.function))
        };
        if simple {
            changed |= run.loop_replaced(header);
        }
    }
    changed
}

/// The condition of `block`'s conditional branch.
pub(crate) fn branch_of(
    function: &Function,
    block: BlockId,
) -> Option<(InstId, Operand)> {
    let term = function.terminator(block)?;
    let instruction = function.instruction(term);
    (instruction.opcode == Opcode::Br && instruction.operands.len() == 3).then(|| (term, instruction.operands[0]))
}

/// The instruction that defines `value`.
pub(crate) fn def_of(
    function: &Function,
    value: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = value else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L770
/// `getLoopPhiForCounter`: the header phi `inc` is the increment of, if it is
/// an add, subtract or one-index GEP of it by a loop invariant.
fn get_loop_phi_for_counter(
    function: &Function,
    l: &Loop,
    inc: Operand,
) -> Option<InstId> {
    let inc_inst = def_of(function, inc)?;
    let instruction = function.instruction(inc_inst);
    let is_gep = matches!(instruction.opcode, Opcode::GetElementPtr { .. });
    match instruction.opcode {
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => {}
        // An IV counter must preserve its type.
        Opcode::GetElementPtr { .. } if instruction.operands.len() == 2 => {}
        _ => return None,
    }
    let header_phi = |operand: Operand| {
        def_of(function, operand).filter(|&phi| {
            function.instruction(phi).opcode == Opcode::Phi && function.parent(phi) == Some(l.get_header())
        })
    };
    if let Some(phi) = header_phi(instruction.operands[0]) {
        return l.is_loop_invariant(function, instruction.operands[1]).then_some(phi);
    }
    if is_gep {
        return None;
    }
    // Allow add/sub to be commuted.
    let phi = header_phi(instruction.operands[1])?;
    l.is_loop_invariant(function, instruction.operands[0]).then_some(phi)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L808
/// `isLoopExitTestBasedOn`: `v` is a direct operand of the exit compare.
fn is_loop_exit_test_based_on(
    function: &Function,
    v: Operand,
    exiting: BlockId,
) -> bool {
    let (_, cond) = branch_of(function, exiting).expect("a conditional branch");
    // TODO in LLVM: allow a non-icmp loop test.
    let Some(icmp) = def_of(function, cond).filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
    else {
        return false;
    };
    let operands = &function.instruction(icmp).operands;
    operands[0] == v || operands[1] == v
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L821
/// `needsLFTR`: unless the current exit test is already sufficiently
/// canonical.
fn needs_lftr(
    function: &Function,
    l: &Loop,
    exiting: BlockId,
) -> bool {
    let latch = l.get_loop_latch(function).expect("Must be in simplified form");
    // Avoid converting a constant or loop invariant test back to a runtime
    // test. This is critical for when SCEV's cached ExitCount is less precise
    // than the current IR.
    let (_, cond) = branch_of(function, exiting).expect("a conditional branch");
    if l.is_loop_invariant(function, cond) {
        return false;
    }
    // Do LFTR to simplify the exit condition to an ICMP.
    let Some(cmp) = def_of(function, cond).filter(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
    else {
        return true;
    };
    // Do LFTR to simplify the exit ICMP to EQ/NE.
    let Opcode::ICmp(pred) = function.instruction(cmp).opcode else { unreachable!("an icmp") };
    if pred != IntPredicate::Ne && pred != IntPredicate::Eq {
        return true;
    }
    // Look for a loop invariant RHS.
    let (mut lhs, mut rhs) = (function.instruction(cmp).operands[0], function.instruction(cmp).operands[1]);
    if !l.is_loop_invariant(function, rhs) {
        if !l.is_loop_invariant(function, lhs) {
            return true;
        }
        std::mem::swap(&mut lhs, &mut rhs);
    }
    // Look for a simple IV counter LHS.
    let phi = def_of(function, lhs)
        .filter(|&one| function.instruction(one).opcode == Opcode::Phi)
        .or_else(|| get_loop_phi_for_counter(function, l, lhs));
    let Some(phi) = phi else { return true };
    // Do LFTR if PHI node is defined in the loop, but is *not* a counter.
    let Some(&(inc, _)) = arms(function, phi).iter().find(|(_, from)| *from == latch) else { return true };
    // Do LFTR if the exit condition's IV is *not* a simple counter.
    Some(phi) != get_loop_phi_for_counter(function, l, inc)
}

impl Lftr<'_> {
    /// The loop's exiting blocks, in the order `getExitingBlocks` gives.
    fn exiting_blocks(
        &self,
        header: BlockId,
    ) -> Vec<BlockId> {
        let cx = self.env.cx(&*self.context, &*self.function);
        cx.loops.get_loop_for(header).expect("a loop").get_exiting_blocks(cx.function)
    }

    fn is_pointer(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(self.context.types.get(ty), MirType::Pointer(_))
    }

    fn type_of_value(
        &self,
        v: Operand,
    ) -> TypeId {
        match v {
            Operand::Value(value) => self.function.value(value).ty,
            Operand::Constant(constant) => self.context.get(constant).ty,
            _ => unreachable!("a value"),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L2123
    /// The LFTR part of `IndVarSimplify::run` for the loop headed by `header`.
    fn loop_replaced(
        &mut self,
        header: BlockId,
    ) -> bool {
        let preheader = {
            let cx = self.env.cx(&*self.context, &*self.function);
            cx.loops
                .get_loop_for(header)
                .and_then(|l| l.get_loop_preheader(cx.function))
                .expect("a loop with a preheader")
        };
        let preheader_term = self.function.terminator(preheader).expect("a terminator");
        let mut changed = false;
        for exiting in self.exiting_blocks(header) {
            // Can't rewrite non-branch yet.
            if branch_of(self.function, exiting).is_none() {
                continue;
            }
            // If our exitting block exits multiple loops, we can only rewrite
            // the innermost one. Otherwise, we're changing how many times the
            // innermost loop runs before it exits.
            let (innermost, simple_exit) = {
                let cx = self.env.cx(&*self.context, &*self.function);
                let l = cx.loops.get_loop_for(header).expect("a loop");
                (cx.loops.get_loop_for(exiting).is_some_and(|one| one == l), needs_lftr(cx.function, &l, exiting))
            };
            if !innermost || !simple_exit {
                continue;
            }
            let exit_count = {
                let cx = self.env.cx(&*self.context, &*self.function);
                self.se.get_exit_count(&cx, header, exiting, ExitCountKind::Exact)
            };
            if exit_count == self.se.get_could_not_compute() {
                continue;
            }
            // This was handled above, but as we form SCEVs, we can sometimes
            // refine existing ones; this allows exit counts to be folded to
            // zero which weren't when optimizeLoopExits saw them.
            if self.se.as_constant(exit_count).is_some_and(|c| c.is_zero()) {
                continue;
            }
            let Some(ind_var) = self.find_loop_counter(header, exiting, exit_count) else { continue };
            // Avoid high cost expansions. Note: This heuristic is questionable
            // in that our definition of "high cost" is not exactly principled.
            let (high_cost, safe) = {
                let cx = self.env.cx(&*self.context, &*self.function);
                (
                    self.rewriter.is_high_cost_expansion(
                        &cx,
                        self.se,
                        &[exit_count],
                        header,
                        CHEAP_EXPANSION_BUDGET,
                        self.tti,
                        preheader_term,
                    ),
                    self.rewriter.is_safe_to_expand(&cx, self.se, exit_count),
                )
            };
            if high_cost || !safe {
                continue;
            }
            changed |= self.linear_function_test_replace(header, exiting, exit_count, ind_var);
        }
        changed
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L913
    /// `isLoopCounter`: `phi` is an add recurrence of the loop with step one
    /// whose latch increment is an add recurrence too.
    fn is_loop_counter(
        &mut self,
        header: BlockId,
        phi: InstId,
    ) -> bool {
        let cx = self.env.cx(&*self.context, &*self.function);
        let l = cx.loops.get_loop_for(header).expect("a loop");
        let latch = l.get_loop_latch(cx.function).expect("a latch");
        let result = Operand::Value(cx.function.instruction(phi).result.expect("a phi value"));
        if !self.se.is_value_scevable(&cx, result) {
            return false;
        }
        let s = self.se.get_scev(&cx, result);
        let is_counter = match self.se.node(s).kind.clone() {
            Kind::AddRec(ops, one) if ops.len() == 2 && one == header => {
                self.se.as_constant(ops[1].scev).is_some_and(|step| !step.is_zero())
            }
            _ => false,
        };
        if !is_counter {
            return false;
        }
        // DEVIATE LFTR1: the pin wants a step of one. A constant step that
        // cannot wrap reaches the limit `start + count * step` exactly,
        // as gcc's `iv_elimination_compare` relies on.
        let unit = matches!(
            self.se.node(s).kind.clone(),
            Kind::AddRec(ops, _) if self.se.as_constant(ops[1].scev).is_some_and(|step| step.is_one())
        );
        let wrap = llrm_analysis::scev::expr::flags::NUW | llrm_analysis::scev::expr::flags::NSW;
        if !unit && self.se.get_no_wrap_flags(s, wrap) == 0 {
            return false;
        }
        let Some(&(inc, _)) = arms(cx.function, phi).iter().find(|(_, from)| *from == latch) else { return false };
        if get_loop_phi_for_counter(cx.function, &l, inc) != Some(phi) {
            return false;
        }
        let inc_scev = self.se.get_scev(&cx, inc);
        matches!(self.se.node(inc_scev).kind, Kind::AddRec(..))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L871
    /// `hasConcreteDefImpl`: every operand reached within six levels is a
    /// constant that is not `undef`, through instructions that read no memory
    /// and are not calls.
    fn has_concrete_def_impl(
        &self,
        v: Operand,
        visited: &mut HashSet<Operand>,
        depth: u32,
    ) -> bool {
        if let Operand::Constant(constant) = v {
            return !matches!(
                self.context.get(constant).kind,
                ConstantKind::Undef | ConstantKind::Poison
            );
        }
        if depth >= 6 {
            return false;
        }
        // Conservatively handle non-constant non-instructions. For example,
        // Arguments may be undef.
        let Some(inst) = def_of(self.function, v) else { return false };
        // Load and return values may be undef.
        let reads = llrm_mir::memory::of(self.context, self.outer.callees(), self.function, inst).reads;
        if reads || matches!(
            self.function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        ) {
            return false;
        }
        // Optimistically handle other instructions.
        for &op in &self.function.instruction(inst).operands {
            if matches!(op, Operand::Block(_)) || !visited.insert(op) {
                continue;
            }
            if !self.has_concrete_def_impl(op, visited, depth + 1) {
                return false;
            }
        }
        true
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L904
    /// `hasConcreteDef`: undef can never reach `v`.
    fn has_concrete_def(
        &self,
        v: Operand,
    ) -> bool {
        let mut visited = HashSet::default();
        visited.insert(v);
        self.has_concrete_def_impl(v, &mut visited, 0)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopUtils.cpp#L507
    /// `isAlmostDeadIV`: the phi is read only by its increment and `cond`, and
    /// the increment only by the phi and `cond`.
    fn is_almost_dead_iv(
        &self,
        phi: InstId,
        latch: BlockId,
        cond: Operand,
    ) -> bool {
        let cond = def_of(self.function, cond);
        let inc = arms(self.function, phi).into_iter().find(|(_, from)| *from == latch).expect("a latch case").0;
        let phi_value = self.function.instruction(phi).result.expect("a phi value");
        let inc_inst = def_of(self.function, inc);
        let Operand::Value(inc_value) = inc else { return false };
        if self.function.users(phi_value).iter().any(|u| Some(u.user) != cond && Some(u.user) != inc_inst) {
            return false;
        }
        !self.function.users(inc_value).iter().any(|u| Some(u.user) != cond && u.user != phi)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L938
    /// `FindLoopCounter`: the header phi best suited to count the loop's
    /// trips.
    fn find_loop_counter(
        &mut self,
        header: BlockId,
        exiting: BlockId,
        be_count: Scev,
    ) -> Option<InstId> {
        let bc_width = {
            let cx = self.env.cx(&*self.context, &*self.function);
            self.se.get_type_size_in_bits(&cx, self.se.type_of(be_count))
        };
        let (_, cond) = branch_of(self.function, exiting).expect("a conditional branch");
        // Loop over all of the PHI nodes, looking for a simple counter.
        let mut best: Option<(InstId, Scev)> = None;
        let latch = {
            let cx = self.env.cx(&*self.context, &*self.function);
            cx.loops
                .get_loop_for(header)
                .expect("a loop")
                .get_loop_latch(cx.function)
                .expect("Must be in simplified form")
        };
        let phis: Vec<InstId> = self
            .function
            .iter_block(header)
            .take_while(|&inst| self.function.instruction(inst).opcode == Opcode::Phi)
            .collect();
        for phi in phis {
            if !self.is_loop_counter(header, phi) {
                continue;
            }
            let phi_value = Operand::Value(self.function.instruction(phi).result.expect("a phi value"));
            let (ar, phi_width, init) = {
                let cx = self.env.cx(&*self.context, &*self.function);
                let ar = self.se.get_scev(&cx, phi_value);
                let Kind::AddRec(ops, _) = self.se.node(ar).kind.clone() else { unreachable!("a counter") };
                (ar, self.se.get_type_size_in_bits(&cx, self.se.type_of(ar)), ops[0].scev)
            };
            // AR may be a pointer type, while BECount is an integer type. AR
            // may be wider than BECount. With eq/ne tests overflow is
            // immaterial. AR may not be a narrower type, or we may never exit.
            let _ = ar;
            if phi_width < bc_width || !self.layout.legal_integer(phi_width) {
                continue;
            }
            // Avoid reusing a potentially undef value to compute other values
            // that may have originally had a concrete definition.
            if !self.has_concrete_def(phi_value) {
                // We explicitly allow unknown phis as long as they are already
                // used by the loop exit test. This is legal since performing
                // LFTR could not increase the number of undef users.
                let inc_phi = arms(self.function, phi).into_iter().find(|(_, from)| *from == latch).expect("a latch").0;
                if !is_loop_exit_test_based_on(self.function, phi_value, exiting)
                    && !is_loop_exit_test_based_on(self.function, inc_phi, exiting)
                {
                    continue;
                }
            }
            // Avoid introducing undefined behavior due to poison which didn't
            // exist in the original program. For integers, we strip and
            // reinfer as needed, see linearFunctionTestReplace. For pointers,
            // we restrict transforms as there is no good way to reinfer
            // inbounds once lost.
            let phi_ty = self.type_of_value(phi_value);
            if self.is_pointer(phi_ty) {
                let term = self.function.terminator(exiting).expect("a terminator");
                let cx = self.env.cx(&*self.context, &*self.function);
                let query = cx.query(None);
                if !llrm_mir::valuetracking::guarantee::must_execute_ub_if_poison_on_path_to(&query, phi, term, cx.tree)
                {
                    continue;
                }
            }
            if let Some((best_phi, best_init)) = best
                && !self.is_almost_dead_iv(best_phi, latch, cond)
            {
                // Don't force a live loop counter if another IV can be used.
                if self.is_almost_dead_iv(phi, latch, cond) {
                    continue;
                }
                // Prefer to count-from-zero. This is a more "canonical"
                // counter form. It also prefers integer to pointer IVs.
                let zero = |this: &Self, s: Scev| this.se.as_constant(s).is_some_and(|c| c.is_zero());
                if zero(self, best_init) != zero(self, init) {
                    if zero(self, best_init) {
                        continue;
                    }
                } else {
                    // If two IVs both count from zero or both count from
                    // nonzero then the narrower is likely a dead phi that has
                    // been widened. Use the wider phi to allow the other to be
                    // eliminated.
                    let best_width = {
                        let best_value = Operand::Value(self.function.instruction(best_phi).result.expect("a value"));
                        let cx = self.env.cx(&*self.context, &*self.function);
                        let s = self.se.get_scev(&cx, best_value);
                        self.se.get_type_size_in_bits(&cx, self.se.type_of(s))
                    };
                    if phi_width <= best_width {
                        continue;
                    }
                }
            }
            best = Some((phi, init));
        }
        best.map(|(phi, _)| phi)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1018
    /// `genLoopLimit`: the value `ind_var` holds after the loop's backedge is
    /// taken `exit_count` times, written before the exiting block's branch.
    fn gen_loop_limit(
        &mut self,
        header: BlockId,
        ind_var: InstId,
        exiting: BlockId,
        exit_count: Scev,
        use_post_inc: bool,
    ) -> Option<Operand> {
        let ind_value = Operand::Value(self.function.instruction(ind_var).result.expect("a phi value"));
        let cx = self.env.cx(&*self.context, &*self.function);
        let mut ar = self.se.get_scev(&cx, ind_value);
        let (ar_ty, count_ty) = (self.se.type_of(ar), self.se.type_of(exit_count));
        let is_int = matches!(ar_ty, Ty::Int(_));
        // For integer IVs, truncate the IV before computing the limit unless
        // we know apriori that the limit must be a constant when evaluated in
        // the bitwidth of the IV. We prefer (potentially) keeping a truncate
        // of the IV in the loop over a (potentially) expensive expansion of
        // the widened exit count add(zext(add)) expression.
        if is_int && self.se.get_type_size_in_bits(&cx, ar_ty) > self.se.get_type_size_in_bits(&cx, count_ty) {
            let Kind::AddRec(ops, _) = self.se.node(ar).kind.clone() else { unreachable!("a counter") };
            let (init, count) = (ops[0].scev, exit_count);
            if self.se.as_constant(init).is_none() || self.se.as_constant(count).is_none() {
                let trunc = self.se.get_truncate_expr(&cx, Use::of(ar), count_ty, 0);
                // The following bailout is necessary due to the interaction
                // with the depth limit in SCEV analysis.
                if !matches!(self.se.node(trunc).kind, Kind::AddRec(..)) {
                    return None;
                }
                ar = trunc;
            }
        }
        let ar_base = if use_post_inc { self.se.get_post_inc_expr(&cx, ar) } else { ar };
        let limit = self.se.evaluate_at_iteration(&cx, ar_base, exit_count);
        let ty = self.se.type_of(ar_base);
        let _ = header;
        let term = self.function.terminator(exiting).expect("a terminator");
        self.rewriter.set_insert_point(term);
        Some(self.rewriter.expand_code_for(
            &self.env,
            self.context,
            self.declared,
            self.function,
            self.se,
            limit,
            Some(ty),
        ))
    }

    /// `IRBuilder::CreateCast` before `before`: a constant operand is folded.
    fn create_cast(
        &mut self,
        op: CastOp,
        v: Operand,
        to: TypeId,
        name: &str,
        before: InstId,
    ) -> Operand {
        if matches!(v, Operand::Constant(_))
            && let Some(folded) =
                llrm_mir::simplify::simplify_cast_operand(self.context, self.layout, self.function, op, v, to)
        {
            return folded;
        }
        let inst = self.function.create_instruction(Opcode::Cast(op), to, vec![v], Flags::default(), Some(name));
        self.function.insert(inst, Position::Before(before)).expect("a placed branch");
        Operand::Value(self.function.instruction(inst).result.expect("a cast value"))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1059
    /// `linearFunctionTestReplace`: the exit test of `exiting` compares
    /// `ind_var` (or its increment) with the limit after `exit_count` trips.
    fn linear_function_test_replace(
        &mut self,
        header: BlockId,
        exiting: BlockId,
        exit_count: Scev,
        ind_var: InstId,
    ) -> bool {
        let (latch, l_contains_first) = {
            let cx = self.env.cx(&*self.context, &*self.function);
            let l = cx.loops.get_loop_for(header).expect("a loop");
            let (branch, _) = branch_of(cx.function, exiting).expect("a branch");
            let Operand::Block(first) = cx.function.instruction(branch).operands[1] else { unreachable!("a target") };
            (l.get_loop_latch(cx.function).expect("a latch"), l.contains(first))
        };
        let ind_value = Operand::Value(self.function.instruction(ind_var).result.expect("a phi value"));
        let inc_var = arms(self.function, ind_var).into_iter().find(|(_, from)| *from == latch).expect("a latch").0;
        let inc_inst = def_of(self.function, inc_var).expect("an increment instruction");
        let ind_ty = self.type_of_value(ind_value);
        let term = self.function.terminator(exiting).expect("a terminator");

        // Initialize CmpIndVar to the preincremented IV.
        let mut cmp_ind_var = ind_value;
        let mut use_post_inc = false;

        // If the exiting block is the same as the backedge block, we prefer to
        // compare against the post-incremented value, otherwise we must
        // compare against the preincremented value.
        if exiting == latch {
            // For pointer IVs, we chose to not strip inbounds which requires
            // us not to add a potentially UB introducing use. We need to
            // either a) show the loop test we're modifying is already in
            // post-inc form, or b) show that adding a use must not introduce
            // UB.
            let safe_to_post_inc =
                !self.is_pointer(ind_ty) || is_loop_exit_test_based_on(self.function, inc_var, exiting) || {
                    let cx = self.env.cx(&*self.context, &*self.function);
                    let query = cx.query(None);
                    llrm_mir::valuetracking::guarantee::must_execute_ub_if_poison_on_path_to(
                        &query, inc_inst, term, cx.tree,
                    )
                };
            if safe_to_post_inc {
                use_post_inc = true;
                cmp_ind_var = inc_var;
            }
        }

        let Some(mut exit_cnt) = self.gen_loop_limit(header, ind_var, exiting, exit_count, use_post_inc) else {
            return false;
        };
        debug_assert_eq!(
            self.is_pointer(self.type_of_value(exit_cnt)),
            self.is_pointer(ind_ty),
            "genLoopLimit missed a cast"
        );

        // It may be necessary to drop nowrap flags on the incrementing
        // instruction if either LFTR moves from a pre-inc check to a post-inc
        // check (in which case the increment might have previously been poison
        // on the last iteration only) or if LFTR switches to a different IV
        // that was previously dynamically dead (and as such may be arbitrarily
        // poison). We remove any nowrap flags that SCEV didn't infer for the
        // post-inc addrec (even if we use a pre-inc check), because the
        // pre-inc addrec flags may be adopted from the original instruction,
        // while SCEV has to explicitly prove the post-inc nowrap flags.
        if matches!(self.function.instruction(inc_inst).opcode, Opcode::Binary(_)) {
            let (nuw, nsw) = {
                let cx = self.env.cx(&*self.context, &*self.function);
                let ar = self.se.get_scev(&cx, inc_var);
                let flags = self
                    .se
                    .get_no_wrap_flags(
                        ar,
                        llrm_analysis::scev::expr::flags::NUW | llrm_analysis::scev::expr::flags::NSW,
                    );
                (flags & llrm_analysis::scev::expr::flags::NUW != 0, flags & llrm_analysis::scev::expr::flags::NSW != 0)
            };
            let mut flags = self.function.instruction(inc_inst).flags;
            if flags.contains(Flags::NUW) && !nuw {
                flags.remove(Flags::NUW);
            }
            if flags.contains(Flags::NSW) && !nsw {
                flags.remove(Flags::NSW);
            }
            self.function.set_flags(inc_inst, flags);
        }

        // Insert a new icmp_ne or icmp_eq instruction before the branch.
        let pred = if l_contains_first { IntPredicate::Ne } else { IntPredicate::Eq };

        // For integer IVs, if we evaluated the limit in the narrower bitwidth
        // to avoid the expensive expansion of the limit expression in the
        // wider type, emit a truncate to narrow the IV to the ExitCount type.
        // This is safe since we know (from the exit count bitwidth), that we
        // can't self-wrap in the narrower type.
        let (cmp_size, cnt_size) = {
            let cx = self.env.cx(&*self.context, &*self.function);
            let bits = |this: &Self, v: Operand| match this.context.types.get(this.type_of_value(v)) {
                MirType::Int(bits) => *bits,
                _ => cx.layout.pointer(0).index_bits,
            };
            (bits(self, cmp_ind_var), bits(self, exit_cnt))
        };
        if cmp_size > cnt_size {
            debug_assert!(
                !self.is_pointer(self.type_of_value(cmp_ind_var)) && !self.is_pointer(self.type_of_value(exit_cnt))
            );
            // Before resorting to actually inserting the truncate, use the
            // same reasoning as from SimplifyIndvar::eliminateTrunc to see if
            // we can extend the other side of the comparison instead. We
            // still evaluate the limit in the narrower bitwidth, we just
            // prefer a zext/sext outside the loop to a truncate within in.
            let mut extended = false;
            let narrow_ty = self.type_of_value(exit_cnt);
            let wide_ty = self.type_of_value(cmp_ind_var);
            let (narrow_scev_ty, wide_scev_ty) = (Ty::Int(cnt_size), Ty::Int(cmp_size));
            let (iv, zext_trunc, sext_trunc) = {
                let cx = self.env.cx(&*self.context, &*self.function);
                let iv = self.se.get_scev(&cx, cmp_ind_var);
                let truncated = self.se.get_truncate_expr(&cx, Use::of(iv), narrow_scev_ty, 0);
                let zext = self.se.get_zero_extend_expr(&cx, Use::of(truncated), wide_scev_ty, 0);
                let sext = self.se.get_sign_extend_expr(&cx, Use::of(truncated), wide_scev_ty, 0);
                (iv, zext, sext)
            };
            if zext_trunc == iv {
                extended = true;
                exit_cnt = self.create_cast(CastOp::ZExt, exit_cnt, ind_ty, "wide.trip.count", term);
            } else if sext_trunc == iv {
                extended = true;
                exit_cnt = self.create_cast(CastOp::SExt, exit_cnt, ind_ty, "wide.trip.count", term);
            }
            if extended {
                self.make_cast_loop_invariant(header, exit_cnt);
            } else {
                cmp_ind_var = self.create_cast(CastOp::Trunc, cmp_ind_var, narrow_ty, "lftr.wideiv", term);
            }
            let _ = wide_ty;
        }

        let i1 = self.context.types.int(1);
        let cond = self
            .function
            .create_instruction(
                Opcode::ICmp(pred),
                i1,
                vec![cmp_ind_var, exit_cnt],
                Flags::default(),
                Some("exitcond"),
            );
        self.function.insert(cond, Position::Before(term)).expect("a placed branch");
        let new_cond = Operand::Value(self.function.instruction(cond).result.expect("a compare value"));
        // It's tempting to use replaceAllUsesWith here to fully replace the
        // old comparison, but that's not immediately safe, since users of the
        // old comparison may not be dominated by the new comparison. Instead,
        // just update the branch to use the new comparison; in the common case
        // this will make old comparison dead.
        self.function.set_operand(term, 0, new_cond);
        true
    }

    /// `L->makeLoopInvariant(ExitCnt, Discard)` for the cast just made: moved
    /// before the preheader's terminator where its operand is invariant (a
    /// cast is always speculatable and reads no memory).
    fn make_cast_loop_invariant(
        &mut self,
        header: BlockId,
        cast: Operand,
    ) {
        let Some(inst) = def_of(self.function, cast) else { return };
        let (inside, invariant, preheader) = {
            let cx = self.env.cx(&*self.context, &*self.function);
            let l = cx.loops.get_loop_for(header).expect("a loop");
            (
                l.contains_instruction(cx.function, inst),
                l.is_loop_invariant(cx.function, self.function.instruction(inst).operands[0]),
                l.get_loop_preheader(cx.function).expect("a preheader"),
            )
        };
        if inside && invariant {
            let term = self.function.terminator(preheader).expect("a terminator");
            self.function.move_to(inst, Position::Before(term)).expect("a placed instruction");
        }
    }
}

#[cfg(test)]
#[path = "lftr_tests.rs"]
mod tests;
