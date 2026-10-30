target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@O = internal global [6 x i8] zeroinitializer
@ARR = internal global [12 x i8] zeroinitializer, align 2
@ARR$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @ARR to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @ARR, [6 x i8] c"\06\00\02\00\00\00" }>
@LASTVAR = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"AB" }>
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"CD" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"EF" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"GH" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c"IJ" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"KL" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 111, ptr @O, !tbaa !2
  %0 = getelementptr inbounds i8, ptr @O, i16 4
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = getelementptr i8, ptr @$string4, i16 4
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %3, i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %1, i16 2)
  %4 = getelementptr inbounds [6 x i8], ptr @ARR, i16 0
  store i32 1, ptr %4, !tbaa !2
  %5 = getelementptr inbounds [6 x i8], ptr @ARR, i16 0
  %6 = getelementptr inbounds i8, ptr %5, i16 4
  %7 = addrspacecast ptr %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr @$string5, i16 4
  %9 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %9, i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %7, i16 2)
  %10 = getelementptr inbounds [6 x i8], ptr @ARR, i16 1
  store i32 2, ptr %10, !tbaa !2
  %11 = getelementptr inbounds [6 x i8], ptr @ARR, i16 1
  %12 = getelementptr inbounds i8, ptr %11, i16 4
  %13 = addrspacecast ptr %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr @$string6, i16 4
  %15 = addrspacecast ptr %14 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %15, i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %13, i16 2)
  %16 = load i32, ptr @O, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %16)
  %17 = getelementptr inbounds i8, ptr @O, i16 4
  %18 = addrspacecast ptr %17 to ptr addrspace(1)
  %19 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %18, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %19)
  %20 = getelementptr inbounds [6 x i8], ptr @ARR, i16 0
  %21 = load i32, ptr %20, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %21)
  %22 = getelementptr inbounds [6 x i8], ptr @ARR, i16 1
  %23 = load i32, ptr %22, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %23)
  %24 = getelementptr inbounds [6 x i8], ptr @ARR, i16 0
  %25 = getelementptr inbounds i8, ptr %24, i16 4
  %26 = addrspacecast ptr %25 to ptr addrspace(1)
  %27 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %26, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %27)
  %28 = getelementptr inbounds [6 x i8], ptr @ARR, i16 1
  %29 = getelementptr inbounds i8, ptr %28, i16 4
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  %31 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %30, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %31)
  call cc 1000 addrspace(1) void @INSIDE()
  store i32 999, ptr @LASTVAR, !tbaa !2
  %32 = load i32, ptr @LASTVAR, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %32)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void
}

define cc 1000 void @INSIDE() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [18 x i8]
  %1 = alloca [6 x i8]
  store i16 0, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 0, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 0, ptr %3, !tbaa !2
  store i16 0, ptr %0, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 4
  store i16 0, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 6
  store i16 0, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %0, i16 8
  store i16 0, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %0, i16 10
  store i16 0, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %0, i16 12
  store i16 0, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %0, i16 14
  store i16 0, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %0, i16 16
  store i16 0, ptr %11, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 6, i16 257, ptr %0)
  store i32 333, ptr %1, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %1, i16 4
  %13 = addrspacecast ptr %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr @$string8, i16 4
  %15 = addrspacecast ptr %14 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %15, i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %13, i16 2)
  %16 = getelementptr i8, ptr %0, i16 2
  %17 = load i16, ptr %16, !tbaa !2
  %18 = inttoptr i16 %17 to ptr addrspace(2)
  %19 = addrspacecast ptr addrspace(2) %18 to ptr addrspace(1)
  %20 = getelementptr i8, ptr addrspace(1) %19, i16 0
  store i32 4, ptr addrspace(1) %20, !tbaa !6
  %21 = getelementptr i8, ptr %0, i16 2
  %22 = load i16, ptr %21, !tbaa !2
  %23 = inttoptr i16 %22 to ptr addrspace(2)
  %24 = addrspacecast ptr addrspace(2) %23 to ptr addrspace(1)
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 0
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 4
  %27 = getelementptr i8, ptr @$string9, i16 4
  %28 = addrspacecast ptr %27 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %28, i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %26, i16 2)
  %29 = getelementptr i8, ptr %0, i16 2
  %30 = load i16, ptr %29, !tbaa !2
  %31 = inttoptr i16 %30 to ptr addrspace(2)
  %32 = addrspacecast ptr addrspace(2) %31 to ptr addrspace(1)
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 6
  store i32 5, ptr addrspace(1) %33, !tbaa !6
  %34 = getelementptr i8, ptr %0, i16 2
  %35 = load i16, ptr %34, !tbaa !2
  %36 = inttoptr i16 %35 to ptr addrspace(2)
  %37 = addrspacecast ptr addrspace(2) %36 to ptr addrspace(1)
  %38 = getelementptr i8, ptr addrspace(1) %37, i16 6
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 4
  %40 = getelementptr i8, ptr @$string10, i16 4
  %41 = addrspacecast ptr %40 to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) %41, i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %39, i16 2)
  %42 = load i32, ptr %1, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %42)
  %43 = getelementptr inbounds i8, ptr %1, i16 4
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  %45 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %44, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %45)
  %46 = getelementptr i8, ptr %0, i16 2
  %47 = load i16, ptr %46, !tbaa !2
  %48 = inttoptr i16 %47 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 0
  %51 = load i32, ptr addrspace(1) %50, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %51)
  %52 = getelementptr i8, ptr %0, i16 2
  %53 = load i16, ptr %52, !tbaa !2
  %54 = inttoptr i16 %53 to ptr addrspace(2)
  %55 = addrspacecast ptr addrspace(2) %54 to ptr addrspace(1)
  %56 = getelementptr i8, ptr addrspace(1) %55, i16 6
  %57 = load i32, ptr addrspace(1) %56, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %57)
  %58 = getelementptr i8, ptr %0, i16 2
  %59 = load i16, ptr %58, !tbaa !2
  %60 = inttoptr i16 %59 to ptr addrspace(2)
  %61 = addrspacecast ptr addrspace(2) %60 to ptr addrspace(1)
  %62 = getelementptr i8, ptr addrspace(1) %61, i16 0
  %63 = getelementptr i8, ptr addrspace(1) %62, i16 4
  %64 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %63, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %64)
  %65 = getelementptr i8, ptr %0, i16 2
  %66 = load i16, ptr %65, !tbaa !2
  %67 = inttoptr i16 %66 to ptr addrspace(2)
  %68 = addrspacecast ptr addrspace(2) %67 to ptr addrspace(1)
  %69 = getelementptr i8, ptr addrspace(1) %68, i16 6
  %70 = getelementptr i8, ptr addrspace(1) %69, i16 4
  %71 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %70, i16 2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %71)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %0)
  ret void
}

declare cc 1000 void @llrm.qb.B$ASSN(ptr addrspace(1), i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LDFS(ptr addrspace(1), i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
