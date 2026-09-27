target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"R%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [4 x i8] c"\02\00R=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 0, ptr @"R%", !tbaa !2
  store i16 2, ptr @"A%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = load i16, ptr @"I%", !tbaa !2
  %2 = icmp sle i16 %1, 10
  br i1 %2, label %b5, label %b6

b5:
  %3 = load i16, ptr @"R%", !tbaa !2
  %4 = shl i16 %1, 1
  %5 = add i16 %3, %4
  store i16 %5, ptr @"R%", !tbaa !2
  %6 = add i16 %1, 1
  store i16 %6, ptr @"I%", !tbaa !2
  br label %b2

b6:
  store i16 3, ptr @"B%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %7 = getelementptr i8, ptr @$data, i16 4
  store i16 10, ptr %7, !tbaa !2
  %8 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %8, !tbaa !2
  br label %b8

b8:
  %9 = load i16, ptr @"I%", !tbaa !2
  %10 = icmp sle i16 %9, 10
  br i1 %10, label %b10, label %b11

b10:
  %11 = load i16, ptr @"R%", !tbaa !2
  %12 = mul i16 %9, 3
  %13 = add i16 %11, %12
  store i16 %13, ptr @"R%", !tbaa !2
  %14 = add i16 %9, 1
  store i16 %14, ptr @"I%", !tbaa !2
  br label %b8

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %15 = load i16, ptr @"R%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %15)
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
