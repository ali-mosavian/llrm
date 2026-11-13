//! `LoopVectorizePass`: widens the loops the legality check accepts and the
//! planner prices, by the factors it chooses.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `LoopVectorize.cpp`
//! (`processLoop`,
//! [L7714](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorize.cpp#L7714))
//! over the translated parts: [`can_vectorize`] (`LoopVectorizationLegality`),
//! [`plan`] (`LoopVectorizationPlanner`) and the execution of the plan
//! (`VPlan::execute`, `InnerLoopVectorizer`). For each innermost loop of the
//! function the pass asks legality, extracts the recipes (the loop's memory
//! accesses and control), asks the planner for VF and IC and, if both agree,
//! rewrites the loop: each load becomes `IC` wide loads of `VF` lanes, each
//! store `IC` wide stores of the loaded values (the part `k` at
//! `k * VF` elements past the first), and the induction steps by `VF * IC`.
//!
//! # Example
//! ```text
//! loop: %i = phi [0, %ph], [%n, %loop]       loop: %i = phi [0, %ph], [%n, %loop]
//!       %v = load i16, gep %a, %i      =>          %wide.load = load <8 x i16>, gep %a, %i
//!       store i16 %v, gep %b, %i                   %g1 = gep (gep %a, %i), 8
//!       %n = add nuw nsw %i, 1                     %wide.load1 = load <8 x i16>, %g1 ...
//!       br (%n == 1920) ...                        %n = add nuw nsw %i, 16
//! ```
//!
//! # DEVIATE rows
//! - LVP1 the vector loop is made in place: the scalar induction is the vector
//!   loop's `index` (the pin makes `vector.ph`, `vector.body`, `middle.block`
//!   and keeps the scalar loop as the remainder). The remainder is a copy of
//!   the loop made first ([`crate::loopvectorizeskeleton`]); with a constant
//!   trip count VF x IC divides there is none, as in the pin after SimplifyCFG.
//!   Exit: the VPlan skeleton (VP1).
//! - LVP2 a loop whose induction value is used after the loop, or whose exit
//!   test is not `icmp eq`/`ne` of the stepped induction against a
//!   loop-invariant bound, is left alone (the induction may start anywhere: the
//!   pin's VPlan makes its own canonical IV). Exit: the middle block's fix-ups
//!   of live-outs (LVP5) and the exit tests `ult`/`slt` against a bound.
//! - LVP3 recipes are loads and stores of consecutive addresses and the loop
//!   control (VP2 of the planner): a stored value is a loaded value or a
//!   loop-invariant one (broadcast in the preheader, as the pin's
//!   `broadcast.splat`). Exit: reverse accesses.
//! - LVP5 recipes also include integer `add`, `sub`, `mul`, `and`, `or`, `xor`
//!   and shifts by a constant, on loaded and loop-invariant operands (the pin's
//!   `VPWidenRecipe`). LVP6 a shift by anything else is left scalar. An
//!   operation of the induction (a vector `step-vector`) is not widened. Exit:
//!   LV-X3 (2026-11-20).
//! - LVP4 the new instructions carry no metadata (`!tbaa`, `!alias.scope`).
//!
//! # Runs
//! At `loop-vectorize` (l.1354 of `PassBuilderPipelines.cpp`), only where the
//! target takes vector values (`Machine::has_vector_mir`).

use llrm_analysis::aggregate::AggregateAA;
use llrm_analysis::basicaa::BasicAA;
use llrm_analysis::ivdescriptors::InductionKind;
use llrm_analysis::loopaccess::{SymbolicStrideMap, get_ptr_stride};
use llrm_analysis::loopaccessinfo::LoopAccessInfo;
use llrm_analysis::memory;
use llrm_analysis::scev::analysis::ScevSession;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_analysis::typebasedaa::TypeBasedAA;
use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode, Tail};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses};
use llrm_mir::tti::{ArithOp, Ty};
use llrm_mir::types::{FloatKind, Type, TypeId};

use crate::loopvectorizeinterleave::{self as interleave, Extracted};
use crate::loopvectorizelegality::{Legality, can_vectorize, const_int_step, widened_intrinsic};
use crate::loopvectorizeplan::{LoopFacts, Plan, Recipe, ScalarFacts, choose_interleave, pin_may_vectorize, plan};
use crate::loopvectorizeskeleton::{self as skeleton, Shape, Skeleton};

/// `LoopVectorizePass`.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoopVectorize {
    /// Vectorize whatever the target says of vector values (the unit tests').
    pub always: bool,
    /// `LoopVectorizeOptions::VectorizeOnlyWhenForced`
    /// (`!PTO.LoopVectorization`, clang's -O1). The pin still widens a loop
    /// that carries `llvm.loop.vectorize.enable`; DEVIATE LV-F1: no
    /// frontend of llrm emits that hint, so a forced loop is not looked
    /// for. Owner arm64-vec, exit 2026-11-24.
    pub only_when_forced: bool,
}

/// What an operand of a widened instruction is.
#[derive(Clone, Copy, Debug)]
enum Source {
    /// The result of a load or binary operation of the loop: its wide parts.
    Wide(InstId),
    /// A loop-invariant value: the pin's `broadcast.splat`.
    Splat(Operand),
}

/// One load or store of the loop, by the instruction it widens.
#[derive(Clone, Copy, Debug)]
struct Access {
    inst: InstId,
    is_load: bool,
    /// The scalar element type.
    element: TypeId,
    align: u64,
    /// For a store, the value it stores.
    stored: Option<Source>,
}

