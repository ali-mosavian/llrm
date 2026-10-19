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
    unit.function.iter_block(block(unit.function, name)).nth(index).unwrap()
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
    // The one store's cell is compared with the cells of its groups (not by
    // bucket picks); the loads pick none.
    assert_eq!(picked, 0, "{picked} bucket picks for 12 loads after which nothing is overwritten");
}
