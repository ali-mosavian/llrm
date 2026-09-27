//! llrm-core's `optimize/loadjoins_tests.rs`, written as rich MIR, each
//! change also run by the interpreter before and after.
//!
//! Skipped, with no rich MIR meaning: the `Fcheck` cases (x87 checks), and
//! the `division` guard (division is C's). The old `critical` guard, an
//! implicit edge, is a switch edge here: a conditional one is split.

use llrm_mir::module::Module;
use llrm_mir::passes::PassManager;

use super::*;
use llrm_analysis::manager::Summaries;
use llrm_analysis::testing::DOS;

use crate::testing::{parsed, printed, results};

const CELL: &str = "getelementptr (i8, ptr @g, i16 32)";

/// `module` after the pass, which must leave it verifying.
fn joined(mut module: Module, insert: bool) -> Module {
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.require::<Summaries>();
    manager.add(LoadJoins { insert });
    manager.run(&mut module).expect("verifies");
    module
}

fn module(body: &str) -> Module {
    parsed(&format!("{DOS}@g = global [64 x i8] zeroinitializer\ndeclare void @anything()\n\n{}", body.replace("CELL", CELL)))
}

/// `@f` of `body` after the pass, printed; the pass again changes nothing,
/// and the interpreter finds the same results for each of `inputs`.
fn after(body: &str, insert: bool, inputs: &[&[i128]]) -> String {
    let before = module(body);
    let once = joined(before.clone(), insert);
    let text = printed(&once);
    assert_eq!(printed(&joined(parsed(&text), insert)), text, "a second run changes nothing");
    assert_eq!(results(&once, inputs), results(&before, inputs), "{text}");
    text.split("define").find(|f| f.contains(" @f(")).map(|f| format!("define{f}")).expect("@f")
}

/// `body` is left as it was.
fn unchanged(body: &str, insert: bool) {
    assert_eq!(printed(&joined(module(body), insert)), printed(&module(body)));
}

// ---- tests/test_memory_joins.py ----

const DIAMOND: &str = "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 10, ptr CELL
  LEFT
  br label %b3

b2:
  store i16 20, ptr CELL
  br label %b3

b3:
  JOIN
  %x = load i16, ptr CELL
  ret i16 %x
}
";

fn diamond(left: &str, join: &str) -> String {
    DIAMOND.replace("LEFT", left).replace("JOIN", join)
}

const BOTH: &[&[i128]] = &[&[0], &[1]];

#[test]
fn test_join_load_uses_each_predecessors_stored_value() {
    let expected = "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  store i16 10, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b2:
  store i16 20, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b3:
  %x1 = phi i16 [ 10, %b1 ], [ 20, %b2 ]
  ret i16 %x1
}
";
    assert_eq!(after(&diamond("", ""), false, BOTH), expected);
}

/// The join laid out before its arms makes the same phi.
#[test]
fn test_join_load_is_independent_of_block_order() {
    let body = "define i16 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b3:
  %x = load i16, ptr CELL
  ret i16 %x

b2:
  store i16 20, ptr CELL
  br label %b3

b1:
  store i16 10, ptr CELL
  br label %b3
}
";
    let text = after(body, false, BOTH);
    assert!(text.contains("%x1 = phi i16 [ 10, %b1 ], [ 20, %b2 ]"), "{text}");
}

const POINTERS: &str = "define i16 @f(i1 %c, i16 %i, i16 %j) {
b0:
  %a = getelementptr i8, ptr @g, i16 %i
  %b = getelementptr i8, ptr @g, i16 %j
  br i1 %c, label %b1, label %b2

b1:
  store i16 10, ptr %a
  br label %b3

b2:
  store i16 20, ptr %b
  br label %b3

b3:
  %q = phi ptr [ LEFT, %b1 ], [ RIGHT, %b2 ]
  JOIN
  %x = load i16, ptr %q
  ret i16 %x
}
";

const INDICES: &[&[i128]] = &[&[0, 4, 6], &[1, 4, 6], &[1, 4, 4], &[0, 6, 4]];

/// ARRPHI still reloaded both elements after sharing their addresses across branches.
#[test]
fn test_pointer_phi_selects_the_matching_store_on_each_edge() {
    let body = POINTERS.replace("LEFT", "%a").replace("RIGHT", "%b").replace("JOIN", "");
    let text = after(&body, false, INDICES);
    assert!(text.contains("%x1 = phi i16 [ 10, %b1 ], [ 20, %b2 ]") && text.contains("ret i16 %x1"), "{text}");
}

#[test]
fn test_crossed_pointer_phi_does_not_reuse_the_other_branches_store() {
    unchanged(&POINTERS.replace("LEFT", "%b").replace("RIGHT", "%a").replace("JOIN", ""), false);
}

