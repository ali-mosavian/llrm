target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"M%" = internal global [22 x i8] zeroinitializer
@"R%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 20, i16 0, i16 20, i16 2, i16 258, ptr @"M%")
  store i16 0, ptr @"T%", !tbaa !2
  store i16 1, ptr @"R%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 4
  %2 = getelementptr i8, ptr @$data, i16 6
  %3 = getelementptr i8, ptr @"M%", i16 2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = inttoptr i16 %4 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  br label %b2

b2:
  %7 = load i16, ptr @"R%", !tbaa !2
  %8 = icmp sle i16 %7, 10
  br i1 %8, label %b5, label %b6

b5:
  store i16 1, ptr @"C%", !tbaa !2
  store i16 10, ptr %1, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b8

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %9 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %9)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6$descriptor)
  ret void

b8:
  %10 = load i16, ptr @"C%", !tbaa !2
  %11 = icmp sle i16 %10, 10
  br i1 %11, label %b10, label %b11

b10:
  %12 = mul i16 %10, 21
  %13 = add i16 %12, %7
  %14 = shl i16 %13, 1
  %15 = getelementptr i8, ptr addrspace(1) %6, i16 %14
  %16 = add i16 %7, %10
  store i16 %16, ptr addrspace(1) %15, !tbaa !4
  %17 = load i16, ptr @"T%", !tbaa !2
  %18 = load i16, ptr addrspace(1) %15, !tbaa !4
  %19 = add i16 %17, %18
  store i16 %19, ptr @"T%", !tbaa !2
  %20 = add i16 %10, 1
  store i16 %20, ptr @"C%", !tbaa !2
  br label %b8

b11:
  %21 = load i16, ptr @"R%", !tbaa !2
  %22 = add i16 %21, 1
  store i16 %22, ptr @"R%", !tbaa !2
  br label %b2
}

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
