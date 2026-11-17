target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@px = internal global [101 x i16] zeroinitializer, align 2
@py = internal global [101 x i16] zeroinitializer, align 2
@pz = internal global [101 x i16] zeroinitializer, align 2
@vx = internal global [101 x i16] zeroinitializer, align 2
@vy = internal global [101 x i16] zeroinitializer, align 2
@vz = internal global [101 x i16] zeroinitializer, align 2

define i64 @bench_particle() nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i64, align 8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  store i16 0, ptr %1, align 2, !tbaa !15
  br label %5

5:
  %6 = load i16, ptr %1, align 2, !tbaa !15
  %7 = sext i16 %6 to i32
  %8 = icmp sle i32 %7, 100
  br i1 %8, label %9, label %53

9:
  %10 = load i16, ptr %1, align 2, !tbaa !15
  %11 = sext i16 %10 to i32
  %12 = sub nsw i32 %11, 50
  %13 = trunc i32 %12 to i16
  store i16 %13, ptr %3, align 2, !tbaa !15
  %14 = load i16, ptr %3, align 2, !tbaa !15
  %15 = load i16, ptr %1, align 2, !tbaa !15
  %16 = sext i16 %15 to i64
  %17 = getelementptr inbounds [101 x i16], ptr @px, i64 0, i64 %16
  store i16 %14, ptr %17, align 2, !tbaa !15
  %18 = load i16, ptr %3, align 2, !tbaa !15
  %19 = sext i16 %18 to i32
  %20 = mul nsw i32 2, %19
  %21 = trunc i32 %20 to i16
  %22 = load i16, ptr %1, align 2, !tbaa !15
  %23 = sext i16 %22 to i64
  %24 = getelementptr inbounds [101 x i16], ptr @py, i64 0, i64 %23
  store i16 %21, ptr %24, align 2, !tbaa !15
  %25 = load i16, ptr %3, align 2, !tbaa !15
  %26 = sext i16 %25 to i32
  %27 = sub nsw i32 0, %26
  %28 = trunc i32 %27 to i16
  %29 = load i16, ptr %1, align 2, !tbaa !15
  %30 = sext i16 %29 to i64
  %31 = getelementptr inbounds [101 x i16], ptr @pz, i64 0, i64 %30
  store i16 %28, ptr %31, align 2, !tbaa !15
  %32 = load i16, ptr %3, align 2, !tbaa !15
  %33 = sext i16 %32 to i32
  %34 = and i32 %33, 7
  %35 = trunc i32 %34 to i16
  %36 = load i16, ptr %1, align 2, !tbaa !15
  %37 = sext i16 %36 to i64
  %38 = getelementptr inbounds [101 x i16], ptr @vx, i64 0, i64 %37
  store i16 %35, ptr %38, align 2, !tbaa !15
  %39 = load i16, ptr %3, align 2, !tbaa !15
  %40 = sext i16 %39 to i32
  %41 = and i32 %40, 3
  %42 = sub nsw i32 3, %41
  %43 = trunc i32 %42 to i16
  %44 = load i16, ptr %1, align 2, !tbaa !15
  %45 = sext i16 %44 to i64
  %46 = getelementptr inbounds [101 x i16], ptr @vy, i64 0, i64 %45
  store i16 %43, ptr %46, align 2, !tbaa !15
  %47 = load i16, ptr %1, align 2, !tbaa !15
  %48 = sext i16 %47 to i64
  %49 = getelementptr inbounds [101 x i16], ptr @vz, i64 0, i64 %48
  store i16 1, ptr %49, align 2, !tbaa !15
  br label %50

50:
  %51 = load i16, ptr %1, align 2, !tbaa !15
  %52 = add i16 %51, 1
  store i16 %52, ptr %1, align 2, !tbaa !15
  br label %5, !llvm.loop !17

53:
  store i16 1, ptr %2, align 2, !tbaa !15
  br label %54

54:
  %55 = load i16, ptr %2, align 2, !tbaa !15
  %56 = sext i16 %55 to i32
  %57 = icmp sle i32 %56, 100
  br i1 %57, label %58, label %62

58:
  call void @advance(i16 noundef 50) nobuiltin "no-builtins"
  br label %59

59:
  %60 = load i16, ptr %2, align 2, !tbaa !15
  %61 = add i16 %60, 1
  store i16 %61, ptr %2, align 2, !tbaa !15
  br label %54, !llvm.loop !19

62:
  store i64 0, ptr %4, align 8, !tbaa !20
  store i16 0, ptr %1, align 2, !tbaa !15
  br label %63

63:
  %64 = load i16, ptr %1, align 2, !tbaa !15
  %65 = sext i16 %64 to i32
  %66 = icmp sle i32 %65, 100
  br i1 %66, label %67, label %92

