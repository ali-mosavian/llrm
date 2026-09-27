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
  %0 = getelementptr inbounds i16, ptr @"KON%", i16 75
  store i16 1, ptr %0, !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i16, ptr @"KON%", i16 77
  %3 = load i16, ptr %2
  %4 = icmp eq i16 %3, 1
  br label %b2

b2:
  %5 = load i16, ptr @"I%", !tbaa !2
  %6 = icmp sle i16 %5, 10
  br i1 %6, label %7, label %b6

7:
  %8 = load i16, ptr @"PA%"
  %9 = add i16 %8, 1
  store i16 %9, ptr @"PA%"
  %10 = icmp sgt i16 %9, 360
  br i1 %10, label %11, label %13

11:
  %12 = add i16 %8, -359
  store i16 %12, ptr @"PA%"
  br label %13

13:
  br i1 %4, label %14, label %20

14:
  %15 = load i16, ptr @"PA%"
  %16 = add i16 %15, -1
  store i16 %16, ptr @"PA%"
  %17 = icmp slt i16 %16, 0
  br i1 %17, label %18, label %20

18:
  %19 = add i16 %15, 359
  store i16 %19, ptr @"PA%"
  br label %20

20:
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = add i16 %21, 1
  store i16 %22, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %23 = load i16, ptr @"PA%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %23)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc1000 void @TURN() addrspace(1) willreturn {
b1:
  %0 = getelementptr inbounds i16, ptr @"KON%", i16 75
  %1 = load i16, ptr %0, !tbaa !2
  %2 = icmp eq i16 %1, 1
  br i1 %2, label %b5, label %b11

b3:
  ret void

b5:
  %3 = load i16, ptr @"PA%", !tbaa !2
  %4 = add i16 %3, 1
  store i16 %4, ptr @"PA%", !tbaa !2
  %5 = icmp sgt i16 %4, 360
  br i1 %5, label %b7, label %b11

b7:
  %6 = add i16 %3, -359
  store i16 %6, ptr @"PA%", !tbaa !2
  br label %b11

b11:
  %7 = getelementptr inbounds i16, ptr @"KON%", i16 77
  %8 = load i16, ptr %7, !tbaa !2
  %9 = icmp eq i16 %8, 1
  br i1 %9, label %b12, label %b3

b12:
  %10 = load i16, ptr @"PA%", !tbaa !2
  %11 = add i16 %10, -1
  store i16 %11, ptr @"PA%", !tbaa !2
  %12 = icmp slt i16 %11, 0
  br i1 %12, label %b14, label %b3

b14:
  %13 = add i16 %10, 359
  store i16 %13, ptr @"PA%", !tbaa !2
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
