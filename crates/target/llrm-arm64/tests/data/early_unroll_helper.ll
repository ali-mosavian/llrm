; ModuleID = 'w15unr.c'
source_filename = "w15unr.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@a = dso_local global [16 x i32] zeroinitializer, align 4
@b = dso_local global [16 x i32] zeroinitializer, align 4

; Function Attrs: nounwind uwtable
define dso_local i32 @run(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !16
  store i32 %1, ptr %4, align 4, !tbaa !16
  %5 = load i32, ptr %3, align 4, !tbaa !16
  %6 = call i32 @fill(ptr noundef @a, i32 noundef %5)
  %7 = load i32, ptr %4, align 4, !tbaa !16
  %8 = call i32 @fill(ptr noundef @b, i32 noundef %7)
  %9 = add nsw i32 %6, %8
  ret i32 %9
}

; Function Attrs: nounwind uwtable
define internal i32 @fill(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !17
  store i32 %1, ptr %4, align 4, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %5) #2
  store i32 0, ptr %5, align 4, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %6) #2
  store i32 0, ptr %6, align 4, !tbaa !16
  br label %8

8:                                                ; preds = %30, %2
  %9 = load i32, ptr %6, align 4, !tbaa !16
  %10 = icmp slt i32 %9, 16
  br i1 %10, label %12, label %11

11:                                               ; preds = %8
  call void @llvm.lifetime.end.p0(ptr %6) #2
  br label %33

12:                                               ; preds = %8
  call void @llvm.lifetime.start.p0(ptr %7) #2
  %13 = load ptr, ptr %3, align 8, !tbaa !17
  %14 = load i32, ptr %6, align 4, !tbaa !16
  %15 = sext i32 %14 to i64
  %16 = getelementptr inbounds i32, ptr %13, i64 %15
  %17 = load i32, ptr %16, align 4, !tbaa !16
  %18 = load i32, ptr %4, align 4, !tbaa !16
  %19 = mul nsw i32 %17, %18
  %20 = load i32, ptr %5, align 4, !tbaa !16
  %21 = add nsw i32 %19, %20
  store i32 %21, ptr %7, align 4, !tbaa !16
  %22 = load i32, ptr %7, align 4, !tbaa !16
  %23 = load i32, ptr %5, align 4, !tbaa !16
  %24 = add nsw i32 %23, %22
  store i32 %24, ptr %5, align 4, !tbaa !16
  %25 = load i32, ptr %7, align 4, !tbaa !16
  %26 = load ptr, ptr %3, align 8, !tbaa !17
  %27 = load i32, ptr %6, align 4, !tbaa !16
  %28 = sext i32 %27 to i64
  %29 = getelementptr inbounds i32, ptr %26, i64 %28
  store i32 %25, ptr %29, align 4, !tbaa !16
  call void @llvm.lifetime.end.p0(ptr %7) #2
  br label %30

30:                                               ; preds = %12
  %31 = load i32, ptr %6, align 4, !tbaa !16
  %32 = add nsw i32 %31, 1
  store i32 %32, ptr %6, align 4, !tbaa !16
  br label %8, !llvm.loop !20

33:                                               ; preds = %11
  %34 = load i32, ptr %5, align 4, !tbaa !16
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i32 %34
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
!18 = !{!"p1 int", !19, i64 0}
!19 = !{!"any pointer", !14, i64 0}
!20 = distinct !{!20, !21}
!21 = !{!"llvm.loop.mustprogress"}
