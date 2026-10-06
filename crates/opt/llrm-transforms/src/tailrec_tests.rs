//! Each body is MIR text run by llrm-mir's interpreter before and after, so a wrong loop shows as a
//! wrong answer; what is asserted of the text is whether the call is gone.

use llrm_analysis::testing::DOS;

use super::TailRecursion;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn eliminated(text: &str, inputs: &[&[i128]]) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, TailRecursion);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

fn calls(text: &str) -> usize {
    text.matches("call i16 @f(").count()
}

const GCD: &str = "define i16 @f(i16 %a, i16 %b) {
b0:
  %z = icmp eq i16 %b, 0
  br i1 %z, label %b1, label %b2

b1:
  ret i16 %a

b2:
  %r = srem i16 %a, %b
  %v = call i16 @f(i16 %b, i16 %r)
  ret i16 %v
}
";

const PAIRS: &[&[i128]] = &[&[12, 18], &[17, 5], &[100, 75], &[7, 7], &[0, 9], &[9, 0]];

/// `ret f(b, a % b)` was a call per step; it is the loop of Euclid.
#[test]
fn a_call_returned_as_it_is_becomes_a_loop() {
    let after = eliminated(GCD, PAIRS);
    assert_eq!(calls(&after), 0, "{after}");
    assert!(after.contains("tailrecurse"), "{after}");
}

const SUM: &str = "define i16 @f(i16 %n) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b1, label %b2

b1:
  ret i16 0

b2:
  %m = sub i16 %n, 1
  %v = call i16 @f(i16 %m)
  %s = add i16 %n, %v
  ret i16 %s
}
";

/// `n + f(n - 1)` is a call whose result only feeds an `add`: an accumulator, and no call.
#[test]
fn a_call_whose_result_only_feeds_an_add_becomes_a_loop_with_an_accumulator() {
    let after = eliminated(SUM, &[&[0], &[1], &[5], &[30]]);
    assert_eq!(calls(&after), 0, "{after}");
}

/// bench/hanoi after promotion: two calls, the first's result added to 1 and then to the second's,
/// and the frontend's join block of one phi. The second call is the tail; the first stays.
#[test]
fn the_last_of_two_calls_summed_is_a_loop_where_the_first_stays() {
    let text = "define i16 @f(i16 %n, i16 %a, i16 %b, i16 %c) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b3, label %b2

b2:
  %m = sub i16 %n, 1
  %x = call i16 @f(i16 %m, i16 %a, i16 %c, i16 %b)
  %y = add i16 %x, 1
  %w = call i16 @f(i16 %m, i16 %c, i16 %b, i16 %a)
  %s = add i16 %y, %w
  br label %b4

b3:
  br label %b4

b4:
  %r = phi i16 [ %s, %b2 ], [ 0, %b3 ]
  ret i16 %r
}
";
    let after = eliminated(text, &[&[0, 1, 3, 2], &[1, 1, 3, 2], &[4, 1, 3, 2], &[7, 1, 3, 2]]);
    assert_eq!(calls(&after), 1, "{after}");
}

/// A recursion the result of which is multiplied is an accumulator that starts at one.
#[test]
fn a_product_accumulates_from_one() {
    let text = "define i16 @f(i16 %n) {
b0:
  %z = icmp ule i16 %n, 1
  br i1 %z, label %b1, label %b2

b1:
  ret i16 1

b2:
  %m = sub i16 %n, 1
  %v = call i16 @f(i16 %m)
  %p = mul i16 %v, %n
  ret i16 %p
}
";
    let after = eliminated(text, &[&[0], &[1], &[5], &[7]]);
    assert_eq!(calls(&after), 0, "{after}");
}

/// A void recursion whose call ends one arm of a branch (bench/bintree's `insert`) is a loop.
#[test]
fn a_void_call_that_ends_an_arm_is_a_loop() {
    let text = "@t = global [8 x i16] [i16 1, i16 2, i16 0, i16 0, i16 0, i16 0, i16 0, i16 0]

define void @g(i16 %at, i16 %v) {
b0:
  %p = getelementptr [8 x i16], ptr @t, i16 0, i16 %at
  %old = load i16, ptr %p
  %z = icmp eq i16 %old, 0
  br i1 %z, label %b1, label %b2

b1:
  store i16 %v, ptr %p
  ret void

b2:
  %next = add i16 %at, 1
  call void @g(i16 %next, i16 %v)
  ret void
}

define i16 @f(i16 %n, i16 %v) {
b0:
  call void @g(i16 0, i16 %v)
  %q = getelementptr [8 x i16], ptr @t, i16 0, i16 %n
  %r = load i16, ptr %q
  ret i16 %r
}
";
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, TailRecursion);
    let inputs: &[&[i128]] = &[&[2, 9], &[1, 9], &[0, 9]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    assert!(!after.contains("call void @g(i16 %next"), "{after}");
}