#[test]
fn test_join_prefix_write_through_the_pointer_phi_blocks_reuse() {
    for insert in [false, true] {
        unchanged(&POINTERS.replace("LEFT", "%a").replace("RIGHT", "%b").replace("JOIN", "store i16 99, ptr %q"), insert);
    }
}

#[test]
fn test_observable_call_invalidates_the_join_value() {
    for (left, join) in [("call void @anything()", ""), ("", "call void @anything()")] {
        unchanged(&diamond(left, join), false);
    }
}

#[test]
fn test_a_missing_edge_provider_keeps_the_load() {
    unchanged(&diamond("", "").replace("  store i16 20, ptr CELL\n", ""), false);
}

#[test]
fn test_a_partial_overwrite_invalidates_the_whole_value() {
    for (left, join) in [("store i8 99, ptr CELL", ""), ("", "store i8 99, ptr CELL")] {
        unchanged(&diamond(left, join), false);
    }
}

#[test]
fn test_predecessor_loads_can_supply_the_join_without_stores() {
    let body = diamond("", "").replace("store i16 10, ptr CELL", "%y = load i16, ptr CELL").replace("store i16 20, ptr CELL", "%z = load i16, ptr CELL");
    assert!(after(&body, false, BOTH).contains("%x1 = phi i16 [ %y, %b1 ], [ %z, %b2 ]"));
}

// ---- tests/test_load_pre.py ----

/// One arm stores `%v`; the join loads it.
const PRE: &str = "define i16 @f(i1 %c, i1 %d, i16 %v, i16 %s) {
b0:
  store i16 %v, ptr CELL
  br i1 %c, label %b1, label %b2

b1:
  %y = load i16, ptr CELL
  br label %b3

b2:
  ARM
  br label %b3

b3:
  JOIN
  %x = load i16, ptr CELL
  ret i16 %x
}
";

const PRE_INPUTS: &[&[i128]] = &[&[0, 0, 5, 1], &[1, 0, 5, 1], &[0, 1, 7, 2], &[1, 1, 7, 2]];

fn pre(arm: &str, join: &str) -> String {
    PRE.replace("ARM", arm).replace("JOIN", join)
}

#[test]
fn test_missing_path_reads_once_and_existing_path_reuses_value() {
    let expected = "define i16 @f(i1 %c, i1 %d, i16 %v, i16 %s) {
b0:
  store i16 %v, ptr getelementptr (i8, ptr @g, i16 32)
  br i1 %c, label %b1, label %b2

b1:
  %y = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b2:
  %0 = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b3:
  %x1 = phi i16 [ %y, %b1 ], [ %0, %b2 ]
  ret i16 %x1
}
";
    assert_eq!(after(&pre("", ""), true, PRE_INPUTS), expected);
    unchanged(&pre("", ""), false);
}

/// An operation that only computes a value is no reason to keep the load.
#[test]
fn test_load_insertion_crosses_a_pure_join_prefix() {
    assert!(after(&pre("", "%t = add i16 %s, 1"), true, PRE_INPUTS).contains("b2:\n  %0 = load i16"));
}

#[test]
fn test_load_insertion_cannot_speculate_or_cross_observable_operations() {
    let switch = "define i16 @f(i1 %c, i16 %s) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %y = load i16, ptr CELL
  br label %b3

b2:
  switch i16 %s, label %b3 [ i16 1, label %b4 ]

b3:
  %x = load i16, ptr CELL
  ret i16 %x

b4:
  ret i16 0
}
";
    unchanged(switch, true);
    for join in ["call void @anything()", "store i16 1, ptr getelementptr (i8, ptr @g, i16 48)"] {
        unchanged(&pre("", join), true);
    }
    unchanged(&pre("", "").replace("  %y = load i16, ptr CELL\n", ""), true);
}

#[test]
fn test_inserted_address_uses_the_missing_edges_pointer() {
    let body = POINTERS.replace("LEFT", "%a").replace("RIGHT", "%b").replace("JOIN", "").replace("store i16 10, ptr %a", "%y = load i16, ptr %a").replace("  store i16 20, ptr %b\n", "");
    let text = after(&body, true, INDICES);
    assert!(text.contains("b2:\n  %0 = load i16, ptr %b\n  br label %b3"), "{text}");
    assert!(text.contains("%x1 = phi i16 [ %y, %b1 ], [ %0, %b2 ]"), "{text}");
}

