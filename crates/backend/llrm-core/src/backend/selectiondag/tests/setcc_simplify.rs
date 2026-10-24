use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 10] = [0, 1, 5, 100, 0x7F, 0x80, 0xFF, 0x1_00, 0xFFFF_FF9C, 0xFFFF_FFFF];

/// The value of `v` (masked to its width) when register 5 holds `x`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    x: u32,
) -> u32 {
    let width = dag.value_type(v).size_in_bits();
    let mask = |w: u32| if w >= 32 { u32::MAX } else { (1u32 << w) - 1 };
    let sext = |bits: u32, w: u32| (((bits << (32 - w)) as i32) >> (32 - w)) as u32;
    let arg = |i: usize| eval(dag, dag.operand(v, i), x);
    let from = |i: usize| dag.value_type(dag.operand(v, i)).size_in_bits();
    mask(width)
        & match dag.opcode(v) {
            ISD::Register => x,
            ISD::Constant => dag.constant(v).unwrap() as u32,
            ISD::And => arg(0) & arg(1),
            ISD::Truncate | ISD::ZeroExtend => arg(0),
            ISD::SignExtend => sext(arg(0), from(0)),
            ISD::SetCC => {
                let (a, b, w) = (arg(0), arg(1), from(0));
                let NodeData::CondCode(cc) = dag.node(dag.operand(v, 2)).data else { panic!("a code") };
                u32::from(match cc {
                    CondCode::SetEq => a == b,
                    CondCode::SetNe => a != b,
                    CondCode::SetULt => a < b,
                    CondCode::SetUGt => a > b,
                    CondCode::SetLt => (sext(a, w) as i32) < sext(b, w) as i32,
                    CondCode::SetGt => (sext(a, w) as i32) > sext(b, w) as i32,
                    other => panic!("code {other:?}"),
                })
            }
            other => panic!("unexpected {other:?}"),
        }
}

/// `build(x)` combined, with the value of the original and the folded DAG
/// checked for every sample.
fn combined(build: impl FnOnce(&mut SelectionDAG, SDValue) -> SDValue) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let root = build(&mut dag, x);
    dag.set_root(root);
    let original: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let folded: Vec<u32> = SAMPLES.iter().map(|&p| eval(&dag, root, p)).collect();
    assert_eq!(original, folded, "the fold changed the value");
    (dag, root)
}

/// `5 <u x` stayed with its constant on the left, hiding it from every
/// compare-with-constant fold and matcher pattern.
#[test]
fn a_constant_on_the_left_moves_right() {
    let (dag, root) = combined(|dag, x| {
        let five = dag.get_constant(5, EVT::I32);
        dag.get_setcc(EVT::I1, five, x, CondCode::SetULt)
    });
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(5));
    assert_eq!(dag.node(dag.operand(root, 2)).data, NodeData::CondCode(CondCode::SetUGt));
}

/// `zext/sext/and-mask (x) == c` compared the wide value; it compares the
/// narrow one when the constant fits.
#[test]
fn an_extension_compared_for_equality_compares_the_narrow_value() {
    let narrow = |dag: &mut SelectionDAG, x: SDValue| dag.get_node(ISD::Truncate, EVT::I8, &[x]);
    let (dag, root) = combined(|dag, x| {
        let low = narrow(dag, x);
        let ext = dag.get_node(ISD::ZeroExtend, EVT::I32, &[low]);
        let c = dag.get_constant(100, EVT::I32);
        dag.get_setcc(EVT::I1, ext, c, CondCode::SetEq)
    });
    assert_eq!(dag.value_type(dag.operand(root, 0)), EVT::I8, "zext");
    let (dag, root) = combined(|dag, x| {
        let low = narrow(dag, x);
        let ext = dag.get_node(ISD::SignExtend, EVT::I32, &[low]);
        let c = dag.get_constant(0xFFFF_FF9C, EVT::I32);
        dag.get_setcc(EVT::I1, ext, c, CondCode::SetNe)
    });
    assert_eq!(dag.value_type(dag.operand(root, 0)), EVT::I8, "sext");
    let (dag, root) = combined(|dag, x| {
        let mask = dag.get_constant(0xFF, EVT::I32);
        let masked = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
        let c = dag.get_constant(100, EVT::I32);
        dag.get_setcc(EVT::I1, masked, c, CondCode::SetEq)
    });
    assert_eq!(dag.value_type(dag.operand(root, 0)), EVT::I8, "and");
}

/// A constant too wide for the narrow type (`zext i8 == 0x100`) must not
/// narrow: the compare is false for every x.
#[test]
fn a_constant_that_does_not_fit_stays_wide() {
    let (dag, root) = combined(|dag, x| {
        let low = dag.get_node(ISD::Truncate, EVT::I8, &[x]);
        let ext = dag.get_node(ISD::ZeroExtend, EVT::I32, &[low]);
        let c = dag.get_constant(0x100, EVT::I32);
        dag.get_setcc(EVT::I1, ext, c, CondCode::SetEq)
    });
    assert_eq!(dag.value_type(dag.operand(root, 0)), EVT::I32);
}
