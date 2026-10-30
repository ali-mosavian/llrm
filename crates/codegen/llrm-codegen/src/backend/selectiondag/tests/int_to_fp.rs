use super::super::legalize_dag::DagLegalizer;
use super::super::legalize_types::TypeLegalizer;
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;

fn p5() -> TargetLowering {
    TargetLowering::x86_32(Features::default(), MVT::I32)
}

fn double() -> EVT {
    EVT::Simple(MVT::F64)
}

fn legalized(
    op: ISD,
    value: impl FnOnce(&mut SelectionDAG) -> SDValue,
) -> (SelectionDAG, SDValue) {
    let tli = p5();
    let mut dag = dag();
    let x = value(&mut dag);
    let converted = dag.get_node(op, double(), &[x]);
    dag.set_root(converted);
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false }).run().unwrap();
    let root = dag.get_root();
    (dag, root)
}

fn mem_of(
    dag: &SelectionDAG,
    node: SDValue,
) -> EVT {
    let NodeData::Mem(info) = dag.node(node).data else { panic!("a memory node") };
    info.mem_vt
}

/// `(double)i` without SSE: store the integer to a slot of its size and
/// `fild` it (the shadow's `SIntToFp` refusals).
#[test]
fn a_signed_conversion_is_a_store_and_a_fild() {
    for src in [EVT::I16, EVT::I32] {
        let (dag, root) = legalized(ISD::SIntToFp, |dag| dag.get_register(src, 5));
        assert_eq!((dag.opcode(root), dag.value_type(root), mem_of(&dag, root)), (x86isd::FILD, double(), src));
        let store = SDValue { node: dag.operand(root, 0).node, res: 0 };
        assert_eq!((dag.opcode(store), mem_of(&dag, store)), (ISD::Store, src));
    }
}

/// A byte is widened first (`i8` is Promote): the conversion reads a word.
#[test]
fn a_byte_conversion_is_widened_to_a_word() {
    let (dag, root) = legalized(ISD::SIntToFp, |dag| dag.get_register(EVT::I8, 5));
    assert_eq!((dag.opcode(root), mem_of(&dag, root)), (x86isd::FILD, EVT::I16));
}

/// An unsigned `i32` is stored with a zero high dword and `fild`ed as an `i64`.
#[test]
fn an_unsigned_dword_is_stored_with_a_zero_high_dword_and_filded_as_i64() {
    let (dag, root) = legalized(ISD::UIntToFp, |dag| dag.get_register(EVT::I32, 5));
    assert_eq!((dag.opcode(root), mem_of(&dag, root)), (x86isd::FILD, EVT::I64));
    let high = SDValue { node: dag.operand(root, 0).node, res: 0 };
    assert_eq!(dag.opcode(high), ISD::Store);
    assert!(dag.is_null_constant(dag.operand(high, 1)), "the high dword is zero");
    let address = dag.operand(high, 2);
    assert_eq!((dag.opcode(address), dag.constant(dag.operand(address, 1))), (ISD::Add, Some(4)), "four bytes up");
    let low = SDValue { node: dag.operand(high, 0).node, res: 0 };
    assert_eq!(dag.opcode(low), ISD::Store);
    assert_eq!(dag.opcode(dag.operand(low, 1)), ISD::Register, "the dword itself");
}

/// An unsigned `i64` is `fild`ed signed and 2^64 added from the pool when
/// the sign was set. The type legalizer asks the target first (the operand is
/// an `i64`), splits the store, and reads the sign from the high dword.
#[test]
fn an_unsigned_i64_adds_the_fudge_when_the_sign_was_set() {
    let tli = p5();
    let mut dag = dag();
    let (lo, hi) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi]);
    let converted = dag.get_node(ISD::UIntToFp, double(), &[x]);
    dag.set_root(converted);
    let mut hooks = X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false };
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut hooks);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::FpRound, "rounded from the x87 sum to a double");
    let sum = dag.operand(root, 0);
    assert_eq!(dag.opcode(sum), ISD::FAdd);
    let fild = dag.operand(sum, 0);
    assert_eq!((dag.opcode(fild), mem_of(&dag, fild)), (x86isd::FILD, EVT::I64));
    let joined = SDValue { node: dag.operand(fild, 0).node, res: 0 };
    assert_eq!(dag.opcode(joined), ISD::TokenFactor, "the two dword stores");
    let fudge = dag.operand(sum, 1);
    assert_eq!((dag.opcode(fudge), mem_of(&dag, fudge)), (ISD::Load, EVT::Simple(MVT::F32)));
    assert_eq!(dag.constant_pool, vec![(0x5F80_0000_0000_0000, EVT::I64)]);
    // The sign is that of the high dword: `hi < 0`.
    let sign = (0..dag.node_count() as u32)
        .map(|node| SDValue { node, res: 0 })
        .find(|&v| dag.opcode(v) == ISD::SetCC && dag.users(v).next().is_some())
        .expect("a compare");
    assert_eq!(dag.operand(sign, 0), hi);
    assert_eq!(dag.node(dag.operand(sign, 2)).data, NodeData::CondCode(CondCode::SetLt));
}
