//! Adapted from llrm-core's `optimize/transform_tests.rs` (`d_tests`), each
//! body now MIR text run by llrm-mir's interpreter.

use llrm_mir::module::Module;

use super::dead;
use crate::testing::{parsed, printed, results};

/// @f's dead instructions removed; whether any went.
fn deadened(module: &mut Module) -> bool {
    let callees = llrm_mir::memory::callees(module);
    let (context, function) = module.function_mut("f").expect("@f");
    dead(context, &callees, function)
}

/// `text` after Dead is `expected`, and computes what it did on `inputs`.
fn check(text: &str, expected: &str, inputs: &[&[i128]]) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(deadened(&mut module), text != expected);
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// A computation nothing reads goes; the one returned stays.
#[test]
fn test_dead_code_goes_and_the_bytes_are_still_accounted_for() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %doomed = mul i16 %x, 3
  %kept = add i16 %x, 1
  ret i16 %kept
}
",
        "define i16 @f(i16 %x) {
b0:
  %kept = add i16 %x, 1
  ret i16 %kept
}
",
        &[&[0], &[7], &[-1]],
    );
}

/// Dead work reading dead work goes whole, in one run.
#[test]
fn test_a_chain_only_dead_work_reads_goes_at_once() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %a = mul i16 %x, 3
  %b = xor i16 %a, 5
  %c = zext i16 %b to i32
  ret i16 %x
}
",
        "define i16 @f(i16 %x) {
b0:
  ret i16 %x
}
",
        &[&[2], &[-3]],
    );
}

/// A sum the loop carries for no one goes, its phi with it, in one run;
/// the counter the exit reads stays.
#[test]
fn test_a_loop_carried_cycle_nothing_reads_goes() {
    check(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %sum = phi i16 [ 0, %b0 ], [ %more, %b1 ]
  %more = add i16 %sum, %i
  %next = add i16 %i, 1
  %c = icmp slt i16 %next, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %next
}
",
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %c = icmp slt i16 %next, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %next
}
",
        &[&[0], &[1], &[5]],
    );
}

/// A call that touches nothing and returns, and a plain load, go unread;
/// a call with effects, a volatile load and a store stay.
#[test]
fn test_only_work_without_an_effect_goes() {
    let mut module = parsed(
        "declare i16 @pure(i16) memory(none) willreturn
declare i16 @effect(i16)

define i16 @f(i16 %x, ptr %p) {
b0:
  %a = call i16 @pure(i16 %x)
  %b = call i16 @effect(i16 %x)
  %v = load volatile i16, ptr %p
  %w = load i16, ptr %p
  store i16 %x, ptr %p
  ret i16 %x
}
",
    );
    assert!(deadened(&mut module));
    assert_eq!(
        printed(&module),
        "declare i16 @pure(i16) memory(none) willreturn

declare i16 @effect(i16)

define i16 @f(i16 %x, ptr %p) {
b0:
  %b = call i16 @effect(i16 %x)
  %v = load volatile i16, ptr %p
  store i16 %x, ptr %p
  ret i16 %x
}
"
    );
}

/// Everything read, nothing goes.
#[test]
fn test_a_body_with_nothing_dead_is_unchanged() {
    let text = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  %r = phi i16 [ %x, %b0 ], [ %y, %b1 ]
  ret i16 %r
}
";
    check(text, text, &[&[3, 0], &[3, 1]]);
}

/// Dead read a defined callee's effects off its attributes, which only
/// llrm-mir's function-attrs stated: an unused call to a body that does
/// nothing stayed. The whole-module stamp states them.
#[test]
fn an_unused_call_to_a_stamped_body_that_does_nothing_goes() {
    let text = "define internal i16 @twice(i16 %x) {
b0:
  %y = add i16 %x, %x
  ret i16 %y
}

define i16 @f(i16 %x) {
b0:
  %r = call i16 @twice(i16 %x)
  ret i16 %x
}
";
    let calls = |module: &Module| printed(module).matches("call i16 @twice").count();
    let mut stamped = parsed(text);
    crate::testing::stamped(&mut stamped).unwrap();
    let before = results(&stamped, &[&[3]]);
    assert!(deadened(&mut stamped));
    assert_eq!(calls(&stamped), 0);
    assert_eq!(results(&stamped, &[&[3]]), before);
    let mut bare = parsed(text);
    deadened(&mut bare);
    assert_eq!(calls(&bare), 1);
}

/// A local only its lifetime markers name is not a local: with its markers it goes. They
/// kept it, and its markers, in the body.
#[test]
fn test_a_local_only_its_lifetime_markers_name_goes_with_them() {
    let mut module = parsed(
        "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define i16 @f(i16 %x) {
b0:
  %s = alloca i16
  call void @llvm.lifetime.start.p0(i64 2, ptr %s)
  call void @llvm.lifetime.end.p0(i64 2, ptr %s)
  ret i16 %x
}
",
    );
    assert!(deadened(&mut module));
    deadened(&mut module);
    let text = printed(&module);
    assert!(!text.contains("alloca") && !text.contains("call void @llvm.lifetime"), "{text}");
}

/// A local something reads keeps its markers, which a frame layout reads.
#[test]
fn test_a_local_something_reads_keeps_its_lifetime_markers() {
    let mut module = parsed(
        "declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define i16 @f(i16 %x) {
b0:
  %s = alloca i16
  call void @llvm.lifetime.start.p0(i64 2, ptr %s)
  store volatile i16 %x, ptr %s
  %v = load volatile i16, ptr %s
  call void @llvm.lifetime.end.p0(i64 2, ptr %s)
  ret i16 %v
}
",
    );
    assert!(!deadened(&mut module));
}
