target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = '/home/alim/scratch/wt-z80f32/tests/run/c/byval_inline.c'
source_filename = "/home/alim/scratch/wt-z80f32/tests/run/c/byval_inline.c"
target triple = "msp430"

%struct.big = type { [10 x i32] }

@shared = dso_local global %struct.big zeroinitializer, align 1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca %struct.big, align 1
  %3 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %16, %0
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = icmp slt i16 %5, 10
  br i1 %6, label %7, label %19

7:                                                ; preds = %4
  %8 = load i16, ptr %3, align 2, !tbaa !7
  %9 = add nsw i16 %8, 1
  %10 = sext i16 %9 to i32
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = getelementptr inbounds [10 x i32], ptr @shared, i16 0, i16 %11
  store i32 %10, ptr %12, align 1, !tbaa !8
  %13 = getelementptr inbounds nuw %struct.big, ptr %2, i32 0, i32 0
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = getelementptr inbounds [10 x i32], ptr %13, i16 0, i16 %14
  store i32 %10, ptr %15, align 1, !tbaa !8
  br label %16

16:                                               ; preds = %7
  %17 = load i16, ptr %3, align 2, !tbaa !7
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !10

19:                                               ; preds = %4
  %20 = call i32 @bump(ptr noundef byval(%struct.big) align 1 %2) #4
  call void @report(i32 noundef %20) #4
  %21 = getelementptr inbounds nuw %struct.big, ptr %2, i32 0, i32 0
  %22 = getelementptr inbounds [10 x i32], ptr %21, i16 0, i16 0
  %23 = load i32, ptr %22, align 1, !tbaa !8
  call void @report(i32 noundef %23) #4
  %24 = call i32 @bump(ptr noundef byval(%struct.big) align 1 @shared) #4
  call void @report(i32 noundef %24) #4
  %25 = load i32, ptr @shared, align 1, !tbaa !8
  call void @report(i32 noundef %25) #4
  %26 = call i32 @peek(ptr noundef byval(%struct.big) align 1 %2) #4
  call void @report(i32 noundef %26) #4
  %27 = getelementptr inbounds nuw %struct.big, ptr %2, i32 0, i32 0
  %28 = getelementptr inbounds [10 x i32], ptr %27, i16 0, i16 3
  store i32 1000, ptr %28, align 1, !tbaa !8
  %29 = call i32 @peek(ptr noundef byval(%struct.big) align 1 %2) #4
  call void @report(i32 noundef %29) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @bump(ptr noundef byval(%struct.big) align 1 %0) #0 {
  %2 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %3 = getelementptr inbounds [10 x i32], ptr %2, i16 0, i16 0
  store i32 99, ptr %3, align 1, !tbaa !8
  %4 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %5 = getelementptr inbounds [10 x i32], ptr %4, i16 0, i16 0
  %6 = load i32, ptr %5, align 1, !tbaa !8
  %7 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %8 = getelementptr inbounds [10 x i32], ptr %7, i16 0, i16 9
  %9 = load i32, ptr %8, align 1, !tbaa !8
  %10 = add nsw i32 %6, %9
  ret i32 %10
}

; Function Attrs: nounwind optsize
define internal i32 @peek(ptr noundef byval(%struct.big) align 1 %0) #0 {
  %2 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %3 = getelementptr inbounds [10 x i32], ptr %2, i16 0, i16 3
  %4 = load i32, ptr %3, align 1, !tbaa !8
  %5 = getelementptr inbounds nuw %struct.big, ptr %0, i32 0, i32 0
  %6 = getelementptr inbounds [10 x i32], ptr %5, i16 0, i16 9
  %7 = load i32, ptr %6, align 1, !tbaa !8
  %8 = add nsw i32 %4, %7
  ret i32 %8
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
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
