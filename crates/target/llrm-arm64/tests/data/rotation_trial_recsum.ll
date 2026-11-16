; ModuleID = '/home/alim/scratch/lvcensus/out-os1/src/recsum.c'
source_filename = "/home/alim/scratch/lvcensus/out-os1/src/recsum.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind optsize
define dso_local i64 @bench_recsum(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !14
  %3 = load i32, ptr %2, align 4, !tbaa !14
  %4 = call i64 @sum(i64 noundef 0, i32 noundef %3) #3
  ret i64 %4
}

; Function Attrs: nounwind optsize
define internal i64 @sum(i64 noundef %0, i32 noundef %1) #1 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  store i64 %0, ptr %4, align 8, !tbaa !15
  store i32 %1, ptr %5, align 4, !tbaa !14
  %6 = load i32, ptr %5, align 4, !tbaa !14
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i64, ptr %4, align 8, !tbaa !15
  store i64 %9, ptr %3, align 8
  br label %20

10:                                               ; preds = %2
  %11 = load i64, ptr %4, align 8, !tbaa !15
  %12 = load i32, ptr %5, align 4, !tbaa !14
  %13 = mul nsw i32 %12, 3
  %14 = srem i32 %13, 11
  %15 = sext i32 %14 to i64
  %16 = add nsw i64 %11, %15
  %17 = load i32, ptr %5, align 4, !tbaa !14
  %18 = sub nsw i32 %17, 1
  %19 = call i64 @sum(i64 noundef %16, i32 noundef %18) #3
  store i64 %19, ptr %3, align 8
  br label %20

20:                                               ; preds = %10, %8
  %21 = load i64, ptr %3, align 8
  ret i64 %21
}

; Function Attrs: nounwind optsize
define dso_local i32 @main() #1 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !14
  %2 = add nsw i32 2000, %1
  %3 = call i64 @bench_recsum(i32 noundef %2) #3
  call void @report(i64 noundef %3) #3
  ret i32 0
}

; Function Attrs: optsize
declare void @report(i64 noundef) #2

attributes #0 = { noinline nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nobuiltin optsize "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = !{!11, !11, i64 0}
!15 = !{!16, !16, i64 0}
!16 = !{!"long", !12, i64 0}
