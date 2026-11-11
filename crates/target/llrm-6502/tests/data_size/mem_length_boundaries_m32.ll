target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@src = dso_local global [20000 x i8] zeroinitializer, align 1
@dst = dso_local global [20000 x i8] zeroinitializer, align 1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %16, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp ult i16 %4, 20000
  br i1 %5, label %6, label %19

6:                                                ; preds = %3
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = and i16 %7, 3
  %9 = load i16, ptr %2, align 2, !tbaa !7
  %10 = lshr i16 %9, 2
  %11 = and i16 %10, 1
  %12 = add i16 %8, %11
  %13 = trunc i16 %12 to i8
  %14 = load i16, ptr %2, align 2, !tbaa !7
  %15 = getelementptr inbounds nuw [20000 x i8], ptr @src, i16 0, i16 %14
  store i8 %13, ptr %15, align 1, !tbaa !8
  br label %16

16:                                               ; preds = %6
  %17 = load i16, ptr %2, align 2, !tbaa !7
  %18 = add i16 %17, 1
  store i16 %18, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !9

19:                                               ; preds = %3
  call void @fill() #4
  %20 = call i32 @total() #4
  call void @report(i32 noundef %20) #4
  call void @copy() #4
  %21 = call i32 @total() #4
  call void @report(i32 noundef %21) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @fill() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %2

2:                                                ; preds = %8, %0
  %3 = load i16, ptr %1, align 2, !tbaa !7
  %4 = icmp ult i16 %3, 20000
  br i1 %4, label %5, label %11

5:                                                ; preds = %2
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = getelementptr inbounds nuw [20000 x i8], ptr @dst, i16 0, i16 %6
  store i8 2, ptr %7, align 1, !tbaa !8
  br label %8

8:                                                ; preds = %5
  %9 = load i16, ptr %1, align 2, !tbaa !7
  %10 = add i16 %9, 1
  store i16 %10, ptr %1, align 2, !tbaa !7
  br label %2, !llvm.loop !11

11:                                               ; preds = %2
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @total() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i32 0, ptr %1, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %13, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp ult i16 %4, 20000
  br i1 %5, label %6, label %16

6:                                                ; preds = %3
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = getelementptr inbounds nuw [20000 x i8], ptr @dst, i16 0, i16 %7
  %9 = load i8, ptr %8, align 1, !tbaa !8
  %10 = zext i8 %9 to i32
  %11 = load i32, ptr %1, align 2, !tbaa !12
  %12 = add i32 %11, %10
  store i32 %12, ptr %1, align 2, !tbaa !12
  br label %13

13:                                               ; preds = %6
  %14 = load i16, ptr %2, align 2, !tbaa !7
  %15 = add i16 %14, 1
  store i16 %15, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !14

16:                                               ; preds = %3
  %17 = load i32, ptr %1, align 2, !tbaa !12
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %17
}

; Function Attrs: nounwind optsize
define internal void @copy() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %2

2:                                                ; preds = %11, %0
  %3 = load i16, ptr %1, align 2, !tbaa !7
  %4 = icmp ult i16 %3, 20000
  br i1 %4, label %5, label %14

5:                                                ; preds = %2
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = getelementptr inbounds nuw [20000 x i8], ptr @src, i16 0, i16 %6
  %8 = load i8, ptr %7, align 1, !tbaa !8
  %9 = load i16, ptr %1, align 2, !tbaa !7
  %10 = getelementptr inbounds nuw [20000 x i8], ptr @dst, i16 0, i16 %9
  store i8 %8, ptr %10, align 1, !tbaa !8
  br label %11

11:                                               ; preds = %5
  %12 = load i16, ptr %1, align 2, !tbaa !7
  %13 = add i16 %12, 1
  store i16 %13, ptr %1, align 2, !tbaa !7
  br label %2, !llvm.loop !15

14:                                               ; preds = %2
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

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
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = !{!13, !13, i64 0}
!13 = !{!"long", !5, i64 0}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
