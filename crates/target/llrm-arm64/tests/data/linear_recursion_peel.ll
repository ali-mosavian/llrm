; ModuleID = '/tmp/claude-1000/lrp.c'
source_filename = "/tmp/claude-1000/lrp.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i64 @bench_recpow(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i64, align 8
  %4 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i64 0, ptr %3, align 8, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %4) #2
  store i32 1, ptr %4, align 4, !tbaa !15
  br label %5

5:                                                ; preds = %20, %1
  %6 = load i32, ptr %4, align 4, !tbaa !15
  %7 = load i32, ptr %2, align 4, !tbaa !15
  %8 = icmp sle i32 %6, %7
  br i1 %8, label %9, label %23

9:                                                ; preds = %5
  %10 = load i32, ptr %4, align 4, !tbaa !15
  %11 = srem i32 %10, 97
  %12 = add nsw i32 %11, 2
  %13 = sext i32 %12 to i64
  %14 = load i32, ptr %4, align 4, !tbaa !15
  %15 = add nsw i32 %14, 40
  %16 = call i64 @power(i64 noundef %13, i32 noundef %15) #3
  %17 = srem i64 %16, 1000
  %18 = load i64, ptr %3, align 8, !tbaa !16
  %19 = add nsw i64 %18, %17
  store i64 %19, ptr %3, align 8, !tbaa !16
  br label %20

20:                                               ; preds = %9
  %21 = load i32, ptr %4, align 4, !tbaa !15
  %22 = add nsw i32 %21, 1
  store i32 %22, ptr %4, align 4, !tbaa !15
  br label %5, !llvm.loop !18

23:                                               ; preds = %5
  %24 = load i64, ptr %3, align 8, !tbaa !16
  call void @llvm.lifetime.end.p0(ptr %4) #2
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret i64 %24
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal i64 @power(i64 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  store i64 %0, ptr %4, align 8, !tbaa !16
  store i32 %1, ptr %5, align 4, !tbaa !15
  %6 = load i32, ptr %5, align 4, !tbaa !15
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %8, label %9

8:                                                ; preds = %2
  store i64 1, ptr %3, align 8
  br label %29

9:                                                ; preds = %2
  %10 = load i32, ptr %5, align 4, !tbaa !15
  %11 = and i32 %10, 1
  %12 = icmp ne i32 %11, 0
  br i1 %12, label %13, label %21

13:                                               ; preds = %9
  %14 = load i64, ptr %4, align 8, !tbaa !16
  %15 = load i64, ptr %4, align 8, !tbaa !16
  %16 = load i32, ptr %5, align 4, !tbaa !15
  %17 = sub nsw i32 %16, 1
  %18 = call i64 @power(i64 noundef %15, i32 noundef %17) #3
  %19 = mul nsw i64 %14, %18
  %20 = srem i64 %19, 30011
  store i64 %20, ptr %3, align 8
  br label %29

21:                                               ; preds = %9
  %22 = load i64, ptr %4, align 8, !tbaa !16
  %23 = load i64, ptr %4, align 8, !tbaa !16
  %24 = mul nsw i64 %22, %23
  %25 = srem i64 %24, 30011
  %26 = load i32, ptr %5, align 4, !tbaa !15
  %27 = ashr i32 %26, 1
  %28 = call i64 @power(i64 noundef %25, i32 noundef %27) #3
  store i64 %28, ptr %3, align 8
  br label %29

29:                                               ; preds = %21, %13, %8
  %30 = load i64, ptr %3, align 8
  ret i64 %30
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }
attributes #3 = { nobuiltin "no-builtins" }

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
!17 = !{!"long", !13, i64 0}
!18 = distinct !{!18, !19}
!19 = !{!"llvm.loop.mustprogress"}
