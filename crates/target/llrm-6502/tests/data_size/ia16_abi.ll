target triple = "msp430"

%struct.S = type { i16, i16, i16 }
%struct.W = type { i16, i16 }

; Function Attrs: nounwind optsize
define dso_local void @three(ptr dead_on_unwind noalias writable sret(%struct.S) align 1 %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %1, ptr %4, align 2, !tbaa !7
  store i16 %2, ptr %5, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 0
  store i16 %6, ptr %7, align 1, !tbaa !8
  %8 = load i16, ptr %5, align 2, !tbaa !7
  %9 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 1
  store i16 %8, ptr %9, align 1, !tbaa !10
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = load i16, ptr %5, align 2, !tbaa !7
  %12 = add nsw i16 %10, %11
  %13 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 2
  store i16 %12, ptr %13, align 1, !tbaa !11
  ret void
}

; Function Attrs: nounwind optsize
define dso_local void @two(ptr dead_on_unwind noalias writable sret(%struct.W) align 1 %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %1, ptr %4, align 2, !tbaa !7
  store i16 %2, ptr %5, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = getelementptr inbounds nuw %struct.W, ptr %0, i32 0, i32 0
  store i16 %6, ptr %7, align 1, !tbaa !12
  %8 = load i16, ptr %5, align 2, !tbaa !7
  %9 = getelementptr inbounds nuw %struct.W, ptr %0, i32 0, i32 1
  store i16 %8, ptr %9, align 1, !tbaa !14
  ret void
}

; Function Attrs: nounwind optsize
define dso_local i32 @wide(i16 noundef %0, i32 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !15
  store i16 %2, ptr %6, align 2, !tbaa !7
  %7 = load i32, ptr %5, align 2, !tbaa !15
  %8 = load i32, ptr %5, align 2, !tbaa !15
  %9 = add nsw i32 %7, %8
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = sext i16 %10 to i32
  %12 = add nsw i32 %9, %11
  %13 = load i16, ptr %6, align 2, !tbaa !7
  %14 = sext i16 %13 to i32
  %15 = add nsw i32 %12, %14
  ret i32 %15
}

; Function Attrs: nounwind optsize
define dso_local i16 @by_value(ptr noundef byval(%struct.S) align 1 %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  store i16 %1, ptr %3, align 2, !tbaa !7
  %4 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 0
  %5 = load i16, ptr %4, align 1, !tbaa !8
  %6 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 1
  %7 = load i16, ptr %6, align 1, !tbaa !10
  %8 = mul nsw i16 %7, 2
  %9 = add nsw i16 %5, %8
  %10 = getelementptr inbounds nuw %struct.S, ptr %0, i32 0, i32 2
  %11 = load i16, ptr %10, align 1, !tbaa !11
  %12 = mul nsw i16 %11, 3
  %13 = add nsw i16 %9, %12
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = mul nsw i16 %14, 4
  %16 = add nsw i16 %13, %15
  ret i16 %16
}

; Function Attrs: nounwind optsize
define dso_local i16 @six(i16 noundef %0, i16 noundef %1, i16 noundef %2, i16 noundef %3, i16 noundef %4, i16 noundef %5) #0 {
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  %12 = alloca i16, align 2
  store i16 %0, ptr %7, align 2, !tbaa !7
  store i16 %1, ptr %8, align 2, !tbaa !7
  store i16 %2, ptr %9, align 2, !tbaa !7
  store i16 %3, ptr %10, align 2, !tbaa !7
  store i16 %4, ptr %11, align 2, !tbaa !7
  store i16 %5, ptr %12, align 2, !tbaa !7
  %13 = load i16, ptr %7, align 2, !tbaa !7
  %14 = load i16, ptr %8, align 2, !tbaa !7
  %15 = mul nsw i16 %14, 2
  %16 = add nsw i16 %13, %15
  %17 = load i16, ptr %9, align 2, !tbaa !7
  %18 = mul nsw i16 %17, 3
  %19 = add nsw i16 %16, %18
  %20 = load i16, ptr %10, align 2, !tbaa !7
  %21 = mul nsw i16 %20, 4
  %22 = add nsw i16 %19, %21
  %23 = load i16, ptr %11, align 2, !tbaa !7
  %24 = mul nsw i16 %23, 5
  %25 = add nsw i16 %22, %24
  %26 = load i16, ptr %12, align 2, !tbaa !7
  %27 = mul nsw i16 %26, 6
  %28 = add nsw i16 %25, %27
  ret i16 %28
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.S, align 1
  %3 = alloca %struct.W, align 1
  %4 = alloca %struct.S, align 1
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @three(ptr dead_on_unwind writable sret(%struct.S) align 1 %2, i16 noundef 7, i16 noundef 8) #4
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @two(ptr dead_on_unwind writable sret(%struct.W) align 1 %3, i16 noundef 3, i16 noundef 4) #4
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %5 = getelementptr inbounds nuw %struct.S, ptr %4, i32 0, i32 0
  store i16 1, ptr %5, align 1, !tbaa !8
  %6 = getelementptr inbounds nuw %struct.S, ptr %4, i32 0, i32 1
  store i16 2, ptr %6, align 1, !tbaa !10
  %7 = getelementptr inbounds nuw %struct.S, ptr %4, i32 0, i32 2
  store i16 3, ptr %7, align 1, !tbaa !11
  %8 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 0
  %9 = load i16, ptr %8, align 1, !tbaa !8
  %10 = mul nsw i16 %9, 100
  %11 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 1
  %12 = load i16, ptr %11, align 1, !tbaa !10
  %13 = mul nsw i16 %12, 10
  %14 = add nsw i16 %10, %13
  %15 = getelementptr inbounds nuw %struct.S, ptr %2, i32 0, i32 2
  %16 = load i16, ptr %15, align 1, !tbaa !11
  %17 = add nsw i16 %14, %16
  %18 = sext i16 %17 to i32
  call void @report(i32 noundef %18) #4
  %19 = getelementptr inbounds nuw %struct.W, ptr %3, i32 0, i32 0
  %20 = load i16, ptr %19, align 1, !tbaa !12
  %21 = mul nsw i16 %20, 10
  %22 = getelementptr inbounds nuw %struct.W, ptr %3, i32 0, i32 1
  %23 = load i16, ptr %22, align 1, !tbaa !14
  %24 = add nsw i16 %21, %23
  %25 = sext i16 %24 to i32
  call void @report(i32 noundef %25) #4
  %26 = call i32 @wide(i16 noundef 1, i32 noundef 131075, i16 noundef 4) #4
  call void @report(i32 noundef %26) #4
  %27 = call i16 @by_value(ptr noundef byval(%struct.S) align 1 %4, i16 noundef 9) #4
  %28 = sext i16 %27 to i32
  call void @report(i32 noundef %28) #4
  %29 = call i16 @six(i16 noundef 1, i16 noundef 2, i16 noundef 3, i16 noundef 4, i16 noundef 5, i16 noundef 6) #4
  %30 = sext i16 %29 to i32
  call void @report(i32 noundef %30) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

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
!8 = !{!9, !4, i64 0}
!9 = !{!"S", !4, i64 0, !4, i64 2, !4, i64 4}
!10 = !{!9, !4, i64 2}
!11 = !{!9, !4, i64 4}
!12 = !{!13, !4, i64 0}
!13 = !{!"W", !4, i64 0, !4, i64 2}
!14 = !{!13, !4, i64 2}
!15 = !{!16, !16, i64 0}
!16 = !{!"long", !5, i64 0}
