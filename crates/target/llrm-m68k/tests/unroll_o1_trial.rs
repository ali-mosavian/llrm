//! -O1 left a loop of five constant-trip iterations that -O2 folds: the pin
//! does not unroll past its threshold at -O1, gcc does (float_in_memory_loop
//! -O1: 16 soft-float adds, 100 B and 1264 B of routines, gcc 70 B). The -O1
//! trial (UNROLL-O1-TRIAL) compiles the unrolled module and keeps it where it
//! is no larger and no slower.

use llrm_m68k::compile::CodeModel;
use llrm_m68k::driver::{Level, Policy, assembly_flagged, cpu_of};

const FILL_AND_SUM: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @g() {
entry:
  %v = alloca [5 x i32], align 4
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %m = mul i32 %i, 3
  %x = add i32 %m, 1
  %p = getelementptr inbounds [5 x i32], ptr %v, i32 0, i32 %i
  store i32 %x, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp ult i32 %i1, 5
  br i1 %c, label %fill, label %sum
sum:
  %j = phi i32 [0, %fill], [%j1, %sum]
  %s = phi i32 [0, %fill], [%s1, %sum]
  %q = getelementptr inbounds [5 x i32], ptr %v, i32 0, i32 %j
  %l = load i32, ptr %q
  %s1 = add i32 %s, %l
  %j1 = add i32 %j, 1
  %d = icmp ult i32 %j1, 5
  br i1 %d, label %sum, label %done
done:
  ret i32 %s1
}";

/// Instructions of `g` at -O1 with the unroller on, as `llrm-cc -O1` has it.
fn instructions(cpu: &str) -> usize {
    let mut policy = Policy::default();
    assert!(policy.take("-funroll-loops"));
    let text = assembly_flagged(FILL_AND_SUM, cpu_of(cpu).unwrap(), Level::O1, false, None, CodeModel::Small, &policy)
        .expect("compiles");
    text.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count()
}

#[test]
fn a_loop_of_five_that_o2_folds_is_folded_at_o1() {
    for cpu in ["68000", "68020"] {
        assert!(instructions(cpu) <= 6, "{cpu}: {} instructions", instructions(cpu));
    }
}
