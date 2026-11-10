//! Builders and queries the idiom recognizers share: constants, placed
//! instructions, `CreateZExtOrTrunc`, `matchCondition`, `getRecurrenceVar`, the
//! uses of a value outside a block, and the dead cascade a rewrite leaves.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::types::TypeId;

pub(super) fn int(
    context: &Context,
    ty: TypeId,
    value: u128,
) -> Operand {
    Operand::Constant(context.int(ty, value as i128))
}

pub(super) fn width_of(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> u32 {
    let ty = function.operand_type(context, operand).expect("a typed operand");
    context.types.int_bits(ty).expect("an integer")
}

/// An instruction made and placed before `at`.
pub(super) fn emit(
    function: &mut Function,
    at: InstId,
    opcode: Opcode,
    ty: TypeId,
    operands: Vec<Operand>,
    flags: Flags,
    name: &str,
) -> Operand {
    let inst = function.create_instruction(opcode, ty, operands, flags, Some(name).filter(|name| !name.is_empty()));
    function.insert(inst, Position::Before(at)).expect("a placed instruction");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

/// `CreateZExtOrTrunc`.
pub(super) fn zext_or_trunc(
    context: &Context,
    function: &mut Function,
    at: InstId,
    value: Operand,
    to: TypeId,
    name: &str,
) -> Operand {
    let from = width_of(context, function, value);
    let target = context.types.int_bits(to).expect("an integer");
    if from == target {
        return value;
    }
    let cast = if from < target { CastOp::ZExt } else { CastOp::Trunc };
    emit(function, at, Opcode::Cast(cast), to, vec![value], Flags::default(), name)
}

/// The users of `value` outside `block`, as (instruction, operand index).
pub(super) fn uses_outside(
    function: &Function,
    value: llrm_mir::module::ValueId,
    block: llrm_mir::module::BlockId,
) -> Vec<(InstId, usize)> {
    function
        .users(value)
        .into_iter()
        .filter(|one| function.parent(one.user) != Some(block))
        .map(|one| (one.user, one.index as usize))
        .collect()
}

/// `RecursivelyDeleteDeadPHINode` for the dead recurrences here: the phi, and
/// each pure instruction that nothing else reads once it is gone.
pub(super) fn delete_dead_cascade(
    function: &mut Function,
    first: InstId,
) {
    let mut worklist = vec![first];
    let mut erased: llrm_mir::dense::IdSet<InstId> = Default::default();
    while let Some(inst) = worklist.pop() {
        if erased.contains(&inst) {
            continue;
        }
        let instruction = function.instruction(inst);
        let pure = match instruction.opcode {
            Opcode::Phi | Opcode::Select | Opcode::Cast(_) | Opcode::ICmp(_) => true,
            Opcode::Binary(op) => !matches!(
                op,
                BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem
            ),
            _ => false,
        };
        let unused = instruction.result.is_none_or(|result| function.users(result).is_empty());
        if !pure || !unused {
            continue;
        }
        let operands: Vec<InstId> = instruction
            .operands
            .iter()
            .filter_map(|operand| match operand {
                Operand::Value(value) => match function.value(*value).def {
                    llrm_mir::module::ValueDef::Instruction(def) => Some(def),
                    llrm_mir::module::ValueDef::Argument(_) => None,
                },
                _ => None,
            })
            .collect();
        function.erase(inst).expect("a dead instruction");
        erased.insert(inst);
        worklist.extend(operands);
    }
}

/// The instruction that defines `operand`, where it is one.
pub(super) fn defining_instruction(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2019
/// `matchCondition`: the variable `terminator`, a conditional branch, compares
/// with zero, when it yields to `loop_entry` on non-zero (on zero:
/// `jmp_on_zero`).
pub(super) fn match_condition(
    context: &Context,
    function: &Function,
    terminator: InstId,
    loop_entry: BlockId,
    jmp_on_zero: bool,
) -> Option<Operand> {
    let branch = function.instruction(terminator);
    let [condition, Operand::Block(mut true_successor), Operand::Block(mut false_successor)] = branch.operands[..]
    else {
        return None;
    };
    let compare = defining_instruction(function, condition)?;
    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { return None };
    let [tested, Operand::Constant(zero)] = function.instruction(compare).operands[..] else { return None };
    if !matches!(
        context.get(zero).kind,
        ConstantKind::Int(0) | ConstantKind::Zero
    ) {
        return None;
    }
    if jmp_on_zero {
        std::mem::swap(&mut true_successor, &mut false_successor);
    }
    ((predicate == IntPredicate::Ne && true_successor == loop_entry)
        || (predicate == IntPredicate::Eq && false_successor == loop_entry))
        .then_some(tested)
}

// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopIdiomRecognize.cpp#L2342
/// `getRecurrenceVar`: `var` as the phi of `loop_entry` that `def` feeds.
pub(super) fn get_recurrence_var(
    function: &Function,
    var: Operand,
    def: InstId,
    loop_entry: BlockId,
) -> Option<InstId> {
    let phi = defining_instruction(function, var)?;
    let phi_inst = function.instruction(phi);
    let result = Operand::Value(function.instruction(def).result?);
    (phi_inst.opcode == Opcode::Phi
        && function.parent(phi) == Some(loop_entry)
        && (phi_inst.operands[0] == result || phi_inst.operands[2] == result))
        .then_some(phi)
}
