//! gcc inlines a static function with one caller wherever the call sits (`-finline-functions-called-once`).
//! The pin refuses where the call's block ends in `unreachable` (`allowSizeGrowth`: Threshold 0, and
//! `updateThreshold` returns before the last-call bonus). QB `repl.c` -Os kept `read_lines` (98 B) and `ready`
//! (94 B), each called once before a noreturn call, as calls.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const ONCE: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
declare void @quit(i32) noreturn
define internal void @read_lines(i32 %x) {
  %a = add i32 %x, 12345
  %b = mul i32 %a, 7
  %c = xor i32 %b, 99999
  %d = shl i32 %c, 3
  call void @report(i32 %d)
  call void @report(i32 %c)
  call void @report(i32 %b)
  ret void
}
define void @enter(i32 %x) optsize {
  call void @read_lines(i32 %x)
  call void @quit(i32 %x)
  unreachable
}";

#[test]
fn a_static_called_once_before_a_noreturn_call_is_inlined_at_os() {
    for cpu in ["68000", "68020"] {
        let out = assembly(ONCE, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let calls = out.lines().filter(|line| line.trim_start().starts_with("jsr\tread_lines") || line.trim_start().starts_with("jsr\t(read_lines")).count();
        assert_eq!(calls, 0, "{cpu}: read_lines stayed a call");
    }
}
