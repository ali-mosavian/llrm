//! A select's true arm is copied where it is taken, not staged ahead of the
//! compare (k_strrchr -O2: `ldy rc2; lda rc3; sta rc6` with `pha`/`pla` around
//! them every trip, 603 clocks for 371).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

#[test]
fn a_select_of_a_live_pointer_is_not_staged_through_registers_every_trip() {
    let ir = format!(
        "target datalayout = \"{}\"\n\
         define ptr @last(ptr %s, i16 %c) {{\nentry:\n  %c8 = trunc i16 %c to i8\n  br label %l\n\
         l:\n  %p = phi ptr [%s, %entry], [%n, %l]\n  %last = phi ptr [null, %entry], [%sel, %l]\n\
         \x20 %ch = load i8, ptr %p\n  %eq = icmp eq i8 %ch, %c8\n  %sel = select i1 %eq, ptr %p, ptr %last\n\
         \x20 %n = getelementptr i8, ptr %p, i16 1\n  %z = icmp eq i8 %ch, 0\n  br i1 %z, label %out, label %l\n\
         out:\n  ret ptr %sel\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("last:").unwrap();
    let body = text[from..].split(".segment").next().unwrap();
    // what runs before the loop's branch on the compare: the compare alone
    let before_branch = body.split("bne").nth(0).unwrap();
    assert!(!before_branch.contains("pha"), "{body}");
}
