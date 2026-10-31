use super::super::legalize_dag::{DagLegalizer, Hooks, NoHooks};
use super::super::legalize_types::LegalizeError;
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

/// With `Add` on `i8` marked Promote and nothing doing it, the add
/// reaches the matcher in a type it has no instruction for. After: a
/// truncate of an `i16` add of any-extended operands.
#[test]
fn a_promoted_add_is_made_wide_and_truncated() {
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::Add, MVT::I8, Action::Promote);
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::I8, 1), load(&mut dag, EVT::I8, 2));
    let sum = dag.get_node(ISD::Add, EVT::I8, &[a, b]);
    dag.set_root(sum);
    DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Truncate, EVT::I8));
    let wide = dag.operand(root, 0);
    assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::Add, EVT::I16));
    let (l, r) = (dag.operand(wide, 0), dag.operand(wide, 1));
    assert_eq!((dag.opcode(l), dag.opcode(r)), (ISD::AnyExtend, ISD::AnyExtend));
    assert_ne!(l, r);
}

/// A node the table says Expand, with no expansion translated, is refused
/// by name and not left in the DAG.
#[test]
fn an_expand_with_no_case_is_refused_by_name() {
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::Sub, MVT::I32, Action::Expand);
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let diff = dag.get_node(ISD::Sub, EVT::I32, &[a, b]);
    dag.set_root(diff);
    let error = DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap_err();
    assert!(matches!(error, LegalizeError::Unsupported { op: ISD::Sub, .. }), "{error:?}");
}

/// Legal nodes are left alone: no node is added.
#[test]
fn legal_nodes_are_untouched() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (a, b) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2));
    let sum = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
    dag.set_root(sum);
    let before = dag.node_count();
    DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
    assert_eq!(dag.node_count(), before);
    assert_eq!(dag.get_root(), sum);
}

/// Users are legalized before their operands, so a custom lowering sees
/// its operand as built: `mul(mul(a,b),c)` is lowered outer first and
/// the outer still reads a `mul`. Walking operands first showed it an
/// `add` already.
#[test]
fn a_custom_lowering_sees_its_operands_as_built() {
    struct MulToAdd(Vec<ISD>);
    impl Hooks for MulToAdd {
        fn lower_operation(
            &mut self,
            dag: &mut SelectionDAG,
            node: SDValue,
        ) -> Option<SDValue> {
            let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
            self.0.push(dag.opcode(lhs));
            Some(dag.get_node(ISD::Add, EVT::I32, &[lhs, rhs]))
        }
    }
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::Mul, MVT::I32, Action::Custom);
    let mut dag = dag();
    let (a, b, c) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2), load(&mut dag, EVT::I32, 3));
    let inner = dag.get_node(ISD::Mul, EVT::I32, &[a, b]);
    let outer = dag.get_node(ISD::Mul, EVT::I32, &[inner, c]);
    dag.set_root(outer);
    let mut hooks = MulToAdd(Vec::new());
    DagLegalizer::new(&mut dag, &tli, &mut hooks).run().unwrap();
    assert_eq!(hooks.0, vec![ISD::Mul, ISD::Load], "outer sees the inner mul; inner sees a load");
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Add);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Add, "the inner one was lowered too");
}

/// `1 << x` with an `i32` amount (the bit-test switch makes it) reached
/// selection with the amount in the wrong type, and the shift patterns, which
/// take an `i8` count, never matched: three `shl` left unselected in the 6-case
/// switch. The pin adjusts it in `LegalizeOp`.
#[test]
fn a_shift_amount_is_made_an_i8() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (one, amount) = (dag.get_constant(1, EVT::I32), load(&mut dag, EVT::I32, 1));
    let shifted = dag.get_node(ISD::Shl, EVT::I32, &[one, amount]);
    dag.set_root(shifted);
    DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Shl);
    assert_eq!(dag.value_type(dag.operand(root, 1)), EVT::I8);
}

/// `frem`, `fsin` and `lrint` marked `LibCall` were refused ("convert node to
/// libcall"): only the two-operand integer routines were translated. The call
/// takes every operand of the node, and `lrint` of a double returns an integer.
#[test]
fn a_float_routine_is_a_libcall_with_all_its_operands() {
    #[derive(Default)]
    struct Calls(Vec<(&'static str, usize, EVT)>);
    impl Hooks for Calls {
        fn lower_operation(
            &mut self,
            _dag: &mut SelectionDAG,
            _node: SDValue,
        ) -> Option<SDValue> {
            None
        }
        fn make_lib_call(
            &mut self,
            dag: &mut SelectionDAG,
            name: &'static str,
            args: &[SDValue],
            ret: EVT,
            _signed: bool,
        ) -> Option<SDValue> {
            self.0.push((name, args.len(), ret));
            Some(dag.get_register(ret, 40 + self.0.len() as u32))
        }
    }
    let f64 = EVT::Simple(MVT::F64);
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    for (op, name) in [(ISD::FRem, "fmod"), (ISD::FSin, "sin"), (ISD::LRint, "lrint")] {
        tli.set_operation_action(op, MVT::F64, Action::LibCall);
        tli.set_libcall_name(op, MVT::F64, name);
    }
    let mut dag = dag();
    let (a, b) = (dag.get_register(f64, 1), dag.get_register(f64, 2));
    let rem = dag.get_node(ISD::FRem, f64, &[a, b]);
    let sin = dag.get_node(ISD::FSin, f64, &[rem]);
    let rint = dag.get_node(ISD::LRint, EVT::I32, &[sin]);
    dag.set_root(rint);
    let mut calls = Calls::default();
    DagLegalizer::new(&mut dag, &tli, &mut calls).run().unwrap();
    assert_eq!(calls.0, vec![("lrint", 1, EVT::I32), ("sin", 1, f64), ("fmod", 2, f64)]);
}
