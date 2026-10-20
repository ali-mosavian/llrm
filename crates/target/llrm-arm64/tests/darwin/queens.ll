source_filename = "queens.c"
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@.str = private unnamed_addr constant [4 x i8] c"%d \00", align 1
@.str.1 = private unnamed_addr constant [10 x i8] c"\0A%ld %lu\0A\00", align 1
@cols = internal global [12 x i32] zeroinitializer, align 4
@d1 = internal global [24 x i32] zeroinitializer, align 4
@d2 = internal global [24 x i32] zeroinitializer, align 4

; Function Attrs: nounwind ssp uwtable(sync)
define i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store i32 4, ptr %2, align 4, !tbaa !9
  br label %3

3:                                                ; preds = %10, %0
  %4 = load i32, ptr %2, align 4, !tbaa !9
  %5 = icmp sle i32 %4, 8
  br i1 %5, label %6, label %13

6:                                                ; preds = %3
  %7 = load i32, ptr %2, align 4, !tbaa !9
  %8 = call i32 @solve(i32 noundef 0, i32 noundef %7)
  %9 = call i32 (ptr, ...) @printf(ptr noundef @.str, i32 noundef %8)
  br label %10

10:                                               ; preds = %6
  %11 = load i32, ptr %2, align 4, !tbaa !9
  %12 = add nsw i32 %11, 1
  store i32 %12, ptr %2, align 4, !tbaa !9
  br label %3, !llvm.loop !10

13:                                               ; preds = %3
  %14 = call i64 @ack(i64 noundef 2, i64 noundef 3)
  %15 = call i64 @mulhi(i64 noundef -2401053089206439923, i64 noundef 1311768467463790321)
  %16 = call i32 (ptr, ...) @printf(ptr noundef @.str.1, i64 noundef %14, i64 noundef %15)
  ret i32 0
}

declare i32 @printf(ptr noundef, ...) #1

