//! `c >= 'a' && c <= 'z'` was joined into one `(c - 97) u< 26` (an `and` of
//! two booleans priced as one byte operation, under SimplifyCFG's limit of
//! two): the subtract wrote `a`, so `c` was copied through `x` and back every
//! trip (k_strupper 48 B / 442 clocks against llvm-mos's 47 / 412).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const UPPER: &str = "define ptr @k_strupper(ptr %s) {
entry:
  %c0 = load i8, ptr %s
  %e = icmp eq i8 %c0, 0
  br i1 %e, label %done, label %head
head:
  %c = phi i8 [ %c0, %entry ], [ %n, %next ]
  %p = phi ptr [ %s, %entry ], [ %p2, %next ]
  %g = icmp ugt i8 %c, 96
  br i1 %g, label %check, label %next
check:
  %l = icmp ult i8 %c, 123
  br i1 %l, label %upd, label %next
upd:
  %u = add nsw i8 %c, -32
  store i8 %u, ptr %p
  br label %next
next:
  %p2 = getelementptr inbounds nuw i8, ptr %p, i16 1
  %n = load i8, ptr %p2
  %z = icmp eq i8 %n, 0
  br i1 %z, label %done, label %head
done:
  ret ptr %s
}
";

#[test]
fn a_range_test_whose_value_lives_on_is_two_compares() {
    let ir = format!("target datalayout = \"{}\"\n{UPPER}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    assert!(body.contains("cmp #97") && body.contains("cmp #123"), "the range test was joined:\n{body}");
}
