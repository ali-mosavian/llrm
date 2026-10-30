//! `UnrolledInstAnalyzer`: what simplifies in one iteration of a loop if it
//! were unrolled completely, to cost the unrolled form without making it.
//!
//! # How it works
//! The caller walks the loop's instructions iteration by iteration, with a map
//! of the simplified values so far (header phis of the iteration included).
//! `visit` answers whether an instruction is free after unrolling, and records
//! what it simplifies to: an operation on operands the map already holds is
//! simplified (`simplify_binop_operands`, compares, casts); a load from a
//! constant global at an offset that ScalarEvolution makes constant at this
//! iteration is folded to the initializer's element; any other instruction
//! asks ScalarEvolution for its expression, which is a constant, or an add
//! recurrence of the loop evaluated at the iteration, or is loop-invariant
//! (free after the first iteration). A pointer that is the base of a global
//! plus an offset known at this iteration is remembered with that offset.
//!
//! # Deviations
//! - U1 `fcmp` is not simplified: no `simplifyFCmpInst` exists in
//!   `llrm_mir::simplify`; it is asked of ScalarEvolution alone.
//! - U2 No `FPMathOperator` fast-math flags beyond those on the instruction.
//!
//! # Example
//! At iteration 2 of a loop with `%i = phi i16 [ 0, %pre ], [ %n, %latch ]`,
//! `%i` is `2`, so `%n = add i16 %i, 1` simplifies to `3`, `icmp slt i16 %n, 4`
//! to `true`, and neither is charged to the unrolled loop.
//!
//! # See also
//! [`UnrolledInstAnalyzer`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/LoopUnrollAnalyzer.cpp).

use llrm_mir::constantfolding::constant_fold_load_from_const;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdMap;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Function, GlobalKind, GlobalValue, InstId, Linkage, MetadataNode, Operand, ValueId};
use llrm_mir::opcode::{CastOp, IntPredicate, Opcode};
use llrm_mir::simplify::{simplify_binop_operands, simplify_cast_operand, simplify_icmp_operands};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::AssumptionCache;

use crate::scev::ap::Ap;
use crate::scev::expr::Scev;
use crate::scev::{Cx, ScalarEvolution};

/// `SimplifiedAddress`: a base value and a constant offset from it.
#[derive(Clone, Copy, Debug)]
pub struct SimplifiedAddress {
    pub base: Operand,
    pub offset: Ap,
}

/// What `visit` reads of the function and the analyses, borrowed for one call.
pub struct Env<'a> {
    pub context: &'a mut Context,
    pub layout: &'a DataLayout,
    pub globals: &'a [GlobalValue],
    pub metadata: &'a [MetadataNode],
    pub function: &'a Function,
    pub loops: &'a LoopInfo,
    pub tree: &'a DomTree,
    pub assumptions: &'a AssumptionCache,
    pub se: &'a mut ScalarEvolution,
    /// The header of the loop `L`.
    pub header: BlockId,
}

/// The `Cx` of an `Env`, made of its fields so that `env.se` can be borrowed
/// mutably beside it.
macro_rules! cx {
    ($env:expr) => {
        Cx {
            context: &*$env.context,
            globals: $env.globals,
            metadata: $env.metadata,
            function: $env.function,
            layout: $env.layout,
            loops: $env.loops,
            tree: $env.tree,
            assumptions: $env.assumptions,
        }
    };
}

/// `UnrolledInstAnalyzer`.
pub struct UnrolledInstAnalyzer {
    /// `SimplifiedAddresses`.
    simplified_addresses: IdMap<ValueId, SimplifiedAddress>,
    /// `IterationNumber`: the iteration simulated, as a SCEV.
    iteration_number: Scev,
    iteration_is_zero: bool,
}

/// `CastInst::castIsValid`.
fn cast_is_valid(
    context: &Context,
    op: CastOp,
    src: TypeId,
    dst: TypeId,
) -> bool {
    let (s, d) = (context.types.get(src), context.types.get(dst));
    let bits = |t: &Type| match t {
        Type::Int(width) => Some(u64::from(*width)),
        Type::Float(kind) => Some(u64::from(llrm_mir::datalayout::float_bits(*kind))),
        _ => None,
    };
    match op {
        CastOp::Trunc => matches!((s, d), (Type::Int(a), Type::Int(b)) if a > b),
        CastOp::ZExt | CastOp::SExt => matches!((s, d), (Type::Int(a), Type::Int(b)) if a < b),
        CastOp::FPTrunc => matches!((s, d), (Type::Float(_), Type::Float(_))) && bits(s) > bits(d),
        CastOp::FPExt => matches!((s, d), (Type::Float(_), Type::Float(_))) && bits(s) < bits(d),
        CastOp::FPRound => src == dst && matches!(s, Type::Float(_)),
        CastOp::UIToFP | CastOp::SIToFP => matches!((s, d), (Type::Int(_), Type::Float(_))),
        CastOp::FPToUI | CastOp::FPToSI => matches!((s, d), (Type::Float(_), Type::Int(_))),
        CastOp::PtrToInt => matches!((s, d), (Type::Pointer(_), Type::Int(_))),
        CastOp::IntToPtr => matches!((s, d), (Type::Int(_), Type::Pointer(_))),
        CastOp::BitCast => match (s, d) {
            (Type::Pointer(a), Type::Pointer(b)) => a == b,
            (Type::Pointer(_), _) | (_, Type::Pointer(_)) => false,
            _ => bits(s).is_some() && bits(s) == bits(d),
        },
        CastOp::AddrSpaceCast => matches!((s, d), (Type::Pointer(a), Type::Pointer(b)) if a != b),
    }
}

