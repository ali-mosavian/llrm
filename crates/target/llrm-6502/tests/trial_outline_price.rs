//! The unroll trial prices the finished text, outliner included.
//! Stride 2, ten trips: a loop with a byte table index (`lda table,x`; unit stride, or any stride once
//! MosIndexIv gives it a byte index) is smaller rolled beyond about ten trips, rightly put back (2026-11-22:
//! twelve stride-2 trips went from kept to +11 B put back). At ten the finished text is -5 B, the model +16 B.
//! static_pointers at -Os (seven trips): the unrolled loop was 174 modelled
//! bytes against 181 rolled (kept), and 165 rolled once `lda table,x` replaced
//! the pointer sequence (put back); the finished images are 148 B unrolled
//! against 149 rolled, because the outliner shares the unrolled body's seven
//! `lda #k; jsr report` runs (26 B) and the rolled loop's (16 B). The unroll
//! was put back for 604 clocks against 258.
//!
//! At -O2 and -O3 the outliner gives up the unrolled body's runs where their
//! clocks are a larger share of the module's than their bytes (`[share.speed]
//! outline`): the unroll is then +11 B for 318 clocks and the trial, bytes
//! first, puts it back. At -Os the outliner is the pin's, bytes alone.
//!
//! Fail first: `LLRM_6502_MODEL_BYTES=1 cargo test` prices the functions as
//! before and the loop is put back.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `for (i = 0; i < 10; i++) report(table[2 * i])`: ten calls of the same shape.
const REPORTS: &str = "@table = internal constant [49 x i8] c\"\\0B\\0C\\1E\\06\\28\\0D\\FF\\05\\09\\21\\02\\03\\04\\07\\08\\0A\\0E\\0F\\10\\11\\12\\13\\14\\16\\17\\18\\19\\1A\\1B\\1C\\1D\\1E\\1F\\20\\21\\22\\23\\24\\25\\26\\27\\29\\2A\\2B\\2C\\2D\\2E\\2F\\30\"
declare void @report(i32)
define i16 @main() {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %m = mul i16 %i, 2
  %p = getelementptr [49 x i8], ptr @table, i16 0, i16 %m
  %v = load i8, ptr %p
  %w = sext i8 %v to i32
  call void @report(i32 %w)
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, 10
  br i1 %done, label %exit, label %loop
exit:
  ret i16 0
}";

#[test]
fn an_unroll_the_outliner_makes_smaller_is_kept() {
    let ir = format!("target datalayout = \"{}\"\n{REPORTS}\n", datalayout());
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    let reports = text.lines().filter(|line| line.trim() == "jsr report").count();
    assert!(reports > 1, "{reports} calls of report: the loop was put back:\n{text}");
}
