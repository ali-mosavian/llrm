use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 8] = [0, 1, 2, 3, 0x8000_0001, 0x7FFF_FFFE, 0xFFFF_FFFF, 0xEDB8_8320];

/// The value of `v` when register 5 holds `x`.
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
        ISD::Shl => arg(0) << arg(1),
        ISD::Sra => ((arg(0) as i32) >> arg(1)) as u32,
        ISD::SignExtendInReg => {
            let NodeData::ValueType(inner) = dag.node(dag.operand(v, 1)).data else { panic!("a type") };
            let up = 32 - inner.size_in_bits();
            (((arg(0) << up) as i32) >> up) as u32
        }
        ISD::SetCC => {
            let NodeData::CondCode(cc) = dag.node(dag.operand(v, 2)).data else { panic!("a code") };
            assert_eq!(cc, CondCode::SetEq);
            u32::from(arg(0) == arg(1))
        }
        ISD::Select => {
            if arg(0) != 0 {
                arg(1)
            } else {
                arg(2)
            }
        }
        other => panic!("unexpected {other:?}"),
    }
}

/// The CRC step `select ((crc & 1) == 0), 0, 0xEDB88320` was a `cmov` (a
/// branch on an i486): it is the bit smeared across the word (a sign extension
/// of the bit), anded with the polynomial (bench `crc` code size 1.2x, `neg;
/// je` per bit).
#[test]
fn a_select_on_a_single_bit_test_is_a_sign_mask() {
    for bit in [0u32, 5, 31] {
        let mut dag = dag();
        let x = dag.get_register(EVT::I32, 5);
        let mask = dag.get_constant(1 << bit, EVT::I32);
        let and = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
        let zero = dag.get_constant(0, EVT::I32);
        let cond = dag.get_setcc(EVT::I1, and, zero, CondCode::SetEq);
        let poly = dag.get_constant(0xEDB8_8320, EVT::I32);
        let root = dag.get_select(EVT::I32, cond, zero, poly);
        dag.set_root(root);
        let before: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
        DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
        let root = dag.get_root();
        let after: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
        assert_eq!(before, after, "bit {bit}: the fold changed the value");
        assert_eq!(dag.opcode(root), ISD::And, "bit {bit}");
        // Bit 0 smears with equal shifts, a sign extension of one bit.
        let smear = if bit == 0 { ISD::SignExtendInReg } else { ISD::Sra };
        assert_eq!(dag.opcode(dag.operand(root, 0)), smear, "bit {bit}");
    }
}
