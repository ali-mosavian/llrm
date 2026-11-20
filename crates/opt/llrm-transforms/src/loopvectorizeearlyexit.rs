//! Early-exit loops: the plan of several blocks and the control flow around it
//! (`handleUncountableEarlyExits`, `createVectorLoopSkeleton`).
//!
//! # How it works
//! A loop that leaves where a computed condition says so, read-only and with
//! loads dereferenceable for every trip (legality:
//! `isVectorizableEarlyExitLoop`), is widened into a vector loop of its own and
//! the original loop stays as the scalar remainder. The vector loop runs `VF`
//! iterations a trip; its exit is the pin's `BranchOnTwoConds` of the any-of of
//! the exit condition and the count (`expandBranchOnTwoConds` makes the two
//! branches). IC is 1 (`selectInterleaveCount`: `Plan.hasEarlyExit()`).
//!
//! ```text
//! preheader:                            vector.ph:
//!   %min.iters.check = icmp ult n, VF     %n.vec = n - n % VF
//!   br %min, scalar.ph, vector.ph         br vector.body
//! vector.body:
//!   %index = phi [start, vector.ph], [%index.next, vector.body.interim]
//!   wide loads, compare:      %mask = icmp ne <VF x i8> ..
//!   %index.next = add %index, VF
//!   %any = call @llvm.vector.reduce.or(%mask)      (AnyOf)
//!   br %any, vector.early.exit, vector.body.interim
//! vector.body.interim:   br (%index.next == %ind.end), middle.block, vector.body
//! vector.early.exit:     the exit phis' values at the first set lane, br early exit
//! middle.block:          br (n == n.vec), countable exit, scalar.ph
//! scalar.ph:             %bc.resume.val = phi [%ind.end, middle.block], [start, preheader]
//! body (the original):   from %bc.resume.val
//! ```
//!
//! # DEVIATE rows
//! - VPEE1 one uncountable exit, and the countable one is the latch's (the pin
//!   chains `vector.early.exit.check` blocks for several). Exit: the chain.
//! - VPEE2 the recipes are loads of consecutive addresses, integer operations
//!   on loaded and loop-invariant values, and one integer compare as the exit
//!   condition (`i1` operations and selects are left scalar).
//! - VPEE3 `FirstActiveLane` is the `reduce.umin` of the lane numbers the mask
//!   selects, not `llvm.experimental.cttz.elts` (llrm has no such intrinsic,
//!   and its verifier takes no `<N x i1>` bitcast to an integer).
//! - VPEE4 the trip count is `bound - start` (LVS1 of the skeleton): an
//!   induction stepping by one against an `eq`/`ne` test of the stepped value.
//! - VPEE5 the cost of the any-of reduction (`VPInstruction::AnyOf`, an `or`
//!   reduction of the mask per vector iteration) and of `vector.early.exit`
//!   (`calculateEarlyExitCost`) is not added to the plan, so a VF may be chosen
//!   that the pin's prices would refuse.
//! - VPEE6 a port takes the loops only where `Tti::vectorizes_early_exit` says
//!   its backend selects the vector loop (x86-64).
//!
//! The rows' owner is x87-cordic-2, exit 2026-12-10.
//!
//! # Example
//! A read-only search loop that leaves at the first match gets a vector loop in front of it; the original loop finishes the rest.
//!
//! ```text
//! for (i = 0; i != n; i++) if (a[i] == x) break;
//! =>
//! vector.body: %mask = icmp eq <VF x i32> %wide.load, %x.splat
//!              %any = reduce.or(%mask); br %any, vector.early.exit, vector.body.interim
//! scalar.ph:   the original loop, from %bc.resume.val
//! ```

use llrm_analysis::ivdescriptors::InductionKind;
use llrm_analysis::loopaccess::{SymbolicStrideMap, get_ptr_stride};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, MetadataNode, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode, Tail};
use llrm_mir::passes::Declared;
use llrm_mir::tti::{ArithOp, CmpSel, Predicate, Tti};
use llrm_mir::types::{Type, TypeId};

use crate::edges;
use crate::lcssa::{arms, from_arms};
use crate::loopvectorize::{Skipped, broadcast, element_ty, primary_and_step};
use crate::loopvectorizelegality::{EarlyExit, Legality};
use crate::loopvectorizeplan::{LoopFacts, Plan, Recipe, plan};
use crate::loopvectorizeskeleton::{bits_of, constant_int, emit, end_with, mark_vectorized, mask};

/// What an operand of a widened instruction is.
#[derive(Clone, Copy, Debug)]
enum Source {
    /// The result of a load or operation of the loop: its wide value.
    Wide(InstId),
    /// A loop-invariant value, broadcast in `vector.ph`.
    Splat(Operand),
}

