//! llrm-core's `analysis/avail_tests.rs`, written as rich MIR.
//!
//! Skipped, x86 operand shapes:
//! `test_preserved_allows_a_move_and_refuses_a_binary`
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
        crate::testing::with_registers(Unit::of(&self.module, &self.layout, function(&self.module, "f")))
    }
}

const CELL: &str = "getelementptr (i8, ptr @g, i16 32)";
const OTHER: &str = "getelementptr (i8, ptr @g, i16 48)";

/// Instruction `index` of block `name`.
fn site(
    unit: &Unit,
    name: &str,
    index: usize,
) -> InstId {
    unit.function.block(block(unit.function, name)).instructions()[index]
}

fn named(
    unit: &Unit,
    name: &str,
) -> Operand {
    Operand::Value(value(unit.function, name))
}

fn cell(
    unit: &Unit,
    inst: InstId,
) -> MemRef {
    MemRef::of(unit, inst).expect("a load or store")
}

/// What forwarding offers each load of `@f`.
fn forwarded(
    unit: &Unit,
    calls: &Calls,
) -> Vec<Forward> {
    let loads = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| unit.function.instruction(inst).opcode.mnemonic() == "load")
        .collect();
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
        assert_eq!(
            provider(&unit, &Accesses::plain(&unit, &Calls::default()), load, &cell(&unit, load)),
            kept.then(|| named(&unit, "v")),
            "{between}"
        );
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
            Some(at) => Calls::from_iter([(
                call,
                std::rc::Rc::from(vec![cell(
                    &unit,
                    if at == CELL { site(&unit, "b0", 0) } else { site(&unit, "spare", 0) },
                )]),
            )]),
        };
        let found = forwarded(&unit, &calls);
        assert_eq!(
            found,
            if reused { vec![Forward { at: site(&unit, "b0", 2), value: named(&unit, "v") }] } else { vec![] },
            "{footprint:?}"
        );
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
        let calls = if disjoint {
            Calls::from_iter([(site(&unit, "b0", 1), std::rc::Rc::from(vec![cell(&unit, site(&unit, "b0", 3))]))])
        } else {
            Calls::default()
        };
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
    let calls = Calls::from_iter([(site(&unit, "b0", 1), std::rc::Rc::from([]))]);
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
    assert_eq!(
        dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), Some(&private)),
        vec![site(&unit, "b0", 1)]
    );
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
    assert_eq!(
        forwarded(&unit, &Calls::default()),
        vec![Forward { at: site(&unit, "b1", 0), value: named(&unit, "v") }]
    );
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
        assert_eq!(
            found,
            if reused { vec![Forward { at: site(&unit, "b1", 0), value: named(&unit, "a") }] } else { vec![] },
            "{write}"
        );
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
        assert_eq!(
            provider(&unit, &Accesses::plain(&unit, &Calls::default()), load, &cell(&unit, load)),
            kept.then(|| named(&unit, "v")),
            "{right}"
        );
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
    assert_eq!(
        found,
        vec![
            Forward { at: site(&unit, "b0", 1), value: seven },
            Forward { at: site(&unit, "b0", 3), value: named(&unit, "y") }
        ]
    );
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
        assert_eq!(
            provider(&unit, &Accesses::plain(&unit, &Calls::default()), load, &cell(&unit, load)),
            kept.then(|| named(&unit, "v")),
            "{write}"
        );
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
        let parsed = Parsed::new(
            &format!(
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
            .replace("OTHER", OTHER),
        );
        let unit = parsed.unit();
        let removed = dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), None);
        assert_eq!(removed, if dead { vec![site(&unit, "b0", 0)] } else { vec![] }, "{between} / {last}");
    }
}

/// Across a branch the cell must be overwritten on every path.
#[test]
fn test_a_store_is_dead_across_a_branch_only_when_every_path_overwrites_it() {
    for (right, dead) in [("store i16 3, ptr CELL", true), ("%y = load i16, ptr CELL", false), ("", false)] {
        let parsed = Parsed::new(
            &format!(
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
            .replace("CELL", CELL),
        );
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
        let removed = dead_stores(
            &unit,
            &Accesses::plain(&unit, &Calls::from_iter([(invoke, std::rc::Rc::from([]))])),
            Some(&private),
        );
        assert_eq!(removed.contains(&site(&unit, "b0", 1)), dead, "{handler}");
    }
}

/// Each load the loop lost was compared with every earlier load, one by one,
/// though most are of one address (#560). Loads of one address are compared
/// once.
#[test]
fn test_loads_of_one_address_are_compared_with_a_missing_one_once() {
    let before_loop: String = (0..20).map(|at| format!("  %a{at} = load i16, ptr {CELL}\n")).collect();
    let in_loop: String = (0..20).map(|at| format!("  %b{at} = load i16, ptr {CELL}\n")).collect();
    let parsed = Parsed::new(&format!(
        "define i16 @f(i1 %c) {{\nb0:\n{before_loop}  br label %b1\n\nb1:\n{in_loop}  store i16 0, ptr {OTHER}\n  br i1 %c, label %b1, label %b2\n\nb2:\n  ret i16 %b0\n}}\n"
    ));
    let unit = parsed.unit();
    let before = same_runs();
    let found = forwarded(&unit, &Calls::default());
    assert_eq!(found.len(), 39, "every load but the first is served");
    assert!(same_runs() - before <= 20, "{} comparisons for 20 loads of one address", same_runs() - before);
}

/// 200 stores to 200 different cells of one array asked every cell held of each
/// write: 19,900 clobber questions where the bytes a write meets hold one cell.
#[test]
fn test_a_store_asks_only_the_cells_it_can_reach() {
    let stores: String =
        (0..200).map(|at| format!("  store i16 %v, ptr getelementptr (i8, ptr @big, i16 {})\n", at * 2)).collect();
    let parsed = Parsed::new(&format!(
        "@big = global [400 x i8] zeroinitializer\n\ndefine void @f(i16 %v) {{\nb0:\n{stores}  ret void\n}}\n"
    ));
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &Calls::default());
    let before = clobber_asks();
    let held = holders(&unit, &accesses);
    let asked = clobber_asks() - before;
    assert_eq!(held.outof.values().map(|cells| cells.len()).max(), Some(200));
    assert!(asked <= 1_000, "{asked} clobber questions for 200 stores to disjoint cells");
}

