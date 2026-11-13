//! Interleaving without vectorizing: `LoopVectorizePass` with VF 1 and IC > 1.
//!
//! # How it works
//! A translation of the interleave-only branch of LLVM `d1106deb71cc`
//! `LoopVectorize.cpp` `processLoop`
//! ([L8067](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorize.cpp#L8067),
//! "interleaved loop (interleaved count: N)"): `computeBestVF` chose VF 1, and
//! `selectInterleaveCount` (L3705,
//! [`crate::loopvectorizeplan::choose_interleave`]) chose IC > 1.
//! `VPlanTransforms::unrollByUF` ([VPlanUnroll.cpp
//! L504](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/VPlanUnroll.cpp#L504))
//! copies every recipe once per part; part `k` reads the induction as
//! `index + k`. Each reduction gets one phi per part, so the parts do not wait
//! for each other (`VPReductionPHIRecipe`), and the middle block folds the
//! parts' results (`bin.rdx`, `VPlanRecipes.cpp` L980). A remainder resumes the
//! reduction from `bc.merge.rdx`.
//!
//! # Example
//! ```text
//! loop: %i = phi [0, %ph], [%n, %loop]         loop: %i = phi [0, %ph], [%n, %loop]
//!       %s = phi [0, %ph], [%a, %loop]    =>         %s = phi [0, %ph], [%a, %loop]
//!       %v = load i16, gep %p, %i                    %s1 = phi [0, %ph], [%a1, %loop]
//!       %x = sext i16 %v to i64                      %v = load .. ; %i1 = add %i, 1 ; %v1 = load gep %p, %i1
//!       %a = add %s, %x                              %a = add %s, %x ; %a1 = add %s1, %x1
//!       %n = add %i, 1                               %n = add %i, 2
//! exit: phi [%a, %loop]                        middle: %bin.rdx = add %a1, %a
//! ```
//!
//! # DEVIATE rows
//! - LVI1 the loop is copied in place (as LVP1), not rebuilt from a VPlan; the
//!   scalar cost of an extension of a load is 0 for every target (the pin asks
//!   the target about the load's use). Exit: VPlan.
//! - LVI2 the reductions are the kinds with an identity (`add`, `mul`, `and`,
//!   `or`, `xor`) whose final value is the only value used after the loop.
//!   Exit: min/max (`select`/`icmp` chains), any-of and find-last.
//! - LVI4 the pin decides VF first (`computeBestVF`): a loop it vectorizes is
//!   not interleaved alone. The widening of casts and reductions is not
//!   translated (LV-X4), so the loop is interleaved only if no vector factor
//!   takes its loads, stores and arithmetic (an `i64` multiply has no NEON
//!   form); a cast or reduction a vector factor would also price is not priced.
//!   Exit: the vector cast and reduction costs with LV-X4.
//! - LVI3 the register pressure is counted on the loop's instructions in order,
//!   one register per live value (`calculateRegisterUsageForPlan`, L106, at VF
//!   1); invariants are the instructions defined outside and read inside.
//!
//! # Runs
//! After the vector plan of [`crate::loopvectorize`] did not widen the loop.

use llrm_analysis::ivdescriptors::RecurKind;
use llrm_analysis::loopaccess::{SymbolicStrideMap, get_ptr_stride};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::{Position, ValueMap};
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::tti::{ArithOp, CastKind, CmpSel, CostKind, MemoryOp, Predicate, Tti, Ty};
use llrm_mir::types::{FloatKind, Type, TypeId};

use crate::lcssa::{arms, from_arms};
use crate::loopvectorize::Skipped;
use crate::loopvectorizelegality::Legality;
use crate::loopvectorizeplan::{Recipe, scalar_control_cost};
use crate::loopvectorizeskeleton::{Shape, Skeleton};

/// A reduction of the loop: its phi, the recurrence kind, the instruction that
/// feeds the phi back and is also the value used after the loop.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reduction {
    pub phi: InstId,
    pub kind: RecurKind,
    pub latch: InstId,
    pub start: Operand,
    pub ty: TypeId,
}

