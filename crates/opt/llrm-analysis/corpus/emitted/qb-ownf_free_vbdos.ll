target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BEFORE&" = internal global [4 x i8] zeroinitializer
@"AFTER&" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$arguments$4 = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"arrays" }>
@$string5 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\07\00", ptr getelementptr (i8, ptr @$string5, i16 4), [8 x i8] c"strings\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"bad\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"x\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"bad\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 1, ptr @$arguments$4, !tbaa !2
  call cc 1000 addrspace(1) void @ARRAYS(ptr @$arguments$4)
  %0 = getelementptr i8, ptr @$arguments$4, i16 2
  store i16 1, ptr %0, !tbaa !2
  %1 = getelementptr i8, ptr @$arguments$4, i16 2
  call cc 1000 addrspace(1) void @STRINGS(ptr %1)
  %2 = call cc 1000 addrspace(1) i32 @llrm.qb.B$FRI2(i16 -1)
  store i32 %2, ptr @"BEFORE&", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 50, ptr @$data, !tbaa !2
  %3 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %3, !tbaa !2
  br label %b2

b2:
  %4 = getelementptr i8, ptr @$data, i16 2
  %5 = load i16, ptr %4, !tbaa !2
  %6 = icmp sge i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sle i16 %7, %8
  br i1 %9, label %b5, label %b6

b4:
  %10 = load i16, ptr @"I%", !tbaa !2
  %11 = load i16, ptr @$data, !tbaa !2
  %12 = icmp sge i16 %10, %11
  br i1 %12, label %b5, label %b6

b5:
  call cc 1000 addrspace(1) void @ARRAYS(ptr @"I%")
  %13 = load i16, ptr @"I%", !tbaa !2
  %14 = getelementptr i8, ptr @$data, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = add nsw i16 %13, %15
  store i16 %16, ptr @"I%", !tbaa !2
  br label %b2

b6:
  %17 = call cc 1000 addrspace(1) i32 @llrm.qb.B$FRI2(i16 -1)
  store i32 %17, ptr @"AFTER&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCSD(ptr @$string4)
  %18 = load i32, ptr @"BEFORE&", !tbaa !2
  %19 = load i32, ptr @"AFTER&", !tbaa !2
  %20 = icmp eq i32 %18, %19
  %21 = sext i1 %20 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %21)
  %22 = call cc 1000 addrspace(1) i32 @llrm.qb.B$FRSD(i16 0)
  store i32 %22, ptr @"BEFORE&", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  %23 = getelementptr i8, ptr @$data, i16 4
  store i16 50, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %24, !tbaa !2
  br label %b7

b7:
  %25 = getelementptr i8, ptr @$data, i16 6
  %26 = load i16, ptr %25, !tbaa !2
  %27 = icmp sge i16 %26, 0
  br i1 %27, label %b8, label %b9

b8:
  %28 = load i16, ptr @"I%", !tbaa !2
  %29 = getelementptr i8, ptr @$data, i16 4
  %30 = load i16, ptr %29, !tbaa !2
  %31 = icmp sle i16 %28, %30
  br i1 %31, label %b10, label %b11

b9:
  %32 = load i16, ptr @"I%", !tbaa !2
  %33 = getelementptr i8, ptr @$data, i16 4
  %34 = load i16, ptr %33, !tbaa !2
  %35 = icmp sge i16 %32, %34
  br i1 %35, label %b10, label %b11

b10:
  call cc 1000 addrspace(1) void @STRINGS(ptr @"I%")
  %36 = load i16, ptr @"I%", !tbaa !2
  %37 = getelementptr i8, ptr @$data, i16 6
  %38 = load i16, ptr %37, !tbaa !2
  %39 = add nsw i16 %36, %38
  store i16 %39, ptr @"I%", !tbaa !2
  br label %b7

