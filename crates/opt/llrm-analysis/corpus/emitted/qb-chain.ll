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
  %0 = sub i32 0, 1049330653
  store i32 %0, ptr @"C&", !tbaa !2
  store i32 100003, ptr @"D&", !tbaa !2
  %1 = load i32, ptr @"A&", !tbaa !2
  %2 = load i32, ptr @"B&", !tbaa !2
  %3 = srem i32 %1, %2
  %4 = load i32, ptr @"C&", !tbaa !2
  %5 = load i32, ptr @"C&", !tbaa !2
  %6 = xor i32 %4, %5
  %7 = and i32 %6, 2147483647
  %8 = or i32 %7, 1
  %9 = srem i32 %3, %8
  store i32 %9, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %10 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %10)
  %11 = load i32, ptr @"A&", !tbaa !2
  %12 = srem i32 %11, 39678839
  %13 = load i32, ptr @"C&", !tbaa !2
  %14 = load i32, ptr @"C&", !tbaa !2
  %15 = xor i32 %13, %14
  %16 = and i32 %15, 2147483647
  %17 = or i32 %16, 1
  %18 = srem i32 %12, %17
  store i32 %18, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %19 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %19)
  %20 = load i32, ptr @"A&", !tbaa !2
  %21 = srem i32 %20, 39678839
  %22 = load i32, ptr @"D&", !tbaa !2
  %23 = and i32 %22, 2147483647
  %24 = or i32 %23, 1
  %25 = srem i32 %21, %24
  store i32 %25, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %26 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %26)
  %27 = load i32, ptr @"A&", !tbaa !2
  %28 = load i32, ptr @"B&", !tbaa !2
  %29 = srem i32 %27, %28
  %30 = load i32, ptr @"D&", !tbaa !2
  %31 = and i32 %30, 2147483647
  %32 = or i32 %31, 1
  %33 = srem i32 %29, %32
  store i32 %33, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10$descriptor)
  %34 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %34)
  %35 = load i32, ptr @"A&", !tbaa !2
  %36 = load i32, ptr @"B&", !tbaa !2
  %37 = sdiv i32 %35, %36
  %38 = load i32, ptr @"C&", !tbaa !2
  %39 = load i32, ptr @"C&", !tbaa !2
  %40 = xor i32 %38, %39
  %41 = and i32 %40, 32767
  %42 = or i32 %41, 3
  %43 = sdiv i32 %37, %42
  store i32 %43, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %44 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %44)
  %45 = sub i32 0, 1073741831
  store i32 %45, ptr @"A&", !tbaa !2
  %46 = load i32, ptr @"A&", !tbaa !2
  %47 = load i32, ptr @"B&", !tbaa !2
  %48 = srem i32 %46, %47
  %49 = load i32, ptr @"D&", !tbaa !2
  %50 = and i32 %49, 2147483647
  %51 = or i32 %50, 1
  %52 = srem i32 %48, %51
  store i32 %52, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14$descriptor)
  %53 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %53)
  %54 = load i32, ptr @"A&", !tbaa !2
  %55 = load i32, ptr @"B&", !tbaa !2
  %56 = sdiv i32 %54, %55
  %57 = load i32, ptr @"C&", !tbaa !2
  %58 = load i32, ptr @"C&", !tbaa !2
  %59 = xor i32 %57, %58
  %60 = and i32 %59, 32767
  %61 = or i32 %60, 3
  %62 = sdiv i32 %56, %61
  store i32 %62, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16$descriptor)
  %63 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %63)
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