/// `ICmpInst::compare` on two offsets of one width.
fn compare(
    a: Ap,
    b: Ap,
    pred: IntPredicate,
) -> bool {
    match pred {
        IntPredicate::Eq => a.bits == b.bits,
        IntPredicate::Ne => a.bits != b.bits,
        IntPredicate::Ugt => a.bits > b.bits,
        IntPredicate::Uge => a.bits >= b.bits,
        IntPredicate::Ult => a.bits < b.bits,
        IntPredicate::Ule => a.bits <= b.bits,
        IntPredicate::Sgt => a.sext_value() > b.sext_value(),
        IntPredicate::Sge => a.sext_value() >= b.sext_value(),
        IntPredicate::Slt => a.sext_value() < b.sext_value(),
        IntPredicate::Sle => a.sext_value() <= b.sext_value(),
    }
}

fn is_signed(pred: IntPredicate) -> bool {
    matches!(
        pred,
        IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
    )
}

impl UnrolledInstAnalyzer {
    /// `UnrolledInstAnalyzer(Iteration, SimplifiedValues, SE, L)`.
    pub fn new(
        se: &mut ScalarEvolution,
        iteration: u32,
    ) -> Self {
        Self {
            simplified_addresses: IdMap::new(),
            iteration_number: se.get_constant(Ap::new(64, u128::from(iteration))),
            iteration_is_zero: iteration == 0,
        }
    }

    /// `Base::visit(I)`: whether `inst` is free after unrolling.
    pub fn visit(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        match env.function.instruction(inst).opcode {
            Opcode::Binary(_) => self.visit_binary_operator(env, simplified, inst),
            Opcode::Load { .. } => self.visit_load(env, simplified, inst),
            Opcode::Cast(_) => self.visit_cast_inst(env, simplified, inst),
            Opcode::ICmp(_) => self.visit_cmp_inst(env, simplified, inst),
            Opcode::Phi => self.visit_phi_node(env, simplified, inst),
            _ => self.visit_instruction(env, simplified, inst),
        }
    }

    /// The operand `op` is simplified to, where it is.
    fn lookup(
        simplified: &IdMap<ValueId, Operand>,
        op: Operand,
    ) -> Operand {
        match op {
            Operand::Value(value) => simplified.get(&value).copied().unwrap_or(op),
            _ => op,
        }
    }

    /// `simplifyInstWithSCEV`: some `AddRec` expressions become constants,
    /// which then trigger the folding of other instructions. Where the
    /// start value is an address instead, the offset from it is remembered.
    fn simplify_inst_with_scev(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        let operation = env.function.instruction(inst);
        let Some(result) = operation.result else { return false };
        if !env.se.is_scevable(env.context.types.get(operation.ty)) {
            return false;
        }

        let s = env.se.get_scev(&cx!(env), Operand::Value(result));
        if let Some(constant) = env.se.as_constant(s) {
            let ty = env.context.types.int(constant.width);
            simplified.insert(result, Operand::Constant(env.context.int(ty, constant.sext_value())));
            return true;
        }

        // A loop-invariant computation is made once: all but the first are
        // free.
        if !self.iteration_is_zero && env.se.is_loop_invariant(&cx!(env), s, Some(env.header)) {
            return true;
        }

        if env.se.addrec_header(s) != Some(env.header) {
            return false;
        }

        let value_at_iteration = env.se.evaluate_at_iteration(&cx!(env), s, self.iteration_number);
        // Check if the add recurrence becomes a constant.
        if let Some(constant) = env.se.as_constant(value_at_iteration) {
            let ty = env.context.types.int(constant.width);
            simplified.insert(result, Operand::Constant(env.context.int(ty, constant.sext_value())));
            return true;
        }

        // Check if the offset from the base address becomes a constant.
        let base = env.se.get_pointer_base(s);
        let Some(base_value) = env.se.unknown_value(base) else { return false };
        let Some(offset) = env.se.compute_constant_difference(&cx!(env), value_at_iteration, base) else {
            return false;
        };
        self.simplified_addresses.insert(result, SimplifiedAddress { base: base_value, offset });
        false
    }

