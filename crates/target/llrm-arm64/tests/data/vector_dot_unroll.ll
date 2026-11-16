; ModuleID = 'bench_dot.c'
source_filename = "bench_dot.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: noinline nounwind
define dso_local i64 @bench_dot() #0 {
  %1 = alloca [64 x i16], align 2
  %2 = alloca [48 x i16], align 2
  %3 = alloca i64, align 8
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i64 0, ptr %3, align 8, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store i16 0, ptr %4, align 2, !tbaa !16
  br label %6

6:                                                ; preds = %19, %0
  %7 = load i16, ptr %4, align 2, !tbaa !16
  %8 = sext i16 %7 to i32
  %9 = icmp slt i32 %8, 64
  br i1 %9, label %10, label %22

10:                                               ; preds = %6
  %11 = load i16, ptr %4, align 2, !tbaa !16
  %12 = sext i16 %11 to i32
  %13 = mul nsw i32 %12, 3
  %14 = sub nsw i32 %13, 90
  %15 = trunc i32 %14 to i16
  %16 = load i16, ptr %4, align 2, !tbaa !16
  %17 = sext i16 %16 to i64
  %18 = getelementptr inbounds [64 x i16], ptr %1, i64 0, i64 %17
  store i16 %15, ptr %18, align 2, !tbaa !16
  br label %19

19:                                               ; preds = %10
  %20 = load i16, ptr %4, align 2, !tbaa !16
  %21 = add i16 %20, 1
  store i16 %21, ptr %4, align 2, !tbaa !16
  br label %6, !llvm.loop !18

22:                                               ; preds = %6
  store i16 0, ptr %4, align 2, !tbaa !16
  br label %23

23:                                               ; preds = %36, %22
  %24 = load i16, ptr %4, align 2, !tbaa !16
  %25 = sext i16 %24 to i32
  %26 = icmp slt i32 %25, 48
  br i1 %26, label %27, label %39

27:                                               ; preds = %23
  %28 = load i16, ptr %4, align 2, !tbaa !16
  %29 = sext i16 %28 to i32
  %30 = mul nsw i32 %29, 2
  %31 = sub nsw i32 70, %30
  %32 = trunc i32 %31 to i16
  %33 = load i16, ptr %4, align 2, !tbaa !16
  %34 = sext i16 %33 to i64
  %35 = getelementptr inbounds [48 x i16], ptr %2, i64 0, i64 %34
  store i16 %32, ptr %35, align 2, !tbaa !16
  br label %36

36:                                               ; preds = %27
  %37 = load i16, ptr %4, align 2, !tbaa !16
  %38 = add i16 %37, 1
  store i16 %38, ptr %4, align 2, !tbaa !16
  br label %23, !llvm.loop !20

39:                                               ; preds = %23
  store i16 0, ptr %5, align 2, !tbaa !16
  br label %40

40:                                               ; preds = %52, %39
  %41 = load i16, ptr %5, align 2, !tbaa !16
  %42 = sext i16 %41 to i32
  %43 = icmp slt i32 %42, 40
  br i1 %43, label %44, label %55

44:                                               ; preds = %40
  %45 = load i16, ptr %5, align 2, !tbaa !16
  %46 = getelementptr inbounds [64 x i16], ptr %1, i64 0, i64 0
  store i16 %45, ptr %46, align 2, !tbaa !16
  %47 = getelementptr inbounds [64 x i16], ptr %1, i64 0, i64 0
  %48 = getelementptr inbounds [48 x i16], ptr %2, i64 0, i64 0
  %49 = call i64 @dot(ptr noundef %47, i16 noundef 64, ptr noundef %48, i16 noundef 48) #5
  %50 = load i64, ptr %3, align 8, !tbaa !14
  %51 = add nsw i64 %50, %49
  store i64 %51, ptr %3, align 8, !tbaa !14
  br label %52

52:                                               ; preds = %44
  %53 = load i16, ptr %5, align 2, !tbaa !16
  %54 = add i16 %53, 1
  store i16 %54, ptr %5, align 2, !tbaa !16
  br label %40, !llvm.loop !21

