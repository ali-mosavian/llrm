//! Making LLVM's loop strength reduction's solution (`Expand`, `Rewrite`,
//! `RewriteForPHI`, `AdjustInsertPositionForExpand`, `HoistInsertPosition` and
//! `ImplementSolution`; LoopStrengthReduce.cpp at `d1106deb71cc`, l.5712-6306).
//!
//! # How it works
//! For each fixup of each use, the chosen formula is expanded by the SCEV
//! expander into instructions, at a point that every operand dominates and that
//! is as high in the dominator tree as the loop nest allows (so expansions
//! share), and the fixup's operand is replaced by it. A compare against zero
//! has its other operand set to the negated offset or scaled register. A phi's
//! operand is rewritten in its incoming block. What the replaced operands leave
//! unread goes; increments of induction variables LSR reused move to the
//! latch.
//!
//! # Example
//! The store's operand `getelementptr i32, %buf, %i` becomes a phi of a pointer
//! that advances by 4; the compare's `%next == %n` becomes `%counter == 0` on a
//! counter that falls by 1.
//!
//! # Runs
//! Behind `LLRM_EXP_LSR` ([`crate::lsr_pass`]).
//!
//! # Limits
//! DEVIATE: the critical edges to a phi that a fixup rewrites are split before
//! the loop is analysed ([`split_phi_edges`]), not in `RewriteForPHI`: the
//! analyses hold the CFG while they are read. Post-indexed addressing and IV
//! chains are not ported (see [`crate::lsr_collect`]).
//!
//! DEVIATE LU5: an exit compare's limit is an existing narrow value whose
//! zero-extension is the register the formula names (`narrow_existing`, the
//! offset-free case: `narrow_original`), not the wide register expanded and
//! truncated back; lcexit.bas m16 -Os -21 B, one other of 258 programs changes.

use llrm_analysis::scev::expander::{Env as ExpandEnv, Expander};
use llrm_analysis::scev::expr::{Scev, Ty, Use as ScevUse};
use llrm_analysis::scev::{Flags, ScalarEvolution};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags as InstFlags, Opcode};
use llrm_mir::passes::Declared;
use llrm_mir::tti::Tti;
use llrm_mir::types::{Type as MirType, TypeId};

use crate::lsr_formula::Formula;
use crate::lsr_use::{Fixup, Kind, Use, is_am_completely_folded};

/// What was decided for a loop: its uses, each with the formula chosen.
pub struct Plan {
    /// The header of the loop.
    pub l: BlockId,
    pub uses: Vec<Use>,
    /// The index of the chosen formula of each use.
    pub solution: Vec<usize>,
    /// The equality compares whose operands are exchanged first, so the operand
    /// to replace is the left one.
    pub swapped_compares: Vec<InstId>,
    /// Where the increments of the loop's induction variables go.
    pub iv_inc_insert_pos: InstId,
    /// The IV chains to generate.
    pub chains: Vec<crate::lsr_chain::IvChain>,
}

/// The phis that a fixup of `plan` rewrites, with the incoming edges whose
/// block has several successors: `RewriteForPHI` splits those so the code is
/// not put on all the paths. Done once before the loop is analysed, so a plan
/// made after it sees the blocks. Whether any was split.
pub fn split_phi_edges(
    function: &mut Function,
    plan: &Plan,
    loops_header_of: &dyn Fn(BlockId) -> bool,
    in_loop: &dyn Fn(BlockId) -> bool,
) -> bool {
    let mut split = false;
    let phis: Vec<InstId> = plan
        .uses
        .iter()
        .flat_map(|lu| lu.fixups.iter())
        .filter(|fixup| function.instruction(fixup.user).opcode == Opcode::Phi)
        .map(|fixup| fixup.user)
        .collect();
    let mut seen = llrm_support::hash::HashSet::default();
    for phi in phis {
        if !seen.insert(phi) {
            continue;
        }
        let parent = function.parent(phi).expect("a placed phi");
        // Not the header of a loop: its back edge is the canonical one.
        if loops_header_of(parent) {
            continue;
        }
        let incoming = function.instruction(phi).operands.len() / 2;
        if incoming == 1 {
            continue;
        }
        let blocks: Vec<BlockId> = function
            .instruction(phi)
            .operands
            .chunks(2)
            .map(|pair| match pair[1] {
                Operand::Block(block) => block,
                _ => unreachable!("a phi's block"),
            })
            .collect();
        for from in blocks {
            let Some(term) = function.terminator(from) else { continue };
            let successors = function
                .instruction(term)
                .operands
                .iter()
                .filter(|operand| matches!(operand, Operand::Block(_)))
                .count();
            if successors > 1
                && let Some(bridge) = crate::edges::split_critical_edge(function, from, parent)
            {
                // A block that leaves the loop and enters a phi outside it sits
                // before the phi's block, not after its predecessor.
                if in_loop(from) && !in_loop(parent) {
                    let at = function.layout().iter().position(|&one| one == parent).expect("a placed phi block");
                    if at > 0 {
                        let before = function.layout()[at - 1];
                        let _ = function.move_block_after(bridge, before);
                    }
                }
                split = true;
            }
        }
    }
    split
}

