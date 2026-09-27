target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@"LO&" = internal global [4 x i8] zeroinitializer
@"HI&" = internal global [4 x i8] zeroinitializer
@"ONE&" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"J%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [6 x i8] c"\04\00AND=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [6 x i8] c"\03\00OR=\00" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [6 x i8] c"\04\00XOR=" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [6 x i8] c"\04\00ADD=" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [6 x i8] c"\04\00SUB=" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [6 x i8] c"\04\00NEG=" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [8 x i8] c"\06\00CHAIN=" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [8 x i8] c"\06\00CARRY=" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>
@$string20$payload = internal addrspace(1) constant <{ [2 x i8], i16, [10 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 4) to i16), [10 x i8] c"\07\00BORROW=\00" }>
@$string20$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 2) to i16), ptr @$fslSegment }>
@$string22$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 4) to i16), [8 x i8] c"\05\00INTS=\00" }>
@$string22$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 2) to i16), ptr @$fslSegment }>
@$string24$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string24$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  store i32 1, ptr @"ONE&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %0 = load i32, ptr @"A&", !tbaa !2
  %1 = load i32, ptr @"B&", !tbaa !2
  %2 = and i32 %0, %1
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %3 = load i32, ptr @"A&", !tbaa !2
  %4 = load i32, ptr @"B&", !tbaa !2
  %5 = or i32 %3, %4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %5)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %6 = load i32, ptr @"A&", !tbaa !2
  %7 = load i32, ptr @"B&", !tbaa !2
  %8 = xor i32 %6, %7
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %8)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10$descriptor)
  %9 = load i32, ptr @"A&", !tbaa !2
  %10 = load i32, ptr @"B&", !tbaa !2
  %11 = add i32 %9, %10
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %12 = load i32, ptr @"A&", !tbaa !2
  %13 = load i32, ptr @"B&", !tbaa !2
  %14 = sub i32 %12, %13
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %14)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14$descriptor)
  %15 = load i32, ptr @"A&", !tbaa !2
  %16 = sub i32 0, %15
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %16)
  %17 = load i32, ptr @"A&", !tbaa !2
  store i32 %17, ptr @"R&", !tbaa !2
  %18 = load i32, ptr @"R&", !tbaa !2
  %19 = load i32, ptr @"B&", !tbaa !2
  %20 = and i32 %18, %19
  store i32 %20, ptr @"R&", !tbaa !2
  %21 = load i32, ptr @"R&", !tbaa !2
  %22 = load i32, ptr @"A&", !tbaa !2
  %23 = xor i32 %21, %22
  store i32 %23, ptr @"R&", !tbaa !2
  %24 = load i32, ptr @"R&", !tbaa !2
  %25 = load i32, ptr @"B&", !tbaa !2
  %26 = add i32 %24, %25
  store i32 %26, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16$descriptor)
  %27 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %27)
  store i32 65535, ptr @"LO&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18$descriptor)
  %28 = load i32, ptr @"LO&", !tbaa !2
  %29 = load i32, ptr @"ONE&", !tbaa !2
  %30 = add i32 %28, %29
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %30)
  store i32 65536, ptr @"HI&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20$descriptor)
  %31 = load i32, ptr @"HI&", !tbaa !2
  %32 = load i32, ptr @"ONE&", !tbaa !2
  %33 = sub i32 %31, %32
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %33)
  store i16 258, ptr @"I%", !tbaa !2
  store i16 772, ptr @"J%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string22$descriptor)
  %34 = load i16, ptr @"I%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %34)
  %35 = load i16, ptr @"J%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %35)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string24$descriptor)
  ret void
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
