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
