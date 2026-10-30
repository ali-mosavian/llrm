//! Loop-invariant values the preheader computes and the loop never reads, moved
//! to the loop's single exit block, where they cost no register in the loop:
//! `IndVarSimplify::sinkUnusedInvariants`, the pass's late cleanup of
//! preheaders.
//!
//! # How it works
//! A loop with one exit block and a preheader. The preheader is walked from its
//! end; an instruction with no effect but its value, that reads no memory and
//! is no `alloca`, moves before the exit block's first non-phi instruction
//! when no use is in the preheader or the loop (a phi's use is where its
//! incoming block is).
//!
//! ```text
//!  preheader: %k = mul i32 %a, 7     preheader: br label %loop
//!             br label %loop   ==>   exit:      %k = mul i32 %a, 7
//!  loop:      ...                               ret i32 %k
//!  exit:      ret i32 %k
//! ```
//!
//! This is not [`crate::exitsink`], which moves a loop's own final update to
//! its exits.

use llrm_analysis::cfg;
use llrm_analysis::cfg::Around;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::memory::{Callees, only_value};
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/IndVarSimplify.cpp#L1206
/// `sinkUnusedInvariants` for each loop of the function; whether anything
/// moved.
pub fn sunk(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    loops: &[Loop],
) -> bool {
    let mut changed = false;
    for loop_ in loops {
        changed |= sunk_from(context, callees, function, loop_);
    }
    changed
}

fn sunk_from(
    context: &Context,
    callees: &Callees,
    function: &mut Function,
    loop_: &Loop,
) -> bool {
    // `getExitBlock`: the one block the loop leaves to.
    let exits: std::collections::BTreeSet<i64> = loop_.exits(function).into_iter().map(|(_, to)| to).collect();
    let [exit] = exits.iter().copied().collect::<Vec<_>>()[..] else { return false };
    let Some(preheader) = loop_.preheader(function) else { return false };
    let (exit, preheader) = (cfg::block(exit), cfg::block(preheader));
    let Some(terminator) = function.terminator(preheader) else { return false };

    let mut changed = false;
    let mut candidates: Vec<InstId> = function.iter_block(preheader).collect();
    candidates.reverse();
    for inst in candidates {
        if inst == terminator {
            continue;
        }
        // New instructions were inserted at the end of the preheader.
        if function.instruction(inst).opcode == Opcode::Phi {
            break;
        }
        // Nothing that has an effect, reads memory, or is an EH pad or an
        // `alloca`: an effect must complete before the loop's, and a static
        // `alloca` stays in the entry block.
        let opcode = &function.instruction(inst).opcode;
        if !only_value(context, callees, function, inst)
            || matches!(
                opcode,
                Opcode::Load { .. } | Opcode::Alloca { .. } | Opcode::LandingPad { .. }
            )
            || function.instruction(inst).result.is_none()
        {
            continue;
        }
        if used_in_or_before_the_loop(function, loop_, preheader, inst) {
            continue;
        }
        let Some(first) = function.iter_block(exit).find(|&one| function.instruction(one).opcode != Opcode::Phi) else {
            continue;
        };
        llrm_support::debug!(
            "indvars",
            "sink {} to the exit {:?}",
            function.instruction(inst).opcode.mnemonic(),
            function.instruction(inst).result.and_then(|value| function.value(value).name.clone())
        );
        function.move_to(inst, Position::Before(first)).expect("a placed instruction");
        changed = true;
    }
    changed
}

/// Whether a use of the result is in the preheader or the loop, where a phi's
/// use is in its incoming block.
fn used_in_or_before_the_loop(
    function: &Function,
    loop_: &Loop,
    preheader: BlockId,
    inst: InstId,
) -> bool {
    let value = function.instruction(inst).result.expect("a value");
    function
        .users(value)
        .iter()
        .any(
            |one| {
                let user = function.instruction(one.user);
                let block = match (&user.opcode, user.operands.get(one.index as usize + 1)) {
                    (Opcode::Phi, Some(&Operand::Block(from))) => Some(from),
                    _ => function.parent(one.user),
                };
                block.is_some_and(|at| at == preheader || loop_.body.contains(&cfg::id(at)))
            },
        )
}

#[cfg(test)]
#[path = "sinkinvariants_tests.rs"]
mod tests;
