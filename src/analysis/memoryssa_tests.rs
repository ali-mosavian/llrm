//! Port of tests/test_memoryssa.py.

use std::collections::BTreeSet;

use super::*;
use crate::model::ir::Operation;
use crate::model::mir::{MirBlock, OpCode};
use crate::objectfile::module::{Addr, Space};

fn addr(space: Space, disp: i64, index: i64) -> Addr {
    Addr { index, ..Addr::new(space, disp) }
}

fn cell() -> MemRef {
    MemRef::new(Some(addr(Space::Segment, 0x20, 1)), 2)
}

fn site(block: i64, index: usize) -> Site {
    Site { block, index }
}

fn operation(at: i64, loads: Vec<MemRef>, stores: Vec<MemRef>, barrier: bool) -> Op {
    let code = if barrier { Operation::Barrier } else { Operation::Move };
    let mut op = Op::new(at, OpCode::Operation(code), "", vec![], vec![]);
    op.kind = if barrier {
        mir::Kind::Opaque
    } else if !loads.is_empty() && stores.is_empty() {
        mir::Kind::Load
    } else {
        mir::Kind::Store
    };
    op.loads = loads;
    op.stores = stores;
    op
}

fn load(at: i64) -> Op {
    operation(at, vec![cell()], vec![], false)
}

fn store(at: i64, one: MemRef) -> Op {
    operation(at, vec![], vec![one], false)
}

fn call(at: i64) -> Op {
    let mut op = Op::new(at, OpCode::Operation(Operation::Call), "", vec![], vec![]);
    op.kind = mir::Kind::Call;
    op
}

fn block(at: i64, ops: Vec<Op>, succ: Vec<i64>) -> MirBlock {
    MirBlock::new(at, vec![], ops, succ)
}

fn incoming(access: &Access) -> Vec<(Option<i64>, usize)> {
    let mut edges = access.incoming.clone();
    edges.sort();
    edges
}

#[test]
fn test_each_join_edge_retains_its_own_stored_value() {
    let body = MirBody::new(
        0,
        vec![
            block(0, vec![], vec![1, 2]),
            block(1, vec![store(1, cell())], vec![3]),
            block(2, vec![store(2, cell())], vec![3]),
            block(3, vec![load(3)], vec![]),
        ],
    );
    let graph = built(&body);
    for parent in [1, 2] {
        assert!(graph.available_on_edge(site(parent, 0), site(3, 0), parent, &cell(), None, None));
        assert!(!graph.available_on_edge(site(3 - parent, 0), site(3, 0), parent, &cell(), None, None));
    }
}

#[test]
fn test_a_load_uses_the_nearest_memory_definition() {
    let body = MirBody::new(0, vec![block(0, vec![store(0, cell()), load(1)], vec![])]);

    let graph = built(&body);
    let write = graph.at(site(0, 0));
    let read = graph.at(site(0, 1));

    assert_eq!(write.kind, Kind::Def);
    assert_eq!(write.defining, Some(graph.live.id));
    assert_eq!(read.kind, Kind::Use);
    assert_eq!(read.defining, Some(write.id));
}

#[test]
fn test_a_join_gets_one_memory_phi() {
    let body = MirBody::new(
        0,
        vec![
            block(0, vec![], vec![1, 2]),
            block(1, vec![store(1, cell())], vec![3]),
            block(2, vec![store(2, cell())], vec![3]),
            block(3, vec![load(3)], vec![]),
        ],
    );

    let graph = built(&body);
    let phi = &graph.phis[&3];

    assert_eq!(phi.kind, Kind::Phi);
    assert_eq!(incoming(phi), vec![(Some(1), graph.at(site(1, 0)).id), (Some(2), graph.at(site(2, 0)).id)]);
    assert_eq!(graph.at(site(3, 0)).defining, Some(phi.id));
}

#[test]
fn test_a_loop_header_phi_carries_the_backedge_definition() {
    let body = MirBody::new(
        0,
        vec![
            block(0, vec![], vec![1]),
            block(1, vec![load(1)], vec![2, 3]),
            block(2, vec![store(2, cell())], vec![1]),
            block(3, vec![], vec![]),
        ],
    );

    let graph = built(&body);
    let phi = &graph.phis[&1];

    assert_eq!(incoming(phi), vec![(Some(0), graph.live.id), (Some(2), graph.at(site(2, 0)).id)]);
    assert_eq!(graph.at(site(1, 0)).defining, Some(phi.id));
}

#[test]
fn test_an_opaque_barrier_defines_memory_even_without_named_cells() {
    let body = MirBody::new(0, vec![block(0, vec![operation(0, vec![], vec![], true)], vec![])]);

    let graph = built(&body);

    assert_eq!(graph.at(site(0, 0)).kind, Kind::Def);
}

#[test]
fn test_a_call_with_no_named_cells_still_defines_memory() {
    let body = MirBody::new(0, vec![block(0, vec![call(0), load(1)], vec![])]);
    let graph = built(&body);
    assert_eq!(graph.at(site(0, 0)).kind, Kind::Def);
    assert_eq!(graph.at(site(0, 1)).defining, Some(graph.at(site(0, 0)).id));
}

