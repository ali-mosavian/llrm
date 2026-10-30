//! llrm-codegen's `analysis/memoryssa_tests.rs`, written as rich MIR.
//!
//! Skipped: `test_entry_backedge_keeps_the_invocation_memory_state`, since
//! nothing branches to an LLVM entry block.

use std::collections::BTreeSet;

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::{InstId, Module};

use super::*;
use crate::consts::Calls;
use crate::testing::{DOS, block, function, layout, parsed};

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module =
            parsed(&format!("{DOS}@g = global [256 x i8] zeroinitializer\ndeclare void @anything()\n\n{body}"));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, function(&self.module, "f")))
    }
}

/// `@g`'s bytes from 32 and from 48.
const CELL: &str = "getelementptr (i8, ptr @g, i16 32)";
const OTHER: &str = "getelementptr (i8, ptr @g, i16 48)";

/// Instruction `index` of block `name`.
fn site(
    unit: &Unit,
    name: &str,
    index: usize,
) -> InstId {
    unit.function.iter_block(block(unit.function, name)).nth(index).unwrap()
}

fn at(
    unit: &Unit,
    name: &str,
) -> i64 {
    cfg::id(block(unit.function, name))
}

fn cell(
    unit: &Unit,
    inst: InstId,
) -> MemRef {
    MemRef::of(unit, inst).expect("a load or store")
}

fn graph<'a>(unit: &Unit<'a>) -> MemorySSA<'a> {
    built(unit, &Accesses::plain(unit, &Calls::default()))
}

fn incoming(access: &Access) -> Vec<(Option<i64>, usize)> {
    let mut edges = access.incoming.clone();
    edges.sort();
    edges
}

/// Each client of a body made the graph's structure again (147 builds for
/// d_faces' 48 float solves and 63 through-memory ones, 61.7 M instructions,
/// one more of them per walk that placed an index). Graphs of one body state
/// and one set of accesses share it; a copy of the accesses, or the body
/// changed, makes its own.
#[test]
fn graphs_of_one_body_and_accesses_share_one_structure() {
    let parsed = diamond();
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &Calls::default());
    let before = structures_made();
    let first = built(&unit, &accesses);
    let registers = Default::default();
    let second = built(&unit.with_registers(&registers), &accesses);
    assert_eq!(structures_made() - before, 1);
    assert_eq!(first.sites.len(), second.sites.len());
    built(&unit, &accesses.clone());
    assert_eq!(structures_made() - before, 2, "a copy of the accesses may be changed: it starts without the structure");
}

/// Each arm stores the cell; the join loads it.
fn diamond() -> Parsed {
    Parsed::new(&format!(
        "define i16 @f(i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr {CELL}
  br label %b3

b2:
  store i16 2, ptr {CELL}
  br label %b3

b3:
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
    ))
}

