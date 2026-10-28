//! Identical constants of a module are one.

use llrm_mir::passes::PassManager;
use llrm_mir::program::Program;

use super::ConstantMerge;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:32:32-i32:32-i64:32-n8:16:32\"\n\n";

fn merged(text: &str) -> String {
    let mut program =
        Program::new(vec![parsed(&format!("{HEAD}{text}"))], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add_program(ConstantMerge);
    manager.run(&mut program).unwrap();
    printed(&program.modules[0])
}

fn globals(text: &str) -> usize {
    text.lines().filter(|one| one.starts_with('@')).count()
}

/// Two copies of one table, each read by its own function, stayed two
/// variables: the pin keeps one and both functions read it.
#[test]
fn test_two_identical_unnamed_addr_tables_become_one() {
    let after = merged(
        "@a = private unnamed_addr constant [2 x i32] [i32 1, i32 2]
@b = private unnamed_addr constant [2 x i32] [i32 1, i32 2]

define i32 @f() {
b0:
  %v = load i32, ptr @a
  ret i32 %v
}

define i32 @g() {
b0:
  %v = load i32, ptr @b
  ret i32 %v
}
",
    );
    assert_eq!(globals(&after), 1, "{after}");
    // Of equals the last is canonical.
    assert_eq!(after.matches("@b").count(), 3, "{after}");
    assert!(!after.contains("@a"), "{after}");
}

/// Neither is `unnamed_addr`: their addresses may be compared, so they stay.
#[test]
fn test_constants_whose_addresses_are_significant_stay_two() {
    let after = merged(
        "@a = internal constant [2 x i32] [i32 1, i32 2]
@b = internal constant [2 x i32] [i32 1, i32 2]

define ptr @f(i1 %c) {
b0:
  %p = select i1 %c, ptr @a, ptr @b
  ret ptr %p
}
",
    );
    assert_eq!(globals(&after), 2, "{after}");
}

/// A variable (not a constant), another initializer, or external linkage on
/// the duplicate: not merged away.
#[test]
fn test_variables_and_different_values_and_external_duplicates_stay() {
    let after = merged(
        "@v1 = private unnamed_addr global [2 x i32] [i32 1, i32 2]
@v2 = private unnamed_addr global [2 x i32] [i32 1, i32 2]
@c1 = private unnamed_addr constant [2 x i32] [i32 1, i32 3]
@c2 = private unnamed_addr constant [2 x i32] [i32 1, i32 4]
@e1 = unnamed_addr constant [2 x i32] [i32 5, i32 6]
@e2 = unnamed_addr constant [2 x i32] [i32 5, i32 6]

define void @f() {
b0:
  %a = load i32, ptr @v1
  %b = load i32, ptr @v2
  %c = load i32, ptr @c1
  %d = load i32, ptr @c2
  %e = load i32, ptr @e1
  %f = load i32, ptr @e2
  ret void
}
",
    );
    assert_eq!(globals(&after), 6, "{after}");
}

/// Merging two tables makes the pointers that name them equal, so the
/// tables of pointers merge on the next round.
#[test]
fn test_merging_makes_the_tables_that_name_the_merged_ones_equal() {
    let after = merged(
        "@a = private unnamed_addr constant [1 x i32] [i32 7]
@b = private unnamed_addr constant [1 x i32] [i32 7]
@pa = private unnamed_addr constant [1 x ptr] [ptr @a]
@pb = private unnamed_addr constant [1 x ptr] [ptr @b]

define ptr @f() {
b0:
  %v = load ptr, ptr @pa
  ret ptr %v
}

define ptr @g() {
b0:
  %v = load ptr, ptr @pb
  ret ptr %v
}
",
    );
    assert_eq!(globals(&after), 2, "{after}");
}

/// The merged variable keeps the larger stated alignment.
#[test]
fn test_the_merged_variable_takes_the_larger_alignment() {
    let after = merged(
        "@a = private unnamed_addr constant [2 x i32] [i32 1, i32 2], align 16
@b = private unnamed_addr constant [2 x i32] [i32 1, i32 2], align 4

define ptr @f(i1 %c) {
b0:
  %p = select i1 %c, ptr @a, ptr @b
  ret ptr %p
}
",
    );
    assert_eq!(globals(&after), 1, "{after}");
    assert!(after.contains("align 16"), "{after}");
}
