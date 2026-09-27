target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [16 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A!" = internal global [4 x i8] zeroinitializer
@"B!" = internal global [4 x i8] zeroinitializer
@"C!" = internal global [4 x i8] zeroinitializer
@"P!" = internal global [4 x i8] zeroinitializer
@"Q!" = internal global [4 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [4 x i8] c"\02\00S=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 2, ptr @$data, !tbaa !2
  %0 = load i16, ptr @$data, !tbaa !2
  %1 = sitofp i16 %0 to float
  store float %1, ptr @"A!", !tbaa !2
  %2 = getelementptr i8, ptr @$data, i16 2
  store i16 4, ptr %2, !tbaa !2
  %3 = getelementptr i8, ptr @$data, i16 2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = sitofp i16 %4 to float
  store float %5, ptr @"B!", !tbaa !2
  %6 = getelementptr i8, ptr @$data, i16 4
  store i16 8, ptr %6, !tbaa !2
  %7 = getelementptr i8, ptr @$data, i16 4
  %8 = load i16, ptr %7, !tbaa !2
  %9 = sitofp i16 %8 to float
  store float %9, ptr @"C!", !tbaa !2
  %10 = getelementptr i8, ptr @$data, i16 6
  store i16 0, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$data, i16 6
  %12 = load i16, ptr %11, !tbaa !2
  %13 = sitofp i16 %12 to float
  store float %13, ptr @"S!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %14 = getelementptr i8, ptr @$data, i16 8
  store i16 10, ptr %14, !tbaa !2
  %15 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %15, !tbaa !2
  br label %b2

b2:
  %16 = getelementptr i8, ptr @$data, i16 10
  %17 = load i16, ptr %16, !tbaa !2
  %18 = icmp sge i16 %17, 0
  %19 = sext i1 %18 to i16
  %20 = icmp ne i16 %19, 0
  br i1 %20, label %b3, label %b4

b3:
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 8
  %23 = load i16, ptr %22, !tbaa !2
  %24 = icmp sle i16 %21, %23
  %25 = sext i1 %24 to i16
  %26 = icmp ne i16 %25, 0
  br i1 %26, label %b5, label %b6

b4:
  %27 = load i16, ptr @"I%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 8
  %29 = load i16, ptr %28, !tbaa !2
  %30 = icmp sge i16 %27, %29
  %31 = sext i1 %30 to i16
  %32 = icmp ne i16 %31, 0
  br i1 %32, label %b5, label %b6

b5:
  %33 = load float, ptr @"A!", !tbaa !2
  %34 = load float, ptr @"B!", !tbaa !2
  %35 = fadd float %33, %34
  %36 = load float, ptr @"C!", !tbaa !2
  %37 = fmul float %35, %36
  store float %37, ptr @"P!", !tbaa !2
  %38 = load float, ptr @"A!", !tbaa !2
  %39 = load float, ptr @"B!", !tbaa !2
  %40 = fadd float %38, %39
  %41 = load float, ptr @"C!", !tbaa !2
  %42 = fdiv float %40, %41
  store float %42, ptr @"Q!", !tbaa !2
  %43 = load float, ptr @"S!", !tbaa !2
  %44 = load float, ptr @"P!", !tbaa !2
  %45 = fadd float %43, %44
  %46 = load float, ptr @"Q!", !tbaa !2
  %47 = fadd float %45, %46
  store float %47, ptr @"S!", !tbaa !2
  %48 = load i16, ptr @"I%", !tbaa !2
  %49 = getelementptr i8, ptr @$data, i16 10
  %50 = load i16, ptr %49, !tbaa !2
  %51 = add i16 %48, %50
  store i16 %51, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %52 = load float, ptr @"S!", !tbaa !2
  %53 = getelementptr i8, ptr @$data, i16 12
  store float %52, ptr %53, !tbaa !2
  %54 = getelementptr i8, ptr @$data, i16 12
  %55 = load float, ptr %54, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PER4(float %55)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6$descriptor)
  ret void
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