/// `write` then a load of the cell, in one block.
fn written(write: &str) -> Parsed {
    Parsed::new(&format!(
        "declare i16 @reads() memory(read)
declare i16 @pure() memory(none)

define i16 @f(ptr %p) {{
b0:
  {write}
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
    ))
}

#[test]
fn test_each_join_edge_retains_its_own_stored_value() {
    let parsed = diamond();
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b3", 0);
    let memory = cell(&unit, load);
    for (arm, other) in [("b1", "b2"), ("b2", "b1")] {
        assert!(graph.available_on_edge(site(&unit, arm, 0), load, at(&unit, arm), &memory, None));
        assert!(!graph.available_on_edge(site(&unit, other, 0), load, at(&unit, arm), &memory, None));
    }
}

/// What a cell holds where a block ends, asked of the block (a counted float
/// loop's preheader ends in a branch no access stands for): the live state
/// before any store, the block's own store after it, both arms at the join.
#[test]
fn test_the_end_of_a_block_is_asked_of_the_block() {
    let parsed = diamond();
    let unit = parsed.unit();
    let graph = graph(&unit);
    let memory = cell(&unit, site(&unit, "b3", 0));
    let id = |name: &str| graph.at(site(&unit, name, 0)).id;
    assert_eq!(graph.clobbers_at_end(at(&unit, "b0"), &memory), BTreeSet::from([graph.live.id]));
    assert_eq!(graph.clobbers_at_end(at(&unit, "b1"), &memory), BTreeSet::from([id("b1")]));
    assert_eq!(graph.clobbers_at_end(at(&unit, "b3"), &memory), BTreeSet::from([id("b1"), id("b2")]));
}

#[test]
fn test_a_load_uses_the_nearest_memory_definition() {
    let parsed = written(&format!("store i16 1, ptr {CELL}"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let write = graph.at(site(&unit, "b0", 0));
    let read = graph.at(site(&unit, "b0", 1));

    assert_eq!(write.kind, Kind::Def);
    assert_eq!(write.defining, Some(graph.live.id));
    assert_eq!(read.kind, Kind::Use);
    assert_eq!(read.defining, Some(write.id));
}

#[test]
fn test_a_join_gets_one_memory_phi() {
    let parsed = diamond();
    let unit = parsed.unit();
    let graph = graph(&unit);
    let phi = &graph.phis[&at(&unit, "b3")];

    assert_eq!(phi.kind, Kind::Phi);
    assert_eq!(
        incoming(phi),
        vec![
            (Some(at(&unit, "b1")), graph.at(site(&unit, "b1", 0)).id),
            (Some(at(&unit, "b2")), graph.at(site(&unit, "b2", 0)).id)
        ]
    );
    assert_eq!(graph.at(site(&unit, "b3", 0)).defining, Some(phi.id));
}

/// A loop reading the cell at its header, and writing `write` on its back edge.
fn looped(write: &str) -> Parsed {
    Parsed::new(&format!(
        "define i16 @f(i1 %c) {{
b0:
  store i16 1, ptr {CELL}
  br label %b1

b1:
  %x = load i16, ptr {CELL}
  br i1 %c, label %b2, label %b3

b2:
  {write}
  br label %b1

b3:
  ret i16 %x
}}
"
    ))
}

#[test]
fn test_a_loop_header_phi_carries_the_backedge_definition() {
    let parsed = looped(&format!("store i16 2, ptr {CELL}"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let phi = &graph.phis[&at(&unit, "b1")];

    assert_eq!(
        incoming(phi),
        vec![
            (Some(at(&unit, "b0")), graph.at(site(&unit, "b0", 0)).id),
            (Some(at(&unit, "b2")), graph.at(site(&unit, "b2", 0)).id)
        ]
    );
    assert_eq!(graph.at(site(&unit, "b1", 0)).defining, Some(phi.id));
}

/// A volatile access touches only its own bytes, as LLVM's: a volatile
/// store defines memory and may change the cell through `%p`, but not the
/// cell beside its own; a volatile load reads, it defines nothing. Both were
/// a barrier, and TEXTFILL read its variables again after every POKE (#257).
#[test]
fn test_a_volatile_access_touches_only_its_own_bytes() {
    for (write, kind, clobbers) in [
        ("store volatile i16 1, ptr %p", Kind::Def, true),
        (&format!("store volatile i16 1, ptr {OTHER}"), Kind::Def, false),
        ("%v = load volatile i16, ptr %p", Kind::Use, false),
    ] {
        let parsed = written(write);
        let unit = parsed.unit();
        let graph = graph(&unit);
        assert_eq!(graph.at(site(&unit, "b0", 0)).kind, kind, "{write}");
        let load = site(&unit, "b0", 1);
        assert_eq!(
            graph.clobbers(load, &cell(&unit, load)).contains(&graph.at(site(&unit, "b0", 0)).id),
            clobbers,
            "{write}"
        );
    }
}

#[test]
fn test_a_call_with_no_named_cells_still_defines_memory() {
    let parsed = written("call void @anything()");
    let unit = parsed.unit();
    let graph = graph(&unit);
    assert_eq!(graph.at(site(&unit, "b0", 0)).kind, Kind::Def);
    assert_eq!(graph.at(site(&unit, "b0", 1)).defining, Some(graph.at(site(&unit, "b0", 0)).id));
}

/// A proven readonly callee used to become an unknown MemoryDef anyway.
#[test]
fn test_a_read_only_call_uses_but_does_not_define_memory() {
    let parsed = written("%r = call i16 @reads()");
    let unit = parsed.unit();
    let graph = graph(&unit);

    assert_eq!(graph.at(site(&unit, "b0", 0)).kind, Kind::Use);
    assert_eq!(graph.at(site(&unit, "b0", 1)).defining, Some(graph.live.id));
}

/// A pure call is still a value operation, but not a memory version.
#[test]
fn test_a_memory_free_call_has_no_memoryssa_access() {
    let parsed = written("%r = call i16 @pure()");
    let unit = parsed.unit();
    let graph = graph(&unit);

    assert!(!graph.sites.contains_key(&site(&unit, "b0", 0)));
    assert_eq!(graph.at(site(&unit, "b0", 1)).defining, Some(graph.live.id));
}

#[test]
fn test_read_only_loop_needs_no_memory_phi() {
    let parsed = Parsed::new(&format!(
        "define void @f() {{
b0:
  br label %b1

b1:
  %x = load i16, ptr {CELL}
  br label %b2

b2:
  br label %b1
}}
"
    ));
    let unit = parsed.unit();
    let graph = graph(&unit);
    assert!(graph.phis.is_empty());
    assert_eq!(graph.at(site(&unit, "b1", 0)).defining, Some(graph.live.id));
}

#[test]
fn test_clobber_skips_a_disjoint_store() {
    let parsed = Parsed::new(&format!(
        "define i16 @f() {{
b0:
  store i16 1, ptr {CELL}
  store i16 2, ptr {OTHER}
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
    ));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b0", 2);
    assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(site(&unit, "b0", 0)).id]));
}

#[test]
fn test_clobber_walks_a_disjoint_loop_backedge() {
    let parsed = looped(&format!("store i16 2, ptr {OTHER}"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b1", 0);
    assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(site(&unit, "b0", 0)).id]));
}

