//! A block that is only a comparison (and a few instructions that can run
//! early) and a branch is folded into a predecessor that branches to it and to
//! one of its successors: SimplifyCFG's `foldBranchToCommonDest`.
//!
//! # How it works
//! `folded` takes the block `BB` ending in a conditional branch on a
//! comparison (or binary operation, select, truncation) of its own that only
//! the branch reads. A predecessor `P` ending in a conditional branch shares a
//! successor with it: `BB`'s instructions are copied into `P` before its
//! branch, and `P` branches on `P`'s condition and `BB`'s joined by `or`
//! or `and`, to the other target of `BB`. The join is a plain operation where
//! `BB`'s condition being poison implies `P`'s is, a `select` otherwise. At
//! most one instruction besides the condition is copied, and the join with
//! the inversion `P` may need costs two at most.
//!
//! # Example
//! The block's comparison runs in the predecessor:
//!
//! ```text
//!   p:  br %a, bb, x                 p:  %b = icmp ...
//!   bb: %b = icmp ...          ==>       %c = select %a, %b, false
//!       br %b, y, x                      br %c, y, x
//! ```
//!
//! # LLVM structure
//! | LLVM | here | |
//! |---|---|---|
//! | [`foldBranchToCommonDest`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4231) | `folded` | MIRROR; D1 |
//! | [`shouldFoldCondBranchesToCommonDestination`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4099) | `recipe` | MIRROR; D2 |
//! | [`performBranchToCommonDestFolding`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4132) | `performed` | MIRROR; D2, D3 |
//! | [`createLogicalOp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4026) | `logical_op` | MIRROR |
//! | [`safeToMergeTerminators`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L388) | `safe_to_merge_terminators` | MIRROR |
//! | [`InvertBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L1879) | `inverted` | MIRROR |
//! | `simplifycfg-branch-fold-threshold` (2), `-bonus-inst-threshold` (1), `-branch-fold-common-dest-vector-multiplier` (2) | `FOLD_THRESHOLD`, `BONUS_THRESHOLD`, `VECTOR_MULTIPLIER` | MIRROR |
//!
//! - D1. No pseudo probes (a `minsize` function is priced as `CodeSize`).
//! - D2. The MIR has no branch weights or `!unpredictable`: the fold is never
//!   held back for a predictable branch, and no weights are computed.
//! - D3. No debug locations. The bonus instructions lose the metadata that can
//!   make a speculated instruction undefined, as in
//!   [`twoentryphi`](crate::twoentryphi) (D4 there).

use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::instructioncost::instruction_cost;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::tti::{ArithOp, OperandInfo, Ty};
use llrm_mir::valuetracking::Query;
use llrm_support::hash::HashMap;

use crate::twoentryphi::Speculation;

/// `simplifycfg-branch-fold-threshold`.
const FOLD_THRESHOLD: i64 = 2;
/// `simplifycfg-bonus-inst-threshold`.
const BONUS_THRESHOLD: usize = 1;
/// `simplifycfg-branch-fold-common-dest-vector-multiplier`.
const VECTOR_MULTIPLIER: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Join {
    And,
    Or,
}

/// The common successor, how the conditions are joined, and whether `PBI`'s
/// is inverted first.
struct Recipe {
    join: Join,
    invert: bool,
}

/// `br c, t, f` as `(c, t, f)`.
fn conditional(
    function: &Function,
    block: BlockId,
) -> Option<(InstId, Operand, BlockId, BlockId)> {
    let last = function.terminator(block)?;
    let instruction = function.instruction(last);
    match instruction.operands[..] {
        [condition, Operand::Block(taken), Operand::Block(other)] if instruction.opcode == Opcode::Br => {
            Some((last, condition, taken, other))
        }
        _ => None,
    }
}

