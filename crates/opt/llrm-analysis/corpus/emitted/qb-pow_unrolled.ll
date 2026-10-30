target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$float3 = internal constant [8 x i8] c"\FD\FF\8F\1E\C4\BC\D6B"
@$arguments$6 = internal global [10 x i8] zeroinitializer
@$float5 = internal constant [8 x i8] c"\00\00\00\00\00\00$@"
@$float6 = internal constant [8 x i8] c"\00\00\00\00\E0\FF\DF@"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 -7, ptr @"I%", !tbaa !2
  store i16 7, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"I%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = load double, ptr @$float3, !tbaa !2
  %11 = load i16, ptr @"I%", !tbaa !2
  store i16 %11, ptr @$arguments$6, !tbaa !2
  %12 = load i16, ptr @$arguments$6, !tbaa !2
  %13 = sitofp i16 %12 to double
  %14 = load double, ptr @$float5, !tbaa !2
  %15 = call double @llvm.log2.f64(double %14)
  %16 = fmul double %15, %13
  %17 = load double, ptr @$float6, !tbaa !2
  %18 = fcmp oge double %16, %17
  br i1 %18, label %b7, label %b8

b6:
  ret void

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b8:
  %19 = call double @llvm.exp2.f64(double %16)
  %20 = fmul double %10, %19
  %21 = fpround double %20 to double
  store double %21, ptr @"D#", !tbaa !2
  %22 = load double, ptr @"D#", !tbaa !2
  %23 = getelementptr i8, ptr @$arguments$6, i16 2
  store double %22, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$arguments$6, i16 2
  %25 = load double, ptr %24, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %25)
  %26 = load i16, ptr @"I%", !tbaa !2
  %27 = getelementptr i8, ptr @$data, i16 2
  %28 = load i16, ptr %27, !tbaa !2
  %29 = add nsw i16 %26, %28
  store i16 %29, ptr @"I%", !tbaa !2
  br label %b2
}

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