#[test]
fn test_clobber_stops_at_an_aliasing_loop_backedge() {
    let parsed = looped(&format!("store i16 2, ptr {CELL}"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b1", 0);
    assert_eq!(
        graph.clobbers(load, &cell(&unit, load)),
        BTreeSet::from([graph.at(site(&unit, "b0", 0)).id, graph.at(site(&unit, "b2", 0)).id])
    );
}

#[test]
fn test_clobber_keeps_both_aliasing_join_definitions() {
    let parsed = Parsed::new(&format!(
        "define i16 @f(i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr {CELL}
  br label %b3

b2:
  call void @anything()
  br label %b3

b3:
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
    ));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b3", 0);
    assert_eq!(
        graph.clobbers(load, &cell(&unit, load)),
        BTreeSet::from([graph.at(site(&unit, "b1", 0)).id, graph.at(site(&unit, "b2", 0)).id])
    );
}

#[test]
fn test_clobber_preserves_partial_and_unknown_writes_and_calls() {
    // A word write at +1 changes one byte of the word being loaded; a
    // pointer argument may point anywhere.
    for write in ["store i16 0, ptr getelementptr (i8, ptr @g, i16 33)", "store i16 0, ptr %p", "call void @anything()"]
    {
        let parsed = written(write);
        let unit = parsed.unit();
        let graph = graph(&unit);
        let load = site(&unit, "b0", 1);
        assert_eq!(
            graph.clobbers(load, &cell(&unit, load)),
            BTreeSet::from([graph.at(site(&unit, "b0", 0)).id]),
            "{write}"
        );
    }
}

#[test]
fn test_disjoint_writes_leave_live_on_entry_as_the_clobber() {
    let parsed = written(&format!("store i16 0, ptr {OTHER}"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b0", 1);
    assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.live.id]));
}

/// Two words off one pointer are apart by their offsets, whatever it points to.
#[test]
fn test_clobber_skips_a_store_at_another_offset_of_the_same_pointer() {
    let parsed = Parsed::new(
        "define i16 @f(ptr %p) {
b0:
  %q = getelementptr inbounds i8, ptr %p, i16 2
  store i16 1, ptr %p
  store i16 2, ptr %q
  %x = load i16, ptr %p
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b0", 3);
    assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(site(&unit, "b0", 1)).id]));
}

/// A call writes only its footprint, where one is known.
#[test]
fn test_a_call_footprint_decides_whether_it_clobbers() {
    let parsed = Parsed::new(&format!(
        "define i16 @f() {{
b0:
  store i16 1, ptr {CELL}
  call void @anything()
  %x = load i16, ptr {CELL}
  %y = load i16, ptr {OTHER}
  ret i16 %x
}}
"
    ));
    let unit = parsed.unit();
    let (store, call, load) = (site(&unit, "b0", 0), site(&unit, "b0", 1), site(&unit, "b0", 2));
    for (footprint, clobber) in [(site(&unit, "b0", 3), store), (load, call)] {
        let graph = built(
            &unit,
            &Accesses::plain(&unit, &Calls::from_iter([(call, std::rc::Rc::from(vec![cell(&unit, footprint)]))])),
        );
        assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(clobber).id]));
    }
}

/// A call is a clobber of a cell where the AA stack cannot rule it out
/// (`instructionClobbersQuery`): `lifetime.end` of another slot is apart from
/// this one (the pin's NoModRef), a call given the slot is not. The footprint
/// made both clobbers (33 `lifetime.end` and 15 `lifetime.start` queries over
/// QCport's modules).
#[test]
fn test_a_call_the_aa_stack_rules_out_does_not_clobber_the_cell() {
    let parsed = Parsed::new(
        "declare void @llvm.lifetime.end.p0(i64, ptr)
declare void @given(ptr)

define i16 @f() {
b0:
  %slot = alloca i16
  %other = alloca i16
  store i16 1, ptr %slot
  call void @llvm.lifetime.end.p0(i64 2, ptr %other)
  %x = load i16, ptr %slot
  call void @given(ptr %slot)
  %y = load i16, ptr %slot
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let (store, ended, load, given, again) =
        (site(&unit, "b0", 2), site(&unit, "b0", 3), site(&unit, "b0", 4), site(&unit, "b0", 5), site(&unit, "b0", 6));
    let graph = graph(&unit);
    assert_eq!(
        graph.clobbers(load, &cell(&unit, load)),
        BTreeSet::from([graph.at(store).id]),
        "lifetime.end {ended:?}"
    );
    assert_eq!(graph.clobbers(again, &cell(&unit, again)), BTreeSet::from([graph.at(given).id]));
}

/// A store the AA stack rules out (a `noalias` argument against a pointer
/// derived from another argument) is not a clobber of the cell; the walker
/// asked the stack only for calls and found the store.
#[test]
fn test_a_store_the_aa_stack_rules_out_does_not_clobber_the_cell() {
    let parsed = Parsed::new(
        "define i16 @f(ptr noalias %p, ptr %q, i16 %i) {
b0:
  %r = getelementptr i16, ptr %q, i16 %i
  store i16 1, ptr %p
  %x = load i16, ptr %r
  store i16 2, ptr %r
  %y = load i16, ptr %p
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let (first, load, _second, again) =
        (site(&unit, "b0", 1), site(&unit, "b0", 2), site(&unit, "b0", 3), site(&unit, "b0", 4));
    let graph = graph(&unit);
    assert_eq!(graph.clobbers(again, &cell(&unit, again)), BTreeSet::from([graph.at(first).id]));
    let _ = load;
}

/// A loop write invalidates a dominating read made before entering the loop.
#[test]
fn test_loop_backedge_write_prevents_read_reuse() {
    let parsed = Parsed::new(&format!(
        "define void @f() {{
b0:
  br label %b1

b1:
  store i16 1, ptr {CELL}
  br label %b2

b2:
  %x = load i16, ptr {CELL}
  br label %b3

b3:
  store i16 2, ptr {CELL}
  %y = load i16, ptr {CELL}
  br label %b3
}}
"
    ));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let later = site(&unit, "b3", 1);
    assert!(!graph.unchanged(site(&unit, "b2", 0), later, &cell(&unit, later)));
}

/// A read before a loop still holds inside it past disjoint writes only.
#[test]
fn test_a_read_before_a_loop_is_unchanged_past_disjoint_writes() {
    for (write, unchanged) in [(OTHER, true), (CELL, false)] {
        let parsed = Parsed::new(&format!(
            "define void @f(i1 %c) {{
b0:
  %x = load i16, ptr {CELL}
  br label %b1

b1:
  %y = load i16, ptr {CELL}
  store i16 2, ptr {write}
  br i1 %c, label %b1, label %b2

b2:
  ret void
}}
"
        ));
        let unit = parsed.unit();
        let graph = graph(&unit);
        let later = site(&unit, "b1", 0);
        assert_eq!(graph.unchanged(site(&unit, "b0", 0), later, &cell(&unit, later)), unchanged, "{write}");
    }
}

