//! `isIntDivCheap`: where a divide is cheap, `x % 251` stayed a multiply-high
//! sequence (`movzx`, `imul`, `shr`, `imul`, `sub`; 13 instructions against 6,
//! +21..25% clocks on bench/huge at m16 -Os).

use super::super::dag::{DivPrices, ProductPrice};
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

/// An `i8` divide has no `DIV8r` in the selector (SD302): at -Oz `(x >> 8) % 6`
/// of examples/dice.nib and easing.nib stopped compiling (`CannotSelect
/// ("UDivRem")`), so it keeps the multiply.
#[test]
fn a_cheap_i8_divide_by_a_constant_stays_a_multiply() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = SelectionDAG::new(DagConfig { int_div_cheap: true, ..DagConfig::default() });
    let x = dag.get_register(EVT::I8, 5);
    let divisor = dag.get_constant(6, EVT::I8);
    let rem = dag.get_node(ISD::URem, EVT::I8, &[x, divisor]);
    dag.set_root(rem);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert_ne!(dag.opcode(dag.get_root()), ISD::URem);
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
    divide: 24,
    divide_long: 43,
    margin: 25,
    product: None,
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
