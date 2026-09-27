//! llrm-core's `analysis/avail_tests.rs`, written as rich MIR.
//!
//! Skipped, x86 operand shapes: `test_preserved_allows_a_move_and_refuses_a_binary`
//! and `test_explicit_load_operand_is_not_a_preserved_half`. A rich MIR
//! load's only operand is its pointer.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;

use super::*;
use crate::consts::Calls;
use crate::testing::{DOS, block, function, layout, parsed, value};

struct Parsed {
    module: Module,
    layout: DataLayout,
}

impl Parsed {
    fn new(body: &str) -> Self {
        let module = parsed(&format!(
            "{DOS}@g = global [64 x i8] zeroinitializer
declare void @anything()
declare void @setter() memory(write)

{body}"
        ));
        let layout = layout(&module);
        Self { module, layout }
    }

    fn unit(&self) -> Unit<'_> {
        Unit::of(&self.module, &self.layout, function(&self.module, "f"))
    }
}

const CELL: &str = "getelementptr (i8, ptr @g, i16 32)";
const OTHER: &str = "getelementptr (i8, ptr @g, i16 48)";

/// Instruction `index` of block `name`.
fn site(unit: &Unit, name: &str, index: usize) -> InstId {
    unit.function.block(block(unit.function, name)).instructions()[index]
}

fn named(unit: &Unit, name: &str) -> Operand {
    Operand::Value(value(unit.function, name))
}

fn cell(unit: &Unit, inst: InstId) -> MemRef {
    MemRef::of(unit, inst).expect("a load or store")
}

/// What forwarding offers each load of `@f`.
fn forwarded(unit: &Unit, calls: &Calls) -> Vec<Forward> {
    let loads = unit.function.walk().map(|(_, inst)| inst).filter(|&inst| unit.function.instruction(inst).opcode.mnemonic() == "load").collect();
    forwardable(unit, &Accesses::plain(unit, calls), &loads)
}

/// `%v` stored to the cell, `between`, then the cell loaded.
fn around(between: &str) -> Parsed {
    Parsed::new(&format!(
        "define i16 @f(i16 %v, i16 %w) {{
b0:
  store i16 %v, ptr {CELL}
  {between}
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
    ))
}

/// A call invalidates memory; an operation touching none does not.
#[test]
fn test_current_mir_decides_whether_a_call_invalidates_memory() {
    for (between, kept) in [("call void @anything()", false), ("%s = add i16 %v, %w", true)] {
        let parsed = around(between);
        let unit = parsed.unit();
        let load = site(&unit, "b0", 2);
        assert_eq!(provider(&unit, &Accesses::plain(&unit, &Calls::default()), load, &cell(&unit, load)), kept.then(|| named(&unit, "v")), "{between}");
    }
}

// ---- tests/test_availability_call_effects.py ----

#[test]
fn test_unknown_call_effects_keep_the_load() {
    let parsed = around("call void @anything()");
    assert!(forwarded(&parsed.unit(), &Calls::default()).is_empty());
}

/// A call's footprint keeps what it misses and only that.
#[test]
fn test_a_footprint_preserves_only_disjoint_values() {
    for (footprint, reused) in [(None, false), (Some(OTHER), true), (Some(CELL), false)] {
        let parsed = Parsed::new(&format!(
            "define i16 @f(i16 %v) {{
b0:
  store i16 %v, ptr {CELL}
  call void @anything()
  %x = load i16, ptr {CELL}
  ret i16 %x

spare:
  store i16 0, ptr {OTHER}
  ret i16 0
}}
"
        ));
        let unit = parsed.unit();
        let call = site(&unit, "b0", 1);
        let calls: Calls = match footprint {
            None => Calls::default(),
            Some(at) => Calls::from_iter([(call, vec![cell(&unit, if at == CELL { site(&unit, "b0", 0) } else { site(&unit, "spare", 0) })])]),
        };
        let found = forwarded(&unit, &calls);
        assert_eq!(found, if reused { vec![Forward { at: site(&unit, "b0", 2), value: named(&unit, "v") }] } else { vec![] }, "{footprint:?}");
    }
}

