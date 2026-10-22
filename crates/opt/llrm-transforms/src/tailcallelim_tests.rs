//! Each body is MIR text run by llrm-mir's interpreter before and after, so a
//! wrong loop shows as a wrong answer; what is asserted of the text is whether
//! the call is gone, or marked `tail`.

use llrm_analysis::testing::DOS;

use super::TailCallElim;
use crate::testing::{managed, parsed, results};

/// `text` through the pass: its printed form, run as before on `inputs`.
fn eliminated(
    text: &str,
    inputs: &[&[i128]],
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, TailCallElim);
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

/// `ret f(b, a % b)` was a call per step; it is the loop of Euclid.
#[test]
fn a_call_returned_as_it_is_becomes_a_loop() {
    let after = eliminated(GCD, PAIRS);
    assert_eq!(calls(&after), 0, "{after}");
    assert!(after.contains("tailrecurse"), "{after}");
    assert!(!after.contains("ret.tr"), "the return value carried by nobody is not carried: {after}");
}

/// `n + f(n - 1)` is a call whose result only feeds an `add`: an accumulator,
/// and no call.
#[test]
fn a_call_whose_result_only_feeds_an_add_becomes_a_loop_with_an_accumulator() {
    let after = eliminated(SUM, &[&[0], &[1], &[5], &[30]]);
    assert_eq!(calls(&after), 0, "{after}");
    assert!(after.contains("accumulator.tr"), "{after}");
}

/// A recursion the result of which is multiplied is an accumulator that starts
/// at one.
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

/// bench/hanoi after promotion: two calls, the first's result added to 1 and
/// then to the second's, and the frontend's join block of one phi. The second
/// call is the tail; the first stays.
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

/// A void recursion whose call ends one arm of a branch (bench/bintree's
/// `insert`) is a loop.
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
    let after = managed(&mut module, TailCallElim);
    let inputs: &[&[i128]] = &[&[2, 9], &[1, 9], &[0, 9]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    assert!(!after.contains("call void @g(i16 %next"), "{after}");
}

/// A subtraction (not associative), a result used twice and work with an effect
/// after the call each keep their call.
#[test]
fn what_cannot_be_a_loop_keeps_its_call() {
    let subtracted = SUM.replace("%s = add i16 %n, %v", "%s = sub i16 %n, %v");
    assert_eq!(calls(&eliminated(&subtracted, &[&[0], &[3]])), 1);
    let twice = SUM.replace("ret i16 %s", "%t = add i16 %s, %v\n  ret i16 %t");
    assert_eq!(calls(&eliminated(&twice, &[&[0], &[3]])), 1);
    let stored = format!(
        "@g = global i16 0\n\n{}",
        SUM.replace("%s = add i16 %n, %v", "store i16 %n, ptr @g\n  %s = add i16 %n, %v")
    );
    assert_eq!(calls(&eliminated(&stored, &[&[0], &[3]])), 1);
}

/// A frame object the callee may read is not reused by the next trip: the call
/// that is passed it is not a tail call.
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
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailCallElim);
    assert_eq!(calls(&after), 1, "{after}");
    assert!(!after.contains("tail call"), "{after}");
}

/// `byval` is the caller's copy in memory: the loop copies the argument into a
/// temporary and from it into the parameter (the pin's
/// `copyByValueOperandIntoLocalTemp`), so the recursion is a loop.
#[test]
fn a_byval_argument_is_copied_through_a_temporary() {
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
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailCallElim);
    assert!(!after.contains("call void @f("), "{after}");
    assert_eq!(after.matches("call void @llvm.memcpy").count(), 2, "{after}");
}

/// A function that calls `setjmp` returns into one frame a second time: no call
/// is marked, none is a loop.
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
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailCallElim);
    assert_eq!(calls(&after), 1, "{after}");
}

/// An argument every call passes on as it got it needs no phi: the pin makes
/// one per parameter and `cleanupAndFinalize` removes the ones
/// `simplifyInstruction` finds to be the parameter.
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

/// bench/fib: `f(n - 1) + f(n - 2)`, both operands of the sum calls. The second
/// is the tail and the first is what it adds to.
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

/// `f(n - 1) << 1` with the base case 1 is a chain of shifts by a constant: the
/// pin's unary accumulator, `2^n`, that starts at the base case and shifts once
/// per trip.
#[test]
fn a_shift_by_a_constant_accumulates_from_the_base_case() {
    let text = "define i16 @f(i16 %n) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b1, label %b2

b1:
  ret i16 1

b2:
  %m = sub i16 %n, 1
  %v = call i16 @f(i16 %m)
  %s = shl i16 %v, 1
  ret i16 %s
}
";
    let after = eliminated(text, &[&[0], &[1], &[5], &[12]]);
    assert_eq!(calls(&after), 0, "{after}");
}

