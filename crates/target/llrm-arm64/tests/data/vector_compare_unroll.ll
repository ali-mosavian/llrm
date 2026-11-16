; ModuleID = 'vector_compare_unroll.c'
source_filename = "vector_compare_unroll.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@a = dso_local global [64 x i16] zeroinitializer, align 2
@b = dso_local global [64 x i64] zeroinitializer, align 8

; Function Attrs: nounwind
define dso_local void @sums() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i64, align 8
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %1, align 2, !tbaa !15
  br label %5

5:                                                ; preds = %38, %0
  %6 = load i16, ptr %1, align 2, !tbaa !15
  %7 = sext i16 %6 to i32
  %8 = icmp slt i32 %7, 6
  br i1 %8, label %9, label %41

9:                                                ; preds = %5
  store i64 0, ptr %4, align 8, !tbaa !17
  store i16 0, ptr %3, align 2, !tbaa !15
  store i16 0, ptr %2, align 2, !tbaa !15
  br label %10

10:                                               ; preds = %31, %9
  %11 = load i16, ptr %2, align 2, !tbaa !15
  %12 = sext i16 %11 to i32
  %13 = icmp slt i32 %12, 64
  br i1 %13, label %14, label %34

14:                                               ; preds = %10
  %15 = load i16, ptr %2, align 2, !tbaa !15
  %16 = sext i16 %15 to i64
  %17 = getelementptr inbounds [64 x i16], ptr @a, i64 0, i64 %16
  %18 = load i16, ptr %17, align 2, !tbaa !15
  %19 = sext i16 %18 to i64
  %20 = load i64, ptr %4, align 8, !tbaa !17
  %21 = add nsw i64 %20, %19
  store i64 %21, ptr %4, align 8, !tbaa !17
  %22 = load i16, ptr %2, align 2, !tbaa !15
  %23 = sext i16 %22 to i64
  %24 = getelementptr inbounds [64 x i64], ptr @b, i64 0, i64 %23
  %25 = load i64, ptr %24, align 8, !tbaa !17
  %26 = icmp ne i64 %25, 0
  br i1 %26, label %27, label %30

27:                                               ; preds = %14
  %28 = load i16, ptr %3, align 2, !tbaa !15
  %29 = add i16 %28, 1
  store i16 %29, ptr %3, align 2, !tbaa !15
  br label %30

30:                                               ; preds = %27, %14
  br label %31

31:                                               ; preds = %30
  %32 = load i16, ptr %2, align 2, !tbaa !15
  %33 = add i16 %32, 1
  store i16 %33, ptr %2, align 2, !tbaa !15
  br label %10, !llvm.loop !19

34:                                               ; preds = %10
  %35 = load i64, ptr %4, align 8, !tbaa !17
  call void @report(i64 noundef %35) #4
  %36 = load i16, ptr %3, align 2, !tbaa !15
  %37 = sext i16 %36 to i64
  call void @report(i64 noundef %37) #4
  br label %38

38:                                               ; preds = %34
  %39 = load i16, ptr %1, align 2, !tbaa !15
  %40 = add i16 %39, 1
  store i16 %40, ptr %1, align 2, !tbaa !15
  br label %5, !llvm.loop !21

41:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare void @report(i64 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin "no-builtins" }

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
!15 = !{!16, !16, i64 0}
!16 = !{!"short", !13, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !13, i64 0}
!19 = distinct !{!19, !20}
!20 = !{!"llvm.loop.mustprogress"}
!21 = distinct !{!21, !20}
