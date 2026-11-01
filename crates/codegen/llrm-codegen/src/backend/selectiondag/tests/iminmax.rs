use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

const SAMPLES: [u32; 7] = [0, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF, 0xFFFF_FFFE, 5];

fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    x: u32,
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), x);
    match dag.opcode(v) {
        ISD::Register => x,
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::SMax => (arg(0) as i32).max(arg(1) as i32) as u32,
        ISD::SMin => (arg(0) as i32).min(arg(1) as i32) as u32,
        ISD::UMax => arg(0).max(arg(1)),
        ISD::UMin => arg(0).min(arg(1)),
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Sra => ((arg(0) as i32) >> arg(1).min(31)) as u32,
        other => panic!("unexpected {other:?}"),
    }
}

/// `build(x)` combined for m32, the value checked before and after.
fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let root = build(&mut dag, x);
    dag.set_root(root);
    let before: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    let after: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    assert_eq!(before, after, "the fold changed the value");
    (dag, root)
}

/// `visitIMINMAX` did not exist: `smax x, -1` stayed a min/max node, which
/// expands to a compare and a branch (no `cmov` before the P6); the pin makes
/// it `x | (x >>s 31)`, and `smin x, 0` `x & (x >>s 31)`.
#[test]
fn a_clamp_at_the_sign_is_a_mask_of_the_sign() {
    for (op, c, want) in [(ISD::SMax, 0xFFFF_FFFF, ISD::Or), (ISD::SMin, 0, ISD::And)] {
        let (dag, root) = combined(|dag, x| {
            let bound = dag.get_constant(c, EVT::I32);
            dag.get_node(op, EVT::I32, &[x, bound])
        });
        assert_eq!(dag.opcode(root), want, "{op:?}");
        assert_eq!(dag.opcode(dag.operand(root, 1)), ISD::Sra, "{op:?}");
    }
}

/// A constant moves to the right, and two nested clamps with constants are one.
#[test]
fn a_constant_moves_right_and_nested_clamps_with_constants_are_one() {
    let (dag, root) = combined(|dag, x| {
        let five = dag.get_constant(5, EVT::I32);
        dag.get_node(ISD::UMax, EVT::I32, &[five, x])
    });
    assert_eq!((dag.opcode(root), dag.opcode(dag.operand(root, 1))), (ISD::UMax, ISD::Constant));
    let (dag, root) = combined(|dag, x| {
        let (three, five) = (dag.get_constant(3, EVT::I32), dag.get_constant(5, EVT::I32));
        let inner = dag.get_node(ISD::UMax, EVT::I32, &[x, three]);
        dag.get_node(ISD::UMax, EVT::I32, &[inner, five])
    });
    assert_eq!(dag.opcode(root), ISD::UMax);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Register);
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(5));
}