/// A widened instruction of the loop, in program order.
#[derive(Clone, Debug)]
enum Step {
    Load { inst: InstId, element: TypeId, align: u64 },
    Binary { inst: InstId, op: BinaryOp, flags: Flags, element: TypeId, operands: [Source; 2] },
    ICmp { inst: InstId, predicate: IntPredicate, operand_element: TypeId, operands: [Source; 2] },
}

impl Step {
    fn inst(&self) -> InstId {
        match self {
            Step::Load { inst, .. } | Step::Binary { inst, .. } | Step::ICmp { inst, .. } => *inst,
        }
    }
}

/// The loop of several blocks and what the vector loop and the blocks around
/// it are made of.
#[derive(Clone, Debug)]
pub(crate) struct EarlyExitPlan {
    steps: Vec<Step>,
    /// The loop's blocks, header first, in execution order.
    blocks: Vec<BlockId>,
    /// The compare whose set lanes leave the loop, and whether the branch
    /// leaves on true.
    condition: InstId,
    exit_when_true: bool,
    preheader: BlockId,
    latch: BlockId,
    /// The block that leaves early, and where it leaves to.
    exiting: BlockId,
    early_exit: BlockId,
    /// Where the latch leaves to.
    countable_exit: BlockId,
    primary: InstId,
    start: Operand,
    bound: Operand,
    index_ty: TypeId,
    /// The recipes the planner prices.
    pub recipes: Vec<Recipe>,
}

fn loop_defined(
    function: &Function,
    blocks: &[BlockId],
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    let ValueDef::Instruction(def) = function.value(value).def else { return None };
    function.parent(def).is_some_and(|block| blocks.contains(&block)).then_some(def)
}

/// The in-loop instructions `operand` is computed by, operands first, where it
/// is address arithmetic (`gep`, integer `add`/`sub`/`mul`/`shl`/`or`, `sext`,
/// `zext`, `trunc`) of the induction and loop-invariant values.
fn address_chain(
    function: &Function,
    blocks: &[BlockId],
    operand: Operand,
    seen: &mut Vec<InstId>,
    depth: u32,
) -> bool {
    let Some(def) = loop_defined(function, blocks, operand) else { return true };
    if seen.contains(&def) {
        return true;
    }
    let inst = function.instruction(def);
    let arithmetic = matches!(
        inst.opcode,
        Opcode::GetElementPtr { .. }
            | Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl | BinaryOp::Or)
            | Opcode::Cast(CastOp::SExt | CastOp::ZExt | CastOp::Trunc)
            | Opcode::Phi
    );
    if !arithmetic || depth > 8 {
        return false;
    }
    if !matches!(inst.opcode, Opcode::Phi)
        && !inst.operands.iter().all(|&one| address_chain(function, blocks, one, seen, depth + 1))
    {
        return false;
    }
    seen.push(def);
    true
}

