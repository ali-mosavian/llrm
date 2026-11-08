; ModuleID = 'nest.c'
source_filename = "nest.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i64 @f(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca [8 x [8 x i16]], align 2
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %3) #2
  call void @llvm.lifetime.start.p0(ptr %4) #2
  store i64 0, ptr %4, align 8, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %5) #2
  call void @llvm.lifetime.start.p0(ptr %6) #2
  call void @llvm.lifetime.start.p0(ptr %7) #2
  store i32 0, ptr %5, align 4, !tbaa !16
  br label %8

8:                                                ; preds = %37, %1
  %9 = load i32, ptr %5, align 4, !tbaa !16
  %10 = icmp slt i32 %9, 8
  br i1 %10, label %11, label %40

11:                                               ; preds = %8
  store i32 0, ptr %6, align 4, !tbaa !16
  br label %12

12:                                               ; preds = %33, %11
  %13 = load i32, ptr %6, align 4, !tbaa !16
  %14 = icmp slt i32 %13, 8
  br i1 %14, label %15, label %36

15:                                               ; preds = %12
  %16 = load i32, ptr %5, align 4, !tbaa !16
  %17 = add nsw i32 %16, 1
  %18 = load i32, ptr %6, align 4, !tbaa !16
  %19 = add nsw i32 %18, 2
  %20 = mul nsw i32 %17, %19
  %21 = load i32, ptr %5, align 4, !tbaa !16
  %22 = load i32, ptr %6, align 4, !tbaa !16
  %23 = xor i32 %21, %22
  %24 = add nsw i32 %20, %23
  %25 = srem i32 %24, 7
  %26 = trunc i32 %25 to i16
  %27 = load i32, ptr %5, align 4, !tbaa !16
  %28 = sext i32 %27 to i64
  %29 = getelementptr inbounds [8 x [8 x i16]], ptr %3, i64 0, i64 %28
  %30 = load i32, ptr %6, align 4, !tbaa !16
  %31 = sext i32 %30 to i64
  %32 = getelementptr inbounds [8 x i16], ptr %29, i64 0, i64 %31
  store i16 %26, ptr %32, align 2, !tbaa !19
  br label %33

33:                                               ; preds = %15
  %34 = load i32, ptr %6, align 4, !tbaa !16
  %35 = add nsw i32 %34, 1
  store i32 %35, ptr %6, align 4, !tbaa !16
  br label %12, !llvm.loop !21

36:                                               ; preds = %12
  br label %37

37:                                               ; preds = %36
  %38 = load i32, ptr %5, align 4, !tbaa !16
  %39 = add nsw i32 %38, 1
  store i32 %39, ptr %5, align 4, !tbaa !16
  br label %8, !llvm.loop !23

40:                                               ; preds = %8
  store i32 0, ptr %7, align 4, !tbaa !16
  br label %41

41:                                               ; preds = %59, %40
  %42 = load i32, ptr %7, align 4, !tbaa !16
  %43 = load i32, ptr %2, align 4, !tbaa !16
  %44 = icmp slt i32 %42, %43
  br i1 %44, label %45, label %62

45:                                               ; preds = %41
  %46 = load i32, ptr %7, align 4, !tbaa !16
  %47 = srem i32 %46, 8
  %48 = sext i32 %47 to i64
  %49 = getelementptr inbounds [8 x [8 x i16]], ptr %3, i64 0, i64 %48
  %50 = load i32, ptr %7, align 4, !tbaa !16
  %51 = sdiv i32 %50, 8
  %52 = srem i32 %51, 8
  %53 = sext i32 %52 to i64
  %54 = getelementptr inbounds [8 x i16], ptr %49, i64 0, i64 %53
  %55 = load i16, ptr %54, align 2, !tbaa !19
  %56 = sext i16 %55 to i64
  %57 = load i64, ptr %4, align 8, !tbaa !17
  %58 = add nsw i64 %57, %56
  store i64 %58, ptr %4, align 8, !tbaa !17
  br label %59

59:                                               ; preds = %45
  %60 = load i32, ptr %7, align 4, !tbaa !16
  %61 = add nsw i32 %60, 1
  store i32 %61, ptr %7, align 4, !tbaa !16
  br label %41, !llvm.loop !24

62:                                               ; preds = %41
  %63 = load i64, ptr %4, align 8, !tbaa !17
  call void @llvm.lifetime.end.p0(ptr %7) #2
  call void @llvm.lifetime.end.p0(ptr %6) #2
  call void @llvm.lifetime.end.p0(ptr %5) #2
  call void @llvm.lifetime.end.p0(ptr %4) #2
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret i64 %63
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}
!llvm.errno.tbaa = !{!11}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"frame-pointer", i32 4}
!10 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!11 = !{!12, !13, i64 0}
!12 = !{!"__libc_errno", !13, i64 0}
!13 = !{!"int", !14, i64 0}
!14 = !{!"omnipotent char", !15, i64 0}
!15 = !{!"Simple C/C++ TBAA"}
!16 = !{!13, !13, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !14, i64 0}
!19 = !{!20, !20, i64 0}
!20 = !{!"short", !14, i64 0}
!21 = distinct !{!21, !22}
!22 = !{!"llvm.loop.mustprogress"}
!23 = distinct !{!23, !22}
!24 = distinct !{!24, !22}
