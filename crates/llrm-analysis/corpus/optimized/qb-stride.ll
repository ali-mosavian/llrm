target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"B%" = internal global [402 x i8] zeroinitializer
@"B%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"B%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"B%", [6 x i8] c"\02\00\C9\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 0, ptr @"T%", !tbaa !2
  store i16 0, ptr @"I%", !tbaa !2
  store i16 100, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 5, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = load i16, ptr @"I%", !tbaa !2
  %2 = icmp sle i16 %1, 100
  br i1 %2, label %b5, label %b6

b5:
  %3 = sext i16 %1 to i32
  %4 = sdiv i32 %3, 5
  %5 = trunc i32 %4 to i16
  %6 = getelementptr inbounds i16, ptr @"B%", i16 %1
  store i16 %5, ptr %6, !tbaa !2
  %7 = load i16, ptr @"T%", !tbaa !2
  %8 = load i16, ptr %6, !tbaa !2
  %9 = add i16 %7, %8
  store i16 %9, ptr @"T%", !tbaa !2
  %10 = add i16 %1, 5
  store i16 %10, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %11 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %11)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
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