/// What an implementation reads and edits.
pub struct Implement<'a, 'e> {
    pub context: &'a mut Context,
    pub function: &'a mut Function,
    pub declared: &'a mut Declared,
    pub se: &'a mut ScalarEvolution,
    pub env: &'a ExpandEnv<'e>,
    pub rewriter: &'a mut Expander,
    pub tti: &'a dyn Tti,
    /// The header of the loop.
    pub l: BlockId,
    /// The blocks of the loop.
    pub body: &'a llrm_mir::dense::IdSet<BlockId>,
    /// Where the increments of the loop's induction variables go: the latch's
    /// terminator.
    pub iv_inc_insert_pos: InstId,
    /// The values whose uses were replaced: what may be dead now.
    pub dead: Vec<ValueId>,
    pub changed: bool,
}

impl Implement<'_, '_> {
    fn expand_code_for(
        &mut self,
        s: Scev,
        ty: Option<Ty>,
    ) -> Operand {
        self.rewriter.expand_code_for(self.env, self.context, self.declared, self.function, self.se, s, ty)
    }

    /// DEVIATE: the compare's other operand when `reg` is that operand
    /// zero-extended. The pin expands the wide register and truncates it back,
    /// a second copy of a value the loop's exit already holds (lcexit.bas
    /// m16 -Os: a 32-bit `umin` beside the 16-bit one). Exit: none.
    fn narrow_original(
        &mut self,
        lf: &Fixup,
        reg: Scev,
        op_ty_id: TypeId,
    ) -> Option<Operand> {
        let other = self.function.instruction(lf.user).operands[1];
        if !matches!(other, Operand::Value(_)) || self.operand_ty(other) != op_ty_id {
            return None;
        }
        let cx = self.env.cx(&*self.context, &*self.function);
        let wide = self.se.type_of(reg);
        let narrow = self.se.get_scev(&cx, other);
        if self.se.get_type_size_in_bits(&cx, self.se.type_of(narrow)) >= self.se.get_type_size_in_bits(&cx, wide) {
            return None;
        }
        (self.se.get_zero_extend_expr(&cx, ScevUse::of(narrow), wide, 0) == reg).then_some(other)
    }

    /// DEVIATE LU5: an existing value of the compare's own narrow type whose
    /// zero-extension is `reg`, one that dominates the compare: the pin expands
    /// the wide register and truncates it back, a second copy of the limit the
    /// loop's exit already holds in 16 bits (lcexit.bas m16 -Os, once the add
    /// and sub arms of foldICmpBinOp made the compare read `iv, umin` and not
    /// `iv + 1, umin + 1`: the exit value `umin + 1` is not its operand). Exit:
    /// none.
    fn narrow_existing(
        &mut self,
        lf: &Fixup,
        reg: Scev,
        op_ty_id: TypeId,
    ) -> Option<Operand> {
        let narrow_ty = self.scev_ty(op_ty_id);
        let cx = self.env.cx(&*self.context, &*self.function);
        let wide = self.se.type_of(reg);
        if wide.is_pointer()
            || self.se.as_constant(reg).is_some()
            || self.se.get_type_size_in_bits(&cx, narrow_ty) >= self.se.get_type_size_in_bits(&cx, wide)
        {
            return None;
        }
        let found: Vec<InstId> = self.function.walk().map(|(_, inst)| inst).collect();
        for inst in found {
            let Some(value) = self.function.instruction(inst).result else { continue };
            let operand = Operand::Value(value);
            if inst == lf.user || self.function.operand_type(self.context, operand) != Some(op_ty_id) {
                continue;
            }
            let cx = self.env.cx(&*self.context, &*self.function);
            let narrow = self.se.get_scev(&cx, operand);
            if self.se.get_zero_extend_expr(&cx, ScevUse::of(narrow), wide, 0) == reg
                && self.env.tree.instruction_dominates(&*self.function, inst, lf.user)
            {
                return Some(operand);
            }
        }
        None
    }

    fn unknown(
        &mut self,
        v: Operand,
    ) -> Scev {
        let cx = self.env.cx(&*self.context, &*self.function);
        self.se.get_unknown(&cx, v)
    }

    fn add(
        &mut self,
        ops: &[Scev],
    ) -> Scev {
        let cx = self.env.cx(&*self.context, &*self.function);
        self.se.get_add_expr(&cx, ops.iter().map(|&op| ScevUse::of(op)).collect(), Flags::default(), 0).scev
    }

    /// The integer constant `value` of `ty`, wrapped as the width.
    fn constant(
        &self,
        ty: Ty,
        value: i128,
    ) -> Operand {
        let Ty::Int(bits) = ty else { unreachable!("an integer type") };
        let id = self.context.types.int(bits);
        Operand::Constant(self.context.int(id, value))
    }

    fn type_id(
        &self,
        ty: Ty,
    ) -> TypeId {
        match ty {
            Ty::Int(bits) => self.context.types.int(bits),
            Ty::Ptr(space) => self.context.types.ptr(space),
        }
    }

