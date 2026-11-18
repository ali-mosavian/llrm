//! A loaded boolean global read as a word is `lda g; ldx #0`. The `assertzext`
//! the legalizer puts on a promoted `i1` load hid the load from the byte
//! narrowing, which left the pointer pair and `lda (zp),y` (`huffman_decode`
//! and `memory_expansion` -O1, +9 B each, 2026-11-14).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const IR: &str = "@bad = external global i1
declare void @report(i32)
define void @f() {
  %b = load i1, ptr @bad
  %z = zext i1 %b to i32
  call void @report(i32 %z)
  ret void
}";

#[test]
fn a_boolean_global_is_loaded_from_its_address() {
    let ir = format!("target datalayout = \"{}\"\n{IR}", datalayout());
    let asm = assembly(&ir, &Request::new(Level::O1)).expect("compiles");
    assert!(asm.contains("lda\tbad") || asm.contains("lda bad"), "{asm}");
    assert!(!asm.contains("(rc2),y"), "{asm}");
}
