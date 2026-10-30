use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 6] = [0, 1, 0x8000_0000, 0xFFFF_FFFF, 0x1234_5678, 0xFFF8_0000];

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
        ISD::Srl => arg(0).checked_shr(arg(1)).unwrap_or(0),
        ISD::Truncate => arg(0) & ((1u64 << dag.value_type(v).size_in_bits()) - 1) as u32,
        other => panic!("unexpected {other:?}"),
    }
}

fn count(
    dag: &SelectionDAG,
    v: SDValue,
    op: ISD,
) -> usize {
    usize::from(dag.opcode(v) == op)
        + (0..dag.node(v).ops.len()).map(|i| count(dag, dag.operand(v, i), op)).sum::<usize>()
}

/// `srl (trunc (srl x, 16)), 3` on an i16 kept two shifts (`shr ecx, 16; shr
/// cx, 3` after the magic multiply of `(i*7) % 26`); the pin has one, `shr ecx,
/// 19`. The general case (`c1 + 16 != 32`) clears the high bits with an `and`.
#[test]
fn a_shift_of_a_truncated_shift_is_one_shift_of_the_wide_value() {
    for (first, second, shifts, ands) in [(16u128, 3u128, 1, 0), (8, 3, 1, 1)] {
        let mut dag = dag();
        let x = dag.get_register(EVT::I32, 5);
        let a = dag.get_constant(first, EVT::I8);
        let inner = dag.get_node(ISD::Srl, EVT::I32, &[x, a]);
        let narrow = dag.get_node(ISD::Truncate, EVT::I16, &[inner]);
        let b = dag.get_constant(second, EVT::I8);
        let root = dag.get_node(ISD::Srl, EVT::I16, &[narrow, b]);
        dag.set_root(root);
        let before: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
        DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
        let root = dag.get_root();
        let after: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
        assert_eq!(before, after, "c1 {first}: the fold changed the value");
        assert_eq!(count(&dag, root, ISD::Srl), shifts, "c1 {first}: shifts");
        assert_eq!(count(&dag, root, ISD::And), ands, "c1 {first}: ands");
    }
}
