use crate::{parse, print, transforms};

/// `text` through the pipeline, printed.
fn optimized(text: &str) -> String {
    let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    transforms::optimized(&mut module).expect("optimizes");
    print::module(&module)
}

/// `text` through `passes` alone, printed.
fn through(passes: &[&str], text: &str) -> String {
    let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    transforms::optimized_with(&mut module, passes).expect("optimizes");
    print::module(&module)
}

/// runtime.nib's `scratch_at` was a far call per digit of every printed
/// number, 40460 instructions to the old path's 36460 in nbody: a small
/// callee's body replaces the call, its two returns a phi.
#[test]
fn test_inline_copies_a_small_callee_into_its_caller() {
    let text = "define internal i16 @magnitude(i16 %v) {
b1:
  %0 = icmp slt i16 %v, 0
  br i1 %0, label %b2, label %b3

b2:
  %1 = sub i16 0, %v
  ret i16 %1

b3:
  ret i16 %v
}

define i16 @f(i16 %n) {
b1:
  %0 = call i16 @magnitude(i16 %n)
  %1 = add i16 %0, 1
  ret i16 %1
}
";
    assert_eq!(
        through(&["inline"], text),
        "define internal i16 @magnitude(i16 %v) {
b1:
  %0 = icmp slt i16 %v, 0
  br i1 %0, label %b2, label %b3

b2:
  %1 = sub i16 0, %v
  ret i16 %1

b3:
  ret i16 %v
}

define i16 @f(i16 %n) {
b1:
  br label %0

0:
  %1 = icmp slt i16 %n, 0
  br i1 %1, label %2, label %4

2:
  %3 = sub i16 0, %n
  br label %5

4:
  br label %5

5:
  %6 = phi i16 [ %3, %2 ], [ %n, %4 ]
  %7 = add i16 %6, 1
  ret i16 %7
}
"
    );
}

/// A call within a cycle of calls stays a call.
#[test]
fn test_inline_keeps_a_recursive_call() {
    let text = "define internal i16 @down(i16 %n) {
b1:
  %0 = icmp eq i16 %n, 0
  br i1 %0, label %b2, label %b3

b2:
  ret i16 0

b3:
  %1 = sub i16 %n, 1
  %2 = call i16 @down(i16 %1)
  ret i16 %2
}
";
    assert_eq!(through(&["inline"], text), through(&[], text));
}

/// A call into a recursion stays a call: bench_agg's call of a recursive
/// `take` was inlined, the copy's own call of `take` inlined again, and
/// the inliner never stopped.
#[test]
fn test_inline_keeps_a_call_into_a_recursion() {
    let text = "define internal i16 @down(i16 %n) {
b1:
  %0 = icmp eq i16 %n, 0
  br i1 %0, label %b2, label %b3

b2:
  ret i16 0

b3:
  %1 = sub i16 %n, 1
  %2 = call i16 @down(i16 %1)
  ret i16 %2
}

define i16 @f(i16 %n) {
b1:
  %0 = call i16 @down(i16 %n)
  ret i16 %0
}
";
    assert_eq!(through(&["inline"], text), through(&[], text));
}

