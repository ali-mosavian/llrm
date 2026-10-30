target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@C = internal global [8 x i8] zeroinitializer
@PTS = internal global [24 x i8] zeroinitializer, align 2
@PTS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @PTS to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @PTS, [6 x i8] c"\08\00\03\00\00\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 305419896, ptr @C, !tbaa !2
  %0 = getelementptr inbounds i8, ptr @C, i16 4
  store i32 252645135, ptr %0, !tbaa !2
  %1 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  store i32 1, ptr %1, !tbaa !2
  %2 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %3 = getelementptr inbounds i8, ptr %2, i16 4
  store i32 2, ptr %3, !tbaa !2
  %4 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  store i32 3, ptr %4, !tbaa !2
  %5 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %6 = getelementptr inbounds i8, ptr %5, i16 4
  store i32 4, ptr %6, !tbaa !2
  %7 = getelementptr inbounds [8 x i8], ptr @PTS, i16 2
  store i32 5, ptr %7, !tbaa !2
  %8 = getelementptr inbounds [8 x i8], ptr @PTS, i16 2
  %9 = getelementptr inbounds i8, ptr %8, i16 4
  store i32 6, ptr %9, !tbaa !2
  %10 = load i32, ptr @C, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %10)
  %11 = getelementptr inbounds i8, ptr @C, i16 4
  %12 = load i32, ptr %11, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %12)
  %13 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %14 = load i32, ptr %13, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %14)
  %15 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %16 = getelementptr inbounds i8, ptr %15, i16 4
  %17 = load i32, ptr %16, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %17)
  %18 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %19 = load i32, ptr %18, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %19)
  %20 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %21 = getelementptr inbounds i8, ptr %20, i16 4
  %22 = load i32, ptr %21, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %22)
  %23 = getelementptr inbounds [8 x i8], ptr @PTS, i16 2
  %24 = load i32, ptr %23, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %24)
  %25 = getelementptr inbounds [8 x i8], ptr @PTS, i16 2
  %26 = getelementptr inbounds i8, ptr %25, i16 4
  %27 = load i32, ptr %26, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %27)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
