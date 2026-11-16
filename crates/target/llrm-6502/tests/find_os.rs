//! `find` at -Os: the step `i + 1` is made after the exit test, in the latch.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

const FIND: &str = "@buf = external global [16 x i8]
define i8 @find(i8 %0) {
entry:
  br label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %n, %loop ]
  %z = zext i8 %i to i16
  %p = getelementptr inbounds nuw i8, ptr @buf, i16 %z
  %v = load i8, ptr %p, align 1
  %hit = icmp eq i8 %v, %0
  %n = add i8 %i, 1
  br i1 %hit, label %out, label %loop
out:
  ret i8 %i
}
";

/// `i + 1` was made before the test while `i` was read by it: the step ran in
/// y and went back to x through a cell (`sty rc2; ldx rc2`), 19 B against
/// the 13 B of `ldx #0; jmp; inx; cmp buf,x; bne; txa; rts`.
#[test]
fn a_search_loop_at_os_steps_x_in_place() {
    let ir = format!("target datalayout = \"{}\"\n{FIND}", datalayout());
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("find:").unwrap_or_else(|| panic!("no find in\n{text}"));
    let body: String = text[from..].split(".segment").next().unwrap().to_owned();
    let bytes = estimate(Cpu::Mos6502, &body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 13 && body.contains("inx"), "{bytes} B\n{body}");
}
