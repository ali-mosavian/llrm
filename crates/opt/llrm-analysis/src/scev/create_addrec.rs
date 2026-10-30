//! `createAddRecFromPHI` and `createSimpleAffineAddRec`: a loop-header PHI
//! that a loop-invariant amount is added to each iteration is an add
//! recurrence. With the three rewriters they use (`SCEVInitRewriter`,
//! `SCEVBackedgeConditionFolder`, `SCEVShiftRewriter`) and
//! `isAddRecNeverPoison`.
//!
//! # DEVIATE 1 (no predicated SCEV)
//! `createAddRecFromPHIWithCasts` is not translated.

use llrm_mir::hash::HashMap;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::guarantee::{self, Operator};

use super::create::{Bop, Opnd};
use super::create_phi::GepFlags;
use super::expr::{Kind, Scev, Ty, Use, flags};
use super::{Cx, Flags, ScalarEvolution};

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7468
    /// `isAddRecNeverPoison`: whether `i` cannot be poison when the add
    /// recurrence of loop `l` it is the post-increment of is computed.
    pub fn is_add_rec_never_poison(
        &mut self,
        cx: &Cx,
        i: InstId,
        l: BlockId,
    ) -> bool {
        // If we know that \c I can never be poison period, then that's enough.
        if self.is_scev_expr_never_poison(cx, i) {
            return true;
        }

        // If the loop only has one exit, then we know that, if the loop is
        // entered, any instruction dominating that exit will be executed. If
        // any such instruction would result in UB, the addrec cannot be
        // poison.
        //
        // This is basically the same reasoning as in isSCEVExprNeverPoison(),
        // but also handles uses outside the loop header (they just need to
        // dominate the single exit).
        let loop_ = super::disposition::loop_of(cx, l);
        let Some(exiting_bb) = loop_.get_exiting_block(cx.function) else { return false };
        if !self.loop_has_no_abnormal_exits(cx, l) {
            return false;
        }

        let q = cx.query(None);
        let mut known_poison = guarantee::PoisonSet::default();
        let mut worklist: Vec<InstId> = Vec::new();

        // We start by assuming \c I, the post-inc add recurrence, is poison.
        // Only things that are known to be poison under that assumption go on
        // the Worklist.
        known_poison.insert(self.inst_value(cx, i));
        worklist.push(i);

        while let Some(poison) = worklist.pop() {
            let Some(result) = cx.function.instruction(poison).result else { continue };
            for used in cx.function.users(result) {
                let poison_user = used.user;
                let user_block = cx.function.parent(poison_user);
                if guarantee::must_trigger_ub(&q, poison_user, &known_poison)
                    && user_block.is_some_and(|block| cx.tree.dominates(block, exiting_bb))
                {
                    return true;
                }

                if guarantee::propagates_poison(&q, Operator::Instruction(poison_user), used.index as usize)
                    && user_block.is_some_and(|block| loop_.contains(block))
                    && let Some(user_value) = cx.function.instruction(poison_user).result
                    && known_poison.insert(Operand::Value(user_value))
                {
                    worklist.push(poison_user);
                }
            }
        }

        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5706
    /// `getNoWrapFlagsForGEP`.
    fn get_no_wrap_flags_for_gep(
        &mut self,
        cx: &Cx,
        gep: InstId,
        accum: Scev,
    ) -> u8 {
        let mut f = flags::NONE;
        let nw = GepFlags::of(cx.function.instruction(gep).flags);
        // If the increment has any nowrap flags, then we know the address space
        // cannot be wrapped around.
        if nw.nusw || nw.nuw {
            f |= flags::NW;
        }
        // If the GEP is nuw or nusw with non-negative offset, we know that no
        // unsigned wrap occurs. We cannot set the nsw flag as only the offset
        // is treated as signed, while the base is unsigned.
        if nw.nuw || (nw.nusw && self.is_known_non_negative(cx, accum)) {
            f |= flags::NUW;
        }
        f
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5730
    /// `createSimpleAffineAddRec`: the add recurrence for the simplest (yet
    /// most common) cases, PN = PHI(Start, OP(Self, LoopInvariant)). If it
    /// fails, createAddRecFromPHI will use a more general, but slow, technique
    /// for finding the AddRec expression.
    fn create_simple_affine_add_rec(
        &mut self,
        cx: &Cx,
        pn: InstId,
        be_value_v: Operand,
        start_value_v: Operand,
    ) -> Option<Scev> {
        let header = cx.function.parent(pn).expect("a phi in a block");
        let loop_ = super::disposition::loop_of(cx, header);
        let pn_value = self.inst_value(cx, pn);

        let accum: Scev;
        let mut f = flags::NONE;
        if let Some(bo) = self.match_binary_op(cx, Opnd::V(be_value_v)) {
            if bo.opcode != Bop::Add {
                return None;
            }

            let invariant = |this: &Self, v: Opnd| match v {
                Opnd::V(op) => this.value_is_loop_invariant(cx, op, &loop_),
                Opnd::Lit(_) => true,
            };
            if bo.lhs == Opnd::V(pn_value) && invariant(self, bo.rhs) {
                accum = self.get_scev_opnd(cx, bo.rhs);
            } else if bo.rhs == Opnd::V(pn_value) && invariant(self, bo.lhs) {
                accum = self.get_scev_opnd(cx, bo.lhs);
            } else {
                return None;
            }

            if bo.is_nuw {
                f |= flags::NUW;
            }
            if bo.is_nsw {
                f |= flags::NSW;
            }
        } else {
            // Handle pointer induction variable: PN = PHI(Start, gep PN,
            // LoopInvariant).
            let gep = self.defining_inst(cx, be_value_v)?;
            let gep_instruction = cx.function.instruction(gep);
            let Opcode::GetElementPtr { source } = gep_instruction.opcode else { return None };
            if gep_instruction.operands[0] != pn_value || gep_instruction.operands.len() != 2 {
                return None;
            }
            let idx = gep_instruction.operands[1];
            if !self.value_is_loop_invariant(cx, idx, &loop_) {
                return None;
            }

            let gep_ty = self.operand_ty(cx, be_value_v);
            let int_idx_ty = self.get_effective_scev_type(cx, gep_ty);
            let idx_scev = self.get_scev(cx, idx);
            let idx_scev = self.get_truncate_or_sign_extend(cx, idx_scev, int_idx_ty, 0);
            let size = self.get_size_of_expr(cx, int_idx_ty, source);
            accum = self.get_mul_expr(cx, vec![Use::of(idx_scev), Use::of(size)], Flags::default(), 0).scev;
            f = self.get_no_wrap_flags_for_gep(cx, gep, accum);
        }

        let start_val = self.get_scev(cx, start_value_v);
        let phi_scev = self.get_add_rec_expr_two(cx, Use::of(start_val), Use::of(accum), header, Flags::expr(f)).scev;
        self.insert_value_to_map(pn_value, phi_scev);

        if matches!(self.node(phi_scev).kind, Kind::AddRec(..)) {
            self.infer_no_wrap_via_constant_ranges(cx, phi_scev);
        }

        // We can add Flags to the post-inc expression only if we know that it
        // is *undefined behavior* for BEValueV to overflow.
        if let Some(be_inst) = self.defining_inst(cx, be_value_v) {
            debug_assert!(
                self.is_loop_invariant(cx, accum, Some(header)),
                "Accum is defined outside L, but is not invariant?"
            );
            if self.is_add_rec_never_poison(cx, be_inst, header) {
                let sum = self.get_add_expr(cx, vec![Use::of(start_val), Use::of(accum)], Flags::default(), 0);
                let _ = self.get_add_rec_expr_two(cx, sum, Use::of(accum), header, Flags::expr(f));
            }
        }

        Some(phi_scev)
    }

    /// `L->isLoopInvariant(Value *)`: not defined by an instruction of the
    /// loop.
    pub(super) fn value_is_loop_invariant(
        &self,
        cx: &Cx,
        v: Operand,
        loop_: &llrm_mir::loopinfo::Loop,
    ) -> bool {
        match self.defining_inst(cx, v) {
            Some(inst) => !loop_.contains_instruction(cx.function, inst),
            None => true,
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5791
    /// `createAddRecFromPHI`.
    pub(super) fn create_add_rec_from_phi(
        &mut self,
        cx: &Cx,
        pn: InstId,
    ) -> Option<Scev> {
        let parent = cx.function.parent(pn)?;
        let loop_ = cx.loops.get_loop_for(parent)?;
        if loop_.get_header() != parent {
            return None;
        }
        let header = parent;
        let pn_value = self.inst_value(cx, pn);

        // The loop may have multiple entrances or multiple exits; we can
        // analyze this phi as an addrec if it has a unique entry value and a
        // unique backedge value.
        let mut be_value_v: Option<Operand> = None;
        let mut start_value_v: Option<Operand> = None;
        for pair in cx.function.instruction(pn).operands.chunks(2) {
            let v = pair[0];
            let Operand::Block(block) = pair[1] else { unreachable!("a phi's block") };
            if loop_.contains(block) {
                match be_value_v {
                    None => be_value_v = Some(v),
                    Some(be) if be != v => {
                        be_value_v = None;
                        break;
                    }
                    Some(_) => {}
                }
            } else {
                match start_value_v {
                    None => start_value_v = Some(v),
                    Some(start) if start != v => {
                        start_value_v = None;
                        break;
                    }
                    Some(_) => {}
                }
            }
        }
        let (be_value_v, start_value_v) = (be_value_v?, start_value_v?);

        debug_assert!(self.get_existing_scev(pn_value).is_none(), "PHI node already processed?");

        // First, try to find AddRec expression without creating a fictituos
        // symbolic value for PN.
        if let Some(s) = self.create_simple_affine_add_rec(cx, pn, be_value_v, start_value_v) {
            return Some(s);
        }

        // Handle PHI node value symbolically.
        let symbolic_name = self.get_unknown(cx, pn_value);
        self.insert_value_to_map(pn_value, symbolic_name);

        // Using this symbolic name for the PHI, analyze the value coming
        // around the back-edge.
        let be_value = self.get_scev(cx, be_value_v);

        // NOTE: If BEValue is loop invariant, we know that the PHI node just
        // has a special value for the first iteration of the loop.

        // If the value coming around the backedge is an add with the symbolic
        // value we just inserted, then we found a simple induction variable!
        if let Kind::Add(add_ops) = self.node(be_value).kind.clone() {
            // If there is a single occurrence of the symbolic value, replace
            // it with a recurrence.
            let found_index = add_ops.iter().position(|op| op.scev == symbolic_name);

            if let Some(found_index) = found_index {
                // Create an add with everything but the specified operand.
                let mut ops: Vec<Use> = Vec::new();
                for (i, op) in add_ops.iter().enumerate() {
                    if i != found_index {
                        ops.push(Use::of(self.backedge_condition_fold(cx, op.scev, header)));
                    }
                }
                let accum = self.get_add_expr(cx, ops, Flags::default(), 0).scev;

                // This is not a valid addrec if the step amount is varying
                // each loop iteration, but is not itself an addrec in this
                // loop.
                if self.is_loop_invariant(cx, accum, Some(header))
                    || matches!(
                        self.node(accum).kind,
                        Kind::AddRec(_, accum_loop) if accum_loop == header
                    )
                {
                    let mut f = flags::NONE;

                    if let Some(bo) = self.match_binary_op(cx, Opnd::V(be_value_v)) {
                        if bo.opcode == Bop::Add && bo.lhs == Opnd::V(pn_value) {
                            if bo.is_nuw {
                                f |= flags::NUW;
                            }
                            if bo.is_nsw {
                                f |= flags::NSW;
                            }
                        }
                    } else if let Some(gep) = self.defining_inst(cx, be_value_v)
                        && matches!(
                            cx.function.instruction(gep).opcode,
                            Opcode::GetElementPtr { .. }
                        )
                    {
                        if cx.function.instruction(gep).operands[0] == pn_value {
                            f = self.get_no_wrap_flags_for_gep(cx, gep, accum);
                        }

                        // We cannot transfer nuw and nsw flags from
                        // subtraction operations -- sub nuw X, Y is not the
                        // same as add nuw X, -Y for instance.
                    }

                    let start_val = self.get_scev(cx, start_value_v);
                    let phi_scev =
                        self.get_add_rec_expr_two(cx, Use::of(start_val), Use::of(accum), header, Flags::expr(f)).scev;

                    // Okay, for the entire analysis of this edge we assumed
                    // the PHI to be symbolic. We now need to go back and purge
                    // all of the entries for the scalars that use the symbolic
                    // expression.
                    self.forget_memoized_results(&[symbolic_name]);
                    self.insert_value_to_map(pn_value, phi_scev);

                    if matches!(self.node(phi_scev).kind, Kind::AddRec(..)) {
                        self.infer_no_wrap_via_constant_ranges(cx, phi_scev);
                    }

                    // We can add Flags to the post-inc expression only if we
                    // know that it is *undefined behavior* for BEValueV to
                    // overflow.
                    if let Some(be_inst) = self.defining_inst(cx, be_value_v)
                        && self.is_loop_invariant(cx, accum, Some(header))
                        && self.is_add_rec_never_poison(cx, be_inst, header)
                    {
                        let sum = self.get_add_expr(cx, vec![Use::of(start_val), Use::of(accum)], Flags::default(), 0);
                        let _ = self.get_add_rec_expr_two(cx, sum, Use::of(accum), header, Flags::expr(f));
                    }

                    return Some(phi_scev);
                }
            }
        } else {
            // Otherwise, this could be a loop like this:
            //     i = 0;  for (j = 1; ..; ++j) { ....  i = j; }
            // In this case, j = {1,+,1}  and BEValue is j.
            // Because the other in-value of i (0) fits the evolution of
            // BEValue i really is an addrec evolution.
            //
            // We can generalize this saying that i is the shifted value of
            // BEValue by one iteration:
            //   PHI(f(0), f({1,+,1})) --> f({0,+,1})

            // Do not allow refinement in rewriting of BEValue.
            let shifted = self.shift_rewrite(cx, be_value, header);
            // LLVM rewrites `Shifted` with the init rewriter even when it is
            // could-not-compute, which the visitor cannot visit; the result is
            // then unused, so here it is could-not-compute as well.
            let cnc = self.get_could_not_compute();
            let start = if shifted == cnc { cnc } else { self.init_rewrite(cx, shifted, header, false) };
            if shifted != cnc
                && start != cnc
                && self.is_guaranteed_not_to_cause_ub(cx, shifted)
                && self.implies_poison(cx, shifted, start)
            {
                let start_val = self.get_scev(cx, start_value_v);
                if start == start_val {
                    // Okay, for the entire analysis of this edge we assumed
                    // the PHI to be symbolic. We now need to go back and purge
                    // all of the entries for the scalars that use the symbolic
                    // expression.
                    self.forget_memoized_results(&[symbolic_name]);
                    self.insert_value_to_map(pn_value, shifted);
                    return Some(shifted);
                }
            }
        }

        // Remove the temporary PHI node SCEV that has been inserted while
        // intending to create an AddRecExpr for this PHI node. We can not keep
        // this temporary as it will prevent later (possibly simpler) SCEV
        // expressions to be added to the ValueExprMap.
        self.erase_value_from_map(pn_value);

        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4868
    /// `SCEVInitRewriter::rewrite`: the start value of every add recurrence of
    /// loop `l` in `s`; could-not-compute if it has an unknown that varies in
    /// the loop, or (unless `ignore_other_loops`) the recurrence of another.
    pub(super) fn init_rewrite(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
        ignore_other_loops: bool,
    ) -> Scev {
        let (mut seen_loop_variant_unknown, mut seen_other_loops) = (false, false);
        let result =
            self.init_visit(cx, s, l, &mut seen_loop_variant_unknown, &mut seen_other_loops, &mut HashMap::default());
        if seen_loop_variant_unknown {
            return self.get_could_not_compute();
        }
        if seen_other_loops && !ignore_other_loops { self.get_could_not_compute() } else { result }
    }

    fn init_visit(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
        variant_unknown: &mut bool,
        other_loops: &mut bool,
        memo: &mut HashMap<Scev, Scev>,
    ) -> Scev {
        // `SCEVRewriteVisitor::visit` remembers each result.
        if let Some(&done) = memo.get(&s) {
            return done;
        }
        let result = match self.node(s).kind.clone() {
            Kind::Unknown(_) => {
                if !self.is_loop_invariant(cx, s, Some(l)) {
                    *variant_unknown = true;
                }
                s
            }
            Kind::AddRec(ops, header) => {
                // Only re-write AddRecExprs for this loop.
                if header == l {
                    return ops[0].scev;
                }
                *other_loops = true;
                s
            }
            _ => {
                let mut recurse =
                    |se: &mut Self, op: Scev| se.init_visit(cx, op, l, variant_unknown, other_loops, memo);
                self.rewrite_operands(cx, s, &mut recurse)
            }
        };
        memo.insert(s, result);
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4912
    /// `SCEVPostIncRewriter::rewrite`: `s` with the post-increment form of each
    /// add recurrence of loop `l`; could-not-compute if it has an unknown that
    /// varies in the loop.
    pub(super) fn post_inc_rewrite(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
    ) -> Scev {
        let mut seen_loop_variant_unknown = false;
        let result = self.post_inc_visit(cx, s, l, &mut seen_loop_variant_unknown, &mut HashMap::default());
        if seen_loop_variant_unknown { self.get_could_not_compute() } else { result }
    }

    fn post_inc_visit(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
        variant_unknown: &mut bool,
        memo: &mut HashMap<Scev, Scev>,
    ) -> Scev {
        if let Some(&done) = memo.get(&s) {
            return done;
        }
        let result = match self.node(s).kind.clone() {
            Kind::Unknown(_) => {
                if !self.is_loop_invariant(cx, s, Some(l)) {
                    *variant_unknown = true;
                }
                s
            }
            // Only re-write AddRecExprs for this loop.
            Kind::AddRec(_, header) if header == l => self.get_post_inc_expr(cx, s),
            Kind::AddRec(..) => s,
            _ => {
                let mut recurse = |se: &mut Self, op: Scev| se.post_inc_visit(cx, op, l, variant_unknown, memo);
                self.rewrite_operands(cx, s, &mut recurse)
            }
        };
        memo.insert(s, result);
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L11383
    /// `SplitIntoInitAndPostInc`: `s` on entry to loop `l` and its
    /// post-increment form; both could-not-compute if it has an unknown that
    /// varies in the loop.
    pub fn split_into_init_and_post_inc(
        &mut self,
        cx: &Cx,
        l: BlockId,
        s: Scev,
    ) -> (Scev, Scev) {
        // Compute SCEV on entry of loop L.
        let start = self.init_rewrite(cx, s, l, true);
        if start == self.get_could_not_compute() {
            return (start, start);
        }
        // Compute post increment SCEV for loop L.
        let post_inc = self.post_inc_rewrite(cx, s, l);
        debug_assert!(post_inc != self.get_could_not_compute(), "Unexpected could not compute");
        (start, post_inc)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5028
    /// `SCEVShiftRewriter::rewrite`: `s` with every add recurrence of loop
    /// `l` one iteration earlier; could-not-compute if anything in it is not
    /// invariant or such a recurrence.
    pub(super) fn shift_rewrite(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
    ) -> Scev {
        let mut valid = true;
        let result = self.shift_visit(cx, s, l, &mut valid, &mut HashMap::default());
        if valid { result } else { self.get_could_not_compute() }
    }

    fn shift_visit(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
        valid: &mut bool,
        memo: &mut HashMap<Scev, Scev>,
    ) -> Scev {
        if let Some(&done) = memo.get(&s) {
            return done;
        }
        let result = match self.node(s).kind.clone() {
            Kind::Unknown(_) => {
                // Only allow AddRecExprs for this loop.
                if !self.is_loop_invariant(cx, s, Some(l)) {
                    *valid = false;
                }
                s
            }
            Kind::AddRec(_, header) => {
                if header == l && self.is_affine(s) {
                    let step = self.get_step_recurrence(cx, s);
                    return self.get_minus_scev(cx, Use::of(s), Use::of(step), flags::NONE, 0);
                }
                *valid = false;
                s
            }
            _ => {
                let mut recurse = |se: &mut Self, op: Scev| se.shift_visit(cx, op, l, valid, memo);
                self.rewrite_operands(cx, s, &mut recurse)
            }
        };
        memo.insert(s, result);
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4952
    /// `SCEVBackedgeConditionFolder::rewrite`: `s` with the condition of the
    /// latch's branch taken as it is on the backedge.
    pub(super) fn backedge_condition_fold(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
    ) -> Scev {
        let loop_ = super::disposition::loop_of(cx, l);
        let mut be_cond: Option<Operand> = None;
        let mut is_pos_be_cond = false;
        if let Some(latch) = loop_.get_loop_latch(cx.function) {
            let terminator = cx.function.terminator(latch).map(|t| cx.function.instruction(t));
            match terminator {
                Some(branch) if branch.opcode == Opcode::Br && branch.operands.len() == 3 => {
                    debug_assert!(
                        branch.operands[1] != branch.operands[2],
                        "Both outgoing branches should not target same header!"
                    );
                    be_cond = Some(branch.operands[0]);
                    is_pos_be_cond = branch.operands[1] == Operand::Block(l);
                }
                _ => return s,
            }
        }
        self.bcf_visit(cx, s, l, be_cond, is_pos_be_cond, &mut HashMap::default())
    }

    fn bcf_visit(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
        be_cond: Option<Operand>,
        is_pos_be_cond: bool,
        memo: &mut HashMap<Scev, Scev>,
    ) -> Scev {
        if let Some(&done) = memo.get(&s) {
            return done;
        }
        let result = self.bcf_visit_once(cx, s, l, be_cond, is_pos_be_cond, memo);
        memo.insert(s, result);
        result
    }

    fn bcf_visit_once(
        &mut self,
        cx: &Cx,
        s: Scev,
        l: BlockId,
        be_cond: Option<Operand>,
        is_pos_be_cond: bool,
        memo: &mut HashMap<Scev, Scev>,
    ) -> Scev {
        let Kind::Unknown(value) = self.node(s).kind else {
            let mut recurse = |se: &mut Self, op: Scev| se.bcf_visit(cx, op, l, be_cond, is_pos_be_cond, memo);
            return self.rewrite_operands(cx, s, &mut recurse);
        };
        let mut result = s;
        let invariant_f = self.is_loop_invariant(cx, s, Some(l));

        // `compareWithBackedgeCondition`: if value matches the backedge
        // condition for loop latch, then return a constant evolution node
        // based on loopback branch taken.
        let compare_with_backedge_condition = |this: &mut Self, ic: Operand| -> Option<Scev> {
            if be_cond == Some(ic) {
                let i1 = Ty::Int(1);
                return Some(if is_pos_be_cond { this.get_one(cx, i1) } else { this.get_zero(cx, i1) });
            }
            None
        };

        if !invariant_f {
            let inst = self.defining_inst(cx, value).expect("a variant unknown is an instruction");
            let instruction = cx.function.instruction(inst);
            match instruction.opcode {
                Opcode::Select => {
                    if let Some(res) = compare_with_backedge_condition(self, instruction.operands[0]) {
                        let is_one = self.as_constant(res).expect("a constant").is_one();
                        result =
                            self.get_scev(cx, if is_one { instruction.operands[1] } else { instruction.operands[2] });
                    }
                }
                _ => {
                    if let Some(res) = compare_with_backedge_condition(self, value) {
                        result = res;
                    }
                }
            }
        }
        result
    }
}