/// What the transform needs to interleave a planned loop.
pub(crate) struct Interleaving {
    pub shape: Shape,
    /// The `add` that steps the induction, and the operand that is the
    /// constant step.
    pub step: InstId,
    pub step_operand: usize,
    /// The step's value is read by the body (`sum += a[i] * (i + 1)`): it is
    /// copied like any instruction and the induction gets an increment of its
    /// own.
    pub step_in_body: bool,
    /// The instructions copied once per part, in order.
    pub body: Vec<InstId>,
    pub reductions: Vec<Reduction>,
}

/// The `ArithOp` that prices an integer `BinaryOp`, if the cost model knows it.
fn arith_of(op: BinaryOp) -> Option<ArithOp> {
    Some(match op {
        BinaryOp::Add => ArithOp::Add,
        BinaryOp::Sub => ArithOp::Sub,
        BinaryOp::Mul => ArithOp::Mul,
        BinaryOp::SDiv => ArithOp::SDiv,
        BinaryOp::UDiv => ArithOp::UDiv,
        BinaryOp::SRem => ArithOp::SRem,
        BinaryOp::URem => ArithOp::URem,
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
        BinaryOp::FRem => ArithOp::FRem,
    })
}

pub(crate) fn cast_kind(op: CastOp) -> Option<CastKind> {
    Some(match op {
        CastOp::Trunc => CastKind::Trunc,
        CastOp::ZExt => CastKind::ZExt,
        CastOp::SExt => CastKind::SExt,
        CastOp::FPTrunc => CastKind::FPTrunc,
        CastOp::FPExt => CastKind::FPExt,
        CastOp::FPToUI => CastKind::FPToUI,
        CastOp::FPToSI => CastKind::FPToSI,
        CastOp::UIToFP => CastKind::UIToFP,
        CastOp::SIToFP => CastKind::SIToFP,
        CastOp::BitCast => CastKind::BitCast,
        CastOp::PtrToInt | CastOp::IntToPtr | CastOp::FPRound | CastOp::AddrSpaceCast => return None,
    })
}

/// The operation a reduction of `kind` folds with, and its identity.
fn reduction_op(kind: RecurKind) -> Option<(BinaryOp, u128)> {
    Some(match kind {
        RecurKind::Add => (BinaryOp::Add, 0),
        RecurKind::Mul => (BinaryOp::Mul, 1),
        RecurKind::Or => (BinaryOp::Or, 0),
        RecurKind::Xor => (BinaryOp::Xor, 0),
        RecurKind::And => (BinaryOp::And, u128::MAX),
        _ => return None,
    })
}

fn int_ty(
    cx: &Cx,
    ty: TypeId,
) -> Option<Ty> {
    match cx.context.types.get(ty) {
        Type::Int(bits @ (1 | 8 | 16 | 32 | 64)) => Some(Ty::Int(*bits)),
        _ => None,
    }
}

/// An element the cost model prices: an integer of a width the target has, or a
/// `float` or `double`.
fn scalar_ty(
    cx: &Cx,
    ty: TypeId,
) -> Option<Ty> {
    match cx.context.types.get(ty) {
        Type::Float(FloatKind::Float) => Some(Ty::Float),
        Type::Float(FloatKind::Double) => Some(Ty::Double),
        _ => int_ty(cx, ty),
    }
}

/// What the planner reads of an interleaved loop, and what is rewritten.
pub(crate) struct Extracted {
    pub interleaving: Interleaving,
    /// `expectedCost(VF=1)`.
    pub cost: i64,
    pub loads: u32,
    pub stores: u32,
    /// `MaxLocalUsers` and `LoopInvariantRegs` of the scalar register class.
    pub users: u32,
    pub invariants: u32,
    /// The accesses and operations a vector plan would price (casts, compares
    /// and addresses are not priced here): if a vector factor takes them, the
    /// pin vectorizes the loop and does not interleave it alone (LVI4).
    pub priced: Vec<Recipe>,
    /// A multiply of two extended values: the pin takes it as a widening
    /// multiply (`isWideningInstruction`), not the scalarized one of an
    /// unextended `i64` (LVI4).
    pub widening_mul: bool,
}

