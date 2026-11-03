target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: nofree noinline norecurse nosync nounwind optsize memory(none)
define i64 @bench_recmany(i32 noundef %0) local_unnamed_addr #0 {
  %2 = icmp eq i32 %0, 0
  br i1 %2, label %25, label %3

3:                                                ; preds = %1, %3
  %4 = phi i32 [ %18, %3 ], [ %0, %1 ]
  %5 = phi i64 [ %17, %3 ], [ 5, %1 ]
  %6 = phi i64 [ %15, %3 ], [ 4, %1 ]
  %7 = phi i64 [ %13, %3 ], [ 3, %1 ]
  %8 = phi i64 [ %12, %3 ], [ 2, %1 ]
  %9 = phi i64 [ %10, %3 ], [ 1, %1 ]
  %10 = add nsw i64 %8, 1
  %11 = srem i64 %9, 5
  %12 = add nsw i64 %11, %7
  %13 = xor i64 %8, %6
  %14 = srem i64 %7, 3
  %15 = add nsw i64 %14, %5
  %16 = srem i64 %6, 7
  %17 = add nsw i64 %9, %16
  %18 = add nsw i32 %4, -1
  %19 = icmp eq i32 %18, 0
  br i1 %19, label %20, label %3

20:                                               ; preds = %3
  %21 = add nsw i64 %12, %10
  %22 = add nsw i64 %21, %13
  %23 = add nsw i64 %22, %15
  %24 = add nsw i64 %23, %17
  br label %25

25:                                               ; preds = %20, %1
  %26 = phi i64 [ 15, %1 ], [ %24, %20 ]
  ret i64 %26
}

; Function Attrs: nounwind optsize
define noundef i32 @main() local_unnamed_addr #1 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !15
  %2 = add nsw i32 %1, 1500
  %3 = tail call i64 @bench_recmany(i32 noundef %2) #3
  tail call void @report(i64 noundef %3) #4
  ret i32 0
}

; Function Attrs: optsize
declare void @report(i64 noundef) local_unnamed_addr #2

attributes #0 = { nofree noinline norecurse nosync nounwind optsize memory(none) "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nobuiltin optsize "no-builtins" }
attributes #4 = { nobuiltin nounwind optsize "no-builtins" }

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
