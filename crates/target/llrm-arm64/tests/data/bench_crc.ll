; ModuleID = 'out/src/crc.c'
source_filename = "out/src/crc.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@bench_crc.data = internal constant [10 x i8] c"123456789\00", align 1
@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind
define dso_local i64 @bench_crc(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i64 %0, ptr %2, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %3) #4
  %6 = load i64, ptr %2, align 8, !tbaa !15
  %7 = xor i64 4294967295, %6
  store i64 %7, ptr %3, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store i16 0, ptr %4, align 2, !tbaa !17
  br label %8

8:                                                ; preds = %36, %1
  %9 = load i16, ptr %4, align 2, !tbaa !17
  %10 = zext i16 %9 to i32
  %11 = icmp slt i32 %10, 9
  br i1 %11, label %12, label %39

12:                                               ; preds = %8
  %13 = load i16, ptr %4, align 2, !tbaa !17
  %14 = zext i16 %13 to i64
  %15 = getelementptr inbounds nuw [10 x i8], ptr @bench_crc.data, i64 0, i64 %14
  %16 = load i8, ptr %15, align 1, !tbaa !19
  %17 = zext i8 %16 to i64
  %18 = load i64, ptr %3, align 8, !tbaa !15
  %19 = xor i64 %18, %17
  store i64 %19, ptr %3, align 8, !tbaa !15
  store i16 0, ptr %5, align 2, !tbaa !17
  br label %20

20:                                               ; preds = %32, %12
  %21 = load i16, ptr %5, align 2, !tbaa !17
  %22 = zext i16 %21 to i32
  %23 = icmp slt i32 %22, 8
  br i1 %23, label %24, label %35

24:                                               ; preds = %20
  %25 = load i64, ptr %3, align 8, !tbaa !15
  %26 = lshr i64 %25, 1
  %27 = load i64, ptr %3, align 8, !tbaa !15
  %28 = and i64 %27, 1
  %29 = sub i64 0, %28
  %30 = and i64 3988292384, %29
  %31 = xor i64 %26, %30
  store i64 %31, ptr %3, align 8, !tbaa !15
  br label %32

32:                                               ; preds = %24
  %33 = load i16, ptr %5, align 2, !tbaa !17
  %34 = add i16 %33, 1
  store i16 %34, ptr %5, align 2, !tbaa !17
  br label %20, !llvm.loop !20

35:                                               ; preds = %20
  br label %36

36:                                               ; preds = %35
  %37 = load i16, ptr %4, align 2, !tbaa !17
  %38 = add i16 %37, 1
  store i16 %38, ptr %4, align 2, !tbaa !17
  br label %8, !llvm.loop !22

39:                                               ; preds = %8
  %40 = load i64, ptr %3, align 8, !tbaa !15
  %41 = xor i64 %40, 4294967295
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret i64 %41
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #2 {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !23
  %2 = add nsw i32 0, %1
  %3 = sext i32 %2 to i64
  %4 = call i64 @bench_crc(i64 noundef %3) #5
  call void @report(i64 noundef %4) #5
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
!16 = !{!"long", !13, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"short", !13, i64 0}
!19 = !{!13, !13, i64 0}
!20 = distinct !{!20, !21}
!21 = !{!"llvm.loop.mustprogress"}
!22 = distinct !{!22, !21}
!23 = !{!12, !12, i64 0}