    fn scev_ty(
        &self,
        id: TypeId,
    ) -> Ty {
        match self.context.types.get(id) {
            MirType::Int(bits) => Ty::Int(*bits),
            MirType::Pointer(space) => Ty::Ptr(*space),
            other => unreachable!("a SCEVable type: {other:?}"),
        }
    }

    fn operand_ty(
        &self,
        v: Operand,
    ) -> TypeId {
        self.function.operand_type(self.context, v).expect("a typed operand")
    }

    /// `CastInst::Create(getCastOpcode(v, false, to, false), v, to, "tmp",
    /// before)`; a constant is folded.
    fn create_cast(
        &mut self,
        v: Operand,
        to: TypeId,
        before: InstId,
    ) -> Operand {
        let from = self.operand_ty(v);
        if from == to {
            return v;
        }
        let op = match (self.context.types.get(from), self.context.types.get(to)) {
            (MirType::Pointer(_), MirType::Int(_)) => CastOp::PtrToInt,
            (MirType::Int(_), MirType::Pointer(_)) => CastOp::IntToPtr,
            (MirType::Int(a), MirType::Int(b)) if a > b => CastOp::Trunc,
            (MirType::Int(a), MirType::Int(b)) if a < b => CastOp::ZExt,
            (MirType::Pointer(_), MirType::Pointer(_)) => CastOp::AddrSpaceCast,
            _ => CastOp::BitCast,
        };
        let inst = self.function.create_instruction(Opcode::Cast(op), to, vec![v], InstFlags::default(), Some("tmp"));
        self.function.insert(inst, Position::Before(before)).expect("a placed instruction");
        Operand::Value(self.function.instruction(inst).result.expect("a cast value"))
    }

    /// `HoistInsertPosition`: climbs the dominator tree as far as the inputs
    /// still dominate, which canonicalizes the point and so encourages sharing.
    fn hoist_insert_position(
        &self,
        mut ip: InstId,
        inputs: &[InstId],
    ) -> InstId {
        let (function, tree, loops) = (&*self.function, self.env.tree, self.env.loops);
        let mut tentative = ip;
        loop {
            let mut all_dominate = true;
            let mut better: Option<InstId> = None;
            for &inst in inputs {
                if inst == tentative || !tree.instruction_dominates(function, inst, tentative) {
                    all_dominate = false;
                    break;
                }
                // A point in the middle of the block, not its end, so others
                // use it.
                if function.parent(tentative) == function.parent(inst)
                    && better.is_none_or(|b| !tree.instruction_dominates(function, inst, b))
                {
                    better = function.next_in_block(inst);
                }
            }
            if !all_dominate {
                break;
            }
            ip = better.unwrap_or(tentative);

            let block = function.parent(ip).expect("a placed instruction");
            let ip_loop = loops.get_loop_for(block);
            let ip_depth = ip_loop.map_or(0, |one| one.get_loop_depth());
            let mut rung = block;
            let idom = loop {
                let Some(up) = tree.immediate_dominator(rung) else { return ip };
                rung = up;
                let idom_loop = loops.get_loop_for(rung);
                let depth = idom_loop.map_or(0, |one| one.get_loop_depth());
                // Not into a loop.
                if depth <= ip_depth && (depth != ip_depth || idom_loop == ip_loop) {
                    break rung;
                }
            };
            tentative = function.terminator(idom).expect("a terminated block");
        }
        ip
    }

    /// `AdjustInsertPositionForExpand`: a point the operands dominate and the
    /// result dominates.
    fn adjust_insert_position_for_expand(
        &self,
        lowest: InstId,
        lf: &Fixup,
        lu: &Use,
        cx: &llrm_analysis::scev::Cx,
    ) -> InstId {
        let function = &*self.function;
        let mut inputs = Vec::new();
        if let Operand::Value(v) = lf.operand
            && let ValueDef::Instruction(def) = function.value(v).def
        {
            inputs.push(def);
        }
        if lu.kind == Kind::ICmpZero
            && let Operand::Value(v) = function.instruction(lf.user).operands[1]
            && let ValueDef::Instruction(def) = function.value(v).def
        {
            inputs.push(def);
        }
        let lp = cx.loops.get_loop_for(self.l).expect("a loop");
        if lf.post_inc_loops.contains(&self.l) {
            if lf.is_use_fully_outside_loop(cx, self.l) {
                inputs.push(function.terminator(lp.get_loop_latch(function).expect("a latch")).expect("a terminator"));
            } else {
                inputs.push(self.iv_inc_insert_pos);
            }
        }
        // The expansion must also follow the increments of the loops it takes
        // the post-incremented value of.
        for &pil in &lf.post_inc_loops {
            if pil == self.l {
                continue;
            }
            // After the loop's exit.
            let Some(pl) = cx.loops.get_loop_for(pil) else { continue };
            let exiting = pl.get_exiting_blocks(function);
            if let Some((&first, rest)) = exiting.split_first() {
                let mut block = first;
                for &other in rest {
                    block = cx.tree.find_nearest_common_dominator(function, block, other).expect("a common dominator");
                }
                inputs.push(function.terminator(block).expect("a terminator"));
            }
        }
        assert!(function.instruction(lowest).opcode != Opcode::Phi, "Insertion point must be a normal instruction");

        // Climb as far as the inputs allow.
        let mut ip = self.hoist_insert_position(lowest, &inputs);
        // Not before phis, nor landing pads.
        while matches!(
            function.instruction(ip).opcode,
            Opcode::Phi | Opcode::LandingPad { .. }
        ) {
            ip = function.next_in_block(ip).expect("a terminator follows");
        }
        // Below what the expander inserted: the point is the same across
        // expansions and they reuse each other.
        while self.rewriter.is_inserted_instruction(&ip) && ip != lowest {
            ip = function.next_in_block(ip).expect("a terminator follows");
        }
        ip
    }

