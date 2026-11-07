//! LFTR rewrote the exit compare of `do { s += i; } while (++i < n)` to a
//! counter and guarded it with `smax(n, 1)`: 8 B for `cmpi`, `bgt` and
//! `moveq` that a 68020's `cmp.l` with a memory limit does not repay (sum -O2
//! 36 B against gcc's 18, arrsum 32 against 22). The 68000 keeps the guard:
//! there the memory compare is 10 clk per trip dearer.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SUM: &str = include_str!("../bench/kernels/sum.ll");

#[test]
fn a_68020_does_not_guard_a_counter_exit_with_smax() {
    let out = assembly(SUM, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("cmpi.l\t#1,"), "68020: the smax guard:\n{out}");
}

#[test]
fn a_68000_keeps_the_pins_counter_exit() {
    let out = assembly(SUM, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert!(out.contains("cmpi.l\t#1,"), "68000: no guard, the pin's exit changed:\n{out}");
}

/// `for (i = 0; i < n; i++) d[i] = c; return d[n - 1] + d[0];` as clang emits it; `no-builtins` keeps it from becoming `memset`.
const FILL: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @k(ptr %d, i32 %c, i32 %n) #0 {
entry:
  %b = trunc i32 %c to i8
  br label %h
h:
  %i = phi i32 [ 0, %entry ], [ %i2, %body ]
  %lt = icmp slt i32 %i, %n
  br i1 %lt, label %body, label %x
body:
  %g = getelementptr inbounds i8, ptr %d, i32 %i
  store i8 %b, ptr %g, align 1
  %i2 = add nsw i32 %i, 1
  br label %h
x:
  %m = sub nsw i32 %n, 1
  %gl = getelementptr inbounds i8, ptr %d, i32 %m
  %l = load i8, ptr %gl, align 1
  %f = load i8, ptr %d, align 1
  %a = sext i8 %l to i32
  %z = sext i8 %f to i32
  %r = add i32 %a, %z
  ret i32 %r
}
attributes #0 = { \"no-builtins\" }";

/// Budget 0 at the 68020+ kept `addq`, `cmp.l`, `blt` in the `d[i] = c` loop
/// (memset -O2 over the worse of gcc and clang: +36 B at 68020); a guard of
/// one operation is repaid by a `subq`+`bne` loop.
#[test]
fn a_68020_counts_a_one_operation_exit_down() {
    for cpu in ["68020", "68040", "68060"] {
        let out = assembly(FILL, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(out.contains("subq.l\t#1,") && out.contains("bne"), "{cpu}: loop not counted down:\n{out}");
    }
}
