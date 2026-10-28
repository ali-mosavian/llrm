target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"STOPAT%" = internal global [2 x i8] zeroinitializer
@"COUNTER%" = internal global [2 x i8] zeroinitializer
@"TOTAL&" = internal global [4 x i8] zeroinitializer
@$qb$readData = internal constant [3 x i8] c" 5\00"
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"COUNT=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"TOTAL=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"STOPAT%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  store i16 0, ptr @"COUNTER%", !tbaa !2
  store i32 0, ptr @"TOTAL&", !tbaa !2
  br label %b2

b2:
  %1 = load i16, ptr @"COUNTER%", !tbaa !2
  %2 = icmp slt i16 %1, 10
  br i1 %2, label %b3, label %b4

b3:
  %3 = load i16, ptr @"COUNTER%", !tbaa !2
  %4 = load i16, ptr @"STOPAT%", !tbaa !2
  %5 = icmp eq i16 %3, %4
  br i1 %5, label %b5, label %b6

b4:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %6 = load i16, ptr @"COUNTER%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %6)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %7 = load i32, ptr @"TOTAL&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %7)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b5:
  br label %b4

b6:
  br label %b7

b7:
  %8 = load i32, ptr @"TOTAL&", !tbaa !2
  %9 = load i16, ptr @"COUNTER%", !tbaa !2
  %10 = sext i16 %9 to i32
  %11 = add i32 %8, %10
  store i32 %11, ptr @"TOTAL&", !tbaa !2
  %12 = load i16, ptr @"COUNTER%", !tbaa !2
  %13 = add i16 %12, 1
  store i16 %13, ptr @"COUNTER%", !tbaa !2
  br label %b2
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
