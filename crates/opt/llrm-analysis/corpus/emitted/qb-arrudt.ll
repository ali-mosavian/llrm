target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@PTS = internal global [16 x i8] zeroinitializer, align 2
@PTS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @PTS to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @PTS, [6 x i8] c"\08\00\02\00\00\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  store i32 11, ptr %0, !tbaa !2
  %1 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %2 = getelementptr inbounds i8, ptr %1, i16 4
  store i32 22, ptr %2, !tbaa !2
  %3 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  store i32 33, ptr %3, !tbaa !2
  %4 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %5 = getelementptr inbounds i8, ptr %4, i16 4
  store i32 44, ptr %5, !tbaa !2
  %6 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %7 = load i32, ptr %6, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %7)
  %8 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %9 = load i32, ptr %8, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %9)
  %10 = getelementptr inbounds [8 x i8], ptr @PTS, i16 0
  %11 = getelementptr inbounds i8, ptr %10, i16 4
  %12 = load i32, ptr %11, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %12)
  %13 = getelementptr inbounds [8 x i8], ptr @PTS, i16 1
  %14 = getelementptr inbounds i8, ptr %13, i16 4
  %15 = load i32, ptr %14, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %15)
  call cc 1000 addrspace(1) void @INSIDE()
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

define cc 1000 void @INSIDE() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [18 x i8]
  store i16 0, ptr %0, !tbaa !2
  %1 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 0, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %0, i16 4
  store i16 0, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i8, ptr %0, i16 6
  store i16 0, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %0, i16 8
  store i16 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 10
  store i16 0, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 12
  store i16 0, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %0, i16 14
  store i16 0, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %0, i16 16
  store i16 0, ptr %8, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 8, i16 257, ptr %0)
  %9 = getelementptr i8, ptr %0, i16 2
  %10 = load i16, ptr %9, !tbaa !2
  %11 = inttoptr i16 %10 to ptr addrspace(2)
  %12 = addrspacecast ptr addrspace(2) %11 to ptr addrspace(1)
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 0
  store i32 55, ptr addrspace(1) %13, !tbaa !6
  %14 = getelementptr i8, ptr %0, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2)
  %17 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 0
  %19 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 4
  store i32 66, ptr addrspace(1) %19, !tbaa !6
  %20 = getelementptr i8, ptr %0, i16 2
  %21 = load i16, ptr %20, !tbaa !2
  %22 = inttoptr i16 %21 to ptr addrspace(2)
  %23 = addrspacecast ptr addrspace(2) %22 to ptr addrspace(1)
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 8
  store i32 77, ptr addrspace(1) %24, !tbaa !6
  %25 = getelementptr i8, ptr %0, i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = inttoptr i16 %26 to ptr addrspace(2)
  %28 = addrspacecast ptr addrspace(2) %27 to ptr addrspace(1)
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 8
  %30 = getelementptr inbounds i8, ptr addrspace(1) %29, i16 4
  store i32 88, ptr addrspace(1) %30, !tbaa !6
  %31 = getelementptr i8, ptr %0, i16 2
  %32 = load i16, ptr %31, !tbaa !2
  %33 = inttoptr i16 %32 to ptr addrspace(2)
  %34 = addrspacecast ptr addrspace(2) %33 to ptr addrspace(1)
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 0
  %36 = load i32, ptr addrspace(1) %35, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %36)
  %37 = getelementptr i8, ptr %0, i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = inttoptr i16 %38 to ptr addrspace(2)
  %40 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 8
  %42 = load i32, ptr addrspace(1) %41, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %42)
  %43 = getelementptr i8, ptr %0, i16 2
  %44 = load i16, ptr %43, !tbaa !2
  %45 = inttoptr i16 %44 to ptr addrspace(2)
  %46 = addrspacecast ptr addrspace(2) %45 to ptr addrspace(1)
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 0
  %48 = getelementptr inbounds i8, ptr addrspace(1) %47, i16 4
  %49 = load i32, ptr addrspace(1) %48, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %49)
  %50 = getelementptr i8, ptr %0, i16 2
  %51 = load i16, ptr %50, !tbaa !2
  %52 = inttoptr i16 %51 to ptr addrspace(2)
  %53 = addrspacecast ptr addrspace(2) %52 to ptr addrspace(1)
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 8
  %55 = getelementptr inbounds i8, ptr addrspace(1) %54, i16 4
  %56 = load i32, ptr addrspace(1) %55, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %56)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %0)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
