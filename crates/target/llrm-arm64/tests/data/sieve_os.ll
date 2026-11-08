; ModuleID = '/home/alim/scratch/w15-out-halfbase1/src/sieve.c'
source_filename = "/home/alim/scratch/w15-out-halfbase1/src/sieve.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind optsize
define dso_local i64 @bench_sieve(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [1024 x i8], align 1
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i64, align 8
  store i16 %0, ptr %2, align 2, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  call void @llvm.lifetime.start.p0(ptr %6) #4
  store i16 0, ptr %6, align 2, !tbaa !13
  call void @llvm.lifetime.start.p0(ptr %7) #4
  store i64 0, ptr %7, align 8, !tbaa !15
  store i16 0, ptr %4, align 2, !tbaa !13
  br label %8

8:                                                ; preds = %18, %1
  %9 = load i16, ptr %4, align 2, !tbaa !13
  %10 = zext i16 %9 to i32
  %11 = load i16, ptr %2, align 2, !tbaa !13
  %12 = zext i16 %11 to i32
  %13 = icmp slt i32 %10, %12
  br i1 %13, label %14, label %21

14:                                               ; preds = %8
  %15 = load i16, ptr %4, align 2, !tbaa !13
  %16 = zext i16 %15 to i64
  %17 = getelementptr inbounds nuw [1024 x i8], ptr %3, i64 0, i64 %16
  store i8 0, ptr %17, align 1, !tbaa !17
  br label %18

18:                                               ; preds = %14
  %19 = load i16, ptr %4, align 2, !tbaa !13
  %20 = add i16 %19, 1
  store i16 %20, ptr %4, align 2, !tbaa !13
  br label %8, !llvm.loop !18

21:                                               ; preds = %8
  store i16 2, ptr %4, align 2, !tbaa !13
  br label %22

22:                                               ; preds = %71, %21
  %23 = load i16, ptr %4, align 2, !tbaa !13
  %24 = zext i16 %23 to i32
  %25 = load i16, ptr %2, align 2, !tbaa !13
  %26 = zext i16 %25 to i32
  %27 = icmp slt i32 %24, %26
  br i1 %27, label %28, label %74

28:                                               ; preds = %22
  %29 = load i16, ptr %4, align 2, !tbaa !13
  %30 = zext i16 %29 to i64
  %31 = getelementptr inbounds nuw [1024 x i8], ptr %3, i64 0, i64 %30
  %32 = load i8, ptr %31, align 1, !tbaa !17
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %34, label %35

34:                                               ; preds = %28
  br label %71

35:                                               ; preds = %28
  %36 = load i16, ptr %6, align 2, !tbaa !13
  %37 = add i16 %36, 1
  store i16 %37, ptr %6, align 2, !tbaa !13
  %38 = load i16, ptr %4, align 2, !tbaa !13
  %39 = zext i16 %38 to i64
  %40 = load i64, ptr %7, align 8, !tbaa !15
  %41 = add i64 %40, %39
  store i64 %41, ptr %7, align 8, !tbaa !15
  %42 = load i16, ptr %4, align 2, !tbaa !13
  %43 = zext i16 %42 to i32
  %44 = icmp sle i32 %43, 31
  br i1 %44, label %45, label %70

45:                                               ; preds = %35
  %46 = load i16, ptr %4, align 2, !tbaa !13
  %47 = zext i16 %46 to i32
  %48 = load i16, ptr %4, align 2, !tbaa !13
  %49 = zext i16 %48 to i32
  %50 = mul nsw i32 %47, %49
  %51 = trunc i32 %50 to i16
  store i16 %51, ptr %5, align 2, !tbaa !13
  br label %52

52:                                               ; preds = %62, %45
  %53 = load i16, ptr %5, align 2, !tbaa !13
  %54 = zext i16 %53 to i32
  %55 = load i16, ptr %2, align 2, !tbaa !13
  %56 = zext i16 %55 to i32
  %57 = icmp slt i32 %54, %56
  br i1 %57, label %58, label %69

58:                                               ; preds = %52
  %59 = load i16, ptr %5, align 2, !tbaa !13
  %60 = zext i16 %59 to i64
  %61 = getelementptr inbounds nuw [1024 x i8], ptr %3, i64 0, i64 %60
  store i8 1, ptr %61, align 1, !tbaa !17
  br label %62

62:                                               ; preds = %58
  %63 = load i16, ptr %5, align 2, !tbaa !13
  %64 = zext i16 %63 to i32
  %65 = load i16, ptr %4, align 2, !tbaa !13
  %66 = zext i16 %65 to i32
  %67 = add nsw i32 %64, %66
  %68 = trunc i32 %67 to i16
  store i16 %68, ptr %5, align 2, !tbaa !13
  br label %52, !llvm.loop !20

69:                                               ; preds = %52
  br label %70

70:                                               ; preds = %69, %35
  br label %71

71:                                               ; preds = %70, %34
  %72 = load i16, ptr %4, align 2, !tbaa !13
  %73 = add i16 %72, 1
  store i16 %73, ptr %4, align 2, !tbaa !13
  br label %22, !llvm.loop !21

74:                                               ; preds = %22
  %75 = load i16, ptr %6, align 2, !tbaa !13
  %76 = zext i16 %75 to i64
  %77 = shl i64 %76, 16
  %78 = load i64, ptr %7, align 8, !tbaa !15
  %79 = xor i64 %77, %78
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret i64 %79
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i32 @main() #2 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !22
  %2 = add nsw i32 1024, %1
  %3 = trunc i32 %2 to i16
  %4 = call i64 @bench_sieve(i16 noundef %3) #5
  call void @report(i64 noundef %4) #5
  ret i32 0
}

; Function Attrs: optsize
declare dso_local void @report(i64 noundef) #3

attributes #0 = { noinline nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin optsize "no-builtins" }

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
!15 = !{!16, !16, i64 0}
!16 = !{!"long", !11, i64 0}
!17 = !{!11, !11, i64 0}
!18 = distinct !{!18, !19}
!19 = !{!"llvm.loop.mustprogress"}
!20 = distinct !{!20, !19}
!21 = distinct !{!21, !19}
!22 = !{!10, !10, i64 0}