55:                                               ; preds = %40
  %56 = load i64, ptr %3, align 8, !tbaa !14
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i64 %56
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal i64 @dot(ptr noundef %0, i16 noundef %1, ptr noundef %2, i16 noundef %3) #2 {
  %5 = alloca ptr, align 8
  %6 = alloca i16, align 2
  %7 = alloca ptr, align 8
  %8 = alloca i16, align 2
  %9 = alloca i64, align 8
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  store ptr %0, ptr %5, align 8, !tbaa !22
  store i16 %1, ptr %6, align 2, !tbaa !16
  store ptr %2, ptr %7, align 8, !tbaa !22
  store i16 %3, ptr %8, align 2, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %9) #4
  store i64 0, ptr %9, align 8, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %10) #4
  %12 = load i16, ptr %6, align 2, !tbaa !16
  %13 = sext i16 %12 to i32
  %14 = load i16, ptr %8, align 2, !tbaa !16
  %15 = sext i16 %14 to i32
  %16 = icmp slt i32 %13, %15
  br i1 %16, label %17, label %20

17:                                               ; preds = %4
  %18 = load i16, ptr %6, align 2, !tbaa !16
  %19 = sext i16 %18 to i32
  br label %23

20:                                               ; preds = %4
  %21 = load i16, ptr %8, align 2, !tbaa !16
  %22 = sext i16 %21 to i32
  br label %23

23:                                               ; preds = %20, %17
  %24 = phi i32 [ %19, %17 ], [ %22, %20 ]
  %25 = trunc i32 %24 to i16
  store i16 %25, ptr %10, align 2, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %11) #4
  store i16 0, ptr %11, align 2, !tbaa !16
  br label %26

26:                                               ; preds = %48, %23
  %27 = load i16, ptr %11, align 2, !tbaa !16
  %28 = sext i16 %27 to i32
  %29 = load i16, ptr %10, align 2, !tbaa !16
  %30 = sext i16 %29 to i32
  %31 = icmp slt i32 %28, %30
  br i1 %31, label %32, label %51

32:                                               ; preds = %26
  %33 = load ptr, ptr %5, align 8, !tbaa !22
  %34 = load i16, ptr %11, align 2, !tbaa !16
  %35 = sext i16 %34 to i64
  %36 = getelementptr inbounds i16, ptr %33, i64 %35
  %37 = load i16, ptr %36, align 2, !tbaa !16
  %38 = sext i16 %37 to i64
  %39 = load ptr, ptr %7, align 8, !tbaa !22
  %40 = load i16, ptr %11, align 2, !tbaa !16
  %41 = sext i16 %40 to i64
  %42 = getelementptr inbounds i16, ptr %39, i64 %41
  %43 = load i16, ptr %42, align 2, !tbaa !16
  %44 = sext i16 %43 to i64
  %45 = mul nsw i64 %38, %44
  %46 = load i64, ptr %9, align 8, !tbaa !14
  %47 = add nsw i64 %46, %45
  store i64 %47, ptr %9, align 8, !tbaa !14
  br label %48

48:                                               ; preds = %32
  %49 = load i16, ptr %11, align 2, !tbaa !16
  %50 = add i16 %49, 1
  store i16 %50, ptr %11, align 2, !tbaa !16
  br label %26, !llvm.loop !25

51:                                               ; preds = %26
  %52 = load i64, ptr %9, align 8, !tbaa !14
  call void @llvm.lifetime.end.p0(ptr %11) #4
  call void @llvm.lifetime.end.p0(ptr %10) #4
  call void @llvm.lifetime.end.p0(ptr %9) #4
  ret i64 %52
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #2 {
  %1 = call i64 @bench_dot() #5
  call void @report(i64 noundef %1) #5
  ret i32 0
}

declare void @report(i64 noundef) #3

attributes #0 = { noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = !{!15, !15, i64 0}
!15 = !{!"long", !12, i64 0}
!16 = !{!17, !17, i64 0}
!17 = !{!"short", !12, i64 0}
!18 = distinct !{!18, !19}
!19 = !{!"llvm.loop.mustprogress"}
!20 = distinct !{!20, !19}
!21 = distinct !{!21, !19}
!22 = !{!23, !23, i64 0}
!23 = !{!"p1 short", !24, i64 0}
!24 = !{!"any pointer", !12, i64 0}
!25 = distinct !{!25, !19}