/// The unrelated arm must not acquire a read that could fault or observe
/// memory: the load goes on the edge, taken or not.
#[test]
fn test_missing_conditional_edge_gets_its_own_load_block() {
    for (branch, split) in [("br i1 %d, label %b3, label %b4", "br i1 %d, label %0, label %b4"), ("br i1 %d, label %b4, label %b3", "br i1 %d, label %b4, label %0")] {
        let body = format!(
            "define i16 @f(i1 %c, i1 %d, i16 %v, i16 %s) {{
b0:
  store i16 %v, ptr CELL
  br i1 %c, label %b1, label %b2

b1:
  %y = load i16, ptr CELL
  br label %b3

b2:
  {branch}

b3:
  %x = load i16, ptr CELL
  ret i16 %x

b4:
  ret i16 0
}}
"
        );
        let text = after(&body, true, PRE_INPUTS);
        assert!(text.contains(&format!("b2:\n  {split}\n")), "{text}");
        assert!(text.contains("\n0:\n  %1 = load i16, ptr getelementptr (i8, ptr @g, i16 32)\n  br label %b3"), "{text}");
        assert!(text.contains("%x1 = phi i16 [ %y, %b1 ], [ %1, %0 ]"), "{text}");
    }
}

// ---- behaviour ----

/// A value from inside a loop reaches a join after it only through the
/// loop's exit.
#[test]
fn test_a_join_after_a_loop_takes_no_provider_from_inside_it() {
    let body = "define i16 @f(i1 %c, i1 %d) {
b0:
  br i1 %d, label %b1, label %b2

b1:
  %y = load i16, ptr CELL
  br i1 %c, label %b1, label %b3

b2:
  store i16 20, ptr CELL
  br label %b3

b3:
  %x = load i16, ptr CELL
  ret i16 %x
}
";
    for insert in [false, true] {
        unchanged(body, insert);
    }
}

/// Loads at two joins: the second's provider is the first, gone to its phi.
#[test]
fn test_a_provider_replaced_by_its_own_phi_serves_a_later_join() {
    let body = "define i16 @f(i1 %c, i1 %d) {
b0:
  br i1 %d, label %b6, label %b7

b6:
  br i1 %c, label %b1, label %b2

b1:
  store i16 10, ptr CELL
  br label %b3

b2:
  store i16 20, ptr CELL
  br label %b3

b3:
  %x = load i16, ptr CELL
  br label %b5

b7:
  store i16 30, ptr CELL
  br label %b5

b5:
  %z = load i16, ptr CELL
  ret i16 %z
}
";
    let text = after(body, false, &[&[0, 0], &[0, 1], &[1, 0], &[1, 1]]);
    assert!(text.contains("%x1 = phi i16 [ 10, %b1 ], [ 20, %b2 ]"), "{text}");
    assert!(text.contains("%z1 = phi i16 [ 30, %b7 ], [ %x1, %b3 ]"), "{text}");
}

/// A value of another type does not serve the load.
#[test]
fn test_a_provider_of_another_type_is_not_used() {
    unchanged(&diamond("", "").replace("store i16 10, ptr CELL", "store ptr @g, ptr CELL"), false);
}

/// Alias proves `@h` apart from `@g`, whether a store or a callee writes it.
#[test]
fn test_two_distinct_globals_are_proven_apart() {
    for write in ["store i16 30, ptr @h", "call void @seth()"] {
        let body = format!(
            "@h = global i16 0

define void @seth() {{
b0:
  store i16 30, ptr @h
  ret void
}}

{}",
            diamond("", write)
        );
        let text = after(&body, false, BOTH);
        assert!(text.contains("%x1 = phi i16 [ 10, %b1 ], [ 20, %b2 ]"), "{write}: {text}");
    }
}

/// The pass keeps every corpus module verifying.
#[test]
fn test_every_corpus_module_verifies_after_loadjoins() {
    let mut changed = 0;
    for (name, mut module) in llrm_analysis::testing::corpus() {
        let mut manager = PassManager::default();
        manager.verify_each = true;
        manager.require::<Summaries>();
        manager.add(LoadJoins { insert: true });
        let stages = manager.run(&mut module).unwrap_or_else(|error| panic!("{name}: {error}"));
        changed += stages.len();
    }
    assert!(changed > 0, "loadjoins changed nothing in the corpus");
}

/// A store's reference had no provenance, so a store to another global
/// met the join's cell and the load stayed. Through an unknown pointer it
/// still may.
#[test]
fn a_store_to_another_global_keeps_the_join_value() {
    let other = format!("@b = global i16 0\n\n{}", diamond("store i16 1, ptr @b", ""));
    let text = after(&other, false, BOTH);
    assert!(!text.contains("load"), "{text}");
    unchanged(&diamond("%p = inttoptr i16 64 to ptr\n  store i16 1, ptr %p", ""), false);
}
