target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"H1%" = internal global [2 x i8] zeroinitializer
@"H2%" = internal global [2 x i8] zeroinitializer
@"H3%" = internal global [2 x i8] zeroinitializer
@"O1%" = internal global [2 x i8] zeroinitializer
@"O2%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"J%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [4 x i8] c"\02\00O=" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 3, ptr @"H1%", !tbaa !2
  store i16 5, ptr @"H2%", !tbaa !2
  store i16 7, ptr @"H3%", !tbaa !2
  store i16 0, ptr @"O1%", !tbaa !2
  store i16 0, ptr @"O2%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 4
  %2 = getelementptr i8, ptr @$data, i16 6
  br label %b2

b2:
  %3 = load i16, ptr @"I%", !tbaa !2
  %4 = icmp sle i16 %3, 10
  br i1 %4, label %b5, label %b6

b5:
  %5 = load i16, ptr @"O1%", !tbaa !2
  %6 = add i16 %5, %3
  store i16 %6, ptr @"O1%", !tbaa !2
  %7 = load i16, ptr @"O2%", !tbaa !2
  %8 = add i16 %7, %6
  store i16 %8, ptr @"O2%", !tbaa !2
  store i16 1, ptr @"J%", !tbaa !2
  store i16 10, ptr %1, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b8

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %9 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %9)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %10 = load i16, ptr @"O2%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %10)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8$descriptor)
  ret void

b8:
  %11 = load i16, ptr @"J%", !tbaa !2
  %12 = icmp sle i16 %11, 10
  br i1 %12, label %b10, label %b11

b10:
  %13 = load i16, ptr @"T%", !tbaa !2
  %14 = add i16 %13, 22
  store i16 %14, ptr @"T%", !tbaa !2
  %15 = add i16 %11, 1
  store i16 %15, ptr @"J%", !tbaa !2
  br label %b8

b11:
  %16 = load i16, ptr @"I%", !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr @"I%", !tbaa !2
  br label %b2
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
