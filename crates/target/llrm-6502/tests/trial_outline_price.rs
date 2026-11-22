//! The unroll trial prices the finished text, outliner included.
//! Stride 2, twelve trips: a unit-stride loop keeps its table index in `x` or `y` (`lda table,x`) and is
//! smaller rolled, rightly put back; a stride-2 loop is not (2026-11-22, the index-IV stack: seven unit-stride
//! trips went from kept to +13 B put back).
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

/// `for (i = 0; i < 12; i++) report(table[2 * i])`: twelve calls of the same shape.
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
  %done = icmp eq i16 %next, 12
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
