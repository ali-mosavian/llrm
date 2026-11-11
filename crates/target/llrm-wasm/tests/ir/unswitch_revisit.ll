target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

; Function Attrs: noinline nounwind
define hidden zeroext i8 @f(i8 noundef zeroext %0, i8 noundef zeroext %1, i8 noundef zeroext %2) #0 {
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  %9 = alloca i8, align 1
  %10 = alloca i8, align 1
  %11 = alloca i32, align 4
  %12 = alloca i8, align 1
  %13 = alloca i8, align 1
  store i8 %0, ptr %4, align 1, !tbaa !6
  store i8 %1, ptr %5, align 1, !tbaa !6
  store i8 %2, ptr %6, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %7) #2
  store i8 0, ptr %7, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %8) #2
  store i8 0, ptr %8, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %9) #2
  store i8 0, ptr %9, align 1, !tbaa !6
  %14 = load i8, ptr %7, align 1, !tbaa !6
  store i8 %14, ptr %4, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %10) #2
  store i8 0, ptr %10, align 1, !tbaa !6
  br label %15

15:                                               ; preds = %83, %3
  %16 = load i8, ptr %10, align 1, !tbaa !6
  %17 = zext i8 %16 to i32
  %18 = load i8, ptr %6, align 1, !tbaa !6
  %19 = zext i8 %18 to i32
  %20 = add nsw i32 %19, 0
  %21 = trunc i32 %20 to i8
  %22 = zext i8 %21 to i32
  %23 = icmp slt i32 %17, %22
  br i1 %23, label %25, label %24

24:                                               ; preds = %15
  store i32 2, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %10) #2
  br label %86

25:                                               ; preds = %15
  call void @llvm.lifetime.start.p0(ptr %12) #2
  store i8 0, ptr %12, align 1, !tbaa !6
  br label %26

26:                                               ; preds = %43, %25
  %27 = load i8, ptr %12, align 1, !tbaa !6
  %28 = zext i8 %27 to i32
  %29 = load i8, ptr %6, align 1, !tbaa !6
  %30 = zext i8 %29 to i32
  %31 = add nsw i32 %30, 1
  %32 = trunc i32 %31 to i8
  %33 = zext i8 %32 to i32
  %34 = icmp slt i32 %28, %33
  br i1 %34, label %36, label %35

35:                                               ; preds = %26
  store i32 5, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %12) #2
  br label %46

36:                                               ; preds = %26
  %37 = load i8, ptr %4, align 1, !tbaa !6
  %38 = zext i8 %37 to i32
  %39 = load i8, ptr %7, align 1, !tbaa !6
  %40 = zext i8 %39 to i32
  %41 = xor i32 %38, %40
  %42 = trunc i32 %41 to i8
  store i8 %42, ptr %7, align 1, !tbaa !6
  br label %43

43:                                               ; preds = %36
  %44 = load i8, ptr %12, align 1, !tbaa !6
  %45 = add i8 %44, 1
  store i8 %45, ptr %12, align 1, !tbaa !6
  br label %26, !llvm.loop !7

46:                                               ; preds = %35
  %47 = load i8, ptr %5, align 1, !tbaa !6
  %48 = zext i8 %47 to i32
  %49 = or i32 %48, 3
  %50 = trunc i32 %49 to i8
  store i8 %50, ptr %8, align 1, !tbaa !6
  %51 = load i8, ptr %8, align 1, !tbaa !6
  %52 = zext i8 %51 to i32
  %53 = icmp ne i32 %52, 3
  br i1 %53, label %54, label %55

54:                                               ; preds = %46
  br label %82

55:                                               ; preds = %46
  %56 = load i8, ptr %5, align 1, !tbaa !6
  %57 = zext i8 %56 to i32
  %58 = xor i32 %57, 3
  %59 = trunc i32 %58 to i8
  store i8 %59, ptr %4, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %13) #2
  store i8 0, ptr %13, align 1, !tbaa !6
  br label %60

60:                                               ; preds = %78, %55
  %61 = load i8, ptr %13, align 1, !tbaa !6
  %62 = zext i8 %61 to i32
  %63 = load i8, ptr %6, align 1, !tbaa !6
  %64 = zext i8 %63 to i32
  %65 = add nsw i32 %64, 1
  %66 = trunc i32 %65 to i8
  %67 = zext i8 %66 to i32
  %68 = icmp slt i32 %62, %67
  br i1 %68, label %70, label %69

69:                                               ; preds = %60
  store i32 8, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %13) #2
  br label %81

70:                                               ; preds = %60
  %71 = load i8, ptr %7, align 1, !tbaa !6
  store i8 %71, ptr %5, align 1, !tbaa !6
  %72 = load i8, ptr %4, align 1, !tbaa !6
  %73 = zext i8 %72 to i32
  %74 = load i8, ptr %7, align 1, !tbaa !6
  %75 = zext i8 %74 to i32
  %76 = add nsw i32 %73, %75
  %77 = trunc i32 %76 to i8
  store i8 %77, ptr %4, align 1, !tbaa !6
  br label %78

78:                                               ; preds = %70
  %79 = load i8, ptr %13, align 1, !tbaa !6
  %80 = add i8 %79, 1
  store i8 %80, ptr %13, align 1, !tbaa !6
  br label %60, !llvm.loop !9

81:                                               ; preds = %69
  br label %82

82:                                               ; preds = %81, %54
  br label %83

83:                                               ; preds = %82
  %84 = load i8, ptr %10, align 1, !tbaa !6
  %85 = add i8 %84, 1
  store i8 %85, ptr %10, align 1, !tbaa !6
  br label %15, !llvm.loop !10

86:                                               ; preds = %24
  %87 = load i8, ptr %4, align 1, !tbaa !6
  %88 = zext i8 %87 to i32
  %89 = load i8, ptr %5, align 1, !tbaa !6
  %90 = zext i8 %89 to i32
  %91 = add nsw i32 %88, %90
  %92 = load i8, ptr %7, align 1, !tbaa !6
  %93 = zext i8 %92 to i32
  %94 = add nsw i32 %91, %93
  %95 = load i8, ptr %8, align 1, !tbaa !6
  %96 = zext i8 %95 to i32
  %97 = add nsw i32 %94, %96
  %98 = load i8, ptr %9, align 1, !tbaa !6
  %99 = zext i8 %98 to i32
  %100 = add nsw i32 %97, %99
  %101 = trunc i32 %100 to i8
  store i32 1, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %9) #2
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  ret i8 %101
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!4, !4, i64 0}
!7 = distinct !{!7, !8}
!8 = !{!"llvm.loop.mustprogress"}
!9 = distinct !{!9, !8}
!10 = distinct !{!10, !8}
