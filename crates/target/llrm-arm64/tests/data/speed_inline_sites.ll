; ModuleID = 'inl2.c'
source_filename = "inl2.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@a = external dso_local global [256 x i32], align 4
@b = external dso_local global [256 x i32], align 4

; Function Attrs: nounwind uwtable
define dso_local i32 @f(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i32 0, ptr %3, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %4) #2
  store i32 0, ptr %4, align 4, !tbaa !14
  br label %5

5:                                                ; preds = %28, %1
  %6 = load i32, ptr %4, align 4, !tbaa !14
  %7 = load i32, ptr %2, align 4, !tbaa !14
  %8 = icmp slt i32 %6, %7
  br i1 %8, label %10, label %9

9:                                                ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #2
  br label %31

10:                                               ; preds = %5
  %11 = load i32, ptr %4, align 4, !tbaa !14
  %12 = sext i32 %11 to i64
  %13 = getelementptr inbounds [256 x i32], ptr @a, i64 0, i64 %12
  %14 = load i32, ptr %13, align 4, !tbaa !14
  %15 = load i32, ptr %4, align 4, !tbaa !14
  %16 = call i32 @mix(i32 noundef %14, i32 noundef %15)
  %17 = load i32, ptr %3, align 4, !tbaa !14
  %18 = add nsw i32 %17, %16
  store i32 %18, ptr %3, align 4, !tbaa !14
  %19 = load i32, ptr %3, align 4, !tbaa !14
  %20 = load i32, ptr %4, align 4, !tbaa !14
  %21 = add nsw i32 %20, 1
  %22 = sext i32 %21 to i64
  %23 = getelementptr inbounds [256 x i32], ptr @a, i64 0, i64 %22
  %24 = load i32, ptr %23, align 4, !tbaa !14
  %25 = call i32 @mix(i32 noundef %19, i32 noundef %24)
  %26 = load i32, ptr %3, align 4, !tbaa !14
  %27 = xor i32 %26, %25
  store i32 %27, ptr %3, align 4, !tbaa !14
  br label %28

28:                                               ; preds = %10
  %29 = load i32, ptr %4, align 4, !tbaa !14
  %30 = add nsw i32 %29, 1
  store i32 %30, ptr %4, align 4, !tbaa !14
  br label %5, !llvm.loop !15

31:                                               ; preds = %9
  %32 = load i32, ptr %3, align 4, !tbaa !14
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret i32 %32
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable
define internal i32 @mix(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !14
  store i32 %1, ptr %4, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %5) #2
  %6 = load i32, ptr %3, align 4, !tbaa !14
  %7 = mul nsw i32 %6, 7
  %8 = load i32, ptr %4, align 4, !tbaa !14
  %9 = add nsw i32 %7, %8
  store i32 %9, ptr %5, align 4, !tbaa !14
  %10 = load i32, ptr %5, align 4, !tbaa !14
  %11 = ashr i32 %10, 3
  %12 = load i32, ptr %5, align 4, !tbaa !14
  %13 = xor i32 %12, %11
  store i32 %13, ptr %5, align 4, !tbaa !14
  %14 = load i32, ptr %3, align 4, !tbaa !14
  %15 = shl i32 %14, 2
  %16 = load i32, ptr %5, align 4, !tbaa !14
  %17 = add nsw i32 %16, %15
  store i32 %17, ptr %5, align 4, !tbaa !14
  %18 = load i32, ptr %5, align 4, !tbaa !14
  %19 = mul nsw i32 %18, 13
  %20 = load i32, ptr %4, align 4, !tbaa !14
  %21 = ashr i32 %20, 1
  %22 = add nsw i32 %19, %21
  store i32 %22, ptr %5, align 4, !tbaa !14
  %23 = load i32, ptr %4, align 4, !tbaa !14
  %24 = and i32 %23, 255
  %25 = sext i32 %24 to i64
  %26 = getelementptr inbounds [256 x i32], ptr @b, i64 0, i64 %25
  %27 = load i32, ptr %26, align 4, !tbaa !14
  %28 = load i32, ptr %5, align 4, !tbaa !14
  %29 = xor i32 %28, %27
  store i32 %29, ptr %5, align 4, !tbaa !14
  %30 = load i32, ptr %5, align 4, !tbaa !14
  %31 = ashr i32 %30, 4
  %32 = and i32 %31, 255
  %33 = sext i32 %32 to i64
  %34 = getelementptr inbounds [256 x i32], ptr @a, i64 0, i64 %33
  %35 = load i32, ptr %34, align 4, !tbaa !14
  %36 = load i32, ptr %5, align 4, !tbaa !14
  %37 = add nsw i32 %36, %35
  store i32 %37, ptr %5, align 4, !tbaa !14
  %38 = load i32, ptr %5, align 4, !tbaa !14
  %39 = load i32, ptr %4, align 4, !tbaa !14
  %40 = mul nsw i32 %39, 5
  %41 = sub nsw i32 %38, %40
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i32 %41
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @g(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i32 0, ptr %3, align 4, !tbaa !14
  call void @llvm.lifetime.start.p0(ptr %4) #2
  store i32 0, ptr %4, align 4, !tbaa !14
  br label %5

5:                                                ; preds = %19, %1
  %6 = load i32, ptr %4, align 4, !tbaa !14
  %7 = load i32, ptr %2, align 4, !tbaa !14
  %8 = icmp slt i32 %6, %7
  br i1 %8, label %10, label %9

9:                                                ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #2
  br label %22

10:                                               ; preds = %5
  %11 = load i32, ptr %4, align 4, !tbaa !14
  %12 = sext i32 %11 to i64
  %13 = getelementptr inbounds [256 x i32], ptr @a, i64 0, i64 %12
  %14 = load i32, ptr %13, align 4, !tbaa !14
  %15 = load i32, ptr %4, align 4, !tbaa !14
  %16 = call i32 @mix(i32 noundef %14, i32 noundef %15)
  %17 = load i32, ptr %3, align 4, !tbaa !14
  %18 = add nsw i32 %17, %16
  store i32 %18, ptr %3, align 4, !tbaa !14
  br label %19

19:                                               ; preds = %10
  %20 = load i32, ptr %4, align 4, !tbaa !14
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %4, align 4, !tbaa !14
  br label %5, !llvm.loop !17

22:                                               ; preds = %9
  %23 = load i32, ptr %3, align 4, !tbaa !14
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret i32 %23
}

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 7, !"uwtable", i32 2}
!7 = !{i32 7, !"frame-pointer", i32 4}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = !{!11, !11, i64 0}
!15 = distinct !{!15, !16}
!16 = !{!"llvm.loop.mustprogress"}
!17 = distinct !{!17, !16}
