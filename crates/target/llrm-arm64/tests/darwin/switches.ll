source_filename = "switches.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [10 x i8] c"%d:%d %s\0A\00", align 1
@__const.main.xs = private unnamed_addr constant [5 x i32] [i32 3, i32 100, i32 5000, i32 -7, i32 4], align 4
@.str.1 = private unnamed_addr constant [4 x i8] c"%d \00", align 1
@.str.2 = private unnamed_addr constant [2 x i8] c"\0A\00", align 1
@.str.3 = private unnamed_addr constant [5 x i8] c"zero\00", align 1
@.str.4 = private unnamed_addr constant [4 x i8] c"one\00", align 1
@.str.5 = private unnamed_addr constant [4 x i8] c"two\00", align 1
@.str.6 = private unnamed_addr constant [5 x i8] c"many\00", align 1

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca [5 x i32], align 4
  %4 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store i32 -2, ptr %2, align 4, !tbaa !9
  br label %5

5:                                                ; preds = %15, %0
  %6 = load i32, ptr %2, align 4, !tbaa !9
  %7 = icmp slt i32 %6, 10
  br i1 %7, label %8, label %18

8:                                                ; preds = %5
  %9 = load i32, ptr %2, align 4, !tbaa !9
  %10 = load i32, ptr %2, align 4, !tbaa !9
  %11 = call i32 @dense(i32 noundef %10)
  %12 = load i32, ptr %2, align 4, !tbaa !9
  %13 = call ptr @name(i32 noundef %12)
  %14 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %9, i32 noundef %11, ptr noundef %13)
  br label %15

15:                                               ; preds = %8
  %16 = load i32, ptr %2, align 4, !tbaa !9
  %17 = add nsw i32 %16, 1
  store i32 %17, ptr %2, align 4, !tbaa !9
  br label %5, !llvm.loop !10

18:                                               ; preds = %5
  call void @llvm.memcpy.p0.p0.i64(ptr align 4 %3, ptr align 4 @__const.main.xs, i64 20, i1 false)
  store i32 0, ptr %4, align 4, !tbaa !9
  br label %19

19:                                               ; preds = %29, %18
  %20 = load i32, ptr %4, align 4, !tbaa !9
  %21 = icmp slt i32 %20, 5
  br i1 %21, label %22, label %32

22:                                               ; preds = %19
  %23 = load i32, ptr %4, align 4, !tbaa !9
  %24 = sext i32 %23 to i64
  %25 = getelementptr inbounds [5 x i32], ptr %3, i64 0, i64 %24
  %26 = load i32, ptr %25, align 4, !tbaa !9
  %27 = call i32 @sparse(i32 noundef %26)
  %28 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i32 noundef %27)
  br label %29

29:                                               ; preds = %22
  %30 = load i32, ptr %4, align 4, !tbaa !9
  %31 = add nsw i32 %30, 1
  store i32 %31, ptr %4, align 4, !tbaa !9
  br label %19, !llvm.loop !12

32:                                               ; preds = %19
  %33 = call i32 (ptr, ...) @printf(ptr noundef @.str.2)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @dense(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !9
  %4 = load i32, ptr %3, align 4, !tbaa !9
  switch i32 %4, label %13 [
    i32 0, label %5
    i32 1, label %6
    i32 2, label %7
    i32 3, label %8
    i32 4, label %9
    i32 5, label %10
    i32 6, label %11
    i32 7, label %12
  ]

5:                                                ; preds = %1
  store i32 10, ptr %2, align 4
  br label %14

6:                                                ; preds = %1
  store i32 11, ptr %2, align 4
  br label %14

7:                                                ; preds = %1
  store i32 25, ptr %2, align 4
  br label %14

8:                                                ; preds = %1
  store i32 31, ptr %2, align 4
  br label %14

9:                                                ; preds = %1
  store i32 47, ptr %2, align 4
  br label %14

10:                                               ; preds = %1
  store i32 59, ptr %2, align 4
  br label %14

11:                                               ; preds = %1
  store i32 61, ptr %2, align 4
  br label %14

12:                                               ; preds = %1
  store i32 77, ptr %2, align 4
  br label %14

13:                                               ; preds = %1
  store i32 -1, ptr %2, align 4
  br label %14

14:                                               ; preds = %13, %12, %11, %10, %9, %8, %7, %6, %5
  %15 = load i32, ptr %2, align 4
  ret i32 %15
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal ptr @name(i32 noundef %0) #0 {
  %2 = alloca ptr, align 8
  %3 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !9
  %4 = load i32, ptr %3, align 4, !tbaa !9
  switch i32 %4, label %8 [
    i32 0, label %5
    i32 1, label %6
    i32 2, label %7
  ]

5:                                                ; preds = %1
  store ptr @.str.3, ptr %2, align 8
  br label %9

6:                                                ; preds = %1
  store ptr @.str.4, ptr %2, align 8
  br label %9

7:                                                ; preds = %1
  store ptr @.str.5, ptr %2, align 8
  br label %9

8:                                                ; preds = %1
  store ptr @.str.6, ptr %2, align 8
  br label %9

9:                                                ; preds = %8, %7, %6, %5
  %10 = load ptr, ptr %2, align 8
  ret ptr %10
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i64(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i64, i1 immarg) #2

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @sparse(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !9
  %4 = load i32, ptr %3, align 4, !tbaa !9
  switch i32 %4, label %9 [
    i32 3, label %5
    i32 100, label %6
    i32 5000, label %7
    i32 -7, label %8
  ]

5:                                                ; preds = %1
  store i32 1, ptr %2, align 4
  br label %10

6:                                                ; preds = %1
  store i32 2, ptr %2, align 4
  br label %10

7:                                                ; preds = %1
  store i32 3, ptr %2, align 4
  br label %10

8:                                                ; preds = %1
  store i32 4, ptr %2, align 4
  br label %10

9:                                                ; preds = %1
  store i32 0, ptr %2, align 4
  br label %10

10:                                               ; preds = %9, %8, %7, %6, %5
  %11 = load i32, ptr %2, align 4
  ret i32 %11
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }

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
