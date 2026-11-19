//! The PatFrag bodies the x86 tables ask of the target beyond the shared ones
//! in [`llrm_dagisel::predicates`]: the ones in `X86InstrFragments.td`,
//! `X86InstrCompiler.td` and `X86InstrInfo.td` that read the subtarget or a
//! node's users.

use llrm_dagisel::dag::{Dag, Hooks, SDValue};
pub use llrm_dagisel::predicates::*;

type Args<'a, D, H> = (&'a D, &'a H, SDValue);

/// What the x86 PatFrags ask of the target beyond [`Hooks`]: the bodies in
/// `X86InstrFragments.td`, `X86InstrCompiler.td` and `X86InstrInfo.td` that
/// read the subtarget or a node's users. The x86 tables' `node_predicate` takes
/// a `Hooks` that also has these; another target's table does not.
pub trait X86Predicates<D: Dag>: Hooks<D> {
    /// `Subtarget->hasSSEUnalignedMem()`.
    fn has_sse_unaligned_mem(&self) -> bool;
    /// `shouldAvoidImmediateInstFormsForSize(N)`.
    fn should_avoid_immediate_inst_forms_for_size(
        &self,
        dag: &D,
        node: u32,
    ) -> bool;
    /// `isUnneededShiftMask(N, width)`.
    fn is_unneeded_shift_mask(
        &self,
        dag: &D,
        node: u32,
        width: u32,
    ) -> bool;
    /// `isSExtAbsoluteSymbolRef(width, N)`.
    fn is_sext_absolute_symbol_ref(
        &self,
        dag: &D,
        width: u32,
        node: u32,
    ) -> bool;
    /// `hasNoCarryFlagUses(Flags)`.
    fn has_no_carry_flag_uses(
        &self,
        dag: &D,
        flags: SDValue,
    ) -> bool;
    /// `isDef32(N)`.
    fn is_def32(
        &self,
        dag: &D,
        node: u32,
    ) -> bool;
    /// `checkTCRetEnoughRegs(N)`.
    fn check_tc_ret_enough_regs(
        &self,
        dag: &D,
        node: u32,
    ) -> bool;
}

pub fn memop<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.has_sse_unaligned_mem() || dag.alignment(v.node) >= dag.memory_store_size(v.node)
}

pub fn not_avoid_immediate_forms<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    !hooks.should_avoid_immediate_inst_forms_for_size(dag, v.node)
}

pub fn shift_mask_8<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 3)
}

pub fn shift_mask_16<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 4)
}

pub fn shift_mask_32<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 5)
}

pub fn shift_mask_64<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 6)
}

pub fn reloc_imm_sext_32<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_sext_absolute_symbol_ref(dag, 32, v.node)
}

pub fn reloc_imm_sext_8<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_sext_absolute_symbol_ref(dag, 8, v.node)
}

/// `X86add_flag_nocf` and its kin: no use of the flags result reads the carry.
pub fn no_carry_flag_uses_of_flags<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.has_no_carry_flag_uses(dag, SDValue::new(v.node, 1))
}

/// `X86sub_flag_nocf`: only when the result is used too, so `dec` is not a pure
/// compare.
pub fn sub_no_carry_flag_uses<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    !dag.has_n_uses_of_value(SDValue::new(v.node, 0), 0) && hooks.has_no_carry_flag_uses(dag, SDValue::new(v.node, 1))
}

/// `X86lock_add_nocf`: the flags are the node's first result.
pub fn no_carry_flag_uses_of_result<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.has_no_carry_flag_uses(dag, SDValue::new(v.node, 0))
}

pub fn def32<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_def32(dag, v.node)
}

pub fn tcret_enough_regs<D: Dag, H: X86Predicates<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.check_tc_ret_enough_regs(dag, v.node)
}
