target datalayout = "@m32-datalayout@"

@a = internal global [1024 x i32] zeroinitializer, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @bench_prefix(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 0, ptr %5, align 4
  store i32 0, ptr %3, align 4
  br label %6

6:                                                ; preds = %16, %1
  %7 = load i32, ptr %3, align 4
  %8 = load i32, ptr %2, align 4
  %9 = icmp slt i32 %7, %8
  br i1 %9, label %10, label %19

10:                                               ; preds = %6
  %11 = load i32, ptr %3, align 4
  %12 = mul nsw i32 %11, 5
  %13 = and i32 %12, 15
  %14 = load i32, ptr %3, align 4
  %15 = getelementptr inbounds [1024 x i32], ptr @a, i32 0, i32 %14
  store i32 %13, ptr %15, align 4
  br label %16

16:                                               ; preds = %10
  %17 = load i32, ptr %3, align 4
  %18 = add nsw i32 %17, 1
  store i32 %18, ptr %3, align 4
  br label %6, !llvm.loop !6

19:                                               ; preds = %6
  store i32 0, ptr %4, align 4
  br label %20

20:                                               ; preds = %54, %19
  %21 = load i32, ptr %4, align 4
  %22 = icmp slt i32 %21, 4
  br i1 %22, label %23, label %57

23:                                               ; preds = %20
  store i32 1, ptr %3, align 4
  br label %24

24:                                               ; preds = %37, %23
  %25 = load i32, ptr %3, align 4
  %26 = load i32, ptr %2, align 4
  %27 = icmp slt i32 %25, %26
  br i1 %27, label %28, label %40

28:                                               ; preds = %24
  %29 = load i32, ptr %3, align 4
  %30 = sub nsw i32 %29, 1
  %31 = getelementptr inbounds [1024 x i32], ptr @a, i32 0, i32 %30
  %32 = load i32, ptr %31, align 4
  %33 = load i32, ptr %3, align 4
  %34 = getelementptr inbounds [1024 x i32], ptr @a, i32 0, i32 %33
  %35 = load i32, ptr %34, align 4
  %36 = add nsw i32 %35, %32
  store i32 %36, ptr %34, align 4
  br label %37

37:                                               ; preds = %28
  %38 = load i32, ptr %3, align 4
  %39 = add nsw i32 %38, 1
  store i32 %39, ptr %3, align 4
  br label %24, !llvm.loop !8

40:                                               ; preds = %24
  store i32 0, ptr %3, align 4
  br label %41

41:                                               ; preds = %50, %40
  %42 = load i32, ptr %3, align 4
  %43 = load i32, ptr %2, align 4
  %44 = icmp slt i32 %42, %43
  br i1 %44, label %45, label %53

45:                                               ; preds = %41
  %46 = load i32, ptr %3, align 4
  %47 = getelementptr inbounds [1024 x i32], ptr @a, i32 0, i32 %46
  %48 = load i32, ptr %47, align 4
  %49 = and i32 %48, 65535
  store i32 %49, ptr %47, align 4
  br label %50

50:                                               ; preds = %45
  %51 = load i32, ptr %3, align 4
  %52 = add nsw i32 %51, 1
  store i32 %52, ptr %3, align 4
  br label %41, !llvm.loop !9

53:                                               ; preds = %41
  br label %54

54:                                               ; preds = %53
  %55 = load i32, ptr %4, align 4
  %56 = add nsw i32 %55, 1
  store i32 %56, ptr %4, align 4
  br label %20, !llvm.loop !10

57:                                               ; preds = %20
  store i32 0, ptr %3, align 4
  br label %58

58:                                               ; preds = %68, %57
  %59 = load i32, ptr %3, align 4
  %60 = load i32, ptr %2, align 4
  %61 = icmp slt i32 %59, %60
  br i1 %61, label %62, label %71

62:                                               ; preds = %58
  %63 = load i32, ptr %3, align 4
  %64 = getelementptr inbounds [1024 x i32], ptr @a, i32 0, i32 %63
  %65 = load i32, ptr %64, align 4
  %66 = load i32, ptr %5, align 4
  %67 = add nsw i32 %66, %65
  store i32 %67, ptr %5, align 4
  br label %68

68:                                               ; preds = %62
  %69 = load i32, ptr %3, align 4
  %70 = add nsw i32 %69, 1
  store i32 %70, ptr %3, align 4
  br label %58, !llvm.loop !11

71:                                               ; preds = %58
  %72 = load i32, ptr %5, align 4
  ret i32 %72
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
!11 = distinct !{!11, !7}
