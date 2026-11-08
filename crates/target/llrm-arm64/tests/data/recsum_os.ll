; ModuleID = '/home/alim/work/personal/llrm/bench/recsum/recsum.c'
source_filename = "/home/alim/work/personal/llrm/bench/recsum/recsum.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind optsize uwtable
define dso_local i64 @bench_recsum(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !16
  %3 = load i32, ptr %2, align 4, !tbaa !16
  %4 = call i64 @sum(i64 noundef 0, i32 noundef %3) #2
  ret i64 %4
}

; Function Attrs: nounwind optsize uwtable
define internal i64 @sum(i64 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  store i64 %0, ptr %4, align 8, !tbaa !17
  store i32 %1, ptr %5, align 4, !tbaa !16
  %6 = load i32, ptr %5, align 4, !tbaa !16
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i64, ptr %4, align 8, !tbaa !17
  store i64 %9, ptr %3, align 8
  br label %20

10:                                               ; preds = %2
  %11 = load i64, ptr %4, align 8, !tbaa !17
  %12 = load i32, ptr %5, align 4, !tbaa !16
  %13 = mul nsw i32 %12, 3
  %14 = srem i32 %13, 11
  %15 = sext i32 %14 to i64
  %16 = add nsw i64 %11, %15
  %17 = load i32, ptr %5, align 4, !tbaa !16
  %18 = sub nsw i32 %17, 1
  %19 = call i64 @sum(i64 noundef %16, i32 noundef %18) #2
  store i64 %19, ptr %3, align 8
  br label %20

20:                                               ; preds = %10, %8
  %21 = load i64, ptr %3, align 8
  ret i64 %21
}

; Function Attrs: nounwind optsize uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i64 @bench_recsum(i32 noundef 2000) #2
  call void @report(i64 noundef %2) #2
  ret i32 0
}

; Function Attrs: optsize
declare void @report(i64 noundef) #1

attributes #0 = { nounwind optsize uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { optsize }

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
!16 = !{!13, !13, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !14, i64 0}
