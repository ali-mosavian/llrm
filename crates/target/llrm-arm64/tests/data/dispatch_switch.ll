; ModuleID = 'tests/programs/dispatch.c'
source_filename = "tests/programs/dispatch.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@__const.main.ops = private unnamed_addr constant [12 x i32] [i32 1, i32 2, i32 3, i32 4, i32 10, i32 100, i32 1000, i32 70000, i32 -5, i32 7, i32 0, i32 -6], align 4
@.str = private unnamed_addr constant [4 x i8] c"%d \00", align 1
@.str.1 = private unnamed_addr constant [2 x i8] c"\0A\00", align 1
@.str.2 = private unnamed_addr constant [5 x i8] c"ten\0A\00", align 1

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = alloca [12 x i32], align 4
  %2 = alloca i32, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %1, ptr align 4 @__const.main.ops, i64 48, i1 false)
  store i32 0, ptr %2, align 4, !tbaa !15
  br label %3

3:                                                ; preds = %15, %0
  %4 = load i32, ptr %2, align 4, !tbaa !15
  %5 = icmp slt i32 %4, 12
  br i1 %5, label %6, label %18

6:                                                ; preds = %3
  %7 = load i32, ptr %2, align 4, !tbaa !15
  %8 = sext i32 %7 to i64
  %9 = getelementptr inbounds [12 x i32], ptr %1, i64 0, i64 %8
  %10 = load i32, ptr %9, align 4, !tbaa !15
  %11 = load i32, ptr %2, align 4, !tbaa !15
  %12 = add nsw i32 20, %11
  %13 = call i32 @run(i32 noundef %10, i32 noundef %12) #4
  %14 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %13) #4
  br label %15

15:                                               ; preds = %6
  %16 = load i32, ptr %2, align 4, !tbaa !15
  %17 = add nsw i32 %16, 1
  store i32 %17, ptr %2, align 4, !tbaa !15
  br label %3, !llvm.loop !16

18:                                               ; preds = %3
  %19 = call i32 (ptr, ...) @printf(ptr noundef @.str.1) #4
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #1

declare i32 @printf(ptr noundef, ...) #2

; Function Attrs: noinline nounwind
define internal i32 @run(i32 noundef %0, i32 noundef %1) #3 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !15
  store i32 %1, ptr %5, align 4, !tbaa !15
  %6 = load i32, ptr %4, align 4, !tbaa !15
  switch i32 %6, label %36 [
    i32 1, label %7
    i32 2, label %10
    i32 3, label %13
    i32 4, label %16
    i32 10, label %19
    i32 100, label %22
    i32 1000, label %26
    i32 70000, label %30
    i32 -5, label %33
  ]

7:                                                ; preds = %2
  %8 = load i32, ptr %5, align 4, !tbaa !15
  %9 = call i32 @a(i32 noundef %8) #4
  store i32 %9, ptr %3, align 4
  br label %37

10:                                               ; preds = %2
  %11 = load i32, ptr %5, align 4, !tbaa !15
  %12 = call i32 @b(i32 noundef %11) #4
  store i32 %12, ptr %3, align 4
  br label %37

13:                                               ; preds = %2
  %14 = load i32, ptr %5, align 4, !tbaa !15
  %15 = add nsw i32 %14, 5
  store i32 %15, ptr %5, align 4, !tbaa !15
  br label %16

16:                                               ; preds = %2, %13
  %17 = load i32, ptr %5, align 4, !tbaa !15
  %18 = call i32 @c(i32 noundef %17) #4
  store i32 %18, ptr %3, align 4
  br label %37

19:                                               ; preds = %2
  %20 = call i32 (ptr, ...) @printf(ptr noundef @.str.2) #4
  %21 = load i32, ptr %5, align 4, !tbaa !15
  store i32 %21, ptr %3, align 4
  br label %37

22:                                               ; preds = %2
  %23 = load i32, ptr %5, align 4, !tbaa !15
  %24 = call i32 @b(i32 noundef %23) #4
  %25 = call i32 @a(i32 noundef %24) #4
  store i32 %25, ptr %3, align 4
  br label %37

26:                                               ; preds = %2
  %27 = load i32, ptr %5, align 4, !tbaa !15
  %28 = call i32 @c(i32 noundef %27) #4
  %29 = call i32 @c(i32 noundef %28) #4
  store i32 %29, ptr %3, align 4
  br label %37

30:                                               ; preds = %2
  %31 = load i32, ptr %5, align 4, !tbaa !15
  %32 = xor i32 %31, 255
  store i32 %32, ptr %3, align 4
  br label %37

33:                                               ; preds = %2
  %34 = load i32, ptr %5, align 4, !tbaa !15
  %35 = sub nsw i32 0, %34
  store i32 %35, ptr %3, align 4
  br label %37

36:                                               ; preds = %2
  store i32 0, ptr %3, align 4
  br label %37

37:                                               ; preds = %36, %33, %30, %26, %22, %19, %16, %10, %7
  %38 = load i32, ptr %3, align 4
  ret i32 %38
}

; Function Attrs: nounwind
define internal i32 @a(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !15
  %3 = load i32, ptr %2, align 4, !tbaa !15
  %4 = add nsw i32 %3, 1
  ret i32 %4
}

; Function Attrs: nounwind
define internal i32 @b(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !15
  %3 = load i32, ptr %2, align 4, !tbaa !15
  %4 = mul nsw i32 %3, 3
  ret i32 %4
}

; Function Attrs: nounwind
define internal i32 @c(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !15
  %3 = load i32, ptr %2, align 4, !tbaa !15
  %4 = sub nsw i32 %3, 7
  ret i32 %4
}

attributes #0 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { noinline nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8}
!llvm.ident = !{!9}
!llvm.errno.tbaa = !{!10}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"frame-pointer", i32 4}
!9 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!10 = !{!11, !12, i64 0}
!11 = !{!"__libc_errno", !12, i64 0}
!12 = !{!"int", !13, i64 0}
!13 = !{!"omnipotent char", !14, i64 0}
!14 = !{!"Simple C/C++ TBAA"}
!15 = !{!12, !12, i64 0}
!16 = distinct !{!16, !17}
!17 = !{!"llvm.loop.mustprogress"}
