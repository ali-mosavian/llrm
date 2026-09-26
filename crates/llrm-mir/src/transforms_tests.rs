use crate::{parse, print, transforms};

/// `text` through the pipeline, printed.
fn optimized(text: &str) -> String {
    let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    transforms::optimized(&mut module).expect("optimizes");
    print::module(&module)
}

/// Every Nib local was a stack cell loaded and stored around each use, so
/// the isel path ran 1.98 times the old path's instructions: a loop's
/// counter is a phi, and what it sums another.
#[test]
fn test_mem2reg_makes_a_loop_counter_a_phi() {
    let text = "define i16 @sum(i16 %n) {
b1:
  %i = alloca i16
  %total = alloca i16
  store i16 0, ptr %i
  store i16 0, ptr %total
  br label %b2

b2:
  %0 = load i16, ptr %i
  %1 = icmp slt i16 %0, %n
  br i1 %1, label %b3, label %b4

b3:
  %2 = load i16, ptr %total
  %3 = load i16, ptr %i
  %4 = add i16 %2, %3
  store i16 %4, ptr %total
  %5 = add i16 %3, 1
  store i16 %5, ptr %i
  br label %b2

b4:
  %6 = load i16, ptr %total
  ret i16 %6
}
";
    assert_eq!(
        optimized(text),
        "define i16 @sum(i16 %n) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %3, %b3 ]
  %1 = phi i16 [ 0, %b1 ], [ %4, %b3 ]
  %2 = icmp slt i16 %1, %n
  br i1 %2, label %b3, label %b4

b3:
  %3 = add i16 %0, %1
  %4 = add i16 %1, 1
  br label %b2

b4:
  ret i16 %0
}
"
    );
}

/// An alloca a GEP or a volatile store reaches stays memory.
#[test]
fn test_mem2reg_keeps_what_is_not_only_loaded_and_stored() {
    let text = "define i16 @f() {
b1:
  %a = alloca [2 x i16]
  %v = alloca i16
  store volatile i16 1, ptr %v
  %p = getelementptr i16, ptr %a, i16 1
  store i16 2, ptr %p
  %x = load i16, ptr %p
  %y = load i16, ptr %v
  %z = add i16 %x, %y
  ret i16 %z
}
";
    assert_eq!(optimized(text), print::module(&parse::module(text).unwrap()));
}

/// Nib's frontend spells a condition `icmp`, `sext i1` to i8, `icmp ne 0`,
/// and scales an index by `mul 1`: three instructions where one decides.
#[test]
fn test_instcombine_takes_the_frontends_booleans_and_scales_apart() {
    let text = "define i16 @f(i16 %i, i16 %n) {
b1:
  %0 = icmp ult i16 %i, %n
  %1 = sext i1 %0 to i8
  %2 = icmp ne i8 %1, 0
  br i1 %2, label %b2, label %b3

b2:
  %3 = mul i16 %n, 1
  %4 = mul i16 %i, %3
  %5 = mul i16 %4, 4
  %6 = sub i16 %5, 3
  %7 = add i16 %6, 1
  %8 = add i16 2, 3
  %9 = add i16 %7, %8
  ret i16 %9

b3:
  ret i16 0
}
";
    assert_eq!(
        optimized(text),
        "define i16 @f(i16 %i, i16 %n) {
b1:
  %0 = icmp ult i16 %i, %n
  br i1 %0, label %b2, label %b3

b2:
  %1 = mul i16 %i, %n
  %2 = shl i16 %1, 2
  %3 = add i16 %2, 3
  ret i16 %3

b3:
  ret i16 0
}
"
    );
}

/// A condition folded to a constant leaves a branch that goes one way, a
/// block nothing reaches, and a chain of blocks that only jump: isel then
/// jumped through each (T060's `value`, 71 executed instructions for 65).
#[test]
fn test_simplifycfg_folds_a_constant_branch_and_joins_the_chain() {
    let text = "define i16 @f(i16 %x) {
b1:
  br i1 true, label %b3, label %b2

b2:
  br label %b3

b3:
  %0 = phi i16 [ %x, %b1 ], [ 0, %b2 ]
  br label %b4

b4:
  %1 = add i16 %0, 1
  br label %b5

b5:
  ret i16 %1
}
";
    assert_eq!(
        optimized(text),
        "define i16 @f(i16 %x) {
b1:
  %0 = add i16 %x, 1
  ret i16 %0
}
"
    );
}

/// A block that only jumps is bypassed; the phi after it takes each of its
/// predecessors in its place.
#[test]
fn test_simplifycfg_bypasses_a_forwarding_block() {
    let text = "define i16 @f(i1 %c, i1 %d, i16 %x) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  br i1 %d, label %b4, label %b5

b3:
  br label %b5

b4:
  br label %b5

b5:
  %0 = phi i16 [ 1, %b2 ], [ 2, %b3 ], [ 3, %b4 ]
  ret i16 %0
}
";
    assert_eq!(
        optimized(text),
        "define i16 @f(i1 %c, i1 %d, i16 %x) {
b1:
  br i1 %c, label %b2, label %b5

b2:
  br i1 %d, label %b4, label %b5

b4:
  br label %b5

b5:
  %0 = phi i16 [ 1, %b2 ], [ 2, %b1 ], [ 3, %b4 ]
  ret i16 %0
}
"
    );
}

/// T048 ran two more instructions per iteration of its first loop once
/// simplifycfg bypassed that loop's exit block: the second loop's phi
/// inputs were then made in the first loop's header. A loop's exit into a
/// block with phis stays.
#[test]
fn test_simplifycfg_keeps_a_loop_exit_before_phis() {
    let text = "define i16 @f(i16 %n) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %1, %b3 ]
  %c = icmp slt i16 %0, %n
  br i1 %c, label %b3, label %b4

b3:
  %1 = add i16 %0, 1
  br label %b2

b4:
  br label %b5

b5:
  %2 = phi i16 [ 0, %b4 ], [ %3, %b5 ]
  %3 = add i16 %2, 3
  %d = icmp slt i16 %3, 9
  br i1 %d, label %b5, label %b6

b6:
  ret i16 %3
}
";
    assert!(optimized(text).contains("b4:\n  br label %b5"), "{}", optimized(text));
}

/// nbody ran 201 more instructions once simplifycfg bypassed an inner
/// loop's preheader: its counter's start moved into the outer header. A
/// block leading into a loop's header stays.
#[test]
fn test_simplifycfg_keeps_a_loop_preheader() {
    let text = "define i16 @f(i16 %n, i1 %p) {
b1:
  br i1 %p, label %b5, label %b4

b5:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b5 ], [ %1, %b3 ]
  %1 = add i16 %0, 1
  %c = icmp slt i16 %1, %n
  br i1 %c, label %b3, label %b4

b3:
  br label %b2

b4:
  ret i16 %n
}
";
    assert!(optimized(text).contains("b5:\n  br label %b2"), "{}", optimized(text));
}
