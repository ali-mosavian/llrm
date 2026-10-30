target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X#" = internal global [8 x i8] zeroinitializer
@$qb$readData = internal constant [71 x i8] c" 123456789012345, 2147483648, -2147483649, 32768, 99999999999999999999\00"
@"I%" = internal global [2 x i8] zeroinitializer
@$arguments$5 = internal global [60 x i8] zeroinitializer
@$float5 = internal constant [8 x i8] c"\00\00\00\00\00\00\F0?"
@"Y#" = internal global [8 x i8] zeroinitializer
@$float6 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0A"
@"Z!" = internal global [4 x i8] zeroinitializer
@$float7 = internal constant [8 x i8] c"\00\00\14\1A\99\BE<B"
@$float8 = internal constant [8 x i8] c"\00\EB*\F2T\8B\11C"
@$float9 = internal constant [8 x i8] c"@\8C\B5x\1D\AF\15D"

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  store i16 1, ptr @"I%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
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
  %10 = addrspacecast ptr @"X#" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) %10)
  %11 = load double, ptr @"X#", !tbaa !2
  store double %11, ptr @$arguments$5, !tbaa !2
  %12 = load double, ptr @$arguments$5, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %12)
  %13 = load i16, ptr @"I%", !tbaa !2
  %14 = getelementptr i8, ptr @$data, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = add nsw i16 %13, %15
  store i16 %16, ptr @"I%", !tbaa !2
  br label %b2

b6:
  %17 = load double, ptr @$float5, !tbaa !2
  %18 = getelementptr i8, ptr @$arguments$5, i16 8
  store double %17, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$arguments$5, i16 8
  %20 = load double, ptr %19, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %20)
  %21 = load double, ptr @$float6, !tbaa !2
  store double %21, ptr @"Y#", !tbaa !2
  %22 = load double, ptr @"Y#", !tbaa !2
  %23 = getelementptr i8, ptr @$arguments$5, i16 16
  store double %22, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$arguments$5, i16 16
  %25 = load double, ptr %24, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %25)
  %26 = load double, ptr @$float7, !tbaa !2
  %27 = fptrunc double %26 to float
  %28 = fpround float %27 to float
  store float %28, ptr @"Z!", !tbaa !2
  %29 = load float, ptr @"Z!", !tbaa !2
  %30 = getelementptr i8, ptr @$arguments$5, i16 24
  store float %29, ptr %30, !tbaa !2
  %31 = getelementptr i8, ptr @$arguments$5, i16 24
  %32 = load float, ptr %31, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %32)
  %33 = load double, ptr @$float6, !tbaa !2
  %34 = getelementptr i8, ptr @$arguments$5, i16 28
  store double %33, ptr %34, !tbaa !2
  %35 = getelementptr i8, ptr @$arguments$5, i16 28
  %36 = load double, ptr %35, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %36)
  %37 = load double, ptr @$float7, !tbaa !2
  %38 = getelementptr i8, ptr @$arguments$5, i16 36
  store double %37, ptr %38, !tbaa !2
  %39 = getelementptr i8, ptr @$arguments$5, i16 36
  %40 = load double, ptr %39, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %40)
  %41 = load double, ptr @$float8, !tbaa !2
  %42 = getelementptr i8, ptr @$arguments$5, i16 44
  store double %41, ptr %42, !tbaa !2
  %43 = getelementptr i8, ptr @$arguments$5, i16 44
  %44 = load double, ptr %43, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %44)
  %45 = load double, ptr @$float9, !tbaa !2
  %46 = getelementptr i8, ptr @$arguments$5, i16 52
  store double %45, ptr %46, !tbaa !2
  %47 = getelementptr i8, ptr @$arguments$5, i16 52
  %48 = load double, ptr %47, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %48)
  ret void
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDR8(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
