//! `getSCEV`: the expression of an IR value. `createSCEVIter` walks the
//! operands of a value without recursing, `getOperandsToCreate` says which of
//! them to make first, and `createSCEV` builds the expression of one value
//! from its operands' expressions. `MatchBinaryOp` reads an instruction as the
//! binary operation SCEV models it as.
//!
//! The PHI and select forms are in `create_phi`; the rewriters they use are in
//! `rewriters`.
//!
//! # DEVIATE 9 (MIR lacks the construct)
//! * `llvm.start.loop.iterations`, `llvm.annotation`, `llvm.ptr.annotation`,
//!   `llvm.vscale` and `llvm.loop.decrement.reg` are not MIR intrinsics
//!   (`Intrinsic::named` is none for them, tested), so a call to one is an
//!   unknown, as any call is. (`llvm.usub.sat` and `llvm.uadd.sat` are, and are
//!   translated.)
//! * No separate `ptrtoaddr`: MIR's `ptrtoint` is modelled as one.
//! * Only a constant `getelementptr` expression has a SCEV; any other constant
//!   expression is an unknown.
//! * `PoisonValue::get(Ty)` as the SCEV of an unreachable instruction needs a
//!   constant interned in the context, and the `Cx` holds it by `&`: the
//!   unknown is of the instruction itself.

use llrm_mir::context::ConstantKind;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp as Bin, CastOp, Flags as IrFlags, Opcode};
use llrm_mir::valuetracking::guarantee;

use super::ap::Ap;
use super::expr::{Kind, Scev, Ty, Use, flags};
use super::{Cx, Flags, ScalarEvolution};

/// An operand of a [`BinaryOp`]: a value, or a constant the match makes up
/// (the `1 << n` of an `lshr`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Opnd {
    V(Operand),
    Lit(Ap),
}

/// The binary operations `MatchBinaryOp` yields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Bop {
    Add,
    Sub,
    Mul,
    UDiv,
    URem,
    And,
    Or,
    Xor,
    Shl,
    AShr,
    LShr,
}

/// `BinaryOp`: an abstract binary operation, which may exist as an instruction
/// or be derived from an expression tree.
#[derive(Clone, Copy, Debug)]
pub(super) struct BinaryOp {
    pub opcode: Bop,
    pub lhs: Opnd,
    pub rhs: Opnd,
    pub is_nsw: bool,
    pub is_nuw: bool,
    /// Set if this corresponds to a concrete instruction.
    pub op: Option<InstId>,
}

impl BinaryOp {
    /// `BinaryOp(unsigned Opcode, LHS, RHS, IsNSW, IsNUW)`: no instruction.
    fn made(
        opcode: Bop,
        lhs: Opnd,
        rhs: Opnd,
    ) -> Self {
        Self { opcode, lhs, rhs, is_nsw: false, is_nuw: false, op: None }
    }
}

impl ScalarEvolution {
    /// The value an instruction defines.
    pub(super) fn inst_value(
        &self,
        cx: &Cx,
        inst: InstId,
    ) -> Operand {
        Operand::Value(cx.function.instruction(inst).result.expect("an instruction that defines a value"))
    }