67:
  %68 = load i64, ptr %4, align 8, !tbaa !20
  %69 = load i16, ptr %1, align 2, !tbaa !15
  %70 = sext i16 %69 to i64
  %71 = getelementptr inbounds [101 x i16], ptr @px, i64 0, i64 %70
  %72 = load i16, ptr %71, align 2, !tbaa !15
  %73 = sext i16 %72 to i64
  %74 = add nsw i64 %68, %73
  %75 = load i16, ptr %1, align 2, !tbaa !15
  %76 = sext i16 %75 to i64
  %77 = getelementptr inbounds [101 x i16], ptr @py, i64 0, i64 %76
  %78 = load i16, ptr %77, align 2, !tbaa !15
  %79 = sext i16 %78 to i64
  %80 = mul nsw i64 3, %79
  %81 = add nsw i64 %74, %80
  %82 = load i16, ptr %1, align 2, !tbaa !15
  %83 = sext i16 %82 to i64
  %84 = getelementptr inbounds [101 x i16], ptr @pz, i64 0, i64 %83
  %85 = load i16, ptr %84, align 2, !tbaa !15
  %86 = sext i16 %85 to i64
  %87 = mul nsw i64 7, %86
  %88 = add nsw i64 %81, %87
  store i64 %88, ptr %4, align 8, !tbaa !20
  br label %89

89:
  %90 = load i16, ptr %1, align 2, !tbaa !15
  %91 = add i16 %90, 1
  store i16 %91, ptr %1, align 2, !tbaa !15
  br label %63, !llvm.loop !22

92:
  %93 = load i64, ptr %4, align 8, !tbaa !20
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i64 %93
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal void @advance(i16 noundef %0) nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  %4 = load i16, ptr %2, align 2, !tbaa !15
  %5 = sext i16 %4 to i32
  %6 = sub nsw i32 50, %5
  %7 = trunc i32 %6 to i16
  store i16 %7, ptr %3, align 2, !tbaa !15
  br label %8

8:
  %9 = load i16, ptr %3, align 2, !tbaa !15
  %10 = sext i16 %9 to i32
  %11 = load i16, ptr %2, align 2, !tbaa !15
  %12 = sext i16 %11 to i32
  %13 = add nsw i32 50, %12
  %14 = icmp sle i32 %10, %13
  br i1 %14, label %15, label %64

15:
  %16 = load i16, ptr %3, align 2, !tbaa !15
  %17 = sext i16 %16 to i64
  %18 = getelementptr inbounds [101 x i16], ptr @px, i64 0, i64 %17
  %19 = load i16, ptr %18, align 2, !tbaa !15
  %20 = sext i16 %19 to i32
  %21 = load i16, ptr %3, align 2, !tbaa !15
  %22 = sext i16 %21 to i64
  %23 = getelementptr inbounds [101 x i16], ptr @vx, i64 0, i64 %22
  %24 = load i16, ptr %23, align 2, !tbaa !15
  %25 = sext i16 %24 to i32
  %26 = add nsw i32 %20, %25
  %27 = trunc i32 %26 to i16
  %28 = load i16, ptr %3, align 2, !tbaa !15
  %29 = sext i16 %28 to i64
  %30 = getelementptr inbounds [101 x i16], ptr @px, i64 0, i64 %29
  store i16 %27, ptr %30, align 2, !tbaa !15
  %31 = load i16, ptr %3, align 2, !tbaa !15
  %32 = sext i16 %31 to i64
  %33 = getelementptr inbounds [101 x i16], ptr @py, i64 0, i64 %32
  %34 = load i16, ptr %33, align 2, !tbaa !15
  %35 = sext i16 %34 to i32
  %36 = load i16, ptr %3, align 2, !tbaa !15
  %37 = sext i16 %36 to i64
  %38 = getelementptr inbounds [101 x i16], ptr @vy, i64 0, i64 %37
  %39 = load i16, ptr %38, align 2, !tbaa !15
  %40 = sext i16 %39 to i32
  %41 = add nsw i32 %35, %40
  %42 = trunc i32 %41 to i16
  %43 = load i16, ptr %3, align 2, !tbaa !15
  %44 = sext i16 %43 to i64
  %45 = getelementptr inbounds [101 x i16], ptr @py, i64 0, i64 %44
  store i16 %42, ptr %45, align 2, !tbaa !15
  %46 = load i16, ptr %3, align 2, !tbaa !15
  %47 = sext i16 %46 to i64
  %48 = getelementptr inbounds [101 x i16], ptr @pz, i64 0, i64 %47
  %49 = load i16, ptr %48, align 2, !tbaa !15
  %50 = sext i16 %49 to i32
  %51 = load i16, ptr %3, align 2, !tbaa !15
  %52 = sext i16 %51 to i64
  %53 = getelementptr inbounds [101 x i16], ptr @vz, i64 0, i64 %52
  %54 = load i16, ptr %53, align 2, !tbaa !15
  %55 = sext i16 %54 to i32
  %56 = add nsw i32 %50, %55
  %57 = trunc i32 %56 to i16
  %58 = load i16, ptr %3, align 2, !tbaa !15
  %59 = sext i16 %58 to i64
  %60 = getelementptr inbounds [101 x i16], ptr @pz, i64 0, i64 %59
  store i16 %57, ptr %60, align 2, !tbaa !15
  br label %61

61:
  %62 = load i16, ptr %3, align 2, !tbaa !15
  %63 = add i16 %62, 1
  store i16 %63, ptr %3, align 2, !tbaa !15
  br label %8, !llvm.loop !23

64:
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  ret void
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = call i64 @bench_particle() nobuiltin "no-builtins"
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
!16 = !{!"short", !13, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
!19 = distinct !{!19, !18}
!20 = !{!21, !21, i64 0}
!21 = !{!"long", !13, i64 0}
!22 = distinct !{!22, !18}
!23 = distinct !{!23, !18}