/// A recursion followed by a return of another value: the value is the
/// function's whenever a trip got there, which `ret.tr` and `ret.known.tr`
/// carry.
#[test]
fn a_value_returned_after_the_call_is_carried_by_the_return_phis() {
    let text = "define i16 @f(i16 %n) {
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %b1, label %b2

b1:
  ret i16 1

b2:
  %m = sub i16 %n, 1
  %v = call i16 @f(i16 %m)
  ret i16 5
}
";
    let after = eliminated(text, &[&[0], &[1], &[4]]);
    assert_eq!(calls(&after), 0, "{after}");
    assert!(after.contains("ret.known.tr"), "{after}");
}

/// A call to a function that reads no memory, of constants and parameters, is a
/// tail call whether or not an alloca has escaped; the pin's
/// `tailcall-readnone` case.
#[test]
fn a_readnone_call_of_parameters_is_marked_tail() {
    let text = "declare i16 @pure(i16) memory(none)

define i16 @h(i16 %x) {
b0:
  %cell = alloca i16
  store ptr %cell, ptr @sink
  %v = call i16 @pure(i16 %x)
  ret i16 %v
}

@sink = global ptr null
";
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailCallElim);
    assert!(after.contains("tail call i16 @pure("), "{after}");
}

/// A call before an alloca's address escapes is a tail call; one after is not
/// (`markTails`' escaped state).
#[test]
fn a_call_after_the_frame_escapes_is_not_marked_tail() {
    let text = "declare void @use(ptr)
declare i16 @g(i16)

@sink = global ptr null

define i16 @h(i16 %x) {
b0:
  %cell = alloca i16
  %before = call i16 @g(i16 %x)
  store ptr %cell, ptr @sink
  %after = call i16 @g(i16 %before)
  ret i16 %after
}
";
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailCallElim);
    assert!(after.contains("tail call i16 @g(i16 %x)"), "{after}");
    assert!(after.contains("%after = call i16 @g("), "{after}");
}

/// `disable-tail-calls`="true" leaves the function as it is.
#[test]
fn the_disable_tail_calls_attribute_leaves_the_function_alone() {
    let text = GCD
        .replace("define i16 @f(i16 %a, i16 %b) {", "define i16 @f(i16 %a, i16 %b) \"disable-tail-calls\"=\"true\" {");
    let after = managed(&mut parsed(&format!("{DOS}{text}")), TailCallElim);
    assert_eq!(calls(&after), 1, "{after}");
}

/// A function of `n` blocks that each return a call to the function.
fn arms(n: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %n) {\nentry:\n  br label %c0\n");
    for k in 0..n {
        text += &format!(
            "\nc{k}:\n  %t{k} = icmp eq i16 %n, {k}\n  br i1 %t{k}, label %r{k}, label %c{}\n\nr{k}:\n  %m{k} = sub i16 %n, 1\n  %v{k} = call i16 @f(i16 %m{k})\n  ret i16 %v{k}\n",
            k + 1
        );
    }
    text + &format!("\nc{n}:\n  ret i16 0\n}}\n")
}

/// The pass is linear in the sites: four times the arms cost about four times,
/// not sixteen (a slope test of the work at 1x and 4x, the least of three runs
/// of each).
#[test]
fn the_time_grows_with_the_arms_not_with_their_square() {
    let least = |n: usize| {
        let text = arms(n);
        (0..3)
            .map(|_| {
                let mut module = parsed(&format!("{DOS}{text}"));
                llrm_support::debug::work_of(|| managed(&mut module, TailCallElim)).1 as f64
            })
            .fold(f64::MAX, f64::min)
    };
    let (small, large) = (least(150), least(600));
    assert!(large < small * 6.0, "150 arms {small:.0} work, 600 arms {large:.0}: a ratio of {:.1}", large / small);
}

/// A function marked `DEFERRED` keeps its call: D10 copies a tree recursion
/// into itself first. gcd was looped before the copies were made.
#[test]
fn a_function_whose_elimination_is_deferred_keeps_its_call() {
    let mut module = parsed(&format!("{DOS}{GCD}"));
    module.function_mut("f").expect("@f").1.attrs.push(llrm_mir::opcode::Attribute::Flag(super::DEFERRED.to_owned()));
    let after = managed(&mut module, TailCallElim);
    assert_eq!(calls(&after), 1, "{after}");
    assert!(!after.contains("tailrecurse"), "{after}");
}
