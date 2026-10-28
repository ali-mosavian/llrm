target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X#" = internal global [8 x i8] zeroinitializer
@$qb$readData = internal constant [43 x i8] c" 10, 3, 2, 0.5, 7.25, 123.456, 1E-5, 99999\00"
@"I!" = internal global [4 x i8] zeroinitializer
@$arguments$3 = internal global [38 x i8] zeroinitializer
@$float5 = internal constant [4 x i8] zeroinitializer
@$float6 = internal constant [8 x i8] zeroinitializer
@$float7 = internal constant [8 x i8] c"\00\00\00\00\00\00$@"
@$float8 = internal constant [8 x i8] c"\F5:\B7\D0,.\D6@"
@$float9 = internal constant [8 x i8] c"\9A\99\99\99\99\99\0D@"
@$float10 = internal constant [8 x i8] c"\00\00\00\00\00\00Y@"

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  store i16 1, ptr @$arguments$3, !tbaa !2
  %0 = load i16, ptr @$arguments$3, !tbaa !2
  %1 = sitofp i16 %0 to float
  %2 = fpround float %1 to float
  store float %2, ptr @"I!", !tbaa !2
  %3 = getelementptr i8, ptr @$arguments$3, i16 2
  store i16 8, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$3, i16 2
  %5 = load i16, ptr %4, !tbaa !2
  %6 = sitofp i16 %5 to float
  %7 = fpround float %6 to float
  store float %7, ptr @$data, !tbaa !2
  %8 = getelementptr i8, ptr @$arguments$3, i16 4
  store i16 1, ptr %8, !tbaa !2
  %9 = getelementptr i8, ptr @$arguments$3, i16 4
  %10 = load i16, ptr %9, !tbaa !2
  %11 = sitofp i16 %10 to float
  %12 = fpround float %11 to float
  %13 = getelementptr i8, ptr @$data, i16 4
  store float %12, ptr %13, !tbaa !2
  br label %b2

b2:
  %14 = getelementptr i8, ptr @$data, i16 4
  %15 = load float, ptr %14, !tbaa !2
  %16 = load float, ptr @$float5, !tbaa !2
  %17 = fcmp oge float %15, %16
  br i1 %17, label %b3, label %b4

b3:
  %18 = load float, ptr @"I!", !tbaa !2
  %19 = load float, ptr @$data, !tbaa !2
  %20 = fcmp ole float %18, %19
  br i1 %20, label %b5, label %b6

b4:
  %21 = load float, ptr @"I!", !tbaa !2
  %22 = load float, ptr @$data, !tbaa !2
  %23 = fcmp oge float %21, %22
  br i1 %23, label %b5, label %b6

b5:
  %24 = addrspacecast ptr @"X#" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) %24)
  %25 = load double, ptr @"X#", !tbaa !2
  %26 = load double, ptr @$float6, !tbaa !2
  %27 = fcmp ole double %25, %26
  br i1 %27, label %b7, label %b8

b6:
  ret void

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b8:
  %28 = call double @llvm.log.f64(double %25)
  %29 = fpround double %28 to double
  %30 = getelementptr i8, ptr @$arguments$3, i16 6
  store double %29, ptr %30, !tbaa !2
  %31 = getelementptr i8, ptr @$arguments$3, i16 6
  %32 = load double, ptr %31, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %32)
  %33 = load double, ptr @"X#", !tbaa !2
  %34 = load double, ptr @$float7, !tbaa !2
  %35 = fdiv double %33, %34
  %36 = load double, ptr @$float8, !tbaa !2
  %37 = fcmp oge double %35, %36
  br i1 %37, label %b9, label %b10

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b10:
  %38 = call double @llvm.exp.f64(double %35)
  %39 = fpround double %38 to double
  %40 = getelementptr i8, ptr @$arguments$3, i16 14
  store double %39, ptr %40, !tbaa !2
  %41 = getelementptr i8, ptr @$arguments$3, i16 14
  %42 = load double, ptr %41, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %42)
  %43 = load double, ptr @"X#", !tbaa !2
  %44 = load double, ptr @$float9, !tbaa !2
  %45 = fmul double %43, %44
  %46 = load double, ptr @$float6, !tbaa !2
  %47 = fcmp ole double %45, %46
  br i1 %47, label %b11, label %b12

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %48 = call double @llvm.log.f64(double %45)
  %49 = fpround double %48 to double
  %50 = getelementptr i8, ptr @$arguments$3, i16 22
  store double %49, ptr %50, !tbaa !2
  %51 = getelementptr i8, ptr @$arguments$3, i16 22
  %52 = load double, ptr %51, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %52)
  %53 = load double, ptr @"X#", !tbaa !2
  %54 = fneg double %53
  %55 = load double, ptr @$float10, !tbaa !2
  %56 = fdiv double %54, %55
  %57 = load double, ptr @$float8, !tbaa !2
  %58 = fcmp oge double %56, %57
  br i1 %58, label %b13, label %b14

b13:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b14:
  %59 = call double @llvm.exp.f64(double %56)
  %60 = fpround double %59 to double
  %61 = getelementptr i8, ptr @$arguments$3, i16 30
  store double %60, ptr %61, !tbaa !2
  %62 = getelementptr i8, ptr @$arguments$3, i16 30
  %63 = load double, ptr %62, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %63)
  %64 = load float, ptr @"I!", !tbaa !2
  %65 = getelementptr i8, ptr @$data, i16 4
  %66 = load float, ptr %65, !tbaa !2
  %67 = fadd float %64, %66
  %68 = fpround float %67 to float
  store float %68, ptr @"I!", !tbaa !2
  br label %b2
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDR8(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare double @llvm.log.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PSR8(double) addrspace(1)

declare double @llvm.exp.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
