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
