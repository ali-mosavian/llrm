; ModuleID = '/home/alim/scratch/w10-ipacp/a64/src/textfill.c'
source_filename = "/home/alim/scratch/w10-ipacp/a64/src/textfill.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@host_text_screen = external local_unnamed_addr global [4000 x i8], align 1

; Function Attrs: nofree noinline norecurse nosync nounwind memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none)
define dso_local i64 @bench_textfill() local_unnamed_addr #0 {
  br label %1

1:                                                ; preds = %1, %0
  %2 = phi i64 [ 0, %0 ], [ %8, %1 ]
  %3 = trunc i64 %2 to i8
  %4 = and i8 %3, 14
  %5 = or disjoint i8 %4, 65
  %6 = getelementptr inbounds nuw i8, ptr @host_text_screen, i64 %2
  store i8 %5, ptr %6, align 1, !tbaa !15
  %7 = getelementptr i8, ptr %6, i64 1
  store i8 31, ptr %7, align 1, !tbaa !15
  %8 = add nuw nsw i64 %2, 2
  %9 = icmp samesign ult i64 %2, 3997
  br i1 %9, label %1, label %10, !llvm.loop !16

10:                                               ; preds = %1, %10
  %11 = phi i64 [ %17, %10 ], [ 0, %1 ]
  %12 = phi i64 [ %16, %10 ], [ 0, %1 ]
  %13 = getelementptr inbounds nuw i8, ptr @host_text_screen, i64 %11
  %14 = load i8, ptr %13, align 1, !tbaa !15
  %15 = zext i8 %14 to i64
  %16 = add nuw nsw i64 %12, %15
  %17 = add nuw nsw i64 %11, 1
  %18 = icmp eq i64 %17, 4000
  br i1 %18, label %19, label %10, !llvm.loop !19

19:                                               ; preds = %10
  ret i64 %16
}

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #1 {
  %1 = tail call i64 @bench_textfill() #3
  tail call void @report(i64 noundef %1) #4
  ret i32 0
}

declare void @report(i64 noundef) local_unnamed_addr #2

attributes #0 = { nofree noinline norecurse nosync nounwind memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none) "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nobuiltin "no-builtins" }
attributes #4 = { nobuiltin nounwind "no-builtins" }

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
!15 = !{!13, !13, i64 0}
!16 = distinct !{!16, !17, !18}
!17 = !{!"llvm.loop.mustprogress"}
!18 = !{!"llvm.loop.unroll.disable"}
!19 = distinct !{!19, !17, !18}
