use super::super::legalize_dag::DagLegalizer;
use super::super::legalize_types::TypeLegalizer;
use super::super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn p5() -> TargetLowering {
    TargetLowering::x86_32(Features::default(), MVT::I32)
}

fn dword(
    dag: &mut SelectionDAG,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: EVT::I32, align: 4, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0, ordering: None };
    dag.get_load(EVT::I32, chain, ptr, info)
}

/// An `i64` argument arrives as two dword loads and a `build_pair` (the
/// shadow's `x_ll_arith` and `x_ll_div` stopped at "BuildPair: expand
/// result"): its halves are its operands.
#[test]
fn a_build_pair_expands_to_its_operands() {
    let mut dag = dag();
    let (lo, hi) = (dword(&mut dag, 1), dword(&mut dag, 2));
    let pair = dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[pair]);
    let shift = dag.get_constant(32, EVT::I8);
    let upper = dag.get_node(ISD::Srl, EVT::I64, &[pair, shift]);
    let high = dag.get_node(ISD::Truncate, EVT::I32, &[upper]);
    let both = dag.get_node(ISD::Sub, EVT::I32, &[low, high]);
    dag.set_root(both);
    TypeLegalizer::new(&mut dag, &p5()).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.operand(root, 0), dag.operand(root, 1)), (lo, hi));
}

fn sum_of_pairs(op: ISD) -> (SelectionDAG, SDValue, SDValue) {
    let mut dag = dag();
    let (a, b, c, d) = (dword(&mut dag, 1), dword(&mut dag, 2), dword(&mut dag, 3), dword(&mut dag, 4));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[a, b]);
    let y = dag.get_node(ISD::BuildPair, EVT::I64, &[c, d]);
    let result = dag.get_node(op, EVT::I64, &[x, y]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[result]);
    let shift = dag.get_constant(32, EVT::I8);
    let upper = dag.get_node(ISD::Srl, EVT::I64, &[result, shift]);
    let high = dag.get_node(ISD::Truncate, EVT::I32, &[upper]);
    let both = dag.get_node(ISD::Xor, EVT::I32, &[low, high]);
    dag.set_root(both);
    TypeLegalizer::new(&mut dag, &p5()).run().unwrap();
    let root = dag.get_root();
    let (lo, hi) = (dag.operand(root, 0), dag.operand(root, 1));
    (dag, lo, hi)
}

/// `x + y` at 32 bits: an add with overflow of the low dwords, then the
/// high dwords with that carry. A sum of the halves alone loses the carry.
#[test]
fn an_i64_add_is_an_overflow_add_and_an_add_with_carry() {
    let (dag, lo, hi) = sum_of_pairs(ISD::Add);
    assert_eq!((dag.opcode(lo), lo.res), (ISD::UAddO, 0));
    assert_eq!((dag.opcode(hi), hi.res), (ISD::UAddOCarry, 0));
    assert_eq!(dag.operand(hi, 2), SDValue { node: lo.node, res: 1 }, "the carry out of the low half");
    assert_eq!(dag.value_type(dag.operand(hi, 2)), EVT::I8);
}

#[test]
fn an_i64_subtract_is_an_overflow_subtract_and_a_subtract_with_borrow() {
    let (dag, lo, hi) = sum_of_pairs(ISD::Sub);
    assert_eq!((dag.opcode(lo), dag.opcode(hi)), (ISD::USubO, ISD::USubOCarry));
}

/// The target's lowering: the low half is `X86ISD::ADD` and its carry
/// `setcc B`; the high half sets CF from the carry (`carry + -1`) and
/// `ADC`s. `sub`/`sbb` the same.
#[test]
fn the_overflow_add_and_add_with_carry_are_add_and_adc() {
    for (op, first, second) in [(ISD::Add, x86isd::ADD, x86isd::ADC), (ISD::Sub, x86isd::SUB, x86isd::SBB)] {
        let (mut dag, lo, hi) = sum_of_pairs(op);
        let joined = dag.get_node(ISD::Xor, EVT::I32, &[lo, hi]);
        dag.set_root(joined);
        let tli = p5();
        DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false })
            .run()
            .unwrap();
        let root = dag.get_root();
        let (low, high) = (dag.operand(root, 0), dag.operand(root, 1));
        assert_eq!((dag.opcode(low), low.res), (ISD::MergeValues, 0));
        let value = dag.operand(low, 0);
        assert_eq!(dag.opcode(value), first, "{op:?}");
        let carry = dag.operand(low, 1);
        assert_eq!((dag.opcode(carry), dag.constant(dag.operand(carry, 0))), (x86isd::SETCC, Some(cond::B)));
        assert_eq!((dag.opcode(high), high.res), (ISD::MergeValues, 0));
        let sum = dag.operand(high, 0);
        assert_eq!(dag.opcode(sum), second, "{op:?}");
        let flags = dag.operand(sum, 2);
        assert_eq!(dag.opcode(flags), x86isd::ADD, "the carry in is moved into CF");
    }
}