/// What a widened operation computes: a binary operation, `fneg`, or the
/// call of `llvm.fmuladd`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WideOp {
    Binary(BinaryOp),
    FNeg,
    FMulAdd,
    Cast(CastOp),
}

impl WideOp {
    fn arity(self) -> usize {
        match self {
            WideOp::Binary(_) => 2,
            WideOp::FNeg | WideOp::Cast(_) => 1,
            WideOp::FMulAdd => 3,
        }
    }
}

/// A binary operation, `fneg` or `fmuladd` of the loop, widened lane by lane.
#[derive(Clone, Copy, Debug)]
struct Operation {
    inst: InstId,
    op: WideOp,
    flags: Flags,
    element: TypeId,
    /// The scalar type of the operands (a cast's source; else `element`).
    operand_element: TypeId,
    /// The first `op.arity()` are read.
    operands: [Source; 3],
}

/// An instruction of the loop that is widened, in program order.
#[derive(Clone, Copy, Debug)]
enum Step {
    Access(Access),
    Operation(Operation),
}

/// What the transform needs to rewrite a planned loop.
struct Widening {
    steps: Vec<Step>,
    /// The `add` that steps the induction, and which of its operands is the
    /// constant step.
    step: InstId,
    step_operand: usize,
    index_ty: TypeId,
    /// Where the control flow around the vector loop is made.
    shape: Shape,
}

/// Why a loop is left alone (LV: Not vectorizing: ...).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Skipped(pub &'static str);

fn element_ty(
    cx: &Cx,
    ty: TypeId,
) -> Option<Ty> {
    match cx.context.types.get(ty) {
        Type::Int(bits @ (8 | 16 | 32 | 64)) => Some(Ty::Int(*bits)),
        // `v4f32`, `v2f64`: the float lanes `FPR128` holds.
        Type::Float(FloatKind::Float) => Some(Ty::Float),
        Type::Float(FloatKind::Double) => Some(Ty::Double),
        _ => None,
    }
}

/// The integer induction that steps by one and its step instruction.
pub(crate) fn primary_and_step(
    se: &mut ScalarEvolution,
    legality: &Legality,
) -> Result<(InstId, InstId), Skipped> {
    let primary = legality
        .inductions
        .iter()
        .find(|(_, descriptor)| {
            descriptor.kind == InductionKind::Int && const_int_step(se, descriptor).is_some_and(|step| step.is_one())
        })
        .map(|(phi, _)| *phi)
        .ok_or(Skipped("No integer induction that steps by one"))?;
    let step = legality
        .inductions
        .iter()
        .find(|(phi, _)| *phi == primary)
        .and_then(|(_, descriptor)| descriptor.induction_bin_op)
        .ok_or(Skipped("The induction has no step instruction"))?;
    Ok((primary, step))
}

