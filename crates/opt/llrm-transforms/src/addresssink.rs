//! Comparison and address sinking, so isel sees one form: LLVM's CodeGenPrepare
//! `sinkCmpExpression` and address sinking, and GVNSink's merge of equal arms.
//!
//! # How it works
//! Four rewrites, each on one function, none of which touches an edge.
//!
//! A comparison (`icmp` or `fcmp`) read by a branch, select or other user in
//! another block is made again at the top of that block (one copy per block);
//! the original goes once nothing reads it. A phi's incoming block is not a
//! read. isel can then keep the result in the flags instead of an `i1` held
//! across the loop body (`sete`, a store of the byte, `cmp`, `jne`). This runs
//! only where the target has a single condition register
//! (`!target().multiple_condition_registers()`).
//!
//! An `and` that only compares of it with zero read (`x & mask == 0`), some in
//! other blocks than its own, is made again before each of those compares,
//! where the target's lowering says the pair folds
//! (`Lowering::is_mask_and_cmp0_folding_beneficial`) and the copy would not
//! stretch two values read nowhere else (`sinkAndCmp0Expression`). Numbering
//! made one value of the repeated tests of a bit; the compares sunk into the
//! blocks that branch on it left the `and` held across all of them.
//!
//! A phi read only as an address (`spill::address_only`: loads, stores through
//! it, and GEPs of up to three levels that are themselves only addresses) whose
//! arms each compute the same pure operation (`gep`, `zext`, `sext`, `trunc`,
//! `add`, `sub`, `mul`, `shl`, `and`, `or`, `xor`, of equal type and flags, to
//! depth 3) over operands that reach the join (they dominate it) is that
//! computation made once, before the join's first non-phi. What only the phi
//! read, the arms' copies, is erased.
//!
//! A phi of this kind whose arms are all one value (or the phi itself) is that
//! value; this is how a closing phi left by `lcssa` goes. Each phi is visited
//! once, and the phis that read it are queued again.
//!
//! A phi read as a value, or whose arms differ in kind, stays.
//!
//! # Example
//! Before:
//!
//! ```mir
//! b1:
//!   %k1 = sub i16 %i, -3
//!   %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
//!   store i16 0, ptr %p1
//!   br label %b5
//!
//! ; ...
//!
//! b5:
//!   %p = phi ptr [ %p1, %b1 ], [ %p3, %b3 ], [ %p4, %b4 ]
//!   store i16 %c, ptr %p
//!   ret void
//! ```
//!
//! After:
//!
//! ```mir
//! b1:
//!   %k1 = sub i16 %i, -3
//!   %p1 = getelementptr inbounds i16, ptr @g, i16 %k1
//!   store i16 0, ptr %p1
//!   br label %b5
//!
//! ; ...
//!
//! b5:
//!   %0 = sub i16 %i, -3
//!   %1 = getelementptr inbounds i16, ptr @g, i16 %0
//!   store i16 %c, ptr %1
//!   ret void
//! ```
//!
//! The phi of the three arms' addresses is the address computed once in the
//! join (`b3` and `b4` are as `b1`, with their own stores); the arms' copies
//! stay where their own stores read them.
//!
//! From unit test
//! `test_a_phi_of_one_address_in_every_arm_is_that_address_in_the_join` in
//! `addresssink_tests.rs`.
//!
//! # Runs
//! Once, last of the function passes before `spares` and `homes`, after
//! `gepoffset`: until then gvn's partial redundancy elimination makes exactly
//! this phi so a load after the join reads what the arms stored, and an earlier
//! sink would be undone. Off with `-fno-strength-reduce` and at -O0.
//!
//! # Keeps
//! Blocks and edges are unchanged, so dominators and loops are preserved. A
//! change that only replaced trivial phis also keeps the assumption cache and
//! the exposed frame objects; value facts are recomputed.
//!
//! # See also
//! LLVM `CodeGenPrepare::sinkCmpExpression`, `sinkAndCmp0Expression` and
//! `optimizeMemoryInst`; issue
//! #386 (RGBLIGHTS `a(i) = ...` under an `IF`).

use llrm_analysis::cfg;
use llrm_analysis::manager::{AssumptionCache, ExposedFrames};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses};
use llrm_mir::types::TypeId;