    /// The instruction that defines `v`.
    pub(super) fn defining_inst(
        &self,
        cx: &Cx,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(id) = v else { return None };
        match cx.function.value(id).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    /// `isSCEVable(V->getType())`.
    pub fn is_value_scevable(
        &self,
        cx: &Cx,
        v: Operand,
    ) -> bool {
        cx.function.operand_type(cx.context, v).is_some_and(|ty| self.ty_of(cx, ty).is_some())
    }

    /// The constant integer `v` is, `dyn_cast<ConstantInt>`.
    pub(super) fn const_int(
        &self,
        cx: &Cx,
        v: Opnd,
    ) -> Option<Ap> {
        match v {
            Opnd::Lit(ap) => Some(ap),
            Opnd::V(op) => cx.query(None).int_constant(op).map(|(bits, width)| Ap::new(width, bits)),
        }
    }

    pub(super) fn opnd_ty(
        &self,
        cx: &Cx,
        v: Opnd,
    ) -> Ty {
        match v {
            Opnd::Lit(ap) => Ty::Int(ap.width),
            Opnd::V(op) => self.operand_ty(cx, op),
        }
    }

    /// `getSCEV(Value *)` for an operand of a [`BinaryOp`].
    pub(super) fn get_scev_opnd(
        &mut self,
        cx: &Cx,
        v: Opnd,
    ) -> Scev {
        match v {
            Opnd::Lit(ap) => self.get_constant(ap),
            Opnd::V(op) => self.get_scev(cx, op),
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L4560
    /// `ScalarEvolution::getSCEV(Value *)`: an existing SCEV if there is one,
    /// otherwise the value analysed and a new one made.
    pub fn get_scev(
        &mut self,
        cx: &Cx,
        v: Operand,
    ) -> Scev {
        debug_assert!(self.is_value_scevable(cx, v), "Value is not SCEVable!");

        if let Some(s) = self.get_existing_scev(v) {
            return s;
        }
        self.create_scev_iter(cx, v)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L5228
    /// `MatchBinaryOp`: `v` as a [`BinaryOp`], or none.
    pub(super) fn match_binary_op(
        &self,
        cx: &Cx,
        v: Opnd,
    ) -> Option<BinaryOp> {
        let Opnd::V(v) = v else { return None };
        let inst = self.defining_inst(cx, v)?;
        let instruction = cx.function.instruction(inst);

        // Implementation detail: all the cleverness here should happen
        // without creating new SCEV expressions -- our caller knowns tricks to
        // avoid creating SCEV expressions when possible, and we should not
        // break that.

        let operand = |at: usize| Opnd::V(instruction.operands[at]);
        // `BinaryOp(Operator *Op)`: nsw and nuw are read from an
        // `OverflowingBinaryOperator`.
        let concrete = |opcode: Bop| {
            let overflowing = matches!(opcode, Bop::Add | Bop::Sub | Bop::Mul | Bop::Shl);
            BinaryOp {
                opcode,
                lhs: operand(0),
                rhs: operand(1),
                is_nsw: overflowing && instruction.flags.contains(IrFlags::NSW),
                is_nuw: overflowing && instruction.flags.contains(IrFlags::NUW),
                op: Some(inst),
            }
        };

        match &instruction.opcode {
            Opcode::Binary(op) => match op {
                Bin::Add => Some(concrete(Bop::Add)),
                Bin::Sub => Some(concrete(Bop::Sub)),
                Bin::Mul => Some(concrete(Bop::Mul)),
                Bin::UDiv => Some(concrete(Bop::UDiv)),
                Bin::URem => Some(concrete(Bop::URem)),
                Bin::And => Some(concrete(Bop::And)),
                Bin::AShr => Some(concrete(Bop::AShr)),
                Bin::Shl => Some(concrete(Bop::Shl)),

                Bin::Or => {
                    // Convert or disjoint into add nuw nsw.
                    if instruction.flags.contains(IrFlags::DISJOINT) {
                        return Some(BinaryOp {
                            opcode: Bop::Add,
                            lhs: operand(0),
                            rhs: operand(1),
                            is_nsw: true,
                            is_nuw: true,
                            // Keep the reference to the original instruction
                            // so that we can later check whether it can
                            // produce poison value or not.
                            op: Some(inst),
                        });
                    }
                    Some(concrete(Bop::Or))
                }

                Bin::Xor => {
                    if let Some(rhs) = self.const_int(cx, operand(1)) {
                        // If the RHS of the xor is a signmask, then this is
                        // just an add. Instcombine turns add of signmask into
                        // xor as a strength reduction step.
                        if rhs.is_signed_min() {
                            return Some(BinaryOp::made(Bop::Add, operand(0), operand(1)));
                        }
                    }
                    // Binary `xor` is a bit-wise `add`.
                    if self.operand_ty(cx, v) == Ty::Int(1) {
                        return Some(BinaryOp::made(Bop::Add, operand(0), operand(1)));
                    }
                    Some(concrete(Bop::Xor))
                }

                Bin::LShr => {
                    // Turn logical shift right of a constant into a unsigned
                    // divide.
                    if let Some(shift) = self.const_int(cx, operand(1)) {
                        let Ty::Int(bit_width) = self.operand_ty(cx, v) else { unreachable!("an integer shift") };

                        // If the shift count is not less than the bitwidth,
                        // the result of the shift is undefined. Don't try to
                        // analyze it, because the resolution chosen here may
                        // differ from the resolution chosen in other parts of
                        // the compiler.
                        if shift.zext_value() < u128::from(bit_width) {
                            let x = Ap::one(bit_width).shl(shift.zext_value() as u32);
                            return Some(BinaryOp::made(Bop::UDiv, operand(0), Opnd::Lit(x)));
                        }
                    }
                    Some(concrete(Bop::LShr))
                }
                _ => None,
            },

            Opcode::ExtractValue(indices) => {
                if indices.len() != 1 || indices[0] != 0 {
                    return None;
                }

                let wo = self.defining_inst(cx, instruction.operands[0])?;
                let q = cx.query(None);
                let Some(Intrinsic::WithOverflow { op, signed }) = q.intrinsic(wo) else { return None };
                let wo_operands = &cx.function.instruction(wo).operands;
                let (lhs, rhs) = (Opnd::V(wo_operands[0]), Opnd::V(wo_operands[1]));
                let opcode = match op {
                    Bin::Add => Bop::Add,
                    Bin::Sub => Bop::Sub,
                    Bin::Mul => Bop::Mul,
                    _ => unreachable!("a with.overflow intrinsic adds, subtracts or multiplies"),
                };

                // TODO: Should add nuw/nsw flags for mul as well.
                if opcode == Bop::Mul || !llrm_mir::valuetracking::range::is_overflow_intrinsic_no_wrap(&q, wo, cx.tree)
                {
                    return Some(BinaryOp::made(opcode, lhs, rhs));
                }

                // Now that we know that all uses of the arithmetic-result
                // component of CI are guarded by the overflow check, we can go
                // ahead and pretend that the arithmetic is non-overflowing.
                Some(BinaryOp { opcode, lhs, rhs, is_nsw: signed, is_nuw: !signed, op: None })
            }

            // The `loop.decrement.reg` intrinsic is not MIR (DEVIATE 9).
            _ => None,
        }
    }

    /// `Operator::getOpcode()` of an instruction that is a cast, as MIR names
    /// it.
    fn cast_of(
        &self,
        cx: &Cx,
        v: Operand,
    ) -> Option<(CastOp, Operand)> {
        let inst = self.defining_inst(cx, v)?;
        let instruction = cx.function.instruction(inst);
        match instruction.opcode {
            Opcode::Cast(op) => Some((op, instruction.operands[0])),
            _ => None,
        }
    }

    /// `CallBase::getReturnedArgOperand`: the argument a `returned` parameter
    /// takes.
    fn returned_arg_operand(
        &self,
        cx: &Cx,
        inst: InstId,
    ) -> Option<Operand> {
        let instruction = cx.function.instruction(inst);
        let Opcode::Call(info) = &instruction.opcode else { return None };
        let args = &instruction.operands[..instruction.operands.len() - 1];
        info.argument_attrs
            .iter()
            .position(|attrs| attrs.iter().any(|one| matches!(one, Attribute::Flag(name) if name == "returned")))
            .and_then(|at| args.get(at).copied())
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7561
    /// `createSCEVIter`: the expression of `v`, made from the bottom up
    /// without recursion.
    fn create_scev_iter(
        &mut self,
        cx: &Cx,
        v: Operand,
    ) -> Scev {
        // A value that is not an instruction or argument has no entry in the
        // map: it is made at once. It is not a node made: the climb of
        // `isBasicBlockEntryGuardedByCond` asked for the same constant once
        // per guard (92% of the count at `functions` N=128).
        if !matches!(v, Operand::Value(_)) {
            let mut ops = Vec::new();
            return self.get_operands_to_create(cx, v, &mut ops).expect("a constant has its SCEV at once");
        }
        llrm_mir::counts::bump(llrm_mir::counts::Event::ScevNodes);

        // Worklist item with a Value and a bool indicating whether all
        // operands have been visited already.
        let mut stack: Vec<(Operand, bool)> = vec![(v, false)];
        while let Some(&(cur_v, ready)) = stack.last() {
            if self.get_existing_scev(cur_v).is_some() {
                stack.pop();
                continue;
            }

            let mut ops: Vec<Operand> = Vec::new();
            // If all operands have been visited already, create the SCEV.
            let created_scev = if ready {
                Some(self.create_scev(cx, cur_v))
            } else {
                // Otherwise get the operands we need to create SCEV's for
                // before creating the SCEV for CurV. If the SCEV for CurV can
                // be constructed trivially, just use it.
                self.get_operands_to_create(cx, cur_v, &mut ops)
            };

            if let Some(created) = created_scev {
                self.insert_value_to_map(cur_v, created);
                stack.pop();
            } else {
                stack.last_mut().expect("the item").1 = true;
                // Queue its operands which need to be constructed. Constants
                // have their expression at once and are not held.
                for op in ops {
                    if matches!(op, Operand::Value(_)) {
                        stack.push((op, false));
                    }
                }
            }
        }

        self.get_existing_scev(v).expect("the SCEV of the value just made")
    }

    /// The operands of a value, other than blocks, that are values (the ones
    /// `createSCEVIter` stacks).
    pub(super) fn push_value_operands(
        &self,
        cx: &Cx,
        ops: &mut Vec<Operand>,
        operands: &[Operand],
    ) {
        for &op in operands {
            if matches!(op, Operand::Value(_)) && self.is_value_scevable(cx, op) {
                ops.push(op);
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7604
    /// `getOperandsToCreate`: the SCEV of `v` if it is trivial, else none and
    /// in `ops` the operands to make first.
    fn get_operands_to_create(
        &mut self,
        cx: &Cx,
        v: Operand,
        ops: &mut Vec<Operand>,
    ) -> Option<Scev> {
        if !self.is_value_scevable(cx, v) {
            return Some(self.get_unknown(cx, v));
        }

        let inst = self.defining_inst(cx, v);
        if let Some(inst) = inst {
            // Don't attempt to analyze instructions in blocks that aren't
            // reachable. Such instructions don't matter, and they aren't
            // required to obey basic rules for definitions dominating uses
            // which this analysis depends on.
            if !cx.function.parent(inst).is_some_and(|block| cx.tree.is_reachable(block)) {
                return Some(self.get_unknown(cx, v));
            }
        } else if let Some(ap) = self.const_int(cx, Opnd::V(v)) {
            return Some(self.get_constant(ap));
        } else if let Operand::Constant(_) = v {
            return Some(self.create_node_for_constant_gep(cx, v).unwrap_or_else(|| self.get_unknown(cx, v)));
        } else {
            // An argument.
            return Some(self.get_unknown(cx, v));
        }
        let inst = inst.expect("an instruction");
        let instruction = cx.function.instruction(inst);

        if let Some(mut bo) = self.match_binary_op(cx, Opnd::V(v)) {
            let is_const_arg = self.const_int(cx, bo.rhs).is_some();
            match bo.opcode {
                Bop::Add | Bop::Mul => {
                    // For additions and multiplications, traverse add/mul
                    // chains for which we can potentially create a single
                    // SCEV, to reduce the number of get{Add,Mul}Expr calls.
                    loop {
                        if let Some(bo_op) = bo.op {
                            let bo_value = self.inst_value(cx, bo_op);
                            if bo_value != v && self.get_existing_scev(bo_value).is_some() {
                                ops.push(bo_value);
                                break;
                            }
                        }
                        self.push_opnd(cx, ops, bo.rhs);
                        let new_bo = self.match_binary_op(cx, bo.lhs);
                        let stop = match &new_bo {
                            None => true,
                            Some(new_bo) => {
                                (bo.opcode == Bop::Add && new_bo.opcode != Bop::Add && new_bo.opcode != Bop::Sub)
                                    || (bo.opcode == Bop::Mul && new_bo.opcode != Bop::Mul)
                            }
                        };
                        if stop {
                            self.push_opnd(cx, ops, bo.lhs);
                            break;
                        }
                        // CreateSCEV calls getNoWrapFlagsFromUB, which under
                        // certain conditions requires a SCEV for the LHS.
                        if let Some(bo_op) = bo.op
                            && (bo.is_nsw || bo.is_nuw)
                        {
                            let value = self.inst_value(cx, bo_op);
                            if guarantee::program_undefined_if_poison(&cx.query(None), value) {
                                self.push_opnd(cx, ops, bo.lhs);
                                break;
                            }
                        }
                        bo = new_bo.expect("a chain that goes on");
                    }
                    return None;
                }
                Bop::Sub | Bop::UDiv | Bop::URem => {}
                Bop::AShr | Bop::Shl | Bop::Xor => {
                    if !is_const_arg {
                        return None;
                    }
                }
                Bop::And | Bop::Or => {
                    if !is_const_arg && self.opnd_ty(cx, bo.lhs) != Ty::Int(1) {
                        return None;
                    }
                }
                Bop::LShr => return Some(self.get_unknown(cx, v)),
            }

            self.push_opnd(cx, ops, bo.lhs);
            self.push_opnd(cx, ops, bo.rhs);
            return None;
        }

        match &instruction.opcode {
            Opcode::Cast(CastOp::Trunc | CastOp::ZExt | CastOp::SExt | CastOp::PtrToInt) => {
                ops.push(instruction.operands[0]);
                None
            }

            Opcode::Cast(CastOp::BitCast) => {
                if self.is_value_scevable(cx, v) && self.is_value_scevable(cx, instruction.operands[0]) {
                    ops.push(instruction.operands[0]);
                    return None;
                }
                Some(self.get_unknown(cx, v))
            }

            Opcode::Binary(Bin::SDiv | Bin::SRem) => {
                ops.push(instruction.operands[0]);
                ops.push(instruction.operands[1]);
                None
            }

            Opcode::GetElementPtr { .. } => {
                self.push_value_operands(cx, ops, &instruction.operands);
                None
            }

            Opcode::Cast(CastOp::IntToPtr) => Some(self.get_unknown(cx, v)),

            Opcode::Phi => self.get_operands_to_create_phi(cx, inst, ops),

            Opcode::Select => {
                // Check if U is a select that can be simplified to a
                // SCEVUnknown.
                if self.select_can_simplify_to_unknown(cx, inst) {
                    return Some(self.get_unknown(cx, v));
                }

                self.push_value_operands(cx, ops, &instruction.operands);
                None
            }

            Opcode::Call(_) => {
                if let Some(rv) = self.returned_arg_operand(cx, inst) {
                    ops.push(rv);
                    return None;
                }

                match cx.query(None).intrinsic(inst) {
                    Some(Intrinsic::MinMax { .. } | Intrinsic::Saturating { signed: false, .. }) => {
                        ops.push(instruction.operands[0]);
                        ops.push(instruction.operands[1]);
                    }
                    Some(Intrinsic::Abs) => ops.push(instruction.operands[0]),
                    _ => {}
                }
                None
            }

            _ => None,
        }
    }

    /// Pushes the value of an operand of a [`BinaryOp`] for `createSCEVIter`
    /// to stack; a constant is not stacked.
    fn push_opnd(
        &self,
        _cx: &Cx,
        ops: &mut Vec<Operand>,
        v: Opnd,
    ) {
        if let Opnd::V(op) = v {
            ops.push(op);
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7335
    /// `getNoWrapFlagsFromUB`: the no-wrap flags of the binary operation
    /// `inst` that hold wherever the expression is defined, because poison
    /// from it is undefined behaviour.
    pub(super) fn get_no_wrap_flags_from_ub(
        &mut self,
        cx: &Cx,
        inst: InstId,
    ) -> u8 {
        let instruction = cx.function.instruction(inst);
        // Return early if there are no flags to propagate to the SCEV.
        let mut f = flags::NONE;
        if instruction.flags.contains(IrFlags::DISJOINT) && matches!(instruction.opcode, Opcode::Binary(Bin::Or)) {
            f = flags::NUW | flags::NSW;
        } else {
            if instruction.flags.contains(IrFlags::NUW) {
                f |= flags::NUW;
            }
            if instruction.flags.contains(IrFlags::NSW) {
                f |= flags::NSW;
            }
        }
        if f == flags::NONE {
            return flags::NONE;
        }

        if self.is_scev_expr_never_poison(cx, inst) { f } else { flags::NONE }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ScalarEvolution.cpp#L7837
    /// `createSCEV`: the expression of `v` from the expressions of its
    /// operands.
    pub(super) fn create_scev(
        &mut self,
        cx: &Cx,
        v: Operand,
    ) -> Scev {
        if !self.is_value_scevable(cx, v) {
            return self.get_unknown(cx, v);
        }

        let inst = self.defining_inst(cx, v);
        if let Some(inst) = inst {
            // Don't attempt to analyze instructions in blocks that aren't
            // reachable. Such instructions don't matter, and they aren't
            // required to obey basic rules for definitions dominating uses
            // which this analysis depends on.
            if !cx.function.parent(inst).is_some_and(|block| cx.tree.is_reachable(block)) {
                return self.get_unknown(cx, v);
            }
        } else if let Some(ap) = self.const_int(cx, Opnd::V(v)) {
            return self.get_constant(ap);
        } else {
            return self.get_unknown(cx, v);
        }
        let inst = inst.expect("an instruction");
        let instruction = cx.function.instruction(inst);

        if let Some(mut bo) = self.match_binary_op(cx, Opnd::V(v)) {
            match bo.opcode {
                Bop::Add => {
                    // The simple thing to do would be to just call getSCEV on
                    // both operands and call getAddExpr with the result.
                    // However if we're looking at a bunch of things all added
                    // together, this can be quite inefficient, because it
                    // leads to N-1 getAddExpr calls for N ultimate operands.
                    // Instead, gather up all the operands and make a single
                    // getAddExpr call. LLVM IR canonical form means we need
                    // only traverse the left operands.
                    let mut add_ops: Vec<Use> = Vec::new();
                    loop {
                        if let Some(bo_op) = bo.op {
                            let bo_value = self.inst_value(cx, bo_op);
                            if let Some(op_scev) = self.get_existing_scev(bo_value) {
                                add_ops.push(Use::of(op_scev));
                                break;
                            }

                            // If a NUW or NSW flag can be applied to the SCEV
                            // for this addition, then compute the SCEV for
                            // this addition by itself with a separate call to
                            // getAddExpr. We need to do that instead of
                            // pushing the operands of the addition onto
                            // AddOps, since the flags are only known to apply
                            // to this particular addition - they may not apply
                            // to other additions that can be formed with
                            // operands from AddOps.
                            let rhs = self.get_scev_opnd(cx, bo.rhs);
                            let f = self.get_no_wrap_flags_from_ub(cx, bo_op);
                            if f != flags::NONE {
                                let lhs = self.get_scev_opnd(cx, bo.lhs);
                                if bo.opcode == Bop::Sub {
                                    add_ops.push(Use::of(self.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), f, 0)));
                                } else {
                                    add_ops.push(self.get_add_expr(
                                        cx,
                                        vec![Use::of(lhs), Use::of(rhs)],
                                        Flags::expr(f),
                                        0,
                                    ));
                                }
                                break;
                            }
                        }

                        if bo.opcode == Bop::Sub {
                            let rhs = self.get_scev_opnd(cx, bo.rhs);
                            add_ops.push(Use::of(self.get_negative_scev(cx, rhs, flags::NONE)));
                        } else {
                            add_ops.push(Use::of(self.get_scev_opnd(cx, bo.rhs)));
                        }

                        let new_bo = self.match_binary_op(cx, bo.lhs);
                        match new_bo {
                            Some(new_bo) if matches!(new_bo.opcode, Bop::Add | Bop::Sub) => bo = new_bo,
                            _ => {
                                add_ops.push(Use::of(self.get_scev_opnd(cx, bo.lhs)));
                                break;
                            }
                        }
                    }

                    return self.get_add_expr(cx, add_ops, Flags::default(), 0).scev;
                }

                Bop::Mul => {
                    let mut mul_ops: Vec<Use> = Vec::new();
                    loop {
                        if let Some(bo_op) = bo.op {
                            let bo_value = self.inst_value(cx, bo_op);
                            if let Some(op_scev) = self.get_existing_scev(bo_value) {
                                mul_ops.push(Use::of(op_scev));
                                break;
                            }

                            let f = self.get_no_wrap_flags_from_ub(cx, bo_op);
                            if f != flags::NONE {
                                let lhs = self.get_scev_opnd(cx, bo.lhs);
                                let rhs = self.get_scev_opnd(cx, bo.rhs);
                                mul_ops.push(self.get_mul_expr(
                                    cx,
                                    vec![Use::of(lhs), Use::of(rhs)],
                                    Flags::expr(f),
                                    0,
                                ));
                                break;
                            }
                        }

                        mul_ops.push(Use::of(self.get_scev_opnd(cx, bo.rhs)));
                        let new_bo = self.match_binary_op(cx, bo.lhs);
                        match new_bo {
                            Some(new_bo) if new_bo.opcode == Bop::Mul => bo = new_bo,
                            _ => {
                                mul_ops.push(Use::of(self.get_scev_opnd(cx, bo.lhs)));
                                break;
                            }
                        }
                    }

                    return self.get_mul_expr(cx, mul_ops, Flags::default(), 0).scev;
                }
                Bop::UDiv => {
                    let lhs = self.get_scev_opnd(cx, bo.lhs);
                    let rhs = self.get_scev_opnd(cx, bo.rhs);
                    return self.get_udiv_expr(cx, Use::of(lhs), Use::of(rhs));
                }
                Bop::URem => {
                    let lhs = self.get_scev_opnd(cx, bo.lhs);
                    let rhs = self.get_scev_opnd(cx, bo.rhs);
                    return self.get_urem_expr(cx, Use::of(lhs), Use::of(rhs));
                }
                Bop::Sub => return self.create_scev_sub(cx, &bo),
                Bop::And => {
                    if let Some(s) = self.create_scev_and(cx, &bo) {
                        return s;
                    }
                }
                Bop::Or => {
                    // Binary `or` is a bit-wise `umax`.
                    if self.opnd_ty(cx, bo.lhs) == Ty::Int(1) {
                        let lhs = self.get_scev_opnd(cx, bo.lhs);
                        let rhs = self.get_scev_opnd(cx, bo.rhs);
                        return self.get_umax_expr(cx, vec![Use::of(lhs), Use::of(rhs)]);
                    }
                }
                Bop::Xor => {
                    if let Some(s) = self.create_scev_xor(cx, &bo) {
                        return s;
                    }
                }
                Bop::Shl => {
                    if let Some(s) = self.create_scev_shl(cx, &bo) {
                        return s;
                    }
                }
                Bop::AShr => {
                    if let Some(s) = self.create_scev_ashr(cx, &bo) {
                        return s;
                    }
                }
                // `MatchBinaryOp` yields `lshr` only when it is not a divide
                // by a power of two, and `createSCEV` has no case for it.
                Bop::LShr => {}
            }
        }

        match &instruction.opcode {
            Opcode::Cast(CastOp::Trunc) => {
                let ty = self.operand_ty(cx, v);
                let op = self.get_scev(cx, instruction.operands[0]);
                self.get_truncate_expr(cx, Use::of(op), ty, 0)
            }

            Opcode::Cast(CastOp::ZExt) => {
                let ty = self.operand_ty(cx, v);
                let op = self.get_scev(cx, instruction.operands[0]);
                self.get_zero_extend_expr(cx, Use::of(op), ty, 0)
            }

            Opcode::Cast(CastOp::SExt) => {
                let ty = self.operand_ty(cx, v);
                if let Some(bo) = self.match_binary_op(cx, Opnd::V(instruction.operands[0])) {
                    // The NSW flag of a subtract does not always survive the
                    // conversion to A + (-1)*B. By pushing sign extension onto
                    // its operands we are much more likely to preserve NSW and
                    // allow later AddRec optimisations.
                    //
                    // NOTE: This is effectively duplicating this logic from
                    // getSignExtend:
                    //   sext((A + B + ...)<nsw>) --> (sext(A) + sext(B) +
                    // ...)<nsw> but by that point the NSW
                    // information has potentially
                    // been lost.
                    if bo.opcode == Bop::Sub && bo.is_nsw {
                        let lhs = self.get_scev_opnd(cx, bo.lhs);
                        let v1 = self.get_sign_extend_expr(cx, Use::of(lhs), ty, 0);
                        let rhs = self.get_scev_opnd(cx, bo.rhs);
                        let v2 = self.get_sign_extend_expr(cx, Use::of(rhs), ty, 0);
                        return self.get_minus_scev(cx, Use::of(v1), Use::of(v2), flags::NSW, 0);
                    }
                }
                let op = self.get_scev(cx, instruction.operands[0]);
                self.get_sign_extend_expr(cx, Use::of(op), ty, 0)
            }

            Opcode::Cast(CastOp::BitCast) => {
                // BitCasts are no-op casts so we just eliminate the cast.
                if self.is_value_scevable(cx, v) && self.is_value_scevable(cx, instruction.operands[0]) {
                    return self.get_scev(cx, instruction.operands[0]);
                }
                self.get_unknown(cx, v)
            }

            // MIR has one pointer-to-integer cast, which is the pin's
            // `ptrtoaddr` (what its clang writes for a pointer difference).
            Opcode::Cast(CastOp::PtrToInt) => {
                let ty = self.operand_ty(cx, v);
                let op = self.get_scev(cx, instruction.operands[0]);
                let addr = self.get_ptr_to_addr_expr(cx, op);
                if matches!(self.node(addr).kind, Kind::CouldNotCompute) || self.type_of(addr) != ty {
                    return self.get_unknown(cx, v);
                }
                addr
            }

            // Just don't deal with inttoptr casts.
            Opcode::Cast(CastOp::IntToPtr) => self.get_unknown(cx, v),

            Opcode::Binary(Bin::SDiv) => {
                // If both operands are non-negative, this is just an udiv.
                let lhs = self.get_scev(cx, instruction.operands[0]);
                let rhs = self.get_scev(cx, instruction.operands[1]);
                if self.is_known_non_negative(cx, lhs) && self.is_known_non_negative(cx, rhs) {
                    return self.get_udiv_expr(cx, Use::of(lhs), Use::of(rhs));
                }
                self.get_unknown(cx, v)
            }

            Opcode::Binary(Bin::SRem) => {
                // If both operands are non-negative, this is just an urem.
                let lhs = self.get_scev(cx, instruction.operands[0]);
                let rhs = self.get_scev(cx, instruction.operands[1]);
                if self.is_known_non_negative(cx, lhs) && self.is_known_non_negative(cx, rhs) {
                    return self.get_urem_expr(cx, Use::of(lhs), Use::of(rhs));
                }
                self.get_unknown(cx, v)
            }

            Opcode::GetElementPtr { .. } => self.create_node_for_gep(cx, inst),

            Opcode::Phi => self.create_node_for_phi(cx, inst),

            Opcode::Select => {
                let [cond, true_val, false_val] = instruction.operands[..] else { unreachable!("a select has three") };
                self.create_node_for_select_or_phi(cx, v, cond, true_val, false_val)
            }

            Opcode::Call(_) => {
                if let Some(rv) = self.returned_arg_operand(cx, inst) {
                    return self.get_scev(cx, rv);
                }

                if let Some(Intrinsic::Abs) = cx.query(None).intrinsic(inst) {
                    let arg = self.get_scev(cx, instruction.operands[0]);
                    // `cast<ConstantInt>(II->getArgOperand(1))->isOne()`.
                    let is_nsw = matches!(
                        instruction.operands[1],
                        Operand::Constant(id) if matches!(cx.context.get(id).kind, ConstantKind::Int(1))
                    );
                    return self.get_abs_expr(cx, arg, is_nsw);
                }
                if let Some(Intrinsic::MinMax { signed, max }) = cx.query(None).intrinsic(inst) {
                    let lhs = self.get_scev(cx, instruction.operands[0]);
                    let rhs = self.get_scev(cx, instruction.operands[1]);
                    let ops = vec![Use::of(lhs), Use::of(rhs)];
                    return match (signed, max) {
                        (false, true) => self.get_umax_expr(cx, ops),
                        (false, false) => self.get_umin_expr(cx, ops, false),
                        (true, true) => self.get_smax_expr(cx, ops),
                        (true, false) => self.get_smin_expr(cx, ops),
                    };
                }
                if let Some(Intrinsic::Saturating { op, signed: false }) = cx.query(None).intrinsic(inst) {
                    let x = self.get_scev(cx, instruction.operands[0]);
                    let y = self.get_scev(cx, instruction.operands[1]);
                    return match op {
                        // usub.sat: X - umin(X, Y), which cannot wrap.
                        Bin::Sub => {
                            let clamped = self.get_umin_expr(cx, vec![Use::of(x), Use::of(y)], false);
                            self.get_minus_scev(cx, Use::of(x), Use::of(clamped), flags::NUW, 0)
                        }
                        // uadd.sat: umin(X, ~Y) + Y, which cannot wrap.
                        Bin::Add => {
                            let not_y = self.get_not_scev(cx, y);
                            let clamped = self.get_umin_expr(cx, vec![Use::of(x), Use::of(not_y)], false);
                            self.get_add_expr(cx, vec![Use::of(clamped), Use::of(y)], Flags::expr(flags::NUW), 0).scev
                        }
                        _ => unreachable!("an unsigned saturating intrinsic adds or subtracts"),
                    };
                }
                self.get_unknown(cx, v)
            }

            _ => self.get_unknown(cx, v),
        }
    }

    /// `createSCEV`, `case Instruction::Sub`.
    fn create_scev_sub(
        &mut self,
        cx: &Cx,
        bo: &BinaryOp,
    ) -> Scev {
        let f = match bo.op {
            Some(op) => self.get_no_wrap_flags_from_ub(cx, op),
            None => flags::NONE,
        };

        // Try to use ptrtoaddr for subtracts with at least one ptrtoint
        // operand. While we don't model ptrtoint directly in SCEV, the
        // difference between two pointer addresses is well-defined.
        let ptr_to_int = |this: &Self, v: Opnd| match v {
            Opnd::V(op) => this.cast_of(cx, op).filter(|(cast, _)| *cast == CastOp::PtrToInt).map(|(_, ptr)| ptr),
            Opnd::Lit(_) => None,
        };
        let (ptr_lhs, ptr_rhs) = (ptr_to_int(self, bo.lhs), ptr_to_int(self, bo.rhs));
        if ptr_lhs.is_some() || ptr_rhs.is_some() {
            // Convert a ptrtoint operand (OrigOp) to ptrtoaddr of its pointer
            // PtrOp. When only one side is ptrtoint (BothPtr is false), skip
            // SCEVUnknown pointers since wrapping them in ptrtoaddr adds no
            // useful structure.
            let get_op = |this: &mut Self, ptr: Option<Operand>, orig: Opnd, both_ptr: bool| -> Scev {
                let Some(ptr_op) = ptr else { return this.get_scev_opnd(cx, orig) };
                let ptr_scev = this.get_scev(cx, ptr_op);
                if both_ptr || !matches!(this.node(ptr_scev).kind, Kind::Unknown(_)) {
                    let addr = this.get_ptr_to_addr_expr(cx, ptr_scev);
                    if !matches!(this.node(addr).kind, Kind::CouldNotCompute)
                        && this.get_type_size_in_bits(cx, this.opnd_ty(cx, orig))
                            <= this.get_type_size_in_bits(cx, this.type_of(addr))
                    {
                        let ty = this.opnd_ty(cx, orig);
                        return this.get_truncate_or_noop(cx, addr, ty);
                    }
                }
                this.get_scev_opnd(cx, orig)
            };
            let l = get_op(self, ptr_lhs, bo.lhs, ptr_rhs.is_some());
            let r = get_op(self, ptr_rhs, bo.rhs, ptr_lhs.is_some());
            return self.get_minus_scev(cx, Use::of(l), Use::of(r), f, 0);
        }

        let lhs = self.get_scev_opnd(cx, bo.lhs);
        let rhs = self.get_scev_opnd(cx, bo.rhs);
        self.get_minus_scev(cx, Use::of(lhs), Use::of(rhs), f, 0)
    }

    /// `createSCEV`, `case Instruction::And`: none is the `break`.
    fn create_scev_and(
        &mut self,
        cx: &Cx,
        bo: &BinaryOp,
    ) -> Option<Scev> {
        // For an expression like x&255 that merely masks off the high bits,
        // use zext(trunc(x)) as the SCEV expression.
        if let Some(a) = self.const_int(cx, bo.rhs) {
            if a.is_zero() {
                return Some(self.get_scev_opnd(cx, bo.rhs));
            }
            if a.is_all_ones() {
                return Some(self.get_scev_opnd(cx, bo.lhs));
            }

            // Instcombine's ShrinkDemandedConstant may strip bits out of
            // constants, obscuring what would otherwise be a low-bits mask.
            // Use computeKnownBits to compute what ShrinkDemandedConstant knew
            // about to reconstruct a low-bits mask value.
            let lz = a.countl_zero();
            let tz = a.countr_zero();
            let bit_width = a.width;
            let Opnd::V(lhs_value) = bo.lhs else { unreachable!("an and of a value") };
            let known = llrm_mir::valuetracking::known::compute_known_bits(&cx.query(None), lhs_value, 0);

            let effective_mask = low_bits_set(bit_width, bit_width - lz - tz).shl(tz);
            let known_zero = Ap::new(bit_width, known.zero);
            if (lz != 0 || tz != 0) && a.not().and(known_zero.not()).and(effective_mask).is_zero() {
                let mul_count = self.get_constant(Ap::one(bit_width).shl(tz));
                let lhs = self.get_scev_opnd(cx, bo.lhs);
                let mut shifted_lhs: Option<Scev> = None;
                if let Kind::Mul(mul_ops) = self.node(lhs).kind.clone()
                    && let Some(op_c) = self.as_constant(mul_ops[0].scev)
                {
                    // For an expression like (x * 8) & 8, simplify the
                    // multiply.
                    let mul_zeros = op_c.countr_zero();
                    let gcd = mul_zeros.min(tz);
                    let div_amt = Ap::one(bit_width).shl(tz - gcd);
                    let mut new_ops: Vec<Use> = vec![Use::of(self.get_constant(op_c.ashr(gcd)))];
                    new_ops.extend(mul_ops[1..].iter().copied());
                    let mul_flags = self.get_no_wrap_flags(lhs, flags::MASK);
                    let new_mul = self.get_mul_expr(cx, new_ops, Flags::expr(mul_flags), 0).scev;
                    let div = self.get_constant(div_amt);
                    shifted_lhs = Some(self.get_udiv_expr(cx, Use::of(new_mul), Use::of(div)));
                }
                let shifted_lhs = match shifted_lhs {
                    Some(s) => s,
                    None => self.get_udiv_expr(cx, Use::of(lhs), Use::of(mul_count)),
                };
                let truncated = self.get_truncate_expr(cx, Use::of(shifted_lhs), Ty::Int(bit_width - lz - tz), 0);
                let outer_ty = self.opnd_ty(cx, bo.lhs);
                let extended = self.get_zero_extend_expr(cx, Use::of(truncated), outer_ty, 0);
                return Some(
                    self.get_mul_expr(cx, vec![Use::of(extended), Use::of(mul_count)], Flags::default(), 0).scev,
                );
            }
        }
        // Binary `and` is a bit-wise `umin`.
        if self.opnd_ty(cx, bo.lhs) == Ty::Int(1) {
            let lhs = self.get_scev_opnd(cx, bo.lhs);
            let rhs = self.get_scev_opnd(cx, bo.rhs);
            return Some(self.get_umin_expr(cx, vec![Use::of(lhs), Use::of(rhs)], false));
        }
        None
    }

    /// `createSCEV`, `case Instruction::Xor`: none is the `break`.
    fn create_scev_xor(
        &mut self,
        cx: &Cx,
        bo: &BinaryOp,
    ) -> Option<Scev> {
        let ci = self.const_int(cx, bo.rhs)?;
        // If the RHS of xor is -1, then this is a not operation.
        if ci.is_all_ones() {
            let lhs = self.get_scev_opnd(cx, bo.lhs);
            return Some(self.get_not_scev(cx, lhs));
        }

        // Model xor(and(x, C), C) as and(~x, C), if C is a low-bits mask. This
        // is a variant of the check for xor with -1, and it handles the case
        // where instcombine has trimmed non-demanded bits out of an xor with
        // -1.
        let Opnd::V(lhs_value) = bo.lhs else { return None };
        let lbo = self.defining_inst(cx, lhs_value)?;
        let lbo_instruction = cx.function.instruction(lbo);
        if !matches!(lbo_instruction.opcode, Opcode::Binary(Bin::And)) {
            return None;
        }
        let lci = self.const_int(cx, Opnd::V(lbo_instruction.operands[1]))?;
        if lci != ci {
            return None;
        }
        let lhs_scev = self.get_scev_opnd(cx, bo.lhs);
        let Kind::ZeroExtend(z0_use) = self.node(lhs_scev).kind else { return None };
        let u_ty = self.operand_ty(cx, lhs_value);
        let z0 = z0_use.scev;
        let z0_ty = self.type_of(z0);
        let z0_ty_size = self.get_type_size_in_bits(cx, z0_ty);

        // If C is a low-bits mask, the zero extend is serving to mask off the
        // high bits. Complement the operand and re-apply the zext.
        if is_mask(ci, z0_ty_size) {
            let not = self.get_not_scev(cx, z0);
            return Some(self.get_zero_extend_expr(cx, Use::of(not), u_ty, 0));
        }

        // If C is a single bit, it may be in the sign-bit position before the
        // zero-extend. In this case, represent the xor using an add, which is
        // equivalent, and re-apply the zext.
        let trunc = ci.trunc(z0_ty_size);
        if trunc.zext(self.get_type_size_in_bits(cx, u_ty)) == ci && trunc.is_signed_min() {
            let c = self.get_constant(trunc);
            let sum = self.get_add_expr(cx, vec![Use::of(z0), Use::of(c)], Flags::default(), 0);
            return Some(self.get_zero_extend_expr(cx, sum, u_ty, 0));
        }
        None
    }

    /// `createSCEV`, `case Instruction::Shl`: none is the `break`.
    fn create_scev_shl(
        &mut self,
        cx: &Cx,
        bo: &BinaryOp,
    ) -> Option<Scev> {
        // Turn shift left of a constant amount into a multiply.
        let sa = self.const_int(cx, bo.rhs)?;
        let bit_width = sa.width;

        // If the shift count is not less than the bitwidth, the result of the
        // shift is undefined. Don't try to analyze it, because the resolution
        // chosen here may differ from the resolution chosen in other parts of
        // the compiler.
        if sa.zext_value() >= u128::from(bit_width) {
            return None;
        }

        // We can safely preserve the nuw flag in all cases. It's also safe to
        // turn a nuw nsw shl into a nuw nsw mul. However, nsw in isolation
        // requires special handling. It can be preserved as long as we're not
        // left shifting by bitwidth - 1.
        let mut f = flags::NONE;
        if let Some(op) = bo.op {
            let mul_flags = self.get_no_wrap_flags_from_ub(cx, op);
            if mul_flags & flags::NSW != 0
                && (mul_flags & flags::NUW != 0 || sa.zext_value() < u128::from(bit_width - 1))
            {
                f |= flags::NSW;
            }
            if mul_flags & flags::NUW != 0 {
                f |= flags::NUW;
            }
        }

        let x = Ap::one(bit_width).shl(sa.zext_value() as u32);
        let lhs = self.get_scev_opnd(cx, bo.lhs);
        let c = self.get_constant(x);
        Some(self.get_mul_expr(cx, vec![Use::of(lhs), Use::of(c)], Flags::expr(f), 0).scev)
    }

    /// `createSCEV`, `case Instruction::AShr`: none is the `break`.
    fn create_scev_ashr(
        &mut self,
        cx: &Cx,
        bo: &BinaryOp,
    ) -> Option<Scev> {
        // AShr X, C, where C is a constant.
        let ci = self.const_int(cx, bo.rhs)?;

        let outer_ty = self.opnd_ty(cx, bo.lhs);
        let bit_width = u64::from(self.get_type_size_in_bits(cx, outer_ty));
        // If the shift count is not less than the bitwidth, the result of the
        // shift is undefined. Don't try to analyze it, because the resolution
        // chosen here may differ from the resolution chosen in other parts of
        // the compiler.
        if ci.zext_value() >= u128::from(bit_width) {
            return None;
        }

        if ci.is_zero() {
            // shift by zero --> noop
            return Some(self.get_scev_opnd(cx, bo.lhs));
        }

        let ashr_amt = ci.zext_value() as u64;
        let trunc_ty = Ty::Int((bit_width - ashr_amt) as u32);

        let l = match bo.lhs {
            Opnd::V(op) => self.defining_inst(cx, op),
            Opnd::Lit(_) => None,
        };
        let l_opcode = l.map(|l| cx.function.instruction(l).opcode.clone());
        let mut add_truncate_expr: Option<Scev> = None;
        let mut shl_amt_ci: Option<Ap> = None;
        let mut add_constant: Option<Scev> = None;

        if matches!(l_opcode, Some(Opcode::Binary(Bin::Add))) {
            // X = Shl A, n
            // Y = Add X, c
            // Z = AShr Y, m
            // n, c and m are constants.
            let l = cx.function.instruction(l.expect("an add"));
            let l_shift = self.defining_inst(cx, l.operands[0]);
            let add_operand_ci = self.const_int(cx, Opnd::V(l.operands[1]));
            if let Some(l_shift) = l_shift
                && matches!(
                    cx.function.instruction(l_shift).opcode,
                    Opcode::Binary(Bin::Shl)
                )
                && let Some(add_operand_ci) = add_operand_ci
            {
                let l_shift = cx.function.instruction(l_shift);
                let shl_op0_scev = self.get_scev(cx, l_shift.operands[0]);
                shl_amt_ci = self.const_int(cx, Opnd::V(l_shift.operands[1]));
                // since we truncate to TruncTy, the AddConstant should be of
                // the same type, so create a new Constant with type same as
                // TruncTy. Also, the Add constant should be shifted right by
                // AShr amount.
                let add_operand = add_operand_ci.ashr(ashr_amt as u32);
                add_constant = Some(self.get_constant(add_operand.trunc((bit_width - ashr_amt) as u32)));
                // we model the expression as sext(add(trunc(A), c << n)),
                // since the sext(trunc) part is already handled below, we
                // create a AddExpr(TruncExp) which will be used later.
                add_truncate_expr = Some(self.get_truncate_expr(cx, Use::of(shl_op0_scev), trunc_ty, 0));
            }
        } else if matches!(l_opcode, Some(Opcode::Binary(Bin::Shl))) {
            // X = Shl A, n
            // Y = AShr X, m
            // Both n and m are constant.
            let l = cx.function.instruction(l.expect("a shl"));
            let shl_op0_scev = self.get_scev(cx, l.operands[0]);
            shl_amt_ci = self.const_int(cx, Opnd::V(l.operands[1]));
            add_truncate_expr = Some(self.get_truncate_expr(cx, Use::of(shl_op0_scev), trunc_ty, 0));
        }

        if let (Some(add_truncate_expr), Some(shl_amt)) = (add_truncate_expr, shl_amt_ci) {
            // We can merge the two given cases into a single SCEV statement,
            // incase n = m, the mul expression will be 2^0, so it gets
            // resolved to a simpler case. The following code handles the two
            // cases:
            //
            // 1) For a two-shift sext-inreg, i.e. n = m, use sext(trunc(x)) as
            //    the SCEV expression.
            //
            // 2) When n > m, use sext(mul(trunc(x), 2^(n-m)))) as the SCEV
            //    expression. We already checked that ShlAmt < BitWidth, so the
            //    multiplier, 1 << (ShlAmt - AShrAmt), fits into TruncTy as
            //    ShlAmt - AShrAmt < Amt.
            if shl_amt.zext_value() < u128::from(bit_width) && shl_amt.zext_value() >= u128::from(ashr_amt) {
                let mul = Ap::one((bit_width - ashr_amt) as u32).shl((shl_amt.zext_value() as u64 - ashr_amt) as u32);
                let mul_c = self.get_constant(mul);
                let mut composite_expr =
                    self.get_mul_expr(cx, vec![Use::of(add_truncate_expr), Use::of(mul_c)], Flags::default(), 0).scev;
                if !matches!(l_opcode, Some(Opcode::Binary(Bin::Shl))) {
                    let add_constant = add_constant.expect("the add form has its constant");
                    composite_expr = self
                        .get_add_expr(cx, vec![Use::of(composite_expr), Use::of(add_constant)], Flags::default(), 0)
                        .scev;
                }

                return Some(self.get_sign_extend_expr(cx, Use::of(composite_expr), outer_ty, 0));
            }
        }
        None
    }

    /// The `createSCEV` select case's `CanSimplifyToUnknown`.
    pub(super) fn select_can_simplify_to_unknown(
        &self,
        cx: &Cx,
        inst: InstId,
    ) -> bool {
        let instruction = cx.function.instruction(inst);
        let result = instruction.result.map(Operand::Value).expect("a select defines a value");
        if self.operand_ty(cx, result) == Ty::Int(1) || self.const_int(cx, Opnd::V(instruction.operands[0])).is_some() {
            return false;
        }

        let Some(icmp) = self.defining_inst(cx, instruction.operands[0]) else { return false };
        let icmp = cx.function.instruction(icmp);
        let Opcode::ICmp(pred) = icmp.opcode else { return false };
        let (lhs, rhs) = (icmp.operands[0], icmp.operands[1]);
        if matches!(
            pred,
            llrm_mir::opcode::IntPredicate::Eq | llrm_mir::opcode::IntPredicate::Ne
        ) {
            if !self.const_int(cx, Opnd::V(rhs)).is_some_and(|c| c.is_zero()) {
                return true;
            }
        } else if self.get_type_size_in_bits(cx, self.operand_ty(cx, lhs))
            > self.get_type_size_in_bits(cx, self.operand_ty(cx, result))
        {
            return true;
        }
        false
    }

    /// Whether `v` is the null constant of a pointer (an unknown, never an
    /// integer).
    #[allow(dead_code)]
    fn is_null_pointer(
        &self,
        cx: &Cx,
        v: Operand,
    ) -> bool {
        matches!(
            v,
            Operand::Constant(id) if matches!(cx.context.get(id).kind, ConstantKind::Null)
        )
    }
}

/// `APInt::getLowBitsSet(width, bits)`.
fn low_bits_set(
    width: u32,
    bits: u32,
) -> Ap {
    if bits == 0 { Ap::zero(width) } else { Ap::all_ones(width).lshr(width - bits) }
}

/// `APInt::isMask(bits)`: the low `bits` bits set, and no others.
fn is_mask(
    a: Ap,
    bits: u32,
) -> bool {
    bits != 0 && bits <= a.width && a == low_bits_set(a.width, bits)
}
