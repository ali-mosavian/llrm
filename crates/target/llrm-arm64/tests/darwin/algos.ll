source_filename = "algos.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@__const.main.a = private unnamed_addr constant [16 x i32] [i32 9, i32 3, i32 7, i32 1, i32 8, i32 2, i32 6, i32 4, i32 5, i32 0, i32 15, i32 11, i32 14, i32 10, i32 13, i32 12], align 4
@.str = private unnamed_addr constant [4 x i8] c"%d \00", align 1
@.str.1 = private unnamed_addr constant [12 x i8] c"\0A%d %lu %d\0A\00", align 1
@.str.2 = private unnamed_addr constant [4 x i8] c"%u\0A\00", align 1
@sieve.composite = internal global [2000 x i8] zeroinitializer, align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [16 x i32], align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %2, ptr align 4 @__const.main.a, i64 64, i1 false)
  %6 = getelementptr inbounds [16 x i32], ptr %2, i64 0, i64 0
  call void @bubble(ptr noundef %6, i32 noundef 16)
  store i32 0, ptr %3, align 4, !tbaa !9
  br label %7

7:                                                ; preds = %16, %0
  %8 = load i32, ptr %3, align 4, !tbaa !9
  %9 = icmp slt i32 %8, 16
  br i1 %9, label %10, label %19

10:                                               ; preds = %7
  %11 = load i32, ptr %3, align 4, !tbaa !9
  %12 = sext i32 %11 to i64
  %13 = getelementptr inbounds [16 x i32], ptr %2, i64 0, i64 %12
  %14 = load i32, ptr %13, align 4, !tbaa !9
  %15 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %14)
  br label %16

16:                                               ; preds = %10
  %17 = load i32, ptr %3, align 4, !tbaa !9
  %18 = add nsw i32 %17, 1
  store i32 %18, ptr %3, align 4, !tbaa !9
  br label %7, !llvm.loop !10

19:                                               ; preds = %7
  %20 = call i32 @sieve(i32 noundef 2000)
  %21 = call i64 @collatz(i64 noundef 837799)
  %22 = call i32 @gcd(i32 noundef 1071, i32 noundef 462)
  %23 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i32 noundef %20, i64 noundef %21, i32 noundef %22)
  store i32 -2128831035, ptr %4, align 4, !tbaa !9
  store i32 0, ptr %5, align 4, !tbaa !9
  br label %24

24:                                               ; preds = %32, %19
  %25 = load i32, ptr %5, align 4, !tbaa !9
  %26 = icmp slt i32 %25, 1000
  br i1 %26, label %27, label %35

27:                                               ; preds = %24
  %28 = load i32, ptr %4, align 4, !tbaa !9
  %29 = load i32, ptr %5, align 4, !tbaa !9
  %30 = xor i32 %28, %29
  %31 = mul i32 %30, 16777619
  store i32 %31, ptr %4, align 4, !tbaa !9
  br label %32

32:                                               ; preds = %27
  %33 = load i32, ptr %5, align 4, !tbaa !9
  %34 = add nsw i32 %33, 1
  store i32 %34, ptr %5, align 4, !tbaa !9
  br label %24, !llvm.loop !12

