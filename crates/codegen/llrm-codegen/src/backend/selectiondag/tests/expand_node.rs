use super::super::legalize_dag::{DagLegalizer, NoHooks};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: vt,
        align: 1,
        volatile: false,
        base: MemBase::Stack(slot),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.get_load(vt, chain, ptr, info)
}

fn p5() -> TargetLowering {
    TargetLowering::x86_32(Features::default(), MVT::I32)
}

/// `x / y` and `x % y` each expand to the two-result divide; common
/// subexpression elimination then makes it one `idiv`. Without the
/// two-result form a program that needs both divides twice.
#[test]
fn a_divide_and_a_remainder_share_one_divrem() {
    let mut dag = dag();
    let (x, y) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let (quotient, remainder) =
        (dag.get_node(ISD::SDiv, EVT::I32, &[x, y]), dag.get_node(ISD::SRem, EVT::I32, &[x, y]));
    let both = dag.get_node(ISD::Add, EVT::I32, &[quotient, remainder]);
    dag.set_root(both);
    DagLegalizer::new(&mut dag, &p5(), &mut NoHooks).run().unwrap();
    let root = dag.get_root();
    let (q, r) = (dag.operand(root, 0), dag.operand(root, 1));
    assert_eq!((dag.opcode(q), q.res, dag.opcode(r), r.res), (ISD::SDivRem, 0, ISD::SDivRem, 1));
    assert_eq!(q.node, r.node, "one divide");
}

/// `mulhu` is the high result of the two-result multiply.
#[test]
fn mulh_is_the_high_half_of_lohi() {
    let mut dag = dag();
    let (x, y) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let high = dag.get_node(ISD::MulHU, EVT::I32, &[x, y]);
    dag.set_root(high);
    DagLegalizer::new(&mut dag, &p5(), &mut NoHooks).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), root.res), (ISD::UMulLoHi, 1));
}

/// With no two-result divide but a divide, `x % y` is `x - (x / y) * y`.
#[test]
fn a_remainder_without_divrem_is_x_minus_quotient_times_y() {
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::URem, MVT::I32, Action::Expand);
    tli.set_operation_action(ISD::UDivRem, MVT::I32, Action::Expand);
    let mut dag = dag();
    let (x, y) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let rem = dag.get_node(ISD::URem, EVT::I32, &[x, y]);
    dag.set_root(rem);
    DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Sub);
    let product = dag.operand(root, 1);
    assert_eq!(dag.opcode(product), ISD::Mul);
    assert_eq!(dag.opcode(dag.operand(product, 0)), ISD::UDiv);
}

/// A word `bswap` is a rotate by 8 (the `i8` shift amount); an `i32` one is
/// legal, so left alone.
#[test]
fn a_word_bswap_is_a_rotate_by_eight() {
    let mut dag = dag();
    let w = load(&mut dag, EVT::I16, 1);
    let swapped = dag.get_node(ISD::BSwap, EVT::I16, &[w]);
    dag.set_root(swapped);
    DagLegalizer::new(&mut dag, &p5(), &mut NoHooks).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Rotl);
    let amount = dag.operand(root, 1);
    assert_eq!((dag.constant(amount), dag.value_type(amount)), (Some(8), EVT::I8));
}

/// At m16 an `i32` is a register type, so a pair of `i16` halves made one (a
/// far call's `ax:dx`) reached selection as `build_pair`, which nothing selects
/// (4 execution programs).
#[test]
fn a_build_pair_is_the_low_half_or_the_shifted_high_half() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = dag();
    let (lo, hi) = (load(&mut dag, EVT::I16, 1), load(&mut dag, EVT::I16, 2));
    let pair = dag.get_node(ISD::BuildPair, EVT::I32, &[lo, hi]);
    dag.set_root(pair);
    DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Or);
    let (low, high) = (dag.operand(root, 0), dag.operand(root, 1));
    assert_eq!((dag.opcode(low), dag.operand(low, 0)), (ISD::ZeroExtend, lo));
    assert_eq!((dag.opcode(high), dag.constant(dag.operand(high, 1))), (ISD::Shl, Some(16)));
}
