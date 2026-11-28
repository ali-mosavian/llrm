//! The node predicates the Z80 tables name: llrm-dagisel's shared ones and the
//! one the Z80 prices itself.

use llrm_dagisel::dag::{Dag, Hooks, SDValue};
pub use llrm_dagisel::predicates::*;

/// `imm_stored_in_immediate_form`: a constant word is stored as two `ld (hl),n`
/// only where that is no longer than the register form, which loads the
/// constant once (`crate::expand::immediate_store_is_not_longer`).
///
/// The pin's x86 idea (`shouldAvoidImmediateInstFormsForSize`,
/// X86ISelDAGToDAG.cpp#L371): at size a constant several nodes use is kept in
/// a register. The count is the users the constant had when selection began,
/// so it does not depend on which of them was selected first; the threshold
/// is the encoding table's bytes, not 2.
///
/// DEVIATE Z80-IMM-USES (owner llrm-target-z80, exit 2026-11-30): see
/// docs/working/above-main-rows.md.
pub fn imm_stored_in_immediate_form<D: Dag, H: Hooks<D>>((dag, hooks, v): (&D, &H, SDValue)) -> bool {
    let users = hooks.uses_at_selection_start(v.node).unwrap_or(if dag.node_has_one_use(v.node) { 1 } else { 2 });
    crate::expand::immediate_store_is_not_longer(users)
}