/// A proven readonly callee used to become an unknown MemoryDef anyway.
#[test]
fn test_a_complete_read_only_call_uses_but_does_not_define_memory() {
    let mut read_only = call(0);
    read_only.loads = vec![cell()];
    read_only.memory_complete = true;
    let body = MirBody::new(0, vec![block(0, vec![read_only, load(1)], vec![])]);

    let graph = built(&body);

    assert_eq!(graph.at(site(0, 0)).kind, Kind::Use);
    assert_eq!(graph.at(site(0, 1)).defining, Some(graph.live.id));
}

/// A pure call is still a value/control operation, but not a memory version.
#[test]
fn test_a_complete_memory_free_call_has_no_memoryssa_access() {
    let mut pure = call(0);
    pure.memory_complete = true;
    let body = MirBody::new(0, vec![block(0, vec![pure, load(1)], vec![])]);

    let graph = built(&body);

    assert!(!graph.sites.contains_key(&site(0, 0)));
    assert_eq!(graph.at(site(0, 1)).defining, Some(graph.live.id));
}

#[test]
fn test_entry_backedge_keeps_the_invocation_memory_state() {
    let body = MirBody::new(0, vec![block(0, vec![store(0, cell())], vec![0])]);
    let graph = built(&body);
    assert_eq!(incoming(&graph.phis[&0]), vec![(None, graph.live.id), (Some(0), graph.at(site(0, 0)).id)]);
}

#[test]
fn test_read_only_loop_needs_no_memory_phi() {
    let body = MirBody::new(
        0,
        vec![block(0, vec![], vec![1]), block(1, vec![load(1)], vec![2]), block(2, vec![], vec![1])],
    );
    let graph = built(&body);
    assert!(graph.phis.is_empty());
    assert_eq!(graph.at(site(1, 0)).defining, Some(graph.live.id));
}

fn other() -> MemRef {
    MemRef::new(Some(addr(Space::Segment, 0x30, 1)), 2)
}

#[test]
fn test_clobber_skips_a_disjoint_store() {
    let body = MirBody::new(0, vec![block(0, vec![store(0, cell()), store(1, other()), load(2)], vec![])]);
    let graph = built(&body);
    assert_eq!(graph.clobbers(site(0, 2), &cell(), None), BTreeSet::from([graph.at(site(0, 0)).id]));
}

#[test]
fn test_clobber_walks_a_disjoint_loop_backedge() {
    let body = MirBody::new(
        0,
        vec![
            block(0, vec![store(0, cell())], vec![1]),
            block(1, vec![load(1)], vec![2]),
            block(2, vec![store(2, other())], vec![1]),
        ],
    );
    let graph = built(&body);
    assert_eq!(graph.clobbers(site(1, 0), &cell(), None), BTreeSet::from([graph.at(site(0, 0)).id]));
}

#[test]
fn test_clobber_keeps_both_aliasing_join_definitions() {
    let body = MirBody::new(
        0,
        vec![
            block(0, vec![], vec![1, 2]),
            block(1, vec![store(1, cell())], vec![3]),
            block(2, vec![operation(2, vec![], vec![], true)], vec![3]),
            block(3, vec![load(3)], vec![]),
        ],
    );
    let graph = built(&body);
    assert_eq!(
        graph.clobbers(site(3, 0), &cell(), None),
        BTreeSet::from([graph.at(site(1, 0)).id, graph.at(site(2, 0)).id])
    );
}

#[test]
fn test_clobber_preserves_partial_and_unknown_writes_and_calls() {
    // A word write at +1 changes one byte of the word being loaded.
    let partial = MemRef::new(Some(addr(Space::Segment, 0x21, 1)), 2);
    for write in [store(0, partial), store(0, MemRef::new(None, 2)), call(0)] {
        let body = MirBody::new(0, vec![block(0, vec![write, load(1)], vec![])]);
        let graph = built(&body);
        assert_eq!(graph.clobbers(site(0, 1), &cell(), None), BTreeSet::from([graph.at(site(0, 0)).id]));
    }
}

#[test]
fn test_disjoint_writes_leave_live_on_entry_as_the_clobber() {
    let body = MirBody::new(0, vec![block(0, vec![store(0, other()), load(1)], vec![])]);
    let graph = built(&body);
    assert_eq!(graph.clobbers(site(0, 1), &cell(), None), BTreeSet::from([graph.live.id]));
}

/// A loop write invalidates a dominating read made before entering the loop.
#[test]
fn test_loop_backedge_write_prevents_read_reuse() {
    let body = MirBody::new(
        0,
        vec![
            block(0, vec![], vec![1]),
            block(1, vec![store(1, cell())], vec![2]),
            block(2, vec![load(2)], vec![3]),
            block(3, vec![store(3, cell()), load(4)], vec![3]),
        ],
    );
    let graph = built(&body);
    assert!(!graph.unchanged(site(2, 0), site(3, 1), &cell(), None));
}
