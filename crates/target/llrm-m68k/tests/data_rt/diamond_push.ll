; ModuleID = '/home/alim/scratch/mfext/tests/run/c/compare_to_int64.c'
source_filename = "/home/alim/scratch/mfext/tests/run/c/compare_to_int64.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i64 0, ptr %1, align 8
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 -1, ptr %2, align 2
  br label %4

4:                                                ; preds = %39, %0
  %5 = load i32, ptr %2, align 2
  %6 = icmp sle i32 %5, 1
  br i1 %6, label %7, label %42

7:                                                ; preds = %4
  store i32 -1, ptr %3, align 2
  br label %8

8:                                                ; preds = %35, %7
  %9 = load i32, ptr %3, align 2
  %10 = icmp sle i32 %9, 1
  br i1 %10, label %11, label %38

11:                                               ; preds = %8
  %12 = load i64, ptr %1, align 8
  %13 = mul nsw i64 %12, 3
  %14 = load i32, ptr %2, align 2
  %15 = load i32, ptr %3, align 2
  %16 = call i64 @lt(i32 noundef %14, i32 noundef %15) #4
  %17 = add nsw i64 %13, %16
  %18 = load i32, ptr %2, align 2
  %19 = load i32, ptr %3, align 2
  %20 = call i64 @ne(i32 noundef %18, i32 noundef %19) #4
  %21 = mul nsw i64 %20, 2
  %22 = add nsw i64 %17, %21
  %23 = load i32, ptr %2, align 2
  %24 = load i32, ptr %3, align 2
  %25 = call i64 @both(i32 noundef %23, i32 noundef %24) #4
  %26 = lshr i64 %25, 33
  %27 = mul nsw i64 %26, 5
  %28 = add nsw i64 %22, %27
  %29 = load i32, ptr %2, align 2
  %30 = load i32, ptr %3, align 2
  %31 = icmp eq i32 %29, %30
  %32 = zext i1 %31 to i32
  %33 = sext i32 %32 to i64
  %34 = sub nsw i64 %28, %33
  store i64 %34, ptr %1, align 8
  br label %35

35:                                               ; preds = %11
  %36 = load i32, ptr %3, align 2
  %37 = add nsw i32 %36, 1
  store i32 %37, ptr %3, align 2
  br label %8

38:                                               ; preds = %8
  br label %39

39:                                               ; preds = %38
  %40 = load i32, ptr %2, align 2
  %41 = add nsw i32 %40, 1
  store i32 %41, ptr %2, align 2
  br label %4

42:                                               ; preds = %4
  %43 = load i64, ptr %1, align 8
  %44 = trunc i64 %43 to i32
  call void @report(i32 noundef %44) #4
  %45 = load i64, ptr %1, align 8
  %46 = ashr i64 %45, 3
  %47 = trunc i64 %46 to i32
  %48 = call i64 @both(i32 noundef 1, i32 noundef 1) #4
  %49 = lshr i64 %48, 32
  %50 = trunc i64 %49 to i32
  %51 = add nsw i32 %47, %50
  call void @report(i32 noundef %51) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal i64 @lt(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2
  store i32 %1, ptr %4, align 2
  %5 = load i32, ptr %3, align 2
  %6 = load i32, ptr %4, align 2
  %7 = icmp slt i32 %5, %6
  %8 = zext i1 %7 to i32
  %9 = sext i32 %8 to i64
  ret i64 %9
}

; Function Attrs: nounwind
define internal i64 @ne(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2
  store i32 %1, ptr %4, align 2
  %5 = load i32, ptr %3, align 2
  %6 = load i32, ptr %4, align 2
  %7 = icmp ne i32 %5, %6
  %8 = zext i1 %7 to i32
  %9 = sext i32 %8 to i64
  ret i64 %9
}

; Function Attrs: nounwind
define internal i64 @both(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2
  store i32 %1, ptr %4, align 2
  %5 = load i32, ptr %3, align 2
  %6 = icmp ne i32 %5, 0
  br i1 %6, label %7, label %10

7:                                                ; preds = %2
  %8 = load i32, ptr %4, align 2
  %9 = icmp ne i32 %8, 0
  br label %10

10:                                               ; preds = %7, %2
  %11 = phi i1 [ false, %2 ], [ %9, %7 ]
  %12 = zext i1 %11 to i32
  %13 = sext i32 %12 to i64
  %14 = shl i64 %13, 33
  ret i64 %14
}

declare void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin "no-builtins" }


