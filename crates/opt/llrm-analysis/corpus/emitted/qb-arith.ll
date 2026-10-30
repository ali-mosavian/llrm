target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"AND=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"OR=\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"XOR=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"ADD=" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"SUB=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"NEG=" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"CHAIN=" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"CARRY=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string11, i16 4), [8 x i8] c"BORROW=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string12, i16 4), [6 x i8] c"INTS=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  store i32 1, ptr @"ONE&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %0 = load i32, ptr @"A&", !tbaa !2
  %1 = load i32, ptr @"B&", !tbaa !2
  %2 = and i32 %0, %1
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %3 = load i32, ptr @"A&", !tbaa !2
  %4 = load i32, ptr @"B&", !tbaa !2
  %5 = or i32 %3, %4
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %5)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %6 = load i32, ptr @"A&", !tbaa !2
  %7 = load i32, ptr @"B&", !tbaa !2
  %8 = xor i32 %6, %7
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %8)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %9 = load i32, ptr @"A&", !tbaa !2
  %10 = load i32, ptr @"B&", !tbaa !2
  %11 = add i32 %9, %10
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %12 = load i32, ptr @"A&", !tbaa !2
  %13 = load i32, ptr @"B&", !tbaa !2
  %14 = sub i32 %12, %13
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %14)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %15 = load i32, ptr @"A&", !tbaa !2
  %16 = sub i32 0, %15
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %16)
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
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  %27 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %27)
  store i32 65535, ptr @"LO&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  %28 = load i32, ptr @"LO&", !tbaa !2
  %29 = load i32, ptr @"ONE&", !tbaa !2
  %30 = add i32 %28, %29
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %30)
  store i32 65536, ptr @"HI&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
  %31 = load i32, ptr @"HI&", !tbaa !2
  %32 = load i32, ptr @"ONE&", !tbaa !2
  %33 = sub i32 %31, %32
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %33)
  store i16 258, ptr @"I%", !tbaa !2
  store i16 772, ptr @"J%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  %34 = load i16, ptr @"I%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %34)
  %35 = load i16, ptr @"J%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %35)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string13)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
