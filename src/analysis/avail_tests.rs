//! Port of tests/test_avail.py, tests/test_availability_call_effects.py,
//! tests/test_availability_operands.py and the avail half of
//! tests/test_memoryssa_forward.py.
//!
//! Waiting for the corpus loader, `ir.decode_module` and `mir.raise_body`:
//! `test_the_map_reaches_a_fixed_point`,
//! `test_no_entry_survives_a_store_that_could_reach_it`,
//! `test_an_accumulate_is_not_a_provider`,
//! `test_a_runtime_name_does_not_replace_missing_effect_proofs`,
//! `test_a_live_provider_is_never_the_value_the_read_defines`,
//! `test_no_stack_slot_crosses_a_block_boundary`,
//! `test_a_stack_slot_never_survives_a_call`,
//! `test_dropping_a_redundant_load_leaves_no_use_without_a_definition`.
//!
//! Waiting for `transform.forwarded`: the transform assertions of
//! `test_preheader_store_serves_a_loop_read` and
//! `test_aliasing_backedge_keeps_the_load`, and all of
//! `test_call_with_proven_disjoint_writes_preserves_the_loop_value`,
//! `test_call_exclusion_must_cover_the_entire_read`,
//! `test_escaped_object_origin_does_not_exclude_an_interior_read`,
//! `test_preheader_load_serves_loop_reads_across_disjoint_writes`,
//! `test_preheader_load_cannot_survive_an_aliasing_backedge`,
//! `test_load_on_only_one_entry_path_is_not_available`.

use std::collections::BTreeSet;

use iced_x86::Register;
use crate::support::hash::IndexMap;

use super::*;
use crate::model::ir::{Loc, Mem, Operation, Reg, Semantics};
use crate::model::mir::{Cell, Held as MirHeld, MirBlock, OpCode, kind_of};
use crate::objectfile::module::Addr;

fn addr(space: Space, disp: i64, index: i64) -> Addr {
    Addr { index, ..Addr::new(space, disp) }
}

fn op(at: i64, code: Operation, defines: Vec<Value>, uses: Vec<Value>, kind: Kind) -> Op {
    let mut op = Op::new(at, OpCode::Operation(code), "", defines, uses);
    op.kind = kind;
    op
}

fn calls(pairs: &[(i64, &str)]) -> IndexMap<i64, String> {
    pairs.iter().map(|(at, name)| (*at, (*name).to_owned())).collect()
}

// ---- tests/test_avail.py ----

