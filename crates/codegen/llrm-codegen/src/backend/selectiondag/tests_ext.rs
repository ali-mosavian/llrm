//! Extensions and truncates in the type legalizer.

use super::legalize_types::TypeLegalizer;
use super::*;

fn dag() -> SelectionDAG {
    SelectionDAG::new(DagConfig::default())
}

use crate::backend::selectiondag::{MemBase, MemInfo};

fn p5() -> TargetLowering {
    TargetLowering::x86_32(Features::default(), MVT::I32)
}

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: vt, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0, ordering: None };
    dag.get_load(vt, chain, ptr, info)
}

/// `x & 1` as an `i1` (a truncate to a type no register holds) feeds a
/// select: 5 shadow functions stopped at "Truncate: promote result". After:
/// a truncate to `i8` (the promoted type) and nothing illegal reachable.
#[test]
fn a_truncate_to_i1_is_promoted_to_a_truncate_to_i8() {
    let tli = p5();
    let mut dag = dag();
    let (x, a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2), load(&mut dag, EVT::I32, 3));
    let bit = dag.get_node(ISD::Truncate, EVT::I1, &[x]);
    let pick = dag.get_select(EVT::I32, bit, a, b);
    dag.set_root(pick);
    let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
    legalizer.run().unwrap();
    assert!(legalizer.all_legal());
    let narrow = (0..dag.node_count() as u32)
        .map(|node| SDValue { node, res: 0 })
        .find(|&v| dag.opcode(v) == ISD::Truncate && dag.value_type(v) == EVT::I8);
    assert_eq!(narrow.map(|v| dag.operand(v, 0)), Some(x));
}

/// An `i64` extension of a dword: the low half is the dword, the high is
/// zero, its sign (`sra 31`), or undefined.
#[test]
fn extending_a_dword_to_i64_gives_the_halves() {
    let halves = |op: ISD| {
        let tli = p5();
        let mut dag = dag();
        let x = load(&mut dag, EVT::I32, 1);
        let wide = dag.get_node(op, EVT::I64, &[x]);
        let low = dag.get_node(ISD::Truncate, EVT::I32, &[wide]);
        let by = dag.get_constant(32, EVT::I8);
        let upper = dag.get_node(ISD::Srl, EVT::I64, &[wide, by]);
        let high = dag.get_node(ISD::Truncate, EVT::I32, &[upper]);
        let both = dag.get_node(ISD::Xor, EVT::I32, &[low, high]);
        dag.set_root(both);
        TypeLegalizer::new(&mut dag, &tli).run().unwrap();
        let root = dag.get_root();
        let (lo, hi) = (dag.operand(root, 0), dag.operand(root, 1));
        (dag, x, lo, hi)
    };
    let (dag, x, lo, hi) = halves(ISD::ZeroExtend);
    assert_eq!(lo, x);
    assert!(dag.is_null_constant(hi));
    let (dag, x, lo, hi) = halves(ISD::SignExtend);
    assert_eq!(lo, x);
    assert_eq!(dag.opcode(hi), ISD::Sra);
    assert_eq!((dag.operand(hi, 0), dag.constant(dag.operand(hi, 1))), (x, Some(31)));
    let (dag, x, lo, hi) = halves(ISD::AnyExtend);
    assert_eq!(lo, x);
    assert!(dag.is_undef(hi));
}

/// A byte extended to `i64` extends it to a dword for the low half.
#[test]
fn extending_a_byte_to_i64_extends_to_a_dword_for_the_low_half() {
    let tli = p5();
    let mut dag = dag();
    let x = load(&mut dag, EVT::I8, 1);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I64, &[x]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[wide]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root), dag.operand(root, 0)), (ISD::ZeroExtend, EVT::I32, x));
}

/// `all_legal` grew its visited set from empty, rehashing about nine times on
/// a 600-node DAG (2,234 `reserve_rehash` calls on d_faces). Sized from the
/// node count it allocates the set once.
#[test]
fn all_legal_sizes_its_visited_set_once() {
    let tli = p5();
    let mut dag = dag();
    let mut level: Vec<SDValue> = (0..256).map(|slot| load(&mut dag, EVT::I32, slot)).collect();
    while level.len() > 1 {
        level = level.chunks(2).map(|pair| dag.get_node(ISD::Add, EVT::I32, &[pair[0], pair[1]])).collect();
    }
    dag.set_root(level[0]);
    assert!(dag.node_count() > 500, "premise: a DAG of several hundred nodes");
    let legalizer = TypeLegalizer::new(&mut dag, &tli);
    let (legal, made) = llrm_support::alloc_count::counted(|| legalizer.all_legal());
    assert!(legal, "premise: nothing illegal is reachable");
    assert!(made <= 6, "{made} allocations to visit 500 nodes");
}
