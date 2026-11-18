target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@px = internal unnamed_addr global [101 x i16] zeroinitializer, align 2
@py = internal unnamed_addr global [101 x i16] zeroinitializer, align 2
@pz = internal unnamed_addr global [101 x i16] zeroinitializer, align 2
@vx = internal unnamed_addr global [101 x i16] zeroinitializer, align 2
@vy = internal unnamed_addr global [101 x i16] zeroinitializer, align 2
@vz = internal unnamed_addr global [101 x i16] zeroinitializer, align 2

; Function Attrs: nofree noinline norecurse nosync nounwind optsize memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none)
define dso_local i64 @bench_particle() local_unnamed_addr #0 {
  br label %1

1:                                                ; preds = %0, %1
  %2 = phi i64 [ 0, %0 ], [ %18, %1 ]
  %3 = add nsw i64 %2, -50
  %4 = getelementptr inbounds nuw [2 x i8], ptr @px, i64 %2
  %5 = trunc nsw i64 %3 to i16
  store i16 %5, ptr %4, align 2, !tbaa !13
  %6 = getelementptr inbounds nuw [2 x i8], ptr @py, i64 %2
  %7 = trunc nsw i64 %3 to i16
  %8 = shl nsw i16 %7, 1
  store i16 %8, ptr %6, align 2, !tbaa !13
  %9 = getelementptr inbounds nuw [2 x i8], ptr @pz, i64 %2
  %10 = trunc i64 %2 to i16
  %11 = sub i16 50, %10
  store i16 %11, ptr %9, align 2, !tbaa !13
  %12 = and i16 %5, 7
  %13 = getelementptr inbounds nuw [2 x i8], ptr @vx, i64 %2
  store i16 %12, ptr %13, align 2, !tbaa !13
  %14 = and i16 %5, 3
  %15 = xor i16 %14, 3
  %16 = getelementptr inbounds nuw [2 x i8], ptr @vy, i64 %2
  store i16 %15, ptr %16, align 2, !tbaa !13
  %17 = getelementptr inbounds nuw [2 x i8], ptr @vz, i64 %2
  store i16 1, ptr %17, align 2, !tbaa !13
  %18 = add nuw nsw i64 %2, 1
  %19 = icmp eq i64 %18, 101
  br i1 %19, label %20, label %1, !llvm.loop !15

20:                                               ; preds = %1, %41
  %21 = phi i16 [ %42, %41 ], [ 1, %1 ]
  br label %22

22:                                               ; preds = %20, %22
  %23 = phi i64 [ %39, %22 ], [ 0, %20 ]
  %24 = getelementptr inbounds nuw [2 x i8], ptr @px, i64 %23
  %25 = load i16, ptr %24, align 2, !tbaa !13
  %26 = getelementptr inbounds nuw [2 x i8], ptr @vx, i64 %23
  %27 = load i16, ptr %26, align 2, !tbaa !13
  %28 = add i16 %27, %25
  store i16 %28, ptr %24, align 2, !tbaa !13
  %29 = getelementptr inbounds nuw [2 x i8], ptr @py, i64 %23
  %30 = load i16, ptr %29, align 2, !tbaa !13
  %31 = getelementptr inbounds nuw [2 x i8], ptr @vy, i64 %23
  %32 = load i16, ptr %31, align 2, !tbaa !13
  %33 = add i16 %32, %30
  store i16 %33, ptr %29, align 2, !tbaa !13
  %34 = getelementptr inbounds nuw [2 x i8], ptr @pz, i64 %23
  %35 = load i16, ptr %34, align 2, !tbaa !13
  %36 = getelementptr inbounds nuw [2 x i8], ptr @vz, i64 %23
  %37 = load i16, ptr %36, align 2, !tbaa !13
  %38 = add i16 %37, %35
  store i16 %38, ptr %34, align 2, !tbaa !13
  %39 = add nuw nsw i64 %23, 1
  %40 = icmp eq i64 %39, 101
  br i1 %40, label %41, label %22, !llvm.loop !17

41:                                               ; preds = %22
  %42 = add nuw nsw i16 %21, 1
  %43 = icmp samesign ult i16 %21, 100
  br i1 %43, label %20, label %44, !llvm.loop !18

44:                                               ; preds = %41, %44
  %45 = phi i64 [ %61, %44 ], [ 0, %41 ]
  %46 = phi i64 [ %60, %44 ], [ 0, %41 ]
  %47 = getelementptr inbounds nuw [2 x i8], ptr @px, i64 %45
  %48 = load i16, ptr %47, align 2, !tbaa !13
  %49 = sext i16 %48 to i64
  %50 = add nsw i64 %46, %49
  %51 = getelementptr inbounds nuw [2 x i8], ptr @py, i64 %45
  %52 = load i16, ptr %51, align 2, !tbaa !13
  %53 = sext i16 %52 to i64
  %54 = mul nsw i64 %53, 3
  %55 = add nsw i64 %50, %54
  %56 = getelementptr inbounds nuw [2 x i8], ptr @pz, i64 %45
  %57 = load i16, ptr %56, align 2, !tbaa !13
  %58 = sext i16 %57 to i64
  %59 = mul nsw i64 %58, 7
  %60 = add nsw i64 %55, %59
  %61 = add nuw nsw i64 %45, 1
  %62 = icmp eq i64 %61, 101
  br i1 %62, label %63, label %44, !llvm.loop !19

63:                                               ; preds = %44
  ret i64 %60

; uselistorder directives
}

; Function Attrs: nounwind optsize
define dso_local noundef i32 @main() local_unnamed_addr #1 {
  %1 = tail call i64 @bench_particle() #3
  tail call void @report(i64 noundef %1) #4
  ret i32 0
}

; Function Attrs: optsize
declare dso_local void @report(i64 noundef) local_unnamed_addr #2

; uselistorder directives

attributes #0 = { nofree noinline norecurse nosync nounwind optsize memory(readwrite, argmem: none, inaccessiblemem: none, target_mem: none) "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #2 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nobuiltin optsize "no-builtins" }
attributes #4 = { nobuiltin nounwind optsize "no-builtins" }

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
!13 = !{!14, !14, i64 0}
!14 = !{!"short", !11, i64 0}
!15 = distinct !{!15, !16}
!16 = !{!"llvm.loop.mustprogress"}
!17 = distinct !{!17, !16}
!18 = distinct !{!18, !16}
!19 = distinct !{!19, !16}
