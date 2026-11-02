//! `TypeSoftenFloat`: on a target without an FPU a float operation is a
//! library call on the integers the floats are carried as. The float never
//! reaches the matcher: before softening, `fadd double` was an `FADD` of type
//! `f64` that no instruction of a soft-float target selects.

use super::super::legalize_dag::Hooks;
use super::super::legalize_types::TypeLegalizer;
use super::*;

const F64: EVT = EVT::Simple(MVT::F64);
const F32: EVT = EVT::Simple(MVT::F32);

/// Records each library call and answers with a register of its result type.
#[derive(Default)]
struct Calls(Vec<(&'static str, Vec<EVT>, EVT)>);

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
        self.0.push((name, args.iter().map(|&arg| dag.value_type(arg)).collect(), ret));
        let at = 40 + 2 * self.0.len() as u32;
        // an `i64` result is the pair of words a call returns in two registers
        Some(if ret == EVT::I64 {
            let (lo, hi) = (dag.get_register(EVT::I32, at), dag.get_register(EVT::I32, at + 1));
            dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi])
        } else {
            dag.get_register(ret, at)
        })
    }
}

fn soft() -> TargetLowering {
    let mut tli =
        TargetLowering::empty(TargetLoweringInfo::new(&[MVT::I8, MVT::I16, MVT::I32], false).with_soft_float());
    for (op, vt, name) in [
        (ISD::FAdd, MVT::F64, "__adddf3"),
        (ISD::FSub, MVT::F64, "__subdf3"),
        (ISD::FMul, MVT::F64, "__muldf3"),
        (ISD::FDiv, MVT::F64, "__divdf3"),
        (ISD::FAdd, MVT::F32, "__addsf3"),
        (ISD::FMul, MVT::F32, "__mulsf3"),
    ] {
        tli.set_libcall_name(op, vt, name);
    }
    tli
}

/// A double made of two `i32` registers, as a call's two stack words are.
fn double(
    dag: &mut SelectionDAG,
    at: u32,
) -> SDValue {
    let (lo, hi) = (dag.get_register(EVT::I32, at), dag.get_register(EVT::I32, at + 1));
    let bits = dag.get_node(ISD::BuildPair, EVT::I64, &[lo, hi]);
    dag.get_node(ISD::BitCast, F64, &[bits])
}

fn single(
    dag: &mut SelectionDAG,
    at: u32,
) -> SDValue {
    let bits = dag.get_register(EVT::I32, at);
    dag.get_node(ISD::BitCast, F32, &[bits])
}

/// `fadd double` was never made legal; it is `__adddf3` on the two `i64`s and
/// the call's `i64` result is what a bitcast back reads.
#[test]
fn a_double_add_is_a_call_on_two_i64() {
    let tli = soft();
    let mut dag = dag();
    let (a, b) = (double(&mut dag, 1), double(&mut dag, 3));
    let sum = dag.get_node(ISD::FAdd, F64, &[a, b]);
    let bits = dag.get_node(ISD::BitCast, EVT::I64, &[sum]);
    dag.set_root(bits);
    let mut calls = Calls::default();
    TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut calls).run().unwrap();
    assert_eq!(calls.0, vec![("__adddf3", vec![EVT::I64, EVT::I64], EVT::I64)]);
}

/// A float is `i32` wide: `fmul float` is `__mulsf3` on two `i32`.
#[test]
fn a_float_multiply_is_a_call_on_two_i32() {
    let tli = soft();
    let mut dag = dag();
    let (a, b) = (single(&mut dag, 1), single(&mut dag, 2));
    let product = dag.get_node(ISD::FMul, F32, &[a, b]);
    let bits = dag.get_node(ISD::BitCast, EVT::I32, &[product]);
    dag.set_root(bits);
    let mut calls = Calls::default();
    TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut calls).run().unwrap();
    assert_eq!(calls.0, vec![("__mulsf3", vec![EVT::I32, EVT::I32], EVT::I32)]);
}

/// The result of one call is the operand of the next: a chain of float
/// operations is a chain of calls, each on the integers the last made.
#[test]
fn a_chain_of_float_operations_is_a_chain_of_calls() {
    let tli = soft();
    let mut dag = dag();
    let (a, b, c) = (double(&mut dag, 1), double(&mut dag, 3), double(&mut dag, 5));
    let sum = dag.get_node(ISD::FAdd, F64, &[a, b]);
    let quotient = dag.get_node(ISD::FDiv, F64, &[sum, c]);
    let bits = dag.get_node(ISD::BitCast, EVT::I64, &[quotient]);
    dag.set_root(bits);
    let mut calls = Calls::default();
    TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut calls).run().unwrap();
    let names: Vec<_> = calls.0.iter().map(|call| call.0).collect();
    assert_eq!(names, ["__adddf3", "__divdf3"]);
}
