target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@g = global [16 x [54 x i8]] zeroinitializer, align 1

define i64 @bench() nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i64, align 8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  store i64 0, ptr %3, align 8, !tbaa !15
  store i16 0, ptr %1, align 2, !tbaa !17
  br label %4

4:
  %5 = load i16, ptr %1, align 2, !tbaa !17
  %6 = sext i16 %5 to i32
  %7 = icmp slt i32 %6, 16
  br i1 %7, label %8, label %27

8:
  store i16 0, ptr %2, align 2, !tbaa !17
  br label %9

9:
  %10 = load i16, ptr %2, align 2, !tbaa !17
  %11 = sext i16 %10 to i32
  %12 = icmp slt i32 %11, 54
  br i1 %12, label %13, label %23

13:
  %14 = load i16, ptr %1, align 2, !tbaa !17
  %15 = sext i16 %14 to i64
  %16 = getelementptr inbounds [16 x [54 x i8]], ptr @g, i64 0, i64 %15
  %17 = load i16, ptr %2, align 2, !tbaa !17
  %18 = sext i16 %17 to i64
  %19 = getelementptr inbounds [54 x i8], ptr %16, i64 0, i64 %18
  store i8 1, ptr %19, align 1, !tbaa !19
  br label %20

20:
  %21 = load i16, ptr %2, align 2, !tbaa !17
  %22 = add i16 %21, 1
  store i16 %22, ptr %2, align 2, !tbaa !17
  br label %9, !llvm.loop !20

23:
  br label %24

24:
  %25 = load i16, ptr %1, align 2, !tbaa !17
  %26 = add i16 %25, 1
  store i16 %26, ptr %1, align 2, !tbaa !17
  br label %4, !llvm.loop !22

27:
  store i16 0, ptr %1, align 2, !tbaa !17
  br label %28

28:
  %29 = load i16, ptr %1, align 2, !tbaa !17
  %30 = sext i16 %29 to i32
  %31 = icmp slt i32 %30, 16
  br i1 %31, label %32, label %51

32:
  store i16 0, ptr %2, align 2, !tbaa !17
  br label %33

33:
  %34 = load i16, ptr %2, align 2, !tbaa !17
  %35 = sext i16 %34 to i32
  %36 = icmp slt i32 %35, 16
  br i1 %36, label %37, label %47

37:
  %38 = load i16, ptr %1, align 2, !tbaa !17
  %39 = sext i16 %38 to i64
  %40 = getelementptr inbounds [16 x [54 x i8]], ptr @g, i64 0, i64 %39
  %41 = load i16, ptr %2, align 2, !tbaa !17
  %42 = sext i16 %41 to i64
  %43 = getelementptr inbounds [54 x i8], ptr %40, i64 0, i64 %42
  store i8 0, ptr %43, align 1, !tbaa !19
  br label %44

44:
  %45 = load i16, ptr %2, align 2, !tbaa !17
  %46 = add i16 %45, 1
  store i16 %46, ptr %2, align 2, !tbaa !17
  br label %33, !llvm.loop !23

47:
  br label %48

48:
  %49 = load i16, ptr %1, align 2, !tbaa !17
  %50 = add i16 %49, 1
  store i16 %50, ptr %1, align 2, !tbaa !17
  br label %28, !llvm.loop !24

51:
  store i16 0, ptr %1, align 2, !tbaa !17
  br label %52

52:
  %53 = load i16, ptr %1, align 2, !tbaa !17
  %54 = sext i16 %53 to i32
  %55 = icmp slt i32 %54, 16
  br i1 %55, label %56, label %79

56:
  store i16 0, ptr %2, align 2, !tbaa !17
  br label %57

57:
  %58 = load i16, ptr %2, align 2, !tbaa !17
  %59 = sext i16 %58 to i32
  %60 = icmp slt i32 %59, 54
  br i1 %60, label %61, label %75

61:
  %62 = load i16, ptr %1, align 2, !tbaa !17
  %63 = sext i16 %62 to i64
  %64 = getelementptr inbounds [16 x [54 x i8]], ptr @g, i64 0, i64 %63
  %65 = load i16, ptr %2, align 2, !tbaa !17
  %66 = sext i16 %65 to i64
  %67 = getelementptr inbounds [54 x i8], ptr %64, i64 0, i64 %66
  %68 = load i8, ptr %67, align 1, !tbaa !19
  %69 = zext i8 %68 to i64
  %70 = load i64, ptr %3, align 8, !tbaa !15
  %71 = add nsw i64 %70, %69
  store i64 %71, ptr %3, align 8, !tbaa !15
  br label %72

72:
  %73 = load i16, ptr %2, align 2, !tbaa !17
  %74 = add i16 %73, 1
  store i16 %74, ptr %2, align 2, !tbaa !17
  br label %57, !llvm.loop !25

75:
  br label %76

76:
  %77 = load i16, ptr %1, align 2, !tbaa !17
  %78 = add i16 %77, 1
  store i16 %78, ptr %1, align 2, !tbaa !17
  br label %52, !llvm.loop !26

79:
  %80 = load i64, ptr %3, align 8, !tbaa !15
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i64 %80
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = call i64 @bench() nobuiltin "no-builtins"
  call void @report(i64 noundef %1) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i64 noundef) "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

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
!16 = !{!"long", !13, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"short", !13, i64 0}
!19 = !{!13, !13, i64 0}
!20 = distinct !{!20, !21}
!21 = !{!"llvm.loop.mustprogress"}
!22 = distinct !{!22, !21}
!23 = distinct !{!23, !21}
!24 = distinct !{!24, !21}
!25 = distinct !{!25, !21}
!26 = distinct !{!26, !21}
