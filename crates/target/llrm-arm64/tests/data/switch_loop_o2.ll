target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@switch.table.bench_switch = private unnamed_addr constant [12 x i8] c"\03\0E\0F\\A#YO &.\1A", align 8

; Function Attrs: nofree norecurse nosync nounwind memory(none) uwtable
define dso_local range(i64 2, 1) i64 @bench_switch(i32 noundef %0) local_unnamed_addr #0 {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %3, label %37

3:                                                ; preds = %1, %31
  %4 = phi i64 [ %9, %31 ], [ 1, %1 ]
  %5 = phi i32 [ %35, %31 ], [ 0, %1 ]
  %6 = phi i64 [ %34, %31 ], [ 0, %1 ]
  %7 = mul nsw i64 %4, 75
  %8 = add nsw i64 %7, 74
  %9 = srem i64 %8, 65537
  %10 = trunc nsw i64 %9 to i32
  %11 = srem i32 %10, 10
  %12 = sext i32 %11 to i64
  %13 = srem i32 %10, 14
  %14 = icmp ult i32 %13, 12
  br i1 %14, label %15, label %20

15:                                               ; preds = %3
  %16 = zext nneg i32 %13 to i64
  %17 = getelementptr inbounds nuw i8, ptr @switch.table.bench_switch, i64 %16
  %18 = load i8, ptr %17, align 1
  %19 = zext i8 %18 to i64
  br label %20

20:                                               ; preds = %3, %15
  %21 = phi i64 [ %19, %15 ], [ 1, %3 ]
  %22 = mul nsw i64 %12, 37
  %23 = mul nsw i64 %22, %12
  switch i64 %23, label %30 [
    i64 0, label %31
    i64 37, label %24
    i64 148, label %25
    i64 333, label %26
    i64 592, label %27
    i64 925, label %28
    i64 1332, label %29
  ]

24:                                               ; preds = %20
  br label %31

25:                                               ; preds = %20
  br label %31

26:                                               ; preds = %20
  br label %31

27:                                               ; preds = %20
  br label %31

28:                                               ; preds = %20
  br label %31

29:                                               ; preds = %20
  br label %31

30:                                               ; preds = %20
  br label %31

31:                                               ; preds = %20, %24, %25, %26, %27, %28, %29, %30
  %32 = phi i64 [ 1, %30 ], [ 13, %29 ], [ 7, %24 ], [ 23, %25 ], [ 5, %26 ], [ 19, %27 ], [ 2, %28 ], [ 11, %20 ]
  %33 = add nuw nsw i64 %21, %6
  %34 = add nuw nsw i64 %33, %32
  %35 = add nuw nsw i32 %5, 1
  %36 = icmp eq i32 %35, %0
  br i1 %36, label %37, label %3

37:                                               ; preds = %31, %1
  %38 = phi i64 [ 0, %1 ], [ %34, %31 ]
  ret i64 %38
}

; Function Attrs: nounwind uwtable


attributes #0 = { nofree norecurse nosync nounwind memory(none) uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nounwind }