/// A write through the join's pointer phi, before the load, blocks every edge.
#[test]
fn test_a_join_prefix_write_blocks_each_edge() {
    let parsed = Parsed::new(
        "define i16 @f(i1 %c, ptr %a, ptr %b) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr %a
  br label %b3

b2:
  store i16 2, ptr %b
  br label %b3

b3:
  %q = phi ptr [ %a, %b1 ], [ %b, %b2 ]
  store i16 3, ptr %q
  %x = load i16, ptr %q
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b3", 2);
    let memory = cell(&unit, load);
    for (arm, pointer) in [("b1", "%a"), ("b2", "%b")] {
        let translated = cell(&unit, site(&unit, arm, 0));
        assert!(
            !graph.available_on_edge(site(&unit, arm, 0), load, at(&unit, arm), &memory, Some(&translated)),
            "{pointer}"
        );
    }
}

/// On each edge, a phi-based address is the edge's own pointer.
#[test]
fn test_a_translated_address_selects_the_edges_store() {
    let parsed = Parsed::new(
        "define i16 @f(i1 %c, ptr %a, ptr %b) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 1, ptr %a
  br label %b3

b2:
  store i16 2, ptr %b
  br label %b3

b3:
  %q = phi ptr [ %a, %b1 ], [ %b, %b2 ]
  %x = load i16, ptr %q
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = site(&unit, "b3", 1);
    let memory = cell(&unit, load);
    for (arm, other) in [("b1", "b2"), ("b2", "b1")] {
        let translated = cell(&unit, site(&unit, arm, 0));
        assert!(graph.available_on_edge(site(&unit, arm, 0), load, at(&unit, arm), &memory, Some(&translated)));
        assert!(!graph.available_on_edge(site(&unit, other, 0), load, at(&unit, arm), &memory, Some(&translated)));
    }
}

/// A word at one displacement covers what lies in it, spelled by a constant
/// or an instruction; a word beside it does not.
#[test]
fn test_covering_compares_bytes_in_one_frame() {
    let parsed = Parsed::new(&format!(
        "define void @f(ptr %p) {{
b0:
  %a = getelementptr inbounds i8, ptr @g, i16 32
  %q = getelementptr inbounds i8, ptr %p, i16 2
  store i32 0, ptr {CELL}
  store i16 0, ptr getelementptr (i8, ptr @g, i16 34)
  store i16 0, ptr %a
  store i16 0, ptr getelementptr (i8, ptr @g, i16 36)
  store i32 0, ptr %p
  store i16 0, ptr %q
  ret void
}}
"
    ));
    let unit = parsed.unit();
    let [long, high, low, beside, pointed, half] = [2, 3, 4, 5, 6, 7].map(|at| cell(&unit, site(&unit, "b0", at)));
    assert!(covers(&unit, &long, &high) && covers(&unit, &long, &low));
    assert!(!covers(&unit, &high, &long) && !covers(&unit, &long, &beside));
    assert!(same_bytes(&unit, &low, &MemRef { width: 2, ..long.clone() }));
    assert!(!same_bytes(&unit, &low, &high));
    assert!(covers(&unit, &pointed, &half) && !covers(&unit, &half, &pointed));
    assert!(!covers(&unit, &pointed, &long), "unrelated roots");
}

