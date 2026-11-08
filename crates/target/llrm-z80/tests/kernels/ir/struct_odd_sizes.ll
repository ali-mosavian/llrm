target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = '/home/alim/scratch/wt-z80rot/tests/run/c/struct_odd_sizes.c'
source_filename = "/home/alim/scratch/wt-z80rot/tests/run/c/struct_odd_sizes.c"
target triple = "msp430"

%struct.tiny = type { i8, i8, i8 }
%struct.five = type { i8, i8, i8, i8, i8 }
%struct.seven = type { i8, i8, i8, i8, i8, i8, i8 }

; Function Attrs: nounwind
define dso_local i16 @three(i16 noundef %0, ptr noundef byval(%struct.tiny) align 1 %1, ptr noundef byval(%struct.tiny) align 1 %2, i32 noundef %3) #0 {
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  store i16 %0, ptr %5, align 2, !tbaa !7
  store i32 %3, ptr %6, align 2, !tbaa !8
  %7 = load i16, ptr %5, align 2, !tbaa !7
  %8 = getelementptr inbounds nuw %struct.tiny, ptr %1, i32 0, i32 0
  %9 = load i8, ptr %8, align 1, !tbaa !10
  %10 = sext i8 %9 to i16
  %11 = mul nsw i16 %10, 10
  %12 = add nsw i16 %7, %11
  %13 = getelementptr inbounds nuw %struct.tiny, ptr %1, i32 0, i32 1
  %14 = load i8, ptr %13, align 1, !tbaa !12
  %15 = sext i8 %14 to i16
  %16 = mul nsw i16 %15, 100
  %17 = add nsw i16 %12, %16
  %18 = getelementptr inbounds nuw %struct.tiny, ptr %1, i32 0, i32 2
  %19 = load i8, ptr %18, align 1, !tbaa !13
  %20 = sext i8 %19 to i16
  %21 = add nsw i16 %17, %20
  %22 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 0
  %23 = load i8, ptr %22, align 1, !tbaa !10
  %24 = sext i8 %23 to i16
  %25 = mul nsw i16 %24, 3
  %26 = add nsw i16 %21, %25
  %27 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 1
  %28 = load i8, ptr %27, align 1, !tbaa !12
  %29 = sext i8 %28 to i16
  %30 = mul nsw i16 %29, 5
  %31 = add nsw i16 %26, %30
  %32 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 2
  %33 = load i8, ptr %32, align 1, !tbaa !13
  %34 = sext i8 %33 to i16
  %35 = mul nsw i16 %34, 7
  %36 = add nsw i16 %31, %35
  %37 = load i32, ptr %6, align 2, !tbaa !8
  %38 = trunc i32 %37 to i16
  %39 = add nsw i16 %36, %38
  ret i16 %39
}

