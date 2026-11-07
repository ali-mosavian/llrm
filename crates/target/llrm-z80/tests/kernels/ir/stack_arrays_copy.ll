target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = 'tests/run/c/stack_arrays_copy.c'
source_filename = "tests/run/c/stack_arrays_copy.c"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i16 @bench(i16 noundef 63) #3
  %3 = mul nsw i16 %2, 100
  %4 = call i16 @bench(i16 noundef 5) #3
  %5 = add nsw i16 %3, %4
  %6 = sext i16 %5 to i32
  call void @report(i32 noundef %6) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define internal i16 @bench(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [70 x i8], align 1
  %4 = alloca [70 x i8], align 1
  %5 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %14, %1
  %7 = load i16, ptr %5, align 2, !tbaa !7
  %8 = icmp slt i16 %7, 70
  br i1 %8, label %9, label %17

9:                                                ; preds = %6
  %10 = load i16, ptr %5, align 2, !tbaa !7
  %11 = trunc i16 %10 to i8
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [70 x i8], ptr %3, i16 0, i16 %12
  store i8 %11, ptr %13, align 1, !tbaa !8
  br label %14

14:                                               ; preds = %9
  %15 = load i16, ptr %5, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %5, align 2, !tbaa !7
  br label %6, !llvm.loop !9

17:                                               ; preds = %6
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %18

18:                                               ; preds = %29, %17
  %19 = load i16, ptr %5, align 2, !tbaa !7
  %20 = load i16, ptr %2, align 2, !tbaa !7
  %21 = and i16 %20, 63
  %22 = icmp slt i16 %19, %21
  br i1 %22, label %23, label %32

23:                                               ; preds = %18
  %24 = load i16, ptr %5, align 2, !tbaa !7
  %25 = getelementptr inbounds [70 x i8], ptr %3, i16 0, i16 %24
  %26 = load i8, ptr %25, align 1, !tbaa !8
  %27 = load i16, ptr %5, align 2, !tbaa !7
  %28 = getelementptr inbounds [70 x i8], ptr %4, i16 0, i16 %27
  store i8 %26, ptr %28, align 1, !tbaa !8
  br label %29

29:                                               ; preds = %23
  %30 = load i16, ptr %5, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %5, align 2, !tbaa !7
  br label %18, !llvm.loop !11

32:                                               ; preds = %18
  %33 = getelementptr inbounds [70 x i8], ptr %4, i16 0, i16 3
  %34 = load i8, ptr %33, align 1, !tbaa !8
  %35 = sext i8 %34 to i16
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret i16 %35
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize }
attributes #4 = { nounwind }

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
