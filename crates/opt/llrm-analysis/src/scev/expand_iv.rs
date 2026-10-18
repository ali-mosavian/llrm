//! `SCEVExpander`'s add recurrences (`ScalarEvolutionExpander.cpp`): the
//! canonical induction variable, and induction variables written literally.
//!
//! # How it works
//! In canonical mode an affine `{X,+,F}` is `X + i*F` over one `{0,+,1}` phi
//! of the loop (reused or made). Otherwise (a chain of three or more operands,
//! or non-canonical mode, which LSR uses) the recurrence gets its own phi: the
//! start goes to the preheader, the step to the header, and the phi's
//! increment before the latch's terminator; an existing phi of the same
//! recurrence is reused.
//!
//! # Used by
//! [`super::expand::Run::visit`].
//!
//! # See also
//! `ScalarEvolutionExpander.cpp`: `visitAddRecExpr`,
//! `expandAddRecExprLiterally`.

use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags as IrFlags, Opcode};
use llrm_mir::types::{Type as MirType, TypeId};

use super::expand::Run;
use super::expr::{Kind, Scev, Ty, Use, flags};
use super::{Flags, ScalarEvolution};

macro_rules! cx {
    ($run:expr) => {
        $run.env.cx(&*$run.context, &*$run.function)
    };
}