; Function Attrs: nounwind ssp uwtable(sync)
define internal i32 @solve(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !9
  store i32 %1, ptr %5, align 4, !tbaa !9
  %8 = load i32, ptr %4, align 4, !tbaa !9
  %9 = load i32, ptr %5, align 4, !tbaa !9
  %10 = icmp eq i32 %8, %9
  br i1 %10, label %11, label %12

11:                                               ; preds = %2
  store i32 1, ptr %3, align 4
  br label %84

12:                                               ; preds = %2
  store i32 0, ptr %6, align 4, !tbaa !9
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %13

13:                                               ; preds = %79, %12
  %14 = load i32, ptr %7, align 4, !tbaa !9
  %15 = load i32, ptr %5, align 4, !tbaa !9
  %16 = icmp slt i32 %14, %15
  br i1 %16, label %17, label %82

17:                                               ; preds = %13
  %18 = load i32, ptr %7, align 4, !tbaa !9
  %19 = sext i32 %18 to i64
  %20 = getelementptr inbounds [12 x i32], ptr @cols, i64 0, i64 %19
  %21 = load i32, ptr %20, align 4, !tbaa !9
  %22 = icmp ne i32 %21, 0
  br i1 %22, label %41, label %23

23:                                               ; preds = %17
  %24 = load i32, ptr %4, align 4, !tbaa !9
  %25 = load i32, ptr %7, align 4, !tbaa !9
  %26 = add nsw i32 %24, %25
  %27 = sext i32 %26 to i64
  %28 = getelementptr inbounds [24 x i32], ptr @d1, i64 0, i64 %27
  %29 = load i32, ptr %28, align 4, !tbaa !9
  %30 = icmp ne i32 %29, 0
  br i1 %30, label %41, label %31

31:                                               ; preds = %23
  %32 = load i32, ptr %4, align 4, !tbaa !9
  %33 = load i32, ptr %7, align 4, !tbaa !9
  %34 = sub nsw i32 %32, %33
  %35 = load i32, ptr %5, align 4, !tbaa !9
  %36 = add nsw i32 %34, %35
  %37 = sext i32 %36 to i64
  %38 = getelementptr inbounds [24 x i32], ptr @d2, i64 0, i64 %37
  %39 = load i32, ptr %38, align 4, !tbaa !9
  %40 = icmp ne i32 %39, 0
  br i1 %40, label %41, label %42

41:                                               ; preds = %31, %23, %17
  br label %79

42:                                               ; preds = %31
  %43 = load i32, ptr %4, align 4, !tbaa !9
  %44 = load i32, ptr %7, align 4, !tbaa !9
  %45 = sub nsw i32 %43, %44
  %46 = load i32, ptr %5, align 4, !tbaa !9
  %47 = add nsw i32 %45, %46
  %48 = sext i32 %47 to i64
  %49 = getelementptr inbounds [24 x i32], ptr @d2, i64 0, i64 %48
  store i32 1, ptr %49, align 4, !tbaa !9
  %50 = load i32, ptr %4, align 4, !tbaa !9
  %51 = load i32, ptr %7, align 4, !tbaa !9
  %52 = add nsw i32 %50, %51
  %53 = sext i32 %52 to i64
  %54 = getelementptr inbounds [24 x i32], ptr @d1, i64 0, i64 %53
  store i32 1, ptr %54, align 4, !tbaa !9
  %55 = load i32, ptr %7, align 4, !tbaa !9
  %56 = sext i32 %55 to i64
  %57 = getelementptr inbounds [12 x i32], ptr @cols, i64 0, i64 %56
  store i32 1, ptr %57, align 4, !tbaa !9
  %58 = load i32, ptr %4, align 4, !tbaa !9
  %59 = add nsw i32 %58, 1
  %60 = load i32, ptr %5, align 4, !tbaa !9
  %61 = call i32 @solve(i32 noundef %59, i32 noundef %60)
  %62 = load i32, ptr %6, align 4, !tbaa !9
  %63 = add nsw i32 %62, %61
  store i32 %63, ptr %6, align 4, !tbaa !9
  %64 = load i32, ptr %4, align 4, !tbaa !9
  %65 = load i32, ptr %7, align 4, !tbaa !9
  %66 = sub nsw i32 %64, %65
  %67 = load i32, ptr %5, align 4, !tbaa !9
  %68 = add nsw i32 %66, %67
  %69 = sext i32 %68 to i64
  %70 = getelementptr inbounds [24 x i32], ptr @d2, i64 0, i64 %69
  store i32 0, ptr %70, align 4, !tbaa !9
  %71 = load i32, ptr %4, align 4, !tbaa !9
  %72 = load i32, ptr %7, align 4, !tbaa !9
  %73 = add nsw i32 %71, %72
  %74 = sext i32 %73 to i64
  %75 = getelementptr inbounds [24 x i32], ptr @d1, i64 0, i64 %74
  store i32 0, ptr %75, align 4, !tbaa !9
  %76 = load i32, ptr %7, align 4, !tbaa !9
  %77 = sext i32 %76 to i64
  %78 = getelementptr inbounds [12 x i32], ptr @cols, i64 0, i64 %77
  store i32 0, ptr %78, align 4, !tbaa !9
  br label %79

79:                                               ; preds = %42, %41
  %80 = load i32, ptr %7, align 4, !tbaa !9
  %81 = add nsw i32 %80, 1
  store i32 %81, ptr %7, align 4, !tbaa !9
  br label %13, !llvm.loop !12

82:                                               ; preds = %13
  %83 = load i32, ptr %6, align 4, !tbaa !9
  store i32 %83, ptr %3, align 4
  br label %84

84:                                               ; preds = %82, %11
  %85 = load i32, ptr %3, align 4
  ret i32 %85
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @ack(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8, !tbaa !13
  store i64 %1, ptr %4, align 8, !tbaa !13
  %5 = load i64, ptr %3, align 8, !tbaa !13
  %6 = icmp eq i64 %5, 0
  br i1 %6, label %7, label %10

7:                                                ; preds = %2
  %8 = load i64, ptr %4, align 8, !tbaa !13
  %9 = add nsw i64 %8, 1
  br label %27

10:                                               ; preds = %2
  %11 = load i64, ptr %4, align 8, !tbaa !13
  %12 = icmp eq i64 %11, 0
  br i1 %12, label %13, label %17

13:                                               ; preds = %10
  %14 = load i64, ptr %3, align 8, !tbaa !13
  %15 = sub nsw i64 %14, 1
  %16 = call i64 @ack(i64 noundef %15, i64 noundef 1)
  br label %25

17:                                               ; preds = %10
  %18 = load i64, ptr %3, align 8, !tbaa !13
  %19 = sub nsw i64 %18, 1
  %20 = load i64, ptr %3, align 8, !tbaa !13
  %21 = load i64, ptr %4, align 8, !tbaa !13
  %22 = sub nsw i64 %21, 1
  %23 = call i64 @ack(i64 noundef %20, i64 noundef %22)
  %24 = call i64 @ack(i64 noundef %19, i64 noundef %23)
  br label %25

25:                                               ; preds = %17, %13
  %26 = phi i64 [ %16, %13 ], [ %24, %17 ]
  br label %27

27:                                               ; preds = %25, %7
  %28 = phi i64 [ %9, %7 ], [ %26, %25 ]
  ret i64 %28
}

; Function Attrs: nounwind ssp uwtable(sync)
define internal i64 @mulhi(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8, !tbaa !13
  store i64 %1, ptr %4, align 8, !tbaa !13
  %5 = load i64, ptr %3, align 8, !tbaa !13
  %6 = zext i64 %5 to i128
  %7 = load i64, ptr %4, align 8, !tbaa !13
  %8 = zext i64 %7 to i128
  %9 = mul i128 %6, %8
  %10 = lshr i128 %9, 64
  %11 = trunc i128 %10 to i64
  ret i64 %11
}

attributes #0 = { nounwind ssp uwtable(sync) "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="apple-m1" "target-features"="+aes,+altnzcv,+ccdp,+ccidx,+ccpp,+complxnum,+crc,+dit,+dotprod,+flagm,+fp-armv8,+fp16fml,+fptoint,+fullfp16,+jsconv,+lse,+neon,+pauth,+perfmon,+predres,+ras,+rcpc,+rdm,+sb,+sha2,+sha3,+specrestrict,+ssbs,+v8.1a,+v8.2a,+v8.3a,+v8.4a,+v8a" "tune-cpu"="apple-m5" }

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
!14 = !{!"long", !7, i64 0}
