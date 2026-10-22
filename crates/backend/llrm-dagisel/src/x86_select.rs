//! `X86DAGToDAGISel::Select` ([L5513][select]): the hand matchers, then the table.
//!
//! # How it works
//! [`select_node`] is what the harness calls for each DAG node in the pin's order. A `STORE` first
//! tries to fold with its load and arithmetic into one read-modify-write instruction
//! ([`fold_load_store_into_mem_operand`]); a shift first has its count rewritten when the hardware
//! mask makes the rewrite free ([`try_shift_amount_mod`]); everything else, and what the hand
//! matchers decline, goes to the generated table through [`Selector`].
//!
//! Numbered deviation: **SD301** the pin's `Select` has more hand cases than these four (`AND`/`OR`/
//! `XOR` immediate shrinking, `MUL_LOHI`, the 64-bit forms, `INTRINSIC`s, atomics, `ADD` to `LEA`
//! for flag-less adds); each is added with the pattern that needs it. Owner sdag-c. Exit: C3
//! follow-ups. Review 2026-12-10.
//!
//! # Used by
//! The shadow harness (sdag-d), once per node, over each block's nodes bottom-up.
//!
//! [select]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L5513

use crate::generated::ids::isd;
use crate::interp::{Outcome, Selector};
use crate::x86_addr::AddrDag;
use crate::x86_divrem::select_divrem;
use crate::x86_hooks::X86Hooks;
use crate::x86_mul::select_mul_lohi;
use crate::x86_rmw::fold_load_store_into_mem_operand;
use crate::x86_shift::try_shift_amount_mod;

/// Selects `node`: the hand matchers first, then the table.
pub fn select_node<D: AddrDag>(selector: &Selector, dag: &mut D, hooks: &mut X86Hooks, node: u32) -> Outcome {
    match dag.opcode(node) {
        isd::STORE => {
            if fold_load_store_into_mem_operand(dag, hooks, node) {
                return Outcome::Selected;
            }
        }
        isd::SDIVREM | isd::UDIVREM => {
            if select_divrem(dag, hooks, node) {
                return Outcome::Selected;
            }
        }
        isd::SMUL_LOHI | isd::UMUL_LOHI => {
            if select_mul_lohi(dag, hooks, node) {
                return Outcome::Selected;
            }
        }
        isd::SHL | isd::SRL | isd::SRA | isd::ROTL | isd::ROTR => {
            // The node may merge with an equal one once its count changes.
            if let Some(updated) = try_shift_amount_mod(dag, node) {
                return selector.select(dag, hooks, updated);
            }
        }
        _ => {}
    }
    selector.select(dag, hooks, node)
}
