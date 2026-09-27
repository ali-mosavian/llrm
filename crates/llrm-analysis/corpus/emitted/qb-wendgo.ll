target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"KON%" = internal global [202 x i8] zeroinitializer
@"KON%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"KON%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"KON%", [6 x i8] c"\02\00e\00\00\00" }>
@"PA%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [6 x i8] c"\03\00PA=\00" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 355, ptr @"PA%", !tbaa !2
  %0 = sub i16 75, 0
  %1 = getelementptr inbounds i16, ptr @"KON%", i16 %0
  store i16 1, ptr %1, !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %2 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %2, !tbaa !2
  br label %b2

b2:
  %3 = getelementptr i8, ptr @$data, i16 2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = icmp sge i16 %4, 0
  %6 = sext i1 %5 to i16
  %7 = icmp ne i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr @"I%", !tbaa !2
  %9 = load i16, ptr @$data, !tbaa !2
  %10 = icmp sle i16 %8, %9
  %11 = sext i1 %10 to i16
  %12 = icmp ne i16 %11, 0
  br i1 %12, label %b5, label %b6

b4:
  %13 = load i16, ptr @"I%", !tbaa !2
  %14 = load i16, ptr @$data, !tbaa !2
  %15 = icmp sge i16 %13, %14
  %16 = sext i1 %15 to i16
  %17 = icmp ne i16 %16, 0
  br i1 %17, label %b5, label %b6

b5:
  call cc1000 addrspace(1) void @TURN()
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = add i16 %18, %20
  store i16 %21, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %22 = load i16, ptr @"PA%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %22)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc1000 void @TURN() addrspace(1) {
b1:
  br label %b4

b2:
  br label %b11

b3:
  ret void

b4:
  %0 = sub i16 75, 0
  %1 = getelementptr inbounds i16, ptr @"KON%", i16 %0
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp eq i16 %2, 1
  %4 = sext i1 %3 to i16
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b5, label %b6

b5:
  %6 = load i16, ptr @"PA%", !tbaa !2
  %7 = add i16 %6, 1
  store i16 %7, ptr @"PA%", !tbaa !2
  %8 = load i16, ptr @"PA%", !tbaa !2
  %9 = icmp sgt i16 %8, 360
  %10 = sext i1 %9 to i16
  %11 = icmp ne i16 %10, 0
  br i1 %11, label %b7, label %b8

b6:
  br label %b2

b7:
  %12 = load i16, ptr @"PA%", !tbaa !2
  %13 = sub i16 %12, 360
  %14 = add i16 0, %13
  store i16 %14, ptr @"PA%", !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  br label %b2

b11:
  %15 = sub i16 77, 0
  %16 = getelementptr inbounds i16, ptr @"KON%", i16 %15
  %17 = load i16, ptr %16, !tbaa !2
  %18 = icmp eq i16 %17, 1
  %19 = sext i1 %18 to i16
  %20 = icmp ne i16 %19, 0
  br i1 %20, label %b12, label %b13

b12:
  %21 = load i16, ptr @"PA%", !tbaa !2
  %22 = sub i16 %21, 1
  store i16 %22, ptr @"PA%", !tbaa !2
  %23 = load i16, ptr @"PA%", !tbaa !2
  %24 = icmp slt i16 %23, 0
  %25 = sext i1 %24 to i16
  %26 = icmp ne i16 %25, 0
  br i1 %26, label %b14, label %b15

b13:
  br label %b3

b14:
  %27 = load i16, ptr @"PA%", !tbaa !2
  %28 = add i16 360, %27
  store i16 %28, ptr @"PA%", !tbaa !2
  br label %b16

b15:
  br label %b16

b16:
  br label %b3
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
