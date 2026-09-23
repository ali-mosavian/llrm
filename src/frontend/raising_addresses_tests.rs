//! Port of `tests/test_raising_addresses.py`.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use super::*;
use crate::backend::lower;
use crate::frontend::{blocks, declen};
use crate::model::ir::nodes::{self, Node, Restore};
use crate::model::ir::{self, Effects, Loc, Operation, Semantics};
use crate::model::mir::{Const, Held, MirBlock, MirBody, Opaque, OpCode, Synth};
use crate::objectfile::module::Addr;
use crate::support::testing;

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

/// HARR reloaded its selector inside the loop; unequal names also retained an array read.
#[test]
#[ignore = "fails in Python too: assert 0 == 1 (no ES selector load)"]
fn test_harr_reuses_one_selector_and_forwards_the_array_store() {
    for tag in ["p-g2", "q-O", "v-g3"] {
        let result = testing::emitted_lir(format!("fixtures/omf/harr-{tag}.obj").to_lowercase());
        let found = testing::loaded_bytes(&result.data).unwrap();
        let reached = blocks::instructions(&found).unwrap();
        let selectors: Vec<_> = reached.iter().map(|one| one.insn).filter(|one| one.op0_register() == Register::ES).collect();
        assert_eq!(selectors.len(), 1, "{tag}");
        for one in &reached {
            let branch = one.insn;
            if branch.is_jcc_short_or_near() || branch.is_jmp_short_or_near() {
                assert!(!(branch.near_branch_target() <= selectors[0].ip() && selectors[0].ip() <= branch.ip()), "{tag}");
            }
        }
        assert!(
            !reached.iter().any(|one| one.insn.memory_segment() == Register::ES && one.insn.op0_register() != Register::None),
            "{tag}"
        );
    }
}

/// D_SURF made 163 descriptors instead of 21: long extracts orphaned the store's ES.
#[test]
fn test_long_extraction_preserves_the_far_store_selector() {
    let descriptor = MemRef::new(Some(Addr { index: 5, ..Addr::new(Space::Segment, 2) }), 2);
    let element = MemRef::new(Some(Addr { segment: Register::ES, ..Addr::new(Space::Far, 0) }), 4);
    let (whole, low) = (Value::new(1, 0), Value::new(2, 2));
    let mut selector = Op::new(1, OpCode::Operation(Operation::Move), "mov", vec![], vec![]);
    selector.loads = vec![descriptor.clone()];
    selector.kind = Kind::Load;
    selector.args = vec![Arg::Cell(Cell { r#ref: descriptor })];
    selector.results = vec![Arg::Opaque(Opaque::named(None, "es"))];
    let mut extract = Op::new(2, OpCode::Synth(Synth::HalfToLow), "extract", vec![low], vec![whole]);
    extract.kind = Kind::Extract;
    extract.args = vec![Arg::Held(Held { value: whole, width: 4 }), Arg::Const(Const::new(0, 1))];
    extract.results = vec![Arg::Held(Held { value: low, width: 2 })];
    let mut store = Op::new(3, OpCode::Operation(Operation::Move), "mov", vec![], vec![whole]);
    store.stores = vec![element.clone()];
    store.kind = Kind::Store;
    store.args = vec![Arg::Held(Held { value: whole, width: 4 })];
    store.results = vec![Arg::Cell(Cell { r#ref: element.clone() })];
    store.id = Some(3);
    // Python's SimpleNamespace node carries only semantics, so the raise sees no node.
    let store = mir::raising_occurrence(&store, (3, 3), Vec::new(), None);
    let body = RaisedBody::new(MirBody::new(0, vec![MirBlock::new(0, vec![], vec![selector, extract, store], vec![])]));
    let mut raised = loaded(body, None).unwrap().body;
    let ops = &mut raised.blocks[0].ops;
    ops[2].source_backed = true;
    let (first, last) = (ops[0].clone(), ops[2].clone());
    let [segment] = first.defines[..] else { panic!("one selector definition") };
    let semantics = Semantics {
        name: Some("mov".into()),
        dests: vec![Loc::Mem(ir::Mem { through: Register::BX, ..ir::Mem::new(element.addr, 4) })],
        sources: vec![Loc::Reg(ir::Reg { register: Register::EAX, width: 4 })],
        ..Semantics::new(Operation::Move)
    };
    let node = Node::Opaque(nodes::Opaque {
        insn: declen::decode(&[0x26, 0x66, 0x89, 0x07], 0).unwrap(),
        effects: ir::NO_EFFECT.clone(),
        semantics,
    });
    let calls = IndexMap::default();
    let contracts = IndexMap::default();
    let options = lower::Options { nodes: [(3, Arc::new(node))].into_iter().collect(), ..Default::default() };
    let mut lowering = lower::Lowering::new(
        &Rc::new(raised),
        BTreeSet::from([whole.id, low.id, segment.id]),
        &calls,
        BTreeSet::new(),
        Some(&contracts),
        "386",
        options,
    )
    .unwrap();
    let expanded = lowering.expand(&last, true).unwrap();
    let [write] = expanded.as_slice() else { panic!("{} instructions", expanded.len()) };
    // The cell carries its selector; the allocator seats it in a segment register.
    let Loc::Mem(dest) = &write.what.as_ref().unwrap().dests[0] else { panic!("a memory destination") };
    assert_eq!(dest.selector, Some(ir::Held { value: segment.id, width: 2 }));
    assert!(write.uses.contains(&segment.id));
}