/// A setter whose footprint misses the cell neither reads the first store
/// nor keeps it from being dead; an unknown one writes anything.
#[test]
fn test_dead_store_uses_call_memory_effects() {
    for disjoint in [false, true] {
        let parsed = Parsed::new(&format!(
            "define void @f(i16 %v) {{
b0:
  store i16 %v, ptr {CELL}
  call void @setter()
  store i16 %v, ptr {CELL}
  store i16 0, ptr {OTHER}
  ret void
}}
"
        ));
        let unit = parsed.unit();
        let calls = if disjoint { Calls::from_iter([(site(&unit, "b0", 1), vec![cell(&unit, site(&unit, "b0", 3))])]) } else { Calls::default() };
        let removed = dead_stores(&unit, &Accesses::plain(&unit, &calls), None);
        assert_eq!(removed, if disjoint { vec![site(&unit, "b0", 0)] } else { vec![] }, "{disjoint}");
    }
}

/// A call that may read keeps the store before it.
#[test]
fn test_a_reading_call_makes_a_prior_store_observable() {
    let parsed = Parsed::new(&format!(
        "define void @f(i16 %v) {{
b0:
  store i16 %v, ptr {CELL}
  call void @anything()
  store i16 %v, ptr {CELL}
  ret void
}}
"
    ));
    let unit = parsed.unit();
    let calls = Calls::from_iter([(site(&unit, "b0", 1), vec![])]);
    assert_eq!(dead_stores(&unit, &Accesses::plain(&unit, &calls), None), vec![]);
}

/// procs' TWICE kept two dead frame stores once its return read through an
/// unknown pointer.
#[test]
fn test_an_unknown_pointer_does_not_observe_a_private_cell() {
    let parsed = Parsed::new(
        "define i16 @f(ptr %p) {
b0:
  %s = alloca i16
  store i16 1, ptr %s
  %x = load i16, ptr %p
  ret i16 %x
}
",
    );
    let unit = parsed.unit();
    let slot = named(&unit, "s");
    let private = |one: &MemRef| one.root == Some(slot);
    assert_eq!(dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), Some(&private)), vec![site(&unit, "b0", 1)]);
    assert_eq!(dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), None), vec![], "the caller may read it");
}

// ---- tests/test_memoryssa_forward.py ----

/// A store before a loop, the loop reading the cell and writing `write`.
fn looped(write: &str) -> Parsed {
    Parsed::new(&format!(
        "define i16 @f(i1 %c, i16 %v) {{
b0:
  store i16 %v, ptr {CELL}
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
fn test_preheader_store_serves_a_loop_read() {
    let parsed = looped(&format!("store i16 0, ptr {OTHER}"));
    let unit = parsed.unit();
    assert_eq!(forwarded(&unit, &Calls::default()), vec![Forward { at: site(&unit, "b1", 0), value: named(&unit, "v") }]);
}

#[test]
fn test_aliasing_backedge_keeps_the_load() {
    let parsed = looped(&format!("store i16 0, ptr {CELL}"));
    assert!(forwarded(&parsed.unit(), &Calls::default()).is_empty());
}

#[test]
fn test_call_on_backedge_invalidates_the_preheader_store() {
    let parsed = looped("call void @anything()");
    assert!(forwarded(&parsed.unit(), &Calls::default()).is_empty());
}

#[test]
fn test_store_on_only_one_entry_path_cannot_supply_the_load() {
    let parsed = Parsed::new(&format!(
        "define i16 @f(i1 %c, i16 %v) {{
b4:
  br i1 %c, label %b0, label %b1

b0:
  store i16 %v, ptr {CELL}
  br label %b1

b1:
  %x = load i16, ptr {CELL}
  br i1 %c, label %b2, label %b3

b2:
  store i16 0, ptr {OTHER}
  br label %b1

b3:
  ret i16 %x
}}
"
    ));
    assert!(forwarded(&parsed.unit(), &Calls::default()).is_empty());
}

/// A load before a loop serves the loop's reads across disjoint writes only.
#[test]
fn test_preheader_load_serves_loop_reads_across_disjoint_writes() {
    for (write, reused) in [(OTHER, true), (CELL, false)] {
        let parsed = Parsed::new(&format!(
            "define i16 @f(i1 %c) {{
b0:
  %a = load i16, ptr {CELL}
  br label %b1

b1:
  %b = load i16, ptr {CELL}
  store i16 0, ptr {write}
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %b
}}
"
        ));
        let unit = parsed.unit();
        let found = forwarded(&unit, &Calls::default());
        assert_eq!(found, if reused { vec![Forward { at: site(&unit, "b1", 0), value: named(&unit, "a") }] } else { vec![] }, "{write}");
    }
}