/// The body of the single-block loop of `the_loop`, or why it is not
/// interleaved.
pub(crate) fn extract(
    se: &mut ScalarEvolution,
    cx: &Cx,
    the_loop: Loop,
    legality: &Legality,
    tti: &dyn Tti,
    primary: InstId,
    step: InstId,
) -> Result<Extracted, Skipped> {
    let function = cx.function;
    let header = the_loop.get_header();
    let kind = CostKind::RecipThroughput;
    let strides = SymbolicStrideMap::default();
    let primary_phi = Operand::Value(function.instruction(primary).result.expect("a phi"));
    let index_ty = function.instruction(primary).ty;
    let preheader = the_loop.get_loop_preheader(function).ok_or(Skipped("The loop has no preheader"))?;
    let exit = the_loop.get_unique_exit_block(function).ok_or(Skipped("The loop has no unique exit block"))?;
    let outside = |value: ValueId, allowed_exit_phi: bool| -> bool {
        function
            .users(value)
            .iter()
            .any(
                |one| function.parent(one.user) != Some(header)
                    && !(allowed_exit_phi
                        && function.parent(one.user) == Some(exit)
                        && function.instruction(one.user).opcode == Opcode::Phi),
            )
    };

    // The exit test: the stepped induction against a loop-invariant bound.
    let terminator = function.terminator(header).ok_or(Skipped("The loop has no terminator"))?;
    let condition = match function.instruction(terminator).operands.first() {
        Some(Operand::Value(value)) => *value,
        _ => return Err(Skipped("The loop exit is not a conditional branch")),
    };
    let compare = match function.value(condition).def {
        ValueDef::Instruction(def) if function.parent(def) == Some(header) => def,
        _ => return Err(Skipped("The exit condition is not computed in the loop")),
    };
    let invariant = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => function.parent(def) != Some(header),
            _ => true,
        },
        _ => true,
    };
    let stepped = function.instruction(step).result.map(Operand::Value);
    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else {
        return Err(Skipped("The exit test is not an integer comparison"));
    };
    let (a, b) = (function.instruction(compare).operands[0], function.instruction(compare).operands[1]);
    let bound_operand = if Some(a) == stepped && invariant(b) {
        1
    } else if Some(b) == stepped && invariant(a) {
        0
    } else {
        return Err(Skipped("The exit test is not the stepped induction against a loop-invariant bound"));
    };
    if !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
        return Err(Skipped("The exit test is not an equality"));
    }
    let step_data = function.instruction(step);
    let step_operand = match (step_data.operands[0], step_data.operands[1]) {
        (a, Operand::Constant(_)) if a == primary_phi => 1,
        (Operand::Constant(_), b) if b == primary_phi => 0,
        _ => return Err(Skipped("The induction step is not a constant")),
    };
    if outside(function.instruction(primary).result.expect("a phi"), false)
        || outside(step_data.result.expect("an add"), false)
    {
        return Err(Skipped("The induction is used after the loop"));
    }

    let step_result = step_data.result.expect("an add");
    let step_in_body = function
        .users(step_result)
        .iter()
        .any(|one| one.user != compare && one.user != primary && function.parent(one.user) == Some(header));
    let mut cost = scalar_control_cost(tti);
    let (mut loads, mut stores) = (0u32, 0u32);
    let mut body = Vec::new();
    let mut priced = Vec::new();
    let mut widening_mul = false;
    let mut reductions = Vec::new();
    for inst in function.iter_block(header).collect::<Vec<_>>() {
        let data = function.instruction(inst);
        if inst == primary || (inst == step && !step_in_body) || inst == compare || inst == terminator {
            continue;
        }
        match data.opcode {
            Opcode::Phi => {
                let Some((_, descriptor)) = legality.reductions.iter().find(|(phi, _)| *phi == inst) else {
                    return Err(Skipped("A header phi other than the induction and the reductions"));
                };
                if reduction_op(descriptor.kind).is_none() {
                    return Err(Skipped("A reduction that is not add, mul, and, or or xor"));
                }
                if descriptor.has_uses_outside_reduction_chain() || descriptor.intermediate_store.is_some() {
                    return Err(Skipped("A reduction used other than by its chain"));
                }
                let back = arms(function, inst).into_iter().find(|&(_, from)| from == header).map(|(value, _)| value);
                let latch = match back {
                    Some(Operand::Value(value)) => match function.value(value).def {
                        ValueDef::Instruction(def) if function.parent(def) == Some(header) => def,
                        _ => return Err(Skipped("A reduction whose update is not in the loop")),
                    },
                    _ => return Err(Skipped("A reduction whose update is not in the loop")),
                };
                if descriptor.loop_exit_instr.is_some_and(|one| one != latch) {
                    return Err(Skipped("A reduction whose used value is not its update"));
                }
                let result = function.instruction(inst).result.expect("a phi");
                if outside(result, false) {
                    return Err(Skipped("A reduction phi used after the loop"));
                }
                reductions.push(Reduction {
                    phi: inst,
                    kind: descriptor.kind,
                    latch,
                    start: descriptor.start_value,
                    ty: function.instruction(inst).ty,
                });
                cost += tti.cf_instr_cost(true, kind).value;
            }
            Opcode::GetElementPtr { .. } => body.push(inst),
            Opcode::Load { volatile, .. } | Opcode::Store { volatile, .. } => {
                if volatile {
                    return Err(Skipped("A volatile access"));
                }
                let is_load = matches!(data.opcode, Opcode::Load { .. });
                let (value_ty, ptr) = if is_load {
                    (data.ty, data.operands[0])
                } else {
                    let stored =
                        function.operand_type(cx.context, data.operands[0]).ok_or(Skipped("An untyped store"))?;
                    (stored, data.operands[1])
                };
                let lane = match cx.context.types.get(value_ty) {
                    Type::Pointer(_) => Ty::Pointer,
                    _ => scalar_ty(cx, value_ty).ok_or(Skipped("An element type the target has no cost for"))?,
                };
                let op = if is_load { MemoryOp::Load } else { MemoryOp::Store };
                let stored_constant = !is_load && matches!(data.operands[0], Operand::Constant(_));
                let price = tti.memory_op_cost(op, lane, kind, stored_constant);
                if !price.valid {
                    return Err(Skipped("An access the target does not price"));
                }
                let scalar = price.value + tti.address_computation_cost(kind).value;
                cost += scalar;
                let align = match data.opcode {
                    Opcode::Load { align, .. } | Opcode::Store { align, .. } => align,
                    _ => None,
                }
                .unwrap_or(cx.layout.size_bits(&cx.context.types, value_ty) / 8);
                // `getWideningDecision`: consecutive addresses widen; a uniform
                // address, a reversal and a small stride (an
                // interleave group) have costs of their own, which this port
                // does not price; any other pattern is
                // scalarized (NEON has no gather or scatter).
                priced.push(match get_ptr_stride(se, cx, value_ty, ptr, header, &strides, true) {
                    Some(1) => {
                        if is_load {
                            Recipe::Load { lane, align }
                        } else {
                            Recipe::Store { lane, align }
                        }
                    }
                    Some(-4..=4) => Recipe::Unpriced { scalar_cost: scalar },
                    _ => Recipe::Scalarized { op, lane },
                });
                if is_load {
                    loads += 1;
                } else {
                    stores += 1;
                }
                body.push(inst);
            }
            // The step's `add` is the loop control's, priced once there.
            Opcode::Binary(_) if inst == step => body.push(inst),
            Opcode::Binary(op) => {
                let arith = arith_of(op).ok_or(Skipped("A binary operation the cost model does not know"))?;
                let ty = scalar_ty(cx, data.ty).ok_or(Skipped("An element type the target has no cost for"))?;
                let price = tti.arithmetic_cost(arith, ty, kind, Default::default(), Default::default());
                if !price.valid {
                    return Err(Skipped("An operation the target does not price"));
                }
                cost += price.value;
                priced.push(Recipe::Binary { op: arith, lane: ty, ext: [None, None] });
                let extended = |operand: Operand| {
                    matches!(
                        operand,
                        Operand::Value(value) if matches!(
                            function.value(value).def,
                            ValueDef::Instruction(def) if matches!(
                                function.instruction(def).opcode,
                                Opcode::Cast(CastOp::SExt | CastOp::ZExt)
                            )
                        )
                    )
                };
                widening_mul |= matches!(op, BinaryOp::Mul) && data.operands.iter().all(|&operand| extended(operand));
                body.push(inst);
            }
            Opcode::Cast(op) => {
                let ck = cast_kind(op).ok_or(Skipped("A cast the cost model does not know"))?;
                let dst = scalar_ty(cx, data.ty).ok_or(Skipped("An element type the target has no cost for"))?;
                let src_ty = function.operand_type(cx.context, data.operands[0]).ok_or(Skipped("An untyped cast"))?;
                let src = scalar_ty(cx, src_ty).ok_or(Skipped("An element type the target has no cost for"))?;
                // LVI1: an extension of a load is folded into it.
                let of_load = matches!(
                    data.operands[0],
                    Operand::Value(value) if matches!(
                        function.value(value).def,
                        ValueDef::Instruction(def) if matches!(function.instruction(def).opcode, Opcode::Load { .. })
                    )
                );
                let folded = of_load && matches!(ck, CastKind::ZExt | CastKind::SExt);
                if !folded {
                    let price = tti.cast_cost(ck, dst, src, kind);
                    if !price.valid {
                        return Err(Skipped("A cast the target does not price"));
                    }
                    cost += price.value;
                }
                priced.push(Recipe::Cast { kind: ck, dst, src, folded, users: Vec::new() });
                body.push(inst);
            }
            Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::Select => {
                let select = matches!(data.opcode, Opcode::Select);
                let operand_ty = function
                    .operand_type(cx.context, if select { data.operands[1] } else { data.operands[0] })
                    .ok_or(Skipped("An untyped comparison"))?;
                let ty = scalar_ty(cx, operand_ty).ok_or(Skipped("An element type the target has no cost for"))?;
                let op = match data.opcode {
                    Opcode::Select => CmpSel::Select,
                    Opcode::FCmp(_) => CmpSel::FCmp,
                    _ => CmpSel::ICmp,
                };
                let own = match data.opcode {
                    Opcode::ICmp(predicate) => Some(Predicate::Int(predicate)),
                    Opcode::FCmp(predicate) => Some(Predicate::Float(predicate)),
                    _ => None,
                };
                // The compare that makes a select's condition, where the loop
                // does: the target prices a select by it.
                let feeding = match data.operands[0] {
                    Operand::Value(value) if select => match function.value(value).def {
                        ValueDef::Instruction(def) if function.parent(def) == Some(header) => {
                            match function.instruction(def).opcode {
                                Opcode::ICmp(predicate) => Some(Predicate::Int(predicate)),
                                Opcode::FCmp(predicate) => Some(Predicate::Float(predicate)),
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let price = tti.cmp_sel_cost_for(op, ty, kind, own.or(feeding));
                if !price.valid {
                    return Err(Skipped("A comparison the target does not price"));
                }
                cost += price.value;
                priced.push(match (select, own, feeding) {
                    (false, Some(predicate), _) => Recipe::Compare { op, lane: ty, predicate },
                    // A select by a condition the loop makes widens to a vector
                    // select; any other (a loop-invariant condition) is a
                    // different recipe to the pin.
                    (true, _, Some(_)) => Recipe::Select { lane: ty, predicate: feeding },
                    _ => Recipe::Unpriced { scalar_cost: price.value },
                });
                body.push(inst);
            }
            _ => return Err(Skipped("An instruction the interleaver does not copy")),
        }
    }
    if body.is_empty() {
        return Err(Skipped("An empty loop body"));
    }
    // A value of the loop is used after it only as a reduction's last value, by
    // a phi of the exit block (LCSSA).
    for &inst in &body {
        if let Some(result) = function.instruction(inst).result {
            let is_reduction_value = reductions.iter().any(|one| one.latch == inst);
            if outside(result, is_reduction_value) {
                return Err(Skipped("A value of the loop used after it"));
            }
        }
    }
    for reduction in &reductions {
        if !body.contains(&reduction.latch) {
            return Err(Skipped("A reduction whose update is not copied"));
        }
    }

    let (users, invariants) = register_usage(function, header, &body, &reductions, primary);
    let shape = Shape { header, preheader, exit, primary, compare, bound_operand, index_ty, reverse: false, tests_phi: false };
    Ok(Extracted {
        interleaving: Interleaving { shape, step, step_operand, step_in_body, body, reductions },
        cost,
        loads,
        stores,
        users,
        invariants,
        priced,
        widening_mul,
    })
}

/// `calculateRegisterUsageForPlan` (VPlanAnalysis.cpp L106) at VF 1: the most
/// values of the loop live at once, and the values defined outside that it
/// reads (LVI3). A value is live from its definition to its last use; the
/// induction and the reductions live across the whole body.
fn register_usage(
    function: &Function,
    header: BlockId,
    body: &[InstId],
    reductions: &[Reduction],
    primary: InstId,
) -> (u32, u32) {
    let order: Vec<InstId> = function.iter_block(header).collect();
    let position = |inst: InstId| order.iter().position(|&one| one == inst);
    let mut last_use: Vec<(InstId, usize)> = Vec::new();
    let mut invariants: Vec<ValueId> = Vec::new();
    for (index, &inst) in order.iter().enumerate() {
        for operand in &function.instruction(inst).operands {
            let Operand::Value(value) = *operand else { continue };
            match function.value(value).def {
                ValueDef::Instruction(def) if function.parent(def) == Some(header) => {
                    match last_use.iter_mut().find(|(one, _)| *one == def) {
                        Some(entry) => entry.1 = index,
                        None => last_use.push((def, index)),
                    }
                }
                ValueDef::Instruction(_) if !invariants.contains(&value) => invariants.push(value),
                _ => {}
            }
        }
    }
    let end = order.len();
    // The induction is read by the step at the end of the body; a reduction
    // phi lives until its update.
    for one in std::iter::once(primary).chain(reductions.iter().map(|one| one.phi)) {
        match last_use.iter_mut().find(|(def, _)| *def == one) {
            Some(entry) if one == primary => entry.1 = end,
            Some(_) => {}
            None => last_use.push((one, end)),
        }
    }
    let mut most = 1u32;
    for index in 0..order.len() {
        let live = last_use
            .iter()
            .filter(|&&(def, last)| {
                function.instruction(def).result.is_some()
                    && position(def).is_some_and(|at| at <= index)
                    && last > index
                    && (body.contains(&def) || def == primary || reductions.iter().any(|one| one.phi == def))
            })
            .count() as u32;
        most = most.max(live);
    }
    (most, invariants.len() as u32)
}

fn int_constant(
    context: &Context,
    ty: TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }))
}

