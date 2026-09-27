target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@PTS = internal global [16 x i8] zeroinitializer
@PTS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @PTS to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @PTS, [6 x i8] c"\08\00\02\00\00\00" }>
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  %0 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  store i32 11, ptr %0, !tbaa !2
  %1 = getelementptr inbounds i8, ptr %0, i16 4
  store i32 22, ptr %1, !tbaa !2
  %2 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  store i32 33, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i8, ptr %2, i16 4
  store i32 44, ptr %3, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 11)
  %4 = load i32, ptr %2, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %4)
  %5 = load i32, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %5)
  %6 = load i32, ptr %3, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %6)
  call cc1000 addrspace(1) void @INSIDE()
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4$descriptor)
  ret void
}

define cc1000 void @INSIDE() addrspace(1) {
b1:
  %0 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 8, i16 257, ptr %0)
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = inttoptr i16 %2 to ptr addrspace(2)
  %4 = addrspacecast ptr addrspace(2) %3 to ptr addrspace(1)
  %5 = getelementptr i8, ptr addrspace(1) %4, i16 0
  store i32 55, ptr addrspace(1) %5, !tbaa !4
  %6 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 4
  store i32 66, ptr addrspace(1) %6, !tbaa !4
  %7 = getelementptr i8, ptr addrspace(1) %4, i16 8
  store i32 77, ptr addrspace(1) %7, !tbaa !4
  %8 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 4
  store i32 88, ptr addrspace(1) %8, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 55)
  %9 = load i16, ptr %1, !tbaa !2
  %10 = inttoptr i16 %9 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 8
  %13 = load i32, ptr addrspace(1) %12, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %13)
  %14 = load i16, ptr %1, !tbaa !2
  %15 = inttoptr i16 %14 to ptr addrspace(2)
  %16 = addrspacecast ptr addrspace(2) %15 to ptr addrspace(1)
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 0
  %18 = getelementptr inbounds i8, ptr addrspace(1) %17, i16 4
  %19 = load i32, ptr addrspace(1) %18, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %19)
  %20 = load i16, ptr %1, !tbaa !2
  %21 = inttoptr i16 %20 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 8
  %24 = getelementptr inbounds i8, ptr addrspace(1) %23, i16 4
  %25 = load i32, ptr addrspace(1) %24, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %25)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %0)
  ret void
}

declare cc1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