    /// `visitBinaryOperator`.
    fn visit_binary_operator(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        let operation = env.function.instruction(inst);
        let Opcode::Binary(op) = operation.opcode else { unreachable!("a binary operator") };
        let (lhs, rhs) =
            (Self::lookup(simplified, operation.operands[0]), Self::lookup(simplified, operation.operands[1]));
        let flags = operation.flags;
        let result = operation.result;
        if let Some(simple) = simplify_binop_operands(env.context, env.layout, env.function, op, flags, lhs, rhs) {
            if let Some(result) = result {
                simplified.insert(result, simple);
            }
            return true;
        }
        self.visit_instruction(env, simplified, inst)
    }

    /// `visitLoad`: a load that folds completely to a constant.
    fn visit_load(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        let operation = env.function.instruction(inst);
        let (Operand::Value(address), Some(result)) = (operation.operands[0], operation.result) else {
            return self.visit_instruction(env, simplified, inst);
        };
        let Some(&known) = self.simplified_addresses.get(&address) else {
            return self.visit_instruction(env, simplified, inst);
        };
        // Only a load that folds to a constant from a global with a definitive
        // initializer.
        let Operand::Constant(base) = known.base else { return false };
        let ConstantKind::Global(global) = env.context.get(base).kind else { return false };
        let value = &env.globals[global.0 as usize];
        let GlobalKind::Variable(variable) = &value.kind else { return false };
        let interposable =
            matches!(
                value.linkage,
                Linkage::Weak | Linkage::LinkOnce | Linkage::Common | Linkage::ExternWeak
            );
        let Some(initializer) = variable.initializer else { return false };
        if !variable.constant || interposable || variable.externally_initialized {
            return false;
        }
        let ty = operation.ty;
        let Some(folded) =
            constant_fold_load_from_const(env.context, env.layout, initializer, ty, known.offset.sext_value() as u64)
        else {
            return false;
        };
        simplified.insert(result, Operand::Constant(folded));
        true
    }

    /// `visitCastInst`.
    fn visit_cast_inst(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        let operation = env.function.instruction(inst);
        let Opcode::Cast(op) = operation.opcode else { unreachable!("a cast") };
        let operand = Self::lookup(simplified, operation.operands[0]);
        let (ty, result) = (operation.ty, operation.result);
        // The cast can be invalid, because the simplified values hold results
        // of ScalarEvolution, which operates on integers (a null pointer might
        // be the integer 0).
        let operand_ty = env.function.operand_type(env.context, operand);
        if operand_ty.is_some_and(|from| cast_is_valid(env.context, op, from, ty))
            && let Some(value) = simplify_cast_operand(env.context, env.layout, env.function, op, operand, ty)
        {
            if let Some(result) = result {
                simplified.insert(result, value);
            }
            return true;
        }
        self.visit_instruction(env, simplified, inst)
    }

    /// `visitCmpInst` (R: `icmp` only, U1).
    fn visit_cmp_inst(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        let operation = env.function.instruction(inst);
        let Opcode::ICmp(pred) = operation.opcode else { unreachable!("an icmp") };
        let (lhs, rhs) =
            (Self::lookup(simplified, operation.operands[0]), Self::lookup(simplified, operation.operands[1]));
        let (ty, result) = (operation.ty, operation.result);

        // Two addresses from one base compare as their offsets.
        if let (Operand::Value(l), Operand::Value(r)) = (lhs, rhs)
            && !is_signed(pred)
            && let (Some(a), Some(b)) =
                (self.simplified_addresses.get(&l).copied(), self.simplified_addresses.get(&r).copied())
            && a.base == b.base
        {
            // Only exact for equality predicates (see the FIXME in LLVM); used
            // for cost modelling alone.
            let holds = compare(a.offset, b.offset, pred);
            if let Some(result) = result {
                simplified.insert(result, Operand::Constant(env.context.int(ty, i128::from(holds))));
            }
            return true;
        }

        if let Some(value) = simplify_icmp_operands(env.context, env.layout, env.function, pred, lhs, rhs) {
            if let Some(result) = result {
                simplified.insert(result, value);
            }
            return true;
        }
        self.visit_instruction(env, simplified, inst)
    }

    /// `visitPHINode`: the loop induction phis are definitionally free.
    fn visit_phi_node(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        // Run the base visitor first. This way useful information is gathered.
        if self.visit_instruction(env, simplified, inst) {
            return true;
        }
        env.function.parent(inst) == Some(env.header)
    }

    /// `visitInstruction`.
    fn visit_instruction(
        &mut self,
        env: &mut Env,
        simplified: &mut IdMap<ValueId, Operand>,
        inst: InstId,
    ) -> bool {
        self.simplify_inst_with_scev(env, simplified, inst)
    }
}
