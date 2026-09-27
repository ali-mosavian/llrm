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
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  %4 = sext i1 %3 to i16
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr @"I%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sle i16 %6, %7
  %9 = sext i1 %8 to i16
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = load i16, ptr @$data, !tbaa !2
  %13 = icmp sge i16 %11, %12
  %14 = sext i1 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b5, label %b6

b5:
  %16 = load i16, ptr @"I%", !tbaa !2
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = sub i16 %16, 0
  %19 = getelementptr inbounds i16, ptr @"A%", i16 %18
  store i16 %17, ptr %19, !tbaa !2
  %20 = load i16, ptr @"T%", !tbaa !2
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = sub i16 %21, 0
  %23 = getelementptr inbounds i16, ptr @"A%", i16 %22
  %24 = load i16, ptr %23, !tbaa !2
  %25 = add i16 %20, %24
  %26 = load i16, ptr @"I%", !tbaa !2
  %27 = add i16 %26, 1
  %28 = sub i16 %27, 0
  %29 = getelementptr inbounds i16, ptr @"A%", i16 %28
  %30 = load i16, ptr %29, !tbaa !2
  %31 = add i16 %25, %30
  store i16 %31, ptr @"T%", !tbaa !2
  %32 = load i16, ptr @"I%", !tbaa !2
  %33 = load i16, ptr @"I%", !tbaa !2
  %34 = sext i16 %33 to i32
  %35 = sub i16 %32, 0
  %36 = getelementptr inbounds i32, ptr @"B&", i16 %35
  store i32 %34, ptr %36, !tbaa !2
  %37 = load i32, ptr @"U&", !tbaa !2
  %38 = load i16, ptr @"I%", !tbaa !2
  %39 = sub i16 %38, 0
  %40 = getelementptr inbounds i32, ptr @"B&", i16 %39
  %41 = load i32, ptr %40, !tbaa !2
  %42 = add i32 %37, %41
  store i32 %42, ptr @"U&", !tbaa !2
  %43 = load i16, ptr @"I%", !tbaa !2
  %44 = getelementptr i8, ptr @$data, i16 2
  %45 = load i16, ptr %44, !tbaa !2
  %46 = add i16 %43, %45
  store i16 %46, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5$descriptor)
  %47 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %47)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %48 = load i32, ptr @"U&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %48)
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
