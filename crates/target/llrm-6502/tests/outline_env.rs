//! `LLRM_6502_NO_OUTLINE=1` switches the outliner off for an A/B of one binary
//! through `llrm-cc`, which does not take `-fno-outline`. The 2026-11-23
//! volatile_bitfield run tried `LLRM_6502_OUTLINE_PLAIN=0` for that and still
//! had all 28 `__outl` references: that variable drops only the variant
//! outlined before known constants move. One test in its own process, as the
//! variable is the process's.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

const FOUR_ADDS: &str = "@x = global i32 0
@y = global i32 0
define void @f() {
  %a1 = load volatile i32, ptr @x
  %b1 = load volatile i32, ptr @y
  %s1 = add i32 %a1, %b1
  store volatile i32 %s1, ptr @x
  %a2 = load volatile i32, ptr @x
  %b2 = load volatile i32, ptr @y
  %s2 = add i32 %a2, %b2
  store volatile i32 %s2, ptr @x
  %a3 = load volatile i32, ptr @x
  %b3 = load volatile i32, ptr @y
  %s3 = add i32 %a3, %b3
  store volatile i32 %s3, ptr @x
  %a4 = load volatile i32, ptr @x
  %b4 = load volatile i32, ptr @y
  %s4 = add i32 %a4, %b4
  store volatile i32 %s4, ptr @x
  ret void
}";

#[test]
fn the_no_outline_variable_removes_every_outlined_routine() {
    let ir = format!("target datalayout = \"{}\"\n{FOUR_ADDS}\n", datalayout());
    let on = assembly(&ir, &Request::new(Level::Os)).expect("compiles");
    assert!(on.contains("__outl_"), "no routine to remove:\n{on}");
    // SAFETY: the only test of this process
    unsafe { std::env::set_var("LLRM_6502_NO_OUTLINE", "1") };
    let off = assembly(&ir, &Request::new(Level::Os)).expect("compiles");
    assert!(!off.contains("__outl_"), "LLRM_6502_NO_OUTLINE left a routine:\n{off}");
}
