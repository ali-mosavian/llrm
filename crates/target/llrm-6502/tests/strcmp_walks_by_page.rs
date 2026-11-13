//! `strcmp` stepped both pointers with `inc p; bne; inc p+1` every trip
//! (k_strcmp: 55 B, 719 clocks against cc65's hand loop at 478): LSR leaves a
//! second `p + 1` beside the pointer's own step, the page walk read it as a
//! use that is no address and kept the loop as written (2026-11-14).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const STRCMP: &str = "define i16 @k_strcmp(ptr %a, ptr %b) {
entry:
  %x = load i8, ptr %a
  %y = load i8, ptr %b
  %e = icmp eq i8 %x, %y
  br i1 %e, label %head, label %diff
head:
  %c = phi i8 [ %x2, %step ], [ %x, %entry ]
  %q = phi ptr [ %q2, %step ], [ %b, %entry ]
  %p = phi ptr [ %p2, %step ], [ %a, %entry ]
  %z = icmp eq i8 %c, 0
  br i1 %z, label %done, label %step
step:
  %p2 = getelementptr inbounds nuw i8, ptr %p, i16 1
  %q2 = getelementptr inbounds nuw i8, ptr %q, i16 1
  %x2 = load i8, ptr %p2
  %y2 = load i8, ptr %q2
  %e2 = icmp eq i8 %x2, %y2
  br i1 %e2, label %head, label %diff
diff:
  %l = phi i8 [ %x, %entry ], [ %x2, %step ]
  %r = phi i8 [ %y, %entry ], [ %y2, %step ]
  %lt = icmp ult i8 %l, %r
  %v = select i1 %lt, i16 -1, i16 1
  br label %done
done:
  %res = phi i16 [ %v, %diff ], [ 0, %head ]
  ret i16 %res
}
";

#[test]
fn a_pointer_walk_with_a_second_next_pointer_steps_one_index() {
    let ir = format!("target datalayout = \"{}\"\n{}", datalayout(), STRCMP);
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(body.contains("iny"), "the loop keeps a pointer step per trip:\n{body}");
}
