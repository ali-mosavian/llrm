target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@A$ = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"abc\00" }>
@B$ = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"abd\00" }>
@C$ = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"ab" }>
@E$ = internal global [4 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string6, i16 4) }>
@$string7 = internal constant <{ [2 x i8], ptr }> <{ [2 x i8] zeroinitializer, ptr getelementptr (i8, ptr @$string7, i16 4) }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"a\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @A$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string4, ptr @B$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string5, ptr @C$)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string6, ptr @E$)
  %0 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @$string7, ptr @$string8)
  %1 = icmp slt i16 %0, 0
  %2 = sext i1 %1 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %2)
  %3 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @E$, ptr @A$)
  %4 = icmp slt i16 %3, 0
  %5 = sext i1 %4 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %5)
  %6 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @A$, ptr @B$)
  %7 = icmp slt i16 %6, 0
  %8 = sext i1 %7 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %8)
  %9 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @B$, ptr @A$)
  %10 = icmp slt i16 %9, 0
  %11 = sext i1 %10 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %11)
  %12 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @C$, ptr @A$)
  %13 = icmp slt i16 %12, 0
  %14 = sext i1 %13 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %14)
  %15 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @A$, ptr @C$)
  %16 = icmp slt i16 %15, 0
  %17 = sext i1 %16 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %17)
  %18 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @E$, ptr @E$)
  %19 = icmp eq i16 %18, 0
  %20 = sext i1 %19 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %20)
  %21 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @A$, ptr @A$)
  %22 = icmp eq i16 %21, 0
  %23 = sext i1 %22 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %23)
  %24 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @A$, ptr @C$)
  %25 = icmp ne i16 %24, 0
  %26 = sext i1 %25 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %26)
  %27 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr @A$, ptr @E$)
  %28 = icmp sgt i16 %27, 0
  %29 = sext i1 %28 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %29)
  %30 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @A$, ptr @B$)
  %31 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @B$, ptr @A$)
  %32 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SCMP(ptr %30, ptr %31)
  %33 = icmp slt i16 %32, 0
  %34 = sext i1 %33 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %34)
  ret void
}

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$SCMP(ptr, ptr) addrspace(1) threeway

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