/// `@g` and `@h` are apart by the AA stack (BasicAA, GlobalsAA) whether or not
/// the accesses are resolved: the store to `@h` and the call are both asked of
/// it, and neither is kept in the accesses.
#[test]
fn test_resolved_accesses_prove_two_globals_apart() {
    let parsed = Parsed::new(&format!(
        "@h = global [64 x i8] zeroinitializer

define void @seth() {{
b0:
  store i16 3, ptr @h
  ret void
}}

define i16 @f() {{
b0:
  store i16 1, ptr {CELL}
  store i16 2, ptr @h
  call void @seth()
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
    ));
    let unit = parsed.unit();
    let (first, call, load) = (site(&unit, "b0", 0), site(&unit, "b0", 2), site(&unit, "b0", 3));
    for accesses in [Accesses::plain(&unit, &Calls::default()), Accesses::resolved(&unit).unwrap()] {
        let graph = built(&unit, &accesses);
        assert_eq!(graph.clobbers(load, &accesses.references[&load]), BTreeSet::from([graph.at(call).id]));
    }
    // A call known to write nothing leaves the first store.
    let plain = Accesses::plain(&unit, &Calls::from_iter([(call, std::rc::Rc::from([]))]));
    let graph = built(&unit, &plain);
    assert_eq!(graph.clobbers(load, &plain.references[&load]), BTreeSet::from([graph.at(first).id]));
}

/// `Accesses`, `memory::unmodeled_write` and `effects::unmodeled` each say
/// what a load or store touches, and all three ask `memory::own_bytes`:
/// only its own bytes, volatile or not. b9a49221 changed one, #257 then
/// reached the other two, and TEXTFILL reloaded every variable after a POKE.
#[test]
fn every_answer_to_what_an_access_touches_is_its_own_bytes() {
    let parsed = Parsed::new(&format!(
        "define i16 @f() {{
b0:
  %a = load i16, ptr {CELL}
  %b = load volatile i16, ptr {CELL}
  store i16 %a, ptr {CELL}
  store volatile i16 %b, ptr {CELL}
  ret i16 %a
}}
"
    ));
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &Calls::default());
    let declarations = parsed.module.declarations();
    for index in 0..4 {
        let inst = site(&unit, "b0", index);
        let own = crate::memory::own_bytes(&unit.function.instruction(inst).opcode).expect("an access");
        let mine = [cell(&unit, inst)];
        let named = |does: bool| Some(if does { &mine[..] } else { &[][..] });
        assert_eq!(
            (accesses.reads(inst), accesses.writes(inst)),
            (named(own.reads), named(own.writes)),
            "Accesses, access {index}"
        );
        assert!(!crate::memory::unmodeled_write(&unit, inst), "memory::unmodeled_write, access {index}");
        let context = &parsed.module.context;
        let unmodeled = (
            crate::effects::unmodeled_read(context, &declarations, unit.function, inst),
            crate::effects::unmodeled_write(context, &declarations, unit.function, inst),
        );
        assert_eq!(unmodeled, (false, false), "effects::unmodeled, access {index}");
    }
}

