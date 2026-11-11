target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

%struct.three = type { i32, i32, i32 }

@counter = dso_local global i16 0, align 2
@t = dso_local global %struct.three zeroinitializer, align 1

; Function Attrs: nounwind optsize
define dso_local i16 @none() #0 {
  %1 = load i16, ptr @counter, align 2, !tbaa !7
  %2 = add nsw i16 %1, 1
  store i16 %2, ptr @counter, align 2, !tbaa !7
  ret i16 42
}

; Function Attrs: nounwind optsize
define dso_local i16 @two() #0 {
  %1 = load i16, ptr @counter, align 2, !tbaa !7
  %2 = add nsw i16 %1, 2
  store i16 %2, ptr @counter, align 2, !tbaa !7
  ret i16 5
}

; Function Attrs: nounwind optsize
define dso_local i16 @framed(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [4 x i16], align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 0
  store volatile i16 %4, ptr %5, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = add nsw i16 %6, 1
  %8 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 1
  store volatile i16 %7, ptr %8, align 2, !tbaa !7
  %9 = load i16, ptr %2, align 2, !tbaa !7
  %10 = add nsw i16 %9, 2
  %11 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 2
  store volatile i16 %10, ptr %11, align 2, !tbaa !7
  %12 = load i16, ptr %2, align 2, !tbaa !7
  %13 = add nsw i16 %12, 3
  %14 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 3
  store volatile i16 %13, ptr %14, align 2, !tbaa !7
  %15 = call i16 @none(ptr noundef byval(%struct.three) align 1 @t, ptr noundef byval(%struct.three) align 1 @t) #4
  %16 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 0
  %17 = load volatile i16, ptr %16, align 2, !tbaa !7
  %18 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 1
  %19 = load volatile i16, ptr %18, align 2, !tbaa !7
  %20 = mul nsw i16 %19, 10
  %21 = add nsw i16 %17, %20
  %22 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 2
  %23 = load volatile i16, ptr %22, align 2, !tbaa !7
  %24 = mul nsw i16 %23, 100
  %25 = add nsw i16 %21, %24
  %26 = getelementptr inbounds [4 x i16], ptr %3, i16 0, i16 3
  %27 = load volatile i16, ptr %26, align 2, !tbaa !7
  %28 = mul nsw i16 %27, 1000
  %29 = add nsw i16 %25, %28
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i16 %29
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 1, ptr @t, align 1, !tbaa !8
  store i32 2, ptr getelementptr inbounds nuw (i8, ptr @t, i16 4), align 1, !tbaa !11
  store i32 3, ptr getelementptr inbounds nuw (i8, ptr @t, i16 8), align 1, !tbaa !12
  %3 = call i16 @none(ptr noundef byval(%struct.three) align 1 @t) #4
  %4 = sext i16 %3 to i32
  call void @report(i32 noundef %4) #4
  %5 = call i16 @none(i16 noundef 1, i16 noundef 2, i16 noundef 3) #4
  %6 = sext i16 %5 to i32
  call void @report(i32 noundef %6) #4
  %7 = call i16 @none(i16 noundef 7) #4
  %8 = call i16 @two(ptr noundef byval(%struct.three) align 1 @t, i32 noundef 9, double noundef 1.500000e+00) #4
  %9 = add nsw i16 %7, %8
  %10 = call i16 @none() #4
  %11 = add nsw i16 %9, %10
  store i16 %11, ptr %2, align 2, !tbaa !7
  %12 = load i16, ptr %2, align 2, !tbaa !7
  %13 = sext i16 %12 to i32
  call void @report(i32 noundef %13) #4
  %14 = call i16 @two(ptr noundef byval(%struct.three) align 1 @t) #4
  %15 = load i16, ptr %2, align 2, !tbaa !7
  %16 = add nsw i16 %14, %15
  %17 = sext i16 %16 to i32
  call void @report(i32 noundef %17) #4
  %18 = call i16 @framed(i16 noundef 1) #4
  %19 = sext i16 %18 to i32
  call void @report(i32 noundef %19) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

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
!8 = !{!9, !10, i64 0}
!9 = !{!"three", !10, i64 0, !10, i64 4, !10, i64 8}
!10 = !{!"long", !5, i64 0}
!11 = !{!9, !10, i64 4}
!12 = !{!9, !10, i64 8}
