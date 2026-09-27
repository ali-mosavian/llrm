target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"B%" = internal global [122 x i8] zeroinitializer
@"B%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"B%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"B%", [6 x i8] c"\02\00=\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"J%" = internal global [2 x i8] zeroinitializer
@"W%" = internal global [2 x i8] zeroinitializer
@"H%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 6, ptr @"W%", !tbaa !2
  store i16 5, ptr @"H%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  store i16 0, ptr @"I%", !tbaa !2
  store i16 4, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 4
  %2 = getelementptr i8, ptr @$data, i16 6
  br label %b2

b2:
  %3 = load i16, ptr @"I%", !tbaa !2
  %4 = icmp sle i16 %3, 4
  br i1 %4, label %b5, label %b6

b5:
  store i16 0, ptr @"J%", !tbaa !2
  store i16 5, ptr %1, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  %5 = mul i16 %3, 6
  %6 = mul i16 %3, 10
  br label %b8

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %7 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %7)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  ret void

b8:
  %8 = load i16, ptr @"J%", !tbaa !2
  %9 = icmp sle i16 %8, 5
  br i1 %9, label %b10, label %b11

b10:
  %10 = add i16 %5, %8
  %11 = add i16 %6, %8
  %12 = getelementptr inbounds i16, ptr @"B%", i16 %10
  store i16 %11, ptr %12, !tbaa !2
  %13 = load i16, ptr @"T%", !tbaa !2
  %14 = load i16, ptr %12, !tbaa !2
  %15 = add i16 %13, %14
  store i16 %15, ptr @"T%", !tbaa !2
  %16 = add i16 %8, 1
  store i16 %16, ptr @"J%", !tbaa !2
  br label %b8

b11:
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = add i16 %17, 1
  store i16 %18, ptr @"I%", !tbaa !2
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
