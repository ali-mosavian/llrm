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

/// A target that expands the unordered float codes (wasm has `lt`/`le`/`gt`/
/// `ge`/`eq`/`ne` and no `ule`) was refused at `setcc` ("convert node to
/// libcall"): only x86's `oeq` and `une` were translated.
/// `LegalizeSetCCCondCode` swaps the operands, inverts the code, or joins two
/// compares.
#[test]
fn an_unordered_float_code_the_target_expands_is_inverted_swapped_or_split() {
    let f64 = EVT::Simple(MVT::F64);
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    for cc in
        [CondCode::SetULt, CondCode::SetULe, CondCode::SetUGt, CondCode::SetUGe, CondCode::SetUEq, CondCode::SetONe]
    {
        tli.set_cond_code_action(cc, MVT::F64, Action::Expand);
    }
    let compare = |cc: CondCode| {
        let mut dag = dag();
        let (a, b) = (dag.get_register(f64, 1), dag.get_register(f64, 2));
        let setcc = dag.get_setcc(EVT::I8, a, b, cc);
        dag.set_root(setcc);
        DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        let code = |dag: &SelectionDAG, node: SDValue| match dag.node(dag.operand(node, 2)).data {
            NodeData::CondCode(cc) => cc,
            other => panic!("not a code: {other:?}"),
        };
        match dag.opcode(root) {
            ISD::SetCC => (ISD::SetCC, vec![code(&dag, root)]),
            op => {
                let operands = [dag.operand(root, 0), dag.operand(root, 1)];
                (op, operands.iter().filter(|o| dag.opcode(**o) == ISD::SetCC).map(|o| code(&dag, *o)).collect())
            }
        }
    };
    // `ule` is not legal: the inverse, `ogt`, is, and the result is negated.
    assert_eq!(compare(CondCode::SetULe), (ISD::Xor, vec![CondCode::SetOGt]));
    // `ult` swapped is `ugt`, also expanded; its inverse `oge` is legal.
    assert_eq!(compare(CondCode::SetULt), (ISD::Xor, vec![CondCode::SetOGe]));
    // `ueq` has no inverse or swap the target takes: `eq` or unordered.
    assert_eq!(compare(CondCode::SetUEq), (ISD::Or, vec![CondCode::SetEq, CondCode::SetUo]));
}

/// A `UAddO` or `USubO` the target expands was refused ("convert node to
/// libcall", Z80 `overflow_builtins`). `expandUADDSUBO`: the wrapped
/// operation and an unsigned compare of its result with an operand, and for
/// `+ 1` and `+ -1` the compares against 0.
#[test]
fn an_unsigned_overflow_op_the_target_expands_is_the_wrapped_op_and_a_compare() {
    let i16 = EVT::Simple(MVT::I16);
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    for op in [ISD::UAddO, ISD::USubO, ISD::UAddOCarry, ISD::USubOCarry] {
        tli.set_operation_action(op, MVT::I16, Action::Expand);
    }
    // the overflow bit of `op a, b` as (compare code, its left operand's
    // opcode, its right operand is `a`)
    let overflow = |op: ISD, b: Option<u128>| {
        let mut dag = dag();
        let a = dag.get_register(i16, 1);
        let b = match b {
            Some(value) => dag.get_constant(value, i16),
            None => dag.get_register(i16, 2),
        };
        let both = dag.get_node_multi(op, &[i16, EVT::I8], &[a, b]);
        dag.set_root(SDValue { node: both.node, res: 1 });
        DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), ISD::SetCC);
        let NodeData::CondCode(cc) = dag.node(dag.operand(root, 2)).data else { panic!("no code") };
        let (left, right) = (dag.operand(root, 0), dag.operand(root, 1));
        (cc, dag.opcode(left), dag.opcode(right) == ISD::Constant, right == a)
    };
    assert_eq!(overflow(ISD::UAddO, None), (CondCode::SetULt, ISD::Add, false, true));
    assert_eq!(overflow(ISD::USubO, None), (CondCode::SetUGt, ISD::Sub, false, true));
    assert_eq!(overflow(ISD::UAddO, Some(1)), (CondCode::SetEq, ISD::Add, true, false));
    assert_eq!(overflow(ISD::UAddO, Some(0xFFFF)), (CondCode::SetNe, ISD::Register, true, false));
}

