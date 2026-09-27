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
  %16 = load i16, ptr @"R%", !tbaa !2
  %17 = load i16, ptr @"A%", !tbaa !2
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = mul i16 %17, %18
  %20 = add i16 %16, %19
  store i16 %20, ptr @"R%", !tbaa !2
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add i16 %21, %23
  store i16 %24, ptr @"I%", !tbaa !2
  br label %b2

b6:
  store i16 3, ptr @"B%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 4
  store i16 10, ptr %25, !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %26, !tbaa !2
  br label %b7

b7:
  %27 = getelementptr i8, ptr @$data, i16 6
  %28 = load i16, ptr %27, !tbaa !2
  %29 = icmp sge i16 %28, 0
  %30 = sext i1 %29 to i16
  %31 = icmp ne i16 %30, 0
  br i1 %31, label %b8, label %b9

b8:
  %32 = load i16, ptr @"I%", !tbaa !2
  %33 = getelementptr i8, ptr @$data, i16 4
  %34 = load i16, ptr %33, !tbaa !2
  %35 = icmp sle i16 %32, %34
  %36 = sext i1 %35 to i16
  %37 = icmp ne i16 %36, 0
  br i1 %37, label %b10, label %b11

b9:
  %38 = load i16, ptr @"I%", !tbaa !2
  %39 = getelementptr i8, ptr @$data, i16 4
  %40 = load i16, ptr %39, !tbaa !2
  %41 = icmp sge i16 %38, %40
  %42 = sext i1 %41 to i16
  %43 = icmp ne i16 %42, 0
  br i1 %43, label %b10, label %b11

b10:
  %44 = load i16, ptr @"R%", !tbaa !2
  %45 = load i16, ptr @"B%", !tbaa !2
  %46 = load i16, ptr @"I%", !tbaa !2
  %47 = mul i16 %45, %46
  %48 = add i16 %44, %47
  store i16 %48, ptr @"R%", !tbaa !2
  %49 = load i16, ptr @"I%", !tbaa !2
  %50 = getelementptr i8, ptr @$data, i16 6
  %51 = load i16, ptr %50, !tbaa !2
  %52 = add i16 %49, %51
  store i16 %52, ptr @"I%", !tbaa !2
  br label %b7

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %53 = load i16, ptr @"R%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %53)
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
