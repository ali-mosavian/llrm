//! A counted loop that stores a value of its counter that fits a byte steps a
//! byte, not a word: llvm-mos's `GenerateZExts` offers the register narrowed to
//! the bytes its range needs, and the cost of a step weighs a word as two
//! adds. The word step was `lda rc3; adc #0; sta rc3` on every trip for a high
//! byte that is always zero (`i * 5 + 2` for 48 trips: 237 at most).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn fill(trips: u32) -> String {
    format!(
        "target datalayout = \"{}\"
@t = global [{trips} x i16] zeroinitializer
define void @fill() {{
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %n, %loop ]
  %m = mul nuw nsw i16 %i, 5
  %v = add nuw nsw i16 %m, 2
  %p = getelementptr inbounds [{trips} x i16], ptr @t, i16 0, i16 %i
  store i16 %v, ptr %p
  %n = add nuw nsw i16 %i, 1
  %c = icmp eq i16 %n, {trips}
  br i1 %c, label %done, label %loop
done:
  ret void
}}
",
        datalayout()
    )
}

/// The value `i * 5 + 2` of 48 trips is at most 237: no step of it adds a
/// carry into a high byte.
#[test]
fn a_counter_value_that_fits_a_byte_is_stepped_as_one() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(&fill(48), &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let body = &text[text.find("fill:").expect("fill")..];
        assert!(!body.contains("adc #0"), "{level:?}: a word step of a byte value:\n{body}");
    }
}

/// `i * 5 + 2` of 100 trips reaches 497: it needs the word, and keeps it.
#[test]
fn a_counter_value_that_needs_a_word_keeps_it() {
    let text = assembly(&fill(100), &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    let body = &text[text.find("fill:").expect("fill")..];
    assert!(body.contains("adc #0"), "the word step is gone:\n{body}");
}

/// An insertion-sort nest over a local array: the pointer base `a + 2 * gap`
/// and a byte index both reach one address use, so the formula sums a word
/// pointer and a byte register. The sum was done in the byte type
/// (`Formula::ty` took the first register's) and panicked in debug with
/// "SCEVAddExpr operand types don't match!".
#[test]
fn a_byte_index_beside_a_word_pointer_is_summed_as_a_word() {
    let ir = format!(
        "target datalayout = \"{}\"
define i16 @sort() {{
entry:
  %a = alloca [64 x i16], align 1
  br label %gaps
gaps:
  %gap = phi i16 [ %half, %gapend ], [ 32, %entry ]
  %start = trunc nuw nsw i16 %gap to i8
  %twice = shl nuw nsw i16 %gap, 1
  %row = getelementptr i8, ptr %a, i16 %twice
  br label %outer
outer:
  %o = phi i8 [ %o.next, %place ], [ 0, %gaps ]
  %i = phi i8 [ %start, %gaps ], [ %i.next, %place ]
  %iw = zext nneg i8 %i to i16
  %ow = zext i8 %o to i16
  %p = getelementptr i8, ptr %row, i16 %ow
  %value = load i16, ptr %p, align 2
  br label %inner
inner:
  %at = phi i16 [ %prev, %shift ], [ %iw, %outer ]
  %prev = sub nuw i16 %at, %gap
  %q = getelementptr inbounds nuw [2 x i8], ptr %a, i16 %prev
  %pv = load i16, ptr %q, align 2
  %gt = icmp ugt i16 %pv, %value
  br i1 %gt, label %shift, label %place
shift:
  %r = getelementptr inbounds nuw [2 x i8], ptr %a, i16 %at
  store i16 %pv, ptr %r, align 2
  %below = icmp ult i16 %prev, %gap
  br i1 %below, label %place, label %inner
place:
  %where = phi i16 [ %prev, %shift ], [ %at, %inner ]
  %s = getelementptr inbounds nuw [2 x i8], ptr %a, i16 %where
  store i16 %value, ptr %s, align 2
  %i.next = add nuw nsw i8 %i, 1
  %done = icmp eq i8 %i.next, 64
  %o.next = add nuw nsw i8 %o, 2
  br i1 %done, label %gapend, label %outer
gapend:
  %half = lshr i16 %gap, 1
  %zero = icmp eq i16 %half, 0
  br i1 %zero, label %exit, label %gaps
exit:
  %e = getelementptr inbounds nuw i8, ptr %a, i16 6
  %v = load i16, ptr %e, align 2
  ret i16 %v
}}
",
        datalayout()
    );
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{level:?}: {e}"));
        assert!(text.contains("sort:"), "{level:?}:\n{text}");
    }
}

/// A word counter compared with a byte-ranged call result: the compare's
/// scaled formula holds a narrowed register, and its scale factor was a word
/// constant, so the product panicked in debug with "SCEVMulExpr operand types
/// don't match!" (the factor is now a constant of each register's own width).
#[test]
fn a_word_counter_compared_with_a_byte_ranged_value_scales_each_in_its_width() {
    let ir = format!(
        "target datalayout = \"{}\"
declare range(i16 0, 256) i16 @next()
define i32 @loop() {{
entry:
  %n = tail call i16 @next()
  br label %body
body:
  %i = phi i16 [ %i.next, %body ], [ 0, %entry ]
  %v = load volatile i16, ptr null, align 1
  %i.next = add i16 %i, 1
  %c = icmp eq i16 %i, %n
  br i1 %c, label %exit, label %body
exit:
  ret i32 0
}}
",
        datalayout()
    );
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{level:?}: {e}"));
        assert!(text.contains("loop:"), "{level:?}:\n{text}");
    }
}
