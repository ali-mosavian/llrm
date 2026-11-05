; ModuleID = '/home/alim/scratch/w15-bar/out/src/recpow.c'
source_filename = "/home/alim/scratch/w15-bar/out/src/recpow.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: nofree noinline nosync nounwind memory(none)
define dso_local i64 @bench_recpow(i32 noundef %0) local_unnamed_addr #0 {
  %2 = icmp slt i32 %0, 1
  br i1 %2, label %15, label %3

3:                                                ; preds = %1, %3
  %4 = phi i32 [ %13, %3 ], [ 1, %1 ]
  %5 = phi i64 [ %12, %3 ], [ 0, %1 ]
  %6 = urem i32 %4, 97
  %7 = add nuw nsw i32 %6, 2
  %8 = zext nneg i32 %7 to i64
  %9 = add nuw nsw i32 %4, 40
  %10 = tail call fastcc i64 @power(i64 noundef %8, i32 noundef %9) #4
  %11 = srem i64 %10, 1000
  %12 = add nsw i64 %11, %5
  %13 = add nuw i32 %4, 1
  %14 = icmp eq i32 %4, %0
  br i1 %14, label %15, label %3, !llvm.loop !13

15:                                               ; preds = %3, %1
  %16 = phi i64 [ 0, %1 ], [ %12, %3 ]
  ret i64 %16

}

; Function Attrs: nofree nosync nounwind memory(none)
define internal fastcc range(i64 -30010, 30011) i64 @power(i64 noundef range(i64 -30010, 30011) %0, i32 noundef %1) unnamed_addr #1 {
  br label %3

3:                                                ; preds = %17, %2
  %4 = phi i64 [ %0, %2 ], [ %19, %17 ]
  %5 = phi i32 [ %1, %2 ], [ %20, %17 ]
  %6 = icmp eq i32 %5, 0
  br i1 %6, label %10, label %7

7:                                                ; preds = %3
  %8 = and i32 %5, 1
  %9 = icmp eq i32 %8, 0
  br i1 %9, label %17, label %12

10:                                               ; preds = %3, %12
  %11 = phi i64 [ %16, %12 ], [ 1, %3 ]
  ret i64 %11

12:                                               ; preds = %7
  %13 = add nsw i32 %5, -1
  %14 = tail call fastcc i64 @power(i64 noundef %4, i32 noundef %13) #4
  %15 = mul nsw i64 %14, %4
  %16 = srem i64 %15, 30011
  br label %10

17:                                               ; preds = %7
  %18 = mul nsw i64 %4, %4
  %19 = urem i64 %18, 30011
  %20 = ashr exact i32 %5, 1
  br label %3

}

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #2 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !16
  %2 = add nsw i32 %1, 400
  %3 = tail call i64 @bench_recpow(i32 noundef %2) #4
  tail call void @report(i64 noundef %3) #5
  ret i32 0
}

declare dso_local void @report(i64 noundef) local_unnamed_addr #3

attributes #0 = { nofree noinline nosync nounwind memory(none) "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nofree nosync nounwind memory(none) "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nobuiltin "no-builtins" }
attributes #5 = { nobuiltin nounwind "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6}
!llvm.ident = !{!7}
!llvm.errno.tbaa = !{!8}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 7, !"frame-pointer", i32 4}
!7 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!8 = !{!9, !10, i64 0}
!9 = !{!"__libc_errno", !10, i64 0}
!10 = !{!"int", !11, i64 0}
!11 = !{!"omnipotent char", !12, i64 0}
!12 = !{!"Simple C/C++ TBAA"}
!13 = distinct !{!13, !14, !15}
!14 = !{!"llvm.loop.mustprogress"}
!15 = !{!"llvm.loop.unroll.disable"}
!16 = !{!10, !10, i64 0}
