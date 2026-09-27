target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"D%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 3, ptr @"A%", !tbaa !2
  store i16 7, ptr @"B%", !tbaa !2
  store i16 2, ptr @"C%", !tbaa !2
  store i16 9, ptr @"D%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  %0 = load i16, ptr @"A%", !tbaa !2
  %1 = load i16, ptr @"B%", !tbaa !2
  %2 = icmp slt i16 %0, %1
  %3 = sext i1 %2 to i16
  store i16 %3, ptr @"X%", !tbaa !2
  %4 = load i16, ptr @"T%", !tbaa !2
  %5 = load i16, ptr @"X%", !tbaa !2
  %6 = add i16 %4, %5
  store i16 %6, ptr @"T%", !tbaa !2
  %7 = load i16, ptr @"A%", !tbaa !2
  %8 = load i16, ptr @"B%", !tbaa !2
  %9 = icmp slt i16 %7, %8
  %10 = sext i1 %9 to i16
  %11 = icmp ne i16 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  %12 = load i16, ptr @"T%", !tbaa !2
  %13 = add i16 %12, 1
  store i16 %13, ptr @"T%", !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  %14 = load i16, ptr @"A%", !tbaa !2
  %15 = load i16, ptr @"B%", !tbaa !2
  %16 = icmp slt i16 %14, %15
  %17 = sext i1 %16 to i16
  %18 = load i16, ptr @"C%", !tbaa !2
  %19 = load i16, ptr @"D%", !tbaa !2
  %20 = icmp slt i16 %18, %19
  %21 = sext i1 %20 to i16
  %22 = and i16 %17, %21
  %23 = icmp ne i16 %22, 0
  br i1 %23, label %b5, label %b6

b5:
  %24 = load i16, ptr @"T%", !tbaa !2
  %25 = add i16 %24, 2
  store i16 %25, ptr @"T%", !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %26 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %26)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6$descriptor)
  ret void
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