// ---- behaviour ----

/// A join keeps a value only where every predecessor holds the same one.
#[test]
fn test_a_join_keeps_what_every_predecessor_agrees_on() {
    for (right, kept) in [("%v", true), ("%w", false)] {
        let parsed = Parsed::new(&format!(
            "define i16 @f(i1 %c, i16 %v, i16 %w) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 %v, ptr {CELL}
  br label %b3

b2:
  store i16 {right}, ptr {CELL}
  br label %b3

b3:
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
        ));
        let unit = parsed.unit();
        let load = site(&unit, "b3", 0);
        assert_eq!(provider(&unit, &Accesses::plain(&unit, &Calls::default()), load, &cell(&unit, load)), kept.then(|| named(&unit, "v")), "{right}");
    }
}

/// A store of a constant, or of another load's value, serves a later load.
#[test]
fn test_constants_and_loaded_values_are_providers() {
    let parsed = Parsed::new(&format!(
        "define i16 @f() {{
b0:
  store i16 7, ptr {CELL}
  %x = load i16, ptr {CELL}
  %y = load i16, ptr {OTHER}
  %z = load i16, ptr {OTHER}
  %s = add i16 %x, %z
  ret i16 %s
}}
"
    ));
    let unit = parsed.unit();
    let found = forwarded(&unit, &Calls::default());
    let seven = unit.function.instruction(site(&unit, "b0", 0)).operands[0];
    assert_eq!(found, vec![Forward { at: site(&unit, "b0", 1), value: seven }, Forward { at: site(&unit, "b0", 3), value: named(&unit, "y") }]);
}

/// Neither a volatile read nor a read of another type is served.
#[test]
fn test_a_volatile_or_retyped_read_is_not_forwarded() {
    let parsed = Parsed::new(&format!(
        "define i16 @f(ptr %p) {{
b0:
  store ptr %p, ptr {CELL}
  %x = load i16, ptr {CELL}
  store i16 1, ptr {OTHER}
  %y = load volatile i16, ptr {OTHER}
  %s = add i16 %x, %y
  ret i16 %s
}}
"
    ));
    let unit = parsed.unit();
    assert_eq!(forwarded(&unit, &Calls::default()), vec![]);
}

/// A store through a pointer that may alias the cell forgets it; a store at
/// another displacement of the same object does not.
#[test]
fn test_a_may_alias_store_forgets_the_cell() {
    for (write, kept) in [("%p", false), (OTHER, true)] {
        let parsed = Parsed::new(&format!(
            "define i16 @f(ptr %p, i16 %v) {{
b0:
  store i16 %v, ptr {CELL}
  store i16 0, ptr {write}
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
        ));
        let unit = parsed.unit();
        let load = site(&unit, "b0", 2);
        assert_eq!(provider(&unit, &Accesses::plain(&unit, &Calls::default()), load, &cell(&unit, load)), kept.then(|| named(&unit, "v")), "{write}");
    }
}