/// `shouldFoldCondBranchesToCommonDestination`.
fn recipe(
    bi: (BlockId, BlockId),
    pbi: (BlockId, BlockId),
) -> Option<Recipe> {
    if pbi.0 == bi.0 {
        Some(Recipe { join: Join::Or, invert: false })
    } else if pbi.1 == bi.1 {
        Some(Recipe { join: Join::And, invert: false })
    } else if pbi.0 == bi.1 {
        Some(Recipe { join: Join::And, invert: true })
    } else if pbi.1 == bi.0 {
        Some(Recipe { join: Join::Or, invert: true })
    } else {
        None
    }
}

/// `safeToMergeTerminators`: the phis of a successor both blocks reach take
/// the same value from either.
fn safe_to_merge_terminators(
    function: &Function,
    first: BlockId,
    second: BlockId,
) -> bool {
    #[cfg(test)]
    SAFE_TO_MERGE_ASKS.with(|count| count.set(count.get() + 1));
    first != second && incompatible_successors(function, first, second).is_empty()
}

/// `safeToMergeTerminators`' `FailBlocks`: the successors of `second` that
/// `first` reaches too and whose phis take different values from them.
pub(crate) fn incompatible_successors(
    function: &Function,
    first: BlockId,
    second: BlockId,
) -> Vec<BlockId> {
    let reached = function.successors(first);
    function
        .successors(second)
        .iter()
        .copied()
        .filter(|successor| reached.contains(successor))
        .filter(|&successor| {
            crate::edges::phis(function, successor)
                .into_iter()
                .any(
                    |phi| {
                        let arms = llrm_analysis::occurrence::arms(function, phi);
                        let from = |at: BlockId| arms.iter().find(|&&(_, one)| one == at).map(|&(value, _)| value);
                        from(first) != from(second)
                    },
                )
        })
        .collect()
}

fn has_one_use(
    function: &Function,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Value(value) if function.users(value).has_n_uses(1)
    )
}

/// Whether `block` is only the comparison its branch reads and instructions
/// that can run before it: the block, cost and predecessors `folded` folds
/// into. Whether anything changed.
pub fn folded(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
) -> bool {
    let Some((branch, condition, taken, other)) = conditional(function, block) else { return false };
    let Operand::Value(cond_value) = condition else { return false };
    let ValueDef::Instruction(cond) = function.value(cond_value).def else { return false };
    if function.parent(cond) != Some(block)
        || !matches!(
            function.instruction(cond).opcode,
            Opcode::ICmp(_)
                | Opcode::FCmp(_)
                | Opcode::Binary(_)
                | Opcode::Select
                | Opcode::Cast(llrm_mir::opcode::CastOp::Trunc)
        )
        || !function.users(cond_value).has_n_uses(1)
    {
        return false;
    }
    // Not an unrolled conditional loop.
    if taken == block || other == block {
        return false;
    }
    let cond_ty = function.value(cond_value).ty;
    let mut preds = Vec::new();
    for &pred in function.predecessors(block).iter() {
        let Some((_, pcondition, ptaken, pother)) = conditional(function, pred) else { continue };
        // The recipe is the cheaper test, and neither reads what the other
        // writes: the pin asks `safeToMergeTerminators` first.
        let Some(plan) = recipe((taken, other), (ptaken, pother)) else { continue };
        if !safe_to_merge_terminators(function, block, pred) {
            continue;
        }
        let ty = Ty::of(&context.types, cond_ty);
        if let Some(ty) = ty {
            let op = if plan.join == Join::Or { ArithOp::Or } else { ArithOp::And };
            let mut cost = spec
                .tti
                .arithmetic_cost(op, ty, crate::speculate::cost_kind(function), OperandInfo::ANY, OperandInfo::ANY);
            let invertible = has_one_use(function, pcondition)
                && matches!(
                    pcondition,
                    Operand::Value(v) if matches!(function.value(v).def, ValueDef::Instruction(d) if matches!(function.instruction(d).opcode, Opcode::ICmp(_) | Opcode::FCmp(_)))
                );
            if plan.invert && !invertible {
                cost = cost
                    + spec.tti.arithmetic_cost(
                        ArithOp::Xor,
                        ty,
                        crate::speculate::cost_kind(function),
                        OperandInfo::ANY,
                        OperandInfo::ANY,
                    );
            }
            if !cost.valid || cost.value > FOLD_THRESHOLD {
                continue;
            }
        }
        preds.push(pred);
    }
    if preds.is_empty() {
        return false;
    }
    // The bonus instructions: all of the block but the condition and the
    // branch.
    let mut bonus = 0;
    let mut vector = false;
    let count = preds.len();
    for inst in function.iter_block(block).collect::<Vec<_>>() {
        if inst == cond || inst == branch {
            continue;
        }
        if !crate::licm::hoist::is_safe_to_speculatively_execute_for_cfg(
            context,
            spec.layout,
            function,
            spec.outer,
            inst,
        ) {
            return false;
        }
        let instruction = function.instruction(inst);
        let is_vector = |ty| matches!(context.types.get(ty), llrm_mir::types::Type::Vector { .. });
        vector |= is_vector(instruction.ty)
            || instruction.operands.iter().any(|&o| function.operand_type(context, o).is_some_and(is_vector));
        let price = instruction_cost(
            spec.tti,
            context,
            spec.layout,
            &spec.outer.globals,
            function,
            inst,
            crate::speculate::cost_kind(function),
        );
        if !price.valid || price.value != 0 {
            bonus += count;
            if bonus > BONUS_THRESHOLD * VECTOR_MULTIPLIER {
                return false;
            }
        }
        // Every use of it is in this block after it, or a phi's input from
        // this block.
        if let Some(result) = instruction.result {
            for one in function.users(result).iter() {
                let user = function.instruction(one.user);
                let fine = if user.opcode == Opcode::Phi {
                    user.operands[one.index as usize + 1] == Operand::Block(block)
                } else {
                    function.parent(one.user) == Some(block) && function.comes_before(inst, one.user)
                };
                if !fine {
                    return false;
                }
            }
        }
    }
    if bonus > BONUS_THRESHOLD * if vector { VECTOR_MULTIPLIER } else { 1 } {
        return false;
    }
    performed(context, spec, function, block, preds[0])
}