/// The recipes of `header`'s loop, and what the transform rewrites (LVP3), or
/// why not.
fn extract(
    cx: &Cx,
    se: &mut ScalarEvolution,
    the_loop: Loop,
    legality: &Legality,
) -> Result<(Vec<Recipe>, Widening), Skipped> {
    let function = cx.function;
    let header = the_loop.get_header();
    // The pin's VPlan makes its own canonical IV, so the original induction may
    // start anywhere: an integer one stepping by one is all the widening needs.
    let (primary, step) = primary_and_step(se, legality)?;
    if !legality.reductions.is_empty() {
        return Err(Skipped("Reductions are not widened yet"));
    }
    let primary_phi = Operand::Value(function.instruction(primary).result.expect("a phi"));
    let index_ty = function.instruction(primary).ty;
    let strides = SymbolicStrideMap::default();
    let preheader = the_loop.get_loop_preheader(function).ok_or(Skipped("The loop has no preheader"))?;
    let exit = the_loop.get_unique_exit_block(function).ok_or(Skipped("The loop has no unique exit block"))?;

    let used_outside = |inst: InstId| -> bool {
        function
            .instruction(inst)
            .result
            .is_some_and(|value| function.users(value).iter().any(|one| function.parent(one.user) != Some(header)))
    };

    // Defined before the loop (a constant, an argument, an outer value).
    let invariant = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            llrm_mir::module::ValueDef::Instruction(def) => function.parent(def) != Some(header),
            _ => true,
        },
        _ => true,
    };
    // Every use is a store's value or an integer binary operation of this
    // loop (the instruction itself is judged where it comes).
    let widened_users = |value: ValueId| {
        function
            .users(value)
            .iter()
            .all(
                |one| function.parent(one.user) == Some(header)
                    && match function.instruction(one.user).opcode {
                        Opcode::Store { .. } => one.index == 0,
                        Opcode::Binary(_) | Opcode::FNeg => true,
                        Opcode::Call(_) => widened_intrinsic(cx, one.user).is_some(),
                        Opcode::Cast(_) => true,
                        _ => false,
                    },
            )
    };
    // What an operand is: a value the loop widened, or a loop-invariant one.
    let source_of = |operand: Operand, widened: &[InstId]| -> Result<Source, Skipped> {
        if let Operand::Value(value) = operand
            && let llrm_mir::module::ValueDef::Instruction(def) = function.value(value).def
            && function.parent(def) == Some(header)
        {
            return if widened.contains(&def) {
                Ok(Source::Wide(def))
            } else {
                Err(Skipped("A value the loop computes that is not widened"))
            };
        }
        if let Operand::Constant(c) = operand
            && !matches!(
                cx.context.get(c).kind,
                ConstantKind::Int(_) | ConstantKind::Float(_) | ConstantKind::Zero
            )
        {
            return Err(Skipped("An operand constant that is not an integer or a float"));
        }
        Ok(Source::Splat(operand))
    };
    let mut recipes = Vec::new();
    let mut steps: Vec<Step> = Vec::new();
    let mut widened: Vec<InstId> = Vec::new();
    let mut step_operand = None;
    let mut exit_compare = None;
    for inst in function.iter_block(header).collect::<Vec<_>>() {
        let data = function.instruction(inst);
        match data.opcode {
            Opcode::Phi => {
                if inst != primary {
                    return Err(Skipped("A header phi other than the induction"));
                }
                // The widened loop leaves the last vector iteration's value:
                // the pin's middle block fixes it (LVP5: not translated).
                if used_outside(inst) {
                    return Err(Skipped("The induction variable is used after the loop"));
                }
            }
            Opcode::GetElementPtr { .. } => {
                // An address: every use is the pointer of a load or store of
                // this loop, or the base of another address.
                let value = data.result.expect("a gep");
                let only_addresses = function
                    .users(value)
                    .iter()
                    .all(
                        |one| {
                            let user = function.instruction(one.user);
                            function.parent(one.user) == Some(header)
                                && match user.opcode {
                                    Opcode::Load { .. } => one.index == 0,
                                    Opcode::Store { .. } => one.index == 1,
                                    // An address built on this one (the base
                                    // operand).
                                    Opcode::GetElementPtr { .. } => one.index == 0,
                                    _ => false,
                                }
                        },
                    );
                if !only_addresses {
                    return Err(Skipped("An address used other than by a load or store"));
                }
            }
            Opcode::Load { align, volatile } | Opcode::Store { align, volatile } => {
                let is_load = matches!(data.opcode, Opcode::Load { .. });
                if volatile {
                    return Err(Skipped("A volatile access"));
                }
                let (value_ty, ptr) = if is_load {
                    (data.ty, data.operands[0])
                } else {
                    let value = data.operands[0];
                    (function.operand_type(cx.context, value).ok_or(Skipped("An untyped store"))?, data.operands[1])
                };
                let lane = element_ty(cx, value_ty).ok_or(Skipped("An element type the target has no vector of"))?;
                if get_ptr_stride(se, cx, value_ty, ptr, header, &strides, true) != Some(1) {
                    return Err(Skipped("An access that is not consecutive"));
                }
                let size = cx.layout.size_bits(&cx.context.types, value_ty) / 8;
                let align = align.unwrap_or(size);
                let mut stored = None;
                if is_load {
                    // Every use of the loaded value is a store of this loop or
                    // an operation the loop widens (LVP3).
                    let value = data.result.expect("a load");
                    if !widened_users(value) {
                        return Err(Skipped("A loaded value used other than by a store or a binary operation"));
                    }
                    recipes.push(Recipe::Load { lane, align });
                } else {
                    stored = Some(source_of(data.operands[0], &widened)?);
                    recipes.push(Recipe::Store { lane, align });
                }
                if is_load {
                    widened.push(inst);
                }
                steps.push(Step::Access(Access { inst, is_load, element: value_ty, align, stored }));
            }
            Opcode::Binary(op) if inst != step => {
                let value = data.result.expect("a binary operation");
                // An index offset: only addresses use it (the accesses use the
                // gep, not the sum).
                let offsets_address = op == BinaryOp::Add
                    && matches!(data.operands[1], Operand::Constant(_))
                    && function.users(value).iter().all(|one| {
                        function.parent(one.user) == Some(header)
                            && matches!(
                                function.instruction(one.user).opcode,
                                Opcode::GetElementPtr { .. }
                            )
                    });
                if offsets_address {
                    continue;
                }
                // `VPWidenRecipe`: an integer operation on widened values and
                // loop-invariant ones (LVP3). Integer division traps and has no
                // NEON lane form; `frem` is a libcall.
                let arith = match op {
                    BinaryOp::Add => ArithOp::Add,
                    BinaryOp::Sub => ArithOp::Sub,
                    BinaryOp::Mul => ArithOp::Mul,
                    BinaryOp::Shl => ArithOp::Shl,
                    BinaryOp::LShr => ArithOp::LShr,
                    BinaryOp::AShr => ArithOp::AShr,
                    BinaryOp::And => ArithOp::And,
                    BinaryOp::Or => ArithOp::Or,
                    BinaryOp::Xor => ArithOp::Xor,
                    BinaryOp::FAdd => ArithOp::FAdd,
                    BinaryOp::FSub => ArithOp::FSub,
                    BinaryOp::FMul => ArithOp::FMul,
                    BinaryOp::FDiv => ArithOp::FDiv,
                    _ => return Err(Skipped("A binary operation the vectorizer does not widen yet")),
                };
                let lane = element_ty(cx, data.ty).ok_or(Skipped("An element type the target has no vector of"))?;
                if used_outside(inst) {
                    return Err(Skipped("A binary operation used after the loop"));
                }
                if !widened_users(value) {
                    return Err(Skipped("A binary result used other than by a store or a binary operation"));
                }
                let operands = [source_of(data.operands[0], &widened)?, source_of(data.operands[1], &widened)?];
                // DEVIATE LVP6: a shift is widened only by a constant (a
                // `shl`/`ushr`/`sshr` of an immediate on arm64); the pin also
                // shifts by a vector or a loop-invariant amount (`ushl`).
                if matches!(op, BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
                    && !matches!(operands[1], Source::Splat(Operand::Constant(_)))
                {
                    return Err(Skipped("A shift by an amount that is not a constant"));
                }
                if operands.iter().all(|one| matches!(one, Source::Splat(_))) {
                    return Err(Skipped("A binary operation of loop-invariant values"));
                }
                recipes.push(Recipe::Binary { op: arith, lane });
                widened.push(inst);
                steps.push(Step::Operation(Operation {
                    inst,
                    op: WideOp::Binary(op),
                    flags: data.flags,
                    element: data.ty,
                    operand_element: data.ty,
                    operands: [operands[0], operands[1], operands[1]],
                }));
            }
            // `VPWidenRecipe` of `fneg`: one widened or loop-invariant operand.
            Opcode::FNeg => {
                let value = data.result.expect("an fneg");
                let lane = element_ty(cx, data.ty).ok_or(Skipped("An element type the target has no vector of"))?;
                if used_outside(inst) {
                    return Err(Skipped("A binary operation used after the loop"));
                }
                if !widened_users(value) {
                    return Err(Skipped("A binary result used other than by a store or a binary operation"));
                }
                let operand = source_of(data.operands[0], &widened)?;
                if matches!(operand, Source::Splat(_)) {
                    return Err(Skipped("A binary operation of loop-invariant values"));
                }
                recipes.push(Recipe::Binary { op: ArithOp::FNeg, lane });
                widened.push(inst);
                steps.push(Step::Operation(Operation {
                    inst,
                    op: WideOp::FNeg,
                    flags: data.flags,
                    element: data.ty,
                    operand_element: data.ty,
                    operands: [operand; 3],
                }));
            }
            // `VPWidenCastRecipe`: a conversion between integer and float
            // lanes of one width. DEVIATE LV-X4 (owner llrm-arm64-vec, exit
            // LV-W3b): the pin also widens `zext`/`sext`/`trunc` and the
            // changes of width, which need the D-register and split
            // legalization.
            Opcode::Cast(op) => {
                let kind = interleave::cast_kind(op).ok_or(Skipped("A cast the cost model does not know"))?;
                if !matches!(
                    op,
                    CastOp::SIToFP | CastOp::UIToFP | CastOp::FPToSI | CastOp::FPToUI
                ) {
                    return Err(Skipped("A cast the target has no vector instruction for yet"));
                }
                let value = data.result.expect("a cast");
                let dst = element_ty(cx, data.ty).ok_or(Skipped("An element type the target has no vector of"))?;
                let source = function.operand_type(cx.context, data.operands[0]).ok_or(Skipped("An untyped cast"))?;
                let src = element_ty(cx, source).ok_or(Skipped("An element type the target has no vector of"))?;
                if cx.layout.size_bits(&cx.context.types, source) != cx.layout.size_bits(&cx.context.types, data.ty) {
                    return Err(Skipped("A cast that changes the lane width"));
                }
                if used_outside(inst) {
                    return Err(Skipped("A cast used after the loop"));
                }
                if !widened_users(value) {
                    return Err(Skipped("A cast result used other than by a store or an operation"));
                }
                let operand = source_of(data.operands[0], &widened)?;
                if matches!(operand, Source::Splat(_)) {
                    return Err(Skipped("A cast of a loop-invariant value"));
                }
                recipes.push(Recipe::Cast { kind, dst, src, folded: false });
                widened.push(inst);
                steps.push(Step::Operation(Operation {
                    inst,
                    op: WideOp::Cast(op),
                    flags: data.flags,
                    element: data.ty,
                    operand_element: source,
                    operands: [operand; 3],
                }));
            }
            // `VPWidenIntrinsicRecipe` of `llvm.fmuladd`: the call's arguments
            // (the callee is the last operand), widened or loop-invariant.
            Opcode::Call(_) if widened_intrinsic(cx, inst) == Some(Intrinsic::FMulAdd) => {
                let value = data.result.expect("an fmuladd");
                let lane = element_ty(cx, data.ty).ok_or(Skipped("An element type the target has no vector of"))?;
                if used_outside(inst) {
                    return Err(Skipped("A call used after the loop"));
                }
                if !widened_users(value) {
                    return Err(Skipped("A call result used other than by a store or a binary operation"));
                }
                let operands = [
                    source_of(data.operands[0], &widened)?,
                    source_of(data.operands[1], &widened)?,
                    source_of(data.operands[2], &widened)?,
                ];
                if operands.iter().all(|one| matches!(one, Source::Splat(_))) {
                    return Err(Skipped("A call of loop-invariant values"));
                }
                recipes.push(Recipe::FMulAdd { lane });
                widened.push(inst);
                steps.push(Step::Operation(Operation {
                    inst,
                    op: WideOp::FMulAdd,
                    flags: data.flags,
                    element: data.ty,
                    operand_element: data.ty,
                    operands,
                }));
            }
            Opcode::Binary(BinaryOp::Add) if inst == step => {
                if used_outside(inst) {
                    return Err(Skipped("The induction is used after the loop"));
                }
                // `%i + 1`: the phi and the constant step, in either order.
                let (a, b) = (data.operands[0], data.operands[1]);
                step_operand = if a == primary_phi && matches!(b, Operand::Constant(_)) {
                    Some(1)
                } else if b == primary_phi && matches!(a, Operand::Constant(_)) {
                    Some(0)
                } else {
                    None
                };
                if step_operand.is_none() {
                    return Err(Skipped("The induction step is not a constant"));
                }
            }
            Opcode::ICmp(predicate) => {
                // The exit test: the stepped induction against a loop-invariant
                // bound (LVP2). The constant trip count (VP3) fixes where the
                // widened induction meets it.
                let stepped = function.instruction(step).result.map(Operand::Value);
                let (a, b) = (data.operands[0], data.operands[1]);
                let bound_operand = if Some(a) == stepped && invariant(b) {
                    Some(1)
                } else if Some(b) == stepped && invariant(a) {
                    Some(0)
                } else {
                    None
                };
                let Some(bound_operand) =
                    bound_operand.filter(|_| matches!(predicate, IntPredicate::Eq | IntPredicate::Ne))
                else {
                    return Err(Skipped("The exit test is not the stepped induction against a loop-invariant bound"));
                };
                exit_compare = Some((inst, bound_operand));
            }
            Opcode::Br => {}
            _ => return Err(Skipped("An instruction the vectorizer does not widen yet")),
        }
    }
    let step_operand = step_operand.ok_or(Skipped("The induction step was not found"))?;
    let (compare, bound_operand) = exit_compare.ok_or(Skipped("No exit compare"))?;
    if !steps.iter().any(|one| matches!(one, Step::Access(_))) {
        return Err(Skipped("No memory access to widen"));
    }
    let shape = Shape { header, preheader, exit, primary, compare, bound_operand, index_ty };
    Ok((recipes, Widening { steps, step, step_operand, index_ty, shape }))
}

