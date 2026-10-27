target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"P!" = internal global [12 x i8] zeroinitializer, align 2
@P$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"P!" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"P!", i16 -4), [6 x i8] c"\04\00\03\00\01\00" }>
@"Q!" = internal global [4 x i8] zeroinitializer
@"K!" = internal global [4 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@"E#" = internal global [8 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$arguments$8 = internal global [12 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"SQ" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"RATIO\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string9, i16 4), [4 x i8] c"MIX\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"=\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string11, i16 4), [4 x i8] c"DSQ=" }>
@$string12 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string12, i16 4), [8 x i8] c"DRATIO=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 12, ptr @$arguments$8, !tbaa !2
  %0 = load i16, ptr @$arguments$8, !tbaa !2
  %1 = sitofp i16 %0 to float
  %2 = fpround float %1 to float
  %3 = getelementptr inbounds float, ptr @"P!", i16 0
  store float %2, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$8, i16 2
  store i16 28, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$arguments$8, i16 2
  %6 = load i16, ptr %5, !tbaa !2
  %7 = sitofp i16 %6 to float
  %8 = fpround float %7 to float
  %9 = getelementptr inbounds float, ptr @"P!", i16 1
  store float %8, ptr %9, !tbaa !2
  %10 = getelementptr i8, ptr @$arguments$8, i16 4
  store i16 60, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$arguments$8, i16 4
  %12 = load i16, ptr %11, !tbaa !2
  %13 = sitofp i16 %12 to float
  %14 = fpround float %13 to float
  %15 = getelementptr inbounds float, ptr @"P!", i16 2
  store float %14, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$arguments$8, i16 6
  store i16 4, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$arguments$8, i16 6
  %18 = load i16, ptr %17, !tbaa !2
  %19 = sitofp i16 %18 to float
  %20 = fpround float %19 to float
  store float %20, ptr @"K!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %21 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %21, !tbaa !2
  br label %b2

b2:
  %22 = getelementptr i8, ptr @$data, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = icmp sge i16 %23, 0
  br i1 %24, label %b3, label %b4

b3:
  %25 = load i16, ptr @"I%", !tbaa !2
  %26 = load i16, ptr @$data, !tbaa !2
  %27 = icmp sle i16 %25, %26
  br i1 %27, label %b5, label %b6

b4:
  %28 = load i16, ptr @"I%", !tbaa !2
  %29 = load i16, ptr @$data, !tbaa !2
  %30 = icmp sge i16 %28, %29
  br i1 %30, label %b5, label %b6

b5:
  %31 = load i16, ptr @"I%", !tbaa !2
  %32 = sub i16 %31, 1
  %33 = getelementptr inbounds float, ptr @"P!", i16 %32
  %34 = load float, ptr %33, !tbaa !2
  %35 = load i16, ptr @"I%", !tbaa !2
  %36 = sub i16 %35, 1
  %37 = getelementptr inbounds float, ptr @"P!", i16 %36
  %38 = load float, ptr %37, !tbaa !2
  %39 = fmul float %34, %38
  %40 = fpround float %39 to float
  store float %40, ptr @"Q!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %41 = load i16, ptr @"I%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %41)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %42 = load float, ptr @"Q!", !tbaa !2
  %43 = call i32 @llvm.lrint.i32.f32(float %42)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %43)
  %44 = load i16, ptr @"I%", !tbaa !2
  %45 = sub i16 %44, 1
  %46 = getelementptr inbounds float, ptr @"P!", i16 %45
  %47 = load float, ptr %46, !tbaa !2
  %48 = load i16, ptr @"I%", !tbaa !2
  %49 = sub i16 %48, 1
  %50 = getelementptr inbounds float, ptr @"P!", i16 %49
  %51 = load float, ptr %50, !tbaa !2
  %52 = fmul float %47, %51
  %53 = load i16, ptr @"I%", !tbaa !2
  %54 = sub i16 %53, 1
  %55 = getelementptr inbounds float, ptr @"P!", i16 %54
  %56 = load float, ptr %55, !tbaa !2
  %57 = load i16, ptr @"I%", !tbaa !2
  %58 = sub i16 %57, 1
  %59 = getelementptr inbounds float, ptr @"P!", i16 %58
  %60 = load float, ptr %59, !tbaa !2
  %61 = fadd float %56, %60
  %62 = fdiv float %52, %61
  %63 = fpround float %62 to float
  store float %63, ptr @"Q!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %64 = load i16, ptr @"I%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %64)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %65 = load float, ptr @"Q!", !tbaa !2
  %66 = call i32 @llvm.lrint.i32.f32(float %65)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %66)
  %67 = load i16, ptr @"I%", !tbaa !2
  %68 = sub i16 %67, 1
  %69 = getelementptr inbounds float, ptr @"P!", i16 %68
  %70 = load float, ptr %69, !tbaa !2
  %71 = load float, ptr @"K!", !tbaa !2
  %72 = fsub float %70, %71
  %73 = load i16, ptr @"I%", !tbaa !2
  %74 = sub i16 %73, 1
  %75 = getelementptr inbounds float, ptr @"P!", i16 %74
  %76 = load float, ptr %75, !tbaa !2
  %77 = load float, ptr @"K!", !tbaa !2
  %78 = fadd float %76, %77
  %79 = fdiv float %72, %78
  %80 = fpround float %79 to float
  store float %80, ptr @"Q!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  %81 = load i16, ptr @"I%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %81)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  %82 = load float, ptr @"Q!", !tbaa !2
  %83 = getelementptr i8, ptr @$arguments$8, i16 8
  store i16 1024, ptr %83, !tbaa !2
  %84 = getelementptr i8, ptr @$arguments$8, i16 8
  %85 = load i16, ptr %84, !tbaa !2
  %86 = sitofp i16 %85 to float
  %87 = fmul float %82, %86
  %88 = call i32 @llvm.lrint.i32.f32(float %87)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %88)
  %89 = load i16, ptr @"I%", !tbaa !2
  %90 = getelementptr i8, ptr @$data, i16 2
  %91 = load i16, ptr %90, !tbaa !2
  %92 = add nsw i16 %89, %91
  store i16 %92, ptr @"I%", !tbaa !2
  br label %b2

b6:
  %93 = getelementptr i8, ptr @$arguments$8, i16 10
  store i16 12, ptr %93, !tbaa !2
  %94 = getelementptr i8, ptr @$arguments$8, i16 10
  %95 = load i16, ptr %94, !tbaa !2
  %96 = sitofp i16 %95 to double
  %97 = fpround double %96 to double
  store double %97, ptr @"D#", !tbaa !2
  %98 = load double, ptr @"D#", !tbaa !2
  %99 = load double, ptr @"D#", !tbaa !2
  %100 = fmul double %98, %99
  %101 = load double, ptr @"D#", !tbaa !2
  %102 = load double, ptr @"D#", !tbaa !2
  %103 = fadd double %101, %102
  %104 = fdiv double %100, %103
  %105 = fpround double %104 to double
  store double %105, ptr @"E#", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
  %106 = load double, ptr @"D#", !tbaa !2
  %107 = load double, ptr @"D#", !tbaa !2
  %108 = fmul double %106, %107
  %109 = call i32 @llvm.lrint.i32.f64(double %108)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %109)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  %110 = load double, ptr @"E#", !tbaa !2
  %111 = call i32 @llvm.lrint.i32.f64(double %110)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %111)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string13)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
