//! Port of `tests/test_raising_addresses.py`.
//!
//! Skipped, needing `wholeseg`: `test_harr_reuses_one_selector_and_forwards_the_array_store`.
//! Skipped, needing `backend.lower`: `test_long_extraction_preserves_the_far_store_selector`.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::model::ir::nodes::{Node, Restore};
use crate::model::ir::{Effects, Operation};
use crate::model::mir::{MirBlock, MirBody, Opaque, OpCode};
use crate::objectfile::module::Addr;

#[test]
fn test_a_clobber_ends_the_raised_selector_dependency() {
    let descriptor = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 2) }), 2);
    let element = MemRef::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) }), 2);
    let mut selector = Op::new(0, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
    selector.loads = vec![descriptor.clone()];
    selector.kind = Kind::Load;
    selector.args = vec![Arg::Cell(Cell { r#ref: descriptor })];
    selector.results = vec![Arg::Opaque(Opaque::named(None, "es"))];
    let store = |at: i64| {
        // Python's SimpleNamespace node: effects reading ES and writing nothing.
        let effects = Effects { uses: Some(BTreeSet::from([Register::ES])), defs: Some(BTreeSet::new()), ..Effects::no_effect() };
        let node = Node::Restore(Restore::new(at as usize, at as usize, 0, effects));
        let mut made = Op::new(at, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
        made.stores = vec![element.clone()];
        made.kind = Kind::Store;
        made.results = vec![Arg::Cell(Cell { r#ref: element.clone() })];
        mir::raising_occurrence(&made, (at, at), Vec::new(), Some(Arc::new(node)))
    };
    let mut clobber = Op::new(2, OpCode::Operation(Operation::Call), "call", vec![], vec![]);
    clobber.kind = Kind::Call;
    let body = RaisedBody::new(MirBody::new(
        0,
        vec![MirBlock::new(0, vec![], vec![selector, store(1), clobber, store(3)], vec![])],
    ));
    let raised = loaded(body, None).unwrap().blocks[0].ops.clone();
    let [value] = raised[0].defines[..] else {
        panic!("one selector definition");
    };
    assert_eq!(raised[1].stores[0].segment, Some(value));
    assert!(raised[1].uses.contains(&value));
    assert!(raised[2].uses.contains(&value));
    let after = raised[3].stores[0].segment.unwrap();
    assert!(
        after != value && raised[2].defines.contains(&after),
        "the store after the call still names the old selector"
    );
}