35:                                               ; preds = %24
  %36 = load i32, ptr %4, align 4, !tbaa !9
  %37 = call i32 (ptr, ...) @printf(ptr noundef @.str.2, i32 noundef %36)
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @bubble(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !13
  store i32 %1, ptr %4, align 4, !tbaa !9
  store i32 0, ptr %5, align 4, !tbaa !9
  br label %8

8:                                                ; preds = %60, %2
  %9 = load i32, ptr %5, align 4, !tbaa !9
  %10 = load i32, ptr %4, align 4, !tbaa !9
  %11 = icmp slt i32 %9, %10
  br i1 %11, label %12, label %63

12:                                               ; preds = %8
  store i32 0, ptr %6, align 4, !tbaa !9
  br label %13

13:                                               ; preds = %56, %12
  %14 = load i32, ptr %6, align 4, !tbaa !9
  %15 = add nsw i32 %14, 1
  %16 = load i32, ptr %4, align 4, !tbaa !9
  %17 = load i32, ptr %5, align 4, !tbaa !9
  %18 = sub nsw i32 %16, %17
  %19 = icmp slt i32 %15, %18
  br i1 %19, label %20, label %59

20:                                               ; preds = %13
  %21 = load ptr, ptr %3, align 8, !tbaa !13
  %22 = load i32, ptr %6, align 4, !tbaa !9
  %23 = sext i32 %22 to i64
  %24 = getelementptr inbounds i32, ptr %21, i64 %23
  %25 = load i32, ptr %24, align 4, !tbaa !9
  %26 = load ptr, ptr %3, align 8, !tbaa !13
  %27 = load i32, ptr %6, align 4, !tbaa !9
  %28 = add nsw i32 %27, 1
  %29 = sext i32 %28 to i64
  %30 = getelementptr inbounds i32, ptr %26, i64 %29
  %31 = load i32, ptr %30, align 4, !tbaa !9
  %32 = icmp sgt i32 %25, %31
  br i1 %32, label %33, label %55

33:                                               ; preds = %20
  %34 = load ptr, ptr %3, align 8, !tbaa !13
  %35 = load i32, ptr %6, align 4, !tbaa !9
  %36 = sext i32 %35 to i64
  %37 = getelementptr inbounds i32, ptr %34, i64 %36
  %38 = load i32, ptr %37, align 4, !tbaa !9
  store i32 %38, ptr %7, align 4, !tbaa !9
  %39 = load ptr, ptr %3, align 8, !tbaa !13
  %40 = load i32, ptr %6, align 4, !tbaa !9
  %41 = add nsw i32 %40, 1
  %42 = sext i32 %41 to i64
  %43 = getelementptr inbounds i32, ptr %39, i64 %42
  %44 = load i32, ptr %43, align 4, !tbaa !9
  %45 = load ptr, ptr %3, align 8, !tbaa !13
  %46 = load i32, ptr %6, align 4, !tbaa !9
  %47 = sext i32 %46 to i64
  %48 = getelementptr inbounds i32, ptr %45, i64 %47
  store i32 %44, ptr %48, align 4, !tbaa !9
  %49 = load i32, ptr %7, align 4, !tbaa !9
  %50 = load ptr, ptr %3, align 8, !tbaa !13
  %51 = load i32, ptr %6, align 4, !tbaa !9
  %52 = add nsw i32 %51, 1
  %53 = sext i32 %52 to i64
  %54 = getelementptr inbounds i32, ptr %50, i64 %53
  store i32 %49, ptr %54, align 4, !tbaa !9
  br label %55

55:                                               ; preds = %33, %20
  br label %56

56:                                               ; preds = %55
  %57 = load i32, ptr %6, align 4, !tbaa !9
  %58 = add nsw i32 %57, 1
  store i32 %58, ptr %6, align 4, !tbaa !9
  br label %13, !llvm.loop !16

59:                                               ; preds = %13
  br label %60

60:                                               ; preds = %59
  %61 = load i32, ptr %5, align 4, !tbaa !9
  %62 = add nsw i32 %61, 1
  store i32 %62, ptr %5, align 4, !tbaa !9
  br label %8, !llvm.loop !17

63:                                               ; preds = %8
  ret void
}