/// The plan of a loop `legality` found to be an early-exit one, or why not.
pub(crate) fn extract(
    cx: &Cx,
    se: &mut ScalarEvolution,
    the_loop: Loop,
    legality: &Legality,
    early: &EarlyExit,
) -> Result<EarlyExitPlan, Skipped> {
    let function = cx.function;
    let header = the_loop.get_header();
    let preheader = the_loop.get_loop_preheader(function).ok_or(Skipped("The loop has no preheader"))?;
    let latch = the_loop.get_loop_latch(function).ok_or(Skipped("The loop has no latch"))?;
    // VPEE1.
    let [exiting] = early.uncountable[..] else { return Err(Skipped("More than one uncountable exit")) };
    if early.countable[..] != [latch] {
        return Err(Skipped("A countable exit that is not the latch's"));
    }
    let (primary, step) = primary_and_step(se, legality)?;
    let primary_value = function.instruction(primary).result.expect("a phi");
    let primary_phi = Operand::Value(primary_value);
    let index_ty = function.instruction(primary).ty;
    if !matches!(
        legality.inductions.iter().find(|(phi, _)| *phi == primary),
        Some((_, one)) if one.kind == InductionKind::Int
    ) {
        return Err(Skipped("No integer induction"));
    }
    let stepped = function.instruction(step).result.expect("a step");

    // The loop is a chain: each block runs, and leaves, or goes on.
    let mut blocks = vec![header];
    let mut at = header;
    while at != latch {
        let next: Vec<BlockId> =
            function.successors(at).iter().copied().filter(|&one| the_loop.contains(one)).collect();
        match next[..] {
            [only] if !blocks.contains(&only) => {
                blocks.push(only);
                at = only;
            }
            _ => return Err(Skipped("A loop that is not a chain of blocks")),
        }
    }
    if blocks.len() != the_loop.get_num_blocks() {
        return Err(Skipped("A loop that is not a chain of blocks"));
    }

    // The exits' branches.
    let leaving = |block: BlockId| -> Result<(Operand, bool, BlockId), Skipped> {
        let term = function.terminator(block).ok_or(Skipped("An unterminated block"))?;
        let [condition, Operand::Block(taken), Operand::Block(other)] = function.instruction(term).operands[..] else {
            return Err(Skipped("An exit that is not a conditional branch"));
        };
        match (the_loop.contains(taken), the_loop.contains(other)) {
            (false, true) => Ok((condition, true, taken)),
            (true, false) => Ok((condition, false, other)),
            _ => Err(Skipped("An exit that is not a conditional branch")),
        }
    };
    let (early_condition, exit_when_true, early_exit) = leaving(exiting)?;
    let (latch_condition, latch_exits_on_true, countable_exit) = leaving(latch)?;
    let latch_compare =
        loop_defined(function, &blocks, latch_condition).ok_or(Skipped("A latch exit test not computed"))?;
    // `stepped == bound` leaves on the iteration that makes it so.
    let Opcode::ICmp(predicate) = function.instruction(latch_compare).opcode else {
        return Err(Skipped("A latch exit test that is not an integer compare"));
    };
    let operands = function.instruction(latch_compare).operands.clone();
    let bound = match (operands[0] == Operand::Value(stepped), operands[1] == Operand::Value(stepped)) {
        (true, false) => operands[1],
        (false, true) => operands[0],
        _ => return Err(Skipped("A latch exit test not of the stepped induction")),
    };
    if loop_defined(function, &blocks, bound).is_some() {
        return Err(Skipped("A bound the loop computes"));
    }
    if !matches!(
        (predicate, latch_exits_on_true),
        (IntPredicate::Eq, true) | (IntPredicate::Ne, false)
    ) {
        return Err(Skipped("A latch exit test that is not an equality of the stepped induction"));
    }
    let step_data = function.instruction(step);
    if !matches!(step_data.opcode, Opcode::Binary(BinaryOp::Add))
        || !blocks.contains(&function.parent(step).expect("placed"))
    {
        return Err(Skipped("The induction step is not an add of the loop"));
    }
    let start = arms(function, primary)
        .into_iter()
        .find(|&(_, from)| from == preheader)
        .map(|(value, _)| value)
        .ok_or(Skipped("The induction has no start"))?;

    let strides = SymbolicStrideMap::default();
    let in_loop = |block: Option<BlockId>| block.is_some_and(|one| blocks.contains(&one));
    let mut steps: Vec<Step> = Vec::new();
    let mut recipes: Vec<Recipe> = Vec::new();
    let mut widened: Vec<InstId> = Vec::new();
    let mut condition = None;
    let source_of = |operand: Operand, widened: &[InstId]| -> Result<Source, Skipped> {
        if let Some(def) = loop_defined(function, &blocks, operand) {
            return if widened.contains(&def) {
                Ok(Source::Wide(def))
            } else {
                Err(Skipped("A value the loop computes that is not widened"))
            };
        }
        if operand == primary_phi {
            return Err(Skipped("The induction read as a value"));
        }
        if let Operand::Constant(c) = operand
            && !matches!(
                cx.context.get(c).kind,
                ConstantKind::Int(_) | ConstantKind::Zero
            )
        {
            return Err(Skipped("An operand constant that is not an integer"));
        }
        Ok(Source::Splat(operand))
    };
    // Only widened operations read a widened value (the exit condition's `br`
    // aside, which the caller names).
    let widened_users = |value, allow: Option<InstId>| {
        function
            .users(value)
            .iter()
            .all(
                |one| Some(one.user) == allow
                    || (in_loop(function.parent(one.user))
                        && matches!(
                            function.instruction(one.user).opcode,
                            Opcode::Binary(_) | Opcode::ICmp(_)
                        )),
            )
    };
    for &block in &blocks {
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            let data = function.instruction(inst);
            match data.opcode {
                Opcode::Phi if inst == primary => {}
                Opcode::Phi => return Err(Skipped("A phi other than the induction")),
                Opcode::Br => {}
                // `return (int)i`: only an exit phi reads the cast induction.
                Opcode::Cast(_) if induction_cast(function, inst, primary_phi, early_exit).is_some() => {}
                _ if inst == step || inst == latch_compare => {}
                // An address: read by loads and by other addresses only.
                Opcode::GetElementPtr { .. } => {
                    let value = data.result.expect("a gep");
                    let only_addresses = function
                        .users(value)
                        .iter()
                        .all(
                            |one| in_loop(function.parent(one.user))
                                && match function.instruction(one.user).opcode {
                                    Opcode::Load { .. } => one.index == 0,
                                    Opcode::GetElementPtr { .. } => one.index == 0,
                                    _ => false,
                                },
                        );
                    if !only_addresses {
                        return Err(Skipped("An address used other than by a load"));
                    }
                }
                Opcode::Load { align, volatile } => {
                    if volatile {
                        return Err(Skipped("A volatile access"));
                    }
                    let lane = element_ty(cx, data.ty).ok_or(Skipped("An element type the target has no vector of"))?;
                    let ptr = data.operands[0];
                    if get_ptr_stride(se, cx, data.ty, ptr, header, &strides, true) != Some(1) {
                        return Err(Skipped("A load that is not consecutive"));
                    }
                    if !address_chain(function, &blocks, ptr, &mut Vec::new(), 0) {
                        return Err(Skipped("An address that is not arithmetic of the induction"));
                    }
                    let value = data.result.expect("a load");
                    if !widened_users(value, None) {
                        return Err(Skipped("A loaded value used other than by an operation of the loop"));
                    }
                    let size = cx.layout.size_bits(&cx.context.types, data.ty) / 8;
                    let align = align.unwrap_or(size);
                    recipes.push(Recipe::Load { lane, align });
                    widened.push(inst);
                    steps.push(Step::Load { inst, element: data.ty, align });
                }
                Opcode::Binary(op) => {
                    let value = data.result.expect("a binary operation");
                    // An index offset: only addresses read it.
                    if function.users(value).iter().all(|one| {
                        matches!(
                            function.instruction(one.user).opcode,
                            Opcode::GetElementPtr { .. }
                        ) && one.index > 0
                    }) {
                        continue;
                    }
                    let arith = match op {
                        BinaryOp::Add => ArithOp::Add,
                        BinaryOp::Sub => ArithOp::Sub,
                        BinaryOp::Mul => ArithOp::Mul,
                        BinaryOp::And => ArithOp::And,
                        BinaryOp::Or => ArithOp::Or,
                        BinaryOp::Xor => ArithOp::Xor,
                        BinaryOp::Shl => ArithOp::Shl,
                        BinaryOp::LShr => ArithOp::LShr,
                        BinaryOp::AShr => ArithOp::AShr,
                        _ => return Err(Skipped("A binary operation the vectorizer does not widen yet")),
                    };
                    let lane = element_ty(cx, data.ty).ok_or(Skipped("An element type the target has no vector of"))?;
                    if !widened_users(value, None) {
                        return Err(Skipped("A binary result used other than by an operation of the loop"));
                    }
                    let operands = [source_of(data.operands[0], &widened)?, source_of(data.operands[1], &widened)?];
                    if matches!(op, BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)
                        && !matches!(operands[1], Source::Splat(Operand::Constant(_)))
                    {
                        return Err(Skipped("A shift by an amount that is not a constant"));
                    }
                    if operands.iter().all(|one| matches!(one, Source::Splat(_))) {
                        return Err(Skipped("A binary operation of loop-invariant values"));
                    }
                    recipes.push(Recipe::Binary { op: arith, lane, ext: [None, None], second: Default::default() });
                    widened.push(inst);
                    steps.push(Step::Binary { inst, op, flags: data.flags, element: data.ty, operands });
                }
                Opcode::ICmp(predicate) => {
                    // The one compare that leaves the loop.
                    let value = data.result.expect("a compare");
                    let exits = Operand::Value(value) == early_condition;
                    let br = function.terminator(exiting).expect("a terminated block");
                    if !exits
                        || condition.is_some()
                        || !widened_users(value, Some(br))
                        || function.users(value).iter().count() != 1
                    {
                        return Err(Skipped("A comparison that does not alone decide the exit"));
                    }
                    let source =
                        function.operand_type(cx.context, data.operands[0]).ok_or(Skipped("An untyped comparison"))?;
                    let lane = element_ty(cx, source).ok_or(Skipped("An element type the target has no vector of"))?;
                    let operands = [source_of(data.operands[0], &widened)?, source_of(data.operands[1], &widened)?];
                    if operands.iter().all(|one| matches!(one, Source::Splat(_))) {
                        return Err(Skipped("A comparison of loop-invariant values"));
                    }
                    recipes.push(Recipe::Compare { op: CmpSel::ICmp, lane, predicate: Predicate::Int(predicate) });
                    widened.push(inst);
                    condition = Some(inst);
                    steps.push(Step::ICmp { inst, predicate, operand_element: source, operands });
                }
                _ => return Err(Skipped("An instruction the vectorizer does not widen yet")),
            }
        }
    }
    let condition = condition.ok_or(Skipped("The exit condition is not a compare of the loop"))?;
    if !steps.iter().any(|one| matches!(one, Step::Load { .. })) {
        return Err(Skipped("No memory access to widen"));
    }

    // What leaves the loop: the exit phis' values. The latch's edge takes
    // loop-invariant values; the early edge also the induction and values the
    // loop widened (taken at the first set lane).
    for block in blocks.iter().copied() {
        for inst in function.iter_block(block) {
            let Some(result) = function.instruction(inst).result else { continue };
            for one in function.users(result).iter() {
                if in_loop(function.parent(one.user)) {
                    continue;
                }
                let parent = function.parent(one.user);
                let is_exit_phi = function.instruction(one.user).opcode == Opcode::Phi
                    && (parent == Some(early_exit) || parent == Some(countable_exit));
                if !is_exit_phi {
                    return Err(Skipped("A value of the loop used after it other than by an exit phi"));
                }
            }
        }
    }
    for phi in edges::phis(function, countable_exit) {
        for (value, from) in arms(function, phi) {
            if from == latch && loop_defined(function, &blocks, value).is_some() {
                return Err(Skipped("An exit phi of a value the loop defines"));
            }
        }
    }
    for phi in edges::phis(function, early_exit) {
        for (value, from) in arms(function, phi) {
            if from != exiting {
                continue;
            }
            if let Some(def) = loop_defined(function, &blocks, value)
                && def != primary
                && induction_cast(function, def, primary_phi, early_exit).is_none()
                && !(widened.contains(&def) && def != condition)
            {
                return Err(Skipped("An exit phi of a value the loop does not widen"));
            }
        }
    }
    Ok(EarlyExitPlan {
        steps,
        blocks,
        condition,
        exit_when_true,
        preheader,
        latch,
        exiting,
        early_exit,
        countable_exit,
        primary,
        start,
        bound,
        index_ty,
        recipes,
    })
}

