//! `createSCEV` for the PHI, select and `getelementptr` forms: add recurrences
//! from loop-header PHIs (`createAddRecFromPHI`), selects as min and max, and
//! pointer arithmetic (`getGEPExpr`).
//!
//! # DEVIATE 1 (no predicated SCEV)
//! `createAddRecFromPHIWithCasts` and the predicated rewrites are not
//! translated.

use llrm_mir::context::{ConstantExpr, ConstantKind};
use llrm_mir::hash::HashSet;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{Flags as IrFlags, IntPredicate, Opcode};
use llrm_mir::types::Type as MirType;

use super::ap::Ap;
use super::create::Opnd;
use super::expr::{Kind, Scev, Ty, Type, Use, flags};
use super::{Cx, Flags, ScalarEvolution};

/// The PHI's incoming value and block pairs.
fn incoming(
    cx: &Cx,
    pn: InstId,
) -> Vec<(Operand, BlockId)> {
    cx.function
        .instruction(pn)
        .operands
        .chunks(2)
        .map(|pair| {
            let Operand::Block(block) = pair[1] else { unreachable!("a phi's block") };
            (pair[0], block)
        })
        .collect()
}

impl ScalarEvolution {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5945
    /// `BrPHIToSelect`: whether the branch that ends `idom` and `merge` form
    /// `C ? LHS : RHS`; the condition and the two hands.
    fn br_phi_to_select(
        &self,
        cx: &Cx,
        br: InstId,
        merge: InstId,
    ) -> Option<(Operand, Operand, Operand)> {
        let instruction = cx.function.instruction(br);
        if instruction.opcode != Opcode::Br || instruction.operands.len() != 3 {
            return None;
        }
        let c = instruction.operands[0];
        let (Operand::Block(left), Operand::Block(right)) = (instruction.operands[1], instruction.operands[2]) else {
            return None;
        };
        let from = cx.function.parent(br)?;

        let left_use = llrm_mir::module::Use { user: merge, index: 0 };
        let right_use = llrm_mir::module::Use { user: merge, index: 2 };
        let merge_operands = &cx.function.instruction(merge).operands;
        let dominates = |end: BlockId, used: &llrm_mir::module::Use| {
            llrm_mir::valuetracking::range::edge_dominates_use(cx.function, cx.tree, from, end, used)
        };

        if dominates(left, &left_use) && dominates(right, &right_use) {
            return Some((c, merge_operands[0], merge_operands[2]));
        }

        if dominates(left, &right_use) && dominates(right, &left_use) {
            return Some((c, merge_operands[2], merge_operands[0]));
        }

        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5970
    /// `getOperandsForSelectLikePHI`: the condition and hands of a PHI that is
    /// a select.
    pub(super) fn get_operands_for_select_like_phi(
        &self,
        cx: &Cx,
        pn: InstId,
    ) -> Option<(Operand, Operand, Operand)> {
        let pairs = incoming(cx, pn);
        if pairs.len() == 2 && pairs.iter().all(|(_, block)| cx.tree.is_reachable(*block)) {
            // Try to match
            //
            //  br %cond, label %left, label %right
            // left:
            //  br label %merge
            // right:
            //  br label %merge
            // merge:
            //  V = phi [ %x, %left ], [ %y, %right ]
            //
            // as "select %cond, %x, %y"
            let parent = cx.function.parent(pn)?;
            let idom = cx.tree.immediate_dominator(parent).expect("At least the entry block should dominate PN");
            let br = cx.function.terminator(idom)?;
            return self.br_phi_to_select(cx, br, pn);
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6007
    /// `getCommonInstForPHI`: the binary operator all of the PHI's incoming
    /// values are copies of.
    pub(super) fn get_common_inst_for_phi(
        &self,
        cx: &Cx,
        pn: InstId,
    ) -> Option<InstId> {
        let mut common_inst: Option<InstId> = None;
        // Check if instructions are identical.
        for (value, _) in incoming(cx, pn) {
            let incoming_inst = self.defining_inst(cx, value)?;
            if !matches!(
                cx.function.instruction(incoming_inst).opcode,
                Opcode::Binary(_)
            ) {
                return None;
            }
            match common_inst {
                Some(common) => {
                    // `isIdenticalToWhenDefined`.
                    let (a, b) = (cx.function.instruction(common), cx.function.instruction(incoming_inst));
                    if a.opcode != b.opcode || a.ty != b.ty || a.operands != b.operands {
                        return None; // Not identical, give up
                    }
                }
                None => {
                    // Remember binary operator
                    common_inst = Some(incoming_inst);
                }
            }
        }
        common_inst
    }

    /// `RangeRefPHIAllowedOperands`.
    pub(super) fn range_ref_phi_allowed_operands(
        &self,
        cx: &Cx,
        pn: InstId,
    ) -> bool {
        if self.get_operands_for_select_like_phi(cx, pn).is_some() {
            return true;
        }

        incoming(cx, pn)
            .iter()
            .all(
                |(value, _)| match self.defining_inst(cx, *value) {
                    Some(def) => cx.tree.instruction_dominates(cx.function, def, pn),
                    None => true,
                },
            )
    }

    /// `simplifyInstruction` of a PHI, with `CanUseUndef` false: the value
    /// the PHI is, when all of its incoming values are one (`simplifyPHINode`).
    fn simplify_phi_value(
        &self,
        cx: &Cx,
        pn: InstId,
    ) -> Option<Operand> {
        let own = self.inst_value(cx, pn);
        let mut common_value: Option<Operand> = None;
        let mut has_poison_input = false;
        let mut poison: Option<Operand> = None;
        for (value, _) in incoming(cx, pn) {
            // If the incoming value is the phi node itself, it can safely be
            // skipped.
            if value == own {
                continue;
            }
            if matches!(
                value,
                Operand::Constant(id) if matches!(cx.context.get(id).kind, llrm_mir::context::ConstantKind::Poison)
            ) {
                has_poison_input = true;
                poison = Some(value);
                continue;
            }
            if common_value.is_some_and(|common| common != value) {
                return None; // Not the same, bail out.
            }
            common_value = Some(value);
        }

        // If CommonValue is null then all of the incoming values were either
        // undef, poison or equal to the phi node itself.
        let Some(common_value) = common_value else {
            // `PoisonValue::get(PN->getType())`: a poison incoming value, or
            // none (DEVIATE 9) and the PHI stays.
            return poison;
        };

        if has_poison_input {
            // If we have a PHI node like phi(X, undef, X), where X is defined
            // by some instruction, we cannot return X as the result of the PHI
            // node unless it dominates the PHI block.
            // `valueDominatesPHI`: an instruction of the PHI's own block
            // dominates it only when it comes first.
            return match self.defining_inst(cx, common_value) {
                Some(def) => cx.tree.instruction_dominates(cx.function, def, pn).then_some(common_value),
                None => Some(common_value),
            };
        }
        Some(common_value)
    }

    /// The `case Instruction::PHI` of `getOperandsToCreate`.
    pub(super) fn get_operands_to_create_phi(
        &mut self,
        cx: &Cx,
        pn: InstId,
        ops: &mut Vec<Operand>,
    ) -> Option<Scev> {
        // getNodeForPHI has four ways to turn a PHI into a SCEV; retrieve the
        // relevant nodes for each of them.
        //
        // The first is just to call simplifyInstruction, and get something
        // back that isn't a PHI.
        if let Some(v) = self.simplify_phi_value(cx, pn) {
            ops.push(v);
            return None;
        }
        // The second is createNodeForPHIWithIdenticalOperands: this looks for
        // operands which all perform the same operation, but haven't been
        // CSE'ed for whatever reason.
        if let Some(bo) = self.get_common_inst_for_phi(cx, pn) {
            ops.push(self.inst_value(cx, bo));
            return None;
        }
        // The third is createNodeFromSelectLikePHI; this takes a PHI which is
        // equivalent to a select, and analyzes it like a select.
        if let Some((cond, lhs, rhs)) = self.get_operands_for_select_like_phi(cx, pn) {
            if let Some(cond_icmp) = self.defining_inst(cx, cond)
                && matches!(cx.function.instruction(cond_icmp).opcode, Opcode::ICmp(_))
            {
                ops.push(cx.function.instruction(cond_icmp).operands[0]);
                ops.push(cx.function.instruction(cond_icmp).operands[1]);
            }
            ops.push(cond);
            ops.push(lhs);
            ops.push(rhs);
            return None;
        }
        // The fourth way is createAddRecFromPHI. It's complicated to handle
        // here, so just construct it recursively.
        //
        // In addition to getNodeForPHI, also construct nodes which might be
        // needed by getRangeRef.
        if self.range_ref_phi_allowed_operands(cx, pn) {
            let operands = cx.function.instruction(pn).operands.clone();
            self.push_value_operands(cx, ops, &operands);
            return None;
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6048
    /// `createNodeForPHI`.
    pub(super) fn create_node_for_phi(
        &mut self,
        cx: &Cx,
        pn: InstId,
    ) -> Scev {
        if let Some(s) = self.create_add_rec_from_phi(cx, pn) {
            return s;
        }

        // We do not allow simplifying phi (undef, X) to X here, to avoid
        // reusing the phi node for X.
        if let Some(v) = self.simplify_phi_value(cx, pn) {
            return self.get_scev(cx, v);
        }

        if let Some(s) = self.create_node_for_phi_with_identical_operands(cx, pn) {
            return s;
        }

        if let Some(s) = self.create_node_from_select_like_phi(cx, pn) {
            return s;
        }

        // If it's not a loop phi, we can't handle it yet.
        let own = self.inst_value(cx, pn);
        self.get_unknown(cx, own)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6035
    /// `createNodeForPHIWithIdenticalOperands`: the SCEV of the first operand
    /// of a PHI if all its operands have identical opcodes and operands.
    /// e.g.
    /// a: %add = %a + %b
    ///    br %c
    /// b: %add1 = %a + %b
    ///    br %c
    /// c: %phi = phi [%add, a], [%add1, b]
    /// scev(%phi) => scev(%add)
    fn create_node_for_phi_with_identical_operands(
        &mut self,
        cx: &Cx,
        pn: InstId,
    ) -> Option<Scev> {
        let common_inst = self.get_common_inst_for_phi(cx, pn)?;

        // Check if SCEV exprs for instructions are identical.
        let common_value = self.inst_value(cx, common_inst);
        let common_scev = self.get_scev(cx, common_value);
        let pairs = incoming(cx, pn);
        let identical = pairs.iter().skip(1).all(|(value, _)| common_scev == self.get_scev(cx, *value));
        identical.then_some(common_scev)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5997
    /// `createNodeFromSelectLikePHI`.
    fn create_node_from_select_like_phi(
        &mut self,
        cx: &Cx,
        pn: InstId,
    ) -> Option<Scev> {
        let (cond, lhs, rhs) = self.get_operands_for_select_like_phi(cx, pn)?;
        let parent = cx.function.parent(pn).expect("a phi in a block");
        let lhs_scev = self.get_scev(cx, lhs);
        if !self.properly_dominates(cx, lhs_scev, parent) {
            return None;
        }
        let rhs_scev = self.get_scev(cx, rhs);
        if !self.properly_dominates(cx, rhs_scev, parent) {
            return None;
        }
        let own = self.inst_value(cx, pn);
        Some(self.create_node_for_select_or_phi(cx, own, cond, lhs, rhs))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6275
    /// `createNodeForSelectOrPHI`.
    pub(super) fn create_node_for_select_or_phi(
        &mut self,
        cx: &Cx,
        v: Operand,
        cond: Operand,
        true_val: Operand,
        false_val: Operand,
    ) -> Scev {
        // Handle "constant" branch or select. This can occur for instance when
        // a loop pass transforms an inner loop and moves on to process the
        // outer loop.
        if let Some(ci) = self.const_int(cx, Opnd::V(cond)) {
            return self.get_scev(cx, if ci.is_one() { true_val } else { false_val });
        }

        if let Some(inst) = self.defining_inst(cx, v)
            && let Some(icmp) = self.defining_inst(cx, cond)
            && matches!(cx.function.instruction(icmp).opcode, Opcode::ICmp(_))
        {
            let ty = self.ty_of(cx, cx.function.instruction(inst).ty).expect("a SCEVable select");
            if let Some(s) =
                self.create_node_for_select_or_phi_inst_with_icmp_inst_cond(cx, ty, icmp, true_val, false_val)
            {
                return s;
            }
        }

        self.create_node_for_select_or_phi_via_umin_seq(cx, v, cond, true_val, false_val)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6257
    /// `createNodeForSelectOrPHIViaUMinSeq`.
    fn create_node_for_select_or_phi_via_umin_seq(
        &mut self,
        cx: &Cx,
        v: Operand,
        cond: Operand,
        true_val: Operand,
        false_val: Operand,
    ) -> Scev {
        debug_assert!(self.operand_ty(cx, cond) == Ty::Int(1), "Select condition is not an i1?");

        // For now, only deal with i1-typed `select`s.
        if self.operand_ty(cx, v) != Ty::Int(1) {
            return self.get_unknown(cx, v);
        }

        if let Some(s) = self.create_node_for_select_via_umin_seq_values(cx, cond, true_val, false_val) {
            return s;
        }

        self.get_unknown(cx, v)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6246
    /// `createNodeForSelectViaUMinSeq(SE, Cond, TrueVal, FalseVal)`.
    fn create_node_for_select_via_umin_seq_values(
        &mut self,
        cx: &Cx,
        cond: Operand,
        true_val: Operand,
        false_val: Operand,
    ) -> Option<Scev> {
        if self.const_int(cx, Opnd::V(true_val)).is_none() && self.const_int(cx, Opnd::V(false_val)).is_none() {
            return None;
        }

        let se_cond = self.get_scev(cx, cond);
        let se_true = self.get_scev(cx, true_val);
        let se_false = self.get_scev(cx, false_val);
        self.create_node_for_select_via_umin_seq(cx, se_cond, se_true, se_false)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6213
    /// `createNodeForSelectViaUMinSeq(SE, CondExpr, TrueExpr, FalseExpr)`.
    fn create_node_for_select_via_umin_seq(
        &mut self,
        cx: &Cx,
        mut cond_expr: Scev,
        true_expr: Scev,
        false_expr: Scev,
    ) -> Option<Scev> {
        debug_assert!(
            self.type_of(cond_expr) == Ty::Int(1)
                && self.type_of(true_expr) == self.type_of(false_expr)
                && self.type_of(true_expr) == Ty::Int(1),
            "Unexpected operands of a select."
        );

        // i1 cond ? i1 x : i1 C  -->  C + (i1  cond ? (i1 x - i1 C) : i1 0)
        //                        -->  C + (umin_seq  cond, x - C)
        //
        // i1 cond ? i1 C : i1 x  -->  C + (i1  cond ? i1 0 : (i1 x - i1 C))
        //                        -->  C + (i1 ~cond ? (i1 x - i1 C) : i1 0)
        //                        -->  C + (umin_seq ~cond, x - C)

        // FIXME: while we can't legally model the case where both of the hands
        // are fully variable, we only require that the *difference* is
        // constant.
        let (true_const, false_const) = (self.as_constant(true_expr).is_some(), self.as_constant(false_expr).is_some());
        if !true_const && !false_const {
            return None;
        }

        let (x, c);
        if true_const {
            cond_expr = self.get_not_scev(cx, cond_expr);
            x = false_expr;
            c = true_expr;
        } else {
            x = true_expr;
            c = false_expr;
        }
        let diff = self.get_minus_scev(cx, Use::of(x), Use::of(c), flags::NONE, 0);
        let umin = self.get_umin_expr(cx, vec![Use::of(cond_expr), Use::of(diff)], true);
        Some(self.get_add_expr(cx, vec![Use::of(c), Use::of(umin)], Flags::default(), 0).scev)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6069
    /// `SCEVMinMaxExprContains`.
    fn scev_min_max_expr_contains(
        &self,
        root: Scev,
        operand_to_find: Scev,
        root_kind: Type,
    ) -> bool {
        debug_assert!(root_kind == Type::SequentialUMin, "Must be a sequential min/max expression.");
        let non_sequential_root_kind = Type::UMin;
        // We can only recurse into the SCEV expression of the same effective
        // type as the type of our root SCEV expression, and into
        // zero-extensions.
        let can_recurse_into =
            |kind: Type| root_kind == kind || non_sequential_root_kind == kind || Type::ZeroExtend == kind;

        let mut visited = HashSet::default();
        let mut worklist = vec![root];
        visited.insert(root);
        while let Some(s) = worklist.pop() {
            // follow
            if s == operand_to_find {
                return true;
            }
            if !can_recurse_into(self.scev_type(s)) {
                continue;
            }
            for op in self.operands_of(s) {
                if visited.insert(op.scev) {
                    worklist.push(op.scev);
                }
            }
        }
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6106
    /// `createNodeForSelectOrPHIInstWithICmpInstCond`.
    fn create_node_for_select_or_phi_inst_with_icmp_inst_cond(
        &mut self,
        cx: &Cx,
        ty: Ty,
        icmp: InstId,
        mut true_val: Operand,
        mut false_val: Operand,
    ) -> Option<Scev> {
        // Try to match some simple smax or umax patterns.
        let icmp_instruction = cx.function.instruction(icmp);
        let Opcode::ICmp(pred) = icmp_instruction.opcode else { unreachable!("an icmp") };
        let (mut lhs, mut rhs) = (icmp_instruction.operands[0], icmp_instruction.operands[1]);

        let mut pred = pred;
        let mut equality_form = false;
        match pred {
            IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Ult | IntPredicate::Ule => {
                std::mem::swap(&mut lhs, &mut rhs);
            }
            IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Ugt | IntPredicate::Uge => {}
            IntPredicate::Ne => {
                // x != 0 ? x+y : C+y  ->  x == 0 ? C+y : x+y
                std::mem::swap(&mut true_val, &mut false_val);
                pred = IntPredicate::Eq;
                equality_form = true;
            }
            IntPredicate::Eq => equality_form = true,
        }

        if !equality_form {
            // a > b ? a+x : b+x  ->  max(a, b)+x
            // a > b ? b+x : a+x  ->  min(a, b)+x
            if self.get_type_size_in_bits(cx, self.operand_ty(cx, lhs)) <= self.get_type_size_in_bits(cx, ty) {
                let signed =
                    matches!(
                        pred,
                        IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge
                    );
                let la = self.get_scev(cx, true_val);
                let ra = self.get_scev(cx, false_val);
                let mut ls = self.get_scev(cx, lhs);
                let mut rs = self.get_scev(cx, rhs);
                if self.type_of(la).is_pointer() {
                    // FIXME: Handle cases where LS/RS are pointers not equal
                    // to LA/RA. Need to make sure we can't produce weird
                    // expressions involving negated pointers.
                    if la == ls && ra == rs {
                        let ops = vec![Use::of(ls), Use::of(rs)];
                        return Some(if signed { self.get_smax_expr(cx, ops) } else { self.get_umax_expr(cx, ops) });
                    }
                    if la == rs && ra == ls {
                        let ops = vec![Use::of(ls), Use::of(rs)];
                        return Some(if signed {
                            self.get_smin_expr(cx, ops)
                        } else {
                            self.get_umin_expr(cx, ops, false)
                        });
                    }
                }
                let coerce_operand = |this: &mut Self, mut op: Scev| -> Scev {
                    if this.type_of(op).is_pointer() {
                        op = this.get_ptr_to_addr_expr(cx, op);
                        if matches!(this.node(op).kind, Kind::CouldNotCompute) {
                            return op;
                        }
                    }
                    if signed {
                        this.get_noop_or_sign_extend(cx, op, ty)
                    } else {
                        this.get_noop_or_zero_extend(cx, op, ty)
                    }
                };
                ls = coerce_operand(self, ls);
                rs = coerce_operand(self, rs);
                if matches!(self.node(ls).kind, Kind::CouldNotCompute)
                    || matches!(self.node(rs).kind, Kind::CouldNotCompute)
                {
                    return None;
                }
                let mut l_diff = self.get_minus_scev(cx, Use::of(la), Use::of(ls), flags::NONE, 0);
                let mut r_diff = self.get_minus_scev(cx, Use::of(ra), Use::of(rs), flags::NONE, 0);
                if l_diff == r_diff {
                    let ops = vec![Use::of(ls), Use::of(rs)];
                    let max = if signed { self.get_smax_expr(cx, ops) } else { self.get_umax_expr(cx, ops) };
                    return Some(self.get_add_expr(cx, vec![Use::of(max), Use::of(l_diff)], Flags::default(), 0).scev);
                }
                l_diff = self.get_minus_scev(cx, Use::of(la), Use::of(rs), flags::NONE, 0);
                r_diff = self.get_minus_scev(cx, Use::of(ra), Use::of(ls), flags::NONE, 0);
                if l_diff == r_diff {
                    let ops = vec![Use::of(ls), Use::of(rs)];
                    let min = if signed { self.get_smin_expr(cx, ops) } else { self.get_umin_expr(cx, ops, false) };
                    return Some(self.get_add_expr(cx, vec![Use::of(min), Use::of(l_diff)], Flags::default(), 0).scev);
                }
            }
            return None;
        }

        // x == 0 ? C+y : x+y  ->  umax(x, C)+y   iff C u<= 1
        let rhs_is_zero = self.const_int(cx, Opnd::V(rhs)).is_some_and(|c| c.is_zero());
        if self.get_type_size_in_bits(cx, self.operand_ty(cx, lhs)) <= self.get_type_size_in_bits(cx, ty) && rhs_is_zero
        {
            let lhs_scev = self.get_scev(cx, lhs);
            let x = self.get_noop_or_zero_extend(cx, lhs_scev, ty);
            let true_val_expr = self.get_scev(cx, true_val); // C+y
            let false_val_expr = self.get_scev(cx, false_val); // x+y
            // y = (x+y)-x
            let y = self.get_minus_scev(cx, Use::of(false_val_expr), Use::of(x), flags::NONE, 0);
            // C = (C+y)-y
            let c = self.get_minus_scev(cx, Use::of(true_val_expr), Use::of(y), flags::NONE, 0);
            if let Some(c_value) = self.as_constant(c)
                && c_value.ule(Ap::one(c_value.width))
            {
                let umax = self.get_umax_expr(cx, vec![Use::of(x), Use::of(c)]);
                return Some(self.get_add_expr(cx, vec![Use::of(umax), Use::of(y)], Flags::default(), 0).scev);
            }
        }
        // x == 0 ? 0 : umin    (..., x, ...)  ->  umin_seq(x, umin    (...))
        // x == 0 ? 0 : umin_seq(..., x, ...)  ->  umin_seq(x, umin_seq(...))
        // x == 0 ? 0 : umin    (..., umin_seq(..., x, ...), ...)
        //                    ->  umin_seq(x, umin (..., umin_seq(...), ...))
        let true_is_zero = self.const_int(cx, Opnd::V(true_val)).is_some_and(|c| c.is_zero());
        if rhs_is_zero && true_is_zero {
            let mut x = self.get_scev(cx, lhs);
            while let Kind::ZeroExtend(op) = self.node(x).kind {
                x = op.scev;
            }
            if self.get_type_size_in_bits(cx, self.type_of(x)) <= self.get_type_size_in_bits(cx, ty) {
                let false_val_expr = self.get_scev(cx, false_val);
                if self.scev_min_max_expr_contains(false_val_expr, x, Type::SequentialUMin) {
                    let x_ext = self.get_noop_or_zero_extend(cx, x, ty);
                    return Some(self.get_umin_expr(cx, vec![Use::of(x_ext), Use::of(false_val_expr)], true));
                }
            }
        }
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L6297
    /// `createNodeForGEP`: a `getelementptr` as adds and multiplies, which
    /// the regular SCEV code can analyse.
    pub(super) fn create_node_for_gep(
        &mut self,
        cx: &Cx,
        gep: InstId,
    ) -> Scev {
        let instruction = cx.function.instruction(gep);
        let mut index_exprs: Vec<Use> = Vec::new();
        for &index in &instruction.operands[1..] {
            index_exprs.push(Use::of(self.get_scev(cx, index)));
        }
        self.get_gep_expr_of_instruction(cx, gep, &index_exprs)
    }

    /// `createSCEV` of a constant `getelementptr` expression (an `Operator`
    /// at the pin): `getGEPExpr` of its operands. A non-instruction takes no
    /// wrap flags from the IR.
    pub(super) fn create_node_for_constant_gep(
        &mut self,
        cx: &Cx,
        v: Operand,
    ) -> Option<Scev> {
        let Operand::Constant(id) = v else { return None };
        let ConstantKind::Expr(ConstantExpr::GetElementPtr { source, operands, .. }) = &cx.context.get(id).kind else {
            return None;
        };
        let base = self.get_scev(cx, Operand::Constant(operands[0]));
        let indexes: Vec<Use> =
            operands[1..].iter().map(|&one| Use::of(self.get_scev(cx, Operand::Constant(one)))).collect();
        Some(self.get_gep_expr(cx, Use::of(base), &indexes, *source, GepFlags::default()))
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3790
    /// `getGEPExpr(GEPOperator *, IndexExprs)`.
    fn get_gep_expr_of_instruction(
        &mut self,
        cx: &Cx,
        gep: InstId,
        index_exprs: &[Use],
    ) -> Scev {
        let instruction = cx.function.instruction(gep);
        let base_expr = self.get_scev(cx, instruction.operands[0]);
        // getSCEV(Base)->getType() has the same address space as
        // Base->getType() because SCEV::getType() preserves the address space.
        let mut nw = GepFlags::of(instruction.flags);
        if nw.any() {
            // We'd like to propagate flags from the IR to the corresponding
            // SCEV nodes, but to do that, we have to ensure that said flag is
            // valid in the entire defined scope of the SCEV.
            // TODO: non-instructions have global scope.  We might be able to
            // prove some global scope cases
            if !self.is_scev_expr_never_poison(cx, gep) {
                nw = GepFlags::default();
            }
        }

        let Opcode::GetElementPtr { source } = instruction.opcode else { unreachable!("a getelementptr") };
        self.get_gep_expr(cx, Use::of(base_expr), index_exprs, source, nw)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L3810
    /// `getGEPExpr(BaseExpr, IndexExprs, SrcElementTy, NW)`.
    pub(super) fn get_gep_expr(
        &mut self,
        cx: &Cx,
        base_expr: Use,
        index_exprs: &[Use],
        src_element_ty: llrm_mir::types::TypeId,
        nw: GepFlags,
    ) -> Scev {
        let mut offset_wrap = flags::NONE;
        if nw.nusw {
            offset_wrap |= flags::NSW;
        }
        if nw.nuw {
            offset_wrap |= flags::NUW;
        }

        let base_ty = self.type_of(base_expr.scev);
        let int_idx_ty = self.get_effective_scev_type(cx, base_ty);
        // The type being indexed: the pointer at first, then what each index
        // steps into.
        let mut cur_ty: Option<llrm_mir::types::TypeId> = None;
        let mut first_iter = true;
        let mut offsets: Vec<Use> = Vec::new();
        for &index_expr in index_exprs {
            // Compute the (potentially symbolic) offset in bytes for this
            // index.
            let struct_ty =
                cur_ty.filter(|ty| matches!(
                    cx.context.types.get(*ty),
                    MirType::Struct { .. } | MirType::Named(_)
                ));
            if let Some(sty) = struct_ty {
                // For a struct, add the member offset.
                let index = self.as_constant(index_expr.scev).expect("a struct's index is a constant");
                let field_no = index.zext_value() as u64;
                let field_offset = self.get_offset_of_expr(cx, int_idx_ty, sty, field_no);
                offsets.push(Use::of(field_offset));

                // Update CurTy to the type of the field at Index.
                cur_ty = cx.context.types.member(sty, field_no);
            } else {
                // Update CurTy to its element type.
                if first_iter {
                    debug_assert!(base_ty.is_pointer(), "The first index of a GEP indexes a pointer");
                    cur_ty = Some(src_element_ty);
                    first_iter = false;
                } else {
                    // The element type of an array or vector whatever its
                    // length: a `[0 x T]` indexes `T`.
                    let indexed = cur_ty.expect("a type being indexed");
                    cur_ty = match cx.context.types.get(indexed) {
                        MirType::Array { element, .. } => Some(*element),
                        MirType::Vector { element, .. } => Some(*element),
                        _ => cx.context.types.member(indexed, 0),
                    };
                }
                // For an array, add the element offset, explicitly scaled.
                let element_size = self.get_size_of_expr(cx, int_idx_ty, cur_ty.expect("an element type"));
                // Getelementptr indices are signed.
                let index_expr = self.get_truncate_or_sign_extend(cx, index_expr.scev, int_idx_ty, 0);

                // Multiply the index by the element size to compute the
                // element offset.
                let local_offset = self.get_mul_expr(
                    cx,
                    vec![Use::of(index_expr), Use::of(element_size)],
                    Flags::expr(offset_wrap),
                    0,
                );
                offsets.push(local_offset);
            }
        }

        // Handle degenerate case of GEP without offsets.
        if offsets.is_empty() {
            return base_expr.scev;
        }

        // Add the offsets together, assuming nsw if inbounds.
        let offset = self.get_add_expr(cx, offsets, Flags::expr(offset_wrap), 0);
        // Add the base address and the offset. We cannot use the nsw flag, as
        // the base address is unsigned. However, if we know that the offset is
        // non-negative, we can use nuw.
        let nuw = nw.nuw || (nw.nusw && self.is_known_non_negative(cx, offset.scev));
        let base_wrap = if nuw { flags::NUW } else { flags::NONE };
        let gep_expr = self.get_add_expr(cx, vec![base_expr, offset], Flags::expr(base_wrap), 0).scev;
        debug_assert!(self.type_of(base_expr.scev) == self.type_of(gep_expr), "GEP should not change type mid-flight.");
        gep_expr
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4403
    /// `getSizeOfExpr(IntTy, AllocTy)`.
    pub fn get_size_of_expr(
        &mut self,
        cx: &Cx,
        int_ty: Ty,
        alloc_ty: llrm_mir::types::TypeId,
    ) -> Scev {
        let size = cx.layout.alloc_size(&cx.context.types, alloc_ty);
        self.get_constant_of_type(cx, int_ty, size as i128)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4407
    /// `getStoreSizeOfExpr(IntTy, StoreTy)`.
    pub fn get_store_size_of_expr(
        &mut self,
        cx: &Cx,
        int_ty: Ty,
        store_ty: llrm_mir::types::TypeId,
    ) -> Scev {
        let size = cx.layout.store_size(&cx.context.types, store_ty);
        self.get_constant_of_type(cx, int_ty, size as i128)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4411
    /// `getOffsetOfExpr(IntTy, STy, FieldNo)`.
    pub fn get_offset_of_expr(
        &mut self,
        cx: &Cx,
        int_ty: Ty,
        struct_ty: llrm_mir::types::TypeId,
        field_no: u64,
    ) -> Scev {
        let (_, offsets) = cx.layout.struct_layout(&cx.context.types, struct_ty);
        self.get_constant_of_type(cx, int_ty, offsets[field_no as usize] as i128)
    }
}

/// `GEPNoWrapFlags`: whether the GEP is `nusw` (`inbounds` implies it) and
/// `nuw`.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct GepFlags {
    pub nusw: bool,
    pub nuw: bool,
}

impl GepFlags {
    pub(super) fn of(f: IrFlags) -> Self {
        Self { nusw: f.contains(IrFlags::INBOUNDS) || f.contains(IrFlags::NUSW), nuw: f.contains(IrFlags::NUW) }
    }

    /// `NW != GEPNoWrapFlags::none()`.
    fn any(&self) -> bool {
        self.nusw || self.nuw
    }
}
