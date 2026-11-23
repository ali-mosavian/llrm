//! A static helper of two calls, called from seven sites, was copied into all
//! seven at -Os: the callee's size counted each inner call as one flat
//! instruction, not its `jsr` and pushed arguments. num_fpmath: 364 B against
//! gcc's 210 B for `fmth_fn`'s seven wrappers.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SEVEN_WRAPPERS: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare i8 @pfpu_fn(i8, i32, ptr)
declare i32 @fmth_check(ptr, i8, i32)
define internal i32 @fmth_fn(ptr %f, i8 %fn, i32 %a) {
  %r = alloca i32
  %st = call i8 @pfpu_fn(i8 %fn, i32 %a, ptr %r)
  %v = load i32, ptr %r
  %o = call i32 @fmth_check(ptr %f, i8 %st, i32 %v)
  ret i32 %o
}
define i32 @s1(ptr %f, i32 %a) #0 {
  %r = call i32 @fmth_fn(ptr %f, i8 1, i32 %a)
  ret i32 %r
}
define i32 @s2(ptr %f, i32 %a) #0 {
  %r = call i32 @fmth_fn(ptr %f, i8 2, i32 %a)
  ret i32 %r
}
define i32 @s3(ptr %f, i32 %a) #0 {
  %r = call i32 @fmth_fn(ptr %f, i8 3, i32 %a)
  ret i32 %r
}
define i32 @s4(ptr %f, i32 %a) #0 {
  %r = call i32 @fmth_fn(ptr %f, i8 4, i32 %a)
  ret i32 %r
}
define i32 @s5(ptr %f, i32 %a) #0 {
  %r = call i32 @fmth_fn(ptr %f, i8 5, i32 %a)
  ret i32 %r
}
define i32 @s6(ptr %f, i32 %a) #0 {
  %r = call i32 @fmth_fn(ptr %f, i8 6, i32 %a)
  ret i32 %r
}
define i32 @s7(ptr %f, i32 %a) #0 {
  %r = call i32 @fmth_fn(ptr %f, i8 7, i32 %a)
  ret i32 %r
}
attributes #0 = { optsize }"#;

#[test]
fn a_helper_with_two_calls_stays_a_call_at_os() {
    for cpu in ["68000", "68020"] {
        let out = assembly(SEVEN_WRAPPERS, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let calls = out.lines().filter(|line| line.trim_start().starts_with("jsr\t(fmth_fn")).count();
        assert_eq!(calls, 7, "{cpu}: fmth_fn was copied into its call sites");
    }
}