pub struct AddressSink;

impl FunctionPass for AddressSink {
    fn name(&self) -> &'static str {
        "addresssink"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        // A closing phi (`lcssa`) whose arms are all one value is that value:
        // nothing after this needs loop-closed form, and isel cannot
        // fold a phi into an addressing mode.
        let closed = trivial_phis(unit.function);
        // Blocks and edges are as they were.
        let one_flag = !analyses.outer().target().multiple_condition_registers();
        let sunk_compares = one_flag && compares_sunk(unit.function);
        let masks =
            one_flag && masks_sunk(unit.context, unit.function, analyses.outer().target().lowering().as_deref());
        if sunk(unit.function) || sunk_compares || masks {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else if closed {
            // A phi's value is replaced by the one it equalled: the blocks and
            // loops stand, and what is said of an assumption or of
            // the frame objects reads no phi. What is said of values (their
            // facts, a loop's bound, the annotations, the edges a
            // condition dominates) may name the phi, and is worked out again.
            PreservedAnalyses::none()
                .preserve::<ExposedFrames>()
                .preserve::<Dominators>()
                .preserve::<Loops>()
                .preserve::<AssumptionCache>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Each phi read only as an address whose arms are one value, or itself,
/// replaced by that value, and the phis that read it looked at again; whether
/// any was. One visit of each phi, not a pass over the function until nothing
/// changes. A phi of anything else costs the allocator nothing it does not
/// coalesce, and a changed shape of it costs a search round (nest 32 at -O2:
/// one more of every machine phase).
fn trivial_phis(function: &mut Function) -> bool {
    let mut pending: Vec<InstId> = function
        .walk()
        .filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Phi)
        .map(|(_, inst)| inst)
        .collect();
    let mut changed = false;
    while let Some(phi) = pending.pop() {
        if function.is_erased(phi) {
            continue;
        }
        let Some(result) = function.instruction(phi).result else { continue };
        if !crate::spill::address_only(function, result, 3) {
            continue;
        }
        let mut only: Option<Operand> = None;
        let mut single = true;
        for pair in function.instruction(phi).operands.chunks(2) {
            if pair[0] == Operand::Value(result) {
                continue;
            }
            match only {
                None => only = Some(pair[0]),
                Some(seen) if seen == pair[0] => {}
                Some(_) => {
                    single = false;
                    break;
                }
            }
        }
        let (true, Some(value)) = (single, only) else { continue };
        pending.extend(
            function
                .users(result)
                .iter()
                .map(|one| one.user)
                .filter(|&user| user != phi && function.instruction(user).opcode == Opcode::Phi),
        );
        function.replace_all_uses_with(result, value);
        function.erase(phi).expect("its uses were replaced");
        changed = true;
    }
    changed
}

/// What the join makes of its phi's arms: a value every arm reads, or an
/// instruction of one pure kind over merged operands.
enum Merged {
    Shared(Operand),
    Made(Opcode, TypeId, Flags, Vec<Merged>),
}

fn sunk(function: &mut Function) -> bool {
    let mut changed = false;
    for (block, phi) in
        function.walk().filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Phi).collect::<Vec<_>>()
    {
        let Some(result) = function.instruction(phi).result else { continue };
        let arms = function.instruction(phi).operands.chunks(2).map(|pair| pair[0]).collect::<Vec<_>>();
        if arms.len() < 2 || arms.iter().all(|one| *one == arms[0]) || !crate::spill::address_only(function, result, 3)
        {
            continue;
        }
        let shape = cfg::Shape::of(function);
        let Some(plan) = merged(function, &shape, block, &arms, 3) else { continue };
        let at = function
            .block(block)
            .iter()
            .find(|&one| function.instruction(one).opcode != Opcode::Phi)
            .expect("a terminated block");
        let made = built(function, &plan, at);
        function.replace_all_uses_with(result, made);
        function.erase(phi).expect("its uses were replaced");
        // What only the phi read is dead: an arm's copy of the address.
        let mut orphans = arms;
        while let Some(operand) = orphans.pop() {
            let Operand::Value(value) = operand else { continue };
            let ValueDef::Instruction(def) = function.value(value).def else { continue };
            let op = function.instruction(def);
            if !function.users(value).is_empty()
                || !matches!(
                    op.opcode,
                    Opcode::GetElementPtr { .. }
                        | Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
                        | Opcode::Binary(
                            BinaryOp::Add
                                | BinaryOp::Sub
                                | BinaryOp::Mul
                                | BinaryOp::Shl
                                | BinaryOp::And
                                | BinaryOp::Or
                                | BinaryOp::Xor
                        )
                )
                || function.is_erased(def)
            {
                continue;
            }
            orphans.extend(op.operands.clone());
            function.erase(def).expect("no one reads it");
        }
        changed = true;
    }
    changed
}

/// LLVM's `sinkCmpExpression`: each comparison made again at the top of every
/// block that reads it and is not its own (a phi's incoming block is not a
/// read), and erased once nothing reads the original.
fn compares_sunk(function: &mut Function) -> bool {
    let compares: Vec<(BlockId, InstId)> = function
        .walk()
        .filter(|&(_, inst)| matches!(
            function.instruction(inst).opcode,
            Opcode::ICmp(_) | Opcode::FCmp(_)
        ))
        .collect();
    let mut changed = false;
    for (home, compare) in compares {
        let Some(result) = function.instruction(compare).result else { continue };
        let mut made: Vec<(BlockId, Operand)> = Vec::new();
        for one in function.users(result).to_vec() {
            if function.instruction(one.user).opcode == Opcode::Phi {
                continue;
            }
            let Some(there) = function.parent(one.user) else { continue };
            if there == home {
                continue;
            }
            let copy = match made.iter().find(|(block, _)| *block == there) {
                Some((_, copy)) => *copy,
                None => {
                    let original = function.instruction(compare);
                    let (opcode, ty, operands, flags) =
                        (original.opcode.clone(), original.ty, original.operands.clone(), original.flags);
                    let first = function
                        .block(there)
                        .iter()
                        .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
                        .expect("a terminated block");
                    let inserted = function.create_instruction(opcode, ty, operands, flags, None);
                    function.insert(inserted, Position::Before(first)).expect("a placed instruction");
                    let copy = Operand::Value(function.instruction(inserted).result.expect("a value"));
                    made.push((there, copy));
                    copy
                }
            };
            let mut operands = function.instruction(one.user).operands.clone();
            operands[one.index as usize] = copy;
            function.set_operands(one.user, operands);
            changed = true;
        }
        if function.users(result).is_empty() && !function.is_erased(compare) {
            function.erase(compare).expect("no one reads it");
        }
    }
    changed
}

/// LLVM's `sinkAndCmp0Expression`: an `and` read only by compares of it with
/// zero, some in other blocks, is made again before each of those compares
/// that is not in its block, and the original goes once nothing reads it. It
/// is `compares_sunk`'s other half: with the compare made in each block the
/// `and` it reads was one value held from its block to the last of them (800
/// diamonds held 31 such values across up to 30 diamonds: 419 stack operands
/// where a test at each branch has none).
///
/// Only where the target's lowering says the pair folds
/// (`isMaskAndCmp0FoldingBeneficial`; none: the base class's `false`), and not
/// where both operands of the `and` are values read nowhere else, whose live
/// ranges the copy would stretch to the compare (CodeGenPrepare.cpp:2224).
fn masks_sunk(
    context: &Context,
    function: &mut Function,
    lowering: Option<&dyn llrm_mir::lowering::Lowering>,
) -> bool {
    if !lowering.is_some_and(|one| one.is_mask_and_cmp0_folding_beneficial()) {
        return false;
    }
    let masks: Vec<(BlockId, InstId)> = function
        .walk()
        .filter(|&(_, inst)| function.instruction(inst).opcode == Opcode::Binary(BinaryOp::And))
        .collect();
    let mut changed = false;
    for (home, mask) in masks {
        let Some(result) = function.instruction(mask).result else { continue };
        let read_elsewhere = |operand: &Operand| match operand {
            Operand::Value(value) => function.users(*value).len() > 1,
            _ => true,
        };
        let operands = &function.instruction(mask).operands;
        if !operands.iter().any(|operand| matches!(operand, Operand::Constant(_)) || read_elsewhere(operand)) {
            continue;
        }
        let users = function.users(result).to_vec();
        let against_zero = |user: &llrm_mir::module::Use| {
            let one = function.instruction(user.user);
            matches!(one.opcode, Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne))
                && user.index == 0
                && matches!(
                    one.operands[1],
                    Operand::Constant(id) if matches!(context.get(id).kind, ConstantKind::Int(0))
                )
        };
        if users.is_empty() || !users.iter().all(against_zero) {
            continue;
        }
        let mut made: Vec<(BlockId, Operand)> = Vec::new();
        for one in users {
            let Some(there) = function.parent(one.user) else { continue };
            if there == home {
                continue;
            }
            let copy = match made.iter().find(|(block, _)| *block == there) {
                Some((_, copy)) => *copy,
                None => {
                    // Before the first of this block's compares.
                    let first = function
                        .block(there)
                        .iter()
                        .find(|&inst| function.users(result).iter().any(|user| user.user == inst))
                        .expect("a compare in its block");
                    let original = function.instruction(mask);
                    let (opcode, ty, operands, flags) =
                        (original.opcode.clone(), original.ty, original.operands.clone(), original.flags);
                    let inserted = function.create_instruction(opcode, ty, operands, flags, None);
                    function.insert(inserted, Position::Before(first)).expect("a placed instruction");
                    let copy = Operand::Value(function.instruction(inserted).result.expect("a value"));
                    made.push((there, copy));
                    copy
                }
            };
            let mut operands = function.instruction(one.user).operands.clone();
            operands[one.index as usize] = copy;
            function.set_operands(one.user, operands);
            changed = true;
        }
        if function.users(result).is_empty() && !function.is_erased(mask) {
            function.erase(mask).expect("no one reads it");
        }
    }
    changed
}

