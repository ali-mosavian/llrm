use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn dword(
    dag: &mut SelectionDAG,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(slot) };
    dag.get_load(EVT::I32, chain, ptr, info)
}

/// `x * y` at 32 bits: the widening multiply of the low dwords, and the
/// cross products `ll*rh` and `lh*rl` added into the high dword. Taking
/// only the low product gives the wrong high half.
#[test]
fn an_i64_multiply_is_a_widening_multiply_and_two_cross_products() {
    let mut dag = dag();
    let (ll, lh, rl, rh) = (dword(&mut dag, 1), dword(&mut dag, 2), dword(&mut dag, 3), dword(&mut dag, 4));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[ll, lh]);
    let y = dag.get_node(ISD::BuildPair, EVT::I64, &[rl, rh]);
    let product = dag.get_node(ISD::Mul, EVT::I64, &[x, y]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[product]);
    let shift = dag.get_constant(32, EVT::I8);
    let upper = dag.get_node(ISD::Srl, EVT::I64, &[product, shift]);
    let high = dag.get_node(ISD::Truncate, EVT::I32, &[upper]);
    let both = dag.get_node(ISD::Xor, EVT::I32, &[low, high]);
    dag.set_root(both);
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    let (lo, hi) = (dag.operand(root, 0), dag.operand(root, 1));
    assert_eq!((dag.opcode(lo), lo.res), (ISD::UMulLoHi, 0));
    assert_eq!((dag.operand(lo, 0), dag.operand(lo, 1)), (ll, rl));
    // hi = (umul_lohi.1 + ll*rh) + lh*rl
    assert_eq!(dag.opcode(hi), ISD::Add);
    let (inner, cross_left) = (dag.operand(hi, 0), dag.operand(hi, 1));
    assert_eq!((dag.opcode(cross_left), dag.operand(cross_left, 0), dag.operand(cross_left, 1)), (ISD::Mul, lh, rl));
    assert_eq!(dag.opcode(inner), ISD::Add);
    assert_eq!(dag.operand(inner, 0), SDValue { node: lo.node, res: 1 });
    let cross_right = dag.operand(inner, 1);
    assert_eq!((dag.opcode(cross_right), dag.operand(cross_right, 0), dag.operand(cross_right, 1)), (ISD::Mul, ll, rh));
}
