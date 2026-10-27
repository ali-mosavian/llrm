//! The bodies of the pin's node predicates (`CheckNodePredicate`), the ones the
//! x86-32 table without vectors still names.
//!
//! The pin writes each as C++ over the node; the generated `node_predicate`
//! calls the function `features.toml` names for it. Each is a translation of
//! its PatFrag body in `X86InstrFragments.td`, `X86InstrCompiler.td` and
//! `TargetSelectionDAG.td`.

use crate::dag::{Dag, FpClass, Hooks, LoadExt, SDValue};
use crate::generated::ids::{isd, mvt};
use crate::matcher::Vt;

type Args<'a, D, H> = (&'a D, &'a H, SDValue);

/// `EnablePromoteAnyextLoad` (`x86-promote-anyext-load`, default true).
const ENABLE_PROMOTE_ANYEXT_LOAD: bool = true;

pub fn non_extending_load<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(LoadExt::NonExt)
}

pub fn unindexed_load<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    !dag.is_indexed(v.node)
}

pub fn unindexed_store<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    unindexed_load(args)
}

pub fn non_truncating_store<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    !dag.is_truncating_store(v.node)
}

pub fn truncating_store<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.is_truncating_store(v.node)
}

pub fn has_one_use<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.node_has_one_use(v.node)
}

pub fn two_uses_of_first_value<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.has_n_uses_of_value(SDValue::new(v.node, 0), 2)
}

/// `loadi8`: a load that leaves the upper bits undefined or zero.
pub fn loadi8<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    matches!(
        dag.load_ext_type(v.node),
        Some(LoadExt::NonExt | LoadExt::Ext | LoadExt::ZExt)
    )
}

fn loadi<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    align: u64,
) -> bool {
    match dag.load_ext_type(v.node) {
        Some(LoadExt::NonExt) => true,
        Some(LoadExt::Ext) if ENABLE_PROMOTE_ANYEXT_LOAD => dag.alignment(v.node) >= align && dag.is_simple(v.node),
        _ => false,
    }
}

pub fn loadi16<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    loadi(args, 2)
}

pub fn loadi32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    loadi(args, 4)
}

pub fn ext_load<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(LoadExt::Ext)
}

pub fn zext_load<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(LoadExt::ZExt)
}

pub fn sext_load<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(LoadExt::SExt)
}

pub fn extload_i64_i32<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(LoadExt::Ext)
        && (dag.memory_vt(v.node) == mvt::i32 || (dag.alignment(v.node) >= 4 && dag.is_simple(v.node)))
}

pub fn memop<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.has_sse_unaligned_mem() || dag.alignment(v.node) >= dag.memory_store_size(v.node)
}

pub fn simple_load<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.is_simple(v.node)
}

pub fn simple_store<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    simple_load(args)
}

pub fn aligned_16<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.alignment(v.node) >= 16
}

pub fn aligned_to_store_size<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.alignment(v.node) >= dag.memory_store_size(v.node)
}

pub fn atomic_non_extending<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    non_extending_load(args)
}

fn memory_vt_is<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    vt: Vt,
) -> bool {
    dag.memory_vt(v.node) == vt
}

pub fn memory_vt_i1<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::i1)
}

pub fn memory_vt_i8<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::i8)
}

pub fn memory_vt_i16<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::i16)
}

pub fn memory_vt_i32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::i32)
}

pub fn memory_vt_i64<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::i64)
}

pub fn memory_vt_f32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::f32)
}

pub fn memory_vt_f64<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::f64)
}

pub fn memory_vt_f80<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    memory_vt_is(args, mvt::f80)
}

/// `nontemporalstore`: the builder sets no `!nontemporal`, so none is.
pub fn non_temporal_store<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

/// `CommutableCMPCC`: a `cmpss`/`cmpsd` predicate whose operands may swap
/// (EQ, UNORD, NEQ, ORD).
pub fn commutable_cmp_cc<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    imm(args, |i| matches!(i & 0x7, 0 | 3 | 4 | 7))
}

/// `fp32imm0`: `N->isPosZero()`.
pub fn is_pos_zero<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    fp_imm_pos_zero(args)
}

fn fp_is<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    class: FpClass,
) -> bool {
    dag.fp_class(v) == Some(class)
}

pub fn fp_imm_pos_zero<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    fp_is(args, FpClass::PosZero)
}

pub fn fp_imm_one<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    fp_is(args, FpClass::One)
}

pub fn fp_imm_neg_zero<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    fp_is(args, FpClass::NegZero)
}

pub fn fp_imm_minus_one<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    fp_is(args, FpClass::MinusOne)
}

fn imm<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    test: impl Fn(i64) -> bool,
) -> bool {
    dag.constant(v).is_some_and(test)
}

pub fn imm_sext_32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    imm(args, |i| i32::try_from(i).is_ok())
}

pub fn imm_sext_8<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    imm(args, |i| i8::try_from(i).is_ok())
}

pub fn imm_zext_32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    imm(args, |i| u32::try_from(i).is_ok())
}

pub fn imm_ff00_ffff<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    imm(args, |i| (0xff00..=0xffff).contains(&i))
}

pub fn btc_bts_mask_64<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    imm(args, |i| i32::try_from(i).is_err() && (i as u64).is_power_of_two())
}

pub fn btr_mask_64<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    imm(args, |i| u32::try_from(i).is_err() && i32::try_from(i).is_err() && (!(i as u64)).is_power_of_two())
}

pub fn not_avoid_immediate_forms<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    !hooks.should_avoid_immediate_inst_forms_for_size(dag, v.node)
}

pub fn shift_mask_8<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 3)
}

pub fn shift_mask_16<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 4)
}

pub fn shift_mask_32<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 5)
}

pub fn shift_mask_64<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_unneeded_shift_mask(dag, v.node, 6)
}

pub fn reloc_imm_sext_32<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_sext_absolute_symbol_ref(dag, 32, v.node)
}

pub fn reloc_imm_sext_8<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_sext_absolute_symbol_ref(dag, 8, v.node)
}

/// `xor_flag_dead_result`: the value result of the `X86xor_flag` has no use,
/// only its flags (64-bit mode).
pub fn xor_flag_dead_result<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.has_n_uses_of_value(SDValue::new(v.node, 0), 0)
}

/// `X86add_flag_nocf` and its kin: no use of the flags result reads the carry.
pub fn no_carry_flag_uses_of_flags<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.has_no_carry_flag_uses(dag, SDValue::new(v.node, 1))
}

/// `X86sub_flag_nocf`: only when the result is used too, so `dec` is not a pure
/// compare.
pub fn sub_no_carry_flag_uses<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    !dag.has_n_uses_of_value(SDValue::new(v.node, 0), 0) && hooks.has_no_carry_flag_uses(dag, SDValue::new(v.node, 1))
}

/// `X86lock_add_nocf`: the flags are the node's first result.
pub fn no_carry_flag_uses_of_result<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.has_no_carry_flag_uses(dag, SDValue::new(v.node, 0))
}

pub fn def32<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.is_def32(dag, v.node)
}

pub fn or_disjoint<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.has_disjoint_flag(v.node) || dag.have_no_common_bits_set(dag.operand(v.node, 0), dag.operand(v.node, 1))
}

pub fn tcret_enough_regs<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    hooks.check_tc_ret_enough_regs(dag, v.node)
}

/// `anyext_sdiv`: the remainder result of a signed divide.
pub fn anyext_sdiv<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    let operand = dag.operand(v.node, 0);
    dag.opcode(operand.node) == isd::SDIVREM && operand.res == 1
}