/// The factors of the planned loop: the pin's VF, and IC 1.
pub(crate) fn choose(
    tti: &dyn Tti,
    early: &EarlyExitPlan,
    mut facts: LoopFacts,
) -> Result<Plan, crate::loopvectorizeplan::NotPlanned> {
    facts.recipes = early.recipes.clone();
    let chosen = plan(tti, &facts)?;
    if facts.trip_count != 0 && facts.trip_count < u64::from(chosen.vf) {
        return Err(crate::loopvectorizeplan::NotPlanned("The loop runs fewer iterations than one vector iteration"));
    }
    Ok(Plan { vf: chosen.vf, ic: 1 })
}

/// A value of `ty` that is `a op b`: folded where both are constants.
fn arithmetic(
    context: &mut Context,
    function: &mut Function,
    at: Position,
    op: BinaryOp,
    ty: TypeId,
    operands: [Operand; 2],
    name: &str,
) -> Operand {
    let bits = bits_of(context, ty);
    if let (Some(a), Some(b)) = (constant_int(context, operands[0]), constant_int(context, operands[1])) {
        let value = match op {
            BinaryOp::Add => a.wrapping_add(b),
            BinaryOp::Sub => a.wrapping_sub(b),
            BinaryOp::And => a & b,
            _ => unreachable!("an operation the skeleton folds"),
        } & mask(bits);
        return Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Int(value) }));
    }
    emit(function, at, Opcode::Binary(op), ty, operands.to_vec(), name)
}