/// `InvertBranch`: `pbi` branches the other way on the negation of its
/// condition.
fn inverted(
    context: &Context,
    function: &mut Function,
    pbi: InstId,
) {
    let condition = function.instruction(pbi).operands[0];
    let compare = match condition {
        Operand::Value(value) if function.users(value).has_n_uses(1) => match function.value(value).def {
            ValueDef::Instruction(def) => match function.instruction(def).opcode {
                Opcode::ICmp(p) => {
                    function.set_icmp_predicate(def, p.inverse());
                    true
                }
                Opcode::FCmp(p) => {
                    function.set_fcmp_predicate(def, p.inverse());
                    true
                }
                _ => false,
            },
            _ => false,
        },
        _ => false,
    };
    if !compare {
        let ty = function.operand_type(context, condition).expect("a condition");
        let one = Operand::Constant(context.int(ty, 1));
        let not = function.create_instruction(
            Opcode::Binary(llrm_mir::opcode::BinaryOp::Xor),
            ty,
            vec![condition, one],
            Flags::default(),
            Some("not"),
        );
        function.insert(not, Position::Before(pbi)).expect("a new instruction");
        let value = Operand::Value(function.instruction(not).result.expect("a value"));
        function.set_operand(pbi, 0, value);
    }
    let operands = function.instruction(pbi).operands.clone();
    function.set_operand(pbi, 1, operands[2]);
    function.set_operand(pbi, 2, operands[1]);
}