; Function Attrs: nounwind
define dso_local i16 @fives(ptr noundef byval(%struct.five) align 1 %0, ptr noundef byval(%struct.seven) align 1 %1) #0 {
  %3 = getelementptr inbounds nuw %struct.five, ptr %0, i32 0, i32 0
  %4 = load i8, ptr %3, align 1, !tbaa !14
  %5 = sext i8 %4 to i16
  %6 = getelementptr inbounds nuw %struct.five, ptr %0, i32 0, i32 4
  %7 = load i8, ptr %6, align 1, !tbaa !16
  %8 = sext i8 %7 to i16
  %9 = mul nsw i16 %8, 2
  %10 = add nsw i16 %5, %9
  %11 = getelementptr inbounds nuw %struct.seven, ptr %1, i32 0, i32 0
  %12 = load i8, ptr %11, align 1, !tbaa !17
  %13 = sext i8 %12 to i16
  %14 = mul nsw i16 %13, 3
  %15 = add nsw i16 %10, %14
  %16 = getelementptr inbounds nuw %struct.seven, ptr %1, i32 0, i32 6
  %17 = load i8, ptr %16, align 1, !tbaa !19
  %18 = sext i8 %17 to i16
  %19 = mul nsw i16 %18, 5
  %20 = add nsw i16 %15, %19
  %21 = getelementptr inbounds nuw %struct.seven, ptr %1, i32 0, i32 3
  %22 = load i8, ptr %21, align 1, !tbaa !20
  %23 = sext i8 %22 to i16
  %24 = add nsw i16 %20, %23
  ret i16 %24
}

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.tiny, align 1
  %3 = alloca %struct.tiny, align 1
  %4 = alloca %struct.five, align 1
  %5 = alloca %struct.seven, align 1
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %6 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 0
  store i8 1, ptr %6, align 1, !tbaa !10
  %7 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 1
  store i8 2, ptr %7, align 1, !tbaa !12
  %8 = getelementptr inbounds nuw %struct.tiny, ptr %2, i32 0, i32 2
  store i8 3, ptr %8, align 1, !tbaa !13
  %9 = getelementptr inbounds nuw %struct.tiny, ptr %3, i32 0, i32 0
  store i8 4, ptr %9, align 1, !tbaa !10
  %10 = getelementptr inbounds nuw %struct.tiny, ptr %3, i32 0, i32 1
  store i8 5, ptr %10, align 1, !tbaa !12
  %11 = getelementptr inbounds nuw %struct.tiny, ptr %3, i32 0, i32 2
  store i8 6, ptr %11, align 1, !tbaa !13
  %12 = getelementptr inbounds nuw %struct.five, ptr %4, i32 0, i32 0
  store i8 1, ptr %12, align 1, !tbaa !14
  %13 = getelementptr inbounds nuw %struct.five, ptr %4, i32 0, i32 1
  store i8 2, ptr %13, align 1, !tbaa !21
  %14 = getelementptr inbounds nuw %struct.five, ptr %4, i32 0, i32 2
  store i8 3, ptr %14, align 1, !tbaa !22
  %15 = getelementptr inbounds nuw %struct.five, ptr %4, i32 0, i32 3
  store i8 4, ptr %15, align 1, !tbaa !23
  %16 = getelementptr inbounds nuw %struct.five, ptr %4, i32 0, i32 4
  store i8 5, ptr %16, align 1, !tbaa !16
  %17 = getelementptr inbounds nuw %struct.seven, ptr %5, i32 0, i32 0
  store i8 1, ptr %17, align 1, !tbaa !17
  %18 = getelementptr inbounds nuw %struct.seven, ptr %5, i32 0, i32 1
  store i8 2, ptr %18, align 1, !tbaa !24
  %19 = getelementptr inbounds nuw %struct.seven, ptr %5, i32 0, i32 2
  store i8 3, ptr %19, align 1, !tbaa !25
  %20 = getelementptr inbounds nuw %struct.seven, ptr %5, i32 0, i32 3
  store i8 4, ptr %20, align 1, !tbaa !20
  %21 = getelementptr inbounds nuw %struct.seven, ptr %5, i32 0, i32 4
  store i8 5, ptr %21, align 1, !tbaa !26
  %22 = getelementptr inbounds nuw %struct.seven, ptr %5, i32 0, i32 5
  store i8 6, ptr %22, align 1, !tbaa !27
  %23 = getelementptr inbounds nuw %struct.seven, ptr %5, i32 0, i32 6
  store i8 7, ptr %23, align 1, !tbaa !19
  %24 = call i16 @three(i16 noundef 1, ptr noundef byval(%struct.tiny) align 1 %2, ptr noundef byval(%struct.tiny) align 1 %3, i32 noundef 100)
  %25 = sext i16 %24 to i32
  call void @report(i32 noundef %25)
  %26 = call i16 @fives(ptr noundef byval(%struct.five) align 1 %4, ptr noundef byval(%struct.seven) align 1 %5)
  %27 = sext i16 %26 to i32
  call void @report(i32 noundef %27)
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = !{!11, !5, i64 0}
!11 = !{!"tiny", !5, i64 0, !5, i64 1, !5, i64 2}
!12 = !{!11, !5, i64 1}
!13 = !{!11, !5, i64 2}
!14 = !{!15, !5, i64 0}
!15 = !{!"five", !5, i64 0, !5, i64 1, !5, i64 2, !5, i64 3, !5, i64 4}
!16 = !{!15, !5, i64 4}
!17 = !{!18, !5, i64 0}
!18 = !{!"seven", !5, i64 0, !5, i64 1, !5, i64 2, !5, i64 3, !5, i64 4, !5, i64 5, !5, i64 6}
!19 = !{!18, !5, i64 6}
!20 = !{!18, !5, i64 3}
!21 = !{!15, !5, i64 1}
!22 = !{!15, !5, i64 2}
!23 = !{!15, !5, i64 3}
!24 = !{!18, !5, i64 1}
!25 = !{!18, !5, i64 2}
!26 = !{!18, !5, i64 4}
!27 = !{!18, !5, i64 5}