fn int_constant(
    context: &llrm_mir::context::Context,
    ty: TypeId,
    value: u64,
) -> Operand {
    Operand::Constant(context.int(ty, i128::from(value)))
}

/// `Builder.CreateVectorSplat(vf, scalar)` in the preheader: a constant
/// splat, or the `insertelement` + `shufflevector` the pin hoists
/// (`broadcast.splatinsert`, `broadcast.splat`).
fn broadcast(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    preheader: BlockId,
    vector_ty: TypeId,
    vf: u32,
    scalar: Operand,
) -> Operand {
    if let Operand::Constant(c) = scalar {
        let kind = match context.get(c).kind.clone() {
            ConstantKind::Int(0) | ConstantKind::Float(0) | ConstantKind::Zero => ConstantKind::Zero,
            _ => ConstantKind::Aggregate(vec![c; vf as usize]),
        };
        return Operand::Constant(context.constant(Constant { ty: vector_ty, kind }));
    }
    let before = Position::Before(function.terminator(preheader).expect("a terminated preheader"));
    let poison = Operand::Constant(context.constant(Constant { ty: vector_ty, kind: ConstantKind::Poison }));
    let zero = Operand::Constant(context.int(context.types.intern(Type::Int(64)), 0));
    let inserted = function.create_instruction(
        Opcode::InsertElement,
        vector_ty,
        vec![poison, scalar, zero],
        Flags::default(),
        Some("broadcast.splatinsert"),
    );
    function.insert(inserted, before).expect("the preheader");
    let inserted = Operand::Value(function.instruction(inserted).result.expect("an insertelement"));
    let shuffled = function.create_instruction(
        Opcode::ShuffleVector(vec![0; vf as usize]),
        vector_ty,
        vec![inserted, poison],
        Flags::default(),
        Some("broadcast.splat"),
    );
    function.insert(shuffled, before).expect("the preheader");
    Operand::Value(function.instruction(shuffled).result.expect("a shufflevector"))
}