/// Each load of one address walked back over every store between it and the
/// first, asking each store's clobbers afresh: `mir gvn` was 10 s of compiling
/// 1600 BASIC statements (#560). A (cell, store) pair is worked out once.
#[test]
fn test_loads_of_one_address_ask_each_store_whether_it_clobbers_once() {
    let steps: String =
        (0..30).map(|at| format!("  store i16 {at}, ptr {OTHER}\n  %y{at} = load i16, ptr {CELL}\n")).collect();
    let parsed =
        Parsed::new(&format!("define void @f() {{\nb0:\n  %x = load i16, ptr {CELL}\n{steps}  ret void\n}}\n"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let first = site(&unit, "b0", 0);
    let before = clobber_runs();
    for at in 0..30 {
        let later = site(&unit, "b0", 2 + 2 * at);
        assert!(graph.unchanged(first, later, &cell(&unit, later)));
    }
    assert!(clobber_runs() - before <= 30, "{} clobber questions for 30 stores", clobber_runs() - before);
    // A walk hashes its cell once for the whole walk (the memo is by number),
    // not once for each store it passes: the hash of a MemRef was 6% of gvn
    // on host.c.
    assert_eq!(graph.slots.borrow().len(), 1, "one cell was asked about");
}

/// The walk back from a load stopped at every access and asked of it. A chain
/// of uses and defs that leave the cell alone ends in the same place for every
/// load of the cell, so a walk remembers where and jumps there. It must find
/// what the walk step by step finds, from every load, for every cell, with and
/// without a boundary.
#[test]
fn test_a_walk_that_jumps_finds_what_a_walk_step_by_step_finds() {
    let stores: String = (0..12).map(|n| format!("  store i8 {n}, ptr getelementptr (i8, ptr @g, i16 {})\n  %v{n} = load i8, ptr getelementptr (i8, ptr @g, i16 {})\n", 8 + n % 3 * 16, 8 + (n + 1) % 3 * 16)).collect();
    let parsed = Parsed::new(&format!(
        "define i8 @f(i1 %c) {{\nb0:\n{stores}  br i1 %c, label %b1, label %b2\n\nb1:\n  store i8 1, ptr getelementptr (i8, ptr @g, i16 24)\n  br label %b3\n\nb2:\n  br label %b3\n\nb3:\n  %w = load i8, ptr getelementptr (i8, ptr @g, i16 24)\n  %x = load i8, ptr getelementptr (i8, ptr @g, i16 8)\n  ret i8 %w\n}}\n"
    ));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let loads: Vec<InstId> = graph
        .sites
        .keys()
        .copied()
        .filter(|&site| matches!(
            unit.function.instruction(site).opcode,
            llrm_mir::opcode::Opcode::Load { .. }
        ))
        .collect();
    assert!(loads.len() >= 14);
    for &site in &loads {
        for &asked in &loads {
            let cell = cell(&unit, asked);
            for boundary in [None, graph.at(site).defining] {
                assert_eq!(
                    graph.walked(site, &cell, boundary, None, None, true, true),
                    graph.walked(site, &cell, boundary, None, None, false, true),
                    "{site:?} for {asked:?}"
                );
            }
        }
    }
}

/// `built` copied every instruction's written references out of `Accesses`
/// (MemRef clones with their provenance sets and their drop): 20% of gvn's
/// `forwarded` on QCport's host.c. The graph shares them.
#[test]
fn the_graph_shares_the_written_references_with_the_accesses_it_is_built_from() {
    let parsed = Parsed::new(&format!(
        "define void @f(i16 %p) {{\nb0:\n  store i16 %p, ptr {CELL}\n  store i16 %p, ptr {OTHER}\n  ret void\n}}\n"
    ));
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &Calls::default());
    let graph = built(&unit, &accesses);
    let store = site(&unit, "b0", 0);
    let kept = accesses.writes(store).expect("a store writes");
    assert_eq!(kept.len(), 1);
    let shared = graph.written[&store].as_deref().expect("the graph has the write");
    assert!(std::ptr::eq(shared.as_ptr(), kept.as_ptr()), "the graph copied the store's references");
}

/// A walk hashed its whole cell (a `MemRef`, provenance and all) at every store
/// it passed, to find the remembered answer: 6% of gvn on host.c. Each walk
/// looks its cell up by value once and the stores by number. Thirty loads of
/// thirty cells, each walked back over thirty stores that leave it alone:
/// thirty lookups, not nine hundred.
#[test]
fn a_walk_looks_its_cell_up_by_value_once_however_many_stores_it_passes() {
    let stores: String = (0..30).map(|at| format!("  store i16 {at}, ptr {OTHER}\n")).collect();
    let loads: String =
        (0..30).map(|at| format!("  %y{at} = load i8, ptr getelementptr (i8, ptr @g, i16 {at})\n")).collect();
    let parsed = Parsed::new(&format!("define void @f() {{\nb0:\n{stores}{loads}  ret void\n}}\n"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let before = CELL_PROBES.with(std::cell::Cell::get);
    for at in 0..30 {
        let load = site(&unit, "b0", 30 + at);
        assert_eq!(graph.clobbers(load, &cell(&unit, load)).len(), 1);
    }
    let probes = CELL_PROBES.with(std::cell::Cell::get) - before;
    assert_eq!(probes, 30, "{probes} lookups of a cell by value for 30 walks");
}

/// Every first question of a walk, whether a store clobbers a cell, went
/// through the alias reasoning (a refined provenance per pair): 64% of those on
/// host.c were stores to objects that cannot meet the cell, and `clobbered` was
/// 1.4% of the compile. A store whose objects cannot meet the cell's is ruled
/// out by `Accesses::reach` alone.
#[test]
fn a_store_to_another_object_is_ruled_out_without_alias_reasoning() {
    let stores: String = (0..20).map(|at| format!("  store i16 {at}, ptr @h\n")).collect();
    let parsed = Parsed::new(&format!(
        "@h = global [8 x i8] zeroinitializer\n\ndefine void @f() {{\nb0:\n{stores}  %y = load i16, ptr {CELL}\n  ret void\n}}\n"
    ));
    let unit = parsed.unit();
    let accesses = Accesses::resolved(&unit).expect("resolved");
    let graph = built(&unit, &accesses);
    let load = site(&unit, "b0", 20);
    let before = MAY_CLOBBERS.with(std::cell::Cell::get);
    let found = graph.clobbers(load, &accesses.references[&load]);
    let asked = MAY_CLOBBERS.with(std::cell::Cell::get) - before;
    assert_eq!(found.len(), 1, "only the live-on-entry memory reaches the load");
    assert_eq!(asked, 0, "{asked} alias questions for 20 stores to another object");
}

/// A cell written in one arm of each of N joins is reached, at the last, by N
/// nearest writes, and the walk found them all: N^2 over a body of N loads
/// (`joins` 256: 10% of the compile). A walk for a caller that serves a load
/// from them stops past `MAX_NEAREST_WRITES`.
#[test]
fn test_a_walk_for_a_load_to_be_served_stops_once_more_writes_reach_it_than_can_serve_it() {
    let joins = 400;
    let diamonds: String = (0..joins)
        .map(|k| {
            format!(
                "  br i1 %c, label %t{k}, label %j{k}\n\nt{k}:\n  store i16 7, ptr {CELL}\n  br label %j{k}\n\nj{k}:\n"
            )
        })
        .collect();
    let parsed = Parsed::new(&format!(
        "define i16 @f(i1 %c) {{\nb0:\n  store i16 7, ptr {CELL}\n{diamonds}  %loaded = load i16, ptr {CELL}\n  ret i16 %loaded\n}}\n"
    ));
    let unit = parsed.unit();
    let accesses = Accesses::resolved(&unit).expect("resolved");
    let graph = built(&unit, &accesses);
    let load = site(&unit, &format!("j{}", joins - 1), 0);
    let reference = &accesses.references[&load];
    let before = STEPS.with(std::cell::Cell::get);
    let all = graph.clobbers_ignoring_invariance(load, reference);
    let walked_all = STEPS.with(std::cell::Cell::get) - before;
    assert_eq!(all.len(), joins + 1, "every write reaches it");
    let before = STEPS.with(std::cell::Cell::get);
    let some = graph.nearest_writes(load, reference);
    let walked_some = STEPS.with(std::cell::Cell::get) - before;
    assert!(some.len() > MAX_NEAREST_WRITES, "past the limit the caller serves nothing");
    assert!(walked_some * 3 < walked_all, "{walked_some} steps for a bounded walk, {walked_all} for the whole");
}

/// 200 stores to 200 cells of one object, then a load of each: each load walked
/// back over every store after its own (200 x 200 / 2 pairs; the `cells` axis'
/// through-memory was 2,088 Minstr at N=1024, quadratic). The index of a
/// frame's last writes finds each at once, stepping over none of the others.
#[test]
fn test_a_load_of_one_of_many_fixed_cells_finds_its_store_without_walking_the_others() {
    let cell = |at: usize| format!("getelementptr (i8, ptr @g, i16 {at})");
    let stores: String = (0..200).map(|at| format!("  store i8 {at}, ptr {}\n", cell(at))).collect();
    let loads: String = (0..200).map(|at| format!("  %y{at} = load i8, ptr {}\n", cell(at))).collect();
    let parsed = Parsed::new(&format!("define void @f() {{\nb0:\n{stores}{loads}  ret void\n}}\n"));
    let unit = parsed.unit();
    let accesses = Accesses::resolved(&unit).expect("resolved");
    let graph = built(&unit, &accesses);
    let before = STEPS.with(std::cell::Cell::get);
    for at in 0..200 {
        let load = site(&unit, "b0", 200 + at);
        let found = graph.clobbers(load, &accesses.references[&load]);
        let store = site(&unit, "b0", at);
        assert_eq!(found, BTreeSet::from([graph.at(store).id]), "the load of cell {at}");
    }
    let steps = STEPS.with(std::cell::Cell::get) - before;
    // The first loads walk (the index is made once the walks have stepped over
    // enough).
    assert!(steps <= 9000, "{steps} accesses stepped over for 200 loads");
}

/// An edit that makes or moves no memory op and no terminator leaves the
/// structure as it was: the manager's plain accesses carry it. Each
/// of those edits made the structure again (44 of 90 re-builds for d_faces
/// -O2, half of the 71 M spent on them); an edit of a store's address still
/// makes it.
#[test]
fn an_edit_that_leaves_memory_ops_and_blocks_alone_keeps_the_structure() {
    use llrm_mir::module::Operand;
    use llrm_mir::passes::{Outer, PreservedAnalyses};
    let mut module = parsed(&format!(
        "{DOS}define i16 @f(ptr %p, ptr %q, i16 %x) {{
b0:
  %a = add i16 %x, 1
  store i16 %a, ptr %p
  %v = load i16, ptr %p
  ret i16 %v
}}
"
    ));
    let layout = layout(&module);
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let structure = |module: &Module, analyses: &mut Analyses| {
        let (context, function) = (&module.context, function(module, "f"));
        let plain = analyses.get::<crate::manager::PlainAccesses>(context, &layout, function);
        let accesses = plain.as_ref().as_ref().unwrap();
        let unit = Unit::within(context, &layout, function, analyses.outer());
        built(&unit, accesses);
    };
    let before = structures_made();
    structure(&module, &mut analyses);
    assert_eq!(structures_made() - before, 1);
    {
        let (context, function) = module.function_mut("f").unwrap();
        let add = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| function.instruction(inst).opcode == Opcode::Binary(llrm_mir::opcode::BinaryOp::Add))
            .unwrap();
        let x = function.instruction(add).operands[0];
        function.set_operand(add, 1, x);
        analyses.invalidate(function, &PreservedAnalyses::none());
        let _ = context;
    }
    structure(&module, &mut analyses);
    assert_eq!(structures_made() - before, 1, "an add rewritten leaves the structure");
    {
        let (_, function) = module.function_mut("f").unwrap();
        let store = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
            .unwrap();
        let q = Operand::Value(crate::testing::value(function, "q"));
        function.set_operand(store, 1, q);
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    structure(&module, &mut analyses);
    assert_eq!(structures_made() - before, 2, "a store given another address is another structure");
}

/// A walk asked each write of the chain whether it may clobber the cell, and
/// each answer followed the pointer chain of the cell and of the write again
/// (`Offsets::stepped`): 28 M instructions, 0.8% of the d_faces -O2 compile,
/// for chains that are the same on every ask. Each pointer's chain is followed
/// once.
#[test]
fn a_walk_follows_the_chain_of_each_pointer_once() {
    let arms = 12;
    let params: String = (0..arms).map(|k| format!(", ptr %r{k}")).collect();
    let cases: String = (0..arms).map(|k| format!("    i16 {k}, label %a{k}\n")).collect();
    let bodies: String = (0..arms)
        .map(|k| {
            format!(
                "a{k}:\n  %w{k} = getelementptr i8, ptr %r{k}, i16 4\n  store i8 1, ptr %w{k}\n  br label %join\n\n"
            )
        })
        .collect();
    let parsed = Parsed::new(&format!(
        "define i8 @f(i16 %n, ptr %q{params}) {{\nb0:\n  %c = getelementptr i8, ptr %q, i16 %n\n  switch i16 %n, label %join [\n{cases}  ]\n\n{bodies}join:\n  %x = load i8, ptr %c\n  ret i8 %x\n}}\n"
    ));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let load = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. }))
        .expect("a load");
    let cell = cell(&unit, load);
    crate::pointerfacts::STEPPED.with(|count| count.set(0));
    let found = graph.nearest_writes(load, &cell);
    let followed = crate::pointerfacts::STEPPED.with(std::cell::Cell::get);
    assert!(found.len() >= arms, "premise: the walk met the writes of every arm: {found:?}");
    assert!(followed <= arms + 2, "{followed} chains followed for {} pointers", arms + 1);
}

