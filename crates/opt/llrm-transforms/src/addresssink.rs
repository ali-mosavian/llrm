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
        _: &mut Analyses,
    ) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if sunk(unit.function) {
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
