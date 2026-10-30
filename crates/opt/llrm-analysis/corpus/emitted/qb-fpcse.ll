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
@$arguments$8 = internal global [12 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"S=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 2, ptr @$arguments$8, !tbaa !2
  %0 = load i16, ptr @$arguments$8, !tbaa !2
  %1 = sitofp i16 %0 to float
  %2 = fpround float %1 to float
  store float %2, ptr @"A!", !tbaa !2
  %3 = getelementptr i8, ptr @$arguments$8, i16 2
  store i16 4, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$8, i16 2
  %5 = load i16, ptr %4, !tbaa !2
  %6 = sitofp i16 %5 to float
  %7 = fpround float %6 to float
  store float %7, ptr @"B!", !tbaa !2
  %8 = getelementptr i8, ptr @$arguments$8, i16 4
  store i16 8, ptr %8, !tbaa !2
  %9 = getelementptr i8, ptr @$arguments$8, i16 4
  %10 = load i16, ptr %9, !tbaa !2
  %11 = sitofp i16 %10 to float
  %12 = fpround float %11 to float
  store float %12, ptr @"C!", !tbaa !2
  %13 = getelementptr i8, ptr @$arguments$8, i16 6
  store i16 0, ptr %13, !tbaa !2
  %14 = getelementptr i8, ptr @$arguments$8, i16 6
  %15 = load i16, ptr %14, !tbaa !2
  %16 = sitofp i16 %15 to float
  %17 = fpround float %16 to float
  store float %17, ptr @"S!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %18, !tbaa !2
  br label %b2

b2:
  %19 = getelementptr i8, ptr @$data, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = icmp sge i16 %20, 0
  br i1 %21, label %b3, label %b4

b3:
  %22 = load i16, ptr @"I%", !tbaa !2
  %23 = load i16, ptr @$data, !tbaa !2
  %24 = icmp sle i16 %22, %23
  br i1 %24, label %b5, label %b6

b4:
  %25 = load i16, ptr @"I%", !tbaa !2
  %26 = load i16, ptr @$data, !tbaa !2
  %27 = icmp sge i16 %25, %26
  br i1 %27, label %b5, label %b6

b5:
  %28 = load float, ptr @"A!", !tbaa !2
  %29 = load float, ptr @"B!", !tbaa !2
  %30 = fadd float %28, %29
  %31 = load float, ptr @"C!", !tbaa !2
  %32 = fmul float %30, %31
  %33 = fpround float %32 to float
  store float %33, ptr @"P!", !tbaa !2
  %34 = load float, ptr @"A!", !tbaa !2
  %35 = load float, ptr @"B!", !tbaa !2
  %36 = fadd float %34, %35
  %37 = load float, ptr @"C!", !tbaa !2
  %38 = fdiv float %36, %37
  %39 = fpround float %38 to float
  store float %39, ptr @"Q!", !tbaa !2
  %40 = load float, ptr @"S!", !tbaa !2
  %41 = load float, ptr @"P!", !tbaa !2
  %42 = fadd float %40, %41
  %43 = load float, ptr @"Q!", !tbaa !2
  %44 = fadd float %42, %43
  %45 = fpround float %44 to float
  store float %45, ptr @"S!", !tbaa !2
  %46 = load i16, ptr @"I%", !tbaa !2
  %47 = getelementptr i8, ptr @$data, i16 2
  %48 = load i16, ptr %47, !tbaa !2
  %49 = add nsw i16 %46, %48
  store i16 %49, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %50 = load float, ptr @"S!", !tbaa !2
  %51 = getelementptr i8, ptr @$arguments$8, i16 8
  store float %50, ptr %51, !tbaa !2
  %52 = getelementptr i8, ptr @$arguments$8, i16 8
  %53 = load float, ptr %52, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %53)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
