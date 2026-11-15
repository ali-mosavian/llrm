//! UNROLL-NEST-TRIAL. The target's full-unroll threshold (32) prices the
//! control of the loop that does the work. It also refused the 4-trip OUTER
//! loop of a nest over a constant table, which the pin's 150 unrolls and its
//! cost analysis never judges (it skips a loop with a subloop): the table loads
//! stayed loads and every shift stayed a variable one (shift_int64_variable -O2
//! 68000: 248818 clk against clang's 162092, rotate_small +29% over gcc). A
//! trial compile with the nest unrolled is kept where the code is no larger and
//! no slower.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const NEST: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@tbl = internal constant [4 x i32] [i32 305419896, i32 -1, i32 -2147483647, i32 65537], align 4
define i32 @k() {
entry:
  br label %outer
outer:
  %i = phi i32 [ 0, %entry ], [ %i1, %inner.done ]
  %acc = phi i32 [ 0, %entry ], [ %acc2, %inner.done ]
  %p = getelementptr i32, ptr @tbl, i32 %i
  %v = load i32, ptr %p
  br label %inner
inner:
  %c = phi i32 [ 0, %outer ], [ %c1, %inner ]
  %a = phi i32 [ %acc, %outer ], [ %a1, %inner ]
  %s = shl i32 %v, %c
  %m = mul i32 %a, 31
  %a1 = add i32 %m, %s
  %c1 = add i32 %c, 1
  %e = icmp eq i32 %c1, 64
  br i1 %e, label %inner.done, label %inner
inner.done:
  %acc2 = phi i32 [ %a1, %inner ]
  %i1 = add i32 %i, 1
  %d = icmp eq i32 %i1, 4
  br i1 %d, label %done, label %outer
done:
  ret i32 %acc2
}
";

#[test]
fn an_outer_loop_over_a_constant_table_is_unrolled_when_that_shrinks_the_code() {
    for cpu in ["68000", "68020"] {
        let out = assembly(NEST, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(!out.contains("tbl"), "{cpu}: the table is still read:\n{out}");
    }
}

/// The nest's table is a global the unrolled side drops, so the trial's set of
/// differing names holds a global as well as the function: lowering it as a
/// function failed, the failure was swallowed and every trial put the unroll
/// back. Such a failure is now the compile's error.
///
/// The trial lowers the nest's function once with the unroll and once without,
/// to price them; the side it keeps was printed from those items. Lowering the
/// winner again for the output cost 1.5x the compile (matmul -O2 68000: 4466 M
/// instructions against 3334 M).
#[test]
fn the_trials_kept_side_is_lowered_once() {
    let before = llrm_m68k::driver::lowerings();
    let out = assembly(NEST, cpu_of("68000").unwrap(), Level::O2).expect("compiles");
    assert!(!out.contains("tbl"), "premise: the unrolled side is kept:\n{out}");
    assert_eq!(llrm_m68k::driver::lowerings() - before, 2, "one function, lowered with and without the unroll");
}
