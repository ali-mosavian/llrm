target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

source_filename = "byval_struct.c"
target triple = "msp430"

%struct.big = type { [10 x i32] }
%struct.odd = type { [41 x i8] }

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.big, align 1
  %3 = alloca %struct.odd, align 1
  %4 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %15, %0
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 10
  br i1 %7, label %8, label %18

8:                                                ; preds = %5
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = add nsw i16 %9, 1
  %11 = sext i16 %10 to i32
  %12 = getelementptr inbounds nuw %struct.big, ptr %2, i32 0, i32 0
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = getelementptr inbounds [10 x i32], ptr %12, i16 0, i16 %13
  store i32 %11, ptr %14, align 1, !tbaa !8
  br label %15

15:                                               ; preds = %8
  %16 = load i16, ptr %4, align 2, !tbaa !7
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !10

18:                                               ; preds = %5
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %28, %18
  %20 = load i16, ptr %4, align 2, !tbaa !7
  %21 = icmp slt i16 %20, 41
  br i1 %21, label %22, label %31

22:                                               ; preds = %19
  %23 = load i16, ptr %4, align 2, !tbaa !7
  %24 = trunc i16 %23 to i8
  %25 = getelementptr inbounds nuw %struct.odd, ptr %3, i32 0, i32 0
  %26 = load i16, ptr %4, align 2, !tbaa !7
  %27 = getelementptr inbounds [41 x i8], ptr %25, i16 0, i16 %26
  store i8 %24, ptr %27, align 1, !tbaa !13
  br label %28

28:                                               ; preds = %22
  %29 = load i16, ptr %4, align 2, !tbaa !7
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %4, align 2, !tbaa !7
  br label %19, !llvm.loop !14

31:                                               ; preds = %19
  %32 = call i32 @sum(ptr noundef byval(%struct.big) align 1 %2, i32 noundef 100)
  call void @report(i32 noundef %32)
  %33 = getelementptr inbounds nuw %struct.big, ptr %2, i32 0, i32 0
  %34 = getelementptr inbounds [10 x i32], ptr %33, i16 0, i16 0
  %35 = load i32, ptr %34, align 1, !tbaa !8
  call void @report(i32 noundef %35)
  %36 = call i32 @again(i32 noundef 1000, ptr noundef byval(%struct.big) align 1 %2, ptr noundef byval(%struct.odd) align 1 %3, i32 noundef 7)
  call void @report(i32 noundef %36)
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind
define internal i32 @sum(ptr noundef byval(%struct.big) align 1 %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  store i32 %1, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %6 = load i32, ptr %3, align 2, !tbaa !8
  store i32 %6, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %17, %2
  %8 = load i16, ptr %5, align 2, !tbaa !7
  %9 = icmp slt i16 %8, 10
  br i1 %9, label %10, label %20

10:                                               ; preds = %7
  %11 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [10 x i32], ptr %11, i16 0, i16 %12
  %14 = load i32, ptr %13, align 1, !tbaa !8
  %15 = load i32, ptr %4, align 2, !tbaa !8
  %16 = add nsw i32 %15, %14
  store i32 %16, ptr %4, align 2, !tbaa !8
  br label %17

17:                                               ; preds = %10
  %18 = load i16, ptr %5, align 2, !tbaa !7
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %5, align 2, !tbaa !7
  br label %7, !llvm.loop !15

20:                                               ; preds = %7
  %21 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %22 = getelementptr inbounds [10 x i32], ptr %21, i16 0, i16 0
  store i32 99, ptr %22, align 1, !tbaa !8
  %23 = load i32, ptr %4, align 2, !tbaa !8
  %24 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %25 = getelementptr inbounds [10 x i32], ptr %24, i16 0, i16 0
  %26 = load i32, ptr %25, align 1, !tbaa !8
  %27 = add nsw i32 %23, %26
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  ret i32 %27
}

; Function Attrs: nounwind
define internal i32 @again(i32 noundef %0, ptr noundef byval(%struct.big) align 1 %1, ptr noundef byval(%struct.odd) align 1 %2, i32 noundef %3) #0 {
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %5, align 2, !tbaa !8
  store i32 %3, ptr %6, align 2, !tbaa !8
  %7 = load i32, ptr %5, align 2, !tbaa !8
  %8 = call i32 @sum(ptr noundef byval(%struct.big) align 1 %1, i32 noundef %7)
  %9 = getelementptr inbounds nuw %struct.odd, ptr %2, i32 0, i32 0
  %10 = getelementptr inbounds [41 x i8], ptr %9, i16 0, i16 40
  %11 = load i8, ptr %10, align 1, !tbaa !13
  %12 = sext i8 %11 to i32
  %13 = add nsw i32 %8, %12
  %14 = load i32, ptr %6, align 2, !tbaa !8
  %15 = add nsw i32 %13, %14
  ret i32 %15
}

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
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
!13 = !{!5, !5, i64 0}
!14 = distinct !{!14, !11, !12}
!15 = distinct !{!15, !11, !12}
