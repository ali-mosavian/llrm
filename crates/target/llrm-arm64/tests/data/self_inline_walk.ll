; ModuleID = 'w15walk.c'
source_filename = "w15walk.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i64 @run(ptr noundef %0) #0 {
  %2 = alloca ptr, align 8
  store ptr %0, ptr %2, align 8, !tbaa !16
  %3 = load ptr, ptr %2, align 8, !tbaa !16
  %4 = call i64 @walk(ptr noundef %3, i32 noundef 1, i32 noundef 1)
  ret i64 %4
}

; Function Attrs: nounwind uwtable
define internal i64 @walk(ptr noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i64, align 8
  %5 = alloca ptr, align 8
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store ptr %0, ptr %5, align 8, !tbaa !16
  store i32 %1, ptr %6, align 4, !tbaa !19
  store i32 %2, ptr %7, align 4, !tbaa !19
  %8 = load i32, ptr %6, align 4, !tbaa !19
  %9 = icmp eq i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %3
  store i64 0, ptr %4, align 8
  br label %46

11:                                               ; preds = %3
  %12 = load ptr, ptr %5, align 8, !tbaa !16
  %13 = load ptr, ptr %5, align 8, !tbaa !16
  %14 = load i32, ptr %6, align 4, !tbaa !19
  %15 = mul nsw i32 3, %14
  %16 = add nsw i32 %15, 1
  %17 = sext i32 %16 to i64
  %18 = getelementptr inbounds i32, ptr %13, i64 %17
  %19 = load i32, ptr %18, align 4, !tbaa !19
  %20 = load i32, ptr %7, align 4, !tbaa !19
  %21 = add nsw i32 %20, 1
  %22 = call i64 @walk(ptr noundef %12, i32 noundef %19, i32 noundef %21)
  %23 = load ptr, ptr %5, align 8, !tbaa !16
  %24 = load i32, ptr %6, align 4, !tbaa !19
  %25 = mul nsw i32 3, %24
  %26 = sext i32 %25 to i64
  %27 = getelementptr inbounds i32, ptr %23, i64 %26
  %28 = load i32, ptr %27, align 4, !tbaa !19
  %29 = sext i32 %28 to i64
  %30 = load i32, ptr %7, align 4, !tbaa !19
  %31 = sext i32 %30 to i64
  %32 = mul nsw i64 %29, %31
  %33 = add nsw i64 %22, %32
  %34 = load ptr, ptr %5, align 8, !tbaa !16
  %35 = load ptr, ptr %5, align 8, !tbaa !16
  %36 = load i32, ptr %6, align 4, !tbaa !19
  %37 = mul nsw i32 3, %36
  %38 = add nsw i32 %37, 2
  %39 = sext i32 %38 to i64
  %40 = getelementptr inbounds i32, ptr %35, i64 %39
  %41 = load i32, ptr %40, align 4, !tbaa !19
  %42 = load i32, ptr %7, align 4, !tbaa !19
  %43 = add nsw i32 %42, 1
  %44 = call i64 @walk(ptr noundef %34, i32 noundef %41, i32 noundef %43)
  %45 = add nsw i64 %33, %44
  store i64 %45, ptr %4, align 8
  br label %46

46:                                               ; preds = %11, %10
  %47 = load i64, ptr %4, align 8
  ret i64 %47
}

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}
!llvm.errno.tbaa = !{!11}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"frame-pointer", i32 4}
!10 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!11 = !{!12, !13, i64 0}
!12 = !{!"__libc_errno", !13, i64 0}
!13 = !{!"int", !14, i64 0}
!14 = !{!"omnipotent char", !15, i64 0}
!15 = !{!"Simple C/C++ TBAA"}
!16 = !{!17, !17, i64 0}
!17 = !{!"p1 int", !18, i64 0}
!18 = !{!"any pointer", !14, i64 0}
!19 = !{!13, !13, i64 0}
