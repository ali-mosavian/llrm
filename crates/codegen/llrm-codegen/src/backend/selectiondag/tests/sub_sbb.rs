use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;

const SAMPLES: [u32; 8] = [0, 1, 2, 7, 8, 0x8000_0000, 0xDEAD_BEEF, 0xFFFF_FFFF];

/// The value of `v` when register 5 holds `x`; the flags of a `bt` are its bit.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    x: u32,
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), x);
    match dag.opcode(v) {
        ISD::Register => x,
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::And => arg(0) & arg(1),
        ISD::Srl => arg(0) >> arg(1),
        ISD::Sub => arg(0).wrapping_sub(arg(1)),
        x86isd::BT => (arg(0) >> arg(1)) & 1,
        x86isd::SETCC_CARRY => 0u32.wrapping_sub(arg(1)),
        other => panic!("unexpected {other:?}"),
    }
}

/// `0 - ((x >> 3) & 1)` was `shr; and; neg`: three instructions for what
/// `bt x, 3; sbb r, r` does in two with no mask (bench `crc`: the pin has
/// 7 `BT32ri8` + 7 `SBB32rr` where we had 7 `AND32ri` + 7 `NEG32r`).
#[test]
fn a_negated_shifted_bit_is_a_bit_test_and_sbb() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let by = dag.get_constant(3, EVT::I8);
    let shifted = dag.get_node(ISD::Srl, EVT::I32, &[x, by]);
    let one = dag.get_constant(1, EVT::I32);
    let bit = dag.get_node(ISD::And, EVT::I32, &[shifted, one]);
    let zero = dag.get_constant(0, EVT::I32);
    let root = dag.get_node(ISD::Sub, EVT::I32, &[zero, bit]);
    dag.set_root(root);
    let before: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut hooks = X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    let after: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    assert_eq!(before, after, "the fold changed the value");
    assert_eq!(dag.opcode(root), x86isd::SETCC_CARRY);
    assert_eq!(dag.opcode(dag.operand(root, 1)), x86isd::BT);
}