/// Part `part` of a widened operand: the wide value, or the loop-invariant
/// value broadcast once (`broadcast.splat`).
fn source_part(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    splat_block: BlockId,
    vector_ty: TypeId,
    vf: u32,
    source: Source,
    part: usize,
    wide: &[(InstId, Vec<ValueId>)],
    splats: &mut Vec<(Operand, Operand)>,
) -> Operand {
    match source {
        Source::Wide(def) => {
            Operand::Value(wide.iter().find(|(old, _)| *old == def).expect("the definition comes first").1[part])
        }
        Source::Splat(scalar) => match splats.iter().find(|(one, _)| *one == scalar) {
            Some((_, made)) => *made,
            None => {
                let made = broadcast(context, function, splat_block, vector_ty, vf, scalar);
                splats.push((scalar, made));
                made
            }
        },
    }
}

/// The wide loads or stores of one access: `IC` parts, part `k` at `k * VF`
/// elements past the first.
fn widen_access(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    widening: &Widening,
    plan: Plan,
    splat_block: BlockId,
    void_ty: TypeId,
    access: Access,
    wide: &mut Vec<(InstId, Vec<ValueId>)>,
    splats: &mut Vec<(Operand, Operand)>,
) {
    let before = Position::Before(access.inst);
    let vector_ty = context.types.intern(Type::Vector { element: access.element, count: plan.vf });
    let pointer = if access.is_load {
        function.instruction(access.inst).operands[0]
    } else {
        function.instruction(access.inst).operands[1]
    };
    let Operand::Value(pointer_value) = pointer else { unreachable!("an address is a gep") };
    let gep_flags = function
        .instruction(match function.value(pointer_value).def {
            llrm_mir::module::ValueDef::Instruction(def) => def,
            _ => unreachable!("an address is a gep"),
        })
        .flags;
    let ptr_ty = function.value(pointer_value).ty;
    let mut parts = Vec::new();
    for part in 0..plan.ic {
        // Part 0 uses the scalar address; part k is k x VF elements on from it.
        let address = if part == 0 {
            pointer
        } else {
            let offset = int_constant(context, widening.index_ty, u64::from(part) * u64::from(plan.vf));
            let gep = function.create_instruction(
                Opcode::GetElementPtr { source: access.element },
                ptr_ty,
                vec![pointer, offset],
                gep_flags,
                Some("gep.part"),
            );
            function.insert(gep, before).expect("the access's block");
            Operand::Value(function.instruction(gep).result.expect("a gep"))
        };
        if access.is_load {
            let load = function.create_instruction(
                Opcode::Load { align: Some(access.align), volatile: false },
                vector_ty,
                vec![address],
                Flags::default(),
                Some("wide.load"),
            );
            function.insert(load, before).expect("the access's block");
            parts.push(function.instruction(load).result.expect("a load"));
        } else {
            let value = source_part(
                context,
                function,
                splat_block,
                vector_ty,
                plan.vf,
                access.stored.expect("a store stores a value"),
                part as usize,
                wide,
                splats,
            );
            let store = function.create_instruction(
                Opcode::Store { align: Some(access.align), volatile: false },
                void_ty,
                vec![value, address],
                Flags::default(),
                None,
            );
            function.insert(store, before).expect("the access's block");
        }
    }
    if access.is_load {
        wide.push((access.inst, parts));
    }
}

