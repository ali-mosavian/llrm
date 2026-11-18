//! DEVIATE UNROLL-SPEED-TRIAL: the pin unrolls a constant-trip loop before it
//! inlines; a callee unrolled that way and then not inlined is a body of
//! stores called three times. It is compiled again with no unroll growth and
//! the smaller image kept. memmove_overlap at -O3 was 681 B with `fill`
//! unrolled to 32 stores, 534 B rolled.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn asm(
    body: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{body}\n", datalayout());
    assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"))
}

/// A static callee filling 32 bytes, called three times.
const FILL_X3: &str = "@buf = global [32 x i8] zeroinitializer
declare void @report(i32)
define internal void @fill() {
entry:
  br label %l
l:
  %i = phi i16 [ 0, %entry ], [ %in, %l ]
  %p = getelementptr [32 x i8], ptr @buf, i16 0, i16 %i
  %v = trunc i16 %i to i8
  %w = add i8 %v, 1
  store i8 %w, ptr %p
  %in = add i16 %i, 1
  %c = icmp ult i16 %in, 32
  br i1 %c, label %l, label %done
done:
  ret void
}
define i16 @main() {
  call void @fill()
  call void @report(i32 1)
  call void @fill()
  call void @report(i32 2)
  call void @fill()
  call void @report(i32 3)
  ret i16 0
}";

/// The shape of write_through_parameter: a static `fill(p, n)` called once with
/// a constant count, and the table summed afterwards. The unroll is the pin's
/// and folds the sum to a constant: 249 B refused against 43 B kept.
const THROUGH_PARAMETER: &str = "@v = internal global [5 x i16] zeroinitializer
define internal void @fill(ptr %p, i16 %n) {
entry:
  br label %l
l:
  %i = phi i16 [ 0, %entry ], [ %in, %l ]
  %q = getelementptr i16, ptr %p, i16 %i
  %t = mul i16 %i, 3
  %v = add i16 %t, 1
  store i16 %v, ptr %q
  %in = add i16 %i, 1
  %c = icmp ult i16 %in, %n
  br i1 %c, label %l, label %done
done:
  ret void
}
define i16 @main() {
  call void @fill(ptr @v, i16 5)
  %a = load i16, ptr getelementptr (i16, ptr @v, i16 2)
  %b = load i16, ptr getelementptr (i16, ptr @v, i16 4)
  %s = add i16 %a, %b
  ret i16 %s
}";

fn stores(text: &str) -> usize {
    text.lines().filter(|line| line.trim_start().starts_with("sta")).count()
}

#[test]
fn a_32_trip_callee_called_three_times_stays_rolled_at_o3() {
    let text = asm(FILL_X3, Level::O3);
    assert!(stores(&text) < 16, "{} stores: fill was unrolled:\n{text}", stores(&text));
}

/// The unrolled and folded image is the smaller one: the trial keeps it.
#[test]
fn a_fill_whose_unroll_folds_stays_unrolled() {
    let text = asm(THROUGH_PARAMETER, Level::O3);
    let branches = text.lines().filter(|line| line.trim_start().starts_with("bcc")).count();
    assert!(branches == 0, "the loop was put back:\n{text}");
}