/// Which stores a later covering store makes dead, in one block.
#[test]
fn test_a_store_is_dead_only_when_overwritten_before_any_read() {
    for (between, last, dead) in [
        ("", "store i16 2, ptr CELL", true),
        ("%x = load i16, ptr CELL", "store i16 2, ptr CELL", false),
        ("%x = load i16, ptr %p", "store i16 2, ptr CELL", false),
        ("", "store i32 2, ptr CELL", true),
        ("", "store i8 2, ptr CELL", false),
        ("", "store i16 2, ptr getelementptr (i8, ptr @g, i16 33)", false),
        // A volatile store writes only its own bytes.
        ("store volatile i16 3, ptr OTHER", "store i16 2, ptr CELL", true),
    ] {
        let parsed = Parsed::new(&format!(
            "define void @f(ptr %p) {{
b0:
  store i16 1, ptr CELL
  {between}
  {last}
  ret void
}}
"
        )
        .replace("CELL", CELL)
        .replace("OTHER", OTHER));
        let unit = parsed.unit();
        let removed = dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), None);
        assert_eq!(removed, if dead { vec![site(&unit, "b0", 0)] } else { vec![] }, "{between} / {last}");
    }
}

/// Across a branch the cell must be overwritten on every path.
#[test]
fn test_a_store_is_dead_across_a_branch_only_when_every_path_overwrites_it() {
    for (right, dead) in [("store i16 3, ptr CELL", true), ("%y = load i16, ptr CELL", false), ("", false)] {
        let parsed = Parsed::new(&format!(
            "define void @f(i1 %c) {{
b0:
  store i16 1, ptr CELL
  br i1 %c, label %b1, label %b2

b1:
  store i16 2, ptr CELL
  ret void

b2:
  {right}
  ret void
}}
"
        )
        .replace("CELL", CELL));
        let unit = parsed.unit();
        let removed = dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), None);
        assert_eq!(removed.contains(&site(&unit, "b0", 0)), dead, "{right}");
    }
}

/// A store in a loop is read after the loop exits: never dead.
#[test]
fn test_a_loop_store_read_after_the_exit_is_live() {
    let parsed = Parsed::new(&format!(
        "define i16 @f(i1 %c, i16 %v) {{
b0:
  br label %b1

b1:
  store i16 %v, ptr {CELL}
  br i1 %c, label %b1, label %b2

b2:
  %x = load i16, ptr {CELL}
  ret i16 %x
}}
"
    ));
    let unit = parsed.unit();
    assert_eq!(dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), None), vec![]);
}

/// A handler reached along an unwind edge reads what was stored.
#[test]
fn test_an_unwind_edge_keeps_a_store_its_handler_reads() {
    for (handler, dead) in ["%x = load i16, ptr %s\n  ret i16 %x", "unreachable"].into_iter().zip([false, true]) {
        let parsed = Parsed::new(&format!(
            "declare i32 @__gxx_personality_v0(...)

define i16 @f() personality ptr @__gxx_personality_v0 {{
b0:
  %s = alloca i16
  store i16 1, ptr %s
  invoke void @setter() to label %ok unwind label %pad

ok:
  store i16 2, ptr %s
  ret i16 0

pad:
  %lp = landingpad {{ ptr, i32 }} cleanup
  {handler}
}}
"
        ));
        let unit = parsed.unit();
        let invoke = site(&unit, "b0", 2);
        let slot = named(&unit, "s");
        let private = |one: &MemRef| one.root == Some(slot);
        let removed = dead_stores(&unit, &Accesses::plain(&unit, &Calls::from_iter([(invoke, vec![])])), Some(&private));
        assert_eq!(removed.contains(&site(&unit, "b0", 1)), dead, "{handler}");
    }
}
