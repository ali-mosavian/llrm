target datalayout = "@m32-datalayout@"

@a = internal global [200 x i32] zeroinitializer, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @bench_x_bubble(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 12345, ptr %3, align 4
  store i32 0, ptr %7, align 4
  store i32 0, ptr %4, align 4
  br label %8

8:                                                ; preds = %21, %1
  %9 = load i32, ptr %4, align 4
  %10 = load i32, ptr %2, align 4
  %11 = icmp slt i32 %9, %10
  br i1 %11, label %12, label %24

12:                                               ; preds = %8
  %13 = load i32, ptr %3, align 4
  %14 = mul i32 %13, 1103515245
  %15 = add i32 %14, 12345
  store i32 %15, ptr %3, align 4
  %16 = load i32, ptr %3, align 4
  %17 = lshr i32 %16, 16
  %18 = and i32 %17, 32767
  %19 = load i32, ptr %4, align 4
  %20 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %19
  store i32 %18, ptr %20, align 4
  br label %21

21:                                               ; preds = %12
  %22 = load i32, ptr %4, align 4
  %23 = add nsw i32 %22, 1
  store i32 %23, ptr %4, align 4
  br label %8, !llvm.loop !6

24:                                               ; preds = %8
  store i32 0, ptr %4, align 4
  br label %25

25:                                               ; preds = %66, %24
  %26 = load i32, ptr %4, align 4
  %27 = load i32, ptr %2, align 4
  %28 = sub nsw i32 %27, 1
  %29 = icmp slt i32 %26, %28
  br i1 %29, label %30, label %69

30:                                               ; preds = %25
  store i32 0, ptr %5, align 4
  br label %31

31:                                               ; preds = %62, %30
  %32 = load i32, ptr %5, align 4
  %33 = load i32, ptr %2, align 4
  %34 = sub nsw i32 %33, 1
  %35 = load i32, ptr %4, align 4
  %36 = sub nsw i32 %34, %35
  %37 = icmp slt i32 %32, %36
  br i1 %37, label %38, label %65

38:                                               ; preds = %31
  %39 = load i32, ptr %5, align 4
  %40 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %39
  %41 = load i32, ptr %40, align 4
  %42 = load i32, ptr %5, align 4
  %43 = add nsw i32 %42, 1
  %44 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %43
  %45 = load i32, ptr %44, align 4
  %46 = icmp sgt i32 %41, %45
  br i1 %46, label %47, label %61

47:                                               ; preds = %38
  %48 = load i32, ptr %5, align 4
  %49 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %48
  %50 = load i32, ptr %49, align 4
  store i32 %50, ptr %6, align 4
  %51 = load i32, ptr %5, align 4
  %52 = add nsw i32 %51, 1
  %53 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %52
  %54 = load i32, ptr %53, align 4
  %55 = load i32, ptr %5, align 4
  %56 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %55
  store i32 %54, ptr %56, align 4
  %57 = load i32, ptr %6, align 4
  %58 = load i32, ptr %5, align 4
  %59 = add nsw i32 %58, 1
  %60 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %59
  store i32 %57, ptr %60, align 4
  br label %61

61:                                               ; preds = %47, %38
  br label %62

62:                                               ; preds = %61
  %63 = load i32, ptr %5, align 4
  %64 = add nsw i32 %63, 1
  store i32 %64, ptr %5, align 4
  br label %31, !llvm.loop !8

65:                                               ; preds = %31
  br label %66

66:                                               ; preds = %65
  %67 = load i32, ptr %4, align 4
  %68 = add nsw i32 %67, 1
  store i32 %68, ptr %4, align 4
  br label %25, !llvm.loop !9

69:                                               ; preds = %25
  store i32 0, ptr %4, align 4
  br label %70

70:                                               ; preds = %81, %69
  %71 = load i32, ptr %4, align 4
  %72 = load i32, ptr %2, align 4
  %73 = icmp slt i32 %71, %72
  br i1 %73, label %74, label %84

74:                                               ; preds = %70
  %75 = load i32, ptr %7, align 4
  %76 = mul nsw i32 %75, 31
  %77 = load i32, ptr %4, align 4
  %78 = getelementptr inbounds [200 x i32], ptr @a, i32 0, i32 %77
  %79 = load i32, ptr %78, align 4
  %80 = add nsw i32 %76, %79
  store i32 %80, ptr %7, align 4
  br label %81

81:                                               ; preds = %74
  %82 = load i32, ptr %4, align 4
  %83 = add nsw i32 %82, 1
  store i32 %83, ptr %4, align 4
  br label %70, !llvm.loop !10

84:                                               ; preds = %70
  %85 = load i32, ptr %7, align 4
  ret i32 %85
}

attributes #0 = { noinline nounwind uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 2}
!5 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git 4b787ddb5b7f8e894de33d3c18ef1a78cd0a094a)"}
!6 = distinct !{!6, !7}
!7 = !{!"llvm.loop.mustprogress"}
!8 = distinct !{!8, !7}
!9 = distinct !{!9, !7}
!10 = distinct !{!10, !7}
