use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 14] =
    [0, 1, 2, 3, 4, 5, 100, 104, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFE, 0xFFFF_FFFF, 12345, 0xDEAD_BEEF];

/// The value of `v` when register 5 holds `x` and register 6 `y`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    (x, y): (u32, u32),
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), (x, y));
    match dag.opcode(v) {
        ISD::Register => match dag.node(v).data {
            NodeData::Register(5) => x,
            _ => y,
        },
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::Or => arg(0) | arg(1),
        ISD::And => arg(0) & arg(1),
        ISD::Xor => arg(0) ^ arg(1),
        ISD::Add => arg(0).wrapping_add(arg(1)),
        ISD::Sub => arg(0).wrapping_sub(arg(1)),
        ISD::UMax => arg(0).max(arg(1)),
        ISD::UMin => arg(0).min(arg(1)),
        ISD::SetCC => {
            let (a, b) = (arg(0), arg(1));
            let NodeData::CondCode(cc) = dag.node(dag.operand(v, 2)).data else { panic!("a code") };
            u32::from(match cc {
                CondCode::SetEq => a == b,
                CondCode::SetNe => a != b,
                CondCode::SetULt => a < b,
                CondCode::SetULe => a <= b,
                CondCode::SetUGt => a > b,
                CondCode::SetUGe => a >= b,
                CondCode::SetLt => (a as i32) < (b as i32),
                CondCode::SetLe => (a as i32) <= (b as i32),
                CondCode::SetGt => (a as i32) > (b as i32),
                CondCode::SetGe => (a as i32) >= (b as i32),
                CondCode::SetFalse | CondCode::SetFalse2 => false,
                CondCode::SetTrue | CondCode::SetTrue2 => true,
                other => panic!("code {other:?}"),
            })
        }
        other => panic!("unexpected {other:?}"),
    }
}

type Operand<'a> = &'a dyn Fn(&mut SelectionDAG, SDValue, SDValue) -> (SDValue, SDValue);

/// `op (setcc ..), (setcc ..)` folded, with the value of the original and of
/// the folded DAG checked for every pair of sample values.
fn fold(
    op: ISD,
    (left, right): (Operand<'_>, Operand<'_>),
    (cc0, cc1): (CondCode, CondCode),
) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let ((a, b), (c, d)) = (left(&mut dag, x, y), right(&mut dag, x, y));
    let (first, second) = (dag.get_setcc(EVT::I1, a, b, cc0), dag.get_setcc(EVT::I1, c, d, cc1));
    let logic = dag.get_node(op, EVT::I1, &[first, second]);
    dag.set_root(logic);
    let original: Vec<u32> = SAMPLES
        .iter()
        .flat_map(|&p| SAMPLES.iter().map(move |&q| (p, q)))
        .map(|pair| eval(&dag, logic, pair))
        .collect();
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let folded: Vec<u32> =
        SAMPLES.iter().flat_map(|&p| SAMPLES.iter().map(move |&q| (p, q))).map(|pair| eval(&dag, root, pair)).collect();
    assert_eq!(original, folded, "the fold changed the value");
    (dag, root)
}

fn with_const(c: u32) -> impl Fn(&mut SelectionDAG, SDValue, SDValue) -> (SDValue, SDValue) + Copy {
    move |dag, x, _| (x, dag.get_constant(u128::from(c), EVT::I32))
}

/// The pin tests `a == 104 && b == 100` as `((a ^ 104) | (b ^ 100)) == 0`:
/// two compares and an `and` became one compare (`pin only XOR32ri`,
/// D_FACES_LAYOUT_OK of the m32 shadow).
#[test]
fn equality_of_two_variables_to_constants_is_one_compare_of_xors() {
    let (a, b) = (
        |dag: &mut SelectionDAG, x, _| (x, dag.get_constant(104, EVT::I32)),
        |dag: &mut SelectionDAG, _, y| (y, dag.get_constant(100, EVT::I32)),
    );
    let (dag, root) = fold(ISD::And, (&a, &b), (CondCode::SetEq, CondCode::SetEq));
    assert_eq!(dag.opcode(root), ISD::SetCC);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Or);
    assert_eq!(dag.opcode(dag.operand(dag.operand(root, 0), 0)), ISD::Xor);
}

/// `x != 0 || y != 0` is `(x | y) != 0`; `x < 0 || y < 0` is `(x | y) < 0`.
#[test]
fn tests_against_zero_share_one_or() {
    let zero = |dag: &mut SelectionDAG, x, _| (x, dag.get_constant(0, EVT::I32));
    let zero_y = |dag: &mut SelectionDAG, _, y| (y, dag.get_constant(0, EVT::I32));
    for (op, cc) in [(ISD::Or, CondCode::SetNe), (ISD::And, CondCode::SetEq), (ISD::Or, CondCode::SetLt)] {
        let (dag, root) = fold(op, (&zero, &zero_y), (cc, cc));
        assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Or, "{op:?} {cc:?}");
    }
}

/// `x != 0 && x != -1` is `x + 1 >=u 2`, which `SimplifySetCC` makes `x + 1 >u
/// 1`.
#[test]
fn not_zero_and_not_all_ones_is_an_unsigned_range() {
    let zero = |dag: &mut SelectionDAG, x, _| (x, dag.get_constant(0, EVT::I32));
    let ones = |dag: &mut SelectionDAG, x, _| (x, dag.get_constant(0xFFFF_FFFF, EVT::I32));
    let (dag, root) = fold(ISD::And, (&zero, &ones), (CondCode::SetNe, CondCode::SetNe));
    assert_eq!(dag.node(dag.operand(root, 2)).data, NodeData::CondCode(CondCode::SetUGt));
    assert_eq!(dag.constant(dag.operand(root, 1)), Some(1));
}

/// Two compares of the same pair combine their codes: `x <u y && x != y` is
/// `x <u y`; `x <u y || x == y` is `x <=u y`; a signed and an unsigned code
/// do not combine.
#[test]
fn two_compares_of_one_pair_combine_their_codes() {
    let pair = |_: &mut SelectionDAG, x, y| (x, y);
    let (dag, root) = fold(ISD::And, (&pair, &pair), (CondCode::SetULt, CondCode::SetNe));
    assert_eq!(dag.node(dag.operand(root, 2)).data, NodeData::CondCode(CondCode::SetULt));
    let (dag, root) = fold(ISD::Or, (&pair, &pair), (CondCode::SetULt, CondCode::SetEq));
    assert_eq!(dag.node(dag.operand(root, 2)).data, NodeData::CondCode(CondCode::SetULe));
    let (dag, root) = fold(ISD::And, (&pair, &pair), (CondCode::SetLt, CondCode::SetULt));
    assert_eq!(dag.opcode(root), ISD::And, "signed and unsigned stay two compares");
}

/// Constants one bit apart: `x != 5 && x != 4` is `((x - 4) & ~1) != 0`.
#[test]
fn constants_a_bit_apart_share_one_masked_compare() {
    let (dag, root) = fold(ISD::And, (&with_const(5), &with_const(4)), (CondCode::SetNe, CondCode::SetNe));
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::And);
}
