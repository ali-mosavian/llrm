//! bench/switch -Os (68000): `bench_switch` made each switch-arm constant with
//! `move.w #n, %a1` (4 B), 308 B against gcc's 262. The allocator tries `a1`,
//! `a0` before `d0`, `d1` for a value either class can hold (M68K-FLEXORDER),
//! and a moveq constant read only by data operands was such a value. The pin
//! tries data registers first, so its `MOVI32ri` is a `moveq`. Now 2 B each.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@switch.table.bench_switch = private unnamed_addr constant [12 x i8] c"\03\0E\0F\\A#YO &.\1A", align 4

define dso_local i32 @bench_switch(i32 noundef %0) optsize {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %3, label %34

3:                                                ; preds = %1, %28
  %4 = phi i32 [ %9, %28 ], [ 1, %1 ]
  %5 = phi i32 [ %32, %28 ], [ 0, %1 ]
  %6 = phi i32 [ %31, %28 ], [ 0, %1 ]
  %7 = mul nsw i32 %4, 75
  %8 = add nsw i32 %7, 74
  %9 = srem i32 %8, 65537
  %10 = srem i32 %9, 10
  %11 = srem i32 %9, 14
  %12 = icmp ult i32 %11, 12
  br i1 %12, label %13, label %17

13:                                               ; preds = %3
  %14 = getelementptr inbounds nuw i8, ptr @switch.table.bench_switch, i32 %11
  %15 = load i8, ptr %14, align 1
  %16 = zext i8 %15 to i32
  br label %17

17:                                               ; preds = %3, %13
  %18 = phi i32 [ %16, %13 ], [ 1, %3 ]
  %19 = mul nsw i32 %10, 37
  %20 = mul nsw i32 %19, %10
  switch i32 %20, label %27 [
    i32 0, label %28
    i32 37, label %21
    i32 148, label %22
    i32 333, label %23
    i32 592, label %24
    i32 925, label %25
    i32 1332, label %26
  ]

21:                                               ; preds = %17
  br label %28

22:                                               ; preds = %17
  br label %28

23:                                               ; preds = %17
  br label %28

24:                                               ; preds = %17
  br label %28

25:                                               ; preds = %17
  br label %28

26:                                               ; preds = %17
  br label %28

27:                                               ; preds = %17
  br label %28

28:                                               ; preds = %17, %21, %22, %23, %24, %25, %26, %27
  %29 = phi i32 [ 1, %27 ], [ 13, %26 ], [ 7, %21 ], [ 23, %22 ], [ 5, %23 ], [ 19, %24 ], [ 2, %25 ], [ 11, %17 ]
  %30 = add nuw nsw i32 %18, %6
  %31 = add nuw nsw i32 %30, %29
  %32 = add nuw nsw i32 %5, 1
  %33 = icmp eq i32 %32, %0
  br i1 %33, label %34, label %3

34:                                               ; preds = %28, %1
  %35 = phi i32 [ 0, %1 ], [ %31, %28 ]
  ret i32 %35
}
"#;

#[test]
fn a_constant_moveq_makes_is_held_in_a_data_register() {
    for cpu in ["68000"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let wide = asm.lines().filter(|line| line.trim_start().starts_with("move.w\t#") && line.contains("%a")).count();
        assert_eq!(wide, 0, "{cpu}: a constant moveq makes goes to an address register:\n{asm}");
        assert!(asm.contains("moveq\t#23,"), "{cpu}: the arm makes 23 with moveq:\n{asm}");
    }
}
