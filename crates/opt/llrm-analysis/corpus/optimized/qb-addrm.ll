target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [202 x i8] zeroinitializer
@"A%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"A%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"A%", [6 x i8] c"\02\00e\00\00\00" }>
@"B&" = internal global [404 x i8] zeroinitializer
@B$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"B&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"B&", [6 x i8] c"\04\00e\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@"U&" = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string5$payload to ptr addrspace(2))
@$string5$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string5$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [4 x i8] c"\02\00U=" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 0, ptr @"T%", !tbaa !2
  store i32 0, ptr @"U&", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 20, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = load i16, ptr @"I%", !tbaa !2
  %2 = icmp sle i16 %1, 20
  br i1 %2, label %b5, label %b6

b5:
  %3 = getelementptr inbounds i16, ptr @"A%", i16 %1
  store i16 %1, ptr %3, !tbaa !2
  %4 = load i16, ptr @"T%", !tbaa !2
  %5 = load i16, ptr %3, !tbaa !2
  %6 = add i16 %4, %5
  %7 = add i16 %1, 1
  %8 = getelementptr inbounds i16, ptr @"A%", i16 %7
  %9 = load i16, ptr %8, !tbaa !2
  %10 = add i16 %6, %9
  store i16 %10, ptr @"T%", !tbaa !2
  %11 = sext i16 %1 to i32
  %12 = getelementptr inbounds i32, ptr @"B&", i16 %1
  store i32 %11, ptr %12, !tbaa !2
  %13 = load i32, ptr @"U&", !tbaa !2
  %14 = load i32, ptr %12, !tbaa !2
  %15 = add i32 %13, %14
  store i32 %15, ptr @"U&", !tbaa !2
  store i16 %7, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5$descriptor)
  %16 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %16)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %17 = load i32, ptr @"U&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %17)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string10$descriptor)
  ret void
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