b11:
  %40 = call cc 1000 addrspace(1) i32 @llrm.qb.B$FRSD(i16 0)
  store i32 %40, ptr @"AFTER&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PCSD(ptr @$string5)
  %41 = load i32, ptr @"BEFORE&", !tbaa !2
  %42 = load i32, ptr @"AFTER&", !tbaa !2
  %43 = icmp eq i32 %41, %42
  %44 = sext i1 %43 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %44)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @ARRAYS(ptr dereferenceable(2) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [18 x i8]
  %2 = alloca [46 x i8]
  store i16 0, ptr %2, !tbaa !2
  %3 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 0, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %2, i16 4
  store i16 0, ptr %4, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %2, i16 6
  store i16 0, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %2, i16 8
  store i16 0, ptr %6, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %2, i16 10
  store i16 0, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 12
  store i16 0, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %2, i16 14
  store i16 0, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %2, i16 16
  store i16 0, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %2, i16 18
  store i16 0, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %2, i16 20
  store i16 0, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %2, i16 22
  store i16 0, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 24
  store i16 0, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %2, i16 26
  store i16 0, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %2, i16 28
  store i16 0, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %2, i16 30
  store i16 0, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %2, i16 32
  store i16 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %2, i16 34
  store i16 0, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %2, i16 36
  store i16 0, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %2, i16 38
  store i16 0, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %2, i16 40
  store i16 0, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %2, i16 42
  store i16 0, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %2, i16 44
  store i16 0, ptr %24, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 0, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 0, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %1, i16 6
  store i16 0, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %1, i16 8
  store i16 0, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %1, i16 10
  store i16 0, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %1, i16 12
  store i16 0, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %1, i16 14
  store i16 0, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %1, i16 16
  store i16 0, ptr %32, !tbaa !2
  %33 = load i16, ptr %0
  %34 = add i16 300, %33
  call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 %34, i16 2, i16 257, ptr %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 40, i16 4, i16 257, ptr %1)
  %35 = load i16, ptr %0
  %36 = mul i16 %35, 2
  %37 = getelementptr i8, ptr %2, i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = getelementptr i8, ptr %2, i16 10
  %40 = load i16, ptr %39, !tbaa !2
  %41 = add i16 %40, %36
  %42 = inttoptr i16 %38 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 %41
  %45 = load i16, ptr %0
  store i16 %45, ptr addrspace(1) %44, !tbaa !6
  %46 = getelementptr i8, ptr %1, i16 2
  %47 = load i16, ptr %46, !tbaa !2
  %48 = inttoptr i16 %47 to ptr addrspace(2)
  %49 = addrspacecast ptr addrspace(2) %48 to ptr addrspace(1)
  %50 = getelementptr i8, ptr addrspace(1) %49, i16 8
  %51 = load i16, ptr %0
  %52 = sext i16 %51 to i32
  store i32 %52, ptr addrspace(1) %50, !tbaa !8
  %53 = load i16, ptr %0
  %54 = mul i16 %53, 2
  %55 = getelementptr i8, ptr %2, i16 2
  %56 = load i16, ptr %55, !tbaa !2
  %57 = getelementptr i8, ptr %2, i16 10
  %58 = load i16, ptr %57, !tbaa !2
  %59 = add i16 %58, %54
  %60 = inttoptr i16 %56 to ptr addrspace(2)
  %61 = addrspacecast ptr addrspace(2) %60 to ptr addrspace(1)
  %62 = getelementptr i8, ptr addrspace(1) %61, i16 %59
  %63 = load i16, ptr addrspace(1) %62, !tbaa !6
  %64 = getelementptr i8, ptr %1, i16 2
  %65 = load i16, ptr %64, !tbaa !2
  %66 = inttoptr i16 %65 to ptr addrspace(2)
  %67 = addrspacecast ptr addrspace(2) %66 to ptr addrspace(1)
  %68 = getelementptr i8, ptr addrspace(1) %67, i16 8
  %69 = load i32, ptr addrspace(1) %68, !tbaa !8
  %70 = sext i16 %63 to i32
  %71 = add i32 %70, %69
  %72 = load i16, ptr %0
  %73 = mul i16 2, %72
  %74 = sext i16 %73 to i32
  %75 = icmp ne i32 %71, %74
  br i1 %75, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  br label %b4

b3:
  br label %b4

b4:
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$ERAS(ptr %1)
  ret void
}

define cc 1000 void @STRINGS(ptr dereferenceable(2) %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  store i16 0, ptr %1, !tbaa !2
  %2 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 0, ptr %2, !tbaa !2
  %3 = load i16, ptr %0
  %4 = add i16 100, %3
  %5 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STRS(i16 %4, ptr @$string7)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %5, ptr %1)
  %6 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr %1)
  %7 = load i16, ptr %0
  %8 = add i16 100, %7
  %9 = icmp ne i16 %6, %8
  br i1 %9, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
  br label %b4

b3:
  br label %b4

b4:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %1)
  ret void
}

declare cc 1000 i32 @llrm.qb.B$FRI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PCSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 i32 @llrm.qb.B$FRSD(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$ERAS(ptr) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1000 ptr @llrm.qb.B$STRS(i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FLEN(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$STDL(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
