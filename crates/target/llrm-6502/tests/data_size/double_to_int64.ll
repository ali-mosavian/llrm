target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

%union.halves = type { i64 }

@main.values = internal constant [9 x double] [double 0.000000e+00, double 1.900000e+00, double -1.900000e+00, double f0x40FE240C00000000, double f0xC0FE240C00000000, double f0x41F0000000080000, double f0xC1F0000000180000, double f0x4270000000000000, double -5.000000e+12], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  %5 = alloca %union.halves, align 1
  %6 = alloca %union.halves, align 1
  %7 = alloca %union.halves, align 1
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !9
  br label %8

8:                                                ; preds = %26, %0
  %9 = load i16, ptr %4, align 2, !tbaa !9
  %10 = icmp slt i16 %9, 9
  br i1 %10, label %11, label %29

11:                                               ; preds = %8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %12 = load i16, ptr %4, align 2, !tbaa !9
  %13 = getelementptr inbounds [9 x double], ptr @main.values, i16 0, i16 %12
  %14 = load double, ptr %13, align 2, !tbaa !10
  %15 = call i64 @trunc_double(double noundef %14) #4
  store i64 %15, ptr %5, align 1, !tbaa !12
  %16 = load i32, ptr %2, align 2, !tbaa !7
  %17 = mul nsw i32 %16, 31
  %18 = getelementptr inbounds [2 x i32], ptr %5, i16 0, i16 0
  %19 = load i32, ptr %18, align 1, !tbaa !12
  %20 = add nsw i32 %17, %19
  store i32 %20, ptr %2, align 2, !tbaa !7
  %21 = load i32, ptr %3, align 2, !tbaa !7
  %22 = mul nsw i32 %21, 31
  %23 = getelementptr inbounds [2 x i32], ptr %5, i16 0, i16 1
  %24 = load i32, ptr %23, align 1, !tbaa !12
  %25 = add nsw i32 %22, %24
  store i32 %25, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %5) #3
  br label %26

26:                                               ; preds = %11
  %27 = load i16, ptr %4, align 2, !tbaa !9
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %4, align 2, !tbaa !9
  br label %8, !llvm.loop !13

29:                                               ; preds = %8
  %30 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %30) #4
  %31 = load i32, ptr %3, align 2, !tbaa !7
  call void @report(i32 noundef %31) #4
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %32 = call i64 @trunc_float(float noundef f0x51000000) #4
  store i64 %32, ptr %6, align 1, !tbaa !12
  %33 = call i64 @trunc_float(float noundef -3.750000e+00) #4
  store i64 %33, ptr %7, align 1, !tbaa !12
  %34 = getelementptr inbounds [2 x i32], ptr %6, i16 0, i16 1
  %35 = load i32, ptr %34, align 1, !tbaa !12
  %36 = shl i32 %35, 2
  %37 = getelementptr inbounds [2 x i32], ptr %6, i16 0, i16 0
  %38 = load i32, ptr %37, align 1, !tbaa !12
  %39 = lshr i32 %38, 30
  %40 = or i32 %36, %39
  call void @report(i32 noundef %40) #4
  %41 = getelementptr inbounds [2 x i32], ptr %7, i16 0, i16 0
  %42 = load i32, ptr %41, align 1, !tbaa !12
  call void @report(i32 noundef %42) #4
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i64 @trunc_double(double noundef %0) #0 {
  %2 = alloca double, align 2
  store double %0, ptr %2, align 2, !tbaa !10
  %3 = load double, ptr %2, align 2, !tbaa !10
  %4 = fptosi double %3 to i64
  ret i64 %4
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i64 @trunc_float(float noundef %0) #0 {
  %2 = alloca float, align 2
  store float %0, ptr %2, align 2, !tbaa !15
  %3 = load float, ptr %2, align 2, !tbaa !15
  %4 = fptosi float %3 to i64
  ret i64 %4
}

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
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"double", !5, i64 0}
!12 = !{!5, !5, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = !{!16, !16, i64 0}
!16 = !{!"float", !5, i64 0}
