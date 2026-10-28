//! Where an expression is defined and whether an instruction is certain to
//! run when another does: `getDefiningScopeBound`,
//! `isGuaranteedToTransferExecutionTo` and `isSCEVExprNeverPoison`.

use llrm_mir::constantfolding;
use llrm_mir::context::ConstantId;
use llrm_mir::hash::HashSet;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Opcode};
use llrm_mir::valuetracking::guarantee;

use super::backedge::ExitCountKind;
use super::expr::{Kind, Scev, Ty, Use, flags};
use super::{Cx, Flags, ScalarEvolution};

/// The `ScanLimit` default of `isGuaranteedToTransferExecutionToSuccessor`.
const SCAN_LIMIT: u32 = 32;

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7367
    /// `getNonTrivialDefiningScopeBound`.
    fn get_non_trivial_defining_scope_bound(
        &self,
        cx: &Cx,
        s: Scev,
    ) -> Option<InstId> {
        match self.node(s).kind {
            Kind::AddRec(_, header) => cx.function.block(header).first(),
            Kind::Unknown(Operand::Value(id)) => match cx.function.value(id).def {
                ValueDef::Instruction(inst) => Some(inst),
                ValueDef::Argument(_) => None,
            },
            _ => None,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7376
    /// `getDefiningScopeBound(Ops, Precise)`: an instruction that dominates
    /// every instruction the expressions of `ops` are defined at, found by a
    /// bounded search.
    pub fn get_defining_scope_bound(
        &self,
        cx: &Cx,
        ops: &[Use],
    ) -> (InstId, bool) {
        let mut precise = true;
        // Do a bounded search of the def relation of the requested SCEVs.
        let mut visited: HashSet<Scev> = HashSet::default();
        let mut worklist: Vec<Scev> = Vec::new();
        let mut push_op = |s: Scev, precise: &mut bool, worklist: &mut Vec<Scev>| {
            if !visited.insert(s) {
                return;
            }
            // Threshold of 30 here is arbitrary.
            if visited.len() > 30 {
                *precise = false;
                return;
            }
            worklist.push(s);
        };

        for s in ops {
            push_op(s.scev, &mut precise, &mut worklist);
        }

        let mut bound: Option<InstId> = None;
        while let Some(s) = worklist.pop() {
            if let Some(def) = self.get_non_trivial_defining_scope_bound(cx, s) {
                if bound.is_none_or(|bound| self.inst_dominates(cx, bound, def)) {
                    bound = Some(def);
                }
            } else {
                for op in self.operands_of(s) {
                    push_op(op.scev, &mut precise, &mut worklist);
                }
            }
        }
        let entry = cx.function.entry().expect("a function with a body");
        (bound.unwrap_or_else(|| cx.function.block(entry).first().expect("a block with an instruction")), precise)
    }

    /// `DT.dominates(const Instruction *, const Instruction *)`.
    fn inst_dominates(
        &self,
        cx: &Cx,
        a: InstId,
        b: InstId,
    ) -> bool {
        cx.tree.instruction_dominates(cx.function, a, b)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7407
    /// `ScalarEvolution::isGuaranteedToTransferExecutionTo`.
    pub fn is_guaranteed_to_transfer_execution_to(
        &self,
        cx: &Cx,
        a: InstId,
        b: InstId,
    ) -> bool {
        let q = cx.query(None);
        let function = cx.function;
        let (Some(a_block), Some(b_block)) = (function.parent(a), function.parent(b)) else { return false };
        if a_block == b_block
            && (a == b || function.comes_before(a, b))
            && guarantee::is_guaranteed_to_transfer_execution_to_successor_range(
                &q,
                function.iter_range(a, b),
                SCAN_LIMIT,
            )
        {
            return true;
        }

        if let Some(b_loop) = cx.loops.get_loop_for(b_block)
            && b_loop.get_header() == b_block
            && b_loop.get_loop_preheader(function) == Some(a_block)
            && guarantee::is_guaranteed_to_transfer_execution_to_successor_range(&q, function.iter_from(a), SCAN_LIMIT)
            && guarantee::is_guaranteed_to_transfer_execution_to_successor_range(
                &q,
                function.iter_block(b_block).take_while(|&one| one != b),
                SCAN_LIMIT,
            )
        {
            return true;
        }
        false
    }

    /// `isGuaranteedToTransferExecutionTo(getDefiningScopeBound(ops),
    /// &*L->getHeader()->begin())`, as `getAddExpr` asks it.
    pub(super) fn is_guaranteed_to_transfer_execution_to_loop_header(
        &mut self,
        cx: &Cx,
        ops: &[Use],
        l: BlockId,
    ) -> bool {
        let (def_i, _) = self.get_defining_scope_bound(cx, ops);
        let reach_i = cx.function.block(l).first().expect("a block with an instruction");
        self.is_guaranteed_to_transfer_execution_to(cx, def_i, reach_i)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7441
    /// `ScalarEvolution::isSCEVExprNeverPoison`: whether `i` is certain not to
    /// yield poison where the expressions of its operands are defined.
    pub fn is_scev_expr_never_poison(
        &mut self,
        cx: &Cx,
        i: InstId,
    ) -> bool {
        // Only proceed if we can prove that I does not yield poison.
        let q = cx.query(None);
        let result = cx.function.instruction(i).result.map(Operand::Value);
        let Some(result) = result else { return false };
        if !guarantee::program_undefined_if_poison(&q, result) {
            return false;
        }

        // At this point we know that if I is executed, then it does not wrap
        // according to at least one of NSW or NUW. If I is not executed, then
        // we do not know if the calculation that I represents would wrap.
        // Multiple instructions can map to the same SCEV. If we apply NSW or
        // NUW from I to the SCEV, we must guarantee no wrapping for that SCEV
        // also when it is derived from other instructions that map to the same
        // SCEV. We cannot make that guarantee for cases where I is not
        // executed. So we need to find a upper bound on the defining scope for
        // the SCEV, and prove that I is executed every time we enter that
        // scope. When the bounding scope is a loop (the common case), this is
        // equivalent to proving I executes on every iteration of that loop.
        let mut scev_ops: Vec<Use> = Vec::new();
        for op in cx.function.instruction(i).operands.clone() {
            // I could be an extractvalue from a call to an overflow intrinsic.
            // TODO: We can do better here in some cases.
            let scevable = match op {
                Operand::Value(id) => self.ty_of(cx, cx.function.value(id).ty).is_some(),
                Operand::Constant(id) => self.ty_of(cx, cx.context.get(id).ty).is_some(),
                Operand::Block(_) => false,
            };
            if scevable {
                scev_ops.push(Use::of(self.get_scev(cx, op)));
            }
        }
        let (def_i, _) = self.get_defining_scope_bound(cx, &scev_ops);
        self.is_guaranteed_to_transfer_execution_to(cx, def_i, i)
    }
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10079
    /// `getSCEVAtScope(S, L)`: `S` as it is in the scope of loop `l` (none:
    /// the function body), with the exit values of the loops inside it folded
    /// in. The `SCEVUse` it gives keeps the use flags: see
    /// [`Self::get_scev_at_scope`] for the pointer form most callers want.
    pub fn get_scev_at_scope_use(
        &mut self,
        cx: &Cx,
        v: Scev,
        l: Option<BlockId>,
    ) -> Use {
        let values = self.values_at_scopes.entry(v).or_default();
        // Check to see if we've folded this expression at this loop before.
        for ls in values.iter() {
            if ls.0 == l {
                return ls.1.unwrap_or(Use::of(v));
            }
        }

        values.push((l, None));

        #[cfg(test)]
        if let Some(hook) = self.compute_hook.take() {
            hook(self, cx, v);
        }

        // Otherwise compute it.
        let c = self.compute_scev_at_scope(cx, v, l);
        // Computing can forget `v` (its exit count asks for the expressions
        // that use it), so the entry is looked up again, as LLVM's
        // `ValuesAtScopes[V]` does.
        let values = self.values_at_scopes.entry(v).or_default();
        if let Some(ls) = values.iter_mut().rev().find(|ls| ls.0 == l) {
            ls.1 = Some(c);
            // Record the dependency under the bare expression: invalidation
            // walks expressions, and any use flags on C do not change which
            // expression this is the value at scope of.
            if self.as_constant(c.scev).is_none() {
                self.values_at_scopes_users.entry(c.scev).or_default().push((l, v));
            }
        }
        c
    }

    /// `getSCEVAtScope(S, L)` as a `const SCEV *`, which is how its callers
    /// hold it.
    pub fn get_scev_at_scope(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: Option<BlockId>,
    ) -> Scev {
        self.get_scev_at_scope_use(cx, s, l).scev
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10413
    /// `getSCEVAtScope(Value *, L)`.
    pub fn get_scev_at_scope_of_value(
        &mut self,
        cx: &Cx,
        v: Operand,
        l: Option<BlockId>,
    ) -> Use {
        let s = self.get_scev(cx, v);
        self.get_scev_at_scope_use(cx, s, l)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10100
    /// `getSCEVAtExit`: `v` as it is after control leaves `l` through
    /// `exiting_block`.
    pub fn get_scev_at_exit(
        &mut self,
        cx: &Cx,
        v: Scev,
        l: BlockId,
        exiting_block: BlockId,
    ) -> Use {
        let parent = super::disposition::loop_of(cx, l).get_parent_loop().map(|one| one.get_header());
        let mut exit_value = self.get_scev_at_scope_use(cx, v, parent);
        if !self.is_loop_invariant(cx, exit_value.scev, Some(l)) {
            // If we failed to evaluate it in the outer scope, try to evaluate
            // an addrec for the specific exit.
            // TODO: Generalize this to other expressions.
            let exit_count = self.get_exit_count(cx, l, exiting_block, ExitCountKind::Exact);
            if !matches!(self.node(exit_count).kind, Kind::CouldNotCompute)
                && matches!(self.node(v).kind, Kind::AddRec(_, header) if header == l)
            {
                exit_value = Use::of(self.evaluate_at_iteration(cx, v, exit_count));
            }
        }
        exit_value
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L1008
    /// `SCEVAddRecExpr::getExitValue`: the value the recurrence has when its
    /// loop exits.
    pub fn get_exit_value(
        &mut self,
        cx: &Cx,
        ar: Scev,
    ) -> Use {
        let Kind::AddRec(ops, header) = self.node(ar).kind.clone() else { unreachable!("an add recurrence") };
        let btc = self.get_backedge_taken_count(cx, header, ExitCountKind::Exact);
        if matches!(self.node(btc).kind, Kind::CouldNotCompute) {
            return Use::of(btc);
        }
        // The loop reaches iteration BTC, so the value this recurrence
        // computes there is the value it had, and that did not wrap.
        let f = if self.is_affine(ar) { self.get_no_wrap_flags(ar, flags::NUW) } else { flags::NONE };
        self.evaluate_at_iteration_ops(cx, &ops, btc, f)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10209
    /// `getWithOperands`: `s` with `new_ops` as its operands.
    pub fn get_with_operands(
        &mut self,
        cx: &Cx,
        s: Scev,
        new_ops: Vec<Use>,
    ) -> Scev {
        let ty = self.node(s).ty;
        match self.node(s).kind.clone() {
            Kind::Truncate(_) => self.get_truncate_expr(cx, new_ops[0], ty.expect("a type"), 0),
            Kind::ZeroExtend(_) => self.get_zero_extend_expr(cx, new_ops[0], ty.expect("a type"), 0),
            Kind::SignExtend(_) => self.get_sign_extend_expr(cx, new_ops[0], ty.expect("a type"), 0),
            Kind::PtrToAddr(_) => self.get_ptr_to_addr_expr(cx, new_ops[0].scev),
            Kind::AddRec(_, header) => {
                let f = self.get_no_wrap_flags(s, flags::MASK);
                self.get_add_rec_expr(cx, new_ops, header, Flags::expr(f)).scev
            }
            Kind::Add(_) => {
                let f = self.get_no_wrap_flags(s, flags::MASK);
                self.get_add_expr(cx, new_ops, Flags::expr(f), 0).scev
            }
            Kind::Mul(_) => {
                let f = self.get_no_wrap_flags(s, flags::MASK);
                self.get_mul_expr(cx, new_ops, Flags::expr(f), 0).scev
            }
            Kind::UDiv(..) => self.get_udiv_expr(cx, new_ops[0], new_ops[1]),
            Kind::UMax(_) => self.get_umax_expr(cx, new_ops),
            Kind::SMax(_) => self.get_smax_expr(cx, new_ops),
            Kind::UMin(_) => self.get_umin_expr(cx, new_ops, false),
            Kind::SMin(_) => self.get_smin_expr(cx, new_ops),
            Kind::SequentialUMin(_) => self.get_umin_expr(cx, new_ops, true),
            Kind::Constant(_) | Kind::Unknown(_) => s,
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10253
    /// `computeSCEVAtScope`.
    ///
    /// # DEVIATE 10
    /// `ConstantEvolutionLoopExitValue` is not cached per phi; the result is memoized per value and scope.
    fn compute_scev_at_scope(
        &mut self,
        cx: &Cx,
        v: Scev,
        l: Option<BlockId>,
    ) -> Use {
        match self.node(v).kind.clone() {
            Kind::Constant(_) => Use::of(v),
            Kind::AddRec(operands, header) => {
                let mut add_rec = v;
                // If this is a loop recurrence for a loop that does not
                // contain L, then we are dealing with the final value computed
                // by the loop.
                // First, attempt to evaluate each operand. Avoid performing
                // the look-up in the common case where the specified
                // expression has no loop-variant portions.
                let e = operands.len();
                let mut i = 0;
                while i != e {
                    let op_at_scope = self.get_scev_at_scope_use(cx, operands[i].scev, l);
                    if op_at_scope == operands[i] {
                        i += 1;
                        continue;
                    }

                    // Okay, at least one of these operands is loop variant but
                    // might be foldable. Build a new instance of the folded
                    // commutative expression.
                    let mut new_ops: Vec<Use> = Vec::with_capacity(e);
                    new_ops.extend_from_slice(&operands[..i]);
                    new_ops.push(op_at_scope);
                    for op in &operands[i + 1..] {
                        new_ops.push(self.get_scev_at_scope_use(cx, op.scev, l));
                    }

                    let nw = self.get_no_wrap_flags(v, flags::NW);
                    let folded_rec = self.get_add_rec_expr(cx, new_ops, header, Flags::expr(nw)).scev;
                    // The addrec may be folded to a nonrecurrence, for
                    // example, if the induction variable is multiplied by zero
                    // after constant folding. Go ahead and return the folded
                    // value.
                    if !matches!(self.node(folded_rec).kind, Kind::AddRec(..)) {
                        return Use::of(folded_rec);
                    }
                    add_rec = folded_rec;
                    break;
                }

                // If the scope is outside the addrec's loop, evaluate it by
                // using the loop exit value of the addrec.
                let Kind::AddRec(_, add_rec_loop) = self.node(add_rec).kind else { unreachable!("an add recurrence") };
                if !self.loop_contains_scope(cx, add_rec_loop, l) {
                    let exit_value = self.get_exit_value(cx, add_rec);
                    if matches!(self.node(exit_value.scev).kind, Kind::CouldNotCompute) {
                        return Use::of(add_rec);
                    }
                    return exit_value;
                }

                Use::of(add_rec)
            }
            Kind::Truncate(_)
            | Kind::ZeroExtend(_)
            | Kind::SignExtend(_)
            | Kind::PtrToAddr(_)
            | Kind::Add(_)
            | Kind::Mul(_)
            | Kind::UDiv(..)
            | Kind::UMax(_)
            | Kind::SMax(_)
            | Kind::UMin(_)
            | Kind::SMin(_)
            | Kind::SequentialUMin(_) => {
                let ops = self.operands_of(v);
                // Avoid performing the look-up in the common case where the
                // specified expression has no loop-variant portions.
                for i in 0..ops.len() {
                    let op_at_scope = self.get_scev_at_scope_use(cx, ops[i].scev, l);
                    if op_at_scope.scev != ops[i].scev {
                        // Okay, at least one of these operands is loop variant
                        // but might be foldable. Build a new instance of the
                        // folded commutative expression.
                        let mut new_ops: Vec<Use> = Vec::with_capacity(ops.len());
                        new_ops.extend_from_slice(&ops[..i]);
                        new_ops.push(op_at_scope);
                        for op in &ops[i + 1..] {
                            new_ops.push(self.get_scev_at_scope_use(cx, op.scev, l));
                        }

                        return Use::of(self.get_with_operands(cx, v, new_ops));
                    }
                }
                // If we got here, all operands are loop invariant.
                Use::of(v)
            }
            Kind::Unknown(value) => {
                // If this instruction is evolved from a constant-evolving PHI,
                // compute the exit value from the loop without using SCEVs.
                let Some(inst) = self.defining_inst(cx, value) else {
                    // This is some other type of SCEVUnknown, just return it.
                    return Use::of(v);
                };

                if cx.function.instruction(inst).opcode == Opcode::Phi {
                    let phi_block = cx.function.parent(inst).expect("a phi in a block");
                    let curr_loop = cx.loops.get_loop_for(phi_block);
                    // Looking for loop exit value.
                    if let Some(curr_loop) = curr_loop
                        && curr_loop.get_parent_loop().map(|one| one.get_header()) == l
                        && phi_block == curr_loop.get_header()
                    {
                        let curr_header = curr_loop.get_header();
                        // Okay, there is no closed form solution for the PHI
                        // node. Check to see if the loop that contains it has
                        // a known backedge-taken count. If so, we may be able
                        // to force computation of the exit value.
                        let backedge_taken_count = self.get_backedge_taken_count(cx, curr_header, ExitCountKind::Exact);
                        // This trivial case can show up in some degenerate
                        // cases where the incoming IR has not yet been fully
                        // simplified.
                        if self.as_constant(backedge_taken_count).is_some_and(|c| c.is_zero()) {
                            let mut init_value: Option<Operand> = None;
                            let mut multiple_init_values = false;
                            for pair in cx.function.instruction(inst).operands.chunks(2) {
                                let Operand::Block(block) = pair[1] else { unreachable!("a phi's block") };
                                if !curr_loop.contains(block) {
                                    match init_value {
                                        None => init_value = Some(pair[0]),
                                        Some(init) if init != pair[0] => {
                                            multiple_init_values = true;
                                            break;
                                        }
                                        Some(_) => {}
                                    }
                                }
                            }
                            if let (false, Some(init_value)) = (multiple_init_values, init_value) {
                                return Use::of(self.get_scev(cx, init_value));
                            }
                        }
                        // Do we have a loop invariant value flowing around the
                        // backedge for a loop which must execute the backedge?
                        let incoming = cx.function.instruction(inst).operands.len() / 2;
                        if !matches!(self.node(backedge_taken_count).kind, Kind::CouldNotCompute)
                            && self.is_known_non_zero(cx, backedge_taken_count)
                            && incoming == 2
                        {
                            let operands = &cx.function.instruction(inst).operands;
                            let in_loop_pred =
                                if matches!(operands[1], Operand::Block(b) if curr_loop.contains(b)) { 0 } else { 1 };
                            let backedge_val = operands[in_loop_pred * 2];
                            if self.value_is_loop_invariant(cx, backedge_val, &curr_loop) {
                                return Use::of(self.get_scev(cx, backedge_val));
                            }
                        }
                        // Okay, we know how many times the containing loop executes. If this is a
                        // constant evolving phi, get the final value at the specified iteration.
                        if let Some(count) = self.as_constant(backedge_taken_count)
                            && let Some(exit) = self.get_constant_evolution_loop_exit_value(
                                cx,
                                inst,
                                count.zext_value(),
                                &curr_loop,
                            )
                        {
                            return Use::of(self.get_scev(cx, Operand::Constant(exit)));
                        }
                    }
                }

                // Okay, this is an expression that we cannot symbolically evaluate into a SCEV. Check
                // to see if it's possible to symbolically evaluate the arguments into constants, and
                // if so, try to constant propagate the result.
                if cx.function.instruction(inst).opcode != Opcode::Phi
                    && self.is_value_scevable(cx, value)
                    && super::exit_value::can_constant_fold(cx, inst)
                    && !matches!(cx.function.instruction(inst).opcode, Opcode::Load { volatile: true, .. })
                {
                    let operands = cx.function.instruction(inst).operands.clone();
                    let mut folded: Vec<ConstantId> = Vec::with_capacity(operands.len());
                    for op in operands {
                        let constant = match op {
                            Operand::Constant(c) => c,
                            Operand::Value(_) if self.is_value_scevable(cx, op) => {
                                let at_scope = self.get_scev_at_scope_of_value(cx, op, l).scev;
                                let Some(c) = self.build_constant_from_scev(cx, at_scope) else { return Use::of(v) };
                                c
                            }
                            _ => return Use::of(v),
                        };
                        folded.push(constant);
                    }
                    let env = constantfolding::Env { globals: cx.globals, layout: cx.layout };
                    if let Some(c) = constantfolding::constant_fold_inst_operands_at(cx.context, env, cx.function, inst, &folded) {
                        return Use::of(self.get_scev(cx, Operand::Constant(c)));
                    }
                }
                Use::of(v)
            }
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L10290
    /// `BuildConstantFromSCEV`: the constant `s` is made of, if it is one.
    fn build_constant_from_scev(
        &self,
        cx: &Cx,
        s: Scev,
    ) -> Option<ConstantId> {
        let ty_id = |ty: Ty| match ty {
            Ty::Int(bits) => cx.context.types.int(bits),
            Ty::Ptr(space) => cx.context.types.ptr(space),
        };
        let node = self.node(s);
        match &node.kind {
            Kind::Constant(ap) => Some(cx.context.int(cx.context.types.int(ap.width), ap.zext_value() as i128)),
            Kind::Unknown(Operand::Constant(c)) => Some(*c),
            Kind::Truncate(op) | Kind::ZeroExtend(op) | Kind::SignExtend(op) => {
                let signed = matches!(node.kind, Kind::SignExtend(_));
                let c = self.build_constant_from_scev(cx, op.scev)?;
                constantfolding::constant_fold_integer_cast(cx.context, cx.layout, c, ty_id(node.ty?), signed)
            }
            Kind::Add(ops) | Kind::Mul(ops) => {
                let env = constantfolding::Env { globals: cx.globals, layout: cx.layout };
                let is_add = matches!(node.kind, Kind::Add(_));
                let mut pointer: Option<ConstantId> = None;
                let mut acc: Option<ConstantId> = None;
                for op in ops {
                    let c = self.build_constant_from_scev(cx, op.scev)?;
                    if self.node(op.scev).ty?.is_pointer() {
                        if !is_add || pointer.replace(c).is_some() {
                            return None;
                        }
                        continue;
                    }
                    let bop = if is_add { BinaryOp::Add } else { BinaryOp::Mul };
                    acc = Some(match acc {
                        None => c,
                        Some(a) => constantfolding::constant_fold_binary_op_operands(cx.context, env, bop, a, c)?,
                    });
                }
                match (pointer, acc) {
                    (Some(p), Some(offset)) => {
                        let i8 = cx.context.types.int(8);
                        Some(constantfolding::constant_fold_get_element_ptr(cx.context, i8, p, &[offset], 0))
                    }
                    (Some(p), None) => Some(p),
                    (None, acc) => acc,
                }
            }
            _ => None,
        }
    }

    /// `AddRec->getLoop()->contains(L)` with `L` a loop or none (the function
    /// body): `Loop::contains(const Loop *)`.
    fn loop_contains_scope(
        &self,
        cx: &Cx,
        add_rec_loop: BlockId,
        l: Option<BlockId>,
    ) -> bool {
        super::disposition::loop_of(cx, add_rec_loop).contains_loop(l.map(|l| super::disposition::loop_of(cx, l)))
    }
}