/// `a pred b` of integers, folded where both are constants.
fn compare(
    context: &mut Context,
    function: &mut Function,
    at: Position,
    predicate: IntPredicate,
    operands: [Operand; 2],
    name: &str,
) -> Operand {
    let bool_ty = context.types.intern(Type::Int(1));
    if let (Some(a), Some(b)) = (constant_int(context, operands[0]), constant_int(context, operands[1])) {
        let value = match predicate {
            IntPredicate::Eq => a == b,
            IntPredicate::Ult => a < b,
            _ => unreachable!("a predicate the skeleton folds"),
        };
        return Operand::Constant(context.int(bool_ty, i128::from(value)));
    }
    emit(function, at, Opcode::ICmp(predicate), bool_ty, operands.to_vec(), name)
}

/// A call of the intrinsic `name`, overloaded as `function_type` says; the
/// callee is the last operand.
fn call(
    context: &mut Context,
    function: &mut Function,
    declared: &mut Declared,
    block: BlockId,
    name: &str,
    returns: TypeId,
    parameters: Vec<TypeId>,
    mut arguments: Vec<Operand>,
    label: &str,
) -> Operand {
    let function_type =
        context.types.intern(Type::Function { returns, parameters: parameters.clone(), variadic: false });
    let callee = declared.declare(name, function_type);
    let pointer = context.types.ptr(0);
    arguments.push(Operand::Constant(context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) })));
    let made = function.create_instruction(
        Opcode::Call(Box::new(CallInfo {
            function_type,
            calling_convention: 0,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); parameters.len()],
            attrs: Vec::new(),
            tail: Tail::default(),
        })),
        returns,
        arguments,
        Flags::default(),
        Some(label),
    );
    function.insert(made, Position::End(block)).expect("a placed block");
    Operand::Value(function.instruction(made).result.expect("a call"))
}

