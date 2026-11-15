//! A word with one byte in `x` and one in a cell is tested against zero with
//! `txa; ora cell`, not a branch round the second byte.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `copy` takes its count in `a`:`x`, so the high byte stays in `x` and the low
/// one is a cell. The zero test was `lda rc6; bne cz; txa; cz:` (5 B and a
/// label) where `txa; ora rc6` is 3 B.
#[test]
fn a_count_split_between_x_and_a_cell_is_tested_with_ora() {
    let ir = format!(
        "target datalayout = \"{}\"
define ptr @copy(ptr %d, ptr %s, i16 %n) {{
entry:
  %lt = icmp ult ptr %d, %s
  %z = icmp eq i16 %n, 0
  %go = and i1 %lt, %z
  br i1 %go, label %done, label %loop
loop:
  %dp = phi ptr [ %d, %entry ], [ %dp1, %loop ]
  %sp = phi ptr [ %s, %entry ], [ %sp1, %loop ]
  %k = phi i16 [ %n, %entry ], [ %k1, %loop ]
  %k1 = add i16 %k, -1
  %c = load i8, ptr %sp
  store i8 %c, ptr %dp
  %dp1 = getelementptr inbounds i8, ptr %dp, i16 1
  %sp1 = getelementptr inbounds i8, ptr %sp, i16 1
  %e = icmp eq i16 %k1, 0
  br i1 %e, label %done, label %loop
done:
  ret ptr %d
}}",
        datalayout()
    );
    let asm = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = asm.split(".segment").next().unwrap();
    assert!(!body.contains("_cz"), "{body}");
}
