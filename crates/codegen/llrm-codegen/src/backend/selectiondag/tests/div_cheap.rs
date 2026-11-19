//! `isIntDivCheap`: where a divide is cheap, `x % 251` stayed a multiply-high
//! sequence (`movzx`, `imul`, `shr`, `imul`, `sub`; 13 instructions against 6,
//! +21..25% clocks on bench/huge at m16 -Os).

use super::super::dag::{DivBytes, DivPrices, ProductPrice};
use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

fn folded(
    op: ISD,
    d: u128,
    int_div_cheap: bool,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = SelectionDAG::new(DagConfig { int_div_cheap, ..DagConfig::default() });
    let x = dag.get_register(EVT::I16, 5);
    let divisor = dag.get_constant(d, EVT::I16);
    let divide = dag.get_node(op, EVT::I16, &[x, divisor]);
    dag.set_root(divide);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root)
}

/// A divide by a constant that is not a power of two stays a divide.
#[test]
fn a_cheap_divide_by_a_constant_stays_a_divide() {
    for op in [ISD::UDiv, ISD::URem, ISD::SDiv, ISD::SRem] {
        let (dag, root) = folded(op, 251, true);
        assert_eq!(dag.opcode(root), op, "{op:?}");
        let (dag, root) = folded(op, 251, false);
        assert_ne!(dag.opcode(root), op, "{op:?} without the flag");
    }
}

/// A power of two is still a shift or a mask.
#[test]
fn a_cheap_divide_by_a_power_of_two_is_still_a_shift() {
    for (op, want) in [(ISD::UDiv, ISD::Srl), (ISD::URem, ISD::And)] {
        let (dag, root) = folded(op, 16, true);
        assert_eq!(dag.opcode(root), want, "{op:?}");
    }
}

/// An `i8` divide selects `DIV8r` (SD302), so where divides are cheap the
/// remainder by a constant stays one, as for every other width. It was the
/// multiply and shifts of the wider types while the selector could not take it.
#[test]
fn a_cheap_i8_divide_by_a_constant_stays_a_divide() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = SelectionDAG::new(DagConfig { int_div_cheap: true, ..DagConfig::default() });
    let x = dag.get_register(EVT::I8, 5);
    let divisor = dag.get_constant(6, EVT::I8);
    let rem = dag.get_node(ISD::URem, EVT::I8, &[x, divisor]);
    dag.set_root(rem);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::URem);
}

/// The i486 sheet in real mode (`machine/tune/x86/i486.toml`): `div r16` 24,
/// `imul r32` 26, `idiv r32` 43.
const I486_REAL_MODE: DivPrices = DivPrices {
    operand: 2,
    prefix: 1,
    add: 1,
    shift: 2,
    extend: 3,
    multiply: 13,
    multiply_long: 26,
    multiply_high: 26,
    divide: 24,
    divide_long: 43,
    margin: 25,
    product: None,
    bytes: None,
};

fn priced(
    op: ISD,
    vt: EVT,
    d: u128,
    div_prices: Option<DivPrices>,
) -> ISD {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = SelectionDAG::new(DagConfig { div_prices, ..DagConfig::default() });
    let x = dag.get_register(vt, 5);
    let divisor = dag.get_constant(d, vt);
    let divide = dag.get_node(op, vt, &[x, divisor]);
    dag.set_root(divide);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    dag.opcode(dag.get_root())
}

/// bench recsum (`(n * 3) % 11` on a 16-bit int) ran 15 instructions and 53
/// clocks a trip, 106017 clocks in all, against 7 and 41, 82025, with `idiv`
/// kept: the multiply-high sequence cost more than the 486's `div r16`.
/// bench recpow's `% 97` and `% 1000` the same (569025 clocks against 544225).
#[test]
fn a_16_bit_remainder_by_a_constant_stays_a_divide_where_the_sheet_prices_it_below_the_magic_sequence() {
    for (op, d) in [(ISD::SRem, 11), (ISD::URem, 97), (ISD::SRem, 1000), (ISD::UDiv, 97)] {
        assert_eq!(priced(op, EVT::I16, d, Some(I486_REAL_MODE)), op, "{op:?} {d}");
    }
}

/// No prices, the pin's rule: the sequence.
#[test]
fn without_prices_a_constant_divide_is_the_magic_sequence() {
    assert_ne!(priced(ISD::URem, EVT::I16, 97, None), ISD::URem);
}

