target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@$arguments$1 = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string4, i16 4), [10 x i8] c"123456789\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 0, ptr @$arguments$1, !tbaa !2
  %0 = call cc 1000 addrspace(1) i32 @"BENCHCRC&"(ptr @$arguments$1)
  %1 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %0)
  %2 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %2)
  ret void
}

define cc 1000 i32 @"BENCHCRC&"(ptr dereferenceable(4) %0) addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [4 x i8]
  %8 = alloca i32
  %9 = alloca i32
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 4, i1 false)
  store i32 0, ptr %8
  store i32 0, ptr %9
  store i16 0, ptr %7, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 0, ptr %10, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string4, ptr %7)
  %11 = load i32, ptr %0
  %12 = xor i32 -1, %11
  store i32 %12, ptr %8, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i16 8, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %4, !tbaa !2
  %14 = icmp sge i16 %13, 0
  br i1 %14, label %b3, label %b4

b3:
  %15 = load i16, ptr %6, !tbaa !2
  %16 = load i16, ptr %5, !tbaa !2
  %17 = icmp sle i16 %15, %16
  br i1 %17, label %b5, label %b6

b4:
  %18 = load i16, ptr %6, !tbaa !2
  %19 = load i16, ptr %5, !tbaa !2
  %20 = icmp sge i16 %18, %19
  br i1 %20, label %b5, label %b6

b5:
  %21 = load i32, ptr %8, !tbaa !2
  %22 = load i16, ptr %6, !tbaa !2
  %23 = add i16 %22, 1
  %24 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %7, i16 %23, i16 1)
  %25 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %24)
  %26 = sext i16 %25 to i32
  %27 = xor i32 %21, %26
  store i32 %27, ptr %8, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i16 7, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b7

b6:
  %28 = load i32, ptr %8, !tbaa !2
  %29 = xor i32 %28, -1
  store i32 %29, ptr %9, !tbaa !2
  br label %b12

b7:
  %30 = load i16, ptr %1, !tbaa !2
  %31 = icmp sge i16 %30, 0
  br i1 %31, label %b8, label %b9

b8:
  %32 = load i16, ptr %3, !tbaa !2
  %33 = load i16, ptr %2, !tbaa !2
  %34 = icmp sle i16 %32, %33
  br i1 %34, label %b10, label %b11

b9:
  %35 = load i16, ptr %3, !tbaa !2
  %36 = load i16, ptr %2, !tbaa !2
  %37 = icmp sge i16 %35, %36
  br i1 %37, label %b10, label %b11

b10:
  %38 = load i32, ptr %8, !tbaa !2
  %39 = and i32 %38, -2
  %40 = sdiv i32 %39, 2
  %41 = and i32 %40, 2147483647
  %42 = load i32, ptr %8, !tbaa !2
  %43 = and i32 %42, 1
  %44 = sub i32 0, %43
  %45 = and i32 -306674912, %44
  %46 = xor i32 %41, %45
  store i32 %46, ptr %8, !tbaa !2
  %47 = load i16, ptr %3, !tbaa !2
  %48 = load i16, ptr %1, !tbaa !2
  %49 = add nsw i16 %47, %48
  store i16 %49, ptr %3, !tbaa !2
  br label %b7

b11:
  %50 = load i16, ptr %6, !tbaa !2
  %51 = load i16, ptr %4, !tbaa !2
  %52 = add nsw i16 %50, %51
  store i16 %52, ptr %6, !tbaa !2
  br label %b2

b12:
  call cc 1000 addrspace(1) void @llrm.qb.B$STDL(ptr %7)
  %53 = load i32, ptr %9, !tbaa !2
  ret i32 %53
}

declare cc 1000 ptr @llrm.qb.B$STI4(i32) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FMID(ptr, i16, i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FASC(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$STDL(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