/// A `UMulO` the target expands was refused next ("convert node to libcall",
/// Z80 `overflow_builtins`). `expandMULO`: by a power of two a shift and its
/// shift back compared with the operand; otherwise the high half of the
/// product (here `forceExpandWideMUL`, as `i32` is not a type) compared with 0.
#[test]
fn an_unsigned_multiply_with_overflow_the_target_expands_is_a_shift_or_a_high_half() {
    let i16 = EVT::Simple(MVT::I16);
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::UMulO, MVT::I16, Action::Expand);
    let overflow = |b: Option<u128>| {
        let mut dag = dag();
        let a = dag.get_register(i16, 1);
        let b = match b {
            Some(value) => dag.get_constant(value, i16),
            None => dag.get_register(i16, 2),
        };
        let both = dag.get_node_multi(ISD::UMulO, &[i16, EVT::I8], &[a, b]);
        dag.set_root(SDValue { node: both.node, res: 1 });
        DagLegalizer::new(&mut dag, &tli, &mut NoHooks).run().unwrap();
        let root = dag.get_root();
        assert_eq!(dag.opcode(root), ISD::SetCC);
        let NodeData::CondCode(cc) = dag.node(dag.operand(root, 2)).data else { panic!("no code") };
        let (left, right) = (dag.operand(root, 0), dag.operand(root, 1));
        (cc, dag.opcode(left), dag.opcode(right) == ISD::Constant, right == a)
    };
    assert_eq!(overflow(Some(8)), (CondCode::SetNe, ISD::Srl, false, true));
    let (cc, _, zero, _) = overflow(None);
    assert_eq!((cc, zero), (CondCode::SetNe, true));
}

/// A `SAddO` or `SSubO` the target expands was refused next ("convert node to
/// libcall", Z80 `overflow_builtins`). `expandSADDSUBO`: the wrapped operation
/// and two signed compares, of `rhs` with 0 and the sum with `lhs`, or of
/// `lhs` with `rhs` and the difference with 0.
#[test]
fn a_signed_overflow_op_the_target_expands_is_the_wrapped_op_and_two_sign_compares() {
    let i16 = EVT::Simple(MVT::I16);
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    for op in [ISD::SAddO, ISD::SSubO] {
        tli.set_operation_action(op, MVT::I16, Action::Expand);
    }
    // the opcodes reachable from the overflow bit, and the codes of its
    // compares
    let reached = |tli: &TargetLowering, op: ISD| {
        let mut dag = dag();
        let (a, b) = (dag.get_register(i16, 1), dag.get_register(i16, 2));
        let both = dag.get_node_multi(op, &[i16, EVT::I8], &[a, b]);
        dag.set_root(SDValue { node: both.node, res: 1 });
        DagLegalizer::new(&mut dag, tli, &mut NoHooks).run().unwrap();
        let (mut ops, mut codes, mut work) = (Vec::new(), Vec::new(), vec![dag.get_root()]);
        while let Some(node) = work.pop() {
            ops.push(dag.opcode(node));
            if let NodeData::CondCode(cc) = dag.node(node).data {
                codes.push(cc);
            }
            work.extend((0..dag.node(node).ops.len()).map(|at| dag.operand(node, at)));
        }
        (ops, codes)
    };
    // the saturating operation is legal: the sum differs from it
    let (_, codes) = reached(&tli, ISD::SAddO);
    assert_eq!(codes, [CondCode::SetNe]);
    tli.set_operation_action(ISD::SAddSat, MVT::I16, Action::Expand);
    tli.set_operation_action(ISD::SSubSat, MVT::I16, Action::Expand);
    for (op, wrapped) in [(ISD::SAddO, ISD::Add), (ISD::SSubO, ISD::Sub)] {
        let (ops, codes) = reached(&tli, op);
        assert!(ops.contains(&wrapped) && !ops.contains(&op), "{ops:?}");
        assert_eq!(codes.iter().filter(|cc| **cc == CondCode::SetLt).count(), 2, "{codes:?}");
    }
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

/// `clz`, `ctz` and `popcount` marked `LibCall` were refused ("convert node to libcall"): only the arithmetic and
/// float routines were translated, so m68k expanded `clzll` inline (544 B for `clzll_ctzll_64` at -Os). The call takes
/// the value as is, returns `int` and is sign-extended or truncated to the node's type; the default action still
/// expands, with no call.
#[test]
fn a_count_marked_libcall_is_a_call_to_its_routine_and_the_default_is_no_call() {
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
    let counts = [(ISD::CtLzZeroPoison, "__clzsi2"), (ISD::CtPop, "__popcountsi2"), (ISD::CtTzZeroPoison, "__ctzsi2")];
    for libcall in [true, false] {
        let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
        if libcall {
            for (op, _) in counts {
                tli.set_operation_action(op, MVT::I32, Action::LibCall);
            }
        }
        for (op, name) in counts {
            let mut dag = dag();
            let x = dag.get_register(EVT::I32, 1);
            let count = dag.get_node(op, EVT::I32, &[x]);
            dag.set_root(count);
            let mut calls = Calls::default();
            let run = DagLegalizer::new(&mut dag, &tli, &mut calls).run();
            if libcall {
                run.unwrap();
                assert_eq!(calls.0, vec![(name, 1, EVT::I32)], "{op:?}");
            } else {
                assert!(calls.0.is_empty(), "{op:?} called {:?} by default", calls.0);
            }
        }
    }
}
