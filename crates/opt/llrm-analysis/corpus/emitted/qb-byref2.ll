target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [24 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@$float3 = internal constant [4 x i8] c"\00\00\80@"
@$float4 = internal constant [4 x i8] c"\00\00\00A"
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string5$payload to ptr addrspace(2))
@$string5$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string5$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  %0 = load float, ptr @$float3, !tbaa !2
  store float %0, ptr @"S!", !tbaa !2
  %1 = load float, ptr @$float4, !tbaa !2
  %2 = fpext float %1 to double
  store double %2, ptr @"D#", !tbaa !2
  %3 = call cc1000 addrspace(1) float @"HALF!"(ptr @"S!", ptr @$data)
  %4 = getelementptr i8, ptr @$data, i16 4
  store float %3, ptr %4, !tbaa !2
  %5 = getelementptr i8, ptr @$data, i16 4
  %6 = load float, ptr %5, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PER4(float %6)
  %7 = getelementptr i8, ptr @$data, i16 8
  %8 = call cc1000 addrspace(1) double @"DOUBLED#"(ptr @"D#", ptr %7)
  %9 = getelementptr i8, ptr @$data, i16 16
  store double %8, ptr %9, !tbaa !2
  %10 = getelementptr i8, ptr @$data, i16 16
  %11 = load double, ptr %10, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PER8(double %11)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5$descriptor)
  ret void
}

define cc1000 float @"HALF!"(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca float
  store i16 0, ptr %2
  store float 0.000000e+00, ptr %3
  %4 = load float, ptr %0
  store i16 2, ptr %2, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = sitofp i16 %5 to float
  %7 = fdiv float %4, %6
  store float %7, ptr %3, !tbaa !2
  br label %b2

b2:
  %8 = load float, ptr %3, !tbaa !2
  ret float %8
}

define cc1000 double @"DOUBLED#"(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca double
  store i16 0, ptr %2
  store double 0.000000e+00, ptr %3
  %4 = load double, ptr %0
  store i16 2, ptr %2, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = sitofp i16 %5 to double
  %7 = fmul double %4, %6
  store double %7, ptr %3, !tbaa !2
  br label %b2

b2:
  %8 = load double, ptr %3, !tbaa !2
  ret double %8
}

declare cc1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc1000 void @llrm.qb.B$PER8(double) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