/// The address `ptr` at the iteration `index`: the in-loop arithmetic of it
/// copied into `block`, with the induction read as `index`.
fn address_at(
    function: &mut Function,
    plan: &EarlyExitPlan,
    block: BlockId,
    ptr: Operand,
    moved: &mut Vec<(Operand, Operand)>,
) -> Operand {
    if let Some(&(_, to)) = moved.iter().find(|(from, _)| *from == ptr) {
        return to;
    }
    let Some(def) = loop_defined(function, &plan.blocks, ptr) else { return ptr };
    let original = function.instruction(def);
    let (opcode, ty, flags, operands) =
        (original.opcode.clone(), original.ty, original.flags, original.operands.clone());
    let operands: Vec<Operand> =
        operands.into_iter().map(|one| address_at(function, plan, block, one, moved)).collect();
    let copy = function.create_instruction(opcode, ty, operands, flags, Some("lane.address"));
    function.insert(copy, Position::End(block)).expect("a placed block");
    let made = Operand::Value(function.instruction(copy).result.expect("address arithmetic has a result"));
    moved.push((ptr, made));
    made
}

/// Makes the vector loop and the blocks around it for `chosen.vf` lanes; the
/// original loop becomes the remainder.
pub(crate) fn execute(
    context: &mut Context,
    function: &mut Function,
    declared: &mut Declared,
    metadata: &[MetadataNode],
    plan: &EarlyExitPlan,
    vf: u32,
) {
    let index_ty = plan.index_ty;
    let header = plan.blocks[0];
    let bool_ty = context.types.intern(Type::Int(1));
    let preheader_term = function.terminator(plan.preheader).expect("a terminated preheader");
    let void = function.instruction(preheader_term).ty;
    let constant = |context: &mut Context, value: u128| {
        Operand::Constant(context.constant(Constant { ty: index_ty, kind: ConstantKind::Int(value) }))
    };

    // The blocks, in front of the loop that stays as the remainder.
    let vector_ph = function.create_block(Some("vector.ph"));
    let body = function.create_block(Some("vector.body"));
    let interim = function.create_block(Some("vector.body.interim"));
    let early = function.create_block(Some("vector.early.exit"));
    let middle = function.create_block(Some("middle.block"));
    let scalar_ph = function.create_block(Some("scalar.ph"));
    for block in [vector_ph, body, interim, early, middle, scalar_ph] {
        function.insert_block_before(block, header).expect("a new block");
    }

    // preheader: the trip count and the check that it fills a vector.
    let before = Position::Before(preheader_term);
    let trip = if constant_int(context, plan.start) == Some(0) {
        plan.bound
    } else {
        arithmetic(context, function, before, BinaryOp::Sub, index_ty, [plan.bound, plan.start], "n.trip")
    };
    let lanes = constant(context, u128::from(vf));
    let min_iters = compare(context, function, before, IntPredicate::Ult, [trip, lanes], "min.iters.check");
    let guard = function.create_instruction(
        Opcode::Br,
        void,
        vec![min_iters, Operand::Block(scalar_ph), Operand::Block(vector_ph)],
        Flags::default(),
        None,
    );
    function.insert(guard, before).expect("a placed terminator");
    function.set_operands(preheader_term, Vec::new());
    function.erase(preheader_term).expect("the old jump");

    // vector.ph: the iterations the vector loop runs.
    let at = Position::End(vector_ph);
    let lane_mask = constant(context, u128::from(vf - 1));
    let remainder_lanes = arithmetic(context, function, at, BinaryOp::And, index_ty, [trip, lane_mask], "n.mod.vf");
    let vector_trip = arithmetic(context, function, at, BinaryOp::Sub, index_ty, [trip, remainder_lanes], "n.vec");
    let end = if constant_int(context, plan.start) == Some(0) {
        vector_trip
    } else {
        arithmetic(context, function, at, BinaryOp::Add, index_ty, [plan.start, vector_trip], "ind.end")
    };
    end_with(function, vector_ph, void, vec![Operand::Block(body)]);

    // vector.body.
    let index_phi = function.create_instruction(Opcode::Phi, index_ty, Vec::new(), Flags::default(), Some("index"));
    function.insert(index_phi, Position::End(body)).expect("a placed block");
    let index = Operand::Value(function.instruction(index_phi).result.expect("a phi"));
    let primary_value = Operand::Value(function.instruction(plan.primary).result.expect("a phi"));
    let vector_of = |context: &Context, element: TypeId| context.types.intern(Type::Vector { element, count: vf });
    let mask_ty = vector_of(context, bool_ty);
    let mut wide: Vec<(InstId, Operand)> = Vec::new();
    let mut splats: Vec<(Operand, Operand)> = Vec::new();
    let mut moved = vec![(primary_value, index)];
    for step in &plan.steps {
        match step {
            &Step::Load { inst, element, align } => {
                let ptr = function.instruction(inst).operands[0];
                let address = address_at(function, plan, body, ptr, &mut moved);
                let ty = vector_of(context, element);
                let load = function.create_instruction(
                    Opcode::Load { align: Some(align), volatile: false },
                    ty,
                    vec![address],
                    Flags::default(),
                    Some("wide.load"),
                );
                function.insert(load, Position::End(body)).expect("a placed block");
                wide.push((inst, Operand::Value(function.instruction(load).result.expect("a load"))));
            }
            Step::Binary { inst, op, flags, element, operands } => {
                let ty = vector_of(context, *element);
                let operands =
                    operands.map(|one| source(context, function, vector_ph, ty, vf, one, &wide, &mut splats));
                let made = function.create_instruction(Opcode::Binary(*op), ty, operands.to_vec(), *flags, None);
                function.insert(made, Position::End(body)).expect("a placed block");
                wide.push((*inst, Operand::Value(function.instruction(made).result.expect("an operation"))));
            }
            Step::ICmp { inst, predicate, operand_element, operands } => {
                let operand_ty = vector_of(context, *operand_element);
                let operands =
                    operands.map(|one| source(context, function, vector_ph, operand_ty, vf, one, &wide, &mut splats));
                let made = function.create_instruction(
                    Opcode::ICmp(*predicate),
                    mask_ty,
                    operands.to_vec(),
                    Flags::default(),
                    None,
                );
                function.insert(made, Position::End(body)).expect("a placed block");
                wide.push((*inst, Operand::Value(function.instruction(made).result.expect("a compare"))));
            }
        }
    }
    let lookup = |wide: &[(InstId, Operand)], def: InstId| wide.iter().find(|(old, _)| *old == def).expect("widened").1;
    // The lanes that leave, as set bits.
    let mut exiting_lanes = lookup(&wide, plan.condition);
    if !plan.exit_when_true {
        let one = Operand::Constant(context.int(bool_ty, 1));
        let ones = broadcast(context, function, vector_ph, mask_ty, vf, one);
        exiting_lanes = emit(
            function,
            Position::End(body),
            Opcode::Binary(BinaryOp::Xor),
            mask_ty,
            vec![exiting_lanes, ones],
            "not.mask",
        );
    }
    let index_next = emit(
        function,
        Position::End(body),
        Opcode::Binary(BinaryOp::Add),
        index_ty,
        vec![index, constant(context, u128::from(vf))],
        "index.next",
    );
    let reduce_name = format!("llvm.vector.reduce.or.{}", llrm_mir::intrinsics::mangle(&context.types, mask_ty));
    let any =
        call(context, function, declared, body, &reduce_name, bool_ty, vec![mask_ty], vec![exiting_lanes], "any.of");
    let counted = emit(
        function,
        Position::End(body),
        Opcode::ICmp(IntPredicate::Eq),
        bool_ty,
        vec![index_next, end],
        "cmp.count",
    );
    end_with(function, body, void, vec![any, Operand::Block(early), Operand::Block(interim)]);
    end_with(function, interim, void, vec![counted, Operand::Block(middle), Operand::Block(body)]);
    function.set_operands(index_phi, from_arms(&[(plan.start, vector_ph), (index_next, interim)]));

    // vector.early.exit: the exit phis' values at the first set lane.
    let mut first_lane = None;
    for phi in edges::phis(function, plan.early_exit) {
        let mut incoming = arms(function, phi);
        let Some(&(value, _)) = incoming.iter().find(|&&(_, from)| from == plan.exiting) else { continue };
        let taken = match loop_defined(function, &plan.blocks, value) {
            None => value,
            Some(def) => {
                let lane = *first_lane.get_or_insert_with(|| {
                    // `FirstActiveLane` (VPEE3): the smallest lane number among
                    // the set lanes.
                    debug_assert!(vf < 255, "a lane number fits a byte");
                    let byte = context.types.intern(Type::Int(8));
                    let lanes_ty = vector_of(context, byte);
                    let numbered: Vec<_> = (0..u128::from(vf))
                        .map(|lane| context.constant(Constant { ty: byte, kind: ConstantKind::Int(lane) }))
                        .collect();
                    let none = context.constant(Constant { ty: byte, kind: ConstantKind::Int(255) });
                    let numbers = Operand::Constant(
                        context.constant(Constant { ty: lanes_ty, kind: ConstantKind::Aggregate(numbered) }),
                    );
                    let nones =
                        Operand::Constant(context.constant(Constant {
                            ty: lanes_ty,
                            kind: ConstantKind::Aggregate(vec![none; vf as usize]),
                        }));
                    let chosen = emit(
                        function,
                        Position::End(early),
                        Opcode::Select,
                        lanes_ty,
                        vec![exiting_lanes, numbers, nones],
                        "active.lanes",
                    );
                    let umin_name =
                        format!("llvm.vector.reduce.umin.{}", llrm_mir::intrinsics::mangle(&context.types, lanes_ty));
                    let first = call(
                        context,
                        function,
                        declared,
                        early,
                        &umin_name,
                        byte,
                        vec![lanes_ty],
                        vec![chosen],
                        "first.active.lane",
                    );
                    emit(
                        function,
                        Position::End(early),
                        Opcode::Cast(CastOp::ZExt),
                        index_ty,
                        vec![first],
                        "first.lane",
                    )
                });
                if def == plan.primary || induction_cast(function, def, primary_value, plan.early_exit).is_some() {
                    let at = emit(
                        function,
                        Position::End(early),
                        Opcode::Binary(BinaryOp::Add),
                        index_ty,
                        vec![index, lane],
                        "early.exit.value",
                    );
                    match induction_cast(function, def, primary_value, plan.early_exit) {
                        Some(cast) => {
                            let ty = function.instruction(def).ty;
                            emit(function, Position::End(early), Opcode::Cast(cast), ty, vec![at], "early.exit.value")
                        }
                        None => at,
                    }
                } else {
                    let element = function.instruction(def).ty;
                    emit(
                        function,
                        Position::End(early),
                        Opcode::ExtractElement,
                        element,
                        vec![lookup(&wide, def), lane],
                        "early.exit.value",
                    )
                }
            }
        };
        incoming.push((taken, early));
        function.set_operands(phi, from_arms(&incoming));
    }
    end_with(function, early, void, vec![Operand::Block(plan.early_exit)]);

    // middle.block: done if the vector loop ran every iteration.
    let all = compare(context, function, Position::End(middle), IntPredicate::Eq, [trip, vector_trip], "cmp.n");
    end_with(function, middle, void, vec![all, Operand::Block(plan.countable_exit), Operand::Block(scalar_ph)]);
    for phi in edges::phis(function, plan.countable_exit) {
        let mut incoming = arms(function, phi);
        let Some(&(value, _)) = incoming.iter().find(|&&(_, from)| from == plan.latch) else { continue };
        incoming.push((value, middle));
        function.set_operands(phi, from_arms(&incoming));
    }

    // scalar.ph: where the original loop resumes.
    let resume =
        function.create_instruction(Opcode::Phi, index_ty, Vec::new(), Flags::default(), Some("bc.resume.val"));
    function.insert(resume, Position::End(scalar_ph)).expect("a placed block");
    let resume_value = Operand::Value(function.instruction(resume).result.expect("a phi"));
    function.set_operands(resume, from_arms(&[(end, middle), (plan.start, plan.preheader)]));
    end_with(function, scalar_ph, void, vec![Operand::Block(header)]);
    let rewired: Vec<_> = arms(function, plan.primary)
        .into_iter()
        .map(|(value, from)| if from == plan.preheader { (resume_value, scalar_ph) } else { (value, from) })
        .collect();
    function.set_operands(plan.primary, from_arms(&rewired));

    // Neither loop is widened again.
    let vector_branch = function.terminator(interim).expect("a terminated block");
    mark_vectorized(context, declared, metadata, function, vector_branch);
    let scalar_branch = function.terminator(plan.latch).expect("a terminated latch");
    mark_vectorized(context, declared, metadata, function, scalar_branch);
}

