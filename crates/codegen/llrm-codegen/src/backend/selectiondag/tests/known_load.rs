//! An extending load's high bits were unknown: `sext a * sext b` (both loaded
//! as `SEXTLOAD i32`) kept the cross products of the double-word multiply
//! (smulh: 20 instructions where the pin has `imul`).

use super::super::actions::LoadExt;
use super::super::known_bits::{known_bits, max_significant_bits, num_sign_bits};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn extending(
    dag: &mut SelectionDAG,
    ext: LoadExt,
) -> SDValue {
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info = MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Stack(0),
        ext: Some(ext),
        space: 0,
        ordering: None,
    };
    dag.get_ext_load(ext, EVT::I64, chain, ptr, info)
}

#[test]
fn a_sign_extending_load_has_the_sign_bits_of_its_extension() {
    let mut dag = dag();
    let load = extending(&mut dag, LoadExt::SExtLoad);
    assert_eq!((num_sign_bits(&dag, load), max_significant_bits(&dag, load)), (33, 32));
}

#[test]
fn a_zero_extending_load_has_zero_high_bits() {
    let mut dag = dag();
    let load = extending(&mut dag, LoadExt::ZExtLoad);
    assert_eq!(known_bits(&dag, load).zero, 0xFFFF_FFFFu128 << 32);
}

/// `ComputeNumSignBits` knew nothing of `sra`: `(x >> 32) * (y >> 32)` on `i64`
/// kept a full multiply where the pin's `smullwithsignbits` has an `SMULL`.
#[test]
fn an_arithmetic_shift_right_by_a_constant_adds_its_amount_to_the_sign_bits() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I64, 7);
    let amount = dag.get_constant(32, EVT::I64);
    let shifted = dag.get_node(ISD::Sra, EVT::I64, &[x, amount]);
    assert_eq!(num_sign_bits(&dag, x), 1, "premise: a register says nothing");
    assert_eq!(num_sign_bits(&dag, shifted), 33);
    let wide = dag.get_constant(100, EVT::I64);
    let past = dag.get_node(ISD::Sra, EVT::I64, &[x, wide]);
    assert_eq!(num_sign_bits(&dag, past), 1, "an amount past the width says nothing");
}

fn masked(
    dag: &mut SelectionDAG,
    reg: u32,
    mask: u128,
) -> SDValue {
    let x = dag.get_register(EVT::I32, reg);
    let mask = dag.get_constant(mask, EVT::I32);
    dag.get_node(ISD::And, EVT::I32, &[x, mask])
}

fn leading_zeros(
    dag: &SelectionDAG,
    v: SDValue,
) -> u32 {
    let zero = known_bits(dag, v).zero;
    (0..32).take_while(|i| zero & (1u128 << (31 - i)) != 0).count() as u32
}

/// `computeKnownBits` knew nothing of the pin's arithmetic cases, so the
/// divide-by-constant pricer saw a 32-bit dividend where it is 8 bits
/// (`a & 0xFF` and `b & 0xF` gave 0 leading zeros for every node here).
#[test]
fn arithmetic_nodes_bound_the_bits_of_narrow_operands() {
    let mut dag = dag();
    let a = masked(&mut dag, 7, 0xFF);
    let b = masked(&mut dag, 8, 0xF);
    for (op, want) in [
        (ISD::Mul, 20),
        (ISD::UDiv, 24),
        (ISD::URem, 28),
        (ISD::MulHU, 32),
        (ISD::SRem, 24),
        (ISD::Shl, 9),
        (ISD::Srl, 24),
        (ISD::Sra, 24),
    ] {
        let v = dag.get_node(op, EVT::I32, &[a, b]);
        let got = leading_zeros(&dag, v);
        assert!(got >= want, "{op:?}: {got} leading zeros, want {want}");
    }
}

/// A shift by an amount that may be zero or past the width keeps nothing of
/// the operand's known bits: `(a & 0xFF) << b` and `(a & 0xFF) >> b` with `b`
/// unknown must not claim the operand's zero high bits survive a left shift.
#[test]
fn a_shift_by_an_unknown_amount_left_keeps_no_known_zero_bits() {
    let mut dag = dag();
    let a = masked(&mut dag, 7, 0xFF);
    let b = dag.get_register(EVT::I32, 8);
    let shl = dag.get_node(ISD::Shl, EVT::I32, &[a, b]);
    assert_eq!(known_bits(&dag, shl).zero, 0);
    let sra = dag.get_node(ISD::Sra, EVT::I32, &[b, a]);
    assert_eq!(known_bits(&dag, sra), super::super::known_bits::KnownBits::unknown(32));
}

/// The sign bits and known bits of a vector were counted over its whole
/// width (`v2i64` as 128 bits) and a `BUILD_VECTOR` knew nothing: the
/// multiply of `sext <2 x i32>` by a splat of a 16-bit value kept a scalarized
/// `v2i64` multiply where the pin has `smull`.
#[test]
fn the_sign_bits_of_a_vector_are_those_of_its_lanes() {
    let v2i64 = EVT::Simple(crate::backend::selectiondag::MVT::V2I64);
    let v2i32 = EVT::Simple(crate::backend::selectiondag::MVT::V2I32);
    let mut dag = dag();
    let x = dag.get_register(v2i32, 9);
    let extended = dag.get_node(ISD::SignExtend, v2i64, &[x]);
    assert_eq!(num_sign_bits(&dag, x), 1, "premise: a register says nothing");
    assert_eq!(num_sign_bits(&dag, extended), 33);
    let scalar = dag.get_register(EVT::I64, 7);
    let from = dag.get_value_type_node(EVT::I16);
    let narrow = dag.get_node(ISD::SignExtendInReg, EVT::I64, &[scalar, from]);
    let splat = dag.get_build_vector(v2i64, &[narrow, narrow]);
    assert_eq!(num_sign_bits(&dag, splat), 49);
    let mixed = dag.get_build_vector(v2i64, &[narrow, scalar]);
    assert_eq!(num_sign_bits(&dag, mixed), 1);
}

#[test]
fn the_known_bits_of_a_vector_are_those_of_its_lanes() {
    let v2i64 = EVT::Simple(crate::backend::selectiondag::MVT::V2I64);
    let v2i32 = EVT::Simple(crate::backend::selectiondag::MVT::V2I32);
    let mut dag = dag();
    let x = dag.get_register(v2i32, 9);
    let extended = dag.get_node(ISD::ZeroExtend, v2i64, &[x]);
    assert_eq!(known_bits(&dag, extended).zero, 0xFFFF_FFFFu128 << 32);
    let low = dag.get_constant(0xFFFF, EVT::I64);
    let splat = dag.get_build_vector(v2i64, &[low, low]);
    assert_eq!(known_bits(&dag, splat).zero, !0xFFFFu128 & u128::from(u64::MAX));
}