/// A 32-bit divide is near the sheet's tie (`idiv r32` 43 against `imul r32` 26
/// and the shifts): the sequence stays. Hardware won there on the sheet by two
/// clocks and lost on bench recmany and switch (+3946 clocks).
#[test]
fn a_32_bit_divide_by_a_constant_stays_the_magic_sequence() {
    for (op, d) in [(ISD::UDiv, 10), (ISD::SDiv, 10), (ISD::SRem, 7), (ISD::SRem, 14)] {
        assert_ne!(priced(op, EVT::I32, d, Some(I486_REAL_MODE)), op, "{op:?} {d}");
    }
}

/// Flat 32-bit code (an `idiv r32` 43 clocks): the sequence, as before.
#[test]
fn a_flat_target_keeps_the_magic_sequence() {
    let flat = DivPrices { operand: 4, divide: 43, divide_long: 43, multiply: 26, multiply_long: 26, ..I486_REAL_MODE };
    for d in [10, 97, 1000] {
        assert_ne!(priced(ISD::URem, EVT::I32, d, Some(flat.clone())), ISD::URem, "{d}");
    }
}

/// The margin is the target's (`divide_margin`), not shared code's: a 32-bit
/// remainder the sheet prices two clocks under the sequence stays a divide at
/// margin 0 and the sequence at the sheet's 25.
#[test]
fn the_margin_is_read_from_the_prices() {
    let none = DivPrices { margin: 0, ..I486_REAL_MODE };
    assert_eq!(priced(ISD::SRem, EVT::I32, 14, Some(none)), ISD::SRem);
    assert_ne!(priced(ISD::SRem, EVT::I32, 14, Some(I486_REAL_MODE)), ISD::SRem);
}

/// recmany `% 5`, `% 3`, `% 7` at m16 -O2 were three magic multiply-high
/// sequences of about 25 bytes each where bcc's `mov ebx,k; cdq; idiv ebx`
/// is 11: the choice weighed clocks alone (the sheet's two prices tie), so the
/// 14 bytes never counted though m16 prices a clock at two bytes. Bytes at the
/// rate decide where the clocks are within the margin.
#[test]
fn the_rate_weighs_the_bytes_of_the_divide_against_the_sequence() {
    let bytes = DivBytes {
        prefix: 1,
        add: 4,
        shift: 6,
        extend: 6,
        multiply: 6,
        multiply_long: 8,
        multiply_high: 8,
        divide: 6,
        divide_long: 8,
        milliclocks_per_byte: 500,
        frequency: 1.0,
    };
    assert_ne!(priced(ISD::SRem, EVT::I32, 5, Some(I486_REAL_MODE)), ISD::SRem, "clocks alone: the sequence");
    let weighed = DivPrices { bytes: Some(bytes), ..I486_REAL_MODE };
    assert_eq!(priced(ISD::SRem, EVT::I32, 5, Some(weighed)), ISD::SRem, "clocks plus bytes: the divide");
}

/// The remainder's `q * d` was priced as a 2-shift 2-add chain the selector
/// does not emit; it emits what `Machine::multiply_by` prices. A dear product
/// makes the divide win where the cheap chain let the sequence.
#[test]
fn the_remainders_product_is_priced_as_what_the_selector_emits() {
    let flat = |clocks: i64| DivPrices {
        margin: 0,
        product: Some(ProductPrice(std::rc::Rc::new(move |_| clocks))),
        ..I486_REAL_MODE
    };
    assert_ne!(priced(ISD::SRem, EVT::I32, 14, Some(flat(0))), ISD::SRem);
    assert_eq!(priced(ISD::SRem, EVT::I32, 14, Some(flat(100))), ISD::SRem);
}

