//! llrm-core's `analysis/memoryssa_tests.rs`, written as rich MIR.
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
        let module = parsed(&format!("{DOS}@g = global [64 x i8] zeroinitializer\ndeclare void @anything()\n\n{body}"));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        Unit::of(&self.module, &self.layout, function(&self.module, "f"))
    }
}

/// `@g`'s bytes from 32 and from 48.
const CELL: &str = "getelementptr (i8, ptr @g, i16 32)";
const OTHER: &str = "getelementptr (i8, ptr @g, i16 48)";

/// Instruction `index` of block `name`.
fn site(unit: &Unit, name: &str, index: usize) -> InstId {
    unit.function.block(block(unit.function, name)).instructions()[index]
}

fn at(unit: &Unit, name: &str) -> i64 {
    cfg::id(block(unit.function, name))
}

fn cell(unit: &Unit, inst: InstId) -> MemRef {
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
    assert_eq!(incoming(phi), vec![(Some(at(&unit, "b1")), graph.at(site(&unit, "b1", 0)).id), (Some(at(&unit, "b2")), graph.at(site(&unit, "b2", 0)).id)]);
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

    assert_eq!(incoming(phi), vec![(Some(at(&unit, "b0")), graph.at(site(&unit, "b0", 0)).id), (Some(at(&unit, "b2")), graph.at(site(&unit, "b2", 0)).id)]);
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
        assert_eq!(graph.clobbers(load, &cell(&unit, load)).contains(&graph.at(site(&unit, "b0", 0)).id), clobbers, "{write}");
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
    assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(site(&unit, "b0", 0)).id, graph.at(site(&unit, "b2", 0)).id]));
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
    assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(site(&unit, "b1", 0)).id, graph.at(site(&unit, "b2", 0)).id]));
}

#[test]
fn test_clobber_preserves_partial_and_unknown_writes_and_calls() {
    // A word write at +1 changes one byte of the word being loaded; a
    // pointer argument may point anywhere.
    for write in ["store i16 0, ptr getelementptr (i8, ptr @g, i16 33)", "store i16 0, ptr %p", "call void @anything()"] {
        let parsed = written(write);
        let unit = parsed.unit();
        let graph = graph(&unit);
        let load = site(&unit, "b0", 1);
        assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(site(&unit, "b0", 0)).id]), "{write}");
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
        let graph = built(&unit, &Accesses::plain(&unit, &Calls::from_iter([(call, vec![cell(&unit, footprint)])])));
        assert_eq!(graph.clobbers(load, &cell(&unit, load)), BTreeSet::from([graph.at(clobber).id]));
    }
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
        assert!(!graph.available_on_edge(site(&unit, arm, 0), load, at(&unit, arm), &memory, Some(&translated)), "{pointer}");
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

/// `@g` and `@h` are apart only once alias names each access's object; a
/// call is apart only once its callee's summary says it writes `@h` alone.
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
    let seth = Procedure::of(Unit::of(&parsed.module, &parsed.layout, function(&parsed.module, "seth")));
    let known = alias::summaries(&IndexMap::from_iter([("seth".to_owned(), seth)]), None).unwrap();
    let (first, second, call, load) = (site(&unit, "b0", 0), site(&unit, "b0", 1), site(&unit, "b0", 2), site(&unit, "b0", 3));
    for (accesses, clobber) in [
        (Accesses::plain(&unit, &Calls::default()), call),
        (Accesses::resolved(&unit, &IndexMap::default()).unwrap(), call),
        (Accesses::resolved(&unit, &known).unwrap(), first),
    ] {
        let graph = built(&unit, &accesses);
        assert_eq!(graph.clobbers(load, &accesses.references[&load]), BTreeSet::from([graph.at(clobber).id]));
    }
    let plain = Accesses::plain(&unit, &Calls::from_iter([(call, vec![])]));
    assert_eq!(built(&unit, &plain).clobbers(load, &plain.references[&load]), BTreeSet::from([graph(&unit).at(second).id]), "unresolved, @h may be @g");
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
        assert_eq!((accesses.reads(inst), accesses.writes(inst)), (named(own.reads), named(own.writes)), "Accesses, access {index}");
        assert!(!crate::memory::unmodeled_write(&unit, inst), "memory::unmodeled_write, access {index}");
        let context = &parsed.module.context;
        let unmodeled = (crate::effects::unmodeled_read(context, &declarations, unit.function, inst), crate::effects::unmodeled_write(context, &declarations, unit.function, inst));
        assert_eq!(unmodeled, (false, false), "effects::unmodeled, access {index}");
    }
}

/// Each load of one address walked back over every store between it and the first, asking each store's
/// clobbers afresh: `mir gvn` was 10 s of compiling 1600 BASIC statements (#560). A (cell, store) pair is
/// worked out once.
#[test]
fn test_loads_of_one_address_ask_each_store_whether_it_clobbers_once() {
    let steps: String = (0..30).map(|at| format!("  store i16 {at}, ptr {OTHER}\n  %y{at} = load i16, ptr {CELL}\n")).collect();
    let parsed = Parsed::new(&format!("define void @f() {{\nb0:\n  %x = load i16, ptr {CELL}\n{steps}  ret void\n}}\n"));
    let unit = parsed.unit();
    let graph = graph(&unit);
    let first = site(&unit, "b0", 0);
    let before = clobber_runs();
    for at in 0..30 {
        let later = site(&unit, "b0", 2 + 2 * at);
        assert!(graph.unchanged(first, later, &cell(&unit, later)));
    }
    assert!(clobber_runs() - before <= 30, "{} clobber questions for 30 stores", clobber_runs() - before);
}
