; ModuleID = '/home/alim/scratch/w15-out-tab2/src/ring.c'
source_filename = "/home/alim/scratch/w15-out-tab2/src/ring.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: noinline nounwind uwtable
define dso_local i64 @bench_ring() #0 {
  %1 = alloca [1024 x i64], align 8
  %2 = alloca i64, align 8
  %3 = alloca i16, align 2
  store i16 0, ptr %3, align 2
  br label %4

4:                                                ; preds = %18, %0
  %5 = load i16, ptr %3, align 2
  %6 = sext i16 %5 to i32
  %7 = icmp slt i32 %6, 1024
  br i1 %7, label %8, label %21

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2
  %10 = sext i16 %9 to i32
  %11 = add nsw i32 %10, 1
  %12 = sext i32 %11 to i64
  %13 = mul nsw i64 %12, 3
  %14 = sub nsw i64 %13, 7
  %15 = load i16, ptr %3, align 2
  %16 = sext i16 %15 to i64
  %17 = getelementptr inbounds [1024 x i64], ptr %1, i64 0, i64 %16
  store i64 %14, ptr %17, align 8
  br label %18

18:                                               ; preds = %8
  %19 = load i16, ptr %3, align 2
  %20 = add i16 %19, 1
  store i16 %20, ptr %3, align 2
  br label %4, !llvm.loop !11

21:                                               ; preds = %4
  store i64 0, ptr %2, align 8
  store i16 0, ptr %3, align 2
  br label %22

22:                                               ; preds = %37, %21
  %23 = load i16, ptr %3, align 2
  %24 = sext i16 %23 to i32
  %25 = icmp slt i32 %24, 6000
  br i1 %25, label %26, label %40

26:                                               ; preds = %22
  %27 = load i16, ptr %3, align 2
  %28 = sext i16 %27 to i32
  %29 = mul nsw i32 %28, 5
  %30 = add nsw i32 %29, 3
  %31 = and i32 %30, 1023
  %32 = sext i32 %31 to i64
  %33 = getelementptr inbounds [1024 x i64], ptr %1, i64 0, i64 %32
  %34 = load i64, ptr %33, align 8
  %35 = load i64, ptr %2, align 8
  %36 = add nsw i64 %35, %34
  store i64 %36, ptr %2, align 8
  br label %37

37:                                               ; preds = %26
  %38 = load i16, ptr %3, align 2
  %39 = add i16 %38, 1
  store i16 %39, ptr %3, align 2
  br label %22, !llvm.loop !13

40:                                               ; preds = %22
  %41 = load i64, ptr %2, align 8
  ret i64 %41
}

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i64 @bench_ring()
  call void @report(i64 noundef %2)
  ret i32 0
}

declare void @report(i64 noundef) #1

attributes #0 = { noinline nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}

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
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
