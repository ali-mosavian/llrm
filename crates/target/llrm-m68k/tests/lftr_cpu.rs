//! LFTR rewrote the exit compare of `do { s += i; } while (++i < n)` to a
//! counter and guarded it with `smax(n, 1)`: 8 B for the `cmp`, `bgt` and
//! `moveq` of the guard and a second counter, which the compare it replaced
//! does not repay (sum -O2 30 B and 422 clk on the 68000 against gcc's 18 B and
//! 358; the 68020 36 B against 18). The tests once looked for `cmpi.l #1,`, a
//! spelling the output no longer uses, and passed without testing anything.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SUM: &str = include_str!("../bench/kernels/sum.ll");

#[test]
fn no_cpu_guards_a_counter_exit_with_smax() {
    for cpu in ["68000", "68020", "68040"] {
        let out = assembly(SUM, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(!out.contains("subq.l\t#1, %d"), "{cpu}: the exit counts a second counter down:\n{out}");
        assert!(out.contains("cmp.l"), "{cpu}: the exit is not a compare:\n{out}");
    }
}

/// `for (i = 0; i < n; i++) d[i] = c; return d[n - 1] + d[0];` as clang emits
/// it; `no-builtins` keeps it from becoming `memset`.
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

/// Budget 0 kept `addq`, `cmp.l`, `blt` in the `d[i] = c` loop
/// (memset -O2 over the worse of gcc and clang: +36 B at 68020); a guard of
/// one operation is repaid by a `subq`+`bne` loop.
#[test]
fn every_cpu_counts_a_one_operation_exit_down() {
    for cpu in ["68000", "68020", "68040", "68060"] {
        let out = assembly(FILL, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(out.contains("subq.l\t#1,") && out.contains("bne"), "{cpu}: loop not counted down:\n{out}");
    }
}
