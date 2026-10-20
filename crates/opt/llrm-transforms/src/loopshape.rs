//! What loop fusion and loop distribution ask of a loop alike: that one
//! counter's test is the loop's only exit, that its body only reads and
//! writes memory and cannot trap, and that nothing it computes is read
//! outside it.
//!
//! # How it works
//! `controls(function, proof, shape)`: `proof` counts the loop from its one
//! exit, which is in the latch where the loop is tested after each trip and in
//! the header where it is tested before (and the header then not its own
//! latch). `quiet(..)`: every instruction of the body is a phi, a branch, a
//! load, a store, or work with no effect that cannot trap. `header_quiet(..)`:
//! the header of a loop tested before each trip runs once more than its body,
//! so its work must be such that running once more or not at all changes
//! nothing: no memory access, as far as a header's own work goes.
//! `read_outside(..)`: a value an instruction of the loop defines is read by
//! an instruction outside it.
//!
//! # Example
//! None: a library. [`crate::loopdistribute`] asks all four of its one loop.
//!
//! # Provides
//! `controls`, `quiet`, `header_quiet`, `read_outside`.
//!
//! # See also
//! LLVM `LoopFuse`'s `isEligibleForFusion`/`containsUnsafeInstructions` and
//! `LoopDistribute`'s refusal of values used outside the loop.

use llrm_analysis::cfg::{self, Canonical};
use llrm_analysis::induction::CountedLoop;
use llrm_mir::context::Context;
use llrm_mir::module::{Function, InstId};
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Outer;

/// `proof` counts `shape` from its one exit, where the loop tests it.
pub fn controls(
    function: &Function,
    proof: &CountedLoop,
    shape: &Canonical,
) -> bool {
    let place = if proof.posttested { shape.latch } else { shape.header };
    function.parent(proof.branch) == Some(shape.exiting)
        && shape.exiting == place
        && !proof.leaves
        && !proof.stops
        && (proof.posttested || shape.header != shape.latch)
}

/// Nothing in `shape`'s body but phis, branches, loads, stores and work with
/// no effect that cannot trap.
pub fn quiet(
    context: &Context,
    outer: &Outer,
    function: &Function,
    shape: &Canonical,
) -> bool {
    shape
        .loop_
        .body
        .iter()
        .flat_map(|&at| function.iter_block(cfg::block(at)))
        .all(
            |inst| {
                let opcode = &function.instruction(inst).opcode;
                opcode.is_terminator()
                    || matches!(
                        opcode,
                        Opcode::Phi | Opcode::Load { .. } | Opcode::Store { .. }
                    )
                    || llrm_mir::memory::speculatable(context, outer.callees(), function, inst)
            },
        )
}

/// The header's own work, which a loop tested before each trip runs once more
/// than its body, has no effect and cannot trap.
pub fn header_quiet(
    context: &Context,
    outer: &Outer,
    function: &Function,
    shape: &Canonical,
) -> bool {
    function
        .iter_block(shape.header)
        .all(
            |inst| {
                let opcode = &function.instruction(inst).opcode;
                *opcode == Opcode::Phi
                    || opcode.is_terminator()
                    || llrm_mir::memory::speculatable(context, outer.callees(), function, inst)
            },
        )
}

/// A value `inst` defines is read outside `shape`.
pub fn read_outside(
    function: &Function,
    shape: &Canonical,
    inst: InstId,
) -> bool {
    function.instruction(inst).result.is_some_and(|result| {
        function.users(result).iter().any(|used| function.parent(used.user).is_none_or(|block| !shape.holds(block)))
    })
}