    /// `Expand`: the code for use `lu`'s formula at fixup `lf`, before `ip`;
    /// the value, and a compare's other operand set.
    fn expand(
        &mut self,
        lu: &Use,
        lf: &Fixup,
        f: &Formula,
        ip: InstId,
    ) -> Operand {
        if lu.rigid_formula {
            return lf.operand;
        }
        // A point the operands dominate and the result dominates.
        let ip = {
            let cx = self.env.cx(&*self.context, &*self.function);
            self.adjust_insert_position_for_expand(ip, lf, lu, &cx)
        };
        self.rewriter.set_insert_point(ip);
        // The expander expands post-incremented uses to advantage.
        for &l in &lf.post_inc_loops {
            self.rewriter.set_post_inc(l);
        }

        // The type the user needs.
        let mut op_ty_id = self.operand_ty(lf.operand);
        let mut op_ty = self.scev_ty(op_ty_id);
        // The type to expand to at first: the user's if the size is right.
        let effective = |this: &Self, ty: Ty| {
            let cx = this.env.cx(&*this.context, &*this.function);
            this.se.get_effective_scev_type(&cx, ty)
        };
        let mut ty = {
            let cx = self.env.cx(&*self.context, &*self.function);
            match f.ty(self.se, &cx) {
                None => op_ty,
                Some(t) if effective(self, t) == effective(self, op_ty) => op_ty,
                Some(t) => t,
            }
        };
        // The type to do the arithmetic in.
        let int_ty = effective(self, ty);
        // A compare of pointers stays in the integers: no inttoptr.
        if lu.kind == Kind::ICmpZero && op_ty.is_pointer() {
            op_ty = int_ty;
            ty = int_ty;
            op_ty_id = self.type_id(int_ty);
        }

        // The operands that add to the full base.
        let mut ops: Vec<Scev> = Vec::new();
        let mut original: Option<Operand> = None;
        for &reg in &f.base_regs {
            let reg = {
                let cx = self.env.cx(&*self.context, &*self.function);
                self.se.denormalize_for_post_inc_use(&cx, reg, &lf.post_inc_loops)
            };
            if lu.kind == Kind::ICmpZero
                && f.base_regs.len() == 1
                && f.base_gv.is_none()
                && !f.base_offset.add_unsigned(lf.offset).is_non_zero()
                && !f.unfolded_offset.is_non_zero()
            {
                original = self.narrow_original(lf, reg, op_ty_id);
                if original.is_some() {
                    continue;
                }
            }
            if lu.kind == Kind::ICmpZero
                && f.base_regs.len() == 1
                && f.base_gv.is_none()
                && !f.unfolded_offset.is_non_zero()
            {
                original = self.narrow_existing(lf, reg, op_ty_id);
                if original.is_some() {
                    continue;
                }
            }
            let v = self.expand_code_for(reg, None);
            let u = self.unknown(v);
            ops.push(u);
        }

        // The scaled register.
        let mut icmp_scaled_v: Option<Operand> = None;
        if f.scale != 0 {
            let scaled = f.scaled_reg.expect("a scaled register");
            let scaled_s = {
                let cx = self.env.cx(&*self.context, &*self.function);
                self.se.denormalize_for_post_inc_use(&cx, scaled, &lf.post_inc_loops)
            };
            if lu.kind == Kind::ICmpZero {
                // As if it were a base register.
                if f.scale == 1 {
                    let v = self.expand_code_for(scaled_s, None);
                    let u = self.unknown(v);
                    ops.push(u);
                } else {
                    // A negated scale is folded by putting the register in the
                    // compare's other operand.
                    assert_eq!(f.scale, -1, "The only scale supported by ICmpZero uses is -1!");
                    icmp_scaled_v = Some(self.expand_code_for(scaled_s, None));
                }
            } else {
                // Expand the register and an explicit scale, to be matched as
                // part of the address. The operands so far are flushed first to
                // keep the expander from hoisting address modes, unless the
                // mode is not folded.
                if !ops.is_empty() && lu.kind == Kind::Address && is_am_completely_folded(self.tti, lu, f) {
                    let sum = self.add(&ops);
                    let full = self.expand_code_for(sum, None);
                    let u = self.unknown(full);
                    ops.clear();
                    ops.push(u);
                }
                let v = self.expand_code_for(scaled_s, None);
                let mut s = self.unknown(v);
                if f.scale != 1 {
                    let cx = self.env.cx(&*self.context, &*self.function);
                    let c = self.se.get_constant_of_type(&cx, self.se.type_of(s), i128::from(f.scale));
                    s = self.se.get_mul_expr(&cx, vec![ScevUse::of(s), ScevUse::of(c)], Flags::default(), 0).scev;
                }
                ops.push(s);
            }
        }

        // The global.
        if let Some(global) = f.base_gv {
            // Flushed, to keep the expander from hoisting.
            if !ops.is_empty() {
                let sum = self.add(&ops);
                let full = self.expand_code_for(sum, Some(int_ty));
                let u = self.unknown(full);
                ops.clear();
                ops.push(u);
            }
            let space = self.env.globals[global.0 as usize].address_space;
            let pointer = self.context.types.ptr(space);
            let constant =
                Operand::Constant(self.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(global) }));
            let u = self.unknown(constant);
            ops.push(u);
        }

        // Flushed: the folded and unfolded offsets live next to their uses.
        if !ops.is_empty() {
            let sum = self.add(&ops);
            let full = self.expand_code_for(sum, Some(ty));
            let u = self.unknown(full);
            ops.clear();
            ops.push(u);
        }

        // The immediate.
        let offset = f.base_offset.add_unsigned(lf.offset);
        if offset.is_non_zero() {
            if lu.kind == Kind::ICmpZero {
                // A compare folds an immediate by negating it into the other
                // operand.
                match icmp_scaled_v {
                    None => icmp_scaled_v = Some(self.constant(int_ty, i128::from(offset.0.wrapping_neg()))),
                    Some(v) => {
                        let u = self.unknown(v);
                        ops.push(u);
                        icmp_scaled_v = Some(self.constant(int_ty, i128::from(offset.0)));
                    }
                }
            } else {
                // Added, to be matched as part of the address.
                let c = self.constant(int_ty, i128::from(offset.0));
                let u = self.unknown(c);
                ops.push(u);
            }
        }
        // The unfolded offset.
        if f.unfolded_offset.is_non_zero() {
            let c = self.constant(int_ty, i128::from(f.unfolded_offset.0));
            let u = self.unknown(c);
            ops.push(u);
        }

        // The instructions that sum the operands.
        let full_s = if ops.is_empty() {
            let cx = self.env.cx(&*self.context, &*self.function);
            self.se.get_constant_of_type(&cx, int_ty, 0)
        } else {
            self.add(&ops)
        };
        let full_v = match original {
            Some(original) => original,
            None => self.expand_code_for(full_s, Some(ty)),
        };
        self.rewriter.clear_post_inc();

        // A compare against zero sets its other operand.
        if lu.kind == Kind::ICmpZero {
            let ci = lf.user;
            if let Operand::Value(v) = self.function.instruction(ci).operands[1] {
                self.dead.push(v);
            }
            assert!(f.base_gv.is_none(), "ICmp does not support folding a global value and a scale at the same time!");
            if f.scale == -1 {
                let mut v = icmp_scaled_v.expect("a scaled value");
                if self.operand_ty(v) != op_ty_id {
                    v = self.create_cast(v, op_ty_id, ci);
                }
                self.function.set_operand(ci, 1, v);
            } else {
                // A scale of 1 was expanded as part of the base registers.
                assert!(
                    f.scale == 0 || f.scale == 1,
                    "ICmp does not support folding a global value and a scale at the same time!"
                );
                let effective_op = effective(self, op_ty);
                let c = self.constant(effective_op, i128::from(offset.0.wrapping_neg()));
                let c = if self.operand_ty(c) != op_ty_id { self.create_cast(c, op_ty_id, ci) } else { c };
                self.function.set_operand(ci, 1, c);
            }
        }
        full_v
    }

    /// `RewriteForPHI`: the operand of a phi is used in its incoming blocks, so
    /// it is expanded in each (once for those that share a value).
    fn rewrite_for_phi(
        &mut self,
        pn: InstId,
        lu: &Use,
        lf: &Fixup,
        f: &Formula,
    ) {
        let mut inserted: Vec<(BlockId, Operand)> = Vec::new();
        let count = self.function.instruction(pn).operands.len() / 2;
        for i in 0..count {
            if self.function.instruction(pn).operands[2 * i] != lf.operand {
                continue;
            }
            let Operand::Block(bb) = self.function.instruction(pn).operands[2 * i + 1] else {
                unreachable!("a phi's block")
            };
            if let Some(&(_, v)) = inserted.iter().find(|(block, _)| *block == bb) {
                self.function.set_operand(pn, 2 * i, v);
                continue;
            }
            let term = self.function.terminator(bb).expect("a terminated block");
            let op_ty = self.operand_ty(lf.operand);
            // DEVIATE: a loop's exit phi with one input sits in a block of its
            // own here, where LLVM's sits in the exit block with the paths
            // that skip the loop, so the edge is split and the code lands
            // there. The code goes at the top of this block, not into the loop,
            // and the phi's readers read it (#4924).
            let parent = self.function.parent(pn).expect("a placed phi");
            if count == 1
                && self.body.contains(&bb)
                && !self.body.contains(&parent)
                && self.function.predecessors(parent).len() == 1
                && let Some(&exit) = crate::lcssa::operations(self.function, parent).first()
                && let Some(result) = self.function.instruction(pn).result
            {
                let mut full = self.expand(lu, lf, f, exit);
                if self.operand_ty(full) != op_ty {
                    full = self.create_cast(full, op_ty, exit);
                }
                self.function.replace_all_uses_with(result, full);
                self.dead.push(result);
                return;
            }
            let mut full = self.expand(lu, lf, f, term);
            // A reuse by no-op cast gets the cast.
            if self.operand_ty(full) != op_ty {
                full = self.create_cast(full, op_ty, term);
            }
            self.function.set_operand(pn, 2 * i, full);
            inserted.push((bb, full));
        }
    }

    /// `Rewrite`: the code for the formula, and the user updated to use it.
    fn rewrite(
        &mut self,
        lu: &Use,
        lf: &Fixup,
        f: &Formula,
    ) {
        if self.function.instruction(lf.user).opcode == Opcode::Phi {
            self.rewrite_for_phi(lf.user, lu, lf, f);
        } else {
            let mut full = self.expand(lu, lf, f, lf.user);
            // A reuse by no-op cast gets the cast; a compare of pointers is
            // already in the integers.
            let op_ty = self.operand_ty(lf.operand);
            let skip = lu.kind == Kind::ICmpZero && matches!(self.context.types.get(op_ty), MirType::Pointer(_));
            if self.operand_ty(full) != op_ty && !skip {
                full = self.create_cast(full, op_ty, lf.user);
            }
            if lu.kind == Kind::ICmpZero {
                // `Expand` may have set the other operand to what the operand
                // to replace now is, so only operand 0 is set.
                self.function.set_operand(lf.user, 0, full);
            } else {
                let slots: Vec<usize> = self
                    .function
                    .instruction(lf.user)
                    .operands
                    .iter()
                    .enumerate()
                    .filter(|&(_, &operand)| operand == lf.operand)
                    .map(|(at, _)| at)
                    .collect();
                for at in slots {
                    self.function.set_operand(lf.user, at, full);
                }
            }
        }
        if let Operand::Value(v) = lf.operand {
            self.dead.push(v);
        }
    }

    /// `ImplementSolution`: every fixup rewritten by the chosen formula of its
    /// use.
    pub fn implement_solution(
        &mut self,
        plan: &Plan,
    ) {
        // The operand to replace goes on the left of an equality compare.
        for &ci in &plan.swapped_compares {
            let operands = self.function.instruction(ci).operands.clone();
            self.function.set_operand(ci, 0, operands[1]);
            self.function.set_operand(ci, 1, operands[0]);
            self.changed = true;
        }
        for (lu, &chosen) in plan.uses.iter().zip(&plan.solution) {
            let f = &lu.formulae[chosen];
            for lf in &lu.fixups {
                // The increment goes at the default point (only an address that
                // can be post-indexed hoists it, and x86 has none).
                self.rewriter.set_iv_inc_insert_pos(self.l, self.iv_inc_insert_pos);
                self.rewrite(lu, lf, f);
                self.changed = true;
            }
        }
        for chain in &plan.chains {
            self.generate_iv_chain(chain);
            self.changed = true;
        }
    }

    /// The increments of the header's simple recurrences move to the point
    /// where LSR costed them: when a recurrence was reused, its increment is
    /// elsewhere.
    pub fn move_increments(&mut self) {
        let function = &*self.function;
        let phis: Vec<InstId> =
            function.iter_block(self.l).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
        for pn in phis {
            let Some(result) = self.function.instruction(pn).result else { continue };
            let arms: Vec<(Operand, BlockId)> = self
                .function
                .instruction(pn)
                .operands
                .chunks(2)
                .map(|pair| match pair[1] {
                    Operand::Block(from) => (pair[0], from),
                    _ => unreachable!("a phi's block"),
                })
                .collect();
            // A simple recurrence: one arm from inside the loop, a binary
            // operator of the phi and a step.
            let lp = self.env.loops.get_loop_for(self.l).expect("a loop");
            let Some(&(value, _)) = arms.iter().find(|(_, from)| lp.contains(*from)) else { continue };
            let Operand::Value(inc) = value else { continue };
            let ValueDef::Instruction(bo) = self.function.value(inc).def else { continue };
            let instruction = self.function.instruction(bo);
            let Opcode::Binary(op) = instruction.opcode else { continue };
            let phi_value = Operand::Value(result);
            match op {
                // A sub is not commutative: the phi is the left operand.
                BinaryOp::Sub if instruction.operands[0] != phi_value => continue,
                BinaryOp::Add | BinaryOp::Sub => {}
                _ => continue,
            }
            let step = if instruction.operands[0] == phi_value {
                instruction.operands[1]
            } else if instruction.operands[1] == phi_value {
                instruction.operands[0]
            } else {
                continue;
            };
            // A step that is not a constant may cost a register.
            if !matches!(step, Operand::Constant(_)) {
                continue;
            }
            // Only across blocks: isel can do the block-local case.
            if self.function.parent(bo) == self.function.parent(self.iv_inc_insert_pos) {
                continue;
            }
            // Where it can legally be scheduled: dominating every use.
            let pos = self.iv_inc_insert_pos;
            let ok = self
                .function
                .users(inc)
                .iter()
                .all(
                    |u| {
                        let user = self.function.instruction(u.user);
                        if user.opcode == Opcode::Phi {
                            let Operand::Block(from) = user.operands[u.index as usize + 1] else {
                                unreachable!("a phi's block")
                            };
                            let at = self.function.parent(pos).expect("a placed instruction");
                            at == from || self.env.tree.dominates(at, from)
                        } else {
                            self.env.tree.instruction_dominates(self.function, pos, u.user)
                        }
                    },
                );
            if !ok {
                continue;
            }
            self.function.move_to(bo, Position::Before(pos)).expect("a placed increment");
            self.changed = true;
        }
    }
}