/// `VPlan::execute` for the plan: IC parts of each access, the induction
/// stepped by VF x IC.
fn execute(
    context: &mut llrm_mir::context::Context,
    function: &mut Function,
    declared: &mut llrm_mir::passes::Declared,
    widening: &Widening,
    plan: Plan,
    splat_block: BlockId,
) {
    let vector_of = |context: &llrm_mir::context::Context, element: TypeId| {
        context.types.intern(Type::Vector { element, count: plan.vf })
    };
    let mut wide: Vec<(InstId, Vec<ValueId>)> = Vec::new();
    let mut erased: Vec<InstId> = Vec::new();
    let mut splats: Vec<(Operand, Operand)> = Vec::new();
    let void_ty = widening
        .steps
        .iter()
        .find_map(|one| match one {
            Step::Access(access) if !access.is_load => Some(function.instruction(access.inst).ty),
            _ => None,
        })
        .expect("a store");
    for step in &widening.steps {
        match *step {
            Step::Access(access) => {
                widen_access(context, function, widening, plan, splat_block, void_ty, access, &mut wide, &mut splats);
                erased.push(access.inst);
            }
            Step::Operation(operation) => {
                let before = Position::Before(operation.inst);
                let vector_ty = vector_of(context, operation.element);
                let operand_ty = vector_of(context, operation.operand_element);
                let mut parts = Vec::new();
                for part in 0..plan.ic as usize {
                    let arity = operation.op.arity();
                    let operands = operation
                        .operands
                        .map(
                            |source| source_part(
                                context,
                                function,
                                splat_block,
                                operand_ty,
                                plan.vf,
                                source,
                                part,
                                &wide,
                                &mut splats,
                            ),
                        );
                    let mut operands = operands[..arity].to_vec();
                    let opcode = match operation.op {
                        WideOp::Binary(op) => Opcode::Binary(op),
                        WideOp::FNeg => Opcode::FNeg,
                        WideOp::Cast(op) => Opcode::Cast(op),
                        WideOp::FMulAdd => {
                            // `llvm.fmuladd.v4f32` of the widened arguments.
                            let function_type = context
                                .types
                                .intern(
                                    Type::Function {
                                        returns: vector_ty,
                                        parameters: vec![vector_ty; 3],
                                        variadic: false,
                                    },
                                );
                            let name =
                                format!("llvm.fmuladd.{}", llrm_mir::intrinsics::mangle(&context.types, vector_ty));
                            let callee = declared.declare(&name, function_type);
                            let pointer = context.types.ptr(0);
                            operands.push(Operand::Constant(
                                context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }),
                            ));
                            Opcode::Call(Box::new(CallInfo {
                                function_type,
                                calling_convention: 0,
                                return_attrs: Vec::new(),
                                argument_attrs: vec![Vec::new(); 3],
                                attrs: Vec::new(),
                                tail: Tail::default(),
                            }))
                        }
                    };
                    let made = function.create_instruction(opcode, vector_ty, operands, operation.flags, None);
                    function.insert(made, before).expect("the operation's block");
                    parts.push(function.instruction(made).result.expect("a binary operation"));
                }
                wide.push((operation.inst, parts));
                erased.push(operation.inst);
            }
        }
    }
    // The stores first (they use the loads and operations), then the rest.
    for inst in erased.iter().rev() {
        function.erase(*inst).expect("an instruction nothing uses");
    }
    // The induction steps by VF x IC.
    let step = int_constant(context, widening.index_ty, u64::from(plan.vf) * u64::from(plan.ic));
    function.set_operand(widening.step, widening.step_operand, step);
}

