//! `memcmp` at -O2: the module trial keeps the smaller of the loop as written
//! (64 B) and the counted page walk (68 B, fewer clocks), as everywhere on this
//! target (2026-11-14).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const MEMCMP: &str = "define i16 @k_memcmp(ptr %a, ptr %b, i16 %n) {
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

/// The module trial priced a byte at 30 clocks at -O2 and kept the walk that is
/// 4 bytes larger for fewer clocks: `k_memcmp` 68 B against 64 B (2026-11-14).
/// Bytes come first, clocks break a tie.
#[test]
fn the_walk_the_trial_keeps_is_the_smaller_one() {
    let ir = format!("target datalayout = \"{}\"\n{MEMCMP}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = text.split(".segment").next().unwrap();
    let size = llrm_6502::timing::estimate(llrm_6502::dagisel::hooks::Cpu::Mos6502, body).unwrap().bytes;
    assert!(size <= 64, "{size} B:\n{body}");
}
