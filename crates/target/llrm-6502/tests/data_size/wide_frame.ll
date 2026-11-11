target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @frame(float noundef 1.500000e+00) #3
  ret i16 0
}

; Function Attrs: nounwind optsize
define internal void @frame(float noundef %0) #0 {
  %2 = alloca float, align 2
  %3 = alloca float, align 2
  %4 = alloca [16400 x i16], align 2
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  store float %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  call void @llvm.lifetime.start.p0(ptr %6) #4
  store i32 0, ptr %6, align 2, !tbaa !9
  %7 = getelementptr inbounds [16400 x i16], ptr %4, i16 0, i16 0
  store i16 7, ptr %7, align 2, !tbaa !11
  store i16 16380, ptr %5, align 2, !tbaa !11
  br label %8

8:                                                ; preds = %16, %1
  %9 = load i16, ptr %5, align 2, !tbaa !11
  %10 = icmp slt i16 %9, 16400
  br i1 %10, label %11, label %19

11:                                               ; preds = %8
  %12 = load i16, ptr %5, align 2, !tbaa !11
  %13 = sub nsw i16 %12, 16370
  %14 = load i16, ptr %5, align 2, !tbaa !11
  %15 = getelementptr inbounds [16400 x i16], ptr %4, i16 0, i16 %14
  store i16 %13, ptr %15, align 2, !tbaa !11
  br label %16

16:                                               ; preds = %11
  %17 = load i16, ptr %5, align 2, !tbaa !11
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %5, align 2, !tbaa !11
  br label %8, !llvm.loop !12

19:                                               ; preds = %8
  %20 = load float, ptr %2, align 2, !tbaa !7
  %21 = getelementptr inbounds [16400 x i16], ptr %4, i16 0, i16 0
  %22 = load i16, ptr %21, align 2, !tbaa !11
  %23 = sitofp i16 %22 to float
  %24 = fadd float %20, %23
  %25 = getelementptr inbounds [16400 x i16], ptr %4, i16 0, i16 16399
  %26 = load i16, ptr %25, align 2, !tbaa !11
  %27 = sitofp i16 %26 to float
  %28 = fadd float %24, %27
  store float %28, ptr %3, align 2, !tbaa !7
  %29 = load float, ptr %3, align 2, !tbaa !7
  call void @note(float noundef %29) #3
  store i16 16380, ptr %5, align 2, !tbaa !11
  br label %30

30:                                               ; preds = %40, %19
  %31 = load i16, ptr %5, align 2, !tbaa !11
  %32 = icmp slt i16 %31, 16400
  br i1 %32, label %33, label %43

33:                                               ; preds = %30
  %34 = load i16, ptr %5, align 2, !tbaa !11
  %35 = getelementptr inbounds [16400 x i16], ptr %4, i16 0, i16 %34
  %36 = load i16, ptr %35, align 2, !tbaa !11
  %37 = sext i16 %36 to i32
  %38 = load i32, ptr %6, align 2, !tbaa !9
  %39 = add nsw i32 %38, %37
  store i32 %39, ptr %6, align 2, !tbaa !9
  br label %40

40:                                               ; preds = %33
  %41 = load i16, ptr %5, align 2, !tbaa !11
  %42 = add nsw i16 %41, 1
  store i16 %42, ptr %5, align 2, !tbaa !11
  br label %30, !llvm.loop !14

43:                                               ; preds = %30
  %44 = load i32, ptr %6, align 2, !tbaa !9
  call void @report(i32 noundef %44) #3
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @note(float noundef %0) #0 {
  %2 = alloca float, align 2
  store float %0, ptr %2, align 2, !tbaa !7
  %3 = load float, ptr %2, align 2, !tbaa !7
  %4 = fmul float %3, 2.000000e+00
  %5 = fptosi float %4 to i32
  call void @report(i32 noundef %5) #3
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!7 = !{!8, !8, i64 0}
!8 = !{!"float", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !5, i64 0}
!11 = !{!4, !4, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
