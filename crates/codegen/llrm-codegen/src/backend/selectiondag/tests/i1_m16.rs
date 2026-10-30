use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn m16_tli() -> TargetLowering {
    TargetLowering::x86_16(Features::default(), MVT::I16)
}

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I16, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0 , ordering: None };
    dag.get_load(vt, chain, ptr, info)
}

/// Every `i1` shape the builder emits, at m16: each must leave nothing
/// illegal reachable.
#[test]
fn i1_values_are_promoted_at_m16() {
    let tli = m16_tli();
    let shapes: [(&str, fn(&mut SelectionDAG) -> SDValue); 7] = [
        ("select on a compare", |dag| {
            let (a, b) = (load(dag, EVT::I16, 1), load(dag, EVT::I16, 2));
            let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
            dag.get_select(EVT::I16, test, a, b)
        }),
        ("select on a constant", |dag| {
            let (a, b) = (load(dag, EVT::I16, 1), load(dag, EVT::I16, 2));
            let test = dag.get_bool_constant(true, EVT::I1);
            dag.get_select(EVT::I16, test, a, b)
        }),
        ("xor of a compare", |dag| {
            let (a, b) = (load(dag, EVT::I16, 1), load(dag, EVT::I16, 2));
            let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetEq);
            let one = dag.get_bool_constant(true, EVT::I1);
            let flipped = dag.get_node(ISD::Xor, EVT::I1, &[test, one]);
            dag.get_node(ISD::ZeroExtend, EVT::I16, &[flipped])
        }),
        ("truncate to i1", |dag| {
            let a = load(dag, EVT::I16, 1);
            let bit = dag.get_node(ISD::Truncate, EVT::I1, &[a]);
            dag.get_node(ISD::ZeroExtend, EVT::I16, &[bit])
        }),
        ("any-extend of an i1", |dag| {
            let (a, b) = (load(dag, EVT::I16, 1), load(dag, EVT::I16, 2));
            let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetULt);
            dag.get_node(ISD::AnyExtend, EVT::I16, &[test])
        }),
        ("sign-extend of an i1", |dag| {
            let (a, b) = (load(dag, EVT::I16, 1), load(dag, EVT::I16, 2));
            let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetLe);
            dag.get_node(ISD::SignExtend, EVT::I16, &[test])
        }),
        ("i1 through memory", |dag| {
            let bit = load(dag, EVT::I1, 1);
            dag.get_node(ISD::ZeroExtend, EVT::I16, &[bit])
        }),
    ];
    let mut failures = Vec::new();
    for (name, build) in shapes {
        let mut dag = dag();
        let root = build(&mut dag);
        dag.set_root(root);
        let mut legalizer = TypeLegalizer::new(&mut dag, &tli);
        let done = legalizer.run();
        if !(done.is_ok() && legalizer.all_legal()) {
            failures.push(format!("{name}: {done:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `br (icmp slt a, b)` at m16 reads an `i8` condition, as at m32.
#[test]
fn a_branch_on_an_i1_compare_reads_an_i8_condition_at_m16() {
    let tli = m16_tli();
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::I16, 1), load(&mut dag, EVT::I16, 2));
    let test = dag.get_setcc(EVT::I1, a, b, CondCode::SetLt);
    let (chain, dest) = (dag.entry_node(), dag.get_register(EVT::OTHER, 9));
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, test, dest]);
    dag.set_root(branch);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.value_type(dag.operand(root, 1)), EVT::I8);
}
