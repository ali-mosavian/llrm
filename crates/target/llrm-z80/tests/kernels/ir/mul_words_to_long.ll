target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@main.values = internal constant [10 x i16] [i16 0, i16 1, i16 2, i16 255, i16 256, i16 12345, i16 32767, i16 -32768, i16 -11215, i16 -1], align 2

; Function Attrs: nounwind
define dso_local i32 @product(i16 noundef zeroext %0, i16 noundef zeroext %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = zext i16 %5 to i32
  %7 = load i16, ptr %4, align 2, !tbaa !7
  %8 = zext i16 %7 to i32
  %9 = mul i32 %6, %8
  ret i32 %9
}

; Function Attrs: nounwind
define dso_local i32 @square(i16 noundef zeroext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = zext i16 %3 to i32
  %5 = load i16, ptr %2, align 2, !tbaa !7
  %6 = zext i16 %5 to i32
  %7 = mul i32 %4, %6
  ret i32 %7
}

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %5

5:                                                ; preds = %30, %0
  %6 = load i16, ptr %2, align 2, !tbaa !9
  %7 = icmp slt i16 %6, 10
  br i1 %7, label %9, label %8

8:                                                ; preds = %5
  store i32 2, ptr %3, align 2
  call void @llvm.lifetime.end.p0(ptr %2) #3
  br label %33

9:                                                ; preds = %5
  %10 = load i16, ptr %2, align 2, !tbaa !9
  %11 = getelementptr inbounds [10 x i16], ptr @main.values, i16 0, i16 %10
  %12 = load i16, ptr %11, align 2, !tbaa !7
  %13 = call i32 @square(i16 noundef zeroext %12)
  call void @report(i32 noundef %13)
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !9
  br label %14

14:                                               ; preds = %26, %9
  %15 = load i16, ptr %4, align 2, !tbaa !9
  %16 = icmp slt i16 %15, 10
  br i1 %16, label %18, label %17

17:                                               ; preds = %14
  store i32 5, ptr %3, align 2
  call void @llvm.lifetime.end.p0(ptr %4) #3
  br label %29

18:                                               ; preds = %14
  %19 = load i16, ptr %2, align 2, !tbaa !9
  %20 = getelementptr inbounds [10 x i16], ptr @main.values, i16 0, i16 %19
  %21 = load i16, ptr %20, align 2, !tbaa !7
  %22 = load i16, ptr %4, align 2, !tbaa !9
  %23 = getelementptr inbounds [10 x i16], ptr @main.values, i16 0, i16 %22
  %24 = load i16, ptr %23, align 2, !tbaa !7
  %25 = call i32 @product(i16 noundef zeroext %21, i16 noundef zeroext %24)
  call void @report(i32 noundef %25)
  br label %26

26:                                               ; preds = %18
  %27 = load i16, ptr %4, align 2, !tbaa !9
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %4, align 2, !tbaa !9
  br label %14, !llvm.loop !10

29:                                               ; preds = %17
  br label %30

30:                                               ; preds = %29
  %31 = load i16, ptr %2, align 2, !tbaa !9
  %32 = add nsw i16 %31, 1
  store i16 %32, ptr %2, align 2, !tbaa !9
  br label %5, !llvm.loop !12

33:                                               ; preds = %8
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

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
!7 = !{!8, !8, i64 0}
!8 = !{!"short", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
