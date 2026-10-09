//! CodeGenPrepare's `sinkCmpExpression`: a comparison a branch in another block
//! reads is made again beside that branch, so isel keeps it in the flags (a
//! loop's test, GVN'd with the copy of it made ahead of the loop, was an `i1`
//! held across the loop's body: `sete`, a store of the byte, `cmp` and `jne`,
//! hanoi).
//!
//! A phi read only as an address, whose arms each compute the same thing of
//! values the join sees, is that computation made once in the join: GVNSink
//! and CodeGenPrepare's address sinking, so that isel matches one addressing
//! form for the access.
//!
//! `a(i) = ...` under an `IF` computes its element's address in each arm, and
//! the store after the join reads a phi of equal addresses: a register held
//! across the arms where `A%+disp[bx]` would do (RGBLIGHTS, #386).
//!
//! It runs last, with `gepoffset`: until then gvn's partial redundancy
//! elimination makes exactly this phi, so that a load after the join reads
//! what the arms stored, and a sink earlier would be undone by it.

use llrm_analysis::cfg;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, Opcode};
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
        // Blocks and edges are as they were.
        let sunk_compares = !analyses.outer().target().multiple_condition_registers() && compares_sunk(unit.function);
        if sunk(unit.function) || sunk_compares {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
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
            .instructions()
            .iter()
            .copied()
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
                        .instructions()
                        .iter()
                        .copied()
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
            ValueDef::Instruction(def) => {
                function.parent(def).is_some_and(|block| shape.dominance.dominates(cfg::id(block), cfg::id(join)))
            }
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
