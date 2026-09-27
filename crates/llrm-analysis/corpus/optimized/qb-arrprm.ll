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
  %0 = getelementptr i8, ptr @NUMS$descriptor, i16 2
  %1 = load i16, ptr %0
  %2 = getelementptr i8, ptr @NUMS$descriptor, i16 10
  %3 = load i16, ptr %2
  %4 = inttoptr i16 %1 to ptr addrspace(2)
  %5 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)
  %6 = getelementptr i8, ptr addrspace(1) %5, i16 %3
  store i32 7, ptr addrspace(1) %6
  %7 = add i16 %3, 4
  %8 = getelementptr i8, ptr addrspace(1) %5, i16 %7
  store i32 8, ptr addrspace(1) %8
  %9 = getelementptr i8, ptr @PTS$descriptor, i16 2
  %10 = load i16, ptr %9
  %11 = getelementptr i8, ptr @PTS$descriptor, i16 10
  %12 = load i16, ptr %11
  %13 = inttoptr i16 %10 to ptr addrspace(2)
  %14 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 %12
  store i32 1, ptr addrspace(1) %15
  %16 = getelementptr inbounds i8, ptr addrspace(1) %15, i16 4
  store i32 2, ptr addrspace(1) %16
  %17 = add i16 %12, 8
  %18 = getelementptr i8, ptr addrspace(1) %14, i16 %17
  store i32 3, ptr addrspace(1) %18
  %19 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 4
  store i32 4, ptr addrspace(1) %19
  %20 = getelementptr inbounds i32, ptr @"NUMS&", i16 0
  %21 = load i32, ptr %20, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %21)
  %22 = getelementptr inbounds i32, ptr @"NUMS&", i16 1
  %23 = load i32, ptr %22, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %23)
  %24 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %25 = load i32, ptr %24, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %25)
  %26 = getelementptr inbounds i8, ptr %24, i16 4
  %27 = load i32, ptr %26, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %27)
  %28 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %29 = load i32, ptr %28, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %29)
  %30 = getelementptr inbounds i8, ptr %28, i16 4
  %31 = load i32, ptr %30, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %31)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5$descriptor)
  ret void
}

define cc1000 void @FILLNUMS(ptr %0) addrspace(1) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load i16, ptr %1
  %3 = getelementptr i8, ptr %0, i16 10
  %4 = load i16, ptr %3
  %5 = inttoptr i16 %2 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 %4
  store i32 7, ptr addrspace(1) %7
  %8 = add i16 %4, 4
  %9 = getelementptr i8, ptr addrspace(1) %6, i16 %8
  store i32 8, ptr addrspace(1) %9
  ret void
}

define cc1000 void @FILLPTS(ptr %0) addrspace(1) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load i16, ptr %1
  %3 = getelementptr i8, ptr %0, i16 10
  %4 = load i16, ptr %3
  %5 = inttoptr i16 %2 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 %4
  store i32 1, ptr addrspace(1) %7
  %8 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 4
  store i32 2, ptr addrspace(1) %8
  %9 = add i16 %4, 8
  %10 = getelementptr i8, ptr addrspace(1) %6, i16 %9
  store i32 3, ptr addrspace(1) %10
  %11 = getelementptr inbounds i8, ptr addrspace(1) %10, i16 4
  store i32 4, ptr addrspace(1) %11
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
