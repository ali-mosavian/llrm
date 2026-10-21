//! `SCEVExpander`'s bodies (`ScalarEvolutionExpander.cpp`): a SCEV written as
//! instructions before the insert point.
//!
//! # How it works
//! A [`Run`] is one call: the expander's state, the SCEV, the context, the
//! function and the declarations it may add. `expand` hoists the insert point
//! out of every loop the SCEV is invariant in, reuses an earlier expansion at
//! that point, and visits the node; each visitor expands its operands first.
//! Instructions are folded as `InstSimplifyFolder` folds them, and a binop
//! equal to one of the six before the insert point is reused.
//!
//! # Example
//! `(%a + (4 * %b))`, expanded in a loop body whose operands are invariant,
//! lands in the preheader: `%0 = shl %b, 2` and `%1 = add %a, %0`.
//!
//! # Used by
//! [`super::expander::Expander`], and so LoopIdiomRecognize and LSR.
//!
//! # See also
//! `ScalarEvolutionExpander.h`.

use std::cmp::Ordering;

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags as IrFlags, IntPredicate, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::types::{Type as MirType, TypeId};

use super::expander::{Env, Expander};
use super::expr::{Kind, Scev, Ty, Type, Use, flags};
use super::{Flags, ScalarEvolution};

/// One call of the expander.
pub(super) struct Run<'a> {
    pub x: &'a mut Expander,
    pub env: Env<'a>,
    pub context: &'a mut Context,
    pub declared: &'a mut Declared,
    pub function: &'a mut Function,
    pub se: &'a mut ScalarEvolution,
}

/// The [`Cx`] over the run's function as it stands.
macro_rules! cx {
    ($run:expr) => {
        $run.env.cx(&*$run.context, &*$run.function)
    };
}

