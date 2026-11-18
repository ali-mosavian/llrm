//! Two functions are the same code when their bodies match; a differing constant, operation or shape separates them.

use llrm_mir::passes::PassManager;
use llrm_mir::program::Program;

use super::{equivalent, function_hash, MergeFunctions, MergeLocalFunctions};
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:32:32-i32:32-i64:32-n8:16:32\"\n\n";

fn same(text: &str, a: &str, b: &str) -> bool {
    let module = parsed(&format!("{HEAD}{text}"));
    let (ia, ib) = (module.named(a).unwrap(), module.named(b).unwrap());
    let (fa, fb) = (module.global(ia).function().unwrap(), module.global(ib).function().unwrap());
    let verdict = equivalent(&module, ia, fa, ib, fb);
    if verdict {
        assert_eq!(function_hash(fa), function_hash(fb), "equivalent functions share a hash");
    }
    verdict
}

const ADD: &str = "define i32 @a(i32 %x) {
b0:
  %r = add i32 %x, 1
  ret i32 %r
}

define i32 @b(i32 %y) {
entry:
  %s = add i32 %y, 1
  ret i32 %s
}

define i32 @c(i32 %y) {
b0:
  %s = add i32 %y, 2
  ret i32 %s
}

define i32 @d(i32 %y) {
b0:
  %s = sub i32 %y, 1
  ret i32 %s
}
";

/// gcc folds `__gtsf2` into `jmp __gesf2` and ours kept 16 B: two bodies that differ in names only are one.
#[test]
fn test_bodies_that_differ_in_names_only_are_equivalent() {
    assert!(same(ADD, "a", "b"));
}

/// A differing constant must not fold: it is another function.
#[test]
fn test_a_differing_constant_is_not_equivalent() {
    assert!(!same(ADD, "a", "c"));
}

#[test]
fn test_a_differing_operation_is_not_equivalent() {
    assert!(!same(ADD, "a", "d"));
}

/// Two directly recursive copies call themselves: the address of each is the other's.
#[test]
fn test_two_recursive_copies_are_equivalent() {
    let text = "define i32 @a(i32 %x) {
b0:
  %r = call i32 @a(i32 %x)
  ret i32 %r
}

define i32 @b(i32 %x) {
b0:
  %r = call i32 @b(i32 %x)
  ret i32 %r
}

define i32 @c(i32 %x) {
b0:
  %r = call i32 @a(i32 %x)
  ret i32 %r
}
";
    assert!(same(text, "a", "b"));
    assert!(!same(text, "b", "c"), "a call to another function is not the function's own address");
}

/// A loop's phi reads a value defined later in the walk: the pairing is checked where it is defined.
#[test]
fn test_loops_with_matching_phis_are_equivalent_and_swapped_ones_are_not() {
    let text = "define i32 @a(i32 %n) {
b0:
  br label %b1
b1:
  %i = phi i32 [ 0, %b0 ], [ %j, %b1 ]
  %j = add i32 %i, 1
  %c = icmp slt i32 %j, %n
  br i1 %c, label %b1, label %b2
b2:
  ret i32 %j
}

define i32 @b(i32 %m) {
b0:
  br label %b1
b1:
  %p = phi i32 [ 0, %b0 ], [ %q, %b1 ]
  %q = add i32 %p, 1
  %d = icmp slt i32 %q, %m
  br i1 %d, label %b1, label %b2
b2:
  ret i32 %q
}

define i32 @c(i32 %m) {
b0:
  br label %b1
b1:
  %p = phi i32 [ 0, %b0 ], [ %q, %b1 ]
  %q = add i32 %p, 1
  %d = icmp slt i32 %q, %m
  br i1 %d, label %b1, label %b2
b2:
  ret i32 %p
}
";
    assert!(same(text, "a", "b"));
    assert!(!same(text, "a", "c"), "returning the phi is not returning its successor");
}

fn merged(text: &str) -> String {
    let mut program =
        Program::new(vec![parsed(&format!("{HEAD}{text}"))], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add_program(MergeFunctions);
    manager.run(&mut program).unwrap();
    printed(&program.modules[0])
}

fn defined(text: &str) -> usize {
    text.lines().filter(|one| one.starts_with("define")).count()
}

/// gcc folded `__gtsf2` into `jmp __gesf2` and ours kept both 16 B bodies: two external copies stay two names,
/// the second a tail call of the first.
#[test]
fn test_an_external_copy_becomes_a_tail_call_of_the_first() {
    let after = merged(
        "define i32 @a(i32 %x) {
b0:
  %t = add i32 %x, 1
  %r = mul i32 %t, 3
  ret i32 %r
}

define i32 @b(i32 %y) {
b0:
  %u = add i32 %y, 1
  %s = mul i32 %u, 3
  ret i32 %s
}
",
    );
    assert_eq!(defined(&after), 2, "an exported name stays defined:\n{after}");
    assert!(after.contains("tail call i32 @a(i32 %0)"), "@b is a thunk of @a:\n{after}");
}

