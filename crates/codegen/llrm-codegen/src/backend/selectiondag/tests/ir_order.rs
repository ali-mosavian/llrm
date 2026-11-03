//! IR order of the nodes the pin makes without an `SDLoc`.

use super::*;

/// A constant made while an instruction is visited carries IR order 0, as the
/// pin's `ConstantSDNode` takes no `SDLoc`; the scheduler ranks it by that.
/// It took the instruction's order: wasm's `irreducible.ll` then scheduled
/// differently from the pin's.
#[test]
fn a_constant_has_no_ir_order() {
    let mut dag = dag();
    dag.cur_order = 7;
    let x = dag.get_register(EVT::I32, 5);
    let c = dag.get_constant(3, EVT::I32);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[x, c]);
    let order = |v: SDValue| dag.nodes()[v.node as usize].order;
    assert_eq!((order(c), order(sum)), (0, 7));
}
