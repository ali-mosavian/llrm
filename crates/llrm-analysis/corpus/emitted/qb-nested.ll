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
  %0 = load i16, ptr @"H%", !tbaa !2
  %1 = sub i16 %0, 1
  store i16 %1, ptr @$data, !tbaa !2
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
  store i16 0, ptr @"J%", !tbaa !2
  %18 = load i16, ptr @"W%", !tbaa !2
  %19 = sub i16 %18, 1
  %20 = getelementptr i8, ptr @$data, i16 4
  store i16 %19, ptr %20, !tbaa !2
  %21 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %21, !tbaa !2
  br label %b7

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %22 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %22)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  ret void

b7:
  %23 = getelementptr i8, ptr @$data, i16 6
  %24 = load i16, ptr %23, !tbaa !2
  %25 = icmp sge i16 %24, 0
  %26 = sext i1 %25 to i16
  %27 = icmp ne i16 %26, 0
  br i1 %27, label %b8, label %b9

b8:
  %28 = load i16, ptr @"J%", !tbaa !2
  %29 = getelementptr i8, ptr @$data, i16 4
  %30 = load i16, ptr %29, !tbaa !2
  %31 = icmp sle i16 %28, %30
  %32 = sext i1 %31 to i16
  %33 = icmp ne i16 %32, 0
  br i1 %33, label %b10, label %b11

b9:
  %34 = load i16, ptr @"J%", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 4
  %36 = load i16, ptr %35, !tbaa !2
  %37 = icmp sge i16 %34, %36
  %38 = sext i1 %37 to i16
  %39 = icmp ne i16 %38, 0
  br i1 %39, label %b10, label %b11

b10:
  %40 = load i16, ptr @"I%", !tbaa !2
  %41 = load i16, ptr @"W%", !tbaa !2
  %42 = mul i16 %40, %41
  %43 = load i16, ptr @"J%", !tbaa !2
  %44 = add i16 %42, %43
  %45 = load i16, ptr @"I%", !tbaa !2
  %46 = mul i16 %45, 10
  %47 = load i16, ptr @"J%", !tbaa !2
  %48 = add i16 %46, %47
  %49 = sub i16 %44, 0
  %50 = getelementptr inbounds i16, ptr @"B%", i16 %49
  store i16 %48, ptr %50, !tbaa !2
  %51 = load i16, ptr @"T%", !tbaa !2
  %52 = load i16, ptr @"I%", !tbaa !2
  %53 = load i16, ptr @"W%", !tbaa !2
  %54 = mul i16 %52, %53
  %55 = load i16, ptr @"J%", !tbaa !2
  %56 = add i16 %54, %55
  %57 = sub i16 %56, 0
  %58 = getelementptr inbounds i16, ptr @"B%", i16 %57
  %59 = load i16, ptr %58, !tbaa !2
  %60 = add i16 %51, %59
  store i16 %60, ptr @"T%", !tbaa !2
  %61 = load i16, ptr @"J%", !tbaa !2
  %62 = getelementptr i8, ptr @$data, i16 6
  %63 = load i16, ptr %62, !tbaa !2
  %64 = add i16 %61, %63
  store i16 %64, ptr @"J%", !tbaa !2
  br label %b7

b11:
  %65 = load i16, ptr @"I%", !tbaa !2
  %66 = getelementptr i8, ptr @$data, i16 2
  %67 = load i16, ptr %66, !tbaa !2
  %68 = add i16 %65, %67
  store i16 %68, ptr @"I%", !tbaa !2
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
