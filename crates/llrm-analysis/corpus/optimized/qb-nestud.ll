target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@O = internal global [6 x i8] zeroinitializer
@ARR = internal global [12 x i8] zeroinitializer
@ARR$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @ARR to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @ARR, [6 x i8] c"\06\00\02\00\00\00" }>
@LASTVAR = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00AB" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [4 x i8] c"\02\00CD" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>
@$string9$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string9$payload, i16 4) to i16), [4 x i8] c"\02\00EF" }>
@$string9$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string9$payload, i16 2) to i16), ptr @$fslSegment }>
@$string11$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string11$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string11$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string11$payload, i16 2) to i16), ptr @$fslSegment }>
@$string13$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string13$payload, i16 4) to i16), [4 x i8] c"\02\00GH" }>
@$string13$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string13$payload, i16 2) to i16), ptr @$fslSegment }>
@$string15$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string15$payload, i16 4) to i16), [4 x i8] c"\02\00IJ" }>
@$string15$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string15$payload, i16 2) to i16), ptr @$fslSegment }>
@$string17$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string17$payload, i16 4) to i16), [4 x i8] c"\02\00KL" }>
@$string17$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string17$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 111, ptr @O, !tbaa !2
  %0 = getelementptr inbounds i8, ptr @O, i16 4
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = getelementptr i8, ptr addrspace(1) @$string4$payload, i16 6
  %3 = addrspacecast ptr addrspace(1) %2 to ptr
  %4 = ptrtoint ptr %3 to i16
  %5 = load i16, ptr @$fslSegment, !tbaa !2
  %6 = inttoptr i16 %5 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 %4
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %8, i16 2, ptr addrspace(1) %1, i16 2)
  %9 = getelementptr inbounds [6 x i8], ptr @ARR, i16 0
  store i32 1, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %9, i16 4
  %11 = addrspacecast ptr %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) @$string7$payload, i16 6
  %13 = addrspacecast ptr addrspace(1) %12 to ptr
  %14 = ptrtoint ptr %13 to i16
  %15 = load i16, ptr @$fslSegment, !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2)
  %17 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 %14
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %18, i16 2, ptr addrspace(1) %11, i16 2)
  %19 = getelementptr inbounds [6 x i8], ptr @ARR, i16 1
  store i32 2, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %19, i16 4
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  %22 = getelementptr i8, ptr addrspace(1) @$string9$payload, i16 6
  %23 = addrspacecast ptr addrspace(1) %22 to ptr
  %24 = ptrtoint ptr %23 to i16
  %25 = load i16, ptr @$fslSegment, !tbaa !2
  %26 = inttoptr i16 %25 to ptr addrspace(2)
  %27 = addrspacecast ptr addrspace(2) %26 to ptr addrspace(1)
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 %24
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %28, i16 2, ptr addrspace(1) %21, i16 2)
  %29 = load i32, ptr @O, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %29)
  %30 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %1, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %30)
  %31 = load i32, ptr %9, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %31)
  %32 = load i32, ptr %19, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %32)
  %33 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %11, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %33)
  %34 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %21, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %34)
  call cc1000 addrspace(1) void @INSIDE()
  store i32 999, ptr @LASTVAR, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 999)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11$descriptor)
  ret void
}

define cc1000 void @INSIDE() addrspace(1) {
b1:
  %0 = alloca [18 x i8]
  %1 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 18, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 6, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 6, i16 257, ptr %0)
  store i32 333, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 4
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  %4 = getelementptr i8, ptr addrspace(1) @$string13$payload, i16 6
  %5 = addrspacecast ptr addrspace(1) %4 to ptr
  %6 = ptrtoint ptr %5 to i16
  %7 = load i16, ptr @$fslSegment, !tbaa !2
  %8 = inttoptr i16 %7 to ptr addrspace(2)
  %9 = addrspacecast ptr addrspace(2) %8 to ptr addrspace(1)
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 %6
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %10, i16 2, ptr addrspace(1) %3, i16 2)
  %11 = getelementptr i8, ptr %0, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  %13 = inttoptr i16 %12 to ptr addrspace(2)
  %14 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 0
  store i32 4, ptr addrspace(1) %15, !tbaa !4
  %16 = getelementptr i8, ptr addrspace(1) %15, i16 4
  %17 = getelementptr i8, ptr addrspace(1) @$string15$payload, i16 6
  %18 = addrspacecast ptr addrspace(1) %17 to ptr
  %19 = ptrtoint ptr %18 to i16
  %20 = load i16, ptr @$fslSegment, !tbaa !2
  %21 = inttoptr i16 %20 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 %19
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %23, i16 2, ptr addrspace(1) %16, i16 2)
  %24 = load i16, ptr %11, !tbaa !2
  %25 = inttoptr i16 %24 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 6
  store i32 5, ptr addrspace(1) %27, !tbaa !4
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 4
  %29 = getelementptr i8, ptr addrspace(1) @$string17$payload, i16 6
  %30 = addrspacecast ptr addrspace(1) %29 to ptr
  %31 = ptrtoint ptr %30 to i16
  %32 = load i16, ptr @$fslSegment, !tbaa !2
  %33 = inttoptr i16 %32 to ptr addrspace(2)
  %34 = addrspacecast ptr addrspace(2) %33 to ptr addrspace(1)
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 %31
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %35, i16 2, ptr addrspace(1) %28, i16 2)
  %36 = load i32, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %36)
  %37 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %3, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %37)
  %38 = load i16, ptr %11, !tbaa !2
  %39 = inttoptr i16 %38 to ptr addrspace(2)
  %40 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 0
  %42 = load i32, ptr addrspace(1) %41, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %42)
  %43 = load i16, ptr %11, !tbaa !2
  %44 = inttoptr i16 %43 to ptr addrspace(2)
  %45 = addrspacecast ptr addrspace(2) %44 to ptr addrspace(1)
  %46 = getelementptr i8, ptr addrspace(1) %45, i16 6
  %47 = load i32, ptr addrspace(1) %46, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %47)
  %48 = load i16, ptr %11, !tbaa !2
  %49 = inttoptr i16 %48 to ptr addrspace(2)
  %50 = addrspacecast ptr addrspace(2) %49 to ptr addrspace(1)
  %51 = getelementptr i8, ptr addrspace(1) %50, i16 0
  %52 = getelementptr i8, ptr addrspace(1) %51, i16 4
  %53 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %52, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %53)
  %54 = load i16, ptr %11, !tbaa !2
  %55 = inttoptr i16 %54 to ptr addrspace(2)
  %56 = addrspacecast ptr addrspace(2) %55 to ptr addrspace(1)
  %57 = getelementptr i8, ptr addrspace(1) %56, i16 6
  %58 = getelementptr i8, ptr addrspace(1) %57, i16 4
  %59 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %58, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %59)
  call cc1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %0)
  ret void
}

declare cc1000 void @llrm.qb.B$ASSN(ptr addrspace(1), i16, ptr addrspace(1), i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LDFS(ptr addrspace(1), i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare cc1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
