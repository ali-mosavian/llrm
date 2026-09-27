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
  %0 = load i16, ptr @$data, !tbaa !2
  %1 = sitofp i16 %0 to float
  %2 = sub i16 1, 1
  %3 = getelementptr inbounds float, ptr @"P!", i16 %2
  store float %1, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$data, i16 2
  store i16 28, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$data, i16 2
  %6 = load i16, ptr %5, !tbaa !2
  %7 = sitofp i16 %6 to float
  %8 = sub i16 2, 1
  %9 = getelementptr inbounds float, ptr @"P!", i16 %8
  store float %7, ptr %9, !tbaa !2
  %10 = getelementptr i8, ptr @$data, i16 4
  store i16 60, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$data, i16 4
  %12 = load i16, ptr %11, !tbaa !2
  %13 = sitofp i16 %12 to float
  %14 = sub i16 3, 1
  %15 = getelementptr inbounds float, ptr @"P!", i16 %14
  store float %13, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$data, i16 6
  store i16 4, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 6
  %18 = load i16, ptr %17, !tbaa !2
  %19 = sitofp i16 %18 to float
  store float %19, ptr @"K!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %20 = getelementptr i8, ptr @$data, i16 8
  store i16 3, ptr %20, !tbaa !2
  %21 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %21, !tbaa !2
  br label %b2

b2:
  %22 = getelementptr i8, ptr @$data, i16 10
  %23 = load i16, ptr %22, !tbaa !2
  %24 = icmp sge i16 %23, 0
  %25 = sext i1 %24 to i16
  %26 = icmp ne i16 %25, 0
  br i1 %26, label %b3, label %b4

b3:
  %27 = load i16, ptr @"I%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 8
  %29 = load i16, ptr %28, !tbaa !2
  %30 = icmp sle i16 %27, %29
  %31 = sext i1 %30 to i16
  %32 = icmp ne i16 %31, 0
  br i1 %32, label %b5, label %b6

b4:
  %33 = load i16, ptr @"I%", !tbaa !2
  %34 = getelementptr i8, ptr @$data, i16 8
  %35 = load i16, ptr %34, !tbaa !2
  %36 = icmp sge i16 %33, %35
  %37 = sext i1 %36 to i16
  %38 = icmp ne i16 %37, 0
  br i1 %38, label %b5, label %b6

b5:
  %39 = load i16, ptr @"I%", !tbaa !2
  %40 = sub i16 %39, 1
  %41 = getelementptr inbounds float, ptr @"P!", i16 %40
  %42 = load float, ptr %41, !tbaa !2
  %43 = load i16, ptr @"I%", !tbaa !2
  %44 = sub i16 %43, 1
  %45 = getelementptr inbounds float, ptr @"P!", i16 %44
  %46 = load float, ptr %45, !tbaa !2
  %47 = fmul float %42, %46
  store float %47, ptr @"Q!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %48 = load i16, ptr @"I%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %48)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7$descriptor)
  %49 = load float, ptr @"Q!", !tbaa !2
  %50 = call i32 @llvm.lrint.i32.f32(float %49)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %50)
  %51 = load i16, ptr @"I%", !tbaa !2
  %52 = sub i16 %51, 1
  %53 = getelementptr inbounds float, ptr @"P!", i16 %52
  %54 = load float, ptr %53, !tbaa !2
  %55 = load i16, ptr @"I%", !tbaa !2
  %56 = sub i16 %55, 1
  %57 = getelementptr inbounds float, ptr @"P!", i16 %56
  %58 = load float, ptr %57, !tbaa !2
  %59 = fmul float %54, %58
  %60 = load i16, ptr @"I%", !tbaa !2
  %61 = sub i16 %60, 1
  %62 = getelementptr inbounds float, ptr @"P!", i16 %61
  %63 = load float, ptr %62, !tbaa !2
  %64 = load i16, ptr @"I%", !tbaa !2
  %65 = sub i16 %64, 1
  %66 = getelementptr inbounds float, ptr @"P!", i16 %65
  %67 = load float, ptr %66, !tbaa !2
  %68 = fadd float %63, %67
  %69 = fdiv float %59, %68
  store float %69, ptr @"Q!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9$descriptor)
  %70 = load i16, ptr @"I%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %70)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11$descriptor)
  %71 = load float, ptr @"Q!", !tbaa !2
  %72 = call i32 @llvm.lrint.i32.f32(float %71)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %72)
  %73 = load i16, ptr @"I%", !tbaa !2
  %74 = sub i16 %73, 1
  %75 = getelementptr inbounds float, ptr @"P!", i16 %74
  %76 = load float, ptr %75, !tbaa !2
  %77 = load float, ptr @"K!", !tbaa !2
  %78 = fsub float %76, %77
  %79 = load i16, ptr @"I%", !tbaa !2
  %80 = sub i16 %79, 1
  %81 = getelementptr inbounds float, ptr @"P!", i16 %80
  %82 = load float, ptr %81, !tbaa !2
  %83 = load float, ptr @"K!", !tbaa !2
  %84 = fadd float %82, %83
  %85 = fdiv float %78, %84
  store float %85, ptr @"Q!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13$descriptor)
  %86 = load i16, ptr @"I%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %86)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15$descriptor)
  %87 = load float, ptr @"Q!", !tbaa !2
  %88 = getelementptr i8, ptr @$data, i16 12
  store i16 1024, ptr %88, !tbaa !2
  %89 = getelementptr i8, ptr @$data, i16 12
  %90 = load i16, ptr %89, !tbaa !2
  %91 = sitofp i16 %90 to float
  %92 = fmul float %87, %91
  %93 = call i32 @llvm.lrint.i32.f32(float %92)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %93)
  %94 = load i16, ptr @"I%", !tbaa !2
  %95 = getelementptr i8, ptr @$data, i16 10
  %96 = load i16, ptr %95, !tbaa !2
  %97 = add i16 %94, %96
  store i16 %97, ptr @"I%", !tbaa !2
  br label %b2

b6:
  %98 = getelementptr i8, ptr @$data, i16 14
  store i16 12, ptr %98, !tbaa !2
  %99 = getelementptr i8, ptr @$data, i16 14
  %100 = load i16, ptr %99, !tbaa !2
  %101 = sitofp i16 %100 to double
  store double %101, ptr @"D#", !tbaa !2
  %102 = load double, ptr @"D#", !tbaa !2
  %103 = load double, ptr @"D#", !tbaa !2
  %104 = fmul double %102, %103
  %105 = load double, ptr @"D#", !tbaa !2
  %106 = load double, ptr @"D#", !tbaa !2
  %107 = fadd double %105, %106
  %108 = fdiv double %104, %107
  store double %108, ptr @"E#", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17$descriptor)
  %109 = load double, ptr @"D#", !tbaa !2
  %110 = load double, ptr @"D#", !tbaa !2
  %111 = fmul double %109, %110
  %112 = call i32 @llvm.lrint.i32.f64(double %111)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %112)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string19$descriptor)
  %113 = load double, ptr @"E#", !tbaa !2
  %114 = call i32 @llvm.lrint.i32.f64(double %113)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %114)
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
