target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@__const.main.d = private unnamed_addr constant [8 x double] [double 0.000000e+00, double 1.500000e+00, double f0x41EFFFFFFFE00000, double f0x41F0000000000000, double f0x43E0000000000000, double f0x43E0000000000000, double f0x43E0000000000001, double f0x43EFFFFFFFFFFFFF], align 2
@__const.main.f = private unnamed_addr constant [4 x float] [float 3.900000e+00, float f0x4F800000, float f0x5F000000, float f0x5F7FFFFF], align 2

; Function Attrs: nounwind optsize
define dso_local i64 @from_double(double noundef %0) #0 {
  %2 = alloca double, align 2
  store double %0, ptr %2, align 2, !tbaa !8
  %3 = load double, ptr %2, align 2, !tbaa !8
  %4 = fptoui double %3 to i64
  ret i64 %4
}

; Function Attrs: nounwind optsize
define dso_local i64 @from_float(float noundef %0) #0 {
  %2 = alloca float, align 2
  store float %0, ptr %2, align 2, !tbaa !10
  %3 = load float, ptr %2, align 2, !tbaa !10
  %4 = fptoui float %3 to i64
  ret i64 %4
}

; Function Attrs: nounwind optsize
define dso_local i64 @from_long_double(double noundef %0) #0 {
  %2 = alloca double, align 2
  store double %0, ptr %2, align 2, !tbaa !12
  %3 = load double, ptr %2, align 2, !tbaa !12
  %4 = fptoui double %3 to i64
  ret i64 %4
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca [8 x double], align 2
  %3 = alloca [4 x float], align 2
  %4 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %2, ptr align 2 @__const.main.d, i16 64, i1 true)
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %3, ptr align 2 @__const.main.f, i16 16, i1 true)
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !14
  br label %5

5:                                                ; preds = %13, %0
  %6 = load i16, ptr %4, align 2, !tbaa !14
  %7 = icmp slt i16 %6, 8
  br i1 %7, label %8, label %16

8:                                                ; preds = %5
  %9 = load i16, ptr %4, align 2, !tbaa !14
  %10 = getelementptr inbounds [8 x double], ptr %2, i16 0, i16 %9
  %11 = load volatile double, ptr %10, align 2, !tbaa !8
  %12 = call i64 @from_double(double noundef %11) #4
  call void @show(i64 noundef %12) #4
  br label %13

13:                                               ; preds = %8
  %14 = load i16, ptr %4, align 2, !tbaa !14
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %4, align 2, !tbaa !14
  br label %5, !llvm.loop !15

16:                                               ; preds = %5
  store i16 0, ptr %4, align 2, !tbaa !14
  br label %17

17:                                               ; preds = %25, %16
  %18 = load i16, ptr %4, align 2, !tbaa !14
  %19 = icmp slt i16 %18, 4
  br i1 %19, label %20, label %28

20:                                               ; preds = %17
  %21 = load i16, ptr %4, align 2, !tbaa !14
  %22 = getelementptr inbounds [4 x float], ptr %3, i16 0, i16 %21
  %23 = load volatile float, ptr %22, align 2, !tbaa !10
  %24 = call i64 @from_float(float noundef %23) #4
  call void @show(i64 noundef %24) #4
  br label %25

25:                                               ; preds = %20
  %26 = load i16, ptr %4, align 2, !tbaa !14
  %27 = add nsw i16 %26, 1
  store i16 %27, ptr %4, align 2, !tbaa !14
  br label %17, !llvm.loop !17

28:                                               ; preds = %17
  %29 = call i64 @from_long_double(double noundef f0x43EFFFFFFFFFFFFF) #4
  call void @show(i64 noundef %29) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: nounwind optsize
define internal void @show(i64 noundef %0) #0 {
  %2 = alloca i64, align 2
  store i64 %0, ptr %2, align 2, !tbaa !18
  %3 = load i64, ptr %2, align 2, !tbaa !18
  %4 = lshr i64 %3, 32
  %5 = trunc i64 %4 to i32
  call void @report(i32 noundef %5) #4
  %6 = load i64, ptr %2, align 2, !tbaa !18
  %7 = trunc i64 %6 to i32
  call void @report(i32 noundef %7) #4
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{i32 1, !"long-double-type", !"double"}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"double", !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"float", !6, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"long double", !6, i64 0}
!14 = !{!5, !5, i64 0}
!15 = distinct !{!15, !16}
!16 = !{!"llvm.loop.mustprogress"}
!17 = distinct !{!17, !16}
!18 = !{!19, !19, i64 0}
!19 = !{!"long long", !6, i64 0}
