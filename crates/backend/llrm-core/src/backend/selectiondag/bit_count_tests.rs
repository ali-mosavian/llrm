//! The bit counts and `abs` the X86 lowering makes (`LowerCTLZ`, `LowerCTTZ`,
//! `LowerPARITY`, `LowerCTPOP`, `LowerABS`) and the generic promotions and
//! expansion behind them.

use super::legalize_dag::DagLegalizer;
use super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    dag.get_load(vt, chain, ptr, MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(1) })
}

/// Legalizes a DAG whose root is `op x` for a load `x`, and returns the new
/// root.
fn legalized(
    op: ISD,
    vt: EVT,
) -> (SelectionDAG, SDValue) {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let x = load(&mut dag, vt);
    let count = dag.get_node(op, vt, &[x]);
    dag.set_root(count);
    let tli = TargetLowering::x86_32(Features { cmov: true, ..Features::default() }, MVT::I32);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: true }).run().expect("legalizes");
    let root = dag.get_root();
    (dag, root)
}

/// Before the lowering existed these were "expand node" refusals: the shadow
/// stopped on every function that counts bits.
#[test]
fn cttz_is_bsf_with_a_cmov_for_zero() {
    let (dag, root) = legalized(ISD::CtTz, EVT::I32);
    assert_eq!(dag.opcode(root), x86isd::CMOV);
    let bsf = dag.operand(root, 0);
    assert_eq!(dag.opcode(bsf), x86isd::BSF);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(32));
    assert_eq!(dag.constant(dag.operand(root, 2)), Some(cond::E));
    assert_eq!(dag.operand(root, 3), SDValue { node: bsf.node, res: 1 });
}

#[test]
fn ctlz_is_bsr_cmov_then_xor_with_the_top_bit() {
    let (dag, root) = legalized(ISD::CtLz, EVT::I32);
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 1))), (ISD::Xor, Some(31)));
    let cmov = dag.operand(root, 0);
    assert_eq!((dag.opcode(cmov), dag.constant(dag.operand(cmov, 1))), (x86isd::CMOV, Some(63)));
    assert_eq!(dag.opcode(dag.operand(cmov, 0)), x86isd::BSR);
}

#[test]
fn ctlz_zero_poison_has_no_cmov() {
    let (dag, root) = legalized(ISD::CtLzZeroPoison, EVT::I32);
    assert_eq!(dag.opcode(root), ISD::Xor);
    assert_eq!(dag.opcode(dag.operand(root, 0)), x86isd::BSR);
}

#[test]
fn cttz_of_a_byte_is_promoted_with_a_sentinel_bit() {
    let (dag, root) = legalized(ISD::CtTz, EVT::I8);
    assert_eq!(dag.opcode(root), ISD::Truncate);
    let count = dag.operand(root, 0);
    assert_eq!(dag.opcode(count), ISD::CtTzZeroPoison);
    let or = dag.operand(count, 0);
    assert_eq!((dag.opcode(or), dag.constant(dag.operand(or, 1))), (ISD::Or, Some(256)));
}

#[test]
fn parity_of_an_int_is_the_not_parity_flag_of_the_xor_of_two_bytes() {
    let (dag, root) = legalized(ISD::Parity, EVT::I32);
    assert_eq!(dag.opcode(root), ISD::ZeroExtend);
    let setnp = dag.operand(root, 0);
    assert_eq!((dag.opcode(setnp), dag.constant(dag.operand(setnp, 0))), (x86isd::SETCC, Some(cond::NP)));
    assert_eq!(dag.opcode(dag.operand(setnp, 1)), x86isd::XOR);
}

#[test]
fn ctpop_of_an_int_is_the_parallel_count_ending_in_a_multiply_and_shift() {
    let (dag, root) = legalized(ISD::CtPop, EVT::I32);
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 1))), (ISD::Srl, Some(24)));
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Mul);
}

#[test]
fn abs_with_cmov_is_a_cmov_on_the_flags_of_zero_minus_x() {
    let (dag, root) = legalized(ISD::Abs, EVT::I32);
    assert_eq!((dag.opcode(root), dag.constant(dag.operand(root, 2))), (x86isd::CMOV, Some(cond::NS)));
    let neg = dag.operand(root, 1);
    assert_eq!(dag.opcode(neg), x86isd::SUB);
    assert_eq!(dag.operand(root, 3), SDValue { node: neg.node, res: 1 });
}
