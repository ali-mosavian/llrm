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
