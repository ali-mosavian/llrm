target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"NOT=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"EQV=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"IMP=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"NAND=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"NOTOR=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  %0 = load i32, ptr @"A&", !tbaa !2
  %1 = xor i32 %0, -1
  store i32 %1, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %2 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %2)
  %3 = load i32, ptr @"A&", !tbaa !2
  %4 = load i32, ptr @"B&", !tbaa !2
  %5 = xor i32 %3, %4
  %6 = xor i32 %5, -1
  store i32 %6, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %7 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %7)
  %8 = load i32, ptr @"A&", !tbaa !2
  %9 = load i32, ptr @"B&", !tbaa !2
  %10 = xor i32 %8, -1
  %11 = or i32 %10, %9
  store i32 %11, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %12 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %12)
  %13 = load i32, ptr @"A&", !tbaa !2
  %14 = load i32, ptr @"B&", !tbaa !2
  %15 = and i32 %13, %14
  %16 = xor i32 %15, -1
  store i32 %16, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %17 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %17)
  %18 = load i32, ptr @"A&", !tbaa !2
  %19 = xor i32 %18, -1
  %20 = load i32, ptr @"B&", !tbaa !2
  %21 = or i32 %19, %20
  store i32 %21, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %22 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %22)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
