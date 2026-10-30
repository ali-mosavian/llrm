target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@R = internal global [10 x i8] zeroinitializer
@Q = internal global [10 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string3, i16 4), [12 x i8] c"OPENMODE.DAT" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"seven\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"three\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string6, i16 4), [12 x i8] c"OPENMODE.DAT" }>
@$string7 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string7, i16 4), [12 x i8] c"OPENMODE.DAT" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string3, i16 1, i16 10, i16 772)
  store i16 7, ptr @R, !tbaa !2
  %0 = getelementptr inbounds i8, ptr @R, i16 2
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = getelementptr i8, ptr @$string4, i16 4
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %3, i16 5, ptr addrspace(1) nocapture writeonly initializes((0, 8)) %1, i16 8)
  %4 = addrspacecast ptr @R to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT4(i16 1, i32 2, ptr addrspace(1) %4, i16 10)
  store i16 3, ptr @R, !tbaa !2
  %5 = getelementptr inbounds i8, ptr @R, i16 2
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  %7 = getelementptr i8, ptr @$string5, i16 4
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %8, i16 5, ptr addrspace(1) nocapture writeonly initializes((0, 8)) %6, i16 8)
  %9 = addrspacecast ptr @R to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PUT4(i16 1, i32 1, ptr addrspace(1) %9, i16 10)
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string6, i16 2, i16 10, i16 4)
  %10 = addrspacecast ptr @Q to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 2, i32 2, ptr addrspace(1) %10, i16 10)
  %11 = load i16, ptr @Q, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %11)
  %12 = getelementptr inbounds i8, ptr @Q, i16 2
  %13 = addrspacecast ptr %12 to ptr addrspace(1)
  %14 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %13, i16 8)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %14)
  %15 = addrspacecast ptr @Q to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$GET4(i16 2, i32 1, ptr addrspace(1) %15, i16 10)
  %16 = load i16, ptr @Q, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %16)
  %17 = getelementptr inbounds i8, ptr @Q, i16 2
  %18 = addrspacecast ptr %17 to ptr addrspace(1)
  %19 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %18, i16 8)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %19)
  %20 = call cc 1000 addrspace(1) i32 @llrm.qb.B$FLOF(i16 2)
  %21 = sdiv i32 %20, 10
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %21)
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 2, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string7, i16 3, i16 -1, i16 257)
  %22 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FEOF(i16 3)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %22)
  call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 3, i16 1)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$ASSN(ptr addrspace(1), i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PUT4(i16, i32, ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CLOS(i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$GET4(i16, i32, ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LDFS(ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 i32 @llrm.qb.B$FLOF(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FEOF(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