/// A local copy nothing takes the address of goes, and its caller calls the kept one.
#[test]
fn test_a_local_copy_is_dropped_and_its_callers_call_the_kept_one() {
    let after = merged(
        "define internal i32 @a(i32 %x) {
b0:
  %r = add i32 %x, 1
  ret i32 %r
}

define internal i32 @b(i32 %y) {
b0:
  %s = add i32 %y, 1
  ret i32 %s
}

define i32 @main(i32 %z) {
b0:
  %p = call i32 @a(i32 %z)
  %q = call i32 @b(i32 %p)
  ret i32 %q
}
",
    );
    assert_eq!(defined(&after), 2, "one copy and main:\n{after}");
    assert!(!after.contains("@b("), "@b is gone:\n{after}");
}

/// Another constant, another operation: nothing merges.
#[test]
fn test_functions_that_differ_are_left_alone() {
    let after = merged(
        "define i32 @a(i32 %x) {
b0:
  %r = add i32 %x, 1
  ret i32 %r
}

define i32 @c(i32 %y) {
b0:
  %s = add i32 %y, 2
  ret i32 %s
}

define i32 @d(i32 %y) {
b0:
  %s = sub i32 %y, 1
  ret i32 %s
}
",
    );
    assert!(!after.contains("tail call"), "no thunk:\n{after}");
    assert_eq!(defined(&after), 3);
}

/// value_of_valueless grew 15 -> 17 B at m32 -Os: two exported `ret` bodies became a `jmp`, which is longer than the `ret`.
/// A body that is only its terminator stays.
#[test]
fn test_a_bare_return_is_not_replaced_by_a_thunk() {
    let after = merged(
        "define i32 @a(i32 %x) {
b0:
  ret i32 %x
}

define i32 @b(i32 %y) {
b0:
  ret i32 %y
}
",
    );
    assert!(!after.contains("tail call"), "a bare return stays a body:\n{after}");
}

/// A guard that kept every body of two instructions lost the sfm16 merge (-46 B became -14 B): `call` plus `ret` with an argument
/// of its own is pushes, a call and a pop, where the thunk is one `jmp`.
#[test]
fn test_a_call_and_a_return_with_a_constant_argument_is_replaced_by_a_thunk() {
    let after = merged(
        "declare i32 @h(i32, i32)

define i32 @a(i32 %x) {
b0:
  %r = call i32 @h(i32 %x, i32 7)
  ret i32 %r
}

define i32 @b(i32 %y) {
b0:
  %s = call i32 @h(i32 %y, i32 7)
  ret i32 %s
}
",
    );
    assert!(after.contains("tail call i32 @a(i32 %0)"), "@b is a thunk of @a:\n{after}");
}

/// shl_long_by_one -O1 (z80) kept two local copies; the -O1 pass drops them, and writes no thunk for an exported copy
/// (a thunk is a `jmp` more per call).
#[test]
fn test_the_o1_pass_drops_a_local_copy_and_thunks_no_external_one() {
    let run = |text: &str| {
        let mut program =
            Program::new(vec![parsed(&format!("{HEAD}{text}"))], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
        let mut manager = PassManager::default();
        manager.add_program(MergeLocalFunctions);
        manager.run(&mut program).unwrap();
        printed(&program.modules[0])
    };
    let body = |linkage: &str, name: &str| {
        format!("define {linkage} i32 @{name}(i32 %x) {{\nb0:\n  %t = add i32 %x, 1\n  %r = mul i32 %t, 3\n  ret i32 %r\n}}\n\n")
    };
    let local = run(&format!("{}{}", body("internal", "a"), body("internal", "b")));
    assert_eq!(defined(&local), 1, "one local copy stays:\n{local}");
    let external = run(&format!("{}{}", body("", "a"), body("", "b")));
    assert_eq!(defined(&external), 2, "an exported copy stays whole:\n{external}");
    assert!(!external.contains("tail call"), "no thunk at -O1:\n{external}");
}
