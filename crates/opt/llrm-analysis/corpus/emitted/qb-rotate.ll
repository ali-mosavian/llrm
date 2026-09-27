target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"N%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 0, ptr @"T%", !tbaa !2
  store i16 10, ptr @"N%", !tbaa !2
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
  %16 = load i16, ptr @"T%", !tbaa !2
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = add i16 %16, %17
  store i16 %18, ptr @"T%", !tbaa !2
  %19 = load i16, ptr @"I%", !tbaa !2
  %20 = getelementptr i8, ptr @$data, i16 2
  %21 = load i16, ptr %20, !tbaa !2
  %22 = add i16 %19, %21
  store i16 %22, ptr @"I%", !tbaa !2
  br label %b2

b6:
  store i16 1, ptr @"I%", !tbaa !2
  %23 = load i16, ptr @"N%", !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 4
  store i16 %23, ptr %24, !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %25, !tbaa !2
  br label %b7

b7:
  %26 = getelementptr i8, ptr @$data, i16 6
  %27 = load i16, ptr %26, !tbaa !2
  %28 = icmp sge i16 %27, 0
  %29 = sext i1 %28 to i16
  %30 = icmp ne i16 %29, 0
  br i1 %30, label %b8, label %b9

b8:
  %31 = load i16, ptr @"I%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 4
  %33 = load i16, ptr %32, !tbaa !2
  %34 = icmp sle i16 %31, %33
  %35 = sext i1 %34 to i16
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %b10, label %b11

b9:
  %37 = load i16, ptr @"I%", !tbaa !2
  %38 = getelementptr i8, ptr @$data, i16 4
  %39 = load i16, ptr %38, !tbaa !2
  %40 = icmp sge i16 %37, %39
  %41 = sext i1 %40 to i16
  %42 = icmp ne i16 %41, 0
  br i1 %42, label %b10, label %b11

b10:
  %43 = load i16, ptr @"T%", !tbaa !2
  %44 = load i16, ptr @"I%", !tbaa !2
  %45 = add i16 %43, %44
  store i16 %45, ptr @"T%", !tbaa !2
  %46 = load i16, ptr @"I%", !tbaa !2
  %47 = getelementptr i8, ptr @$data, i16 6
  %48 = load i16, ptr %47, !tbaa !2
  %49 = add i16 %46, %48
  store i16 %49, ptr @"I%", !tbaa !2
  br label %b7

b11:
  store i16 1, ptr @"I%", !tbaa !2
  %50 = getelementptr i8, ptr @$data, i16 8
  store i16 0, ptr %50, !tbaa !2
  %51 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %51, !tbaa !2
  br label %b12

b12:
  %52 = getelementptr i8, ptr @$data, i16 10
  %53 = load i16, ptr %52, !tbaa !2
  %54 = icmp sge i16 %53, 0
  %55 = sext i1 %54 to i16
  %56 = icmp ne i16 %55, 0
  br i1 %56, label %b13, label %b14

b13:
  %57 = load i16, ptr @"I%", !tbaa !2
  %58 = getelementptr i8, ptr @$data, i16 8
  %59 = load i16, ptr %58, !tbaa !2
  %60 = icmp sle i16 %57, %59
  %61 = sext i1 %60 to i16
  %62 = icmp ne i16 %61, 0
  br i1 %62, label %b15, label %b16

b14:
  %63 = load i16, ptr @"I%", !tbaa !2
  %64 = getelementptr i8, ptr @$data, i16 8
  %65 = load i16, ptr %64, !tbaa !2
  %66 = icmp sge i16 %63, %65
  %67 = sext i1 %66 to i16
  %68 = icmp ne i16 %67, 0
  br i1 %68, label %b15, label %b16

b15:
  %69 = load i16, ptr @"T%", !tbaa !2
  %70 = add i16 %69, 999
  store i16 %70, ptr @"T%", !tbaa !2
  %71 = load i16, ptr @"I%", !tbaa !2
  %72 = getelementptr i8, ptr @$data, i16 10
  %73 = load i16, ptr %72, !tbaa !2
  %74 = add i16 %71, %73
  store i16 %74, ptr @"I%", !tbaa !2
  br label %b12

b16:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %75 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %75)
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
