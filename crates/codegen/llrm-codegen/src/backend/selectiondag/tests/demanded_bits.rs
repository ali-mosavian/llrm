use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 8] = [0, 1, 0xFF, 0x0F0F_0F0F, 0x8000_0001, 0xDEAD_BEEF, 0x1234_5678, 0xFFFF_FFFF];

/// The value of `v` when register 5 holds `x`, 6 `y`.
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
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Xor => arg(0) ^ arg(1),
        ISD::Shl => arg(0).checked_shl(arg(1)).unwrap_or(0),
        ISD::Srl => arg(0).checked_shr(arg(1)).unwrap_or(0),
        ISD::Rotl => arg(0).rotate_left(arg(1)),
        ISD::SignExtendInReg => {
            let NodeData::ValueType(ext) = dag.node(dag.operand(v, 1)).data else { panic!("no type") };
            let shift = 32 - ext.size_in_bits();
            (((arg(0) << shift) as i32) >> shift) as u32
        }
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

/// `build(x, y)` combined, the value checked for every pair of samples. Only
/// the bits `care` selects are compared: the rewrite may change the others.
fn combined(
    care: u32,
    build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue,
) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let root = build(&mut dag, x, y);
    dag.set_root(root);
    let pairs = || SAMPLES.iter().flat_map(|&p| SAMPLES.iter().map(move |&q| (p, q)));
    let before: Vec<u32> = pairs().map(|pair| eval(&dag, root, pair) & care).collect();
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let after: Vec<u32> = pairs().map(|pair| eval(&dag, root, pair) & care).collect();
    assert_eq!(before, after, "the rewrite changed a demanded bit");
    (dag, root)
}

fn k(
    dag: &mut SelectionDAG,
    value: u128,
) -> SDValue {
    dag.get_constant(value, EVT::I32)
}

fn shift(
    dag: &mut SelectionDAG,
    op: ISD,
    x: SDValue,
    by: u128,
) -> SDValue {
    let amount = dag.get_constant(by, EVT::I8);
    dag.get_node(op, EVT::I32, &[x, amount])
}

/// `(or (srl x, 16), (shl x, 16)) & 255` read only the low byte, which the
/// `shl` never reaches: the `shl` and the `or` went to instructions anyway
/// (bench `x_popcount`: `rev32(s) & 255`, `ROL32ri` and an `OR32rr` the pin
/// does not have).
#[test]
fn a_shift_that_reaches_no_demanded_bit_is_dropped() {
    let (dag, root) = combined(u32::MAX, |dag, x, _| {
        let (hi, lo) = (shift(dag, ISD::Srl, x, 16), shift(dag, ISD::Shl, x, 16));
        let or = dag.get_node(ISD::Or, EVT::I32, &[hi, lo]);
        let low = k(dag, 255);
        dag.get_node(ISD::And, EVT::I32, &[or, low])
    });
    assert!(!has(&dag, root, ISD::Shl));
    assert!(!has(&dag, root, ISD::Or));
}

/// A constant with bits no one reads is narrowed: `(or x, 0xF0F0) & 0xFF` is
/// `(or x, 0xF0) & 0xFF`.
#[test]
fn a_constant_is_narrowed_to_the_demanded_bits() {
    let (dag, root) = combined(u32::MAX, |dag, x, _| {
        let (wide, low) = (k(dag, 0xF0F0), k(dag, 0xFF));
        let or = dag.get_node(ISD::Or, EVT::I32, &[x, wide]);
        dag.get_node(ISD::And, EVT::I32, &[or, low])
    });
    let or = dag.operand(root, 0);
    assert_eq!(dag.opcode(or), ISD::Or);
    assert_eq!(dag.constant(dag.operand(or, 1)), Some(0xF0));
}

/// An operand whose demanded bits are known is a constant: the `xor` of two
/// values masked to disjoint bits, read through one of the masks.
#[test]
fn an_operand_known_on_the_demanded_bits_is_replaced() {
    let (dag, root) = combined(u32::MAX, |dag, x, y| {
        let (hi, low) = (k(dag, 0xFF00), k(dag, 0xFF));
        let masked_x = dag.get_node(ISD::And, EVT::I32, &[x, hi]);
        let xor = dag.get_node(ISD::Xor, EVT::I32, &[masked_x, y]);
        dag.get_node(ISD::And, EVT::I32, &[xor, low])
    });
    assert!(!has(&dag, root, ISD::Xor), "the xor reads only y on the low byte");
}

/// `rotl x, 16` read through `& 255` is `srl x, 16`: a rotate was a `rol`
/// (bench `x_popcount`'s `fshl(x, x, 16) & 255`).
#[test]
fn a_rotate_read_in_its_low_bits_is_a_shift() {
    let (dag, root) = combined(255, |dag, x, _| {
        let by = dag.get_constant(16, EVT::I8);
        let rot = dag.get_node(ISD::Rotl, EVT::I32, &[x, by]);
        let low = k(dag, 255);
        dag.get_node(ISD::And, EVT::I32, &[rot, low])
    });
    assert!(!has(&dag, root, ISD::Rotl));
}

/// `(sext_in_reg (or (shl hi, 16), lo), i16)` kept the `shl` and
/// the `or`: no `SIGN_EXTEND_INREG` demanded-bits case. A call returning an
/// `i32` in dx:ax and truncated to `i16` built `shl; movzx; or; movsx` before
/// every multiply-high (recpow m16: +94 B).
#[test]
fn a_sext_in_reg_of_a_rebuilt_pair_reads_only_the_low_half() {
    let (dag, root) = combined(!0, |dag, x, y| {
        let high = shift(dag, ISD::Shl, y, 16);
        let pair = dag.get_node(ISD::Or, EVT::I32, &[high, x]);
        let ty = dag.get_value_type_node(EVT::I16);
        dag.get_node(ISD::SignExtendInReg, EVT::I32, &[pair, ty])
    });
    assert!(!has(&dag, root, ISD::Shl) && !has(&dag, root, ISD::Or), "the high half is still read");
}

/// `shld eax, edx, cl` kept the `and ecx, 31` the IR had on its count
/// (x_ll_arith -O2 m32: +1 instruction a trip against the pin): at a
/// power-of-two width a funnel shift demands only the count's low bits.
#[test]
fn a_funnel_shift_count_loses_its_mask_to_the_width() {
    let mut dag = dag();
    let (x, y, z) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6), dag.get_register(EVT::I32, 7));
    let thirty_one = dag.get_constant(31, EVT::I32);
    let count = dag.get_node(ISD::And, EVT::I32, &[z, thirty_one]);
    let root = dag.get_node(ISD::FShl, EVT::I32, &[x, y, count]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::FShl);
    assert_eq!(dag.opcode(dag.operand(root, 2)), ISD::Register, "the count kept its and");
}

/// `trunc (and x, 31)` stayed wide, so a shift count read the byte through
/// `and ecx, 31` where the pin's `movzx ecx, byte [m]` does (visitTRUNCATE,
/// before operations are legal).
#[test]
fn a_truncated_and_with_a_constant_is_the_and_of_the_truncation() {
    let mut dag = dag();
    let z = dag.get_register(EVT::I32, 7);
    let thirty_one = dag.get_constant(31, EVT::I32);
    let wide = dag.get_node(ISD::And, EVT::I32, &[z, thirty_one]);
    let root = dag.get_node(ISD::Truncate, EVT::I8, &[wide]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::And, EVT::I8));
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Truncate);
}
