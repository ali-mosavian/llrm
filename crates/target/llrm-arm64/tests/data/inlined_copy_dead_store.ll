; ModuleID = 't10.c'
source_filename = "t10.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@state = internal global i64 1, align 8

; Function Attrs: nounwind optsize
define dso_local i64 @f(i64 noundef %0, ptr noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca ptr, align 8
  %5 = alloca i64, align 8
  %6 = alloca i64, align 8
  %7 = alloca i64, align 8
  %8 = alloca i64, align 8
  store i64 %0, ptr %3, align 8, !tbaa !15
  store ptr %1, ptr %4, align 8, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %5) #2
  store i64 0, ptr %5, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %6) #2
  store i64 0, ptr %6, align 8, !tbaa !15
  br label %9

9:                                                ; preds = %26, %2
  %10 = load i64, ptr %6, align 8, !tbaa !15
  %11 = load i64, ptr %3, align 8, !tbaa !15
  %12 = icmp slt i64 %10, %11
  br i1 %12, label %14, label %13

13:                                               ; preds = %9
  call void @llvm.lifetime.end.p0(ptr %6) #2
  br label %29

14:                                               ; preds = %9
  call void @llvm.lifetime.start.p0(ptr %7) #2
  %15 = call i64 @next() #3
  store i64 %15, ptr %7, align 8, !tbaa !15
  %16 = load ptr, ptr %4, align 8, !tbaa !17
  %17 = load volatile i64, ptr %16, align 8, !tbaa !15
  %18 = load i64, ptr %5, align 8, !tbaa !15
  %19 = add i64 %18, %17
  store i64 %19, ptr %5, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %8) #2
  %20 = call i64 @next() #3
  store i64 %20, ptr %8, align 8, !tbaa !15
  %21 = load i64, ptr %7, align 8, !tbaa !15
  %22 = load i64, ptr %8, align 8, !tbaa !15
  %23 = add i64 %21, %22
  %24 = load i64, ptr %5, align 8, !tbaa !15
  %25 = add i64 %24, %23
  store i64 %25, ptr %5, align 8, !tbaa !15
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  br label %26

26:                                               ; preds = %14
  %27 = load i64, ptr %6, align 8, !tbaa !15
  %28 = add nsw i64 %27, 1
  store i64 %28, ptr %6, align 8, !tbaa !15
  br label %9, !llvm.loop !20

29:                                               ; preds = %13
  %30 = load i64, ptr %5, align 8, !tbaa !15
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i64 %30

; uselistorder directives
  uselistorder ptr %6, { 1, 2, 0, 3, 4, 5 }
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i64 @next() #0 {
  %1 = load i64, ptr @state, align 8, !tbaa !15
  %2 = shl i64 %1, 13
  %3 = load i64, ptr @state, align 8, !tbaa !15
  %4 = xor i64 %3, %2
  store i64 %4, ptr @state, align 8, !tbaa !15
  %5 = load i64, ptr @state, align 8, !tbaa !15
  %6 = lshr i64 %5, 17
  %7 = load i64, ptr @state, align 8, !tbaa !15
  %8 = xor i64 %7, %6
  store i64 %8, ptr @state, align 8, !tbaa !15
  %9 = load i64, ptr @state, align 8, !tbaa !15
  %10 = shl i64 %9, 5
  %11 = load i64, ptr @state, align 8, !tbaa !15
  %12 = xor i64 %11, %10
  store i64 %12, ptr @state, align 8, !tbaa !15
  %13 = load i64, ptr @state, align 8, !tbaa !15
  ret i64 %13
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; uselistorder directives
uselistorder ptr @llvm.lifetime.start.p0, { 3, 2, 1, 0 }
uselistorder ptr @next, { 1, 0 }
uselistorder ptr @llvm.lifetime.end.p0, { 1, 3, 2, 0 }

attributes #0 = { nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }
attributes #3 = { nobuiltin optsize "no-builtins" }

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
!18 = !{!"p1 long", !19, i64 0}
!19 = !{!"any pointer", !13, i64 0}
!20 = distinct !{!20, !21}
!21 = !{!"llvm.loop.mustprogress"}