impl Run<'_> {
    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1498
    /// `expandCodeFor(SH, Ty)`.
    pub fn expand_code_for(
        &mut self,
        s: Scev,
        ty: Option<Ty>,
    ) -> Operand {
        let v = self.expand(s);
        match ty {
            Some(ty) => {
                let to = self.ir_ty(ty);
                self.insert_noop_cast_of_to(v, to)
            }
            None => v,
        }
    }

    pub(super) fn ir_ty(
        &mut self,
        ty: Ty,
    ) -> TypeId {
        match ty {
            Ty::Int(bits) => self.context.types.int(bits),
            Ty::Ptr(space) => self.context.types.ptr(space),
        }
    }

    pub(super) fn ty_of_operand(
        &self,
        v: Operand,
    ) -> TypeId {
        match v {
            Operand::Value(value) => self.function.value(value).ty,
            Operand::Constant(constant) => self.context.get(constant).ty,
            Operand::Block(_) => unreachable!("a block is not a value"),
        }
    }

    pub(super) fn is_pointer(
        &self,
        v: Operand,
    ) -> bool {
        matches!(
            self.context.types.get(self.ty_of_operand(v)),
            MirType::Pointer(_)
        )
    }

    pub(super) fn constant(
        &mut self,
        ty: TypeId,
        value: i128,
    ) -> Operand {
        Operand::Constant(self.context.int(ty, value))
    }

    fn is_constant(
        &self,
        v: Operand,
    ) -> bool {
        matches!(v, Operand::Constant(_))
    }

    /// `rememberInstruction` and the builder's insert: `inst` goes before the
    /// insert point.
    pub(super) fn emit(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: IrFlags,
    ) -> Operand {
        let at = self.x.insert_point.expect("an insert point");
        let inst = self.function.create_instruction(opcode, ty, operands, flags, None);
        self.function.insert(inst, Position::Before(at)).expect("a placed position");
        self.x.remember(inst);
        Operand::Value(self.function.instruction(inst).result.expect("a value"))
    }

    /// `Builder.CreateCast`, folded as `InstSimplifyFolder` does.
    pub(super) fn create_cast(
        &mut self,
        op: CastOp,
        v: Operand,
        to: TypeId,
        flags: IrFlags,
    ) -> Operand {
        if flags.is_empty() {
            if let Some(folded) =
                llrm_mir::simplify::simplify_cast_operand(self.context, self.env.layout, self.function, op, v, to)
            {
                return folded;
            }
        }
        self.emit(Opcode::Cast(op), to, vec![v], flags)
    }

    /// `Builder.CreateIntrinsic(llvm.<name>, {ty}, {lhs, rhs})`.
    fn create_binary_intrinsic(
        &mut self,
        name: &str,
        lhs: Operand,
        rhs: Operand,
    ) -> Operand {
        let ty = self.ty_of_operand(lhs);
        let MirType::Int(bits) = *self.context.types.get(ty) else { unreachable!("an integer intrinsic") };
        let function_type =
            self.context.types.intern(MirType::Function { returns: ty, parameters: vec![ty, ty], variadic: false });
        let callee = self.declared.declare(&format!("llvm.{name}.i{bits}"), function_type);
        let pointer = self.context.types.ptr(0);
        let callee = Operand::Constant(self.context.constant(llrm_mir::context::Constant {
            ty: pointer,
            kind: llrm_mir::context::ConstantKind::Global(callee),
        }));
        let info = CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 2],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        self.emit(Opcode::Call(Box::new(info)), ty, vec![lhs, rhs, callee], IrFlags::default())
    }

    /// The instruction after the last of `inst`'s block's phis that follows
    /// `inst`, past what this expander inserted: `findInsertPointAfter`.
    pub(super) fn find_insert_point_after(
        &self,
        inst: InstId,
        must_dominate: InstId,
    ) -> InstId {
        let block = self.function.parent(inst).expect("a placed instruction");
        self.function
            .iter_block(block)
            .skip_while(|&one| one != inst)
            .skip(1)
            .skip_while(|&one| matches!(self.function.instruction(one).opcode, Opcode::Phi))
            .find(|&one| one == must_dominate || !self.x.is_inserted_instruction(&one))
            .expect("in its block")
    }

    /// `getFirstInsertionPt` of `block`.
    pub(super) fn first_insertion_pt(
        &self,
        block: BlockId,
    ) -> InstId {
        self.function
            .iter_block(block)
            .find(|&one| !matches!(self.function.instruction(one).opcode, Opcode::Phi))
            .expect("a terminator")
    }

    /// Whether `v` is defined outside the loop with header `header`:
    /// `Loop::isLoopInvariant(V)`.
    fn is_invariant(
        &self,
        header: BlockId,
        v: Operand,
    ) -> bool {
        let Operand::Value(value) = v else { return true };
        match self.function.value(value).def {
            ValueDef::Argument(_) => true,
            ValueDef::Instruction(inst) => {
                let Some(block) = self.function.parent(inst) else { return true };
                !self.env.loops.get_loop_for(header).is_some_and(|l| l.contains(block))
            }
        }
    }

    /// Hoists the insert point out of every loop `operands` are invariant in:
    /// the loop at `block`, then its parents.
    fn hoisted_insert_point(
        &self,
        at: InstId,
        operands: &[Operand],
    ) -> InstId {
        let mut at = at;
        while let Some(block) = self.function.parent(at) {
            let Some(l) = self.env.loops.get_loop_for(block) else { break };
            if !operands.iter().all(|&v| self.is_invariant(l.get_header(), v)) {
                break;
            }
            let Some(preheader) = l.get_loop_preheader(self.function) else { break };
            at = self.function.terminator(preheader).expect("a terminator");
        }
        at
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L236
    /// `InsertBinop`.
    fn insert_binop(
        &mut self,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        wrap: u8,
        safe_to_hoist: bool,
    ) -> Operand {
        let mut ir_flags = IrFlags::default();
        if wrap & flags::NUW != 0 {
            ir_flags.insert(IrFlags::NUW);
        }
        if wrap & flags::NSW != 0 {
            ir_flags.insert(IrFlags::NSW);
        }
        // Fold a binop with constant operands.
        if self.is_constant(lhs) && self.is_constant(rhs) {
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
        }
        // Scan the six instructions before the insert point for this binop.
        let ip = self.x.insert_point.expect("an insert point");
        let block = self.function.parent(ip).expect("a placed insert point");
        let before_ip = self.function.iter_block_rev(block).skip_while(|&one| one != ip).skip(1).take(6);
        for before in before_ip {
            let one = self.function.instruction(before);
            let overflowing = matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl
            );
            let incompatible = (overflowing
                && (one.flags.contains(IrFlags::NSW) != (wrap & flags::NSW != 0)
                    || one.flags.contains(IrFlags::NUW) != (wrap & flags::NUW != 0)))
                || one.flags.contains(IrFlags::EXACT);
            if one.opcode == Opcode::Binary(op) && one.operands == [lhs, rhs] && !incompatible {
                return Operand::Value(one.result.expect("a value"));
            }
        }
        let saved = ip;
        if safe_to_hoist {
            let hoisted = self.hoisted_insert_point(ip, &[lhs, rhs]);
            self.x.insert_point = Some(hoisted);
        }
        // Post-inc rewrites made outside their loop keep the instructions
        // they asked for; the rest go through `InstSimplifyFolder`.
        let rewrite = self.x.lsr_mode && !self.x.post_inc_loops.is_empty() && {
            let block = self.function.parent(self.x.insert_point.expect("an insert point")).expect("placed");
            self.x
                .post_inc_loops
                .iter()
                .all(|&header| !self.env.loops.get_loop_for(header).is_some_and(|l| l.contains(block)))
        };
        let folded = (!rewrite)
            .then(|| {
                llrm_mir::simplify::simplify_binop_operands(
                    self.context,
                    self.env.layout,
                    self.function,
                    op,
                    IrFlags::default(),
                    lhs,
                    rhs,
                )
            })
            .flatten();
        let made = match folded {
            Some(folded) => folded,
            None => {
                let ty = self.ty_of_operand(lhs);
                self.emit(Opcode::Binary(op), ty, vec![lhs, rhs], ir_flags)
            }
        };
        self.x.insert_point = Some(saved);
        made
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L170
    /// `InsertNoopCastOfTo`: `v` as `to`, a cast that changes no bits.
    fn insert_noop_cast_of_to(
        &mut self,
        v: Operand,
        to: TypeId,
    ) -> Operand {
        let from = self.ty_of_operand(v);
        if from == to {
            return v;
        }
        let op = match (self.context.types.get(from), self.context.types.get(to)) {
            (MirType::Pointer(_), MirType::Int(_)) => CastOp::PtrToInt,
            (MirType::Int(_), MirType::Pointer(_)) => CastOp::IntToPtr,
            _ => CastOp::BitCast,
        };
        if let Some(folded) =
            llrm_mir::simplify::simplify_cast_operand(self.context, self.env.layout, self.function, op, v, to)
        {
            return folded;
        }
        // A cast of a cast back to the original type is the original.
        if let Operand::Value(value) = v {
            if let ValueDef::Instruction(inst) = self.function.value(value).def {
                let one = self.function.instruction(inst);
                if matches!(one.opcode, Opcode::Cast(CastOp::PtrToInt | CastOp::IntToPtr)) {
                    if self.ty_of_operand(one.operands[0]) == to {
                        return one.operands[0];
                    }
                }
            }
        }
        let ip = self.cast_insert_point(v);
        self.reuse_or_create_cast(v, to, op, ip)
    }

    /// `GetOptimalInsertionPointForCastOf`.
    fn cast_insert_point(
        &self,
        v: Operand,
    ) -> InstId {
        let at = self.x.insert_point.expect("an insert point");
        let Operand::Value(value) = v else { return at };
        match self.function.value(value).def {
            ValueDef::Instruction(inst) => self.find_insert_point_after(inst, at),
            ValueDef::Argument(_) => {
                // After the casts of other arguments at the start of the entry.
                let entry = self.function.entry().expect("an entry");
                self.function
                    .iter_block(entry)
                    .find(|&one| {
                        let inst = self.function.instruction(one);
                        let argument_cast = matches!(inst.opcode, Opcode::Cast(CastOp::BitCast))
                            && matches!(
                                inst.operands[0],
                                Operand::Value(o) if matches!(self.function.value(o).def, ValueDef::Argument(_)) && inst.operands[0] != v
                            );
                        !matches!(inst.opcode, Opcode::Phi) && !argument_cast
                    })
                    .expect("a terminator")
            }
        }
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L77
    /// `ReuseOrCreateCast`.
    fn reuse_or_create_cast(
        &mut self,
        v: Operand,
        to: TypeId,
        op: CastOp,
        ip: InstId,
    ) -> Operand {
        if let Operand::Value(value) = v {
            let block = self.function.parent(ip);
            for user in self.function.users(value) {
                let one = self.function.instruction(user.user);
                if one.ty != to || one.opcode != Opcode::Cast(op) || self.function.parent(user.user) != block {
                    continue;
                }
                // At or before the insert point, and not the builder's own.
                if (user.user == ip || self.function.comes_before(user.user, ip))
                    && Some(user.user) != self.x.insert_point
                {
                    return Operand::Value(one.result.expect("a value"));
                }
            }
        }
        let saved = self.x.insert_point;
        self.x.insert_point = Some(ip);
        let made = self.emit(Opcode::Cast(op), to, vec![v], IrFlags::default());
        self.x.insert_point = saved;
        made
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L303
    /// `expandAddToGEP`: `base + sum(offsets)` as an `i8` GEP.
    pub(super) fn expand_add_to_gep(
        &mut self,
        offsets: Vec<Scev>,
        base: Operand,
    ) -> Operand {
        let sum = {
            let cx = cx!(self);
            let ops = offsets.into_iter().map(Use::of).collect();
            self.se.get_add_expr(&cx, ops, Flags::default(), 0).scev
        };
        let idx = self.expand(sum);
        let ptr_ty = self.ty_of_operand(base);
        let i8 = self.context.types.int(8);
        let gep = Opcode::GetElementPtr { source: i8 };
        // Fold a GEP with constant operands.
        if self.is_constant(base) && self.is_constant(idx) {
            if let Some(folded) = self.fold_constant_gep(base, idx, ptr_ty) {
                return folded;
            }
        }
        // Scan the six instructions before the insert point for this GEP.
        let ip = self.x.insert_point.expect("an insert point");
        let block = self.function.parent(ip).expect("placed");
        let before_ip = self.function.iter_block_rev(block).skip_while(|&one| one != ip).skip(1).take(6);
        for before in before_ip {
            let one = self.function.instruction(before);
            if one.opcode == gep && one.operands == [base, idx] && one.flags.is_empty() {
                return Operand::Value(one.result.expect("a value"));
            }
        }
        let saved = ip;
        let hoisted = self.hoisted_insert_point(ip, &[base, idx]);
        self.x.insert_point = Some(hoisted);
        let made = self.emit(gep, ptr_ty, vec![base, idx], IrFlags::default());
        self.x.insert_point = Some(saved);
        made
    }

    /// A GEP of constants, where the context can write it as a constant.
    fn fold_constant_gep(
        &mut self,
        _base: Operand,
        _idx: Operand,
        _ty: TypeId,
    ) -> Option<Operand> {
        None
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1424
    /// `PickMostRelevantLoop`.
    fn pick_most_relevant_loop(
        &self,
        a: Option<BlockId>,
        b: Option<BlockId>,
    ) -> Option<BlockId> {
        let (Some(x), Some(y)) = (a, b) else { return a.or(b) };
        let loops = self.env.loops;
        let contains = |outer: BlockId, inner: BlockId| loops.get_loop_for(outer).is_some_and(|l| l.contains(inner));
        if contains(x, y) {
            return b;
        }
        if contains(y, x) {
            return a;
        }
        if self.env.tree.dominates(x, y) {
            return b;
        }
        if self.env.tree.dominates(y, x) {
            return a;
        }
        a
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1450
    /// `getRelevantLoop`.
    fn get_relevant_loop(
        &mut self,
        s: Scev,
    ) -> Option<BlockId> {
        if let Some(&found) = self.x.relevant_loops.get(&s) {
            return found;
        }
        let kind = self.se.node(s).kind.clone();
        let found = match kind {
            Kind::Constant(_) | Kind::CouldNotCompute => None,
            Kind::Unknown(Operand::Value(value)) => match self.function.value(value).def {
                ValueDef::Instruction(inst) => {
                    let block = self.function.parent(inst)?;
                    self.env.loops.get_loop_for(block).map(|l| l.get_header())
                }
                ValueDef::Argument(_) => None,
            },
            Kind::Unknown(_) => None,
            Kind::Truncate(op) | Kind::ZeroExtend(op) | Kind::SignExtend(op) | Kind::PtrToAddr(op) => {
                self.get_relevant_loop(op.scev)
            }
            Kind::UDiv(l, r) => {
                let (l, r) = (self.get_relevant_loop(l.scev), self.get_relevant_loop(r.scev));
                self.pick_most_relevant_loop(l, r)
            }
            Kind::AddRec(ops, header) => {
                let mut found = Some(header);
                for op in ops {
                    let l = self.get_relevant_loop(op.scev);
                    found = self.pick_most_relevant_loop(found, l);
                }
                found
            }
            Kind::Add(ops)
            | Kind::Mul(ops)
            | Kind::UMax(ops)
            | Kind::SMax(ops)
            | Kind::UMin(ops)
            | Kind::SMin(ops)
            | Kind::SequentialUMin(ops) => {
                let mut found = None;
                for op in ops {
                    let l = self.get_relevant_loop(op.scev);
                    found = self.pick_most_relevant_loop(found, l);
                }
                found
            }
        };
        self.x.relevant_loops.insert(s, found);
        found
    }

    /// `LoopCompare`: `lhs` sorts before `rhs`.
    fn loop_less(
        &self,
        lhs: (Option<BlockId>, Scev),
        rhs: (Option<BlockId>, Scev),
    ) -> bool {
        // Pointer operands stay before the others.
        let (lp, rp) = (self.se.type_of(lhs.1).is_pointer(), self.se.type_of(rhs.1).is_pointer());
        if lp != rp {
            return lp;
        }
        if lhs.0 != rhs.0 {
            return self.pick_most_relevant_loop(lhs.0, rhs.0) != lhs.0;
        }
        // A non-constant negative goes right, so a `sub` can replace the
        // negate and add.
        if self.se.is_non_constant_negative(lhs.1) {
            if !self.se.is_non_constant_negative(rhs.1) {
                return false;
            }
        } else if self.se.is_non_constant_negative(rhs.1) {
            return true;
        }
        false
    }

    /// The operands in reverse with their loops, stably sorted by
    /// `LoopCompare`.
    fn ops_and_loops(
        &mut self,
        ops: &[Use],
    ) -> Vec<(Option<BlockId>, Scev)> {
        let mut all: Vec<_> = ops.iter().rev().map(|op| (self.get_relevant_loop(op.scev), op.scev)).collect();
        all.sort_by(|&a, &b| {
            if self.loop_less(a, b) {
                Ordering::Less
            } else if self.loop_less(b, a) {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        });
        all
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1688
    /// `expand`: `s` before the insert point, hoisted out of the loops it is
    /// invariant in.
    pub fn expand(
        &mut self,
        s: Scev,
    ) -> Operand {
        let here = self.x.insert_point.expect("an insert point");
        let mut at = here;
        let mut l = self.function.parent(here).and_then(|b| self.env.loops.get_loop_for(b)).map(|l| l.get_header());
        loop {
            let invariant = {
                let cx = cx!(self);
                self.se.is_loop_invariant(&cx, s, l)
            };
            if invariant {
                let Some(header) = l else { break };
                let cx = cx!(self);
                let l_ref = self.env.loops.get_loop_for(header).expect("a loop");
                if let Some(preheader) = l_ref.get_loop_preheader(cx.function) {
                    at = self.function.terminator(preheader).expect("a terminator");
                    break;
                }
                // LSR puts the insert point of an addrec's start and step at
                // the start of the block; correct it to a valid position.
                at = self.first_insertion_pt(header);
            } else {
                // Not in a loop it is expanded post-incremented for: its
                // increment is below the header's start, and
                // does not dominate it.
                let computable = {
                    let cx = cx!(self);
                    l.is_some_and(|header| !self.x.post_inc_loops.contains(&header))
                        && self.se.has_computable_loop_evolution(&cx, s, l)
                };
                if computable {
                    at = self.first_insertion_pt(l.expect("a loop"));
                }
                while at != here && self.x.is_inserted_instruction(&at) {
                    let block = self.function.parent(at).expect("placed");
                    at = self.function.iter_block(block).skip_while(|&one| one != at).nth(1).expect("in its block");
                }
                break;
            }
            l = l.and_then(|header| self.env.loops.get_loop_for(header)?.get_parent_loop()).map(|p| p.get_header());
        }
        if let Some(&found) = self.x.expressions.get(&(s, at)) {
            return found;
        }
        self.x.insert_point = Some(at);
        let v = self.visit(s);
        let v = self.fixup_lcssa_form_for(v);
        self.x.insert_point = Some(here);
        self.x.expressions.insert((s, at), v);
        v
    }

    fn visit(
        &mut self,
        s: Scev,
    ) -> Operand {
        let kind = self.se.node(s).kind.clone();
        match kind {
            Kind::Constant(value) => {
                let ty = self.context.types.int(value.width);
                self.constant(ty, value.sext_value())
            }
            Kind::Unknown(v) => v,
            Kind::Truncate(op) => {
                let v = self.expand(op.scev);
                let to = self.scev_ir_ty(s);
                self.create_cast(CastOp::Trunc, v, to, IrFlags::default())
            }
            Kind::ZeroExtend(op) => {
                let v = self.expand(op.scev);
                let to = self.scev_ir_ty(s);
                let nonneg = {
                    let cx = cx!(self);
                    self.se.is_known_non_negative(&cx, op.scev)
                };
                let flags = if nonneg { IrFlags::NNEG } else { IrFlags::default() };
                self.create_cast(CastOp::ZExt, v, to, flags)
            }
            Kind::SignExtend(op) => {
                let v = self.expand(op.scev);
                let to = self.scev_ir_ty(s);
                self.create_cast(CastOp::SExt, v, to, IrFlags::default())
            }
            // The address of a pointer is its integer: MIR has no `ptrtoaddr`,
            // and the index width is the address width here.
            Kind::PtrToAddr(op) => {
                let v = self.expand(op.scev);
                let to = self.scev_ir_ty(s);
                let ip = self.cast_insert_point(v);
                self.reuse_or_create_cast(v, to, CastOp::PtrToInt, ip)
            }
            Kind::Add(ops) => self.visit_add(s, &ops),
            Kind::Mul(ops) => self.visit_mul(s, &ops),
            Kind::UDiv(l, r) => self.visit_udiv(l.scev, r.scev),
            Kind::UMax(ops) => self.expand_min_max(s, &ops, false),
            Kind::SMax(ops) => self.expand_min_max(s, &ops, false),
            Kind::UMin(ops) => self.expand_min_max(s, &ops, false),
            Kind::SMin(ops) => self.expand_min_max(s, &ops, false),
            Kind::SequentialUMin(ops) => self.expand_min_max(s, &ops, true),
            Kind::AddRec(ops, header) => self.visit_add_rec(s, &ops, header),
            Kind::CouldNotCompute => unreachable!("Attempt to use a SCEVCouldNotCompute object!"),
        }
    }

    pub(super) fn scev_ir_ty(
        &mut self,
        s: Scev,
    ) -> TypeId {
        let ty = self.se.type_of(s);
        self.ir_ty(ty)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1084
    /// `visitAddExpr`.
    fn visit_add(
        &mut self,
        s: Scev,
        ops: &[Use],
    ) -> Operand {
        let wrap = self.se.get_no_wrap_flags(s, flags::NUW | flags::NSW);
        let all = self.ops_and_loops(ops);
        let mut sum: Option<Operand> = None;
        let mut i = 0;
        while i < all.len() {
            let (current, op) = all[i];
            let Some(so_far) = sum else {
                sum = Some(self.expand(op));
                i += 1;
                continue;
            };
            if self.is_pointer(so_far) {
                // A pointer sum: the operands of this loop form one GEP. An
                // unknown that is not an instruction is not looked through,
                // for want of `getSCEV` (S3).
                let mut offsets = Vec::new();
                while i < all.len() && all[i].0 == current {
                    offsets.push(all[i].1);
                    i += 1;
                }
                sum = Some(self.expand_add_to_gep(offsets, so_far));
            } else if self.se.is_non_constant_negative(op) {
                // A subtract instead of a negate and an add.
                let negated = {
                    let cx = cx!(self);
                    self.se.get_negative_scev(&cx, op, 0)
                };
                let w = self.expand(negated);
                sum = Some(self.insert_binop(BinaryOp::Sub, so_far, w, flags::NONE, true));
                i += 1;
            } else {
                let w = self.expand(op);
                // A constant goes to the right.
                let (a, b) = if self.is_constant(so_far) { (w, so_far) } else { (so_far, w) };
                sum = Some(self.insert_binop(BinaryOp::Add, a, b, wrap, true));
                i += 1;
            }
        }
        sum.expect("an add has operands")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1176
    /// `visitMulExpr`.
    fn visit_mul(
        &mut self,
        s: Scev,
        ops: &[Use],
    ) -> Operand {
        let mut wrap = self.se.get_no_wrap_flags(s, flags::NUW | flags::NSW);
        let all = self.ops_and_loops(ops);
        let mut prod: Option<Operand> = None;
        let mut i = 0;
        // `X pow N` as the product of `X pow P` for each power of two `P` in
        // `N`, over the run of equal operands at `i`.
        let pow_n = |run: &mut Self, i: &mut usize| -> Operand {
            let mut end = *i;
            while end < all.len() && all[end] == all[*i] {
                end += 1;
            }
            let exponent = end - *i;
            let mut p = run.expand(all[*i].1);
            let mut result = if exponent & 1 != 0 { Some(p) } else { None };
            let mut bin = 2;
            while bin <= exponent {
                p = run.insert_binop(BinaryOp::Mul, p, p, flags::NONE, true);
                if exponent & bin != 0 {
                    result = Some(match result {
                        Some(r) => run.insert_binop(BinaryOp::Mul, r, p, flags::NONE, true),
                        None => p,
                    });
                }
                bin <<= 1;
            }
            *i = end;
            result.expect("something was expanded")
        };
        while i < all.len() {
            let Some(so_far) = prod else {
                prod = Some(pow_n(self, &mut i));
                continue;
            };
            let is_all_ones = self.se.as_constant(all[i].1).is_some_and(|c| c.is_all_ones());
            if is_all_ones {
                // A negate instead of a multiply by -1.
                let ty = self.ty_of_operand(so_far);
                let zero = self.constant(ty, 0);
                prod = Some(self.insert_binop(BinaryOp::Sub, zero, so_far, flags::NONE, true));
                i += 1;
            } else {
                let w = pow_n(self, &mut i);
                let (a, b) = if self.is_constant(so_far) { (w, so_far) } else { (so_far, w) };
                let power = match b {
                    Operand::Constant(c) => match self.context.get(c).kind {
                        llrm_mir::context::ConstantKind::Int(v) if v.count_ones() == 1 => Some(v),
                        _ => None,
                    },
                    _ => None,
                };
                prod = Some(match power {
                    Some(value) => {
                        // `Prod * (1 << C)` is `Prod << C`.
                        let bits = value.trailing_zeros();
                        let ty = self.ty_of_operand(a);
                        let width = self.context.types.int_bits(ty).expect("an integer");
                        if bits == width - 1 {
                            wrap &= !flags::NSW;
                        }
                        let by = self.constant(ty, bits as i128);
                        self.insert_binop(BinaryOp::Shl, a, by, wrap, true)
                    }
                    None => self.insert_binop(BinaryOp::Mul, a, b, wrap, true),
                });
            }
        }
        prod.expect("a mul has operands")
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L1266
    /// `visitUDivExpr`.
    fn visit_udiv(
        &mut self,
        lhs: Scev,
        rhs: Scev,
    ) -> Operand {
        let l = self.expand(lhs);
        if let Some(c) = self.se.as_constant(rhs) {
            if c.bits.count_ones() == 1 {
                let ty = self.ty_of_operand(l);
                let by = self.constant(ty, c.bits.trailing_zeros() as i128);
                return self.insert_binop(BinaryOp::LShr, l, by, flags::NONE, true);
            }
        }
        let r = self.expand(rhs);
        let nonzero = {
            let cx = cx!(self);
            self.se.is_known_non_zero(&cx, rhs)
        };
        self.insert_binop(BinaryOp::UDiv, l, r, flags::NONE, nonzero)
    }

    // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/ScalarEvolutionExpander.cpp#L2022
    /// `expandMinMaxExpr`; a sequential min freezes the operands it may skip.
    fn expand_min_max(
        &mut self,
        s: Scev,
        ops: &[Use],
        sequential: bool,
    ) -> Operand {
        let tag = self.se.scev_type(s);
        let (name, pred) = match tag {
            Type::SMax => ("smax", IntPredicate::Sgt),
            Type::SMin => ("smin", IntPredicate::Slt),
            Type::UMax => ("umax", IntPredicate::Ugt),
            Type::UMin | Type::SequentialUMin => ("umin", IntPredicate::Ult),
            _ => unreachable!("a min or max"),
        };
        let mut lhs = self.expand(ops[ops.len() - 1].scev);
        let ty = self.ty_of_operand(lhs);
        if sequential {
            lhs = self.emit(Opcode::Freeze, ty, vec![lhs], IrFlags::default());
        }
        for i in (0..ops.len() - 1).rev() {
            let mut rhs = self.expand(ops[i].scev);
            if sequential && i != 0 {
                rhs = self.emit(Opcode::Freeze, ty, vec![rhs], IrFlags::default());
            }
            lhs = if matches!(self.context.types.get(ty), MirType::Int(_)) {
                self.create_binary_intrinsic(name, lhs, rhs)
            } else {
                let bit = self.context.types.int(1);
                let cmp = self.emit(Opcode::ICmp(pred), bit, vec![lhs, rhs], IrFlags::default());
                self.emit(Opcode::Select, ty, vec![cmp, lhs, rhs], IrFlags::default())
            };
        }
        // A mixed integer and pointer min or max returns the pointer type.
        let to = self.scev_ir_ty(s);
        if self.ty_of_operand(lhs) != to {
            lhs = self.insert_noop_cast_of_to(lhs, to);
        }
        lhs
    }
}
