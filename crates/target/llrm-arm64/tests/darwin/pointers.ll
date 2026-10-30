source_filename = "pointers.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [7 x i8] c"%d %d\0A\00", align 1
@.str.1 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@.str.2 = private unnamed_addr constant [13 x i8] c"hello, arm64\00", align 1
@.str.3 = private unnamed_addr constant [7 x i8] c"%s %d\0A\00", align 1
@.str.4 = private unnamed_addr constant [10 x i8] c"%d %d %d\0A\00", align 1
@mat = internal global [4 x [5 x i32]] zeroinitializer, align 4

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca [20 x i32], align 4
  %3 = alloca [32 x i8], align 1
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  store i32 0, ptr %1, align 4
  %6 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 0
  call void @fill(ptr noundef %6, i32 noundef 20)
  %7 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 0
  %8 = call i32 @sum(ptr noundef %7, i32 noundef 20)
  %9 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 0
  %10 = getelementptr inbounds i32, ptr %9, i64 5
  %11 = call i32 @sum(ptr noundef %10, i32 noundef 10)
  %12 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %8, i32 noundef %11)
  %13 = call i32 @trace()
  %14 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i32 noundef %13)
  %15 = getelementptr inbounds [32 x i8], ptr %3, i64 0, i64 0
  %16 = call ptr @copy(ptr noundef %15, ptr noundef @.str.2)
  %17 = getelementptr inbounds [32 x i8], ptr %3, i64 0, i64 0
  %18 = getelementptr inbounds [32 x i8], ptr %3, i64 0, i64 0
  %19 = call i64 @strlen(ptr noundef %18) #3
  %20 = trunc i64 %19 to i32
  %21 = call i32 (ptr, ...) @printf(ptr noundef @.str.3, ptr noundef %17, i32 noundef %20)
  %22 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 7
  store ptr %22, ptr %4, align 8, !tbaa !9
  store ptr %4, ptr %5, align 8, !tbaa !12
  %23 = load ptr, ptr %5, align 8, !tbaa !12
  %24 = load ptr, ptr %23, align 8, !tbaa !9
  %25 = load i32, ptr %24, align 4, !tbaa !15
  %26 = add nsw i32 %25, 100
  store i32 %26, ptr %24, align 4, !tbaa !15
  %27 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 7
  %28 = load i32, ptr %27, align 4, !tbaa !15
  %29 = load ptr, ptr %4, align 8, !tbaa !9
  %30 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 0
  %31 = ptrtoaddr ptr %29 to i64
  %32 = ptrtoaddr ptr %30 to i64
  %33 = sub i64 %31, %32
  %34 = sdiv exact i64 %33, 4
  %35 = trunc i64 %34 to i32
  %36 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 19
  %37 = getelementptr inbounds [20 x i32], ptr %2, i64 0, i64 3
  %38 = ptrtoaddr ptr %36 to i64
  %39 = ptrtoaddr ptr %37 to i64
  %40 = sub i64 %38, %39
  %41 = sdiv exact i64 %40, 4
  %42 = trunc i64 %41 to i32
  %43 = call i32 (ptr, ...) @printf(ptr noundef @.str.4, i32 noundef %28, i32 noundef %35, i32 noundef %42)
  ret i32 0
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal void @fill(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !9
  store i32 %1, ptr %4, align 4, !tbaa !15
  store i32 0, ptr %5, align 4, !tbaa !15
  br label %6

6:                                                ; preds = %21, %2
  %7 = load i32, ptr %5, align 4, !tbaa !15
  %8 = load i32, ptr %4, align 4, !tbaa !15
  %9 = icmp slt i32 %7, %8
  br i1 %9, label %10, label %24

10:                                               ; preds = %6
  %11 = load i32, ptr %5, align 4, !tbaa !15
  %12 = load i32, ptr %5, align 4, !tbaa !15
  %13 = mul nsw i32 %11, %12
  %14 = load i32, ptr %5, align 4, !tbaa !15
  %15 = mul nsw i32 3, %14
  %16 = sub nsw i32 %13, %15
  %17 = load ptr, ptr %3, align 8, !tbaa !9
  %18 = load i32, ptr %5, align 4, !tbaa !15
  %19 = sext i32 %18 to i64
  %20 = getelementptr inbounds i32, ptr %17, i64 %19
  store i32 %16, ptr %20, align 4, !tbaa !15
  br label %21

21:                                               ; preds = %10
  %22 = load i32, ptr %5, align 4, !tbaa !15
  %23 = add nsw i32 %22, 1
  store i32 %23, ptr %5, align 4, !tbaa !15
  br label %6, !llvm.loop !16

24:                                               ; preds = %6
  ret void
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @sum(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store ptr %0, ptr %3, align 8, !tbaa !9
  store i32 %1, ptr %4, align 4, !tbaa !15
  store i32 0, ptr %5, align 4, !tbaa !15
  store i32 0, ptr %6, align 4, !tbaa !15
  br label %7

7:                                                ; preds = %19, %2
  %8 = load i32, ptr %6, align 4, !tbaa !15
  %9 = load i32, ptr %4, align 4, !tbaa !15
  %10 = icmp slt i32 %8, %9
  br i1 %10, label %11, label %22

11:                                               ; preds = %7
  %12 = load ptr, ptr %3, align 8, !tbaa !9
  %13 = load i32, ptr %6, align 4, !tbaa !15
  %14 = sext i32 %13 to i64
  %15 = getelementptr inbounds i32, ptr %12, i64 %14
  %16 = load i32, ptr %15, align 4, !tbaa !15
  %17 = load i32, ptr %5, align 4, !tbaa !15
  %18 = add nsw i32 %17, %16
  store i32 %18, ptr %5, align 4, !tbaa !15
  br label %19

19:                                               ; preds = %11
  %20 = load i32, ptr %6, align 4, !tbaa !15
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %6, align 4, !tbaa !15
  br label %7, !llvm.loop !18

22:                                               ; preds = %7
  %23 = load i32, ptr %5, align 4, !tbaa !15
  ret i32 %23
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @trace() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 0, ptr %1, align 4, !tbaa !15
  br label %5

5:                                                ; preds = %27, %0
  %6 = load i32, ptr %1, align 4, !tbaa !15
  %7 = icmp slt i32 %6, 4
  br i1 %7, label %8, label %30

8:                                                ; preds = %5
  store i32 0, ptr %2, align 4, !tbaa !15
  br label %9

9:                                                ; preds = %23, %8
  %10 = load i32, ptr %2, align 4, !tbaa !15
  %11 = icmp slt i32 %10, 5
  br i1 %11, label %12, label %26

12:                                               ; preds = %9
  %13 = load i32, ptr %1, align 4, !tbaa !15
  %14 = mul nsw i32 %13, 10
  %15 = load i32, ptr %2, align 4, !tbaa !15
  %16 = add nsw i32 %14, %15
  %17 = load i32, ptr %1, align 4, !tbaa !15
  %18 = sext i32 %17 to i64
  %19 = getelementptr inbounds [4 x [5 x i32]], ptr @mat, i64 0, i64 %18
  %20 = load i32, ptr %2, align 4, !tbaa !15
  %21 = sext i32 %20 to i64
  %22 = getelementptr inbounds [5 x i32], ptr %19, i64 0, i64 %21
  store i32 %16, ptr %22, align 4, !tbaa !15
  br label %23

23:                                               ; preds = %12
  %24 = load i32, ptr %2, align 4, !tbaa !15
  %25 = add nsw i32 %24, 1
  store i32 %25, ptr %2, align 4, !tbaa !15
  br label %9, !llvm.loop !19

26:                                               ; preds = %9
  br label %27

27:                                               ; preds = %26
  %28 = load i32, ptr %1, align 4, !tbaa !15
  %29 = add nsw i32 %28, 1
  store i32 %29, ptr %1, align 4, !tbaa !15
  br label %5, !llvm.loop !20

30:                                               ; preds = %5
  store i32 0, ptr %3, align 4, !tbaa !15
  store i32 0, ptr %4, align 4, !tbaa !15
  br label %31

31:                                               ; preds = %54, %30
  %32 = load i32, ptr %4, align 4, !tbaa !15
  %33 = icmp slt i32 %32, 4
  br i1 %33, label %34, label %57

34:                                               ; preds = %31
  %35 = load i32, ptr %4, align 4, !tbaa !15
  %36 = sext i32 %35 to i64
  %37 = getelementptr inbounds [4 x [5 x i32]], ptr @mat, i64 0, i64 %36
  %38 = load i32, ptr %4, align 4, !tbaa !15
  %39 = add nsw i32 %38, 1
  %40 = sext i32 %39 to i64
  %41 = getelementptr inbounds [5 x i32], ptr %37, i64 0, i64 %40
  %42 = load i32, ptr %41, align 4, !tbaa !15
  %43 = load i32, ptr %4, align 4, !tbaa !15
  %44 = sub nsw i32 3, %43
  %45 = sext i32 %44 to i64
  %46 = getelementptr inbounds [4 x [5 x i32]], ptr @mat, i64 0, i64 %45
  %47 = load i32, ptr %4, align 4, !tbaa !15
  %48 = sext i32 %47 to i64
  %49 = getelementptr inbounds [5 x i32], ptr %46, i64 0, i64 %48
  %50 = load i32, ptr %49, align 4, !tbaa !15
  %51 = mul nsw i32 %42, %50
  %52 = load i32, ptr %3, align 4, !tbaa !15
  %53 = add nsw i32 %52, %51
  store i32 %53, ptr %3, align 4, !tbaa !15
  br label %54

54:                                               ; preds = %34
  %55 = load i32, ptr %4, align 4, !tbaa !15
  %56 = add nsw i32 %55, 1
  store i32 %56, ptr %4, align 4, !tbaa !15
  br label %31, !llvm.loop !21

57:                                               ; preds = %31
  %58 = load i32, ptr %3, align 4, !tbaa !15
  ret i32 %58
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal ptr @copy(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 8
  %4 = alloca ptr, align 8
  %5 = alloca ptr, align 8
  store ptr %0, ptr %3, align 8, !tbaa !22
  store ptr %1, ptr %4, align 8, !tbaa !22
  %6 = load ptr, ptr %3, align 8, !tbaa !22
  store ptr %6, ptr %5, align 8, !tbaa !22
  br label %7

7:                                                ; preds = %14, %2
  %8 = load ptr, ptr %4, align 8, !tbaa !22
  %9 = getelementptr inbounds nuw i8, ptr %8, i32 1
  store ptr %9, ptr %4, align 8, !tbaa !22
  %10 = load i8, ptr %8, align 1, !tbaa !24
  %11 = load ptr, ptr %3, align 8, !tbaa !22
  %12 = getelementptr inbounds nuw i8, ptr %11, i32 1
  store ptr %12, ptr %3, align 8, !tbaa !22
  store i8 %10, ptr %11, align 1, !tbaa !24
  %13 = icmp ne i8 %10, 0
  br i1 %13, label %14, label %15

14:                                               ; preds = %7
  br label %7, !llvm.loop !25

15:                                               ; preds = %7
  %16 = load ptr, ptr %5, align 8, !tbaa !22
  ret ptr %16
}

; Function Attrs: nounwind
declare i64 @strlen(ptr noundef) #2

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #2 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #3 = { nounwind }

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
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 int", !11, i64 0}
!11 = !{!"any pointer", !7, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"p2 int", !14, i64 0}
!14 = !{!"any p2 pointer", !11, i64 0}
!15 = !{!6, !6, i64 0}
!16 = distinct !{!16, !17}
!17 = !{!"llvm.loop.mustprogress"}
!18 = distinct !{!18, !17}
!19 = distinct !{!19, !17}
!20 = distinct !{!20, !17}
!21 = distinct !{!21, !17}
!22 = !{!23, !23, i64 0}
!23 = !{!"p1 omnipotent char", !11, i64 0}
!24 = !{!7, !7, i64 0}
!25 = distinct !{!25, !17}
