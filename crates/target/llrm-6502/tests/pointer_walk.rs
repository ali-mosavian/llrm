//! A pointer stepped each trip is addressed as the stepped value.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// `stpcpy`: both pointers step before the load and the store read them.
const STPCPY: &str = "define ptr @f(ptr %dst, ptr %src) {
entry:
  %c = load i8, ptr %src
  store i8 %c, ptr %dst
  %z = icmp eq i8 %c, 0
  br i1 %z, label %out, label %loop
loop:
  %s = phi ptr [ %s1, %loop ], [ %src, %entry ]
  %d = phi ptr [ %d1, %loop ], [ %dst, %entry ]
  %s1 = getelementptr inbounds i8, ptr %s, i16 1
  %d1 = getelementptr inbounds i8, ptr %d, i16 1
  %v = load i8, ptr %s1
  store i8 %v, ptr %d1
  %e = icmp eq i8 %v, 0
  br i1 %e, label %out, label %loop
out:
  %r = phi ptr [ %dst, %entry ], [ %d1, %loop ]
  ret ptr %r
}";

/// `stpcpy` copied the destination pointer each trip (`lda rc2; sta rc6; lda
/// rc3; sta rc7`) to store at offset 1 through the old value after the step:
/// 44 bytes at -O2 against 33 with the store through the stepped pointer (35
/// with the destination arriving in A:X and stored to its cells).
#[test]
fn a_stepped_pointer_is_the_address_of_the_store_that_follows() {
    let ir = format!("target datalayout = \"{}\"\n{STPCPY}\n", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("f:").unwrap();
    let body: String = text[from..].split("  rts").next().unwrap().to_owned() + "  rts\n";
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 35, "{bytes} B for stpcpy:\n{body}");
}