/// Removed HARY sites killed frame facts; real unknown calls must still invalidate them.
#[test]
fn test_current_mir_decides_whether_a_call_invalidates_memory() {
    for real_call in [false, true] {
        for metadata in [false, true] {
            let r#ref = MemRef::new(Some(Addr::new(Space::Frame, -20)), 2);
            let value = Value::new(1, 0);
            let call = if real_call {
                op(10, Operation::Call, vec![], vec![], Kind::Call)
            } else {
                op(10, Operation::Nothing, vec![], vec![], Kind::Nothing)
            };
            let held: Holders = IndexMap::from_iter([(r#ref, Holder::Value(value))]);
            let calls = if metadata { calls(&[(10, "B$HARY")]) } else { IndexMap::default() };
            let expected = if real_call { IndexMap::default() } else { held.clone() };
            assert_eq!(_after(&call, held.clone(), None, &calls, None), expected);
        }
    }
}

/// The guard itself, on the two shapes it has to separate.
#[test]
fn test_preserved_allows_a_move_and_refuses_a_binary() {
    let old = Value::new(1, 0x100);
    let new = Value::new(2, 0x100);
    let cell = MemRef::new(Some(addr(Space::Literal, 0, 0)), 2);
    let r#where = Loc::Mem(Mem::new(Some(addr(Space::Literal, 0, 0)), 2));
    let into = Loc::Reg(Reg { register: Register::AX, width: 2 });

    let make = |what: Semantics, args: Vec<Arg>, results: Vec<Arg>| -> Op {
        let mut op = Op::new(0x100, OpCode::Operation(what.op), what.name.clone().unwrap_or_default(), vec![new], vec![old]);
        op.loads = vec![cell.clone()];
        op.kind = kind_of(&what, &args, &results);
        op.args = args;
        op.results = results;
        op.raised = Some((vec![], vec![]));
        op
    };

    let moved = make(
        Semantics {
            name: Some("mov".into()),
            dests: vec![into.clone()],
            sources: vec![r#where.clone()],
            ..Semantics::new(Operation::Move)
        },
        vec![Arg::Cell(Cell { r#ref: cell.clone() })],
        vec![Arg::Held(MirHeld { value: new, width: 2 })],
    );
    assert_eq!(_preserved(&moved), BTreeSet::from([old]), "a move's read of its own destination is the high half");

    let accumulated = make(
        Semantics {
            name: Some("sub".into()),
            dests: vec![into.clone()],
            sources: vec![into, r#where],
            ..Semantics::new(Operation::Binary)
        },
        vec![Arg::Held(MirHeld { value: old, width: 2 }), Arg::Cell(Cell { r#ref: cell.clone() })],
        vec![Arg::Held(MirHeld { value: new, width: 2 })],
    );
    assert_eq!(_preserved(&accumulated), BTreeSet::new(), "a subtract reads its destination as data");
    assert_eq!(loaded_into(&accumulated), None, "and so is not a load");
}

// ---- tests/test_availability_call_effects.py ----

fn body_with_call(effect: MemRef, complete: bool) -> MirBody {
    let cell = MemRef::new(Some(addr(Space::Segment, 16, 1)), 2);
    let (source, result) = (Value::new(1, 0), Value::new(2, 2));
    let mut store = op(0, Operation::Move, vec![], vec![source], Kind::Store);
    store.args = vec![Arg::Held(MirHeld { value: source, width: 2 })];
    store.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    store.stores = vec![cell.clone()];
    let mut call = op(1, Operation::Call, vec![], vec![], Kind::Call);
    call.loads = vec![effect.clone()];
    call.stores = vec![effect];
    call.memory_complete = complete;
    let mut load = op(2, Operation::Move, vec![result], vec![], Kind::Load);
    load.args = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    load.results = vec![Arg::Held(MirHeld { value: result, width: 2 })];
    load.loads = vec![cell];
    MirBody::new(0, vec![MirBlock::new(0, vec![], vec![store, call, load], vec![])])
}

fn beyond(owner: i64) -> MemRef {
    let mut effect = MemRef::new(None, 0);
    effect.beyond = Some((owner, BTreeSet::new()));
    effect
}

#[test]
fn test_runtime_name_cannot_override_unknown_mir_effects() {
    let body = body_with_call(MemRef::new(None, 0), false);
    assert!(forwardable(&body, None, &calls(&[(1, "B$MUI4")]), &BTreeSet::from([2])).is_empty());
}

#[test]
fn test_disjoint_mir_effects_keep_values_without_runtime_names() {
    let body = body_with_call(beyond(1), true);
    let forwarded = forwardable(&body, None, &IndexMap::default(), &BTreeSet::from([2]));
    assert_eq!(forwarded.len(), 1);
    assert_eq!(forwarded[0].value, Holder::Value(body.blocks[0].ops[0].uses[0]));
}

#[test]
fn test_dead_store_uses_call_memory_effects() {
    for disjoint in [false, true] {
        let effect = if disjoint { beyond(1) } else { MemRef::new(None, 0) };
        let mut body = body_with_call(effect, true);
        let mut again = body.blocks[0].ops[0].clone();
        again.at = 2;
        body.blocks[0].ops[2] = again;
        let calls = if disjoint { IndexMap::default() } else { calls(&[(1, "B$MUI4")]) };
        let removed = dead_stores(&body, None, &calls, None, None, true);
        assert_eq!(!removed.is_empty(), disjoint);
    }
}

#[test]
fn test_opaque_memory_footprint_preserves_only_disjoint_values() {
    // A known status-word store may preserve another local, never its own
    // old contents; a barrier with no verified footprint remains unknown.
    for (complete, overlap, reused) in [(false, false, false), (true, false, true), (true, true, false)] {
        let effect = MemRef::new(Some(addr(Space::Segment, if overlap { 16 } else { 18 }, 1)), 2);
        let mut body = body_with_call(effect, false);
        let barrier = &mut body.blocks[0].ops[1];
        barrier.op = Some(OpCode::Operation(Operation::Barrier));
        barrier.kind = Kind::Opaque;
        barrier.memory_complete = complete;
        assert_eq!(!forwardable(&body, None, &IndexMap::default(), &BTreeSet::from([2])).is_empty(), reused);
    }
}

/// A setter with a complete disjoint footprint used to read all memory.
#[test]
fn test_complete_write_only_call_does_not_make_prior_store_observable() {
    let mut body = body_with_call(MemRef::new(Some(addr(Space::Segment, 18, 1)), 2), false);
    let call = &mut body.blocks[0].ops[1];
    call.loads = vec![];
    call.memory_complete = true;
    let mut again = body.blocks[0].ops[0].clone();
    again.at = 2;
    body.blocks[0].ops[2] = again;

    let removed = dead_stores(&body, None, &IndexMap::default(), None, None, true);
    assert_eq!(removed, vec![&body.blocks[0].ops[0]]);
    assert!(std::ptr::eq(removed[0], &body.blocks[0].ops[0]));
}

#[test]
fn test_an_unknown_pointer_does_not_observe_a_private_cell() {
    // procs' TWICE kept two dead frame stores once its return read through UNKNOWN provenance.
    use crate::model::memory::{MemoryKind, MemoryObject, Provenance};

    let body = body_with_call(MemRef::new(None, 0), false);
    let (mut store, mut load) = (body.blocks[0].ops[0].clone(), body.blocks[0].ops[2].clone());
    let cell = MemRef::new(Some(addr(Space::Frame, -0x18, 0)), 2);
    let mut unknown = MemRef::new(None, 2);
    unknown.provenance = Some(Provenance::one(MemoryObject::new(MemoryKind::Unknown)));
    store.results = vec![Arg::Cell(Cell { r#ref: cell.clone() })];
    store.stores = vec![cell.clone()];
    load.args = vec![Arg::Cell(Cell { r#ref: unknown.clone() })];
    load.loads = vec![unknown];
    let mut body = body;
    body.blocks[0].ops = vec![store, load];

    let private = |one: &MemRef| *one == cell;
    let removed = dead_stores(&body, None, &IndexMap::default(), Some(&private), None, true);
    assert_eq!(removed, vec![&body.blocks[0].ops[0]]);
}

// ---- tests/test_availability_operands.py ----

#[test]
fn test_explicit_load_operand_is_not_a_preserved_half() {
    // avail.Held shadowed mir.Held, hiding every explicitly read SSA operand.
    let source = Value::new(1, 0);
    let result = Value::new(2, 1);
    let cell = MemRef::new(Some(addr(Space::Literal, 0, 0)), 2);
    let mut load = op(1, Operation::Move, vec![result], vec![source], Kind::Load);
    load.loads = vec![cell.clone()];
    load.args = vec![Arg::Cell(Cell { r#ref: cell }), Arg::Held(MirHeld { value: source, width: 2 })];
    load.results = vec![Arg::Held(MirHeld { value: result, width: 2 })];
    assert_eq!(_preserved(&load), BTreeSet::new());
    assert_eq!(loaded_into(&load), None);
}

// ---- tests/test_memoryssa_forward.py ----

fn loop_body(alias: bool) -> MirBody {
    let cell = MemRef::new(Some(addr(Space::Segment, 0x20, 1)), 2);
    let other = if alias { cell.clone() } else { MemRef::new(Some(addr(Space::Segment, 0x30, 1)), 2) };
    let (source, result) = (Value::new(1, 0), Value::new(2, 1));
    let mut store = op(0, Operation::Move, vec![], vec![source], Kind::Store);
    store.stores = vec![cell.clone()];
    let mut load = op(1, Operation::Move, vec![result], vec![], Kind::Load);
    load.loads = vec![cell.clone()];
    load.args = vec![Arg::Cell(Cell { r#ref: cell })];
    let mut write = op(2, Operation::Move, vec![], vec![], Kind::Store);
    write.stores = vec![other];
    MirBody::new(
        0,
        vec![
            MirBlock::new(0, vec![], vec![store], vec![1]),
            MirBlock::new(1, vec![], vec![load], vec![2, 3]),
            MirBlock::new(2, vec![], vec![write], vec![1]),
            MirBlock::new(3, vec![], vec![], vec![]),
        ],
    )
}

#[test]
fn test_preheader_store_serves_a_loop_read() {
    let body = loop_body(false);
    let found = forwardable(&body, None, &IndexMap::default(), &BTreeSet::from([1]));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].value, Holder::Value(body.blocks[0].ops[0].uses[0]));
}

#[test]
fn test_aliasing_backedge_keeps_the_load() {
    let body = loop_body(true);
    assert!(forwardable(&body, None, &IndexMap::default(), &BTreeSet::from([1])).is_empty());
}

#[test]
fn test_store_on_only_one_entry_path_cannot_supply_the_load() {
    let mut body = loop_body(false);
    body.entry = 4;
    body.blocks.push(MirBlock::new(4, vec![], vec![], vec![0, 1]));
    assert!(forwardable(&body, None, &IndexMap::default(), &BTreeSet::from([1])).is_empty());
}

#[test]
fn test_call_on_backedge_invalidates_the_preheader_store() {
    let mut body = loop_body(false);
    body.blocks[2].ops = vec![op(2, Operation::Call, vec![], vec![], Kind::Call)];
    assert!(forwardable(&body, None, &IndexMap::default(), &BTreeSet::from([1])).is_empty());
}