impl FunctionPass for LoopVectorize {
    fn name(&self) -> &'static str {
        "loop-vectorize"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("loop-vectorize")
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if self.only_when_forced {
            return PreservedAnalyses::all();
        }
        let outer = std::rc::Rc::clone(analyses.outer());
        let target = outer.target();
        if !self.always && !target.has_vector_mir() {
            return PreservedAnalyses::all();
        }
        let Some(tti) = target.tti() else { return PreservedAnalyses::all() };
        if tti.vector_register_bits() == 0 {
            return PreservedAnalyses::all();
        }
        if llrm_support::debug::enabled("loop-vectorize-ir") {
            eprintln!("{}", llrm_mir::print::body(unit.context, unit.function));
        }
        let opt_size = has_opt_size(unit.function);
        // DEVIATE LV-OS (owner llrm-arm64-vec, exit 2026-11-24): the pin widens
        // an optsize loop whose trip count VF divides; here none, until
        // the cause card for the bytes it adds at -Os (histo, scroll,
        // jpeg) is closed.
        if opt_size {
            return PreservedAnalyses::all();
        }
        let mut changed = crate::loopsimplify::simplified(unit.function);
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let worklist: Vec<BlockId> = {
            let (_, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
            env.loops
                .get_loops_in_preorder()
                .into_iter()
                .filter(|one| one.is_innermost())
                .map(|one| one.get_header())
                .collect()
        };
        for header in worklist {
            let planned = {
                let (se, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                let cx = env.cx(&*unit.context, &*unit.function);
                let Some(found) = cx.loops.get_loop_for(header) else { continue };
                // The product of an earlier run (the vector loop, or its
                // remainder) is left alone.
                if let Some(latch) = found.get_loop_latch(cx.function).and_then(|latch| cx.function.terminator(latch))
                    && skeleton::already_vectorized(unit.metadata, cx.function, latch)
                {
                    why(unit, &outer, header, "already vectorized");
                    continue;
                }
                let memory_unit = memory::Unit::within(&*unit.context, unit.layout, &*unit.function, &outer);
                let mut aa = AggregateAA::new();
                aa.add_aa_result(BasicAA::new(&memory_unit));
                aa.add_aa_result(llrm_analysis::scopednoaliasaa::ScopedNoAliasAA::new(&memory_unit));
                aa.add_aa_result(TypeBasedAA::new(&memory_unit));
                let max_width = tti.vector_register_bits() * 2;
                let lai = LoopAccessInfo::new(se, &cx, header, &mut aa, false, max_width);
                let legality = match can_vectorize(&cx, se, &lai, found) {
                    Ok(legality) => legality,
                    Err(refusal) => {
                        why(unit, &outer, header, &format!("legality: {refusal:?}"));
                        continue;
                    }
                };
                let trip_count = u64::from(se.get_small_constant_trip_count(&cx, header));
                let dep = lai.dep_checker();
                let vector = match extract(&cx, se, found, &legality) {
                    Ok((recipes, widening)) => {
                        let facts = LoopFacts {
                            recipes,
                            trip_count,
                            max_safe_width_in_bits: dep.max_safe_vector_width_in_bits(),
                            safe_for_any_vector_width: dep.is_safe_for_any_vector_width(),
                            store_load_forward_bits: (!dep.is_safe_for_any_store_load_forward_distances())
                                .then(|| dep.store_load_forward_safe_distance_in_bits()),
                            widest_induction_bits: legality.widest_induction_bits.unwrap_or(64),
                            opt_size,
                        };
                        match plan(&*tti, &facts) {
                            Ok(chosen) => Some(Planned::Vector(chosen, widening)),
                            Err(not) => {
                                why(unit, &outer, header, &format!("plan: {not:?}"));
                                None
                            }
                        }
                    }
                    Err(Skipped(reason)) => {
                        why(unit, &outer, header, &format!("recipes: {reason}"));
                        None
                    }
                };
                // `computeBestVF` chose VF 1 (or the widening is not
                // translated): the loop may still be
                // interleaved (L8067).
                vector.or_else(|| {
                    let (primary, step) = primary_and_step(se, &legality).ok()?;
                    let extracted = match interleave::extract(se, &cx, found, &legality, &*tti, primary, step) {
                        Ok(extracted) => extracted,
                        Err(Skipped(reason)) => {
                            why(unit, &outer, header, &format!("interleave: {reason}"));
                            return None;
                        }
                    };
                    // LVI4: where a vector factor takes the loop, the pin
                    // vectorizes it; the widening is not translated here.
                    let priced = LoopFacts {
                        recipes: extracted.priced.clone(),
                        trip_count,
                        max_safe_width_in_bits: dep.max_safe_vector_width_in_bits(),
                        safe_for_any_vector_width: dep.is_safe_for_any_vector_width(),
                        store_load_forward_bits: (!dep.is_safe_for_any_store_load_forward_distances())
                            .then(|| dep.store_load_forward_safe_distance_in_bits()),
                        widest_induction_bits: legality.widest_induction_bits.unwrap_or(64),
                        opt_size,
                    };
                    if extracted.widening_mul || pin_may_vectorize(&*tti, &priced) {
                        why(
                            unit,
                            &outer,
                            header,
                            &format!(
                                "interleave: the pin may vectorize this loop ({}; casts, reductions and some operations are not widened yet)",
                                crate::loopvectorizeplan::describe_costs(&*tti, &priced)
                            ),
                        );
                        return None;
                    }
                    let facts = ScalarFacts {
                        cost: extracted.cost,
                        loads: extracted.loads,
                        stores: extracted.stores,
                        users: extracted.users,
                        invariants: extracted.invariants,
                        has_reductions: !extracted.interleaving.reductions.is_empty(),
                        nested: found.get_loop_depth() > 1,
                        trip_count,
                        safe_for_any_vector_width: dep.is_safe_for_any_vector_width(),
                        opt_size,
                    };
                    match choose_interleave(&*tti, &facts) {
                        Ok(ic) => {
                            why(unit, &outer, header, &format!("scalar loop costs: {}", facts.cost));
                            Some(Planned::Interleave(ic, extracted))
                        }
                        Err(not) => {
                            why(unit, &outer, header, &format!("interleave plan: {not:?}"));
                            None
                        }
                    }
                })
            };
            let Some(planned) = planned else { continue };
            let (lanes, shape, reduced) = match &planned {
                Planned::Vector(chosen, widening) => {
                    (u64::from(chosen.vf) * u64::from(chosen.ic), widening.shape, Vec::new())
                }
                Planned::Interleave(ic, extracted) => {
                    let latches = extracted.interleaving.reductions.iter().map(|one| one.latch).collect();
                    (u64::from(*ic), extracted.interleaving.shape, latches)
                }
            };
            let trip_count = {
                let (se, env) = session.split(unit.layout, &outer.globals, &outer.metadata);
                let cx = env.cx(&*unit.context, &*unit.function);
                u64::from(se.get_small_constant_trip_count(&cx, header))
            };
            // A constant trip count the lanes divide needs no remainder (LVP1).
            let with_middle = !reduced.is_empty();
            let skeleton = if trip_count != 0 && trip_count % lanes == 0 {
                let branch = unit.function.terminator(header).expect("a terminated loop");
                skeleton::mark_vectorized(unit.context, unit.declared, unit.metadata, unit.function, branch);
                let middle = with_middle.then(|| skeleton::add_middle(unit.function, shape));
                Skeleton { vector_ph: shape.preheader, middle, scalar_ph: None, remainder: None }
            } else if !exit_phis_are_invariant(unit.function, shape, &reduced) {
                why(unit, &outer, header, "recipes: An exit phi of a value the loop defines");
                continue;
            } else {
                match skeleton::add_remainder(
                    unit.context,
                    unit.function,
                    unit.declared,
                    unit.metadata,
                    shape,
                    lanes,
                    with_middle,
                ) {
                    Some(skeleton) => skeleton,
                    None => {
                        why(unit, &outer, header, "plan: The loop runs fewer iterations than one vector iteration");
                        continue;
                    }
                }
            };
            match planned {
                Planned::Vector(chosen, widening) => {
                    why(unit, &outer, header, &format!("vectorized VF {} IC {}", chosen.vf, chosen.ic));
                    execute(unit.context, unit.function, unit.declared, &widening, chosen, skeleton.vector_ph);
                }
                Planned::Interleave(ic, extracted) => {
                    why(unit, &outer, header, &format!("interleaved IC {ic}"));
                    interleave::execute(unit.context, unit.function, &extracted.interleaving, ic, &skeleton);
                }
            }
            changed = true;
            // The loop's body is new: the expressions are stale.
            session = ScevSession::of(unit.context, unit.function, &outer);
        }
        if changed && llrm_support::debug::enabled("loop-vectorize-ir-after") {
            eprintln!("{}", llrm_mir::print::body(unit.context, unit.function));
        }
        if changed { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// What the pass does to a loop.
enum Planned {
    /// Widen by VF lanes, interleaved IC times.
    Vector(Plan, Widening),
    /// Copy the body IC times (VF 1).
    Interleave(u32, Extracted),
}

/// The exit block's phis read values the loop does not define (LVS3): a
/// remainder adds an arm from the middle block, which has no iteration's value
/// to give, but for the last value of a reduction (`reductions`, the updates)
/// that block computes.
fn exit_phis_are_invariant(
    function: &Function,
    shape: Shape,
    reductions: &[InstId],
) -> bool {
    crate::edges::phis(function, shape.exit)
        .into_iter()
        .all(
            |phi| crate::lcssa::arms(function, phi)
                .into_iter()
                .all(
                    |(value, from)| from != shape.header
                        || match value {
                            Operand::Value(value) => match function.value(value).def {
                                llrm_mir::module::ValueDef::Instruction(def) => {
                                    function.parent(def) != Some(shape.header) || reductions.contains(&def)
                                }
                                _ => true,
                            },
                            _ => true,
                        },
                ),
        )
}

/// `LLVM_DEBUG` of LoopVectorize (D3), on `LLRM_DEBUG=loop-vectorize`: the
/// verdict on the loop.
fn why(
    unit: &passes::Unit,
    outer: &passes::Outer,
    header: BlockId,
    verdict: &str,
) {
    if llrm_support::debug::enabled("loop-vectorize") {
        let owner = unit.id.and_then(|id| (*outer.globals[id.0 as usize].name).clone()).unwrap_or_default();
        let labels = llrm_mir::print::block_labels(unit.function);
        eprintln!("LV: @{owner} %{}: {verdict}", labels[&header]);
    }
}

/// `Function::hasOptSize`.
fn has_opt_size(function: &Function) -> bool {
    use llrm_mir::opcode::Attribute;
    function
        .attrs
        .iter()
        .any(
            |attr| matches!(
                attr,
                Attribute::Flag(name) if name == "optsize" || name == "minsize"
            ),
        )
}

#[cfg(test)]
#[path = "loopvectorize_tests.rs"]
mod tests;
