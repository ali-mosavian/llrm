//! The bodies of the pin's node predicates (`CheckNodePredicate`) that the
//! AArch64 scalar table names, from `AArch64InstrInfo.td`,
//! `AArch64InstrFormats.td` and `TargetSelectionDAG.td`. llrm-dagisel's shared
//! ones are re-exported.
//!
//! DEVIATE SD500: the predicates below that answer false by name ([`atomic_store_hint`],
//! [`exclusive_load_intrinsic`], [`f16_immediate`], [`sve_increment`], [`sme_streaming_mode`]) guard
//! patterns of a feature or a node the scalar subset never builds (FEAT_PCDPHINT, an AArch64
//! intrinsic, `f16`, SVE, SME): false is the pin's answer for them on apple-m1 too. Exit: with that
//! feature.
//!
//! DEVIATE SD501: [`negated_immediate_cheaper`] (`isWorthNegatingImm`) and [`aligned_global_literal`]
//! (`alignedglobal`) are untranslated and answer false: `add x, imm` keeps the add, and a global is
//! addressed by `adrp` + `add`, not `ldr` literal. Exit: `isWorthNegatingImm` over the immediate
//! expansion; the global's alignment as a `Dag` query.

pub use crate::predicates::*;

use crate::dag::{Dag, Hooks, SDValue};
use crate::generated::aarch64::ids::reg;
use crate::generated::ids::{isd, mvt};

type Args<'a, D, H> = (&'a D, &'a H, SDValue);

pub fn atomic_store_hint<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

pub fn exclusive_load_intrinsic<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

pub fn f16_immediate<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

pub fn sve_increment<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

pub fn sme_streaming_mode<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

pub fn negated_immediate_cheaper<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

pub fn aligned_global_literal<D: Dag, H: Hooks<D>>(_: Args<D, H>) -> bool {
    false
}

/// `smullwithsignbits`: both operands of the multiply have more than 32 sign
/// bits, so a 64-bit multiply is one `SMULL`.
pub fn smull_with_sign_bits<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    (0..2).all(|at| dag.compute_num_sign_bits(dag.operand(v.node, at)) > 32)
}

/// `AtomicOrdering` numbers (LLVM's enum), as [`Dag::atomic_ordering`] returns them.
const MONOTONIC: u8 = 2;
const ACQUIRE: u8 = 4;
const RELEASE: u8 = 5;
const ACQ_REL: u8 = 6;
const SEQ_CST: u8 = 7;

fn ordering<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> u8 {
    dag.atomic_ordering(v.node).expect("an atomic predicate on a node that is not atomic")
}

/// `getMergedOrdering() == Acquire`.
pub fn atomic_acquire<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    ordering(args) == ACQUIRE
}

/// `getMergedOrdering() == Monotonic`.
pub fn atomic_monotonic<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    ordering(args) == MONOTONIC
}

/// `getMergedOrdering() == Release`.
pub fn atomic_release<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    ordering(args) == RELEASE
}

/// `getMergedOrdering() == AcquireRelease`.
pub fn atomic_acq_rel<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    ordering(args) == ACQ_REL
}

/// `getMergedOrdering() == SequentiallyConsistent`.
pub fn atomic_seq_cst<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    ordering(args) == SEQ_CST
}

/// `isReleaseOrStronger`: release, acq_rel or seq_cst.
pub fn atomic_release_or_stronger<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    ordering(args) >= RELEASE
}

/// `!isReleaseOrStronger`.
pub fn atomic_not_release_or_stronger<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    ordering(args) < RELEASE
}

/// `!isAcquireOrStronger`: acquire, acq_rel and seq_cst acquire.
pub fn atomic_not_acquire_or_stronger<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    !matches!(ordering(args), ACQUIRE | ACQ_REL | SEQ_CST)
}

/// `atomic_load_nonext`: `getExtensionType() == NON_EXTLOAD`.
pub fn atomic_load_nonext<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(crate::dag::LoadExt::NonExt)
}

/// `atomic_load_zext`: `getExtensionType() == ZEXTLOAD`.
pub fn atomic_load_zext<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(crate::dag::LoadExt::ZExt)
}

/// `atomic_load_aext`: `getExtensionType() == EXTLOAD`.
pub fn atomic_load_ext<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.load_ext_type(v.node) == Some(crate::dag::LoadExt::Ext)
}

/// `pre_store`: `getAddressingMode()` is `PRE_INC` or `PRE_DEC`.
pub fn pre_indexed<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.is_indexed(v.node) && !dag.is_post_indexed(v.node)
}

/// `post_store`: `getAddressingMode()` is `POST_INC` or `POST_DEC`.
pub fn post_indexed<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.is_post_indexed(v.node)
}

/// `fpimm32`: `getFP32Imm(Imm) != -1`.
pub fn fp_imm_fmov32<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    dag.fp_bits(v).is_some_and(|bits| hooks.fp_imm_code(bits, false).is_some())
}

/// `fpimm64`: `getFP64Imm(Imm) != -1`.
pub fn fp_imm_fmov64<D: Dag, H: Hooks<D>>((dag, hooks, v): Args<D, H>) -> bool {
    dag.fp_bits(v).is_some_and(|bits| hooks.fp_imm_code(bits, true).is_some())
}

/// `ConstantSDNode::getSExtValue`.
fn imm<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> i64 {
    dag.constant(v).expect("an immediate predicate on a constant")
}

macro_rules! imm_predicates {
    ($($name:ident => |$i:ident| $body:expr;)*) => {
        $(pub fn $name<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
            let $i = imm(args);
            $body
        })*
    };
}

