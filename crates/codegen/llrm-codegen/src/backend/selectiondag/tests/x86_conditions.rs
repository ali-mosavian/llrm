use super::super::legalize_dag::DagLegalizer;
use super::super::x86_lowering::{X86Lowering, cond, x86isd};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: vt,
        align: 1,
        volatile: false,
        base: MemBase::Stack(slot),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.get_load(vt, chain, ptr, info)
}

fn lowered(
    dag: &mut SelectionDAG,
    cmov: bool,
) {
    let tli = TargetLowering::x86_32(Features { cmov, ..Features::default() }, MVT::I32);
    DagLegalizer::new(dag, &tli, &mut X86Lowering { cmov, sse2: false, min_size: false, i16_mul_chains: false })
        .run()
        .unwrap();
}

fn code(
    dag: &SelectionDAG,
    value: SDValue,
) -> u128 {
    dag.constant(value).expect("a condition code constant")
}

/// `select (a <u b), x, y` is `CMOV y, x, B, flags(a - b)`; the flags are
/// the second result of a `SUB`, so a real subtraction of the same
/// operands shares them. The same DAG with or without CMOV.
#[test]
fn a_select_on_a_compare_is_a_cmov_of_the_subtract_flags() {
    for cmov in [false, true] {
        let mut dag = dag();
        let (a, b, x, y) = (
            load(&mut dag, EVT::I32, 1),
            load(&mut dag, EVT::I32, 2),
            load(&mut dag, EVT::I32, 3),
            load(&mut dag, EVT::I32, 4),
        );
        let test = dag.get_setcc(EVT::I8, a, b, CondCode::SetULt);
        let pick = dag.get_select(EVT::I32, test, x, y);
        dag.set_root(pick);
        lowered(&mut dag, cmov);
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), x86isd::CMOV, "cmov={cmov}");
        assert_eq!((dag.operand(root, 0), dag.operand(root, 1)), (y, x), "false value first");
        assert_eq!(code(&dag, dag.operand(root, 2)), cond::B);
        let flags = dag.operand(root, 3);
        assert_eq!((dag.opcode(flags), flags.res), (x86isd::SUB, 1));
        assert_eq!((dag.operand(flags, 0), dag.operand(flags, 1)), (a, b));
    }
}

/// A compare against zero tests the value (`cmp x, 0`), and `x < 0` is
/// the sign condition: no subtraction is built.
#[test]
fn a_branch_on_a_compare_with_zero_is_a_test() {
    let mut dag = dag();
    let x = load(&mut dag, EVT::I32, 1);
    let zero = dag.get_constant(0, EVT::I32);
    let negative = dag.get_setcc(EVT::I8, x, zero, CondCode::SetLt);
    let chain = dag.entry_node();
    let dest = dag.get_register(EVT::OTHER, 9);
    let branch = dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, negative, dest]);
    dag.set_root(branch);
    lowered(&mut dag, false);
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), x86isd::BRCOND);
    assert_eq!(code(&dag, dag.operand(root, 2)), cond::S);
    let flags = dag.operand(root, 3);
    assert_eq!(dag.opcode(flags), x86isd::CMP);
    assert!(dag.is_null_constant(dag.operand(flags, 1)));
}

/// `a > 5` becomes `a >= 6` (a smaller immediate on the comparison).
#[test]
fn greater_than_a_constant_becomes_greater_or_equal_plus_one() {
    let mut dag = dag();
    let a = load(&mut dag, EVT::I32, 1);
    let five = dag.get_constant(5, EVT::I32);
    let test = dag.get_setcc(EVT::I8, a, five, CondCode::SetGt);
    let (x, y) = (load(&mut dag, EVT::I32, 2), load(&mut dag, EVT::I32, 3));
    let pick = dag.get_select(EVT::I32, test, x, y);
    dag.set_root(pick);
    lowered(&mut dag, false);
    let root = dag.get_root();
    assert_eq!(code(&dag, dag.operand(root, 2)), cond::GE);
    let flags = dag.operand(root, 3);
    assert_eq!(dag.constant(dag.operand(flags, 1)), Some(6));
}

/// A select on a plain `i8` is tested against zero: `NE` on `cmp c, 0`.
#[test]
fn a_select_on_a_plain_byte_tests_it() {
    let mut dag = dag();
    let c = load(&mut dag, EVT::I8, 1);
    let (x, y) = (load(&mut dag, EVT::I32, 2), load(&mut dag, EVT::I32, 3));
    let pick = dag.get_select(EVT::I32, c, x, y);
    dag.set_root(pick);
    lowered(&mut dag, false);
    let root = dag.get_root();
    assert_eq!(code(&dag, dag.operand(root, 2)), cond::NE);
    assert_eq!(dag.opcode(dag.operand(root, 3)), x86isd::CMP);
}

/// `i16` selects widen to a 32-bit `cmov` and truncate; `i8` only when the
/// CPU has CMOV (else the `CMOV_GR8` pseudo takes the byte).
#[test]
fn narrow_selects_widen_by_cpu() {
    for (vt, cmov, widens) in [(EVT::I16, false, true), (EVT::I8, false, false), (EVT::I8, true, true)] {
        let mut dag = dag();
        let c = load(&mut dag, EVT::I8, 1);
        let (x, y) = (dag.get_register(vt, 2), dag.get_register(vt, 3));
        let pick = dag.get_select(vt, c, x, y);
        dag.set_root(pick);
        lowered(&mut dag, cmov);
        let root = dag.get_root();
        assert_eq!(dag.opcode(root) == ISD::Truncate, widens, "{vt:?} cmov={cmov}");
    }
}
