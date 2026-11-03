//! An `i64` argument or result is two registers or stack words, the high one
//! first (big-endian, as the pin and gcc pass it). A call of a function that
//! adds two of them must get the sum back.

use llrm_m68k::compile::{Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

const SOURCE: &str = "
define i64 @add(i64 %x, i64 %y) {
  %r = add i64 %x, %y
  ret i64 %r
}
define i32 @f() {
  %r = call i64 @add(i64 4294967295, i64 3)
  %hi = lshr i64 %r, 32
  %h = trunc i64 %hi to i32
  %l = trunc i64 %r to i32
  %h8 = shl i32 %h, 4
  %o = or i32 %h8, %l
  ret i32 %o
}";

#[test]
fn an_i64_sum_returns_high_then_low_under_qemu() {
    let Some(qemu) = support::qemu() else { return };
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{SOURCE}")).expect("parses");
    let mut all = start("f", &[]);
    for name in ["f", "add"] {
        all.extend(items_with(&module, name, Cpu::M68000, Options { mid_end: false, ..Options::default() }).unwrap());
    }
    let path = support::unique_path("i64abi");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &["-cpu", "m68000"], &path);
    std::fs::remove_file(&path).ok();
    // 0xffffffff + 2 = 0x1_0000_0001: high 1, low 1 -> (1 << 4) | 1
    assert_eq!(code, Some(0x12));
}
