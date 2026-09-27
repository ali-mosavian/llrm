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
  %16 = load i16, ptr @"O1%", !tbaa !2
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = add i16 %16, %17
  store i16 %18, ptr @"O1%", !tbaa !2
  %19 = load i16, ptr @"O2%", !tbaa !2
  %20 = load i16, ptr @"O1%", !tbaa !2
  %21 = add i16 %19, %20
  store i16 %21, ptr @"O2%", !tbaa !2
  store i16 1, ptr @"J%", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 4
  store i16 10, ptr %22, !tbaa !2
  %23 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %23, !tbaa !2
  br label %b7

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %24 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %24)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %25 = load i16, ptr @"O2%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %25)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8$descriptor)
  ret void

b7:
  %26 = getelementptr i8, ptr @$data, i16 6
  %27 = load i16, ptr %26, !tbaa !2
  %28 = icmp sge i16 %27, 0
  %29 = sext i1 %28 to i16
  %30 = icmp ne i16 %29, 0
  br i1 %30, label %b8, label %b9

b8:
  %31 = load i16, ptr @"J%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 4
  %33 = load i16, ptr %32, !tbaa !2
  %34 = icmp sle i16 %31, %33
  %35 = sext i1 %34 to i16
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %b10, label %b11

b9:
  %37 = load i16, ptr @"J%", !tbaa !2
  %38 = getelementptr i8, ptr @$data, i16 4
  %39 = load i16, ptr %38, !tbaa !2
  %40 = icmp sge i16 %37, %39
  %41 = sext i1 %40 to i16
  %42 = icmp ne i16 %41, 0
  br i1 %42, label %b10, label %b11

b10:
  %43 = load i16, ptr @"T%", !tbaa !2
  %44 = load i16, ptr @"H1%", !tbaa !2
  %45 = load i16, ptr @"H2%", !tbaa !2
  %46 = mul i16 %44, %45
  %47 = add i16 %43, %46
  %48 = load i16, ptr @"H3%", !tbaa !2
  %49 = add i16 %47, %48
  store i16 %49, ptr @"T%", !tbaa !2
  %50 = load i16, ptr @"J%", !tbaa !2
  %51 = getelementptr i8, ptr @$data, i16 6
  %52 = load i16, ptr %51, !tbaa !2
  %53 = add i16 %50, %52
  store i16 %53, ptr @"J%", !tbaa !2
  br label %b7

b11:
  %54 = load i16, ptr @"I%", !tbaa !2
  %55 = getelementptr i8, ptr @$data, i16 2
  %56 = load i16, ptr %55, !tbaa !2
  %57 = add i16 %54, %56
  store i16 %57, ptr @"I%", !tbaa !2
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
