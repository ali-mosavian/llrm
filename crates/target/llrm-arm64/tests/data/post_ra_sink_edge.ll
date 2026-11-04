target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4
@switch.table.bench_switch = private unnamed_addr constant [12 x i8] c"\03\0E\0F\\A#YO &.\1A", align 8

; Function Attrs: nofree noinline norecurse nosync nounwind optsize memory(none)
define range(i64 2, 1) i64 @bench_switch(i32 noundef %0) local_unnamed_addr #0 {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %.lr.ph, label %._crit_edge

.lr.ph:                                           ; preds = %1, %route.exit
  %.017 = phi i64 [ %5, %route.exit ], [ 1, %1 ]
  %.01016 = phi i32 [ %21, %route.exit ], [ 0, %1 ]
  %.01115 = phi i64 [ %20, %route.exit ], [ 0, %1 ]
  %3 = mul nsw i64 %.017, 75
  %4 = add nsw i64 %3, 74
  %5 = srem i64 %4, 65537
  %.lhs.trunc = trunc nsw i64 %5 to i32
  %6 = srem i32 %.lhs.trunc, 10
  %.sext = sext i32 %6 to i64
  %7 = srem i32 %.lhs.trunc, 14
  %8 = icmp ult i32 %7, 12
  br i1 %8, label %switch.lookup, label %weight.exit

switch.lookup:                                    ; preds = %.lr.ph
  %9 = zext nneg i32 %7 to i64
  %switch.gep = getelementptr inbounds nuw i8, ptr @switch.table.bench_switch, i64 %9
  %switch.load = load i8, ptr %switch.gep, align 1
  %switch.ext = zext i8 %switch.load to i64
  br label %weight.exit

weight.exit:                                      ; preds = %.lr.ph, %switch.lookup
  %.0.i = phi i64 [ %switch.ext, %switch.lookup ], [ 1, %.lr.ph ]
  %10 = mul nsw i64 %.sext, 37
  %11 = mul nsw i64 %10, %.sext
  switch i64 %11, label %18 [
    i64 0, label %route.exit
    i64 37, label %12
    i64 148, label %13
    i64 333, label %14
    i64 592, label %15
    i64 925, label %16
    i64 1332, label %17
  ]

12:                                               ; preds = %weight.exit
  br label %route.exit

13:                                               ; preds = %weight.exit
  br label %route.exit

14:                                               ; preds = %weight.exit
  br label %route.exit

15:                                               ; preds = %weight.exit
  br label %route.exit

16:                                               ; preds = %weight.exit
  br label %route.exit

17:                                               ; preds = %weight.exit
  br label %route.exit

18:                                               ; preds = %weight.exit
  br label %route.exit

route.exit:                                       ; preds = %weight.exit, %12, %13, %14, %15, %16, %17, %18
  %.0.i12 = phi i64 [ 1, %18 ], [ 13, %17 ], [ 7, %12 ], [ 23, %13 ], [ 5, %14 ], [ 19, %15 ], [ 2, %16 ], [ 11, %weight.exit ]
  %19 = add nuw nsw i64 %.0.i, %.01115
  %20 = add nuw nsw i64 %19, %.0.i12
  %21 = add nuw nsw i32 %.01016, 1
  %exitcond.not = icmp eq i32 %21, %0
  br i1 %exitcond.not, label %._crit_edge, label %.lr.ph, !llvm.loop !15

._crit_edge:                                      ; preds = %route.exit, %1
  %.011.lcssa = phi i64 [ 0, %1 ], [ %20, %route.exit ]
  ret i64 %.011.lcssa
}

; Function Attrs: nounwind optsize
define noundef i32 @main() local_unnamed_addr #1 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !17
  %2 = add nsw i32 %1, 2000
  %3 = tail call i64 @bench_switch(i32 noundef %2) #3
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
!15 = distinct !{!15, !16}
!16 = !{!"llvm.loop.mustprogress"}
!17 = !{!12, !12, i64 0}