/// The cast of the induction that only an exit phi of `early_exit` reads: the
/// early edge takes it of the first set lane's index.
fn induction_cast(
    function: &Function,
    inst: InstId,
    primary: Operand,
    early_exit: BlockId,
) -> Option<CastOp> {
    let data = function.instruction(inst);
    let Opcode::Cast(cast @ (CastOp::SExt | CastOp::ZExt | CastOp::Trunc)) = data.opcode else { return None };
    let value = data.result?;
    let only_exit_phis = function
        .users(value)
        .iter()
        .all(
            |one| function.instruction(one.user).opcode == Opcode::Phi && function.parent(one.user) == Some(early_exit),
        );
    (data.operands[0] == primary && only_exit_phis).then_some(cast)
}

/// An operand of a widened instruction: the wide value, or the loop-invariant
/// value broadcast once.
fn source(
    context: &mut Context,
    function: &mut Function,
    vector_ph: BlockId,
    vector_ty: TypeId,
    vf: u32,
    source: Source,
    wide: &[(InstId, Operand)],
    splats: &mut Vec<(Operand, Operand)>,
) -> Operand {
    match source {
        Source::Wide(def) => wide.iter().find(|(old, _)| *old == def).expect("the definition comes first").1,
        Source::Splat(scalar) => match splats.iter().find(|(one, _)| *one == scalar) {
            Some((_, made)) => *made,
            None => {
                let made = broadcast(context, function, vector_ph, vector_ty, vf, scalar);
                splats.push((scalar, made));
                made
            }
        },
    }
}
