target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"M%" = internal global [802 x i8] zeroinitializer
@"M%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"M%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"M%", [6 x i8] c"\02\00\91\01\00\00" }>
@"R%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"W%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 20, ptr @"W%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  store i16 0, ptr @"R%", !tbaa !2
  store i16 19, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 4
  %2 = getelementptr i8, ptr @$data, i16 6
  br label %b2

b2:
  %3 = load i16, ptr @"R%", !tbaa !2
  %4 = icmp sle i16 %3, 19
  br i1 %4, label %b5, label %b6

b5:
  store i16 0, ptr @"C%", !tbaa !2
  store i16 19, ptr %1, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  %5 = mul i16 %3, 20
  br label %b8

b6:
  store i16 0, ptr @"R%", !tbaa !2
  %6 = getelementptr i8, ptr @$data, i16 8
  store i16 19, ptr %6, !tbaa !2
  %7 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %7, !tbaa !2
  %8 = load i16, ptr @"W%", !tbaa !2
  br label %b12

b8:
  %9 = load i16, ptr @"C%", !tbaa !2
  %10 = icmp sle i16 %9, 19
  br i1 %10, label %b10, label %b11

b10:
  %11 = add i16 %5, %9
  %12 = add i16 %3, %9
  %13 = getelementptr inbounds i16, ptr @"M%", i16 %11
  store i16 %12, ptr %13, !tbaa !2
  %14 = add i16 %9, 1
  store i16 %14, ptr @"C%", !tbaa !2
  br label %b8

b11:
  %15 = load i16, ptr @"R%", !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr @"R%", !tbaa !2
  br label %b2

b12:
  %17 = load i16, ptr @"R%", !tbaa !2
  %18 = icmp sle i16 %17, 19
  br i1 %18, label %b15, label %b16

b15:
  %19 = load i16, ptr @"T%", !tbaa !2
  %20 = mul i16 %17, %8
  %21 = add i16 %20, %17
  %22 = getelementptr inbounds i16, ptr @"M%", i16 %21
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add i16 %19, %23
  store i16 %24, ptr @"T%", !tbaa !2
  %25 = add i16 %17, 1
  store i16 %25, ptr @"R%", !tbaa !2
  br label %b12

b16:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %26 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %26)
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
