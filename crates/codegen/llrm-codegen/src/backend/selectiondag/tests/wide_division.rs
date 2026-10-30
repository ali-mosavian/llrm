use super::super::legalize_types::TypeLegalizer;
use super::super::x86isd;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn quotient_of(
    op: ISD,
    narrow: bool,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let mut dword = |slot: i32| {
        let info =
            MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0, ordering: None };
        dag.get_load(EVT::I32, chain, ptr, info)
    };
    let (a, b, c, d) = (dword(1), dword(2), dword(3), dword(4));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[a, b]);
    let high = if narrow { dag.get_constant(0, EVT::I32) } else { d };
    let y = dag.get_node(ISD::BuildPair, EVT::I64, &[c, high]);
    let result = dag.get_node(op, EVT::I64, &[x, y]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[result]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).with_target(&llrm_x86_m32::M32).run().unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// The shadow listed `i64` SRem/UDiv/... "expand result" (37+1 functions). A
/// division is a call to the target's inline routine: the callee is its
/// name, and the operands are copied to the registers it reads.
fn call_of(dag: &SelectionDAG) -> (String, usize) {
    let call = (0..dag.node_count() as u32)
        .map(|node| SDValue { node, res: 0 })
        .find(|&v| dag.opcode(v) == x86isd::CALL)
        .expect("a call");
    let callee = dag.operand(call, 1);
    let NodeData::Symbol(id) = dag.node(callee).data else { panic!("a symbol callee") };
    let copies = dag.node(call).ops.iter().filter(|op| dag.opcode(**op) == ISD::Register).count();
    (dag.symbol_names[id as usize].clone(), copies)
}

#[test]
fn an_i64_unsigned_divide_by_a_wide_divisor_calls_the_wide_routine() {
    let (dag, root) = quotient_of(ISD::UDiv, false);
    assert_eq!(dag.opcode(root), ISD::CopyFromReg, "the quotient's low dword is read from a register");
    assert_eq!(call_of(&dag), ("div64_u".to_owned(), 4), "four operand registers");
}

#[test]
fn a_divisor_with_a_zero_high_dword_calls_the_dword_routine() {
    let (dag, _) = quotient_of(ISD::SDiv, true);
    // a quotient read alone: the routine that fixes only its sign (SD232)
    assert_eq!(call_of(&dag), ("div64_s32q".to_owned(), 3), "no register for the divisor's high dword");
}

/// A quotient is read from the routine's quotient register, a remainder from
/// its remainder register: swapping them returns the wrong half of the pair.
#[test]
fn quotient_and_remainder_are_read_from_their_own_registers() {
    use llrm_target::Target;
    let routine = llrm_x86_m32::M32.divide_routine(false, true).expect("a routine");
    let register = |dag: &SelectionDAG, v: SDValue| dag.node(dag.operand(v, 1)).data;
    let (quotient, qroot) = quotient_of(ISD::UDiv, false);
    let (remainder, rroot) = quotient_of(ISD::URem, false);
    assert_eq!(register(&quotient, qroot), NodeData::PhysReg(routine.quotient[0]));
    assert_eq!(register(&remainder, rroot), NodeData::PhysReg(routine.remainder[0]));
}
