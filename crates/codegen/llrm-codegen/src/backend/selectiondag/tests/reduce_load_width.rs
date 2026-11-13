//! `(x >> 8) & 255` of a loaded word stayed `load; shr; movzx` (x_radix, +2
//! instructions); it is one byte `zextload` at `p + 1`.

use super::super::actions::LoadExt;
use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    combined_with(false, build)
}

fn combined_with(
    sext_load: bool,
    build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue,
) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info = MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Stack(0),
        ext: None,
        space: 0,
        ordering: None,
    };
    let load = dag.get_load(EVT::I32, chain, ptr, info);
    let root = build(&mut dag, load);
    dag.set_root(root);
    let mut hooks = X86Lowering {
        multiply_early_out: None,
        share_products: false,
        imul_if_smaller: false,
        hot_chains: false,
        cmov: true,
        sse2: false,
        min_size: false,
        three_address_adds: false,
        i16_mul_chains: false,
        sext_load,
        zext_free: false,
        branch_constant_selects: false,
        profile: None,
    };
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root)
}

#[test]
fn a_masked_shifted_load_is_a_byte_zextload_at_the_offset() {
    let (dag, root) = combined(|dag, load| {
        let (eight, mask) = (dag.get_constant(8, EVT::I32), dag.get_constant(255, EVT::I32));
        let shr = dag.get_node(ISD::Srl, EVT::I32, &[load, eight]);
        dag.get_node(ISD::And, EVT::I32, &[shr, mask])
    });
    assert_eq!(dag.opcode(root), ISD::Load, "{root:?}");
    let NodeData::Mem(info) = &dag.node(root).data else { panic!() };
    assert_eq!((info.mem_vt, info.base), (EVT::I8, MemBase::Stack(1)));
}

#[test]
fn a_masked_load_is_a_byte_zextload() {
    let (dag, root) = combined(|dag, load| {
        let mask = dag.get_constant(255, EVT::I32);
        dag.get_node(ISD::And, EVT::I32, &[load, mask])
    });
    let NodeData::Mem(info) = &dag.node(root).data else { panic!("{:?}", dag.opcode(root)) };
    assert_eq!((info.mem_vt, info.base), (EVT::I8, MemBase::Stack(0)));
}

/// m16 `f_lshr_16`'s high half: a dword load shifted right by 16 is a word
/// `zextload` at `p + 2`.
#[test]
fn a_dword_load_shifted_by_sixteen_is_a_word_zextload_at_plus_two() {
    let (dag, root) = combined(|dag, load| {
        let sixteen = dag.get_constant(16, EVT::I32);
        dag.get_node(ISD::Srl, EVT::I32, &[load, sixteen])
    });
    let NodeData::Mem(info) = &dag.node(root).data else { panic!("{:?}", dag.opcode(root)) };
    assert_eq!((info.mem_vt, info.base, info.ext), (EVT::I16, MemBase::Stack(2), Some(LoadExt::ZExtLoad)));
}

/// m64 `bitfield_overhang_init`: `(x << 24) >> 24` of a dword load stayed
/// `mov; movsx eax, al; movsxd` (34 vs the pin's 28 instructions); it is one
/// byte `sextload`.
#[test]
fn a_sign_extended_low_byte_of_a_dword_load_is_a_byte_sextload() {
    let (dag, root) = combined_with(true, |dag, load| {
        let c = dag.get_constant(24, EVT::I32);
        let shl = dag.get_node(ISD::Shl, EVT::I32, &[load, c]);
        let sra = dag.get_node(ISD::Sra, EVT::I32, &[shl, c]);
        sra
    });
    let NodeData::Mem(info) = &dag.node(root).data else { panic!("{:?}", dag.opcode(root)) };
    assert_eq!((info.mem_vt, info.base, info.ext), (EVT::I8, MemBase::Stack(0), Some(LoadExt::SExtLoad)));
}

#[test]
fn a_sign_extended_low_byte_of_a_dword_load_reaches_a_quad_as_a_byte_sextload() {
    let (dag, root) = combined_with(true, |dag, load| {
        let c = dag.get_constant(24, EVT::I32);
        let shl = dag.get_node(ISD::Shl, EVT::I32, &[load, c]);
        let sra = dag.get_node(ISD::Sra, EVT::I32, &[shl, c]);
        dag.get_node(ISD::SignExtend, EVT::I64, &[sra])
    });
    let NodeData::Mem(info) = &dag.node(root).data else { panic!("{:?}", dag.opcode(root)) };
    assert_eq!((info.mem_vt, info.ext), (EVT::I8, Some(LoadExt::SExtLoad)));
}

/// 6502 `a - *p` of a zero-extended byte: the low byte of the word sum read
/// `trunc (zextload i8)`, which no byte op takes as its memory operand
/// (`lda (p); sta t; sbc t`, 17 B for `sec; lda a; sbc (p)`, 13 B). It is the
/// byte load, as the pin's `reduceLoadWidth` makes it.
#[test]
fn a_truncated_byte_zextload_is_the_byte_load() {
    let (dag, root) = combined(|dag, _| {
        let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
        let info = MemInfo {
            mem_vt: EVT::I8,
            align: 1,
            volatile: false,
            base: MemBase::Stack(0),
            ext: None,
            space: 0,
            ordering: None,
        };
        let load = dag.get_ext_load(LoadExt::ZExtLoad, EVT::I32, chain, ptr, info);
        dag.get_node(ISD::Truncate, EVT::I8, &[load])
    });
    let NodeData::Mem(info) = &dag.node(root).data else { panic!("{:?}", dag.opcode(root)) };
    assert_eq!((dag.opcode(root), info.mem_vt, info.ext), (ISD::Load, EVT::I8, None));
}