/// With no margin the multiply-high is priced as the dividend-multiplier's
/// early-out (`multiply_high`, the sheet's shortest multiply), not the midpoint
/// of 13..42 (`multiply_long`): at the midpoint a 32-bit remainder by 7 or 14
/// chose `idiv` (the sheet's 43) on its own, where the sequence is what recmany
/// and switch run faster.
#[test]
fn a_32_bit_remainder_is_the_sequence_when_the_multiply_high_is_the_shortest_multiply() {
    let flat = DivPrices { margin: 0, multiply_high: 13, ..I486_REAL_MODE };
    for (op, d) in [(ISD::SRem, 7), (ISD::SRem, 14), (ISD::URem, 7), (ISD::URem, 97)] {
        assert_ne!(priced(op, EVT::I32, d, Some(flat.clone())), op, "{op:?} {d}");
    }
    let midpoint = DivPrices { margin: 0, ..I486_REAL_MODE };
    assert_eq!(
        priced(ISD::SRem, EVT::I32, 14, Some(midpoint)),
        ISD::SRem,
        "the premise: at the midpoint it was the divide"
    );
}

/// recmany `% 5` and `% 7` sat in a loop at m16 -O2, and the bytes weighed
/// against one execution's clocks chose `idiv`: 98 bytes saved for 28889
/// clocks lost (+12.6%), switch the same (-63 B, +4.9%). The clocks recur with
/// the block's frequency; the bytes do not.
#[test]
fn a_divide_in_a_hot_block_keeps_the_sequence_the_bytes_would_drop() {
    let bytes = |frequency| DivBytes {
        prefix: 1,
        add: 4,
        shift: 6,
        extend: 6,
        multiply: 6,
        multiply_long: 8,
        multiply_high: 8,
        divide: 6,
        divide_long: 8,
        milliclocks_per_byte: 500,
        frequency,
    };
    let at = |frequency| DivPrices { bytes: Some(bytes(frequency)), ..I486_REAL_MODE };
    assert_eq!(priced(ISD::SRem, EVT::I32, 5, Some(at(1.0))), ISD::SRem, "once: the divide");
    assert_ne!(priced(ISD::SRem, EVT::I32, 5, Some(at(16.0))), ISD::SRem, "sixteen times: the sequence");
}

fn weighed_prices() -> DivPrices {
    let bytes = DivBytes {
        prefix: 1,
        add: 4,
        shift: 6,
        extend: 6,
        multiply: 6,
        multiply_long: 8,
        multiply_high: 8,
        divide: 6,
        divide_long: 8,
        milliclocks_per_byte: 500,
        frequency: 1.0,
    };
    DivPrices { bytes: Some(bytes), ..I486_REAL_MODE }
}

/// The DAG after combining, for `x / 5` and, beside it, `x % 5` of the same
/// `i32` operand.
fn combined_pair(with_remainder: bool) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = SelectionDAG::new(DagConfig { div_prices: Some(weighed_prices()), ..DagConfig::default() });
    let x = dag.get_register(EVT::I32, 5);
    let five = dag.get_constant(5, EVT::I32);
    let quotient = dag.get_node(ISD::SDiv, EVT::I32, &[x, five]);
    let root = if with_remainder {
        let remainder = dag.get_node(ISD::SRem, EVT::I32, &[x, five]);
        dag.get_node(ISD::Add, EVT::I32, &[quotient, remainder])
    } else {
        quotient
    };
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root)
}

fn reaches(
    dag: &SelectionDAG,
    v: SDValue,
    ops: &[ISD],
) -> bool {
    ops.contains(&dag.opcode(v)) || (0..dag.node(v).ops.len()).any(|i| reaches(dag, dag.operand(v, i), ops))
}

/// qlight (`ls_scale_byte`: floor division, `x / 120` and `x % 120` of one
/// `i32`) at m16 -O2 ran a multiply-high quotient and an `idiv` for the
/// remainder, both: 736 clocks against 561 at -O1 and the bar's 617. The divide
/// alone is the sequence and the remainder alone the `idiv` (the test above); a
/// pair is priced once, as the remainder's, and both stay divides.
#[test]
fn a_divide_beside_its_remainder_is_priced_as_the_pair() {
    let (dag, root) = combined_pair(false);
    assert!(!reaches(&dag, root, &[ISD::SDiv, ISD::SDivRem]), "the premise: the divide alone is the sequence");
    let (dag, root) = combined_pair(true);
    assert!(
        !reaches(&dag, root, &[ISD::MulHS, ISD::SMulLoHi]),
        "no multiply-high sequence beside the `idiv` that already leaves the quotient"
    );
    assert!(
        reaches(&dag, root, &[ISD::SDiv]) && reaches(&dag, root, &[ISD::SRem]),
        "both stay divides, which selection pairs"
    );
}