/// The arms as one `Merged`, if they are one computation over values that reach
/// `join`.
fn merged(
    function: &Function,
    shape: &cfg::Shape,
    join: BlockId,
    arms: &[Operand],
    depth: u32,
) -> Option<Merged> {
    let reaches = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => function
                .parent(def)
                .is_some_and(|block| shape.dominance.dominates(function, cfg::id(block), cfg::id(join))),
            ValueDef::Argument(_) => true,
        },
        _ => true,
    };
    if arms.iter().all(|one| *one == arms[0]) {
        return reaches(arms[0]).then_some(Merged::Shared(arms[0]));
    }
    let defined = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(def) => Some(def),
            ValueDef::Argument(_) => None,
        },
        _ => None,
    };
    let made = arms.iter().map(|&one| defined(one)).collect::<Option<Vec<InstId>>>()?;
    let first = function.instruction(made[0]);
    let pure = matches!(
        first.opcode,
        Opcode::GetElementPtr { .. }
            | Opcode::Cast(CastOp::ZExt | CastOp::SExt | CastOp::Trunc)
            | Opcode::Binary(
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Shl
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
            )
    );
    if depth == 0
        || !pure
        || made.iter().any(|&one| {
            let other = function.instruction(one);
            other.opcode != first.opcode
                || other.ty != first.ty
                || other.flags != first.flags
                || other.operands.len() != first.operands.len()
        })
    {
        return None;
    }
    let operands = (0..first.operands.len())
        .map(|at| {
            merged(
                function,
                shape,
                join,
                &made.iter().map(|&one| function.instruction(one).operands[at]).collect::<Vec<_>>(),
                depth - 1,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Merged::Made(first.opcode.clone(), first.ty, first.flags, operands))
}

/// `plan` made before `at`.
fn built(
    function: &mut Function,
    plan: &Merged,
    at: InstId,
) -> Operand {
    match plan {
        Merged::Shared(operand) => *operand,
        Merged::Made(opcode, ty, flags, operands) => {
            let operands = operands.iter().map(|one| built(function, one, at)).collect();
            let made = function.create_instruction(opcode.clone(), *ty, operands, *flags, None);
            function.insert(made, Position::Before(at)).expect("a placed instruction");
            Operand::Value(function.instruction(made).result.expect("a value"))
        }
    }
}

#[cfg(test)]
#[path = "addresssink_tests.rs"]
mod tests;
