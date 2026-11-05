; ModuleID = '/home/alim/scratch/w15-bar/out/src/tile.c'
source_filename = "/home/alim/scratch/w15-bar/out/src/tile.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@tile = internal global [64 x [64 x i16]] zeroinitializer, align 2

; Function Attrs: noinline nounwind
define dso_local i64 @bench_tile() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i64, align 8
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i16 40, ptr %1, align 2, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store i16 25, ptr %2, align 2, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i16 7, ptr %3, align 2, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store i16 61, ptr %4, align 2, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store i64 0, ptr %5, align 8, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %6) #4
  call void @llvm.lifetime.start.p0(ptr %7) #4
  store i16 0, ptr %7, align 2, !tbaa !15
  br label %8

8:                                                ; preds = %46, %0
  %9 = load i16, ptr %7, align 2, !tbaa !15
  %10 = sext i16 %9 to i32
  %11 = load i16, ptr %2, align 2, !tbaa !15
  %12 = sext i16 %11 to i32
  %13 = icmp slt i32 %10, %12
  br i1 %13, label %14, label %49

14:                                               ; preds = %8
  store i16 0, ptr %6, align 2, !tbaa !15
  br label %15

15:                                               ; preds = %42, %14
  %16 = load i16, ptr %6, align 2, !tbaa !15
  %17 = sext i16 %16 to i32
  %18 = load i16, ptr %1, align 2, !tbaa !15
  %19 = sext i16 %18 to i32
  %20 = icmp slt i32 %17, %19
  br i1 %20, label %21, label %45

21:                                               ; preds = %15
  %22 = load i16, ptr %7, align 2, !tbaa !15
  %23 = sext i16 %22 to i32
  %24 = load i16, ptr %4, align 2, !tbaa !15
  %25 = sext i16 %24 to i32
  %26 = add nsw i32 %23, %25
  %27 = and i32 %26, 63
  %28 = sext i32 %27 to i64
  %29 = getelementptr inbounds [64 x [64 x i16]], ptr @tile, i64 0, i64 %28
  %30 = load i16, ptr %6, align 2, !tbaa !15
  %31 = sext i16 %30 to i32
  %32 = load i16, ptr %3, align 2, !tbaa !15
  %33 = sext i16 %32 to i32
  %34 = add nsw i32 %31, %33
  %35 = and i32 %34, 63
  %36 = sext i32 %35 to i64
  %37 = getelementptr inbounds [64 x i16], ptr %29, i64 0, i64 %36
  %38 = load i16, ptr %37, align 2, !tbaa !15
  %39 = sext i16 %38 to i64
  %40 = load i64, ptr %5, align 8, !tbaa !17
  %41 = add nsw i64 %40, %39
  store i64 %41, ptr %5, align 8, !tbaa !17
  br label %42

42:                                               ; preds = %21
  %43 = load i16, ptr %6, align 2, !tbaa !15
  %44 = add i16 %43, 1
  store i16 %44, ptr %6, align 2, !tbaa !15
  br label %15, !llvm.loop !19

45:                                               ; preds = %15
  br label %46

46:                                               ; preds = %45
  %47 = load i16, ptr %7, align 2, !tbaa !15
  %48 = add i16 %47, 1
  store i16 %48, ptr %7, align 2, !tbaa !15
  br label %8, !llvm.loop !21

49:                                               ; preds = %8
  %50 = load i64, ptr %5, align 8, !tbaa !17
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i64 %50
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #2 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store i16 0, ptr %2, align 2, !tbaa !15
  br label %3

3:                                                ; preds = %32, %0
  %4 = load i16, ptr %2, align 2, !tbaa !15
  %5 = sext i16 %4 to i32
  %6 = icmp slt i32 %5, 64
  br i1 %6, label %7, label %35

7:                                                ; preds = %3
  store i16 0, ptr %1, align 2, !tbaa !15
  br label %8

8:                                                ; preds = %28, %7
  %9 = load i16, ptr %1, align 2, !tbaa !15
  %10 = sext i16 %9 to i32
  %11 = icmp slt i32 %10, 64
  br i1 %11, label %12, label %31

12:                                               ; preds = %8
  %13 = load i16, ptr %1, align 2, !tbaa !15
  %14 = sext i16 %13 to i32
  %15 = mul nsw i32 %14, 3
  %16 = load i16, ptr %2, align 2, !tbaa !15
  %17 = sext i16 %16 to i32
  %18 = mul nsw i32 %17, 5
  %19 = add nsw i32 %15, %18
  %20 = and i32 %19, 255
  %21 = trunc i32 %20 to i16
  %22 = load i16, ptr %2, align 2, !tbaa !15
  %23 = sext i16 %22 to i64
  %24 = getelementptr inbounds [64 x [64 x i16]], ptr @tile, i64 0, i64 %23
  %25 = load i16, ptr %1, align 2, !tbaa !15
  %26 = sext i16 %25 to i64
  %27 = getelementptr inbounds [64 x i16], ptr %24, i64 0, i64 %26
  store i16 %21, ptr %27, align 2, !tbaa !15
  br label %28

28:                                               ; preds = %12
  %29 = load i16, ptr %1, align 2, !tbaa !15
  %30 = add i16 %29, 1
  store i16 %30, ptr %1, align 2, !tbaa !15
  br label %8, !llvm.loop !22

31:                                               ; preds = %8
  br label %32

32:                                               ; preds = %31
  %33 = load i16, ptr %2, align 2, !tbaa !15
  %34 = add i16 %33, 1
  store i16 %34, ptr %2, align 2, !tbaa !15
  br label %3, !llvm.loop !23

35:                                               ; preds = %3
  %36 = call i64 @bench_tile() #5
  call void @report(i64 noundef %36) #5
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 0
}

declare void @report(i64 noundef) #3

attributes #0 = { noinline nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }

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
!22 = distinct !{!22, !20}
!23 = distinct !{!23, !20}
