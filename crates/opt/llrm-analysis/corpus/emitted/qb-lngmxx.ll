target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"V&" = internal global [4 x i8] zeroinitializer
@"S&" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [8 x i8] c" 100000\00"
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00S=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"V&" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %0)
  store i32 0, ptr @"S&", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = getelementptr i8, ptr @$data, i16 2
  %3 = load i16, ptr %2, !tbaa !2
  %4 = icmp sge i16 %3, 0
  %5 = sext i1 %4 to i16
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sle i16 %7, %8
  %10 = sext i1 %9 to i16
  %11 = icmp ne i16 %10, 0
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr @"I%", !tbaa !2
  %13 = load i16, ptr @$data, !tbaa !2
  %14 = icmp sge i16 %12, %13
  %15 = sext i1 %14 to i16
  %16 = icmp ne i16 %15, 0
  br i1 %16, label %b5, label %b6

b5:
  %17 = load i32, ptr @"S&", !tbaa !2
  %18 = load i32, ptr @"V&", !tbaa !2
  %19 = sdiv i32 %18, 7
  %20 = add i32 %17, %19
  %21 = load i32, ptr @"V&", !tbaa !2
  %22 = srem i32 %21, 7
  %23 = add i32 %20, %22
  store i32 %23, ptr @"S&", !tbaa !2
  %24 = load i16, ptr @"I%", !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = add i16 %24, %26
  store i16 %27, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %28 = load i32, ptr @"S&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %28)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  ret void
}

declare cc1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
