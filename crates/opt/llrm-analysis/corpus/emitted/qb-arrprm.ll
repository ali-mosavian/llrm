target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"NUMS&" = internal global [8 x i8] zeroinitializer, align 2
@NUMS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"NUMS&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"NUMS&", [6 x i8] c"\04\00\02\00\00\00" }>
@PTS = internal global [16 x i8] zeroinitializer, align 2
@PTS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @PTS to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @PTS, [6 x i8] c"\08\00\02\00\00\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @FILLNUMS(ptr @NUMS$descriptor)
  call cc 1000 addrspace(1) void @FILLPTS(ptr @PTS$descriptor)
  %0 = getelementptr inbounds i32, ptr @"NUMS&", i16 0
  %1 = load i32, ptr %0, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %1)
  %2 = getelementptr inbounds i32, ptr @"NUMS&", i16 1
  %3 = load i32, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %3)
  %4 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %5 = load i32, ptr %4, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %5)
  %6 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %7 = getelementptr inbounds i8, ptr %6, i16 4
  %8 = load i32, ptr %7, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %8)
  %9 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %10 = load i32, ptr %9, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %10)
  %11 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %12 = getelementptr inbounds i8, ptr %11, i16 4
  %13 = load i32, ptr %12, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %13)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

define cc 1000 void @FILLNUMS(ptr dereferenceable(18) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load i16, ptr %1
  %3 = getelementptr i8, ptr %0, i16 10
  %4 = load i16, ptr %3
  %5 = add i16 %4, 0
  %6 = inttoptr i16 %2 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 %5
  store i32 7, ptr addrspace(1) %8
  %9 = add i16 %4, 4
  %10 = inttoptr i16 %2 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 %9
  store i32 8, ptr addrspace(1) %12
  ret void
}

define cc 1000 void @FILLPTS(ptr dereferenceable(18) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load i16, ptr %1
  %3 = getelementptr i8, ptr %0, i16 10
  %4 = load i16, ptr %3
  %5 = add i16 %4, 0
  %6 = inttoptr i16 %2 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 %5
  store i32 1, ptr addrspace(1) %8
  %9 = add i16 %4, 0
  %10 = inttoptr i16 %2 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 %9
  %13 = getelementptr inbounds i8, ptr addrspace(1) %12, i16 4
  store i32 2, ptr addrspace(1) %13
  %14 = add i16 %4, 8
  %15 = inttoptr i16 %2 to ptr addrspace(2)
  %16 = addrspacecast ptr addrspace(2) %15 to ptr addrspace(1)
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 %14
  store i32 3, ptr addrspace(1) %17
  %18 = add i16 %4, 8
  %19 = inttoptr i16 %2 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %18
  %22 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 4
  store i32 4, ptr addrspace(1) %22
  ret void
}

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
