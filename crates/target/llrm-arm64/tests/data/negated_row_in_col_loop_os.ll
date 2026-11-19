target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define dso_local i32 @place(ptr noundef nonnull nocapture nofree %0, i32 noundef %1, i32 noundef %2) nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" nofree nosync memory(argmem: readwrite) {
  %4 = icmp eq i32 %1, %2
  br i1 %4, label %41, label %5

5:
  %6 = icmp sgt i32 %2, 0
  br i1 %6, label %7, label %41

7:
  %8 = zext i32 %1 to i64
  %9 = sext i32 %1 to i64
  %10 = add nsw i32 %1, 1
  %11 = sub i32 0, %1
  br label %12

12:
  %13 = phi i32 [ 0, %7 ], [ %39, %38 ]
  %14 = phi i32 [ 0, %7 ], [ %40, %38 ]
  %15 = icmp sgt i32 %1, 0
  br i1 %15, label %16, label %33

16:
  br label %21

17:
  %18 = add i32 %24, 1
  %19 = add nsw i64 %23, -1
  %20 = getelementptr i8, ptr %22, i64 4
  %exitcond = icmp eq i64 %19, 0
  br i1 %exitcond, label %33, label %21, !llvm.loop !21

21:
  %22 = phi ptr [ %0, %16 ], [ %20, %17 ]
  %23 = phi i64 [ %8, %16 ], [ %19, %17 ]
  %24 = phi i32 [ %11, %16 ], [ %18, %17 ]
  %25 = load i32, ptr %22, align 4, !tbaa !15
  %26 = sub nsw i32 %25, %14
  %27 = icmp eq i32 %25, %14
  %28 = zext i32 %26 to i64
  %29 = icmp eq i64 %23, %28
  %30 = icmp eq i32 %24, %26
  %31 = or i1 %30, %29
  %32 = select i1 %27, i1 true, i1 %31
  br i1 %32, label %38, label %17

33:
  %34 = mul i64 %9, 4
  %35 = getelementptr inbounds i8, ptr %0, i64 %34
  store i32 %14, ptr %35, align 4, !tbaa !15
  %36 = tail call fastcc i32 @place(ptr noundef %0, i32 noundef %10, i32 noundef %2) nobuiltin optsize "no-builtins"
  %37 = add nsw i32 %36, %13
  br label %38

38:
  %39 = phi i32 [ %37, %33 ], [ %13, %21 ]
  %40 = add nuw nsw i32 %14, 1
  %exitcond1 = icmp eq i32 %40, %2
  br i1 %exitcond1, label %41, label %12, !llvm.loop !19

41:
  %42 = phi i32 [ 1, %3 ], [ 0, %5 ], [ %39, %38 ]
  ret i32 %42
}

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
!16 = !{!17, !17, i64 0}
!17 = !{!"p1 int", !18, i64 0}
!18 = !{!"any pointer", !13, i64 0}
!19 = distinct !{!19, !20}
!20 = !{!"llvm.loop.mustprogress"}
!21 = distinct !{!21, !20}
