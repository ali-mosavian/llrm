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
  %3 = fdiv float %0, 2.000000e+00
  %4 = getelementptr i8, ptr @$data, i16 4
  store float %3, ptr %4, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PER4(float %3)
  %5 = load double, ptr @"D#"
  %6 = fmul double %5, 2.000000e+00
  %7 = getelementptr i8, ptr @$data, i16 16
  store double %6, ptr %7, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PER8(double %6)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5$descriptor)
  ret void
}

define cc1000 float @"HALF!"(ptr %0, ptr %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  %2 = load float, ptr %0
  %3 = fdiv float %2, 2.000000e+00
  ret float %3
}

define cc1000 double @"DOUBLED#"(ptr %0, ptr %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  %2 = load double, ptr %0
  %3 = fmul double %2, 2.000000e+00
  ret double %3
}

declare cc1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc1000 void @llrm.qb.B$PER8(double) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
