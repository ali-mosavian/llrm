//! A function that fills with a run-time length kept the `memset` call, and
//! its 39 B of library with it when no other function calls it: k_strncpy was
//! 128 B against 102 B for cc65's hand code (2026-11-14). The fill is the loop
//! when the loop is the smaller image.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const FILL: &str = "declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define ptr @fill(ptr %d, i16 %n) {
  call void @llvm.memset.p0.i16(ptr %d, i8 0, i16 %n, i1 false)
  ret ptr %d
}
";

#[test]
fn a_fill_that_is_the_only_caller_of_memset_is_a_loop() {
    let ir = format!("target datalayout = \"{}\"\n{FILL}", datalayout());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("memset"), "the call and its library are gone:\n{text}");
}
