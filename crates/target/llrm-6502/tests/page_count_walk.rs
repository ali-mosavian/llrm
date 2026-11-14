//! `memcmp`'s counted walk stepped a 16-bit counter beside its index
//! (`lda; bne; dec; dec; lda; ora; beq` a trip: 61 B, priced 739 clocks)
//! where ending on the index and a page count is 64 B and 379 (2026-11-14).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// The count is the fourth argument, in `rc6:rc7` (a count arriving in A:X is
/// priced 2 B higher in the counted form, which the page walk then beats at
/// -O2).
const MEMCMP: &str = "define i16 @k_memcmp(ptr %a, ptr %b, i16 %unused, i16 %n) {
entry:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %head
head:
  %k = phi i16 [ %k2, %latch ], [ %n, %entry ]
  %q = phi ptr [ %q2, %latch ], [ %b, %entry ]
  %p = phi ptr [ %p2, %latch ], [ %a, %entry ]
  %x = load i8, ptr %p
  %y = load i8, ptr %q
  %e = icmp eq i8 %x, %y
  br i1 %e, label %latch, label %diff
diff:
  %lt = icmp ult i8 %x, %y
  %v = select i1 %lt, i16 -1, i16 1
  br label %done
latch:
  %k2 = add i16 %k, -1
  %p2 = getelementptr inbounds nuw i8, ptr %p, i16 1
  %q2 = getelementptr inbounds nuw i8, ptr %q, i16 1
  %c = icmp eq i16 %k2, 0
  br i1 %c, label %done, label %head
done:
  %r = phi i16 [ %v, %diff ], [ 0, %entry ], [ 0, %latch ]
  ret i16 %r
}
";

#[test]
fn a_counted_walk_has_no_counter_of_its_own() {
    let ir = format!("target datalayout = \"{}\"\n{MEMCMP}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(body.contains("cpy rc"), "the loop does not test its index against the limit:\n{body}");
    assert_eq!(body.matches("ora rc").count(), 1, "the loop still tests a word counter:\n{body}");
}