/// The identity of a reduction as a constant of its type.
fn identity(
    context: &Context,
    reduction: &Reduction,
    value: u128,
) -> Operand {
    let bits = match context.types.get(reduction.ty) {
        Type::Int(bits) => *bits,
        other => unreachable!("a reduction of {other:?}"),
    };
    let mask = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
    int_constant(context, reduction.ty, value & mask)
}

/// `VPlanTransforms::unrollByUF` for `ic` parts, and the middle block's
/// `bin.rdx`: each instruction of the body copied `ic - 1` times after itself
/// with the induction read as `index + k` and each reduction on its own phi;
/// the induction steps by `ic`.
pub(crate) fn execute(
    context: &mut Context,
    function: &mut Function,
    interleaving: &Interleaving,
    ic: u32,
    skeleton: &Skeleton,
) {
    let Interleaving { shape, step, step_operand, step_in_body, body, reductions } = interleaving;
    let header = shape.header;
    let parts = ic as usize;
    let first =
        function.iter_block(header).find(|&inst| function.instruction(inst).opcode != Opcode::Phi).expect("a body");
    let primary_value = function.instruction(shape.primary).result.expect("a phi");

    // part k's view of the loop-variant values.
    let mut maps: Vec<ValueMap> = (0..parts).map(|_| ValueMap::default()).collect();
    // One phi per part and reduction, starting from the identity.
    let mut part_phis: Vec<Vec<InstId>> = Vec::new();
    for reduction in reductions {
        let original = function.instruction(reduction.phi).result.expect("a phi");
        let mut phis = vec![reduction.phi];
        for part in 1..parts {
            let phi =
                function.create_instruction(Opcode::Phi, reduction.ty, Vec::new(), Flags::default(), Some("vec.phi"));
            function.insert(phi, Position::Before(first)).expect("the header");
            let value = Operand::Value(function.instruction(phi).result.expect("a phi"));
            maps[part].values.insert(original, value);
            phis.push(phi);
        }
        part_phis.push(phis);
    }

    for part in 1..parts {
        let offset = int_constant(context, shape.index_ty, part as u128);
        let index = function.create_instruction(
            Opcode::Binary(BinaryOp::Add),
            shape.index_ty,
            vec![Operand::Value(primary_value), offset],
            Flags::default(),
            Some("index.part"),
        );
        function.insert(index, Position::Before(first)).expect("the body");
        maps[part].values.insert(primary_value, Operand::Value(function.instruction(index).result.expect("an add")));
    }

    // The copies, each after the original or its previous copy.
    let mut copies: Vec<Vec<InstId>> = vec![Vec::new(); parts];
    for &inst in body {
        let mut after = inst;
        for part in 1..parts {
            let copy = function.clone_instruction(inst);
            function.remap_instruction(copy, &maps[part]);
            function.insert(copy, Position::After(after)).expect("the body");
            if let (Some(from), Some(to)) = (function.instruction(inst).result, function.instruction(copy).result) {
                maps[part].values.insert(from, Operand::Value(to));
                if let Some(name) = function.value(from).name.clone() {
                    function.set_name(to, &format!("{name}.{part}"));
                }
            }
            copies[part].push(copy);
            after = copy;
        }
    }

    // The phis' arms: the identity on entry, the part's update on the back
    // edge.
    let enter = skeleton.vector_ph;
    for (reduction, phis) in reductions.iter().zip(&part_phis) {
        let (_, neutral) = reduction_op(reduction.kind).expect("a kind with an identity");
        let original = function.instruction(reduction.latch).result.expect("an update");
        for part in 1..parts {
            let start = identity(context, reduction, neutral);
            let update = *maps[part].values.get(&original).expect("a copy of the update");
            function.set_operands(phis[part], from_arms(&[(start, enter), (update, header)]));
        }
    }

    // The induction steps by the lanes.
    let lanes = int_constant(context, shape.index_ty, u128::from(ic));
    if *step_in_body {
        // The body reads the step (`i + 1`), so it stays; `index.next` is new.
        let stepped = Operand::Value(function.instruction(*step).result.expect("an add"));
        let next = function.create_instruction(
            Opcode::Binary(BinaryOp::Add),
            shape.index_ty,
            vec![Operand::Value(primary_value), lanes],
            Flags::default(),
            Some("index.next"),
        );
        function.insert(next, Position::Before(shape.compare)).expect("the exit test's block");
        let next = Operand::Value(function.instruction(next).result.expect("an add"));
        let back: Vec<_> = arms(function, shape.primary)
            .into_iter()
            .map(|(v, from)| (if v == stepped { next } else { v }, from))
            .collect();
        function.set_operands(shape.primary, from_arms(&back));
        function.set_operand(shape.compare, 1 - shape.bound_operand, next);
    } else {
        function.set_operand(*step, *step_operand, lanes);
    }

    // middle.block: fold the parts (`bin.rdx`), then hand the result on.
    let Some(middle) = skeleton.middle else {
        debug_assert!(reductions.is_empty(), "reductions need a middle block");
        return;
    };
    let before = Position::Before(function.terminator(middle).expect("a terminated middle block"));
    let mut folded: Vec<Operand> = Vec::new();
    for reduction in reductions {
        let (op, _) = reduction_op(reduction.kind).expect("a kind with an identity");
        let original = function.instruction(reduction.latch).result.expect("an update");
        let mut total = Operand::Value(original);
        for part in 1..parts {
            let value = *maps[part].values.get(&original).expect("a copy of the update");
            let made = function.create_instruction(
                Opcode::Binary(op),
                reduction.ty,
                vec![value, total],
                Flags::default(),
                Some("bin.rdx"),
            );
            function.insert(made, before).expect("the middle block");
            total = Operand::Value(function.instruction(made).result.expect("a binary operation"));
        }
        folded.push(total);
    }
    let exit = shape.exit;
    let remainder_block = skeleton.remainder.as_ref().map(|(block, _)| *block);
    for (reduction, total) in reductions.iter().zip(&folded) {
        let original = Operand::Value(function.instruction(reduction.latch).result.expect("an update"));
        let copy_in_remainder = skeleton.remainder.as_ref().map(|(_, pairs)| {
            let phi = pairs.iter().find(|(from, _)| *from == reduction.phi).expect("the copy of the phi").1;
            let latch = pairs.iter().find(|(from, _)| *from == reduction.latch).expect("the copy of the update").1;
            (phi, Operand::Value(function.instruction(latch).result.expect("an update")))
        });
        // The exit block reads the folded value from the middle block, and the
        // remainder's own last value from the remainder.
        for phi in crate::edges::phis(function, exit) {
            let mut incoming = arms(function, phi);
            let mut changed = false;
            for (value, from) in &mut incoming {
                if *value != original {
                    continue;
                }
                if *from == middle {
                    *value = *total;
                    changed = true;
                } else if Some(*from) == remainder_block
                    && let Some((_, last)) = copy_in_remainder
                {
                    *value = last;
                    changed = true;
                }
            }
            if changed {
                function.set_operands(phi, from_arms(&incoming));
            }
        }
        // The remainder resumes the reduction from the folded value
        // (`bc.merge.rdx`), or from the start where the vector loop was
        // skipped.
        if let Some((phi, _)) = copy_in_remainder {
            let (enter_from, merged) = match skeleton.scalar_ph {
                Some(scalar_ph) => {
                    let merge = function.create_instruction(
                        Opcode::Phi,
                        reduction.ty,
                        Vec::new(),
                        Flags::default(),
                        Some("bc.merge.rdx"),
                    );
                    let at = function.iter_block(scalar_ph).next().expect("a terminated block");
                    function.insert(merge, Position::Before(at)).expect("scalar.ph");
                    let merge_value = Operand::Value(function.instruction(merge).result.expect("a phi"));
                    function.set_operands(merge, from_arms(&[(*total, middle), (reduction.start, shape.preheader)]));
                    (scalar_ph, merge_value)
                }
                None => (middle, *total),
            };
            let mut incoming = arms(function, phi);
            for (value, from) in &mut incoming {
                if *from != remainder_block.expect("a remainder") {
                    *value = merged;
                    *from = enter_from;
                }
            }
            function.set_operands(phi, from_arms(&incoming));
        }
    }
}
