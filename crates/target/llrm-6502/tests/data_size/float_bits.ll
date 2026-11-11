target triple = "msp430"

%union.D = type { double }
%union.F = type { float }

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca double, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store double 1.500000e+00, ptr %2, align 2, !tbaa !7
  %3 = load double, ptr %2, align 2, !tbaa !7
  %4 = call i32 @hi(double noundef %3) #4
  %5 = lshr i32 %4, 12
  call void @report(i32 noundef %5) #4
  %6 = load double, ptr %2, align 2, !tbaa !7
  %7 = call i32 @lo(double noundef %6) #4
  call void @report(i32 noundef %7) #4
  %8 = call i32 @hi(double noundef -2.000000e+00) #4
  %9 = lshr i32 %8, 16
  call void @report(i32 noundef %9) #4
  %10 = call double @join(i32 noundef 1074790400, i32 noundef 0) #4
  %11 = fmul double %10, 3.000000e+00
  %12 = fptosi double %11 to i32
  call void @report(i32 noundef %12) #4
  %13 = call i32 @fbits(float noundef 1.000000e+00) #4
  %14 = lshr i32 %13, 8
  call void @report(i32 noundef %14) #4
  %15 = call float @ffrom(i32 noundef 1092616192) #4
  %16 = fmul float %15, 2.000000e+00
  %17 = fptosi float %16 to i32
  call void @report(i32 noundef %17) #4
  %18 = call i32 @fbits(float noundef 7.000000e+00) #4
  %19 = xor i32 %18, -2147483648
  %20 = call float @ffrom(i32 noundef %19) #4
  %21 = fptosi float %20 to i32
  call void @report(i32 noundef %21) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @hi(double noundef %0) #0 {
  %2 = alloca double, align 2
  %3 = alloca %union.D, align 1
  store double %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load double, ptr %2, align 2, !tbaa !7
  store double %4, ptr %3, align 1, !tbaa !9
  %5 = getelementptr inbounds [2 x i32], ptr %3, i16 0, i16 1
  %6 = load i32, ptr %5, align 1, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %6
}

; Function Attrs: nounwind optsize
define internal i32 @lo(double noundef %0) #0 {
  %2 = alloca double, align 2
  %3 = alloca %union.D, align 1
  store double %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load double, ptr %2, align 2, !tbaa !7
  store double %4, ptr %3, align 1, !tbaa !9
  %5 = getelementptr inbounds [2 x i32], ptr %3, i16 0, i16 0
  %6 = load i32, ptr %5, align 1, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %6
}

; Function Attrs: nounwind optsize
define internal double @join(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca %union.D, align 1
  store i32 %0, ptr %3, align 2, !tbaa !10
  store i32 %1, ptr %4, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %6 = load i32, ptr %3, align 2, !tbaa !10
  %7 = getelementptr inbounds [2 x i32], ptr %5, i16 0, i16 1
  store i32 %6, ptr %7, align 1, !tbaa !9
  %8 = load i32, ptr %4, align 2, !tbaa !10
  %9 = getelementptr inbounds [2 x i32], ptr %5, i16 0, i16 0
  store i32 %8, ptr %9, align 1, !tbaa !9
  %10 = load double, ptr %5, align 1, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret double %10
}

; Function Attrs: nounwind optsize
define internal i32 @fbits(float noundef %0) #0 {
  %2 = alloca float, align 2
  %3 = alloca %union.F, align 1
  store float %0, ptr %2, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load float, ptr %2, align 2, !tbaa !12
  store float %4, ptr %3, align 1, !tbaa !9
  %5 = load i32, ptr %3, align 1, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %5
}

; Function Attrs: nounwind optsize
define internal float @ffrom(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca %union.F, align 1
  store i32 %0, ptr %2, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i32, ptr %2, align 2, !tbaa !10
  store i32 %4, ptr %3, align 1, !tbaa !9
  %5 = load float, ptr %3, align 1, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret float %5
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
!7 = !{!8, !8, i64 0}
!8 = !{!"double", !5, i64 0}
!9 = !{!5, !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"float", !5, i64 0}
