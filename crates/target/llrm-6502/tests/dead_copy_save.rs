//! A save of `a` around a copy nothing reads leaves no `pha`/`pla`.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `k_memmove` copied the source pair to two cells that nothing read while `d`
/// waited in `a` (`pha; lda rc4; sta rc7; lda rc5; sta rc16; pla`, 12 B). The
/// scan for dead stores stopped at the `pha`, so the copy stayed, and the
/// bracket with it; the extra bytes pushed a branch out of range (+21 B).
#[test]
fn a_dead_copy_in_a_save_of_a_leaves_no_push() {
    let ir = format!(
        "target datalayout = \"{}\"
define ptr @mm(ptr %d, ptr %s, i16 %n) {{
entry:
  %lt = icmp ult ptr %d, %s
  br i1 %lt, label %fwd, label %back
fwd:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %floop
floop:
  %dp = phi ptr [ %d, %fwd ], [ %dp1, %floop ]
  %sp = phi ptr [ %s, %fwd ], [ %sp1, %floop ]
  %k = phi i16 [ %n, %fwd ], [ %k1, %floop ]
  %k1 = add i16 %k, -1
  %c = load i8, ptr %sp
  store i8 %c, ptr %dp
  %dp1 = getelementptr inbounds i8, ptr %dp, i16 1
  %sp1 = getelementptr inbounds i8, ptr %sp, i16 1
  %e = icmp eq i16 %k1, 0
  br i1 %e, label %done, label %floop
back:
  %bz = icmp eq i16 %n, 0
  br i1 %bz, label %done, label %bpre
bpre:
  %de = getelementptr inbounds i8, ptr %d, i16 %n
  %se = getelementptr inbounds i8, ptr %s, i16 %n
  br label %bloop
bloop:
  %bd = phi ptr [ %de, %bpre ], [ %bd1, %bloop ]
  %bs = phi ptr [ %se, %bpre ], [ %bs1, %bloop ]
  %bk = phi i16 [ %n, %bpre ], [ %bk1, %bloop ]
  %bk1 = add i16 %bk, -1
  %bd1 = getelementptr inbounds i8, ptr %bd, i16 -1
  %bs1 = getelementptr inbounds i8, ptr %bs, i16 -1
  %bc = load i8, ptr %bs1
  store i8 %bc, ptr %bd1
  %be = icmp eq i16 %bk1, 0
  br i1 %be, label %done, label %bloop
done:
  ret ptr %d
}}",
        datalayout()
    );
    let asm = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body = asm.split(".segment").next().unwrap();
    assert!(!body.contains("pha") && !body.contains("pla"), "{body}");
}
