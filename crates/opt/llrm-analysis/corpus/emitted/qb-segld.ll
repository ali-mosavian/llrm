target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [18 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"J%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 100, i16 2, i16 257, ptr @"A%")
  store i16 0, ptr @"T%", !tbaa !2
  store i16 1, ptr @"J%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
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
  %6 = load i16, ptr @"J%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sle i16 %6, %7
  %9 = sext i1 %8 to i16
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr @"J%", !tbaa !2
  %12 = load i16, ptr @$data, !tbaa !2
  %13 = icmp sge i16 %11, %12
  %14 = sext i1 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b5, label %b6

b5:
  store i16 1, ptr @"I%", !tbaa !2
  %16 = getelementptr i8, ptr @$data, i16 4
  store i16 20, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %17, !tbaa !2
  br label %b7

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %18 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %18)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6$descriptor)
  ret void

b7:
  %19 = getelementptr i8, ptr @$data, i16 6
  %20 = load i16, ptr %19, !tbaa !2
  %21 = icmp sge i16 %20, 0
  %22 = sext i1 %21 to i16
  %23 = icmp ne i16 %22, 0
  br i1 %23, label %b8, label %b9

b8:
  %24 = load i16, ptr @"I%", !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 4
  %26 = load i16, ptr %25, !tbaa !2
  %27 = icmp sle i16 %24, %26
  %28 = sext i1 %27 to i16
  %29 = icmp ne i16 %28, 0
  br i1 %29, label %b10, label %b11

b9:
  %30 = load i16, ptr @"I%", !tbaa !2
  %31 = getelementptr i8, ptr @$data, i16 4
  %32 = load i16, ptr %31, !tbaa !2
  %33 = icmp sge i16 %30, %32
  %34 = sext i1 %33 to i16
  %35 = icmp ne i16 %34, 0
  br i1 %35, label %b10, label %b11

b10:
  %36 = load i16, ptr @"I%", !tbaa !2
  %37 = mul i16 %36, 2
  %38 = getelementptr i8, ptr @"A%", i16 2
  %39 = load i16, ptr %38, !tbaa !2
  %40 = add i16 0, %37
  %41 = inttoptr i16 %39 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 %40
  %44 = load i16, ptr @"I%", !tbaa !2
  store i16 %44, ptr addrspace(1) %43, !tbaa !4
  %45 = load i16, ptr @"T%", !tbaa !2
  %46 = load i16, ptr @"I%", !tbaa !2
  %47 = mul i16 %46, 2
  %48 = getelementptr i8, ptr @"A%", i16 2
  %49 = load i16, ptr %48, !tbaa !2
  %50 = add i16 0, %47
  %51 = inttoptr i16 %49 to ptr addrspace(2)
  %52 = addrspacecast ptr addrspace(2) %51 to ptr addrspace(1)
  %53 = getelementptr i8, ptr addrspace(1) %52, i16 %50
  %54 = load i16, ptr addrspace(1) %53, !tbaa !4
  %55 = add i16 %45, %54
  store i16 %55, ptr @"T%", !tbaa !2
  %56 = load i16, ptr @"I%", !tbaa !2
  %57 = getelementptr i8, ptr @$data, i16 6
  %58 = load i16, ptr %57, !tbaa !2
  %59 = add i16 %56, %58
  store i16 %59, ptr @"I%", !tbaa !2
  br label %b7

b11:
  %60 = load i16, ptr @"J%", !tbaa !2
  %61 = getelementptr i8, ptr @$data, i16 2
  %62 = load i16, ptr %61, !tbaa !2
  %63 = add i16 %60, %62
  store i16 %63, ptr @"J%", !tbaa !2
  br label %b2
}

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
