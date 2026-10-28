target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A#" = internal global [8 x i8] zeroinitializer
@"B#" = internal global [8 x i8] zeroinitializer
@"C#" = internal global [8 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@"E#" = internal global [8 x i8] zeroinitializer
@"F#" = internal global [8 x i8] zeroinitializer
@"G#" = internal global [8 x i8] zeroinitializer
@"H#" = internal global [8 x i8] zeroinitializer
@"P#" = internal global [8 x i8] zeroinitializer
@"Q#" = internal global [8 x i8] zeroinitializer
@"R#" = internal global [8 x i8] zeroinitializer
@$float3 = internal constant [8 x i8] c"\9A\99\99\99\99\99\F1?"
@$float4 = internal constant [8 x i8] c"\9A\99\99\99\99\99\01@"
@$float5 = internal constant [8 x i8] c"ffffff\0A@"
@$float6 = internal constant [8 x i8] c"\9A\99\99\99\99\99\11@"
@$float7 = internal constant [8 x i8] c"\00\00\00\00\00\00\16@"
@$float8 = internal constant [8 x i8] c"ffffff\1A@"
@$float9 = internal constant [8 x i8] c"\CD\CC\CC\CC\CC\CC\1E@"
@$float10 = internal constant [8 x i8] c"\9A\99\99\99\99\99!@"
@$float11 = internal constant [8 x i8] c"\CD\CC\CC\CC\CC\CC#@"
@$float12 = internal constant [8 x i8] c"333333$@"
@$float13 = internal constant [8 x i8] c"\B8\1E\85\EBQ8&@"
@"I%" = internal global [2 x i8] zeroinitializer
@$arguments$26 = internal global [92 x i8] zeroinitializer
@$float15 = internal constant [8 x i8] c"\00\00\00\00\00\00$@"
@$float16 = internal constant [8 x i8] c"\00\00\00\00\E0\FF\DF@"
@$float17 = internal constant [8 x i8] c"\8D\ED\B5\A0\F7\C6\B0>"
@$float18 = internal constant [8 x i8] c"q\AC\8B\DBh\00\F0?"
@$float19 = internal constant [8 x i8] c"\FC\A9\F1\D2MbP?"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = load double, ptr @$float3, !tbaa !2
  store double %0, ptr @"A#", !tbaa !2
  %1 = load double, ptr @$float4, !tbaa !2
  store double %1, ptr @"B#", !tbaa !2
  %2 = load double, ptr @$float5, !tbaa !2
  store double %2, ptr @"C#", !tbaa !2
  %3 = load double, ptr @$float6, !tbaa !2
  store double %3, ptr @"D#", !tbaa !2
  %4 = load double, ptr @$float7, !tbaa !2
  store double %4, ptr @"E#", !tbaa !2
  %5 = load double, ptr @$float8, !tbaa !2
  store double %5, ptr @"F#", !tbaa !2
  %6 = load double, ptr @$float9, !tbaa !2
  store double %6, ptr @"G#", !tbaa !2
  %7 = load double, ptr @$float10, !tbaa !2
  store double %7, ptr @"H#", !tbaa !2
  %8 = load double, ptr @$float11, !tbaa !2
  store double %8, ptr @"P#", !tbaa !2
  %9 = load double, ptr @$float12, !tbaa !2
  store double %9, ptr @"Q#", !tbaa !2
  %10 = load double, ptr @$float13, !tbaa !2
  store double %10, ptr @"R#", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 9, ptr @$data, !tbaa !2
  %11 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %11, !tbaa !2
  br label %b2

b2:
  %12 = getelementptr i8, ptr @$data, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = icmp sge i16 %13, 0
  br i1 %14, label %b3, label %b4

b3:
  %15 = load i16, ptr @"I%", !tbaa !2
  %16 = load i16, ptr @$data, !tbaa !2
  %17 = icmp sle i16 %15, %16
  br i1 %17, label %b5, label %b6

b4:
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = load i16, ptr @$data, !tbaa !2
  %20 = icmp sge i16 %18, %19
  br i1 %20, label %b5, label %b6

b5:
  %21 = load double, ptr @"A#", !tbaa !2
  %22 = load i16, ptr @"I%", !tbaa !2
  %23 = sub i16 %22, 5
  store i16 %23, ptr @$arguments$26, !tbaa !2
  %24 = load i16, ptr @$arguments$26, !tbaa !2
  %25 = sitofp i16 %24 to double
  %26 = load double, ptr @$float15, !tbaa !2
  %27 = call double @llvm.log2.f64(double %26)
  %28 = fmul double %27, %25
  %29 = load double, ptr @$float16, !tbaa !2
  %30 = fcmp oge double %28, %29
  br i1 %30, label %b7, label %b8

b6:
  ret void

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b8:
  %31 = call double @llvm.exp2.f64(double %28)
  %32 = fmul double %21, %31
  %33 = load double, ptr @"B#", !tbaa !2
  %34 = fadd double %32, %33
  %35 = fpround double %34 to double
  store double %35, ptr @"A#", !tbaa !2
  %36 = load double, ptr @"B#", !tbaa !2
  %37 = load double, ptr @"C#", !tbaa !2
  %38 = load i16, ptr @"I%", !tbaa !2
  %39 = getelementptr i8, ptr @$arguments$26, i16 2
  store i16 %38, ptr %39, !tbaa !2
  %40 = getelementptr i8, ptr @$arguments$26, i16 2
  %41 = load i16, ptr %40, !tbaa !2
  %42 = sitofp i16 %41 to double
  %43 = fadd double %37, %42
  %44 = load double, ptr @"D#", !tbaa !2
  %45 = fmul double %43, %44
  %46 = fadd double %36, %45
  %47 = fpround double %46 to double
  store double %47, ptr @"B#", !tbaa !2
  %48 = load double, ptr @"P#", !tbaa !2
  %49 = load double, ptr @"D#", !tbaa !2
  %50 = load double, ptr @"E#", !tbaa !2
  %51 = fmul double %49, %50
  %52 = load double, ptr @$float17, !tbaa !2
  %53 = fmul double %51, %52
  %54 = fadd double %48, %53
  %55 = fpround double %54 to double
  store double %55, ptr @"P#", !tbaa !2
  %56 = load double, ptr @"Q#", !tbaa !2
  %57 = load double, ptr @$float18, !tbaa !2
  %58 = fmul double %56, %57
  %59 = load double, ptr @"G#", !tbaa !2
  %60 = fadd double %58, %59
  %61 = fpround double %60 to double
  store double %61, ptr @"Q#", !tbaa !2
  %62 = load double, ptr @"R#", !tbaa !2
  %63 = load double, ptr @"H#", !tbaa !2
  %64 = load double, ptr @$float19, !tbaa !2
  %65 = fmul double %63, %64
  %66 = fadd double %62, %65
  %67 = fpround double %66 to double
  store double %67, ptr @"R#", !tbaa !2
  %68 = load double, ptr @"A#", !tbaa !2
  %69 = getelementptr i8, ptr @$arguments$26, i16 4
  store double %68, ptr %69, !tbaa !2
  %70 = getelementptr i8, ptr @$arguments$26, i16 4
  %71 = load double, ptr %70, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %71)
  %72 = load double, ptr @"B#", !tbaa !2
  %73 = getelementptr i8, ptr @$arguments$26, i16 12
  store double %72, ptr %73, !tbaa !2
  %74 = getelementptr i8, ptr @$arguments$26, i16 12
  %75 = load double, ptr %74, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %75)
  %76 = load double, ptr @"C#", !tbaa !2
  %77 = getelementptr i8, ptr @$arguments$26, i16 20
  store double %76, ptr %77, !tbaa !2
  %78 = getelementptr i8, ptr @$arguments$26, i16 20
  %79 = load double, ptr %78, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %79)
  %80 = load double, ptr @"D#", !tbaa !2
  %81 = getelementptr i8, ptr @$arguments$26, i16 28
  store double %80, ptr %81, !tbaa !2
  %82 = getelementptr i8, ptr @$arguments$26, i16 28
  %83 = load double, ptr %82, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %83)
  %84 = load double, ptr @"E#", !tbaa !2
  %85 = getelementptr i8, ptr @$arguments$26, i16 36
  store double %84, ptr %85, !tbaa !2
  %86 = getelementptr i8, ptr @$arguments$26, i16 36
  %87 = load double, ptr %86, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %87)
  %88 = load double, ptr @"F#", !tbaa !2
  %89 = getelementptr i8, ptr @$arguments$26, i16 44
  store double %88, ptr %89, !tbaa !2
  %90 = getelementptr i8, ptr @$arguments$26, i16 44
  %91 = load double, ptr %90, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %91)
  %92 = load double, ptr @"G#", !tbaa !2
  %93 = getelementptr i8, ptr @$arguments$26, i16 52
  store double %92, ptr %93, !tbaa !2
  %94 = getelementptr i8, ptr @$arguments$26, i16 52
  %95 = load double, ptr %94, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %95)
  %96 = load double, ptr @"H#", !tbaa !2
  %97 = getelementptr i8, ptr @$arguments$26, i16 60
  store double %96, ptr %97, !tbaa !2
  %98 = getelementptr i8, ptr @$arguments$26, i16 60
  %99 = load double, ptr %98, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %99)
  %100 = load double, ptr @"P#", !tbaa !2
  %101 = getelementptr i8, ptr @$arguments$26, i16 68
  store double %100, ptr %101, !tbaa !2
  %102 = getelementptr i8, ptr @$arguments$26, i16 68
  %103 = load double, ptr %102, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %103)
  %104 = load double, ptr @"Q#", !tbaa !2
  %105 = getelementptr i8, ptr @$arguments$26, i16 76
  store double %104, ptr %105, !tbaa !2
  %106 = getelementptr i8, ptr @$arguments$26, i16 76
  %107 = load double, ptr %106, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCR8(double %107)
  %108 = load double, ptr @"R#", !tbaa !2
  %109 = getelementptr i8, ptr @$arguments$26, i16 84
  store double %108, ptr %109, !tbaa !2
  %110 = getelementptr i8, ptr @$arguments$26, i16 84
  %111 = load double, ptr %110, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %111)
  %112 = load i16, ptr @"I%", !tbaa !2
  %113 = getelementptr i8, ptr @$data, i16 2
  %114 = load i16, ptr %113, !tbaa !2
  %115 = add nsw i16 %112, %114
  store i16 %115, ptr @"I%", !tbaa !2
  br label %b2
}

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PCR8(double) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
