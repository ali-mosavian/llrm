//! A value a loop's preheader computes and the loop never reads, moved to the
//! loop's exit: LLVM's `IndVarSimplify::sinkUnusedInvariants`.
//!
//! # How it works
//! For a loop with one preheader and one exit block, the preheader is walked
//! backwards. An instruction that has no side effects, does not read memory
//! and has no user in the preheader or the loop (a phi's use counts at its
//! incoming block) moves before the first non-phi instruction of the exit. The
//! walk is backwards, so a chain of such values moves together.
//!
//! A counter's increment that an inner loop's preheader holds is such a value:
//! only the outer latch reads it, and it moves there instead of living across
//! the inner loop.
//!
//! ```text
//!  outer:  %i.next = add %i, 1           outer:  br label %inner
//!          br label %inner         ==>   inner:  ...
//!  inner:  ...                           latch:  %i.next = add %i, 1
//!  latch:  %go = icmp %i.next, %n                %go = icmp %i.next, %n
//! ```
//!
//! # Example
//! From unit test `test_an_increment_only_the_latch_reads_moves_there`.

use std::collections::BTreeSet;

use llrm_analysis::cfg::{self, Around};
use llrm_analysis::effects::Declarations;
use llrm_analysis::graph::loops::Loop;
use llrm_mir::context::Context;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::Opcode;

use crate::local::may_have_side_effects;

/// Every loop's unused preheader values sunk; whether any moved. `loops` are
/// the body's, innermost first: a sink moves an instruction between blocks
/// and leaves the blocks and edges as they are.
pub fn sunk(
    context: &Context,
    callees: &Callees,
    declarations: &Declarations,
    function: &mut Function,
    loops: &[Loop],
    keep_pressure: bool,
) -> bool {
    let mut changed = false;
    for loop_ in loops {
        changed |= sunk_from(context, callees, declarations, function, loop_, keep_pressure);
    }
    changed
}

/// Whether sinking `inst` past the loop would keep more values alive across it
/// than it frees: its result is one live value, and each operand that nothing
/// outside the preheader reads is another one that the loop would now carry.
/// gcc's `move_loop_invariants` weighs the same registers
/// (`ira_loop_border_costs`); the pin does not.
fn lengthens_live_ranges(
    function: &Function,
    inst: InstId,
    preheader: BlockId,
) -> bool {
    let mut carried = BTreeSet::new();
    for operand in &function.instruction(inst).operands {
        let Operand::Value(value) = *operand else { continue };
        let read_elsewhere =
            function.users(value).iter().any(|one| one.user != inst && function.parent(one.user) != Some(preheader));
        if !read_elsewhere {
            carried.insert(value);
        }
    }
    carried.len() > 1
}

fn sunk_from(
    context: &Context,
    callees: &Callees,
    declarations: &Declarations,
    function: &mut Function,
    loop_: &Loop,
    keep_pressure: bool,
) -> bool {
    let exits: BTreeSet<i64> = loop_.exits(function).into_iter().map(|(_, to)| to).collect();
    let (Some(&exit), 1) = (exits.iter().next(), exits.len()) else { return false };
    let Some(preheader) = loop_.preheader(function) else { return false };
    let (preheader, exit) = (cfg::block(preheader), cfg::block(exit));
    let mut changed = false;
    let held: Vec<InstId> = function.iter_block(preheader).collect();
    // The last is the terminator.
    for &inst in held.iter().rev().skip(1) {
        let opcode = &function.instruction(inst).opcode;
        if *opcode == Opcode::Phi {
            break;
        }
        if matches!(opcode, Opcode::Alloca { .. } | Opcode::LandingPad { .. })
            || memory::of(context, callees, function, inst).reads
            || may_have_side_effects(context, callees, declarations, function, inst)
        {
            continue;
        }
        let Some(result) = function.instruction(inst).result else { continue };
        let used_in_loop = function
            .users(result)
            .iter()
            .any(
                |one| {
                    let user = function.instruction(one.user);
                    let at = match (&user.opcode, user.operands.get(one.index as usize + 1)) {
                        (Opcode::Phi, Some(&Operand::Block(from))) => Some(from),
                        _ => function.parent(one.user),
                    };
                    at.is_some_and(|at| at == preheader || loop_.body.contains(&cfg::id(at)))
                },
            );
        if used_in_loop || (keep_pressure && lengthens_live_ranges(function, inst, preheader)) {
            continue;
        }
        let first = function.iter_block(exit).find(|&one| function.instruction(one).opcode != Opcode::Phi);
        let Some(first) = first else { continue };
        function.move_to(inst, Position::Before(first)).expect("a placed instruction");
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "invariantsink_tests.rs"]
pub(crate) mod tests;