/// `createLogicalOp`: `lhs` joined to `rhs`, by the operation where `rhs`
/// being poison implies `lhs` is, by a select otherwise.
pub(crate) fn logical_op(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    before: InstId,
    join: Join,
    lhs: Operand,
    rhs: Operand,
) -> Operand {
    let ty = function.operand_type(context, lhs).expect("a condition");
    let plain = {
        let query = Query::bare(context, function, spec.layout);
        llrm_mir::valuetracking::guarantee::implies_poison(&query, rhs, lhs)
    };
    let (opcode, operands) = if plain {
        (
            Opcode::Binary(if join == Join::And {
                llrm_mir::opcode::BinaryOp::And
            } else {
                llrm_mir::opcode::BinaryOp::Or
            }),
            vec![lhs, rhs],
        )
    } else {
        let constant = |value| Operand::Constant(context.int(ty, value));
        let operands = if join == Join::And { vec![lhs, rhs, constant(0)] } else { vec![lhs, constant(1), rhs] };
        (Opcode::Select, operands)
    };
    let inst = function.create_instruction(opcode, ty, operands, Flags::default(), Some("or.cond"));
    function.insert(inst, Position::Before(before)).expect("a new instruction");
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

/// `performBranchToCommonDestFolding`.
fn performed(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    block: BlockId,
    pred: BlockId,
) -> bool {
    let (branch, _, taken, other) = conditional(function, block).expect("a conditional branch");
    let (pbi, _, ptaken, pother) = conditional(function, pred).expect("a conditional branch");
    let plan = recipe((taken, other), (ptaken, pother)).expect("a recipe");
    if plan.invert {
        inverted(context, function, pbi);
    }
    let (ptaken, _) = match (function.instruction(pbi).operands[1], function.instruction(pbi).operands[2]) {
        (Operand::Block(a), Operand::Block(b)) => (a, b),
        _ => unreachable!("a conditional branch"),
    };
    let unique = if ptaken == block { taken } else { other };
    // The successor gains a predecessor with what `block` gave it.
    for phi in crate::edges::phis(function, unique) {
        let arms = llrm_analysis::occurrence::arms(function, phi);
        let value = arms.iter().find(|&&(_, one)| one == block).map(|&(value, _)| value).expect("an arm");
        let mut operands = function.instruction(phi).operands.clone();
        operands.push(value);
        operands.push(Operand::Block(pred));
        function.set_operands(phi, operands);
    }
    // PBI's edge to `block` goes to the successor.
    let at = if function.instruction(pbi).operands[1] == Operand::Block(block) { 1 } else { 2 };
    function.set_operand(pbi, at, Operand::Block(unique));
    if let Some(loop_id) =
        function.instruction(branch).metadata.iter().find(|(kind, _)| kind == "llvm.loop").map(|&(_, id)| id)
    {
        function.set_metadata(pbi, "llvm.loop", Some(loop_id));
    }
    // Copies of the block's instructions run before PBI.
    let mut mapped: HashMap<Operand, Operand> = HashMap::default();
    for inst in function.iter_block(block).collect::<Vec<_>>() {
        if inst == branch {
            continue;
        }
        let copy = function.clone_instruction(inst);
        for index in 0..function.instruction(copy).operands.len() {
            if let Some(&to) = mapped.get(&function.instruction(copy).operands[index]) {
                function.set_operand(copy, index, to);
            }
        }
        crate::twoentryphi::metadata_dropped(function, copy);
        function.insert(copy, Position::Before(pbi)).expect("a new instruction");
        if let (Some(old), Some(new)) = (function.instruction(inst).result, function.instruction(copy).result) {
            mapped.insert(Operand::Value(old), Operand::Value(new));
            // A phi input from the predecessor reads the copy.
            for one in function.users(old).iter().copied().collect::<Vec<_>>() {
                let user = function.instruction(one.user);
                if user.opcode == Opcode::Phi && user.operands[one.index as usize + 1] == Operand::Block(pred) {
                    function.set_operand(one.user, one.index as usize, Operand::Value(new));
                }
            }
        }
    }
    let original = function.instruction(branch).operands[0];
    let copied = *mapped.get(&original).expect("the condition was cloned");
    let pcondition = function.instruction(pbi).operands[0];
    let joined = logical_op(context, spec, function, pbi, plan.join, pcondition, copied);
    function.set_operand(pbi, 0, joined);
    true
}

#[cfg(test)]
#[path = "branchtocommon_tests.rs"]
mod tests;

#[cfg(test)]
thread_local! {
    /// Predecessor pairs `safe_to_merge_terminators` has been asked about on
    /// this thread.
    pub(crate) static SAFE_TO_MERGE_ASKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
