use super::super::legalize_types::TypeLegalizer;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn counted(op: ISD) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let mut dword = |slot: i32| {
        let info = MemInfo {
            mem_vt: EVT::I32,
            align: 4,
            volatile: false,
            base: MemBase::Stack(slot),
            ext: None,
            space: 0,
            ordering: None,
        };
        dag.get_load(EVT::I32, chain, ptr, info)
    };
    let (lo, hi) = (dword(1), dword(2));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi]);
    let count = dag.get_node(op, EVT::I64, &[x]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[count]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).run().unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// `ctpop i64` at 32 bits: the count of each half, added (an `i64` ctpop
/// stopped sdag-c's bit counts).
#[test]
fn an_i64_ctpop_is_the_sum_of_the_halves_counts() {
    let (dag, root) = counted(ISD::CtPop);
    assert_eq!(dag.opcode(root), ISD::Add);
    for half in [dag.operand(root, 0), dag.operand(root, 1)] {
        assert_eq!((dag.opcode(half), dag.value_type(half)), (ISD::CtPop, EVT::I32));
    }
    assert_ne!(dag.operand(root, 0), dag.operand(root, 1), "low half and high half");
}

/// `ctlz i64`: the high half's count when it is not zero, else the low half's
/// plus 32.
#[test]
fn an_i64_ctlz_selects_between_the_halves() {
    let (dag, root) = counted(ISD::CtLz);
    assert_eq!(dag.opcode(root), ISD::Select);
    let test = dag.operand(root, 0);
    assert_eq!(dag.opcode(test), ISD::SetCC);
    assert_eq!(
        dag.node(dag.operand(test, 2)).data,
        NodeData::CondCode(CondCode::SetNe),
        "the half that is counted first is not zero"
    );
    let (taken, other) = (dag.operand(root, 1), dag.operand(root, 2));
    assert_eq!(dag.opcode(taken), ISD::CtLzZeroPoison, "the high half's count");
    assert_eq!(dag.opcode(other), ISD::Add);
    assert_eq!(dag.constant(dag.operand(other, 1)), Some(32));
}

/// `cttz i64`: the low half's count when it is not zero, else the high half's
/// plus 32.
#[test]
fn an_i64_cttz_selects_between_the_halves() {
    let (dag, root) = counted(ISD::CtTz);
    assert_eq!(dag.opcode(root), ISD::Select);
    let test = dag.operand(root, 0);
    assert_eq!(dag.opcode(test), ISD::SetCC);
    assert_eq!(
        dag.node(dag.operand(test, 2)).data,
        NodeData::CondCode(CondCode::SetNe),
        "the half that is counted first is not zero"
    );
    let (taken, other) = (dag.operand(root, 1), dag.operand(root, 2));
    assert_eq!(dag.opcode(taken), ISD::CtTzZeroPoison, "the low half's count");
    assert_eq!(dag.opcode(other), ISD::Add);
    assert_eq!(dag.constant(dag.operand(other, 1)), Some(32));
}

/// `clz`, `ctz` and `popcount` of an `i64` marked `LibCall` were split into
/// halves anyway (m68k `clzll_ctzll_64` -Os: 216 and 162 B inline against 114 B
/// of shared `__clzdi2`/`__clzsi2`): the pin's `ExpandIntRes_CTPOP` takes the
/// routine when the action says `LibCall` (DEVIATE BC1 extends that to the
/// zero-poison clz and ctz). One call of the `i64` routine, no split; the
/// default still splits.
#[test]
fn a_wide_count_marked_libcall_is_one_call_to_the_wide_routine() {
    use super::super::legalize_dag::Hooks;
    #[derive(Default)]
    struct Calls(Vec<(&'static str, usize)>);
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
            self.0.push((name, args.len()));
            Some(dag.get_register(ret, 90))
        }
    }
    for (op, name) in
        [(ISD::CtPop, "__popcountdi2"), (ISD::CtLzZeroPoison, "__clzdi2"), (ISD::CtTzZeroPoison, "__ctzdi2")]
    {
        for libcall in [true, false] {
            let mut tli = TargetLowering::x86_32(Features::default(), MVT::I32);
            if libcall {
                tli.set_operation_action(op, MVT::I64, Action::LibCall);
            }
            let mut dag = dag();
            let ptr = dag.get_register(EVT::I32, 7);
            let chain = dag.entry_node();
            let info = |slot| MemInfo {
                mem_vt: EVT::I32,
                align: 4,
                volatile: false,
                base: MemBase::Stack(slot),
                ext: None,
                space: 0,
                ordering: None,
            };
            let (lo, hi) = (dag.get_load(EVT::I32, chain, ptr, info(1)), dag.get_load(EVT::I32, chain, ptr, info(2)));
            let x = dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi]);
            let count = dag.get_node(op, EVT::I64, &[x]);
            let low = dag.get_node(ISD::Truncate, EVT::I32, &[count]);
            dag.set_root(low);
            let mut calls = Calls::default();
            TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut calls).run().unwrap();
            if libcall {
                assert_eq!(calls.0, vec![(name, 1)], "{op:?}");
            } else {
                assert!(calls.0.is_empty(), "{op:?} called {:?} by default", calls.0);
            }
        }
    }
}
