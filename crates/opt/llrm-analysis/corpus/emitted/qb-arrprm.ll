target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"NUMS&" = internal global [8 x i8] zeroinitializer
@NUMS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"NUMS&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"NUMS&", [6 x i8] c"\04\00\02\00\00\00" }>
@PTS = internal global [16 x i8] zeroinitializer
@PTS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @PTS to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @PTS, [6 x i8] c"\08\00\02\00\00\00" }>
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string5$payload to ptr addrspace(2))
@$string5$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string5$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @FILLNUMS(ptr @NUMS$descriptor)
  call cc1000 addrspace(1) void @FILLPTS(ptr @PTS$descriptor)
  %0 = sub i16 0, 0
  %1 = getelementptr inbounds i32, ptr @"NUMS&", i16 %0
  %2 = load i32, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %2)
  %3 = sub i16 1, 0
  %4 = getelementptr inbounds i32, ptr @"NUMS&", i16 %3
  %5 = load i32, ptr %4, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %5)
  %6 = sub i16 0, 0
  %7 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %6
  %8 = load i32, ptr %7, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %8)
  %9 = sub i16 0, 0
  %10 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %9
  %11 = getelementptr inbounds i8, ptr %10, i16 4
  %12 = load i32, ptr %11, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %12)
  %13 = sub i16 1, 0
  %14 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %13
  %15 = load i32, ptr %14, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %15)
  %16 = sub i16 1, 0
  %17 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %16
  %18 = getelementptr inbounds i8, ptr %17, i16 4
  %19 = load i32, ptr %18, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %19)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5$descriptor)
  ret void
}

define cc1000 void @FILLNUMS(ptr %0) addrspace(1) {
b1:
  %1 = mul i16 0, 4
  %2 = getelementptr i8, ptr %0, i16 2
  %3 = load i16, ptr %2
  %4 = getelementptr i8, ptr %0, i16 10
  %5 = load i16, ptr %4
  %6 = add i16 %5, %1
  %7 = inttoptr i16 %3 to ptr addrspace(2)
  %8 = addrspacecast ptr addrspace(2) %7 to ptr addrspace(1)
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 %6
  store i32 7, ptr addrspace(1) %9
  %10 = mul i16 1, 4
  %11 = add i16 %5, %10
  %12 = inttoptr i16 %3 to ptr addrspace(2)
  %13 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr addrspace(1) %13, i16 %11
  store i32 8, ptr addrspace(1) %14
  ret void
}

define cc1000 void @FILLPTS(ptr %0) addrspace(1) {
b1:
  %1 = mul i16 0, 8
  %2 = getelementptr i8, ptr %0, i16 2
  %3 = load i16, ptr %2
  %4 = getelementptr i8, ptr %0, i16 10
  %5 = load i16, ptr %4
  %6 = add i16 %5, %1
  %7 = inttoptr i16 %3 to ptr addrspace(2)
  %8 = addrspacecast ptr addrspace(2) %7 to ptr addrspace(1)
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 %6
  store i32 1, ptr addrspace(1) %9
  %10 = mul i16 0, 8
  %11 = add i16 %5, %10
  %12 = inttoptr i16 %3 to ptr addrspace(2)
  %13 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr addrspace(1) %13, i16 %11
  %15 = getelementptr inbounds i8, ptr addrspace(1) %14, i16 4
  store i32 2, ptr addrspace(1) %15
  %16 = mul i16 1, 8
  %17 = add i16 %5, %16
  %18 = inttoptr i16 %3 to ptr addrspace(2)
  %19 = addrspacecast ptr addrspace(2) %18 to ptr addrspace(1)
  %20 = getelementptr i8, ptr addrspace(1) %19, i16 %17
  store i32 3, ptr addrspace(1) %20
  %21 = mul i16 1, 8
  %22 = add i16 %5, %21
  %23 = inttoptr i16 %3 to ptr addrspace(2)
  %24 = addrspacecast ptr addrspace(2) %23 to ptr addrspace(1)
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 %22
  %26 = getelementptr inbounds i8, ptr addrspace(1) %25, i16 4
  store i32 4, ptr addrspace(1) %26
  ret void
}

declare cc1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
