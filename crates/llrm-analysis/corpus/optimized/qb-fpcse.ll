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
  store float 2.000000e+00, ptr @"A!", !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 4, ptr %0, !tbaa !2
  store float 4.000000e+00, ptr @"B!", !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 4
  store i16 8, ptr %1, !tbaa !2
  store float 8.000000e+00, ptr @"C!", !tbaa !2
  %2 = getelementptr i8, ptr @$data, i16 6
  store i16 0, ptr %2, !tbaa !2
  store float 0.000000e+00, ptr @"S!", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %3 = getelementptr i8, ptr @$data, i16 8
  store i16 10, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr @"I%", !tbaa !2
  %6 = icmp sle i16 %5, 10
  br i1 %6, label %b5, label %b6

b5:
  store float 4.800000e+01, ptr @"P!", !tbaa !2
  store float 7.500000e-01, ptr @"Q!", !tbaa !2
  %7 = load float, ptr @"S!", !tbaa !2
  %8 = fadd float %7, 4.800000e+01
  %9 = fadd float %8, 7.500000e-01
  store float %9, ptr @"S!", !tbaa !2
  %10 = add i16 %5, 1
  store i16 %10, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %11 = load float, ptr @"S!", !tbaa !2
  %12 = getelementptr i8, ptr @$data, i16 12
  store float %11, ptr %12, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PER4(float %11)
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
