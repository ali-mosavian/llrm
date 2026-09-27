target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [10 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A!" = internal global [4 x i8] zeroinitializer
@"B!" = internal global [4 x i8] zeroinitializer
@"C!" = internal global [4 x i8] zeroinitializer
@"P!" = internal global [4 x i8] zeroinitializer
@"Q!" = internal global [4 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [9 x i8] c" 2, 4, 8\00"
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00S=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"A!" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"B!" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) %1)
  %2 = addrspacecast ptr @"C!" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) %2)
  store i16 0, ptr @$data, !tbaa !2
  %3 = load i16, ptr @$data, !tbaa !2
  %4 = sitofp i16 %3 to float
  store float %4, ptr @"S!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %5 = getelementptr i8, ptr @$data, i16 2
  store i16 10, ptr %5, !tbaa !2
  %6 = getelementptr i8, ptr @$data, i16 4
  store i16 1, ptr %6, !tbaa !2
  br label %b2

b2:
  %7 = getelementptr i8, ptr @$data, i16 4
  %8 = load i16, ptr %7, !tbaa !2
  %9 = icmp sge i16 %8, 0
  %10 = sext i1 %9 to i16
  %11 = icmp ne i16 %10, 0
  br i1 %11, label %b3, label %b4

b3:
  %12 = load i16, ptr @"I%", !tbaa !2
  %13 = getelementptr i8, ptr @$data, i16 2
  %14 = load i16, ptr %13, !tbaa !2
  %15 = icmp sle i16 %12, %14
  %16 = sext i1 %15 to i16
  %17 = icmp ne i16 %16, 0
  br i1 %17, label %b5, label %b6

b4:
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = icmp sge i16 %18, %20
  %22 = sext i1 %21 to i16
  %23 = icmp ne i16 %22, 0
  br i1 %23, label %b5, label %b6

b5:
  %24 = load float, ptr @"A!", !tbaa !2
  %25 = load float, ptr @"B!", !tbaa !2
  %26 = fadd float %24, %25
  %27 = load float, ptr @"C!", !tbaa !2
  %28 = fmul float %26, %27
  store float %28, ptr @"P!", !tbaa !2
  %29 = load float, ptr @"A!", !tbaa !2
  %30 = load float, ptr @"B!", !tbaa !2
  %31 = fadd float %29, %30
  %32 = load float, ptr @"C!", !tbaa !2
  %33 = fdiv float %31, %32
  store float %33, ptr @"Q!", !tbaa !2
  %34 = load float, ptr @"S!", !tbaa !2
  %35 = load float, ptr @"P!", !tbaa !2
  %36 = fadd float %34, %35
  %37 = load float, ptr @"Q!", !tbaa !2
  %38 = fadd float %36, %37
  store float %38, ptr @"S!", !tbaa !2
  %39 = load i16, ptr @"I%", !tbaa !2
  %40 = getelementptr i8, ptr @$data, i16 4
  %41 = load i16, ptr %40, !tbaa !2
  %42 = add i16 %39, %41
  store i16 %42, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %43 = load float, ptr @"S!", !tbaa !2
  %44 = getelementptr i8, ptr @$data, i16 6
  store float %43, ptr %44, !tbaa !2
  %45 = getelementptr i8, ptr @$data, i16 6
  %46 = load float, ptr %45, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PER4(float %46)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  ret void
}

declare cc1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc1000 void @llrm.qb.B$RDR4(ptr addrspace(1)) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
