//! `long bench_fib(int n) { return fib(n); }` laid out right before `fib` was
//! `bra fib` and then `fib:`: a 4-byte jump to the next instruction (fib -Os
//! 76 B against gcc's 68, 10 clocks a call).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @wrapper(i32 %x) {
  %r = tail call i32 @target(i32 %x)
  ret i32 %r
}
define i32 @target(i32 %x) noinline {
  %r = mul i32 %x, 3
  ret i32 %r
}
define i32 @first(i32 %x) {
  %r = tail call i32 @target(i32 %x)
  ret i32 %r
}";

#[test]
fn a_tail_jump_to_the_next_function_is_dropped_and_other_jumps_stay() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(IR, cpu_of("68000").unwrap(), level).unwrap_or_else(|why| panic!("{level:?}: {why}"));
        let wrapper = text.split("wrapper:").nth(1).and_then(|rest| rest.split("target:").next()).expect("wrapper");
        assert!(!wrapper.contains("bra") && !wrapper.contains("jmp"), "{level:?}\n{text}");
        let first = text.split("first:").nth(1).expect("first");
        assert!(first.contains("bra\t") || first.contains("jmp\t"), "{level:?}: a jump elsewhere stays\n{text}");
    }
}