imm_predicates! {
    imm_u64_lt_2 => |i| (i as u64) < 2;
    imm_u64_lt_4 => |i| (i as u64) < 4;
    imm_u64_lt_8 => |i| (i as u64) < 8;
    imm_u64_lt_16 => |i| (i as u64) < 16;
    imm_u64_lt_32 => |i| (i as u64) < 32;
    imm_u64_lt_64 => |i| (i as u64) < 64;
    imm_u64_lt_65536 => |i| (i as u64) < 65536;
    imm_u32_lt_8 => |i| (i as u32) < 8;
    imm_u32_lt_16 => |i| (i as u32) < 16;
    imm_u32_lt_32 => |i| (i as u32) < 32;
    imm_u32_lt_128 => |i| (i as u32) < 128;
    imm_u32_lt_256 => |i| (i as u32) < 256;
    imm_u32_lt_65536 => |i| (i as u32) < 65536;
    imm_u32_le_63 => |i| (i as u32) <= 63;
    imm_u32_1_to_16 => |i| (i as u32) > 0 && (i as u32) < 17;
    imm_u32_1_to_32 => |i| (i as u32) > 0 && (i as u32) < 33;
    imm_u32_1_to_64 => |i| (i as u32) > 0 && (i as u32) < 65;
    imm_u32_32_to_63 => |i| (i as u32) > 31 && (i as u32) < 64;
    imm_u8 => |i| (0..256).contains(&i);
    simm9 => |i| (-256..256).contains(&i);
    simm32 => |i| i >= i64::from(i32::MIN) && i <= i64::from(i32::MAX);
    zimm32 => |i| (i & 0xffff_ffff) == i;
}

/// `Op.getValueType() == vt && MaskedValueIsZero(Op, high `bits` bits set)`.
fn top_bits_zero<D: Dag, H: Hooks<D>>(
    (dag, _, v): Args<D, H>,
    vt: crate::matcher::Vt,
    width: u32,
    bits: u32,
) -> bool {
    let mask = (u64::MAX >> (64 - bits)) << (width - bits);
    dag.value_type(v) == vt && dag.masked_value_is_zero(v, mask)
}

pub fn top32_zero<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    top_bits_zero(args, mvt::i64, 64, 32)
}

pub fn topbits_all_zero64<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    top_bits_zero(args, mvt::i64, 64, 63)
}

pub fn topbits_all_zero32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    top_bits_zero(args, mvt::i32, 32, 31)
}

pub fn top16_zero<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    top_bits_zero(args, mvt::i32, 32, 16)
}

/// `copyFromSP`: a `CopyFromReg` of `SP`.
pub fn copy_from_sp<D: Dag, H: Hooks<D>>((dag, _, v): Args<D, H>) -> bool {
    dag.opcode(v.node) == isd::CopyFromReg && dag.register_of(dag.operand(v.node, 1).node) == Some(reg::SP)
}

pub fn logical_imm32<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    encode_logical_immediate(imm(args) as u32 as u64, 32).is_some()
}

pub fn logical_imm64<D: Dag, H: Hooks<D>>(args: Args<D, H>) -> bool {
    encode_logical_immediate(imm(args) as u64, 64).is_some()
}

/// `AArch64_AM::processLogicalImmediate`: the `N:immr:imms` field of a bitmask
/// immediate of a `reg_size`-bit register, or `None` where it has no encoding.
pub fn encode_logical_immediate(
    imm: u64,
    reg_size: u32,
) -> Option<u64> {
    if imm == 0 || imm == u64::MAX || (reg_size != 64 && (imm >> reg_size != 0 || imm == u64::from(u32::MAX))) {
        return None;
    }
    // The element size: the smallest power of two the pattern repeats at.
    let mut size = reg_size;
    loop {
        size /= 2;
        let mask = (1u64 << size) - 1;
        if (imm & mask) != ((imm >> size) & mask) {
            size *= 2;
            break;
        }
        if size <= 2 {
            break;
        }
    }
    let mask = u64::MAX >> (64 - size);
    let mut imm = imm & mask;
    let shifted_mask = |x: u64| x != 0 && ((x - 1) | x).wrapping_add(1) & ((x - 1) | x) == 0;
    let (rotation, ones) = if shifted_mask(imm) {
        let i = imm.trailing_zeros();
        (i, (imm >> i).trailing_ones())
    } else {
        imm |= !mask;
        if !shifted_mask(!imm) {
            return None;
        }
        let leading = imm.leading_ones();
        (64 - leading, leading + imm.trailing_ones() - (64 - size))
    };
    let imm_r = (size - rotation) & (size - 1);
    let n_imms = (!(size - 1) << 1) | (ones - 1);
    let n = ((n_imms >> 6) & 1) ^ 1;
    Some(u64::from((n << 12) | (imm_r << 6) | (n_imms & 0x3f)))
}

#[cfg(test)]
mod tests {
    use super::encode_logical_immediate as enc;

    /// Encodings the pin's `llvm-mc` gives: `and w0, w0, #0xff` is N=0,
    /// immr=0, imms=7; `#0x5555...` repeats at two bits.
    #[test]
    fn bitmask_immediates_encode_as_the_assembler_does() {
        assert_eq!(enc(0xff, 32), Some(0x007));
        assert_eq!(enc(0xffff_ffff_0000_0000, 64), Some(1 << 12 | 32 << 6 | 31));
        assert_eq!(enc(0x5555_5555, 32), Some(0x03c));
        assert_eq!(enc(0, 32), None);
        assert_eq!(enc(u64::from(u32::MAX), 32), None);
        assert_eq!(enc(0x1234_5678, 32), None, "not a rotated run of ones");
    }
}
