target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [16 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"P!" = internal global [12 x i8] zeroinitializer
@P$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"P!" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"P!", i16 -4), [6 x i8] c"\04\00\03\00\01\00" }>
@"Q!" = internal global [4 x i8] zeroinitializer
@"K!" = internal global [4 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@"E#" = internal global [8 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00SQ" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>
@$string9$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string9$payload, i16 4) to i16), [8 x i8] c"\05\00RATIO\00" }>
@$string9$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string9$payload, i16 2) to i16), ptr @$fslSegment }>
@$string11$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string11$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string11$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string11$payload, i16 2) to i16), ptr @$fslSegment }>
@$string13$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string13$payload, i16 4) to i16), [6 x i8] c"\03\00MIX\00" }>
@$string13$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string13$payload, i16 2) to i16), ptr @$fslSegment }>
@$string15$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string15$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string15$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string15$payload, i16 2) to i16), ptr @$fslSegment }>
@$string17$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string17$payload, i16 4) to i16), [6 x i8] c"\04\00DSQ=" }>
@$string17$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string17$payload, i16 2) to i16), ptr @$fslSegment }>
@$string19$payload = internal addrspace(1) constant <{ [2 x i8], i16, [10 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string19$payload, i16 4) to i16), [10 x i8] c"\07\00DRATIO=\00" }>
@$string19$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string19$payload, i16 2) to i16), ptr @$fslSegment }>
@$string21$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string21$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string21$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string21$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 12, ptr @$data, !tbaa !2
  %0 = getelementptr inbounds float, ptr @"P!", i16 0
  store float 1.200000e+01, ptr %0, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 2
  store i16 28, ptr %1, !tbaa !2
  %2 = getelementptr inbounds float, ptr @"P!", i16 1
  store float 2.800000e+01, ptr %2, !tbaa !2
  %3 = getelementptr i8, ptr @$data, i16 4
  store i16 60, ptr %3, !tbaa !2
  %4 = getelementptr inbounds float, ptr @"P!", i16 2
  store float 6.000000e+01, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$data, i16 6
  store i16 4, ptr %5, !tbaa !2
  store float 4.000000e+00, ptr @"K!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %6 = getelementptr i8, ptr @$data, i16 8
  store i16 3, ptr %6, !tbaa !2
  %7 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %7, !tbaa !2
  %8 = getelementptr i8, ptr @$data, i16 12
  br label %b2

b2:
  %9 = load i16, ptr %7, !tbaa !2
  %10 = icmp sge i16 %9, 0
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = load i16, ptr %6, !tbaa !2
  %13 = icmp sle i16 %11, %12
  br i1 %13, label %b5, label %b6

b4:
  %14 = load i16, ptr @"I%", !tbaa !2
  %15 = load i16, ptr %6, !tbaa !2
  %16 = icmp sge i16 %14, %15
  br i1 %16, label %b5, label %b6

b5:
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = add i16 %17, -1
  %19 = getelementptr inbounds float, ptr @"P!", i16 %18
  %20 = load float, ptr %19, !tbaa !2
  %21 = load float, ptr %19, !tbaa !2
  %22 = fmul float %20, %21
  store float %22, ptr @"Q!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %23 = load i16, ptr @"I%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %23)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7$descriptor)
  %24 = load float, ptr @"Q!", !tbaa !2
  %25 = call i32 @llvm.lrint.i32.f32(float %24)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %25)
  %26 = load i16, ptr @"I%", !tbaa !2
  %27 = add i16 %26, -1
  %28 = getelementptr inbounds float, ptr @"P!", i16 %27
  %29 = load float, ptr %28, !tbaa !2
  %30 = load float, ptr %28, !tbaa !2
  %31 = fmul float %29, %30
  %32 = load float, ptr %28, !tbaa !2
  %33 = load float, ptr %28, !tbaa !2
  %34 = fadd float %32, %33
  %35 = fdiv float %31, %34
  store float %35, ptr @"Q!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9$descriptor)
  %36 = load i16, ptr @"I%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %36)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11$descriptor)
  %37 = load float, ptr @"Q!", !tbaa !2
  %38 = call i32 @llvm.lrint.i32.f32(float %37)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %38)
  %39 = load i16, ptr @"I%", !tbaa !2
  %40 = add i16 %39, -1
  %41 = getelementptr inbounds float, ptr @"P!", i16 %40
  %42 = load float, ptr %41, !tbaa !2
  %43 = load float, ptr @"K!", !tbaa !2
  %44 = fsub float %42, %43
  %45 = load float, ptr %41, !tbaa !2
  %46 = fadd float %45, %43
  %47 = fdiv float %44, %46
  store float %47, ptr @"Q!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13$descriptor)
  %48 = load i16, ptr @"I%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %48)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15$descriptor)
  %49 = load float, ptr @"Q!", !tbaa !2
  store i16 1024, ptr %8, !tbaa !2
  %50 = fmul float %49, 1.024000e+03
  %51 = call i32 @llvm.lrint.i32.f32(float %50)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %51)
  %52 = load i16, ptr @"I%", !tbaa !2
  %53 = load i16, ptr %7, !tbaa !2
  %54 = add i16 %52, %53
  store i16 %54, ptr @"I%", !tbaa !2
  br label %b2

b6:
  %55 = getelementptr i8, ptr @$data, i16 14
  store i16 12, ptr %55, !tbaa !2
  store double 1.200000e+01, ptr @"D#", !tbaa !2
  store double 6.000000e+00, ptr @"E#", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17$descriptor)
  %56 = load double, ptr @"D#", !tbaa !2
  %57 = fmul double %56, %56
  %58 = call i32 @llvm.lrint.i32.f64(double %57)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %58)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19$descriptor)
  %59 = load double, ptr @"E#", !tbaa !2
  %60 = call i32 @llvm.lrint.i32.f64(double %59)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %60)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string21$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
