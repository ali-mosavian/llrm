//! LLVM's `CanonicalizeFreezeInLoops` (`canon-freeze`;
//! CanonicalizeFreezeInLoops.cpp at `d1106deb71cc`): a freeze of an induction
//! variable or of its step is pushed out of the loop, to the preheader.
//!
//! # How it works
//! Each loop of simplify form, innermost first, is searched for the phis of its
//! header that are induction variables stepped by an `add`, `sub` or `mul`
//! whose step value is not defined in the loop. A phi or step with a `freeze`
//! user is a candidate. For each candidate phi once: the step's poison
//! generating flags are dropped unless it is already not poison, the step value
//! and the phi's start value are frozen in the preheader unless they are
//! already not undef or poison, and the freezes in the loop are replaced by
//! their operands. ScalarEvolution forgets what it changed.
//!
//! # Example
//! `i = phi [init, pre], [i.next, loop]; i.next = add nsw i, 1;
//! f = freeze i.next` becomes `init.frozen = freeze init` in the preheader,
//! `i = phi [init.frozen, pre], [i.next, loop]` and `i.next = add i, 1`: the
//! loop is an add recurrence again and the interpreter agrees.
//!
//! # Runs
//! In the strength slot, before `lsr` (the pin runs it with loop-reduce).
//!
//! # Limits
//! No frontend emits `freeze`, so on the corpus it changes nothing (0 of 77
//! IRs differ). No DEVIATE: the pin's `DenseMapInfo` set of candidates is a
//! list keyed by the freeze.

use llrm_analysis::ivdescriptors::is_induction_phi;
use llrm_analysis::scev::ScalarEvolution;
use llrm_analysis::scev::analysis::{ScevSession, Scevs};
use llrm_analysis::scev::expander::Env;
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::passes::{self, Analyses, FunctionPass, Outer, PreservedAnalyses};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_undef_or_poison;

use crate::local;

/// `CanonicalizeFreezeInLoopsPass`.
pub struct CanonicalizeFreezeInLoops;

