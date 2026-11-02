//! The pin unrolls in a late LoopUnrollPass too, after the combines that fold a
//! constant into an address.

mod common;

use std::process::Command;

/// clang's -O0 IR of tile.c's `bench_tile`: a 40-trip inner loop over `tile[(y
/// + 61) & 63][(x + 7) & 63]`. The early unroll costs the body at 6 and refuses
/// (162 against 150); once the constant is in the address it costs 5.
/// `tile` is external: an internal, never-stored table is all zeros, the loads
/// fold and the pin too leaves `ret i32 0`, with no loop to unroll.
/// `llrm-llc` had no late unroll and left the loop rolled: 8321 clocks against
/// the pin's 4277.
fn tile() -> String {
    r#"target datalayout = "@LAYOUT@"
@tile = global [64 x [64 x i16]] zeroinitializer, align 2

define i32 @bench_tile() {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 4
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  store i16 40, ptr %1, align 2
  store i16 25, ptr %2, align 2
  store i16 7, ptr %3, align 2
  store i16 61, ptr %4, align 2
  store i32 0, ptr %5, align 4
  store i16 0, ptr %7, align 2
  br label %8

8:                                                ; preds = %44, %0
  %9 = load i16, ptr %7, align 2
  %10 = sext i16 %9 to i32
  %11 = load i16, ptr %2, align 2
  %12 = sext i16 %11 to i32
  %13 = icmp slt i32 %10, %12
  br i1 %13, label %14, label %47

14:                                               ; preds = %8
  store i16 0, ptr %6, align 2
  br label %15

15:                                               ; preds = %40, %14
  %16 = load i16, ptr %6, align 2
  %17 = sext i16 %16 to i32
  %18 = load i16, ptr %1, align 2
  %19 = sext i16 %18 to i32
  %20 = icmp slt i32 %17, %19
  br i1 %20, label %21, label %43

21:                                               ; preds = %15
  %22 = load i16, ptr %7, align 2
  %23 = sext i16 %22 to i32
  %24 = load i16, ptr %4, align 2
  %25 = sext i16 %24 to i32
  %26 = add nsw i32 %23, %25
  %27 = and i32 %26, 63
  %28 = getelementptr inbounds [64 x [64 x i16]], ptr @tile, i32 0, i32 %27
  %29 = load i16, ptr %6, align 2
  %30 = sext i16 %29 to i32
  %31 = load i16, ptr %3, align 2
  %32 = sext i16 %31 to i32
  %33 = add nsw i32 %30, %32
  %34 = and i32 %33, 63
  %35 = getelementptr inbounds [64 x i16], ptr %28, i32 0, i32 %34
  %36 = load i16, ptr %35, align 2
  %37 = sext i16 %36 to i32
  %38 = load i32, ptr %5, align 4
  %39 = add nsw i32 %38, %37
  store i32 %39, ptr %5, align 4
  br label %40

40:                                               ; preds = %21
  %41 = load i16, ptr %6, align 2
  %42 = add i16 %41, 1
  store i16 %42, ptr %6, align 2
  br label %15

43:                                               ; preds = %15
  br label %44

44:                                               ; preds = %43
  %45 = load i16, ptr %7, align 2
  %46 = add i16 %45, 1
  store i16 %46, ptr %7, align 2
  br label %8

47:                                               ; preds = %8
  %48 = load i32, ptr %5, align 4
  ret i32 %48
}
"#
    .replace("@LAYOUT@", &common::datalayout("x86-m32"))
}

fn movsx_count(level: &str) -> usize {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("a.ll"), tile()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .current_dir(scratch.path())
        .args(["-m32", "-mabi=sysv", "-march=i486", level, "--isel=dag", "-S", "-o", "-", "a.ll"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).matches("movsx").count()
}

#[test]
fn test_a_loop_the_early_unroll_refused_is_unrolled_late_at_o3() {
    let loads = movsx_count("-O3");
    assert!(loads >= 40, "the 40 loads are copied out ({loads} movsx)");
}

/// m32's table holds the full unroll to the code that does not grow below -O3
/// (bench_tile 149 B against gcc's 161 and clang's 507 at -O2).
#[test]
fn test_the_same_loop_stays_rolled_at_o2() {
    let loads = movsx_count("-O2");
    assert!(loads < 40, "the loop is kept ({loads} movsx)");
}