declare i32 @printf(ptr noundef, ...) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @sieve(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !9
  store i32 0, ptr %3, align 4, !tbaa !9
  store i32 2, ptr %4, align 4, !tbaa !9
  br label %6

6:                                                ; preds = %35, %1
  %7 = load i32, ptr %4, align 4, !tbaa !9
  %8 = load i32, ptr %2, align 4, !tbaa !9
  %9 = icmp slt i32 %7, %8
  br i1 %9, label %10, label %38

10:                                               ; preds = %6
  %11 = load i32, ptr %4, align 4, !tbaa !9
  %12 = sext i32 %11 to i64
  %13 = getelementptr inbounds [2000 x i8], ptr @sieve.composite, i64 0, i64 %12
  %14 = load i8, ptr %13, align 1, !tbaa !18
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %34, label %16

16:                                               ; preds = %10
  %17 = load i32, ptr %3, align 4, !tbaa !9
  %18 = add nsw i32 %17, 1
  store i32 %18, ptr %3, align 4, !tbaa !9
  %19 = load i32, ptr %4, align 4, !tbaa !9
  %20 = mul nsw i32 %19, 2
  store i32 %20, ptr %5, align 4, !tbaa !9
  br label %21

21:                                               ; preds = %29, %16
  %22 = load i32, ptr %5, align 4, !tbaa !9
  %23 = load i32, ptr %2, align 4, !tbaa !9
  %24 = icmp slt i32 %22, %23
  br i1 %24, label %25, label %33

25:                                               ; preds = %21
  %26 = load i32, ptr %5, align 4, !tbaa !9
  %27 = sext i32 %26 to i64
  %28 = getelementptr inbounds [2000 x i8], ptr @sieve.composite, i64 0, i64 %27
  store i8 1, ptr %28, align 1, !tbaa !18
  br label %29

29:                                               ; preds = %25
  %30 = load i32, ptr %4, align 4, !tbaa !9
  %31 = load i32, ptr %5, align 4, !tbaa !9
  %32 = add nsw i32 %31, %30
  store i32 %32, ptr %5, align 4, !tbaa !9
  br label %21, !llvm.loop !19

33:                                               ; preds = %21
  br label %34

34:                                               ; preds = %33, %10
  br label %35

35:                                               ; preds = %34
  %36 = load i32, ptr %4, align 4, !tbaa !9
  %37 = add nsw i32 %36, 1
  store i32 %37, ptr %4, align 4, !tbaa !9
  br label %6, !llvm.loop !20

38:                                               ; preds = %6
  %39 = load i32, ptr %3, align 4, !tbaa !9
  ret i32 %39
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @collatz(i64 noundef %0) #0 {
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !21
  store i64 0, ptr %3, align 8, !tbaa !21
  br label %4

4:                                                ; preds = %18, %1
  %5 = load i64, ptr %2, align 8, !tbaa !21
  %6 = icmp ne i64 %5, 1
  br i1 %6, label %7, label %22

7:                                                ; preds = %4
  %8 = load i64, ptr %2, align 8, !tbaa !21
  %9 = and i64 %8, 1
  %10 = icmp ne i64 %9, 0
  br i1 %10, label %11, label %15

11:                                               ; preds = %7
  %12 = load i64, ptr %2, align 8, !tbaa !21
  %13 = mul i64 3, %12
  %14 = add i64 %13, 1
  br label %18

15:                                               ; preds = %7
  %16 = load i64, ptr %2, align 8, !tbaa !21
  %17 = udiv i64 %16, 2
  br label %18

18:                                               ; preds = %15, %11
  %19 = phi i64 [ %14, %11 ], [ %17, %15 ]
  store i64 %19, ptr %2, align 8, !tbaa !21
  %20 = load i64, ptr %3, align 8, !tbaa !21
  %21 = add i64 %20, 1
  store i64 %21, ptr %3, align 8, !tbaa !21
  br label %4, !llvm.loop !23

22:                                               ; preds = %4
  %23 = load i64, ptr %3, align 8, !tbaa !21
  ret i64 %23
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @gcd(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !9
  store i32 %1, ptr %4, align 4, !tbaa !9
  br label %6

6:                                                ; preds = %9, %2
  %7 = load i32, ptr %4, align 4, !tbaa !9
  %8 = icmp ne i32 %7, 0
  br i1 %8, label %9, label %15

9:                                                ; preds = %6
  %10 = load i32, ptr %3, align 4, !tbaa !9
  %11 = load i32, ptr %4, align 4, !tbaa !9
  %12 = srem i32 %10, %11
  store i32 %12, ptr %5, align 4, !tbaa !9
  %13 = load i32, ptr %4, align 4, !tbaa !9
  store i32 %13, ptr %3, align 4, !tbaa !9
  %14 = load i32, ptr %5, align 4, !tbaa !9
  store i32 %14, ptr %4, align 4, !tbaa !9
  br label %6, !llvm.loop !24

15:                                               ; preds = %6
  %16 = load i32, ptr %3, align 4, !tbaa !9
  ret i32 %16
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"uwtable", i32 1}
!2 = !{i32 7, !"frame-pointer", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!6, !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = !{!14, !14, i64 0}
!14 = !{!"p1 int", !15, i64 0}
!15 = !{!"any pointer", !7, i64 0}
!16 = distinct !{!16, !11}
!17 = distinct !{!17, !11}
!18 = !{!7, !7, i64 0}
!19 = distinct !{!19, !11}
!20 = distinct !{!20, !11}
!21 = !{!22, !22, i64 0}
!22 = !{!"long", !7, i64 0}
!23 = distinct !{!23, !11}
!24 = distinct !{!24, !11}