impl Run<'_> {
    /// `expand(S, I)`: `s` expanded before `ip`, the insert point restored.
    pub(super) fn expand_at(
        &mut self,
        s: Scev,
        ip: InstId,
    ) -> Operand {
        let saved = self.x.insert_point;
        self.x.insert_point = Some(ip);
        let v = self.expand(s);
        self.x.insert_point = saved;
        v
    }

    fn instruction_of(
        &self,
        v: Operand,
    ) -> Option<InstId> {
        let Operand::Value(value) = v else { return None };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => Some(inst),
            ValueDef::Argument(_) => None,
        }
    }

    fn result_of(
        &self,
        inst: InstId,
    ) -> Operand {
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    fn scev_ty_of(
        &self,
        v: Operand,
    ) -> Ty {
        let ty = self.ty_of_operand(v);
        match self.context.types.get(ty) {
            MirType::Int(bits) => Ty::Int(*bits),
            MirType::Pointer(space) => Ty::Ptr(*space),
            other => unreachable!("a SCEVable type, not {other:?}"),
        }
    }

    /// The incoming value of `phi` from `block`.
    fn incoming_for(
        &self,
        phi: InstId,
        block: BlockId,
    ) -> Option<Operand> {
        self.function
            .instruction(phi)
            .operands
            .chunks(2)
            .find(|pair| pair[1] == Operand::Block(block))
            .map(|pair| pair[0])
    }

    fn phis_of(
        &self,
        block: BlockId,
    ) -> Vec<InstId> {
        self.function
            .block(block)
            
            .iter()
            .take_while(|&one| matches!(self.function.instruction(one).opcode, Opcode::Phi))
            .collect()
    }

    /// `Loop::getCanonicalInductionVariable`: a phi `{0,+,1}`.
    fn canonical_induction_variable(
        &self,
        header: BlockId,
    ) -> Option<InstId> {
        let l = self.env.loops.get_loop_for(header)?;
        let preds = self.function.predecessors(header);
        let [a, b] = preds.as_slice() else { return None };
        let (incoming, backedge) = match (l.contains(*a), l.contains(*b)) {
            (false, true) => (*a, *b),
            (true, false) => (*b, *a),
            _ => return None,
        };
        self.phis_of(header)
            .into_iter()
            .find(
                |&pn| {
                    let zero = match self.incoming_for(pn, incoming) {
                        Some(Operand::Constant(c)) => {
                            matches!(
                                self.context.get(c).kind,
                                llrm_mir::context::ConstantKind::Int(0)
                            )
                        }
                        _ => false,
                    };
                    let Some(Operand::Value(next)) = self.incoming_for(pn, backedge) else { return false };
                    let ValueDef::Instruction(inc) = self.function.value(next).def else { return false };
                    let inc = self.function.instruction(inc);
                    let one = matches!(
                        inc.operands.get(1),
                        Some(Operand::Constant(c)) if matches!(self.context.get(*c).kind, llrm_mir::context::ConstantKind::Int(1))
                    );
                    zero && inc.opcode == Opcode::Binary(BinaryOp::Add) && inc.operands[0] == self.result_of(pn) && one
                },
            )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1338
    /// `visitAddRecExpr`.
    pub(super) fn visit_add_rec(
        &mut self,
        s: Scev,
        ops: &[Use],
        header: BlockId,
    ) -> Operand {
        if !self.x.canonical_mode || ops.len() > 2 {
            return self.expand_add_rec_literally(s);
        }
        let ty = {
            let cx = cx!(self);
            let own = self.se.type_of(s);
            self.se.get_effective_scev_type(&cx, own)
        };
        let ty_bits = {
            let cx = cx!(self);
            self.se.get_type_size_in_bits(&cx, ty)
        };
        // An existing canonical IV of a suitable type.
        let mut canonical = self
            .canonical_induction_variable(header)
            .filter(
                |&pn| {
                    let pn_ty = self.scev_ty_of(self.result_of(pn));
                    let cx = cx!(self);
                    self.se.get_type_size_in_bits(&cx, pn_ty) >= ty_bits
                },
            );
        // A narrower addrec is rewritten in terms of the wider canonical IV.
        if let Some(pn) = canonical {
            let pn_ty = self.scev_ty_of(self.result_of(pn));
            let pn_bits = {
                let cx = cx!(self);
                self.se.get_type_size_in_bits(&cx, pn_ty)
            };
            if pn_bits > ty_bits && !self.se.type_of(s).is_pointer() {
                let new_ops: Vec<Use> = {
                    let cx = cx!(self);
                    ops.iter().map(|&op| Use::of(self.se.get_any_extend_expr(&cx, op, pn_ty))).collect()
                };
                let nw = self.se.get_no_wrap_flags(s, flags::NW);
                let wide = {
                    let cx = cx!(self);
                    self.se.get_add_rec_expr(&cx, new_ops, header, Flags { expr: nw, use_: 0 }).scev
                };
                let v = self.expand(wide);
                let here = self.x.insert_point.expect("an insert point");
                let at = match self.instruction_of(v) {
                    Some(inst) => self.find_insert_point_after(inst, here),
                    None => here,
                };
                let unknown = {
                    let cx = cx!(self);
                    self.se.get_unknown(&cx, v)
                };
                let narrow = {
                    let cx = cx!(self);
                    self.se.get_truncate_expr(&cx, Use::of(unknown), ty, 0)
                };
                return self.expand_at(narrow, at);
            }
        }
        // If `s` is expanded outside the loop, a matching LCSSA phi may do.
        if let Some(v) = self.try_to_reuse_lcssa_phi(s, header) {
            return v;
        }
        // `{X,+,F}` is `X + {0,+,F}`.
        let start = ops[0].scev;
        if self.se.as_constant(start).is_none_or(|c| !c.is_zero()) {
            if self.se.type_of(s).is_pointer() {
                let base = self.se.get_pointer_base(s);
                let start_v = self.expand(base);
                let rest = {
                    let cx = cx!(self);
                    self.se.remove_pointer_base(&cx, s)
                };
                return self.expand_add_to_gep(vec![rest], start_v);
            }
            let zero = {
                let cx = cx!(self);
                self.se.get_zero(&cx, ty)
            };
            let mut new_ops = ops.to_vec();
            new_ops[0] = Use::of(zero);
            let nw = self.se.get_no_wrap_flags(s, flags::NW);
            let rest = {
                let cx = cx!(self);
                self.se.get_add_rec_expr(&cx, new_ops, header, Flags { expr: nw, use_: 0 }).scev
            };
            // Expand the operands first, so the add does not fold them.
            let lhs_v = self.expand(start);
            let rhs_v = self.expand(rest);
            let (lhs, rhs) = {
                let cx = cx!(self);
                (self.se.get_unknown(&cx, lhs_v), self.se.get_unknown(&cx, rhs_v))
            };
            let sum = {
                let cx = cx!(self);
                self.se.get_add_expr(&cx, vec![Use::of(lhs), Use::of(rhs)], Flags::default(), 0).scev
            };
            return self.expand(sum);
        }
        // Without a canonical IV, make one.
        let pn = match canonical {
            Some(pn) => pn,
            None => {
                let made = self.make_canonical_iv(header, ty);
                canonical = Some(made);
                made
            }
        };
        let civ = self.result_of(pn);
        // `{0,+,1}` is the canonical IV.
        if self.se.as_constant(ops[1].scev).is_some_and(|c| c.is_one()) {
            return civ;
        }
        // `{0,+,F}` is `i * F`.
        let civ_ty = self.scev_ty_of(civ);
        let civ_scev = {
            let cx = cx!(self);
            self.se.get_unknown(&cx, civ)
        };
        let product = {
            let cx = cx!(self);
            let step = self.se.get_noop_or_any_extend(&cx, ops[1].scev, civ_ty);
            self.se.get_mul_expr(&cx, vec![Use::of(civ_scev), Use::of(step)], Flags::default(), 0).scev
        };
        let narrowed = {
            let cx = cx!(self);
            self.se.get_truncate_or_noop(&cx, product, ty)
        };
        let _ = canonical;
        self.expand(narrowed)
    }

    /// The canonical `{0,+,1}` phi of the loop: `indvar` and `indvar.next`.
    fn make_canonical_iv(
        &mut self,
        header: BlockId,
        ty: Ty,
    ) -> InstId {
        let ir_ty = self.ir_ty(ty);
        let zero = self.constant(ir_ty, 0);
        let one = self.constant(ir_ty, 1);
        let preds = self.function.predecessors(header);
        let l = self.env.loops.get_loop_for(header).expect("a loop");
        // A placeholder for each backedge value, filled once the phi exists.
        let operands: Vec<Operand> = preds.iter().flat_map(|&p| [zero, Operand::Block(p)]).collect();
        let phi = self.function.create_instruction(Opcode::Phi, ir_ty, operands, IrFlags::default(), None);
        let first = self.function.block(header).first().expect("a header");
        self.function.insert(phi, Position::Before(first)).expect("a placed phi");
        self.x.remember(phi);
        let value = self.result_of(phi);
        for (at, &pred) in preds.iter().enumerate() {
            if !l.contains(pred) {
                continue;
            }
            let terminator = self.function.terminator(pred).expect("a terminator");
            let add = self
                .function
                .create_instruction(Opcode::Binary(BinaryOp::Add), ir_ty, vec![value, one], IrFlags::default(), None);
            self.function.insert(add, Position::Before(terminator)).expect("a placed add");
            self.x.remember(add);
            let next = self.result_of(add);
            self.function.set_operand(phi, 2 * at, next);
        }
        phi
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1282
    /// `tryToReuseLCSSAPhi`: the value of `s` after the loop as a phi of the
    /// exit block plus a constant or an unknown.
    fn try_to_reuse_lcssa_phi(
        &mut self,
        s: Scev,
        header: BlockId,
    ) -> Option<Operand> {
        let l = self.env.loops.get_loop_for(header)?;
        let exit = l.get_exit_block(self.function)?;
        let here = self.x.insert_point.expect("an insert point");
        let insert_block = self.function.parent(here)?;
        let single = self.function.predecessors(exit);
        if single.len() != 1 || !self.env.tree.dominates(exit, insert_block) {
            return None;
        }
        let sty = self.se.type_of(s);
        for pn in self.phis_of(exit) {
            let pn_value = self.result_of(pn);
            let phi_ty = self.scev_ty_of(pn_value);
            let exit_scev = {
                let cx = cx!(self);
                self.se.get_scev(&cx, pn_value)
            };
            if !matches!(self.se.node(exit_scev).kind, Kind::AddRec(..)) {
                continue;
            }
            let address_matches = sty.is_integer() && phi_ty.is_pointer() && {
                let cx = cx!(self);
                self.se.get_type_size_in_bits(&cx, sty)
                    == self
                        .env
                        .layout
                        .pointer(match phi_ty {
                            Ty::Ptr(space) => space,
                            Ty::Int(_) => 0,
                        })
                        .index_bits
            };
            let diff = if address_matches {
                let addr = {
                    let cx = cx!(self);
                    self.se.get_ptr_to_addr_expr(&cx, exit_scev)
                };
                self.reusable_difference(s, addr)
            } else if sty == phi_ty {
                self.reusable_difference(s, exit_scev)
            } else {
                None
            };
            let Some(diff) = diff else { continue };
            let diff_v = self.expand(diff);
            let base = self.fixup_lcssa_form_for(pn_value);
            if phi_ty.is_pointer() {
                if sty.is_pointer() {
                    return Some(self.create_ptr_add(base, diff_v));
                }
                let int = self.scev_ir_ty_of(sty);
                let addr = self.create_cast_flags(CastOp::PtrToInt, base, int);
                return Some(self.create_add(addr, diff_v));
            }
            return Some(self.create_add(base, diff_v));
        }
        None
    }

    /// `CanReuse`: `s - exit` when it is `C + x`, `-1 * x` or a `ptrtoaddr` of
    /// a constant or an unknown.
    fn reusable_difference(
        &mut self,
        s: Scev,
        exit: Scev,
    ) -> Option<Scev> {
        if matches!(self.se.node(exit).kind, Kind::CouldNotCompute) {
            return None;
        }
        let diff = {
            let cx = cx!(self);
            self.se.get_minus_scev(&cx, Use::of(s), Use::of(exit), 0, 0)
        };
        let mut op = diff;
        if let Kind::Add(ops) = self.se.node(op).kind.clone() {
            if ops.len() == 2 && self.se.as_constant(ops[0].scev).is_some() {
                op = ops[1].scev;
            }
        }
        if let Kind::Mul(ops) = self.se.node(op).kind.clone() {
            if ops.len() == 2 && self.se.as_constant(ops[0].scev).is_some_and(|c| c.is_all_ones()) {
                op = ops[1].scev;
            }
        }
        if let Kind::PtrToAddr(inner) = self.se.node(op).kind.clone() {
            op = inner.scev;
        }
        matches!(self.se.node(op).kind, Kind::Constant(_) | Kind::Unknown(_)).then_some(diff)
    }

    fn scev_ir_ty_of(
        &mut self,
        ty: Ty,
    ) -> TypeId {
        self.ir_ty(ty)
    }

    /// `Builder.CreateAdd`, folded as `InstSimplifyFolder` does.
    pub(super) fn create_add(
        &mut self,
        lhs: Operand,
        rhs: Operand,
    ) -> Operand {
        self.create_binop(BinaryOp::Add, lhs, rhs)
    }

    /// `Builder.CreateBinOp`, folded as `InstSimplifyFolder` does.
    pub(super) fn create_binop(
        &mut self,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
    ) -> Operand {
        if let Some(folded) = llrm_mir::simplify::simplify_binop_operands(
            self.context,
            self.env.layout,
            self.function,
            op,
            IrFlags::default(),
            lhs,
            rhs,
        ) {
            return folded;
        }
        let ty = self.ty_of_operand(lhs);
        self.emit(Opcode::Binary(op), ty, vec![lhs, rhs], IrFlags::default())
    }

    fn create_cast_flags(
        &mut self,
        op: CastOp,
        v: Operand,
        to: TypeId,
    ) -> Operand {
        self.create_cast(op, v, to, IrFlags::default())
    }

    /// `Builder.CreatePtrAdd`: an `i8` GEP.
    pub(super) fn create_ptr_add(
        &mut self,
        base: Operand,
        offset: Operand,
    ) -> Operand {
        let i8 = self.context.types.int(8);
        let ty = self.ty_of_operand(base);
        self.emit(Opcode::GetElementPtr { source: i8 }, ty, vec![base, offset], IrFlags::default())
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L2468
    /// `fixupLCSSAFormFor`: `v` read at the insert point. MIR code is not
    /// generally in LCSSA form; a use outside the defining loop needs the
    /// phis `formLCSSAForInstructions` makes (`llrm-transforms`' `lcssa`).
    pub(super) fn fixup_lcssa_form_for(
        &mut self,
        v: Operand,
    ) -> Operand {
        let Some(def) = self.instruction_of(v) else { return v };
        let here = self.x.insert_point.expect("an insert point");
        let def_loop = self.function.parent(def).and_then(|b| self.env.loops.get_loop_for(b));
        let use_loop = self.function.parent(here).and_then(|b| self.env.loops.get_loop_for(b));
        let Some(def_loop) = def_loop else { return v };
        if use_loop
            .as_ref()
            .is_some_and(|u| u.get_header() == def_loop.get_header() || def_loop.contains(u.get_header()))
        {
            return v;
        }
        todo!("S6: formLCSSAForInstructions for a use outside the defining loop")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L929
    /// `expandIVInc`: `pn + step` (a GEP for a pointer phi), or `pn - step`.
    fn expand_iv_inc(
        &mut self,
        pn: Operand,
        step: Operand,
        use_subtract: bool,
    ) -> Operand {
        if self.is_pointer(pn) {
            return self.create_ptr_add(pn, step);
        }
        self.create_binop(if use_subtract { BinaryOp::Sub } else { BinaryOp::Add }, pn, step)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L788
    /// `getIVIncOperand`: the induction variable an increment reads.
    pub(super) fn get_iv_inc_operand(
        &self,
        inc: InstId,
        insert_pos: InstId,
        allow_scale: bool,
    ) -> Option<InstId> {
        if inc == insert_pos {
            return None;
        }
        let one = self.function.instruction(inc);
        let dominates = |def: Operand| match self.instruction_of(def) {
            Some(d) => self.env.tree.instruction_dominates(self.function, d, insert_pos),
            None => true,
        };
        match &one.opcode {
            Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) => {
                if self.instruction_of(one.operands[1]).is_none() || dominates(one.operands[1]) {
                    self.instruction_of(one.operands[0])
                } else {
                    None
                }
            }
            Opcode::Cast(CastOp::BitCast) => self.instruction_of(one.operands[0]),
            Opcode::GetElementPtr { source } => {
                for &op in &one.operands[1..] {
                    if matches!(op, Operand::Constant(_)) {
                        continue;
                    }
                    if self.instruction_of(op).is_some() && !dominates(op) {
                        return None;
                    }
                    if allow_scale {
                        continue;
                    }
                    // The GEPs SCEVExpander makes are over `i8`.
                    if self.context.types.int_bits(*source) != Some(8) {
                        return None;
                    }
                    break;
                }
                self.instruction_of(one.operands[0])
            }
            _ => None,
        }
    }

    fn may_have_side_effects(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.function.instruction(inst).opcode,
            Opcode::Store { .. } | Opcode::Load { volatile: true, .. } | Opcode::Call(_) | Opcode::Invoke(_)
        )
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L751
    /// `isNormalAddRecExprPHI`: `inc` reaches `pn` through simple increments.
    fn is_normal_add_rec_expr_phi(
        &self,
        pn: InstId,
        inc: InstId,
        header: BlockId,
    ) -> bool {
        let mut inc = inc;
        loop {
            let one = self.function.instruction(inc);
            if one.operands.is_empty()
                || matches!(one.opcode, Opcode::Phi)
                || matches!(one.opcode, Opcode::Cast(op) if op != CastOp::BitCast)
            {
                return false;
            }
            // Every other operand must dominate the increment's position.
            if let Some((l, pos)) = self.x.iv_inc_insert {
                if l == header {
                    for &op in &one.operands[1..] {
                        if let Some(d) = self.instruction_of(op) {
                            if !self.env.tree.instruction_dominates(self.function, d, pos) {
                                return false;
                            }
                        }
                    }
                }
            }
            let Some(next) = self.instruction_of(one.operands[0]) else { return false };
            if self.may_have_side_effects(next) {
                return false;
            }
            if next == pn {
                return true;
            }
            inc = next;
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L915
    /// `isExpandedAddRecExprPHI`: `inc` reaches `pn` through the increments
    /// `getAddRecExprPHILiterally` and `expandAddToGEP` make.
    fn is_expanded_add_rec_expr_phi(
        &self,
        pn: InstId,
        inc: InstId,
        header: BlockId,
    ) -> bool {
        let Some(l) = self.env.loops.get_loop_for(header) else { return false };
        let Some(preheader) = l.get_loop_preheader(self.function) else { return false };
        let pos = self.function.terminator(preheader).expect("a terminator");
        let mut oper = Some(inc);
        while let Some(current) = oper.and_then(|one| self.get_iv_inc_operand(one, pos, false)) {
            if current == pn {
                return true;
            }
            oper = Some(current);
        }
        false
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L946
    /// `canBeCheaplyTransformed`: `phi` truncated, or with its step inverted,
    /// is `requested`. The flag is whether the step is inverted.
    fn can_be_cheaply_transformed(
        &mut self,
        phi: Scev,
        requested: Scev,
    ) -> Option<bool> {
        let (phi_ty, req_ty) = (self.se.type_of(phi), self.se.type_of(requested));
        if phi_ty.is_pointer() || req_ty.is_pointer() {
            return None;
        }
        let (Ty::Int(phi_bits), Ty::Int(req_bits)) = (phi_ty, req_ty) else { return None };
        if req_bits > phi_bits {
            return None;
        }
        let truncated = {
            let cx = cx!(self);
            self.se.get_truncate_or_noop(&cx, phi, req_ty)
        };
        if !matches!(self.se.node(truncated).kind, Kind::AddRec(..)) {
            return None;
        }
        if truncated == requested {
            return Some(false);
        }
        // `{R,+,-1}` is `R - {0,+,1}`.
        let start = self.se.operands(requested)[0].scev;
        let inverted = {
            let cx = cx!(self);
            self.se.get_minus_scev(&cx, Use::of(start), Use::of(requested), 0, 0)
        };
        (inverted == truncated).then_some(true)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L979
    /// `IsIncrementNSW` and `IsIncrementNUW`: the add `ar + step` does not
    /// wrap, signed or unsigned, because extending before or after is equal.
    fn is_increment_no_wrap(
        &mut self,
        ar: Scev,
        signed: bool,
    ) -> bool {
        let Ty::Int(bits) = self.se.type_of(ar) else { return false };
        if bits * 2 > 128 {
            return false;
        }
        let wide = Ty::Int(bits * 2);
        let cx = cx!(self);
        let step = self.se.get_step_recurrence(&cx, ar);
        let extend = |se: &mut ScalarEvolution, op: Scev| {
            if signed {
                se.get_sign_extend_expr(&cx, Use::of(op), wide, 0)
            } else {
                se.get_zero_extend_expr(&cx, Use::of(op), wide, 0)
            }
        };
        let (step_wide, ar_wide) = (extend(self.se, step), extend(self.se, ar));
        let op_after_extend =
            self.se.get_add_expr(&cx, vec![Use::of(step_wide), Use::of(ar_wide)], Flags::default(), 0).scev;
        let sum = self.se.get_add_expr(&cx, vec![Use::of(ar), Use::of(step)], Flags::default(), 0).scev;
        let extend_after_op = extend(self.se, sum);
        extend_after_op == op_after_extend
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1010
    /// `getAddRecExprPHILiterally`: the phi of `normalized` in `l`, reused or
    /// made, with the type to truncate it to and whether to invert its step.
    fn get_add_rec_expr_phi_literally(
        &mut self,
        normalized: Scev,
        header: BlockId,
    ) -> (InstId, Option<Ty>, bool) {
        let l = self.env.loops.get_loop_for(header).expect("a loop");
        let latch = l.get_loop_latch(self.function);
        if let Some(latch) = latch {
            let mut found: Option<(InstId, Operand)> = None;
            let mut trunc_ty: Option<Ty> = None;
            let mut invert = false;
            let try_non_matching = self
                .x
                .iv_inc_insert
                .is_some_and(|(loop_header, _)| self.env.tree.properly_dominates(latch, loop_header));
            for pn in self.phis_of(header) {
                let value = self.result_of(pn);
                let ty = self.ty_of_operand(value);
                if !matches!(
                    self.context.types.get(ty),
                    MirType::Int(_) | MirType::Pointer(_)
                ) {
                    continue;
                }
                let phi_scev = {
                    let cx = cx!(self);
                    self.se.get_scev(&cx, value)
                };
                if !matches!(self.se.node(phi_scev).kind, Kind::AddRec(..)) {
                    continue;
                }
                let matching = phi_scev == normalized;
                if !matching && !try_non_matching {
                    continue;
                }
                let Some(temp_inc) = self.incoming_for(pn, latch).and_then(|v| self.instruction_of(v)) else {
                    continue;
                };
                let reusable = if self.x.lsr_mode {
                    self.is_expanded_add_rec_expr_phi(pn, temp_inc, header)
                } else {
                    self.is_normal_add_rec_expr_phi(pn, temp_inc, header)
                };
                if !reusable {
                    continue;
                }
                if matching {
                    found = Some((pn, self.result_of(temp_inc)));
                    trunc_ty = None;
                    invert = false;
                    break;
                }
                if trunc_ty.is_none() || invert {
                    if let Some(inverted) = self.can_be_cheaply_transformed(phi_scev, normalized) {
                        found = Some((pn, self.result_of(temp_inc)));
                        invert = inverted;
                        trunc_ty = Some(self.se.type_of(normalized));
                    }
                }
            }
            if let Some((pn, inc)) = found {
                self.x.remember(pn);
                if let Some(inc) = self.instruction_of(inc) {
                    self.x.remember(inc);
                    self.x.reused.insert(inc);
                }
                self.x.reused.insert(pn);
                return (pn, trunc_ty, invert);
            }
        }

        let saved_post_inc = std::mem::take(&mut self.x.post_inc_loops);
        let preheader =
            l.get_loop_preheader(self.function).expect("Can't expand add recurrences without a loop preheader!");
        let preheader_end = self.function.terminator(preheader).expect("a terminator");
        let start = self.se.operands(normalized)[0].scev;
        let start_v = self.expand_at(start, preheader_end);

        let mut step = {
            let cx = cx!(self);
            self.se.get_step_recurrence(&cx, normalized)
        };
        let expand_ty = self.se.type_of(normalized);
        // A negative non-constant stride is a subtract.
        let use_subtract = !expand_ty.is_pointer() && self.se.is_non_constant_negative(step);
        if use_subtract {
            let cx = cx!(self);
            step = self.se.get_negative_scev(&cx, step, 0);
        }
        let first = self.first_insertion_pt(header);
        let step_v = self.expand_at(step, first);
        let increment_nuw = !use_subtract && self.is_increment_no_wrap(normalized, false);
        let increment_nsw = !use_subtract && self.is_increment_no_wrap(normalized, true);

        let ir_ty = self.ir_ty(expand_ty);
        let preds = self.function.predecessors(header);
        let operands: Vec<Operand> = preds.iter().flat_map(|&p| [start_v, Operand::Block(p)]).collect();
        let phi = self.function.create_instruction(Opcode::Phi, ir_ty, operands, IrFlags::default(), None);
        let head = self.function.block(header).first().expect("a header");
        self.function.insert(phi, Position::Before(head)).expect("a placed phi");
        let pn = self.result_of(phi);
        for (at, &pred) in preds.iter().enumerate() {
            if !l.contains(pred) {
                continue;
            }
            let insert_pos = match self.x.iv_inc_insert {
                Some((loop_header, pos)) if loop_header == header => pos,
                _ => self.function.terminator(pred).expect("a terminator"),
            };
            let saved = self.x.insert_point;
            self.x.insert_point = Some(insert_pos);
            let inc = self.expand_iv_inc(pn, step_v, use_subtract);
            self.x.insert_point = saved;
            if let Some(inc_inst) = self.instruction_of(inc) {
                if matches!(self.function.instruction(inc_inst).opcode, Opcode::Binary(_)) {
                    let mut f = self.function.instruction(inc_inst).flags;
                    if increment_nuw {
                        f.insert(IrFlags::NUW);
                    }
                    if increment_nsw {
                        f.insert(IrFlags::NSW);
                    }
                    self.function.set_flags(inc_inst, f);
                }
            }
            self.function.set_operand(phi, 2 * at, inc);
        }
        self.x.post_inc_loops = saved_post_inc;
        self.x.remember(phi);
        self.x.inserted_ivs.push(phi);
        (phi, None, false)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1133
    /// `expandAddRecExprLiterally`.
    fn expand_add_rec_literally(
        &mut self,
        s: Scev,
    ) -> Operand {
        let Kind::AddRec(_, header) = self.se.node(s).kind.clone() else { unreachable!("an add recurrence") };
        // The expression before any post-increment adjustment.
        let post_inc = self.x.post_inc_loops.contains(&header);
        let normalized = if post_inc {
            let cx = cx!(self);
            self.se
                .normalize_for_post_inc_use(&cx, s, &[header], false)
                .expect("a normalization that is not checked for invertibility")
        } else {
            s
        };
        let (pn, trunc_ty, invert) = self.get_add_rec_expr_phi_literally(normalized, header);
        let mut result = if !post_inc { self.result_of(pn) } else { self.post_inc_result(s, normalized, pn, header) };
        if let Some(trunc_ty) = trunc_ty {
            let to = self.ir_ty(trunc_ty);
            if to != self.ty_of_operand(result) || invert {
                result = self.fixup_lcssa_form_for(result);
            }
            if to != self.ty_of_operand(result) {
                result = self.create_cast(CastOp::Trunc, result, to, IrFlags::default());
            }
            if invert {
                let start = self.se.operands(normalized)[0].scev;
                let start_v = self.expand(start);
                result = self.create_binop(BinaryOp::Sub, start_v, result);
            }
        }
        result
    }

    /// The post-incremented value of the phi `pn`: its increment from the
    /// latch, or a new one at the insert point where that does not dominate it.
    fn post_inc_result(
        &mut self,
        s: Scev,
        normalized: Scev,
        pn: InstId,
        header: BlockId,
    ) -> Operand {
        let l = self.env.loops.get_loop_for(header).expect("a loop");
        let latch = l.get_loop_latch(self.function).expect("PostInc mode requires a unique loop latch!");
        let mut result = self.incoming_for(pn, latch).expect("an increment from the latch");
        // A new use of the post-inc IV that is not poison safe keeps only the
        // flags SCEV has proved always hold.
        if let Some(inc) = self.instruction_of(result) {
            if matches!(self.function.instruction(inc).opcode, Opcode::Binary(_)) {
                let mut f = self.function.instruction(inc).flags;
                if self.se.get_no_wrap_flags(s, flags::NUW) == 0 {
                    f.remove(IrFlags::NUW);
                }
                if self.se.get_no_wrap_flags(s, flags::NSW) == 0 {
                    f.remove(IrFlags::NSW);
                }
                self.function.set_flags(inc, f);
            }
        }
        let here = self.x.insert_point.expect("an insert point");
        let dominates = match self.instruction_of(result) {
            Some(inc) => self.env.tree.instruction_dominates(self.function, inc, here),
            None => true,
        };
        if !dominates {
            // The post-inc expansion does not dominate this use: add another
            // increment at the insert point.
            let mut step = {
                let cx = cx!(self);
                self.se.get_step_recurrence(&cx, normalized)
            };
            let use_subtract = !self.se.type_of(s).is_pointer() && self.se.is_non_constant_negative(step);
            if use_subtract {
                let cx = cx!(self);
                step = self.se.get_negative_scev(&cx, step, 0);
            }
            let first = self.first_insertion_pt(header);
            let step_v = self.expand_at(step, first);
            result = self.expand_iv_inc(self.result_of(pn), step_v, use_subtract);
        }
        result
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L835
    /// `fixupInsertPoints`: the insert point past `inst`, which is moving.
    fn fixup_insert_points(
        &mut self,
        inst: InstId,
    ) {
        if self.x.insert_point == Some(inst) {
            let block = self.function.parent(inst).expect("placed");
            let all = self.function.block(block).instructions();
            let at = all.iter().position(|&one| one == inst).expect("in its block");
            self.x.insert_point = Some(all[at + 1]);
        }
    }

    /// `rememberFlags`: keep the first flags seen, for a cleanup to restore.
    fn remember_flags(
        &mut self,
        inst: InstId,
    ) {
        if self.x.orig_flags.get(&inst).is_none() {
            let flags = self.function.instruction(inst).flags;
            self.x.orig_flags.insert(inst, flags);
        }
    }

    /// `FixupPoisonFlags` of `hoistIVInc`: drop the flags inferred from the old
    /// context and infer them again in the new one.
    fn fixup_poison_flags(
        &mut self,
        inst: InstId,
    ) {
        self.remember_flags(inst);
        let mut f = self.function.instruction(inst).flags;
        for flag in [
            IrFlags::NUW,
            IrFlags::NSW,
            IrFlags::EXACT,
            IrFlags::DISJOINT,
            IrFlags::NNEG,
            IrFlags::SAMESIGN,
            IrFlags::INBOUNDS,
            IrFlags::NUSW,
        ] {
            f.remove(flag);
        }
        if let Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)) =
            self.function.instruction(inst).opcode
        {
            // `getStrengthenedNoWrapFlagsFromBinOp`, over the flags it had.
            let was = self.function.instruction(inst).flags;
            let operands = self.function.instruction(inst).operands.clone();
            let (lhs, rhs) = {
                let cx = cx!(self);
                (self.se.get_scev(&cx, operands[0]), self.se.get_scev(&cx, operands[1]))
            };
            let mut deduced = false;
            let mut nuw = was.contains(IrFlags::NUW);
            let mut nsw = was.contains(IrFlags::NSW);
            if !(nuw && nsw) {
                let cx = cx!(self);
                if !nuw && self.se.will_not_overflow(&cx, op, false, lhs, rhs, Some(inst)) {
                    nuw = true;
                    deduced = true;
                }
                if !nsw && self.se.will_not_overflow(&cx, op, true, lhs, rhs, Some(inst)) {
                    nsw = true;
                    deduced = true;
                }
            }
            if deduced {
                if nuw {
                    f.insert(IrFlags::NUW);
                }
                if nsw {
                    f.insert(IrFlags::NSW);
                }
            }
        }
        self.function.set_flags(inst, f);
    }

    /// `LoopInfo::movementPreservesLCSSAForm`.
    fn movement_preserves_lcssa_form(
        &self,
        inst: InstId,
        new_loc: InstId,
    ) -> bool {
        let (old_bb, new_bb) =
            (self.function.parent(inst).expect("placed"), self.function.parent(new_loc).expect("placed"));
        if old_bb == new_bb {
            return true;
        }
        let loops = self.env.loops;
        let (old_loop, new_loop) =
            (loops.get_loop_for(old_bb).map(|l| l.get_header()), loops.get_loop_for(new_bb).map(|l| l.get_header()));
        if old_loop == new_loop {
            return true;
        }
        // The null loop is the outermost.
        let contains = |outer: Option<BlockId>, inner: Option<BlockId>| match (outer, inner) {
            (None, _) => true,
            (Some(o), Some(i)) => loops.get_loop_for(o).is_some_and(|l| l.contains(i)),
            (Some(_), None) => false,
        };
        // Hoisting out of an inner loop: the uses of `inst` need no check.
        if !contains(new_loop, old_loop) {
            if let Some(result) = self.function.instruction(inst).result {
                for u in self.function.users(result) {
                    let user = self.function.instruction(u.user);
                    let use_bb = if matches!(user.opcode, Opcode::Phi) {
                        match user.operands[u.index as usize | 1] {
                            Operand::Block(b) => b,
                            _ => return false,
                        }
                    } else {
                        self.function.parent(u.user).expect("placed")
                    };
                    let use_loop = loops.get_loop_for(use_bb).map(|l| l.get_header());
                    if use_bb != new_bb && use_loop != new_loop {
                        return false;
                    }
                }
            }
        }
        // Sinking into an inner loop: the operands of `inst` need no check.
        if !contains(old_loop, new_loop) {
            if matches!(self.function.instruction(inst).opcode, Opcode::Phi) {
                return false;
            }
            for &op in &self.function.instruction(inst).operands {
                let Some(def) = self.instruction_of(op) else { return false };
                let def_bb = self.function.parent(def).expect("placed");
                if def_bb != new_bb && loops.get_loop_for(def_bb).map(|l| l.get_header()) != new_loop {
                    return false;
                }
            }
        }
        true
    }

    /// `hoistIVInc`.
    pub(super) fn hoist_iv_inc(
        &mut self,
        inc: InstId,
        insert_pos: InstId,
        recompute_poison_flags: bool,
    ) -> bool {
        let dominates = |run: &Self, def: InstId| run.env.tree.instruction_dominates(run.function, def, insert_pos);
        if dominates(self, inc) {
            if recompute_poison_flags {
                self.fixup_poison_flags(inc);
            }
            return true;
        }
        // `insert_pos` must itself dominate `inc`, so that `inc`'s new
        // position satisfies its existing users.
        let pos_block = self.function.parent(insert_pos).expect("placed");
        let inc_block = self.function.parent(inc).expect("placed");
        if matches!(self.function.instruction(insert_pos).opcode, Opcode::Phi)
            || !self.env.tree.dominates(pos_block, inc_block)
        {
            return false;
        }
        if !self.movement_preserves_lcssa_form(inc, insert_pos) {
            return false;
        }
        // The chain of IV operands leading back to the phi must be hoistable.
        let mut ivincs = Vec::new();
        let mut current = inc;
        loop {
            let Some(oper) = self.get_iv_inc_operand(current, insert_pos, true) else { return false };
            ivincs.push(current);
            current = oper;
            if dominates(self, current) {
                break;
            }
        }
        for &one in ivincs.iter().rev() {
            self.fixup_insert_points(one);
            self.function.move_to(one, Position::Before(insert_pos)).expect("a placed position");
            if recompute_poison_flags {
                self.fixup_poison_flags(one);
            }
        }
        true
    }
}
