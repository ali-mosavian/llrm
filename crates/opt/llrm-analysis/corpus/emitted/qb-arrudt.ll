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
  %0 = sub i16 0, 0
  %1 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %0
  store i32 11, ptr %1, !tbaa !2
  %2 = sub i16 0, 0
  %3 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %2
  %4 = getelementptr inbounds i8, ptr %3, i16 4
  store i32 22, ptr %4, !tbaa !2
  %5 = sub i16 1, 0
  %6 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %5
  store i32 33, ptr %6, !tbaa !2
  %7 = sub i16 1, 0
  %8 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %7
  %9 = getelementptr inbounds i8, ptr %8, i16 4
  store i32 44, ptr %9, !tbaa !2
  %10 = sub i16 0, 0
  %11 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %10
  %12 = load i32, ptr %11, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %12)
  %13 = sub i16 1, 0
  %14 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %13
  %15 = load i32, ptr %14, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %15)
  %16 = sub i16 0, 0
  %17 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %16
  %18 = getelementptr inbounds i8, ptr %17, i16 4
  %19 = load i32, ptr %18, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %19)
  %20 = sub i16 1, 0
  %21 = getelementptr inbounds [8 x i8], ptr @PTS, i16 %20
  %22 = getelementptr inbounds i8, ptr %21, i16 4
  %23 = load i32, ptr %22, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %23)
  call cc1000 addrspace(1) void @INSIDE()
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4$descriptor)
  ret void
}

define cc1000 void @INSIDE() addrspace(1) {
b1:
  %0 = alloca [18 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 18, i1 false)
  call cc1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 8, i16 257, ptr %0)
  %1 = mul i16 0, 8
  %2 = getelementptr i8, ptr %0, i16 2
  %3 = load i16, ptr %2, !tbaa !2
  %4 = add i16 0, %1
  %5 = inttoptr i16 %3 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 %4
  store i32 55, ptr addrspace(1) %7, !tbaa !4
  %8 = mul i16 0, 8
  %9 = getelementptr i8, ptr %0, i16 2
  %10 = load i16, ptr %9, !tbaa !2
  %11 = add i16 0, %8
  %12 = inttoptr i16 %10 to ptr addrspace(2)
  %13 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr addrspace(1) %13, i16 %11
  %15 = getelementptr inbounds i8, ptr addrspace(1) %14, i16 4
  store i32 66, ptr addrspace(1) %15, !tbaa !4
  %16 = mul i16 1, 8
  %17 = getelementptr i8, ptr %0, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = add i16 0, %16
  %20 = inttoptr i16 %18 to ptr addrspace(2)
  %21 = addrspacecast ptr addrspace(2) %20 to ptr addrspace(1)
  %22 = getelementptr i8, ptr addrspace(1) %21, i16 %19
  store i32 77, ptr addrspace(1) %22, !tbaa !4
  %23 = mul i16 1, 8
  %24 = getelementptr i8, ptr %0, i16 2
  %25 = load i16, ptr %24, !tbaa !2
  %26 = add i16 0, %23
  %27 = inttoptr i16 %25 to ptr addrspace(2)
  %28 = addrspacecast ptr addrspace(2) %27 to ptr addrspace(1)
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 %26
  %30 = getelementptr inbounds i8, ptr addrspace(1) %29, i16 4
  store i32 88, ptr addrspace(1) %30, !tbaa !4
  %31 = mul i16 0, 8
  %32 = getelementptr i8, ptr %0, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = add i16 0, %31
  %35 = inttoptr i16 %33 to ptr addrspace(2)
  %36 = addrspacecast ptr addrspace(2) %35 to ptr addrspace(1)
  %37 = getelementptr i8, ptr addrspace(1) %36, i16 %34
  %38 = load i32, ptr addrspace(1) %37, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %38)
  %39 = mul i16 1, 8
  %40 = getelementptr i8, ptr %0, i16 2
  %41 = load i16, ptr %40, !tbaa !2
  %42 = add i16 0, %39
  %43 = inttoptr i16 %41 to ptr addrspace(2)
  %44 = addrspacecast ptr addrspace(2) %43 to ptr addrspace(1)
  %45 = getelementptr i8, ptr addrspace(1) %44, i16 %42
  %46 = load i32, ptr addrspace(1) %45, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %46)
  %47 = mul i16 0, 8
  %48 = getelementptr i8, ptr %0, i16 2
  %49 = load i16, ptr %48, !tbaa !2
  %50 = add i16 0, %47
  %51 = inttoptr i16 %49 to ptr addrspace(2)
  %52 = addrspacecast ptr addrspace(2) %51 to ptr addrspace(1)
  %53 = getelementptr i8, ptr addrspace(1) %52, i16 %50
  %54 = getelementptr inbounds i8, ptr addrspace(1) %53, i16 4
  %55 = load i32, ptr addrspace(1) %54, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %55)
  %56 = mul i16 1, 8
  %57 = getelementptr i8, ptr %0, i16 2
  %58 = load i16, ptr %57, !tbaa !2
  %59 = add i16 0, %56
  %60 = inttoptr i16 %58 to ptr addrspace(2)
  %61 = addrspacecast ptr addrspace(2) %60 to ptr addrspace(1)
  %62 = getelementptr i8, ptr addrspace(1) %61, i16 %59
  %63 = getelementptr inbounds i8, ptr addrspace(1) %62, i16 4
  %64 = load i32, ptr addrspace(1) %63, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %64)
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
