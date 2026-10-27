target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A!" = internal global [4 x i8] zeroinitializer
@"B!" = internal global [4 x i8] zeroinitializer
@"C!" = internal global [4 x i8] zeroinitializer
@"P!" = internal global [4 x i8] zeroinitializer
@"Q!" = internal global [4 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [9 x i8] c" 2, 4, 8\00"
@$arguments$8 = internal global [6 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"S=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"A!" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"B!" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) %1)
  %2 = addrspacecast ptr @"C!" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) %2)
  store i16 0, ptr @$arguments$8, !tbaa !2
  %3 = load i16, ptr @$arguments$8, !tbaa !2
  %4 = sitofp i16 %3 to float
  %5 = fpround float %4 to float
  store float %5, ptr @"S!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %6 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %6, !tbaa !2
  br label %b2

b2:
  %7 = getelementptr i8, ptr @$data, i16 2
  %8 = load i16, ptr %7, !tbaa !2
  %9 = icmp sge i16 %8, 0
  br i1 %9, label %b3, label %b4

b3:
  %10 = load i16, ptr @"I%", !tbaa !2
  %11 = load i16, ptr @$data, !tbaa !2
  %12 = icmp sle i16 %10, %11
  br i1 %12, label %b5, label %b6

b4:
  %13 = load i16, ptr @"I%", !tbaa !2
  %14 = load i16, ptr @$data, !tbaa !2
  %15 = icmp sge i16 %13, %14
  br i1 %15, label %b5, label %b6

b5:
  %16 = load float, ptr @"A!", !tbaa !2
  %17 = load float, ptr @"B!", !tbaa !2
  %18 = fadd float %16, %17
  %19 = load float, ptr @"C!", !tbaa !2
  %20 = fmul float %18, %19
  %21 = fpround float %20 to float
  store float %21, ptr @"P!", !tbaa !2
  %22 = load float, ptr @"A!", !tbaa !2
  %23 = load float, ptr @"B!", !tbaa !2
  %24 = fadd float %22, %23
  %25 = load float, ptr @"C!", !tbaa !2
  %26 = fdiv float %24, %25
  %27 = fpround float %26 to float
  store float %27, ptr @"Q!", !tbaa !2
  %28 = load float, ptr @"S!", !tbaa !2
  %29 = load float, ptr @"P!", !tbaa !2
  %30 = fadd float %28, %29
  %31 = load float, ptr @"Q!", !tbaa !2
  %32 = fadd float %30, %31
  %33 = fpround float %32 to float
  store float %33, ptr @"S!", !tbaa !2
  %34 = load i16, ptr @"I%", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 2
  %36 = load i16, ptr %35, !tbaa !2
  %37 = add nsw i16 %34, %36
  store i16 %37, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %38 = load float, ptr @"S!", !tbaa !2
  %39 = getelementptr i8, ptr @$arguments$8, i16 2
  store float %38, ptr %39, !tbaa !2
  %40 = getelementptr i8, ptr @$arguments$8, i16 2
  %41 = load float, ptr %40, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %41)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  ret void
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDR4(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