impl FunctionPass for CanonicalizeFreezeInLoops {
    fn name(&self) -> &'static str {
        "canonfreeze"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("canon-freeze")
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let mut session = ScevSession::of(unit.context, unit.function, &outer);
        let held = analyses.get::<Scevs>(unit.context, unit.layout, unit.function);
        session.adopt(held.take());
        let changed = canonicalized_in(&mut session, unit.context, unit.layout, unit.function, &outer);
        held.put(session.into_scev());
        if changed {
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<passes::Dominators>().preserve::<passes::Loops>().preserve::<Scevs>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// `function`'s loops canonicalized, innermost first; whether any was.
pub fn canonicalized(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let mut session = ScevSession::of(context, function, outer);
    canonicalized_in(&mut session, context, layout, function, outer)
}

/// [`canonicalized`] over `session`, whose expressions are valid for
/// `function`.
pub fn canonicalized_in(
    session: &mut ScevSession,
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let headers: Vec<BlockId> = {
        let (_, env) = session.split(layout, &outer.globals, &outer.metadata);
        let mut headers: Vec<BlockId> = env.loops.get_loops_in_preorder().iter().map(|one| one.get_header()).collect();
        headers.reverse();
        headers
    };
    let mut changed = false;
    for header in headers {
        let (se, env) = session.split(layout, &outer.globals, &outer.metadata);
        changed |= run(se, &env, context, function, header);
    }
    changed
}

/// `FrozenIndPHIInfo`: a freeze that uses an induction phi or its step.
struct Candidate {
    /// `FI`.
    freeze: InstId,
    /// `PHI`.
    phi: InstId,
    /// `StepInst`.
    step: InstId,
    /// `StepValIdx`: the operand of `step` that is the step value.
    step_value_index: usize,
}

/// `Instruction::getOperandUse`'s user and operand number.
type Slot = (InstId, usize);

/// `isGuaranteedNotToBeUndefOrPoison(V, nullptr, CtxI, &DT)`: the assumption
/// cache is not passed.
fn guaranteed(
    env: &Env,
    context: &Context,
    function: &Function,
    value: Operand,
    at: InstId,
) -> bool {
    let query = Query {
        globals: env.globals,
        metadata: env.metadata,
        dt: Some(env.tree),
        ..Query::bare(context, function, env.layout).with_cxt(Some(at))
    };
    is_guaranteed_not_to_be_undef_or_poison(&query, value, 0)
}

/// `ScalarEvolution::forgetValue(I)`.
fn forget(
    se: &mut ScalarEvolution,
    env: &Env,
    context: &Context,
    function: &Function,
    inst: InstId,
) {
    if let Some(result) = function.instruction(inst).result {
        se.forget_value(&env.cx(context, function), Operand::Value(result));
    }
}

/// `CanonicalizeFreezeInLoopsImpl::InsertFreezeAndForgetFromSCEV`: the operand
/// `slot` of its user is replaced by a freeze of it, placed in `preheader`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CanonicalizeFreezeInLoops.cpp#L117>
fn insert_freeze_and_forget_from_scev(
    se: &mut ScalarEvolution,
    env: &Env,
    context: &Context,
    function: &mut Function,
    preheader: BlockId,
    (user, index): Slot,
) {
    let value = function.instruction(user).operands[index];
    if guaranteed(env, context, function, value, user) {
        return;
    }
    let ty = function.operand_type(context, value).expect("a typed operand");
    let name = match value {
        Operand::Value(id) => function.value(id).name.as_ref().map(|name| format!("{name}.frozen")),
        _ => None,
    };
    let freeze = function.create_instruction(Opcode::Freeze, ty, vec![value], Flags::default(), name.as_deref());
    let terminator = function.terminator(preheader).expect("a terminator");
    function.insert(freeze, Position::Before(terminator)).expect("a placed terminator");
    let frozen = Operand::Value(function.instruction(freeze).result.expect("a freeze's value"));
    function.set_operand(user, index, frozen);
    forget(se, env, context, function, user);
}

/// `CanonicalizeFreezeInLoopsImpl::canHandleInst`: a freeze can be pushed into
/// the operands of `inst` if its flags, once dropped, do not make a poison.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CanonicalizeFreezeInLoops.cpp#L72>
fn can_handle_inst(
    function: &Function,
    inst: InstId,
) -> bool {
    matches!(
        function.instruction(inst).opcode,
        Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
    )
}

/// `CanonicalizeFreezeInLoopsImpl::run` for the loop headed by `header`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/CanonicalizeFreezeInLoops.cpp#L137>
fn run(
    se: &mut ScalarEvolution,
    env: &Env,
    context: &Context,
    function: &mut Function,
    header: BlockId,
) -> bool {
    let mut candidates: Vec<Candidate> = Vec::new();
    let preheader;
    {
        let cx = env.cx(context, function);
        let Some(lp) = cx.loops.get_loop_for(header) else { return false };
        // The loop should be in LoopSimplify form.
        if !lp.is_loop_simplify_form(function) {
            return false;
        }
        preheader = lp.get_loop_preheader(function).expect("a preheader");
        let phis: Vec<InstId> =
            function.iter_block(header).take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi).collect();
        for phi in phis {
            let Some(descriptor) = is_induction_phi(&cx, se, phi, lp) else { continue };
            let Some(step) = descriptor.induction_bin_op else { continue };
            if !can_handle_inst(function, step) {
                // The stepping instruction has unknown form.
                continue;
            }
            let phi_result = function.instruction(phi).result.expect("a phi's value");
            let step_value_index = usize::from(function.instruction(step).operands[0] == Operand::Value(phi_result));
            if let Operand::Value(id) = function.instruction(step).operands[step_value_index]
                && let ValueDef::Instruction(def) = function.value(id).def
                && lp.contains_instruction(function, def)
            {
                // The step value is inside the loop. Freezing it will
                // introduce another freeze into the loop, so skip this phi.
                continue;
            }
            let step_result = function.instruction(step).result.expect("a step's value");
            for value in [phi_result, step_result] {
                for used in function.users(value).iter() {
                    if function.instruction(used.user).opcode == Opcode::Freeze
                        && !candidates.iter().any(|one| one.freeze == used.user)
                    {
                        candidates.push(Candidate { freeze: used.user, phi, step, step_value_index });
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        return false;
    }

    let mut processed: Vec<InstId> = Vec::new();
    for candidate in &candidates {
        let phi = candidate.phi;
        if processed.contains(&phi) {
            continue;
        }
        processed.push(phi);

        let step = candidate.step;
        let step_result = Operand::Value(function.instruction(step).result.expect("a step's value"));
        // Drop flags from the step instruction.
        if !guaranteed(env, context, function, step_result, step) {
            local::drop_poison_generating_flags(context, function, step);
            forget(se, env, context, function, step);
        }

        insert_freeze_and_forget_from_scev(se, env, context, function, preheader, (step, candidate.step_value_index));

        // The phi's other incoming value than the step: operand `2 * i` of the
        // phi's `[value, block]` pairs.
        let operand_index = if function.instruction(phi).operands[0] == step_result { 2 } else { 0 };
        insert_freeze_and_forget_from_scev(se, env, context, function, preheader, (phi, operand_index));
    }

    // Finally, remove the old freeze instructions.
    for candidate in &candidates {
        let freeze = candidate.freeze;
        forget(se, env, context, function, freeze);
        let result = function.instruction(freeze).result.expect("a freeze's value");
        let operand = function.instruction(freeze).operands[0];
        function.replace_all_uses_with(result, operand);
        function.erase(freeze).expect("a freeze nothing uses");
    }
    true
}

#[cfg(test)]
#[path = "canonfreeze_tests.rs"]
mod tests;
