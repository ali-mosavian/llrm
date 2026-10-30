use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;

const SAMPLES: [u32; 6] = [0, 1, 2, 3, 0x8000_0001, 0xFFFF_FFFF];

/// The value of `v` when registers 5, 6 and 7 hold `x`, `y` and `z`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    (x, y, z): (u32, u32, u32),
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), (x, y, z));
    match dag.opcode(v) {
        ISD::Register => match dag.node(v).data {
            NodeData::Register(5) => x,
            NodeData::Register(6) => y,
            _ => z,
        },
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Xor => arg(0) ^ arg(1),
        ISD::Add => arg(0).wrapping_add(arg(1)),
        ISD::Sub => arg(0).wrapping_sub(arg(1)),
        other => panic!("unexpected {other:?}"),
    }
}

fn has(
    dag: &SelectionDAG,
    v: SDValue,
    op: ISD,
) -> bool {
    dag.opcode(v) == op || (0..dag.node(v).ops.len()).any(|i| has(dag, dag.operand(v, i), op))
}

/// `select ((x & 1) == 0), a, b` legalized for a CPU without `cmov`; the value
/// of the result checked against the select for every sample.
fn lowered(build: impl FnOnce(&mut SelectionDAG, [SDValue; 3]) -> (SDValue, SDValue)) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let [x, y, z] = [5, 6, 7].map(|r| dag.get_register(EVT::I32, r));
    let (then, other) = build(&mut dag, [x, y, z]);
    let (one, zero) = (dag.get_constant(1, EVT::I32), dag.get_constant(0, EVT::I32));
    let low = dag.get_node(ISD::And, EVT::I32, &[x, one]);
    let test = dag.get_setcc(EVT::I8, low, zero, CondCode::SetEq);
    let root = dag.get_select(EVT::I32, test, then, other);
    dag.set_root(root);
    let want = |(a, b, c): (u32, u32, u32)| {
        if a & 1 == 0 { eval(&dag, then, (a, b, c)) } else { eval(&dag, other, (a, b, c)) }
    };
    let expected: Vec<u32> = SAMPLES.iter().flat_map(|&a| SAMPLES.iter().map(move |&b| (a, b, 5))).map(want).collect();
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false }).run().unwrap();
    let root = dag.get_root();
    let got: Vec<u32> =
        SAMPLES.iter().flat_map(|&a| SAMPLES.iter().map(move |&b| (a, b, 5))).map(|p| eval(&dag, root, p)).collect();
    assert_eq!(expected, got, "the lowering changed the value");
    (dag, root)
}

/// `select ((x & 1) == 0), 0, 0xEDB88320` and its CRC cousin
/// `select ((x & 1) == 0), y, y ^ z` were a `cmov` (a branch on an i486): the
/// pin makes `-(x & 1) & z` and combines it with `y` (bench `x_crc32`:
/// 8 x `cmov` + `test` where the pin has `and`, `neg`, `xor`).
#[test]
fn a_select_on_the_low_bit_without_cmov_is_arithmetic() {
    let (dag, root) = lowered(|dag, [_, y, z]| (y, dag.get_node(ISD::Xor, EVT::I32, &[y, z])));
    assert!(!has(&dag, root, x86isd::CMOV), "identity pattern");
    let (dag, root) = lowered(|dag, _| (dag.get_constant(5, EVT::I32), dag.get_constant(9, EVT::I32)));
    assert!(!has(&dag, root, x86isd::CMOV), "two constants");
    let (dag, root) = lowered(|dag, [_, y, z]| (dag.get_node(ISD::And, EVT::I32, &[y, z]), y));
    assert!(!has(&dag, root, x86isd::CMOV), "and identity");
}

/// The CRC step as the IR has it: `select (trunc c to i1), y ^ z, y` is a
/// select on the raw bit `c & 1`, the same arithmetic with the arms swapped.
#[test]
fn a_select_on_a_raw_low_bit_is_arithmetic_too() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let [x, y, z] = [5, 6, 7].map(|r| dag.get_register(EVT::I32, r));
    let one = dag.get_constant(1, EVT::I32);
    let bit = dag.get_node(ISD::And, EVT::I32, &[x, one]);
    let flipped = dag.get_node(ISD::Xor, EVT::I32, &[y, z]);
    let root = dag.get_select(EVT::I32, bit, flipped, y);
    dag.set_root(root);
    let want = |(a, b, c): (u32, u32, u32)| if a & 1 != 0 { b ^ c } else { b };
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: false, sse2: false, min_size: false }).run().unwrap();
    let root = dag.get_root();
    assert!(!has(&dag, root, x86isd::CMOV));
    for (a, b) in SAMPLES.iter().flat_map(|&a| SAMPLES.iter().map(move |&b| (a, b))) {
        assert_eq!(eval(&dag, root, (a, b, 5)), want((a, b, 5)), "x={a:#x} y={b:#x}");
    }
}