impl crate::lsr_collect::Collection<'_, '_> {
    /// The plan: the uses, with the formula chosen for each.
    pub fn into_plan(
        self,
        solution: Vec<usize>,
    ) -> Plan {
        Plan {
            l: self.l,
            uses: self.uses,
            solution,
            swapped_compares: self.swapped_compares,
            iv_inc_insert_pos: self.iv_inc_insert_pos,
            chains: Vec::new(),
        }
    }
}

impl Implement<'_, '_> {
    /// `canFoldIVIncExpr`: whether the increment folds into the address mode of
    /// the user.
    fn can_fold_iv_inc_expr(
        &self,
        inc_expr: Scev,
        user: InstId,
        operand: Operand,
    ) -> bool {
        let Some(c) = self.se.as_constant(inc_expr) else { return false };
        if c.significant_bits() > 64 {
            return false;
        }
        let cx = self.env.cx(&*self.context, &*self.function);
        crate::lsr_collect::is_address_use(&cx, user, operand)
            && crate::lsr_collect::is_always_foldable(
                self.tti,
                Kind::Address,
                crate::lsr_collect::get_access_type(&cx, user, operand),
                false,
                crate::lsr_formula::Immediate(c.sext_value() as i64),
                false,
            )
    }

    /// `GenerateIVChain`: an add or subtract for each increment of the chain,
    /// making the user's operand from the previous user's.
    pub fn generate_iv_chain(
        &mut self,
        chain: &crate::lsr_chain::IvChain,
    ) {
        use crate::lsr_chain::{find_iv_operand, wide_operand};
        // The head's IV operand, which LSR may have replaced.
        let head = &chain.incs[0];
        let mut from = 0;
        let mut found: Option<(usize, Operand)> = None;
        loop {
            let at = {
                let cx = self.env.cx(&*self.context, &*self.function);
                find_iv_operand(self.se, &cx, self.l, head.user, from)
            };
            let Some(at) = at else { break };
            let operand = self.function.instruction(head.user).operands[at];
            let src = {
                let cx = self.env.cx(&*self.context, &*self.function);
                wide_operand(&cx, operand)
            };
            // The operand computes the expression the chain needs.
            let (own, wide) = {
                let cx = self.env.cx(&*self.context, &*self.function);
                (self.se.get_scev(&cx, operand), self.se.get_scev(&cx, src))
            };
            if own == head.inc_expr || wide == head.inc_expr {
                found = Some((at, src));
                break;
            }
            from = at + 1;
        }
        // A concealed head: give up on the chain.
        let Some((_, mut iv_src)) = found else { return };

        let iv_ty = self.operand_ty(iv_src);
        let int_ty = {
            let cx = self.env.cx(&*self.context, &*self.function);
            self.se.get_effective_scev_type(&cx, self.scev_ty(iv_ty))
        };
        let mut left_over: Option<Scev> = None;
        let mut accum = {
            let cx = self.env.cx(&*self.context, &*self.function);
            self.se.get_constant_of_type(&cx, int_ty, 0)
        };
        let mut bases: Vec<(Scev, Operand)> = vec![(accum, iv_src)];
        let latch_term = {
            let cx = self.env.cx(&*self.context, &*self.function);
            let lp = cx.loops.get_loop_for(self.l).expect("a loop");
            self.function.terminator(lp.get_loop_latch(self.function).expect("a latch")).expect("a terminator")
        };

        for inc in chain.rest() {
            let insert_pt =
                if self.function.instruction(inc.user).opcode == Opcode::Phi { latch_term } else { inc.user };
            // What replaces the user's operand: the IV value held in a
            // register, or a new one.
            let mut iv_oper = iv_src;
            if !self.se.match_zero(inc.inc_expr) {
                // The increment was made by subtracting two narrow values:
                // signed.
                let (inc_expr, new_accum) = {
                    let cx = self.env.cx(&*self.context, &*self.function);
                    let inc_expr = self.se.get_noop_or_sign_extend(&cx, inc.inc_expr, int_ty);
                    let new_accum = self
                        .se
                        .get_add_expr(&cx, vec![ScevUse::of(accum), ScevUse::of(inc_expr)], Flags::default(), 0)
                        .scev;
                    (inc_expr, new_accum)
                };
                accum = new_accum;
                left_over = Some(match left_over {
                    Some(prev) => {
                        let cx = self.env.cx(&*self.context, &*self.function);
                        self.se
                            .get_add_expr(&cx, vec![ScevUse::of(prev), ScevUse::of(inc_expr)], Flags::default(), 0)
                            .scev
                    }
                    None => inc_expr,
                });
            }
            // A base from which the increment folds into the address mode.
            let mut found_base = false;
            for &(map_scev, map_iv_oper) in bases.iter().rev() {
                let remainder = {
                    let cx = self.env.cx(&*self.context, &*self.function);
                    self.se.get_minus_scev(&cx, ScevUse::of(accum), ScevUse::of(map_scev), 0, 0)
                };
                if self.can_fold_iv_inc_expr(remainder, inc.user, inc.iv_operand) {
                    if !self.se.match_zero(remainder) {
                        self.rewriter.clear_post_inc();
                        self.rewriter.set_insert_point(insert_pt);
                        let inc_v = self.expand_code_for(remainder, Some(int_ty));
                        let (a, b) = (self.unknown(map_iv_oper), self.unknown(inc_v));
                        let expr = self.add(&[a, b]);
                        self.rewriter.set_insert_point(insert_pt);
                        iv_oper = self.expand_code_for(expr, Some(self.scev_ty(iv_ty)));
                    } else {
                        iv_oper = map_iv_oper;
                    }
                    found_base = true;
                    break;
                }
            }
            if !found_base && let Some(rest) = left_over.filter(|&rest| !self.se.match_zero(rest)) {
                // The increment, expanded.
                self.rewriter.clear_post_inc();
                self.rewriter.set_insert_point(insert_pt);
                let inc_v = self.expand_code_for(rest, Some(int_ty));
                let (a, b) = (self.unknown(iv_src), self.unknown(inc_v));
                let expr = self.add(&[a, b]);
                self.rewriter.set_insert_point(insert_pt);
                iv_oper = self.expand_code_for(expr, Some(self.scev_ty(iv_ty)));
                // An increment that does not fold is the next IV value.
                if !self.can_fold_iv_inc_expr(rest, inc.user, inc.iv_operand) {
                    bases.push((accum, iv_oper));
                    iv_src = iv_oper;
                    left_over = None;
                }
            }
            let oper_ty = self.operand_ty(inc.iv_operand);
            if iv_ty != oper_ty {
                iv_oper = self.create_cast(iv_oper, oper_ty, insert_pt);
            }
            let slots: Vec<usize> = self
                .function
                .instruction(inc.user)
                .operands
                .iter()
                .enumerate()
                .filter(|&(_, &operand)| operand == inc.iv_operand)
                .map(|(at, _)| at)
                .collect();
            for at in slots {
                self.function.set_operand(inc.user, at, iv_oper);
            }
            if let Operand::Value(v) = inc.iv_operand {
                self.dead.push(v);
            }
        }

        // A new, wider phi may also replace its post-inc, when the head's wide
        // value was found.
        if self.function.instruction(chain.tail_user()).opcode == Opcode::Phi {
            let latch_block = self.function.parent(latch_term).expect("a placed terminator");
            let phis: Vec<InstId> = self
                .function
                .iter_block(self.l)
                .take_while(|&inst| self.function.instruction(inst).opcode == Opcode::Phi)
                .collect();
            for phi in phis {
                let Some(result) = self.function.instruction(phi).result else { continue };
                if self.function.value(result).ty != iv_ty {
                    continue;
                }
                let Some(slot) = self
                    .function
                    .instruction(phi)
                    .operands
                    .chunks(2)
                    .position(|pair| pair[1] == Operand::Block(latch_block))
                else {
                    continue;
                };
                let post_inc = self.function.instruction(phi).operands[2 * slot];
                let Operand::Value(post_value) = post_inc else { continue };
                if !matches!(self.function.value(post_value).def, ValueDef::Instruction(_)) {
                    continue;
                }
                let same = {
                    let cx = self.env.cx(&*self.context, &*self.function);
                    self.se.get_scev(&cx, post_inc) == self.se.get_scev(&cx, iv_src)
                };
                if !same {
                    continue;
                }
                let post_ty = self.operand_ty(post_inc);
                let oper = if iv_ty != post_ty { self.create_cast(iv_src, post_ty, latch_term) } else { iv_src };
                self.function.set_operand(phi, 2 * slot, oper);
                self.dead.push(post_value);
            }
        }
    }
}