/// A subtraction (not associative), a result used twice and work with an effect after the call each
/// keep their call.
#[test]
fn what_cannot_be_a_loop_keeps_its_call() {
    let subtracted = SUM.replace("%s = add i16 %n, %v", "%s = sub i16 %n, %v");
    assert_eq!(calls(&eliminated(&subtracted, &[&[0], &[3]])), 1);
    let twice = SUM.replace("ret i16 %s", "%t = add i16 %s, %v\n  ret i16 %t");
    assert_eq!(calls(&eliminated(&twice, &[&[0], &[3]])), 1);
    let stored = format!("@g = global i16 0\n\n{}", SUM.replace("%s = add i16 %n, %v", "store i16 %n, ptr @g\n  %s = add i16 %n, %v"));
    assert_eq!(calls(&eliminated(&stored, &[&[0], &[3]])), 1);
}

/// A frame object the callee may read is not reused by the next trip.
#[test]
fn a_call_that_is_passed_the_frame_keeps_its_call() {
    let text = "define i16 @f(i16 %n, ptr %p) {
b0:
  %cell = alloca i16
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b1, label %b2

b1:
  ret i16 0

b2:
  store i16 %n, ptr %cell
  %m = sub i16 %n, 1
  %v = call i16 @f(i16 %m, ptr %cell)
  ret i16 %v
}
";
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailRecursion);
    assert_eq!(calls(&after), 1, "{after}");
}

/// A `byval` parameter is the caller's copy in memory: the loop would store through the next trip's
/// argument, `@g` itself, where the call stored through a copy. The call stays.
#[test]
fn a_byval_parameter_keeps_its_call() {
    let text = "@g = global i16 0

define void @f(ptr byval(i16) %p, i16 %n) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b1, label %b2

b1:
  ret void

b2:
  store i16 9, ptr %p
  %m = sub i16 %n, 1
  call void @f(ptr byval(i16) @g, i16 %m)
  ret void
}
";
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailRecursion);
    assert!(after.contains("call void @f("), "{after}");
}

/// A function that calls `setjmp` returns into one frame a second time: it keeps one per level.
#[test]
fn a_function_that_calls_a_returns_twice_routine_keeps_its_call() {
    let text = "declare i16 @setjmp(ptr) returns_twice

define i16 @f(i16 %n) {
b0:
  %j = call i16 @setjmp(ptr @buf) returns_twice
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b1, label %b2

b1:
  ret i16 %j

b2:
  %m = sub i16 %n, 1
  %v = call i16 @f(i16 %m)
  ret i16 %v
}

@buf = global [8 x i16] zeroinitializer
";
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailRecursion);
    assert_eq!(calls(&after), 1, "{after}");
}

/// An argument every call passes on as it got it needs no phi: `a` stays the parameter, where the
/// loop passes would first find `phi [%a, entry], [%phi, latch]`.
#[test]
fn an_argument_passed_on_unchanged_gets_no_phi() {
    let text = "define i16 @f(i16 %n, i16 %a) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b1, label %b2

b1:
  ret i16 %a

b2:
  %m = sub i16 %n, 1
  %v = call i16 @f(i16 %m, i16 %a)
  ret i16 %v
}
";
    let after = eliminated(text, &[&[0, 5], &[3, 5]]);
    assert_eq!(after.matches("phi i16").count(), 1, "{after}");
}

/// bench/fib: `f(n - 1) + f(n - 2)`, both operands of the sum calls. The second is the tail and the
/// first is what it adds to: 350256 instructions, gcc's loop 206373.
#[test]
fn of_two_calls_summed_the_later_is_the_loop() {
    let text = "define i16 @f(i16 %n) {
b0:
  %z = icmp slt i16 %n, 2
  br i1 %z, label %b3, label %b2

b2:
  %a = sub i16 %n, 1
  %x = call i16 @f(i16 %a)
  %b = sub i16 %n, 2
  %y = call i16 @f(i16 %b)
  %s = add i16 %x, %y
  br label %b4

b3:
  br label %b4

b4:
  %r = phi i16 [ %s, %b2 ], [ %n, %b3 ]
  ret i16 %r
}
";
    let after = eliminated(text, &[&[0], &[1], &[2], &[7], &[12]]);
    assert_eq!(calls(&after), 1, "{after}");
}
