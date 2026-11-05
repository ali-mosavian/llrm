target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !8
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %8

8:                                                ; preds = %78, %0
  %9 = load i16, ptr %2, align 2, !tbaa !7
  %10 = icmp slt i16 %9, 3
  br i1 %10, label %11, label %81

11:                                               ; preds = %8
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %12

12:                                               ; preds = %74, %11
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = icmp slt i16 %13, 3
  br i1 %14, label %15, label %77

15:                                               ; preds = %12
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %16 = load i16, ptr %2, align 2, !tbaa !7
  %17 = icmp ne i16 %16, 0
  br i1 %17, label %18, label %21

18:                                               ; preds = %15
  %19 = load i16, ptr %3, align 2, !tbaa !7
  %20 = icmp ne i16 %19, 0
  br label %21

21:                                               ; preds = %18, %15
  %22 = phi i1 [ false, %15 ], [ %20, %18 ]
  %23 = zext i1 %22 to i16
  store i16 %23, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %24 = load i16, ptr %2, align 2, !tbaa !7
  %25 = icmp ne i16 %24, 0
  br i1 %25, label %29, label %26

26:                                               ; preds = %21
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = icmp ne i16 %27, 0
  br label %29

29:                                               ; preds = %26, %21
  %30 = phi i1 [ true, %21 ], [ %28, %26 ]
  %31 = zext i1 %30 to i16
  store i16 %31, ptr %7, align 2, !tbaa !7
  %32 = load i16, ptr %4, align 2, !tbaa !7
  %33 = mul nsw i16 %32, 3
  %34 = load i16, ptr %6, align 2, !tbaa !7
  %35 = add nsw i16 %33, %34
  %36 = load i16, ptr %7, align 2, !tbaa !7
  %37 = mul nsw i16 %36, 2
  %38 = add nsw i16 %35, %37
  %39 = load i16, ptr %2, align 2, !tbaa !7
  %40 = load i16, ptr %3, align 2, !tbaa !7
  %41 = call i16 @both(i16 noundef %39, i16 noundef %40) #4
  %42 = mul nsw i16 %41, 5
  %43 = add nsw i16 %38, %42
  %44 = load i16, ptr %2, align 2, !tbaa !7
  %45 = load i16, ptr %3, align 2, !tbaa !7
  %46 = call i16 @either(i16 noundef %44, i16 noundef %45) #4
  %47 = mul nsw i16 %46, 7
  %48 = add nsw i16 %43, %47
  store i16 %48, ptr %4, align 2, !tbaa !7
  %49 = load i16, ptr %2, align 2, !tbaa !7
  %50 = icmp ne i16 %49, 0
  br i1 %50, label %51, label %55

51:                                               ; preds = %29
  %52 = load i16, ptr %3, align 2, !tbaa !7
  %53 = icmp ne i16 %52, 0
  %54 = xor i1 %53, true
  br label %55

55:                                               ; preds = %51, %29
  %56 = phi i1 [ false, %29 ], [ %54, %51 ]
  %57 = zext i1 %56 to i16
  %58 = load i16, ptr %3, align 2, !tbaa !7
  %59 = icmp ne i16 %58, 0
  br i1 %59, label %66, label %60

60:                                               ; preds = %55
  %61 = load i16, ptr %2, align 2, !tbaa !7
  %62 = icmp ne i16 %61, 0
  br i1 %62, label %63, label %64

63:                                               ; preds = %60
  br label %64

64:                                               ; preds = %63, %60
  %65 = phi i1 [ false, %60 ], [ false, %63 ]
  br label %66

66:                                               ; preds = %64, %55
  %67 = phi i1 [ true, %55 ], [ %65, %64 ]
  %68 = zext i1 %67 to i16
  %69 = mul nsw i16 %68, 2
  %70 = add nsw i16 %57, %69
  %71 = sext i16 %70 to i32
  %72 = load i32, ptr %5, align 2, !tbaa !8
  %73 = add nsw i32 %72, %71
  store i32 %73, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  br label %74

74:                                               ; preds = %66
  %75 = load i16, ptr %3, align 2, !tbaa !7
  %76 = add nsw i16 %75, 1
  store i16 %76, ptr %3, align 2, !tbaa !7
  br label %12, !llvm.loop !10

77:                                               ; preds = %12
  br label %78

78:                                               ; preds = %77
  %79 = load i16, ptr %2, align 2, !tbaa !7
  %80 = add nsw i16 %79, 1
  store i16 %80, ptr %2, align 2, !tbaa !7
  br label %8, !llvm.loop !12

81:                                               ; preds = %8
  %82 = load i16, ptr %4, align 2, !tbaa !7
  %83 = sext i16 %82 to i32
  call void @report(i32 noundef %83) #4
  %84 = load i32, ptr %5, align 2, !tbaa !8
  call void @report(i32 noundef %84) #4
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i16 @both(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %7, label %10

7:                                                ; preds = %2
  %8 = load i16, ptr %4, align 2, !tbaa !7
  %9 = icmp ne i16 %8, 0
  br label %10

10:                                               ; preds = %7, %2
  %11 = phi i1 [ false, %2 ], [ %9, %7 ]
  %12 = zext i1 %11 to i16
  ret i16 %12
}

; Function Attrs: nounwind optsize
define internal i16 @either(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %10, label %7

7:                                                ; preds = %2
  %8 = load i16, ptr %4, align 2, !tbaa !7
  %9 = icmp ne i16 %8, 0
  br label %10

10:                                               ; preds = %7, %2
  %11 = phi i1 [ true, %2 ], [ %9, %7 ]
  %12 = zext i1 %11 to i16
  ret i16 %12
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