/// A machine that counts how often it is asked for its address spaces.
struct Counting;

thread_local! {
    static ASKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl llrm_mir::target::Machine for Counting {
    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        ASKED.with(|asked| asked.set(asked.get() + 1));
        llrm_mir::spaces::Spaces::FLAT
    }

    fn foreign_span(
        &self,
        _: (i64, i64),
        _: (i64, i64),
        _: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn costs(&self) -> llrm_mir::target::OperationCosts {
        llrm_mir::target::OperationCosts::default()
    }

    fn registers(&self) -> i64 {
        0
    }

    fn call_registers(&self) -> i64 {
        0
    }

    fn address_forms(&self) -> Vec<llrm_mir::target::AddressForm> {
        llrm_mir::target::Neutral.address_forms()
    }
}

/// `regions::foreign` asked the machine for its address spaces at every
/// reference of every pair (a virtual call and a copy of the table: 16 M
/// instructions, 0.45% of the d_faces -O2 compile); the program proxy holds
/// them, asked once.
#[test]
fn overlap_asks_the_program_for_its_spaces_not_the_machine() {
    let parsed = Parsed::new(&format!(
        "define i16 @f(ptr %p, ptr %q) {{\nb0:\n  store i16 1, ptr %p\n  %x = load i16, ptr %q\n  ret i16 %x\n}}\n"
    ));
    let unit = parsed.unit();
    let program = llrm_mir::program::Program::new(vec![parsed.module.clone()], std::rc::Rc::new(Counting)).unwrap();
    let proxy = llrm_mir::program::ProgramAnalyses::default().proxy(&program, 0);
    let (store, load) = (site(&unit, "b0", 0), site(&unit, "b0", 1));
    let (written, read) = (cell(&unit, store), cell(&unit, load));
    ASKED.with(|asked| asked.set(0));
    for _ in 0..10 {
        let _ = crate::regions::overlapping(&read, &written, None, None, Some(&proxy));
    }
    let asked = ASKED.with(std::cell::Cell::get);
    assert_eq!(asked, 0, "the machine was asked for its spaces {asked} times in 10 pairs");
}

/// A load apart from 3,000 stores stepped over all of them (the walk found the
/// live state): no bound on one walk. It stops at `MAX_WALK_STEPS` and answers
/// with the access it stopped at, which the load is taken to be clobbered by.
#[test]
fn test_a_walk_stops_at_the_step_limit_with_the_access_it_stopped_at() {
    let stores: String =
        (1..=3000).map(|k| format!("  %a{k} = getelementptr i8, ptr %p, i16 {k}\n  store i8 0, ptr %a{k}\n")).collect();
    let parsed =
        Parsed::new(&format!("define i8 @f(ptr %p) {{\nb0:\n{stores}  %y = load i8, ptr %p\n  ret i8 %y\n}}\n"));
    let unit = parsed.unit();
    let accesses = Accesses::resolved(&unit).expect("resolved");
    let graph = built(&unit, &accesses);
    let load = site(&unit, "b0", 6000);
    let before = STEPS.with(std::cell::Cell::get);
    let found = graph.clobbers(load, &accesses.references[&load]);
    let steps = STEPS.with(std::cell::Cell::get) - before;
    assert!(steps <= MAX_WALK_STEPS + 1, "{steps} steps for one walk");
    assert!(found.iter().all(|id| graph.access(*id).kind != Kind::Live), "the walk said nothing clobbers the load");
}

/// A store moved (the structure is not kept) and then an edit that leaves the
/// structure alone: the second carried the cell's structure over as if it were
/// the first's result, and the structure made before the store moved passed
/// for the body's. The x_rle kernel under the order switch
/// (`LLRM_CHECK_CLOBBERS`: "the structure kept past the edits is not the one
/// the body makes", 396 of 408 failing compiles): `hoist` and `loopmotion`
/// moved stores, nothing asked the structure, then `lcssa` inserted phis.
#[test]
fn a_structure_is_not_carried_past_an_edit_that_changed_it() {
    use llrm_mir::edit::Position;
    use llrm_mir::passes::{Outer, PreservedAnalyses};
    let mut module = parsed(&format!(
        "{DOS}define i16 @f(ptr %p, i16 %x) {{
b0:
  br label %b1

b1:
  store i16 %x, ptr %p
  br label %b2

b2:
  %a = add i16 %x, 1
  %v = load i16, ptr %p
  ret i16 %v
}}
"
    ));
    let layout = layout(&module);
    let mut analyses = Analyses::new(Rc::new(Outer::of(&module, None)));
    let store_block = |module: &Module, analyses: &mut Analyses| {
        let (context, function) = (&module.context, function(module, "f"));
        let accesses = Accesses::managed(context, &layout, function, analyses).unwrap();
        let unit = Unit::within(context, &layout, function, analyses.outer());
        let graph = built(&unit, &accesses);
        let store = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
            .unwrap();
        graph.at(store).block
    };
    let b0 = Some(cfg::id(block(function(&module, "f"), "b0")));
    assert_ne!(store_block(&module, &mut analyses), b0);
    {
        let (_, function) = module.function_mut("f").unwrap();
        let store = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Store { .. }))
            .unwrap();
        let end = function.terminator(block(function, "b0")).unwrap();
        function.move_to(store, Position::Before(end)).unwrap();
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    // Something asks for the accesses, not the structure; then an edit that
    // leaves a structure alone.
    {
        let (context, function) = (&module.context, function(&module, "f"));
        Accesses::managed(context, &layout, function, &mut analyses).unwrap();
    }
    {
        let (_, function) = module.function_mut("f").unwrap();
        let add = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| function.instruction(inst).opcode == Opcode::Binary(llrm_mir::opcode::BinaryOp::Add))
            .unwrap();
        let x = function.instruction(add).operands[0];
        function.set_operand(add, 1, x);
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    assert_eq!(store_block(&module, &mut analyses), b0, "the store moved to b0, the structure still has it in b1");
}
