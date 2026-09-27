target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"C&" = internal global [4 x i8] zeroinitializer
@"D&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [6 x i8] c"\04\00ONE=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [8 x i8] c"\06\00CONST=" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [10 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [10 x i8] c"\07\00CONST2=\00" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [10 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [10 x i8] c"\07\00MODMOD=\00" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [10 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [10 x i8] c"\07\00DIVDIV=\00" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [10 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [10 x i8] c"\07\00NEGMOD=\00" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [10 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [10 x i8] c"\07\00NEGDIV=\00" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 1073741831, ptr @"A&", !tbaa !2
  store i32 39678839, ptr @"B&", !tbaa !2
  store i32 -1049330653, ptr @"C&", !tbaa !2
  store i32 100003, ptr @"D&", !tbaa !2
  store i32 0, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %0 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %0)
  %1 = load i32, ptr @"A&", !tbaa !2
  %2 = srem i32 %1, 39678839
  %3 = srem i32 %2, 1
  store i32 %3, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %4 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %4)
  %5 = load i32, ptr @"A&", !tbaa !2
  %6 = srem i32 %5, 39678839
  %7 = load i32, ptr @"D&", !tbaa !2
  %8 = and i32 %7, 2147483647
  %9 = or i32 %8, 1
  %10 = srem i32 %6, %9
  store i32 %10, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %11 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  %12 = load i32, ptr @"A&", !tbaa !2
  %13 = load i32, ptr @"B&", !tbaa !2
  %14 = srem i32 %12, %13
  %15 = load i32, ptr @"D&", !tbaa !2
  %16 = and i32 %15, 2147483647
  %17 = or i32 %16, 1
  %18 = srem i32 %14, %17
  store i32 %18, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10$descriptor)
  %19 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %19)
  %20 = load i32, ptr @"A&", !tbaa !2
  %21 = load i32, ptr @"B&", !tbaa !2
  %22 = sdiv i32 %20, %21
  %23 = sdiv i32 %22, 3
  store i32 %23, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %24 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %24)
  store i32 -1073741831, ptr @"A&", !tbaa !2
  %25 = load i32, ptr @"B&", !tbaa !2
  %26 = srem i32 -1073741831, %25
  %27 = load i32, ptr @"D&", !tbaa !2
  %28 = and i32 %27, 2147483647
  %29 = or i32 %28, 1
  %30 = srem i32 %26, %29
  store i32 %30, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14$descriptor)
  %31 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %31)
  %32 = load i32, ptr @"A&", !tbaa !2
  %33 = load i32, ptr @"B&", !tbaa !2
  %34 = sdiv i32 %32, %33
  %35 = sdiv i32 %34, 3
  store i32 %35, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16$descriptor)
  %36 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %36)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string18$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