/// The cell naming a load's bytes was found by a scan of every cell held. It is
/// asked of the cells a write to the load's address can reach, and must be the
/// one the scan finds (the first held), at every point of a function whose
/// cells overlap and are rewritten.
#[test]
fn test_the_held_cell_naming_a_loads_bytes_is_the_one_a_scan_finds() {
    let parsed = Parsed::new(&format!(
        "define i8 @f(i1 %c) {{\nb0:\n  store i8 1, ptr {CELL}\n  store i8 2, ptr {OTHER}\n  %a = load i8, ptr {CELL}\n  br i1 %c, label %b1, label %b2\n\nb1:\n  store i8 3, ptr {CELL}\n  %b = load i8, ptr {OTHER}\n  br label %b3\n\nb2:\n  %d = load i8, ptr {CELL}\n  br label %b3\n\nb3:\n  %e = load i8, ptr {CELL}\n  %f = load i8, ptr {OTHER}\n  ret i8 %e\n}}\n"
    ));
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &Calls::default());
    let held = holders(&unit, &accesses);
    let loads: Vec<_> =
        unit.function.walk().map(|(_, inst)| inst).filter_map(|inst| loaded_into(&unit, &accesses, inst)).collect();
    assert!(loads.len() >= 5);
    let mut asked = 0;
    for into in held.into.values().chain(held.outof.values()) {
        for (cell, _) in &loads {
            let whole = into.iter().find(|(one, _)| same_bytes(&unit, one, cell));
            assert_eq!(into.naming(&unit, cell, |_| true), whole);
            asked += usize::from(whole.is_some());
        }
    }
    assert!(asked > 0, "no held cell named a load's bytes: the test asks nothing");
}

#[test]
fn a_dead_store_solve_picks_no_buckets_where_nothing_is_overwritten_yet() {
    // Every load and store asked overlap_buckets what it could clobber, even
    // with no cell overwritten to forget: 50% of dse in QCport's host.c.
    let loads: String =
        (0..12).map(|i| format!("  %l{i} = load i16, ptr getelementptr (i8, ptr @g, i16 {})\n", 2 * i)).collect();
    let parsed =
        Parsed::new(&format!("define i16 @f() {{\nb0:\n  store i16 1, ptr {CELL}\n{loads}  ret i16 %l0\n}}\n"));
    let unit = parsed.unit();
    let before = crate::regions::PICKED.with(|picked| picked.get().0);
    let removed = dead_stores(&unit, &Accesses::plain(&unit, &Calls::default()), None);
    let picked = crate::regions::PICKED.with(|picked| picked.get().0) - before;
    assert_eq!(removed, vec![]);
    assert_eq!(picked, 0, "{picked} bucket picks for 12 loads after which nothing is overwritten");
}

/// `memory_providers` compared each missing load's cell with every group of
/// loads (`same_bytes`, two `covered` calls each): quadratic in the distinct
/// cells a function reads, 0.4% of host.c's compile and what the `cells` axis
/// grows. Groups are found by what a group naming the same bytes shares.
#[test]
fn a_load_is_compared_with_the_groups_that_share_its_bytes_not_with_every_group() {
    let loads: String = (0..40)
        .map(|k| format!("  %a{k} = load i16, ptr getelementptr (i8, ptr @g, i16 {})\n  %b{k} = load i16, ptr getelementptr (i8, ptr @g, i16 {})\n", 2 * k, 2 * k))
        .collect();
    let parsed = Parsed::new(&format!("define i16 @f() {{\nb0:\n{loads}  ret i16 %a0\n}}\n"));
    let unit = parsed.unit();
    let accesses = Accesses::plain(&unit, &Calls::default());
    let missing: Vec<InstId> = unit
        .function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(unit.function.instruction(inst).opcode, Opcode::Load { .. }))
        .collect();
    assert_eq!(missing.len(), 80);
    let before = same_runs();
    let found = memory_providers(&unit, &accesses, &missing);
    let compared = same_runs() - before;
    assert_eq!(found.len(), 40, "each second load of a cell takes the first");
    assert!(compared <= 3 * 80, "{compared} group comparisons for 80 loads of 40 cells");
}
