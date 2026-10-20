//! A conditional branch in an empty block that a conditional branch
//! predecessor shares a destination with, folded into the predecessor: LLVM's
//! [`SimplifyCondBranchToCondBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L4773).
//!
//! In order: a branch on the predecessor's own condition, in a block only it
//! reaches, is decided; `mergeConditionalStores` (the `MergeCondStores` step);
//! then the two conditions are joined (`brmerge`) into the predecessor's
//! branch, which now goes to the common destination or the other one.
//!
//! | # | DEVIATE |
//! |---|---|
//! | D1 | `tryWidenCondBranchToCondBranch` is not ported: it needs `llvm.experimental.widenable.condition` and a deoptimize call, which the MIR has no form for. |
//! | D2 | No branch weights, and no `unpredictable`/`TTI` predictable-threshold test of the predecessor's probability: the MIR carries none. |
//!
//! # Example
//! ```text
//!   b0: br i1 %a, label %b1, label %d         b0: %m = or i1 %a, %b   (a select if %b may be poison)
//!   b1: br i1 %b, label %d, label %e    ==>       br i1 %m, label %d, label %e
//! ```

use llrm_analysis::occurrence::{arms, from_arms};
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};

use crate::branchtocommon::{Join, logical_op};
use crate::edges;
use crate::twoentryphi::Speculation;

/// The phis a destination may have for the fold to be made
/// (`NumPhis > 2` disables it).
const MAX_PHIS: usize = 2;

/// `not x`, before `at`: `xor x, true`.
fn inverted(
    context: &Context,
    function: &mut Function,
    at: InstId,
    value: Operand,
) -> Operand {
    let ty = function.operand_type(context, value).expect("a condition");
    let one = Operand::Constant(context.int(ty, 1));
    let not =
        function.create_instruction(Opcode::Binary(BinaryOp::Xor), ty, vec![value, one], Flags::default(), Some("not"));
    function.insert(not, Position::Before(at)).expect("a new instruction");
    Operand::Value(function.instruction(not).result.expect("a value"))
}

fn successors(
    function: &Function,
    branch: InstId,
) -> Option<(Operand, BlockId, BlockId)> {
    match function.instruction(branch).operands[..] {
        [cond, Operand::Block(taken), Operand::Block(other)] if function.instruction(branch).opcode == Opcode::Br => {
            Some((cond, taken, other))
        }
        _ => None,
    }
}

/// `pbi`, a conditional branch to `bi`'s block, simplified with `bi`. Whether
/// anything changed.
pub fn simplified(
    context: &Context,
    spec: &Speculation,
    function: &mut Function,
    pbi: InstId,
    bi: InstId,
) -> bool {
    let (Some((pcond, ptaken, pother)), Some((cond, taken, other))) =
        (successors(function, pbi), successors(function, bi))
    else {
        return false;
    };
    let block = function.parent(bi).expect("a placed branch");
    let pblock = function.parent(pbi).expect("a placed branch");

    // The same condition: BI's way is known, where only PBI reaches its block;
    // `foldCondBranchOnValueKnownInPredecessor` does the rest.
    if pcond == cond && ptaken != pother && function.predecessors(block)[..] == [pblock] {
        let value = Operand::Constant(context.int(context.types.int(1), i128::from(ptaken == block)));
        function.set_operand(bi, 0, value);
        return true;
    }

    // `tryWidenCondBranchToCondBranch`: D1.
    if crate::condstores::merged(context, spec, function, pbi, bi) {
        return true;
    }

    // Only a branch in an empty block (not a phi, either) is folded.
    if function.iter_block(block).next() != Some(bi) {
        return false;
    }
    let (pbi_op, bi_op) = if ptaken == taken {
        (0, 0)
    } else if ptaken == other {
        (0, 1)
    } else if pother == taken {
        (1, 0)
    } else if pother == other {
        (1, 1)
    } else {
        return false;
    };
    let common = if pbi_op == 0 { ptaken } else { pother };
    // The infinite loop, left to the loop.
    if common == block {
        return false;
    }
    // A large number of selects is a pessimization without predication.
    if edges::phis(function, common).len() > MAX_PHIS {
        return false;
    }
    let mut other_dest = if bi_op == 0 { other } else { taken };
    if other_dest == block {
        // BB loops on itself: make that an obvious infinite loop.
        let void = function.instruction(bi).ty;
        let infloop = function.create_block(Some("infloop"));
        function.insert_block(infloop, None).expect("a new block");
        let jump = function.create_instruction(Opcode::Br, void, vec![Operand::Block(infloop)], Flags::default(), None);
        function.insert(jump, Position::End(infloop)).expect("a placed block");
        other_dest = infloop;
    }

    // Both conditions, true on the way to the common destination.
    let pbi_cond = if pbi_op == 1 { inverted(context, function, pbi, pcond) } else { pcond };
    let bi_cond = if bi_op == 1 { inverted(context, function, pbi, cond) } else { cond };
    let merged = logical_op(context, spec, function, pbi, Join::Or, pbi_cond, bi_cond);
    function.set_operands(pbi, vec![merged, Operand::Block(common), Operand::Block(other_dest)]);

    // OtherDest gains the edge from PBI's block that BB had.
    edges::predecessor_added(function, other_dest, pblock, block);
    // The common destination's phis may name different values for the two
    // blocks: a select picks the right one.
    for phi in edges::phis(function, common) {
        let mut incoming = arms(function, phi);
        let from_bi = incoming.iter().find(|&&(_, from)| from == block).map(|&(value, _)| value);
        let Some(at) = incoming.iter().position(|&(_, from)| from == pblock) else { continue };
        let (Some(from_bi), from_pbi) = (from_bi, incoming[at].0) else { continue };
        if from_bi != from_pbi {
            let ty = function.instruction(phi).ty;
            let select = function.create_instruction(
                Opcode::Select,
                ty,
                vec![pbi_cond, from_pbi, from_bi],
                Flags::default(),
                Some("mux"),
            );
            function.insert(select, Position::Before(pbi)).expect("a new instruction");
            incoming[at].0 = Operand::Value(function.instruction(select).result.expect("a value"));
            function.set_operands(phi, from_arms(&incoming));
        }
    }
    true
}

#[cfg(test)]
#[path = "condtocond_tests.rs"]
mod tests;
