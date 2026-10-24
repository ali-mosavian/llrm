//! CodeGenPrepare's `optimizeBranch`: a branch on a compare with a constant
//! becomes a branch on a compare with zero of a value a successor computes
//! anyway, where the target prefers one
//! ([CodeGenPrepare.cpp#L8878](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L8878)).
//!
//! # How it works
//! Only where `Lowering::prefer_zero_compare_branch` says so (x86 does,
//! `X86ISelLowering.cpp` L3805). For a block that ends in a branch on an
//! `icmp` of `%x` with a constant, read by nothing else, each user of `%x`
//! in the block or in a successor with one predecessor is looked at:
//! - `icmp ult %x, 2^k` and `lshr`/`ashr %x, k`: the shift is compared with
//!   zero (`icmp eq`);
//! - `icmp eq|ne %x, c` and `add %x, -c`, `sub %x, c` or `xor %x, c`: that
//!   value is compared with zero by the same predicate.
//!
//! The user moves to just before the branch (its poison-generating flags
//! dropped: it runs on paths it did not), the new compare is made there and
//! takes the old one's place. One conversion per branch.
//!
//! ```text
//!  b0: %c = icmp ult i16 %x, 8            b0: %t = lshr i16 %x, 3
//!      br i1 %c, label %b1, label %b2         %c.0 = icmp eq i16 %t, 0
//!  b1: %t = lshr i16 %x, 3                    br i1 %c.0, label %b1, label %b2
//! ```
//!
//! # Example
//! `test_an_unsigned_compare_with_a_power_of_two_becomes_a_compare_of_the_shift_with_zero`
//! in `cgpbranch_tests.rs` is the sketch above.
//!
//! # Runs
//! In `cgpaddr`'s run, before the compare sinks.

use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::edit::Position;
use llrm_mir::lowering::Lowering;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};

/// `optimizeBranch` over every conditional branch of `function`. Whether any
/// changed.
pub fn branches_optimized(
    context: &Context,
    function: &mut Function,
    lowering: Option<&dyn Lowering>,
) -> bool {
    if !lowering.is_some_and(|one| one.prefer_zero_compare_branch()) {
        return false;
    }
    let branches: Vec<(BlockId, InstId)> = function
        .layout()
        .iter()
        .filter_map(|&block| function.terminator(block).map(|branch| (block, branch)))
        .filter(|&(_, branch)| function.instruction(branch).opcode == Opcode::Br)
        .collect();
    let mut changed = false;
    for (home, branch) in branches {
        changed |= optimize_branch(context, function, home, branch);
    }
    changed
}

fn constant(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

fn optimize_branch(
    context: &Context,
    function: &mut Function,
    home: BlockId,
    branch: InstId,
) -> bool {
    let operands = function.instruction(branch).operands.clone();
    let [Operand::Value(condition), Operand::Block(yes), Operand::Block(no)] = operands[..] else { return false };
    let llrm_mir::module::ValueDef::Instruction(compare) = function.value(condition).def else { return false };
    let Opcode::ICmp(predicate) = function.instruction(compare).opcode else { return false };
    if !function.users(condition).has_one_use() {
        return false;
    }
    let [Operand::Value(x), limit] = function.instruction(compare).operands[..] else { return false };
    let Some(limit) = constant(context, limit) else { return false };
    let ty = function.value(x).ty;
    let Some(bits) = context.types.int_bits(ty) else { return false };
    let mask = llrm_mir::context::mask(bits);
    let negated = limit.wrapping_neg() & mask;
    // The first user the pin would convert, found by walking the use list.
    let mut found: Option<(InstId, BlockId, IntPredicate)> = None;
    for one in function.users(x).iter() {
        let user = one.user;
        if user == compare {
            continue;
        }
        // A quick dominance check: the block of the branch, or a successor
        // that only the branch reaches.
        let Some(parent) = function.parent(user) else { continue };
        if parent != home && (parent != yes && parent != no || function.sole_predecessor(parent).is_none()) {
            continue;
        }
        let instruction = function.instruction(user);
        let [Operand::Value(first), second] = instruction.operands[..] else { continue };
        if first != x {
            continue;
        }
        let against = constant(context, second);
        let power_of_two = limit.is_power_of_two() && predicate == IntPredicate::Ult;
        let shifted = matches!(
            instruction.opcode,
            Opcode::Binary(BinaryOp::LShr | BinaryOp::AShr)
        )
            && against == Some(u128::from(limit.trailing_zeros()));
        let equality = matches!(predicate, IntPredicate::Eq | IntPredicate::Ne);
        let folded = match instruction.opcode {
            Opcode::Binary(BinaryOp::Add) => against == Some(negated),
            Opcode::Binary(BinaryOp::Sub | BinaryOp::Xor) => against == Some(limit),
            _ => false,
        };
        let new_predicate = if power_of_two && shifted {
            IntPredicate::Eq
        } else if equality && folded {
            predicate
        } else {
            continue;
        };
        found = Some((user, parent, new_predicate));
        break;
    }
    let Some((user, parent, new_predicate)) = found else { return false };
    let value_ty = function.instruction(user).ty;
    let Some(result) = function.instruction(user).result else { return false };
    if parent != home {
        function.move_to(user, Position::Before(branch)).expect("a placed instruction");
    }
    function.drop_poison_generating_annotations(user);
    let zero = Operand::Constant(context.int(value_ty, 0));
    let bool_ty = function.instruction(compare).ty;
    let new = function.create_instruction(
        Opcode::ICmp(new_predicate),
        bool_ty,
        vec![Operand::Value(result), zero],
        Flags::default(),
        None,
    );
    function.insert(new, Position::Before(branch)).expect("an unplaced instruction");
    let made = function.instruction(new).result.expect("a compare's value");
    function.replace_all_uses_with(condition, Operand::Value(made));
    function.erase(compare).expect("nothing reads it now");
    true
}

#[cfg(test)]
#[path = "cgpbranch_tests.rs"]
mod tests;
