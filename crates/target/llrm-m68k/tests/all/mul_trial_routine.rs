//! The -Os multiply trial priced the call of `__mulsi3` without the routine's
//! 54 B, so a chain of adds that was a little larger than `pea 3; move.l
//! %d2,-(%sp); jsr (%a2)` lost to the call, and the program linked the routine
//! (div8_os 68000: 754 -> 800 B linked, 5.3 -> 7.5 M clocks). The build that
//! calls it pays for it.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// `total` of tests/run/c/div8_os.c as the shared mid-end leaves it: `sum * 3`
/// in a loop beside byte divides.
const TOTAL: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @total() nounwind optsize minsize {
  br label %1

1:
  %2 = phi i32 [ 0, %0 ], [ %50, %53 ]
  %3 = phi i32 [ 0, %0 ], [ %54, %53 ]
  %4 = mul i32 %2, 3
  %.lhs.trunc = trunc nuw i32 %3 to i8
  %.lhs.trunc.frozen = freeze i8 %.lhs.trunc
  %5 = udiv i8 %.lhs.trunc.frozen, 6
  %.zext = zext nneg i8 %5 to i32
  %6 = add i32 %4, %.zext
  %7 = mul i8 %5, 6
  %8 = sub i8 %.lhs.trunc.frozen, %7
  %9 = mul nuw nsw i8 %8, 7
  %10 = zext nneg i8 %9 to i32
  %11 = shl nuw i32 %3, 24
  %12 = ashr exact i32 %11, 24
  %.lhs.trunc.frozen1 = freeze i8 %.lhs.trunc
  %13 = sdiv i8 %.lhs.trunc.frozen1, 6
  %.sext = sext i8 %13 to i32
  %14 = mul i8 %13, 6
  %15 = sub i8 %.lhs.trunc.frozen1, %14
  %16 = mul nsw i8 %15, 5
  %17 = sext i8 %16 to i32
  %.lhs.trunc.frozen2 = freeze i8 %.lhs.trunc
  %18 = udiv i8 %.lhs.trunc.frozen2, 100
  %.zext2 = zext nneg i8 %18 to i32
  %19 = mul i8 %18, 100
  %20 = sub i8 %.lhs.trunc.frozen2, %19
  %.zext3 = zext nneg i8 %20 to i32
  %21 = add i32 %6, %.sext
  %22 = add i32 %21, %.zext2
  %23 = add i32 %22, %.zext3
  %24 = add i32 %23, %10
  %25 = add i32 %24, %17
  %26 = mul i32 %25, 3
  %27 = sdiv i8 %.lhs.trunc, -7
  %.sext2 = sext i8 %27 to i32
  %28 = srem i8 %.lhs.trunc, 7
  %29 = mul nsw i8 %28, 3
  %30 = sext i8 %29 to i32
  %31 = add nsw i32 %30, %.sext2
  %32 = add i32 %31, %26
  %.lhs.trunc8 = trunc nsw i32 %12 to i16
  %.lhs.trunc9 = trunc nsw i32 %12 to i16
  br label %33

33:
  %34 = phi i32 [ %52, %33 ], [ 16777216, %1 ]
  %35 = phi i32 [ %32, %1 ], [ %50, %33 ]
  %36 = phi i32 [ 1, %1 ], [ %51, %33 ]
  %37 = mul i32 %35, 3
  %38 = udiv i32 %3, %36
  %39 = add i32 %38, %37
  %40 = mul i32 %38, %36
  %41 = sub i32 %3, %40
  %42 = mul nuw nsw i32 %41, 11
  %43 = add i32 %39, %42
  %44 = ashr exact i32 %34, 24
  %.rhs.trunc = trunc nsw i32 %44 to i16
  %45 = sdiv i16 %.lhs.trunc8, %.rhs.trunc
  %.sext4 = sext i16 %45 to i32
  %46 = add i32 %43, %.sext4
  %.rhs.trunc1 = trunc nsw i32 %44 to i16
  %47 = srem i16 %.lhs.trunc9, %.rhs.trunc1
  %48 = mul nsw i16 %47, 13
  %49 = sext i16 %48 to i32
  %50 = add i32 %46, %49
  %51 = add i32 %36, 17
  %52 = add i32 %34, 285212672
  %exitcond = icmp eq i32 %51, 256
  br i1 %exitcond, label %53, label %33

53:
  %54 = add i32 %3, 1
  %exitcond1 = icmp eq i32 %54, 256
  br i1 %exitcond1, label %55, label %1

55:
  ret i32 %50
}";

#[test]
fn a_chain_of_adds_beats_a_call_that_links_the_multiply_routine() {
    let asm = assembly(TOTAL, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    assert!(
        !asm.contains("__mulsi3"),
        "sum * 3 is a call, and links 54 B of __mulsi3:
{asm}"
    );
}
