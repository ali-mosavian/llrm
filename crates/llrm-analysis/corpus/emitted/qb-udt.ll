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
  %1 = sub i16 0, 0
  %2 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %1
  store i32 1, ptr %2, !tbaa !2
  %3 = sub i16 0, 0
  %4 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %3
  %5 = getelementptr inbounds i8, ptr %4, i16 4
  store i32 2, ptr %5, !tbaa !2
  %6 = sub i16 1, 0
  %7 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %6
  store i32 3, ptr %7, !tbaa !2
  %8 = sub i16 1, 0
  %9 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %8
  %10 = getelementptr inbounds i8, ptr %9, i16 4
  store i32 4, ptr %10, !tbaa !2
  %11 = sub i16 2, 0
  %12 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %11
  store i32 5, ptr %12, !tbaa !2
  %13 = sub i16 2, 0
  %14 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %13
  %15 = getelementptr inbounds i8, ptr %14, i16 4
  store i32 6, ptr %15, !tbaa !2
  %16 = load i32, ptr @C, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %16)
  %17 = getelementptr inbounds i8, ptr @C, i16 4
  %18 = load i32, ptr %17, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %18)
  %19 = sub i16 0, 0
  %20 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %19
  %21 = load i32, ptr %20, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %21)
  %22 = sub i16 0, 0
  %23 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %22
  %24 = getelementptr inbounds i8, ptr %23, i16 4
  %25 = load i32, ptr %24, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %25)
  %26 = sub i16 1, 0
  %27 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %26
  %28 = load i32, ptr %27, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %28)
  %29 = sub i16 1, 0
  %30 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %29
  %31 = getelementptr inbounds i8, ptr %30, i16 4
  %32 = load i32, ptr %31, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %32)
  %33 = sub i16 2, 0
  %34 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %33
  %35 = load i32, ptr %34, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %35)
  %36 = sub i16 2, 0
  %37 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %36
  %38 = getelementptr inbounds i8, ptr %37, i16 4
  %39 = load i32, ptr %38, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %39)
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
