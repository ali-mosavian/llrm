//! `isIntDivCheap`: where a divide is cheap, `x % 251` stayed a multiply-high
//! sequence (`movzx`, `imul`, `shr`, `imul`, `sub`; 13 instructions against 6,
//! +21..25% clocks on bench/huge at m16 -Os).

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
