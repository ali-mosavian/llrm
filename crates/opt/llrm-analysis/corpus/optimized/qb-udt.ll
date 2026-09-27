target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@C = internal global [8 x i8] zeroinitializer
@PTS = internal global [24 x i8] zeroinitializer
@PTS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @PTS to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @PTS, [6 x i8] c"\08\00\03\00\00\00" }>
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 305419896, ptr @C, !tbaa !2
  %0 = getelementptr inbounds i8, ptr @C, i16 4
  store i32 252645135, ptr %0, !tbaa !2
  %1 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  store i32 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 4
  store i32 2, ptr %2, !tbaa !2
  %3 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  store i32 3, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %3, i16 4
  store i32 4, ptr %4, !tbaa !2
  %5 = getelementptr inbounds [8 x i8], ptr @PTS, i16 2
  store i32 5, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %5, i16 4
  store i32 6, ptr %6, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 305419896)
  %7 = load i32, ptr %0, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %7)
  %8 = load i32, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %8)
  %9 = load i32, ptr %2, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %9)
  %10 = load i32, ptr %3, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %10)
  %11 = load i32, ptr %4, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %11)
  %12 = load i32, ptr %5, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %12)
  %13 = load i32, ptr %6, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %13)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
