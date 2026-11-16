; ModuleID = 'tests/run/c/memmove_overlap.c'
source_filename = "tests/run/c/memmove_overlap.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@buf = dso_local global [32 x i8] zeroinitializer, align 1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 10, ptr %2, align 2, !tbaa !7
  call void @fill()
  %3 = load i16, ptr %2, align 2, !tbaa !7
  call void @down(i16 noundef %3)
  %4 = call i32 @sum(i16 noundef 0, i16 noundef 16)
  call void @report(i32 noundef %4)
  call void @fill()
  %5 = load i16, ptr %2, align 2, !tbaa !7
  call void @up(i16 noundef %5)
  %6 = call i32 @sum(i16 noundef 0, i16 noundef 16)
  call void @report(i32 noundef %6)
  call void @fill()
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = add i16 %7, 17
  call void @down(i16 noundef %8)
  %9 = call i32 @sum(i16 noundef 18, i16 noundef 32)
  call void @report(i32 noundef %9)
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal void @fill() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %2

2:                                                ; preds = %11, %0
  %3 = load i16, ptr %1, align 2, !tbaa !7
  %4 = icmp slt i16 %3, 32
  br i1 %4, label %5, label %14

5:                                                ; preds = %2
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = add nsw i16 %6, 1
  %8 = trunc i16 %7 to i8
  %9 = load i16, ptr %1, align 2, !tbaa !7
  %10 = getelementptr inbounds [32 x i8], ptr @buf, i16 0, i16 %9
  store i8 %8, ptr %10, align 1, !tbaa !8
  br label %11

11:                                               ; preds = %5
  %12 = load i16, ptr %1, align 2, !tbaa !7
  %13 = add nsw i16 %12, 1
  store i16 %13, ptr %1, align 2, !tbaa !7
  br label %2, !llvm.loop !9

14:                                               ; preds = %2
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nounwind
define internal void @down(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i16, ptr %2, align 2, !tbaa !7
  store i16 %4, ptr %3, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %16, %1
  %6 = load i16, ptr %3, align 2, !tbaa !7
  %7 = icmp ugt i16 %6, 0
  br i1 %7, label %8, label %19

8:                                                ; preds = %5
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = sub i16 %9, 1
  %11 = getelementptr inbounds nuw [32 x i8], ptr @buf, i16 0, i16 %10
  %12 = load i8, ptr %11, align 1, !tbaa !8
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = add i16 %13, 2
  %15 = getelementptr inbounds nuw [32 x i8], ptr @buf, i16 0, i16 %14
  store i8 %12, ptr %15, align 1, !tbaa !8
  br label %16

16:                                               ; preds = %8
  %17 = load i16, ptr %3, align 2, !tbaa !7
  %18 = add i16 %17, -1
  store i16 %18, ptr %3, align 2, !tbaa !7
  br label %5, !llvm.loop !11

19:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind
define internal i32 @sum(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %7 = load i16, ptr %3, align 2, !tbaa !7
  store i16 %7, ptr %6, align 2, !tbaa !7
  br label %8

8:                                                ; preds = %20, %2
  %9 = load i16, ptr %6, align 2, !tbaa !7
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = icmp slt i16 %9, %10
  br i1 %11, label %12, label %23

12:                                               ; preds = %8
  %13 = load i32, ptr %5, align 2, !tbaa !12
  %14 = mul nsw i32 %13, 3
  %15 = load i16, ptr %6, align 2, !tbaa !7
  %16 = getelementptr inbounds [32 x i8], ptr @buf, i16 0, i16 %15
  %17 = load i8, ptr %16, align 1, !tbaa !8
  %18 = sext i8 %17 to i32
  %19 = add nsw i32 %14, %18
  store i32 %19, ptr %5, align 2, !tbaa !12
  br label %20

20:                                               ; preds = %12
  %21 = load i16, ptr %6, align 2, !tbaa !7
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %6, align 2, !tbaa !7
  br label %8, !llvm.loop !14

23:                                               ; preds = %8
  %24 = load i32, ptr %5, align 2, !tbaa !12
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %24
}

; Function Attrs: nounwind
define internal void @up(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %15, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp ult i16 %5, %6
  br i1 %7, label %8, label %18

8:                                                ; preds = %4
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = add i16 %9, 3
  %11 = getelementptr inbounds nuw [32 x i8], ptr @buf, i16 0, i16 %10
  %12 = load i8, ptr %11, align 1, !tbaa !8
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = getelementptr inbounds nuw [32 x i8], ptr @buf, i16 0, i16 %13
  store i8 %12, ptr %14, align 1, !tbaa !8
  br label %15

15:                                               ; preds = %8
  %16 = load i16, ptr %3, align 2, !tbaa !7
  %17 = add i16 %16, 1
  store i16 %17, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !15

18:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
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
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = !{!13, !13, i64 0}
!13 = !{!"long", !5, i64 0}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
