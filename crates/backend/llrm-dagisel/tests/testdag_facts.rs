//! What `TestDag` answers about a node, for the targets whose predicates ask.

use llrm_dagisel::dag::Dag;
use llrm_dagisel::generated::ids::{isd, mvt};
use llrm_dagisel::testing::{Node, TestDag};

/// `masked_value_is_zero` was always false: an or-fold on `and x, 0xFF` could
/// not be tested. A bit the constant clears is zero; a bit it keeps is not.
#[test]
fn an_and_with_a_constant_has_known_zero_bits() {
    let mut dag = TestDag::new();
    let x = dag.input(1, mvt::i32);
    let mask = dag.add(Node { opcode: isd::Constant, vts: vec![mvt::i32], constant: Some(0xFF), ..Node::default() });
    let and = dag.node(isd::AND, &[mvt::i32], &[x, llrm_dagisel::dag::SDValue::new(mask, 0)]);
    let value = llrm_dagisel::dag::SDValue::new(and, 0);

    assert!(dag.masked_value_is_zero(value, 0xFF00));
    assert!(!dag.masked_value_is_zero(value, 0x0F));
    assert!(!dag.masked_value_is_zero(x, 0xFF00));
}

/// `add nuw`: the wrap flag a pattern like wasm's `add_nuw` reads.
#[test]
fn the_unsigned_wrap_flag_is_what_the_node_says() {
    let mut dag = TestDag::new();
    let plain = dag.add(Node { opcode: isd::ADD, vts: vec![mvt::i32], ..Node::default() });
    let nuw = dag.add(Node { opcode: isd::ADD, vts: vec![mvt::i32], no_unsigned_wrap: true, ..Node::default() });

    assert!(!dag.has_no_unsigned_wrap(plain));
    assert!(dag.has_no_unsigned_wrap(nuw));
}
