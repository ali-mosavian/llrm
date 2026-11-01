//! `llvm.bitreverse`: a call to an unknown intrinsic, so `--isel=dag` panicked
//! at assemble.rs on `llvm.bitreverse.i32` (the old selector refuses it too).
//! It is `expandBITREVERSE`: a byte swap and three mask-and-swap steps.

use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;
use crate::testing::m16_ir;

fn module(bits: u32) -> String {
    m16_ir(&format!(
        "target datalayout = \"@m16-datalayout@-ni:2:3\"
declare i{bits} @llvm.bitreverse.i{bits}(i{bits})
define i{bits} @rev(i{bits} %x) {{
  %r = call i{bits} @llvm.bitreverse.i{bits}(i{bits} %x)
  ret i{bits} %r
}}
"
    ))
    .to_owned()
}

/// Every width the targets hold selects, at m16 and m32.
#[test]
fn bitreverse_selects_at_every_width() {
    for bits in [8, 16, 32, 64] {
        let module = llrm_mir::parse::module(&module(bits)).expect("parses");
        let done = crate::backend::selectiondag::dag_function::selected(
            &module,
            "rev",
            &llrm_x86_m16::M16,
            crate::backend::cpu::profile("486").expect("a cpu"),
            &[],
        )
        .unwrap_or_else(|why| panic!("i{bits}: {}", why.0));
        assert!(!done.body.insns().is_empty(), "i{bits}");
    }
}

/// The value of `v` when register 5 holds `x`, over the nodes the expansion
/// makes.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    x: u128,
) -> u128 {
    let bits = dag.value_type(v).size_in_bits();
    let mask = super::super::fold::mask(bits);
    let arg = |i: usize| eval(dag, dag.operand(v, i), x);
    let r = match dag.opcode(v) {
        ISD::Register => x,
        ISD::Constant => dag.constant(v).unwrap(),
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Shl => arg(0) << arg(1),
        ISD::Srl => (arg(0) & mask) >> arg(1),
        ISD::Rotl => {
            let (v, by) = (arg(0) & mask, arg(1) as u32 % bits);
            (v << by) | (v >> (bits - by) % bits)
        }
        ISD::BSwap => (arg(0) & mask).swap_bytes() >> (128 - bits),
        other => panic!("unexpected {other:?}"),
    };
    r & mask
}

/// The expansion gives the reversed bits for every width.
#[test]
fn the_expansion_reverses_the_bits() {
    for bits in [8u32, 16, 32] {
        let vt = EVT::integer(bits);
        let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
        let mut dag = dag();
        let x = dag.get_register(vt, 5);
        let node = dag.get_node(ISD::BitReverse, vt, &[x]);
        dag.set_root(node);
        DagLegalizer::new(
            &mut dag,
            &tli,
            &mut X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None },
        )
        .run()
        .expect("legalizes");
        let root = dag.get_root();
        let m = super::super::fold::mask(bits);
        for x in [0u128, 1, 2, 0x80, 0xA5, 0x1234, 0xDEAD_BEEF, m, m / 3, 12345 & m] {
            let x = x & m;
            assert_eq!(eval(&dag, root, x), x.reverse_bits() >> (128 - bits), "i{bits} x={x:#x}");
        }
    }
}
