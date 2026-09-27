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
  %9 = sub i16 0, 0
  %10 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %9
  store i32 1, ptr %10, !tbaa !2
  %11 = sub i16 0, 0
  %12 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %11
  %13 = getelementptr inbounds i8, ptr %12, i16 4
  %14 = addrspacecast ptr %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) @$string7$payload, i16 6
  %16 = addrspacecast ptr addrspace(1) %15 to ptr
  %17 = ptrtoint ptr %16 to i16
  %18 = load i16, ptr @$fslSegment, !tbaa !2
  %19 = inttoptr i16 %18 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %17
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %21, i16 2, ptr addrspace(1) %14, i16 2)
  %22 = sub i16 1, 0
  %23 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %22
  store i32 2, ptr %23, !tbaa !2
  %24 = sub i16 1, 0
  %25 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %24
  %26 = getelementptr inbounds i8, ptr %25, i16 4
  %27 = addrspacecast ptr %26 to ptr addrspace(1)
  %28 = getelementptr i8, ptr addrspace(1) @$string9$payload, i16 6
  %29 = addrspacecast ptr addrspace(1) %28 to ptr
  %30 = ptrtoint ptr %29 to i16
  %31 = load i16, ptr @$fslSegment, !tbaa !2
  %32 = inttoptr i16 %31 to ptr addrspace(2)
  %33 = addrspacecast ptr addrspace(2) %32 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %33, i16 %30
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %34, i16 2, ptr addrspace(1) %27, i16 2)
  %35 = load i32, ptr @O, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %35)
  %36 = getelementptr inbounds i8, ptr @O, i16 4
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  %38 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %37, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %38)
  %39 = sub i16 0, 0
  %40 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %39
  %41 = load i32, ptr %40, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %41)
  %42 = sub i16 1, 0
  %43 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %42
  %44 = load i32, ptr %43, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %44)
  %45 = sub i16 0, 0
  %46 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %45
  %47 = getelementptr inbounds i8, ptr %46, i16 4
  %48 = addrspacecast ptr %47 to ptr addrspace(1)
  %49 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %48, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %49)
  %50 = sub i16 1, 0
  %51 = getelementptr inbounds [6 x i8], ptr @ARR, i16 %50
  %52 = getelementptr inbounds i8, ptr %51, i16 4
  %53 = addrspacecast ptr %52 to ptr addrspace(1)
  %54 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %53, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %54)
  call cc1000 addrspace(1) void @INSIDE()
  store i32 999, ptr @LASTVAR, !tbaa !2
  %55 = load i32, ptr @LASTVAR, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %55)
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
  %11 = mul i16 0, 6
  %12 = getelementptr i8, ptr %0, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = add i16 0, %11
  %15 = inttoptr i16 %13 to ptr addrspace(2)
  %16 = addrspacecast ptr addrspace(2) %15 to ptr addrspace(1)
  %17 = getelementptr i8, ptr addrspace(1) %16, i16 %14
  store i32 4, ptr addrspace(1) %17, !tbaa !4
  %18 = mul i16 0, 6
  %19 = getelementptr i8, ptr %0, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = add i16 0, %18
  %22 = inttoptr i16 %20 to ptr addrspace(2)
  %23 = addrspacecast ptr addrspace(2) %22 to ptr addrspace(1)
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 %21
  %25 = trunc i32 4 to i16
  %26 = getelementptr i8, ptr addrspace(1) %24, i16 %25
  %27 = getelementptr i8, ptr addrspace(1) @$string15$payload, i16 6
  %28 = addrspacecast ptr addrspace(1) %27 to ptr
  %29 = ptrtoint ptr %28 to i16
  %30 = load i16, ptr @$fslSegment, !tbaa !2
  %31 = inttoptr i16 %30 to ptr addrspace(2)
  %32 = addrspacecast ptr addrspace(2) %31 to ptr addrspace(1)
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 %29
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %33, i16 2, ptr addrspace(1) %26, i16 2)
  %34 = mul i16 1, 6
  %35 = getelementptr i8, ptr %0, i16 2
  %36 = load i16, ptr %35, !tbaa !2
  %37 = add i16 0, %34
  %38 = inttoptr i16 %36 to ptr addrspace(2)
  %39 = addrspacecast ptr addrspace(2) %38 to ptr addrspace(1)
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 %37
  store i32 5, ptr addrspace(1) %40, !tbaa !4
  %41 = mul i16 1, 6
  %42 = getelementptr i8, ptr %0, i16 2
  %43 = load i16, ptr %42, !tbaa !2
  %44 = add i16 0, %41
  %45 = inttoptr i16 %43 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 %44
  %48 = trunc i32 4 to i16
  %49 = getelementptr i8, ptr addrspace(1) %47, i16 %48
  %50 = getelementptr i8, ptr addrspace(1) @$string17$payload, i16 6
  %51 = addrspacecast ptr addrspace(1) %50 to ptr
  %52 = ptrtoint ptr %51 to i16
  %53 = load i16, ptr @$fslSegment, !tbaa !2
  %54 = inttoptr i16 %53 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 %52
  call cc1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %56, i16 2, ptr addrspace(1) %49, i16 2)
  %57 = load i32, ptr %1, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %57)
  %58 = getelementptr inbounds i8, ptr %1, i16 4
  %59 = addrspacecast ptr %58 to ptr addrspace(1)
  %60 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %59, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %60)
  %61 = mul i16 0, 6
  %62 = getelementptr i8, ptr %0, i16 2
  %63 = load i16, ptr %62, !tbaa !2
  %64 = add i16 0, %61
  %65 = inttoptr i16 %63 to ptr addrspace(2)
  %66 = addrspacecast ptr addrspace(2) %65 to ptr addrspace(1)
  %67 = getelementptr i8, ptr addrspace(1) %66, i16 %64
  %68 = load i32, ptr addrspace(1) %67, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %68)
  %69 = mul i16 1, 6
  %70 = getelementptr i8, ptr %0, i16 2
  %71 = load i16, ptr %70, !tbaa !2
  %72 = add i16 0, %69
  %73 = inttoptr i16 %71 to ptr addrspace(2)
  %74 = addrspacecast ptr addrspace(2) %73 to ptr addrspace(1)
  %75 = getelementptr i8, ptr addrspace(1) %74, i16 %72
  %76 = load i32, ptr addrspace(1) %75, !tbaa !4
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %76)
  %77 = mul i16 0, 6
  %78 = getelementptr i8, ptr %0, i16 2
  %79 = load i16, ptr %78, !tbaa !2
  %80 = add i16 0, %77
  %81 = inttoptr i16 %79 to ptr addrspace(2)
  %82 = addrspacecast ptr addrspace(2) %81 to ptr addrspace(1)
  %83 = getelementptr i8, ptr addrspace(1) %82, i16 %80
  %84 = trunc i32 4 to i16
  %85 = getelementptr i8, ptr addrspace(1) %83, i16 %84
  %86 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %85, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %86)
  %87 = mul i16 1, 6
  %88 = getelementptr i8, ptr %0, i16 2
  %89 = load i16, ptr %88, !tbaa !2
  %90 = add i16 0, %87
  %91 = inttoptr i16 %89 to ptr addrspace(2)
  %92 = addrspacecast ptr addrspace(2) %91 to ptr addrspace(1)
  %93 = getelementptr i8, ptr addrspace(1) %92, i16 %90
  %94 = trunc i32 4 to i16
  %95 = getelementptr i8, ptr addrspace(1) %93, i16 %94
  %96 = call cc1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %95, i16 2)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr %96)
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
