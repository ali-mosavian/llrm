target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [18 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"J%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"T=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 100, i16 2, i16 257, ptr @"A%")
  store i16 0, ptr @"T%", !tbaa !2
  store i16 1, ptr @"J%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"J%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"J%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  store i16 1, ptr @"I%", !tbaa !2
  %10 = getelementptr i8, ptr @$data, i16 4
  store i16 20, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %11, !tbaa !2
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %12 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void

b7:
  %13 = getelementptr i8, ptr @$data, i16 6
  %14 = load i16, ptr %13, !tbaa !2
  %15 = icmp sge i16 %14, 0
  br i1 %15, label %b8, label %b9

b8:
  %16 = load i16, ptr @"I%", !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 4
  %18 = load i16, ptr %17, !tbaa !2
  %19 = icmp sle i16 %16, %18
  br i1 %19, label %b10, label %b11

b9:
  %20 = load i16, ptr @"I%", !tbaa !2
  %21 = getelementptr i8, ptr @$data, i16 4
  %22 = load i16, ptr %21, !tbaa !2
  %23 = icmp sge i16 %20, %22
  br i1 %23, label %b10, label %b11

b10:
  %24 = load i16, ptr @"I%", !tbaa !2
  %25 = mul i16 %24, 2
  %26 = getelementptr i8, ptr @"A%", i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = add i16 0, %25
  %29 = inttoptr i16 %27 to ptr addrspace(2)
  %30 = addrspacecast ptr addrspace(2) %29 to ptr addrspace(1)
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %28
  %32 = load i16, ptr @"I%", !tbaa !2
  store i16 %32, ptr addrspace(1) %31, !tbaa !6
  %33 = load i16, ptr @"T%", !tbaa !2
  %34 = load i16, ptr @"I%", !tbaa !2
  %35 = mul i16 %34, 2
  %36 = getelementptr i8, ptr @"A%", i16 2
  %37 = load i16, ptr %36, !tbaa !2
  %38 = add i16 0, %35
  %39 = inttoptr i16 %37 to ptr addrspace(2)
  %40 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 %38
  %42 = load i16, ptr addrspace(1) %41, !tbaa !6
  %43 = add i16 %33, %42
  store i16 %43, ptr @"T%", !tbaa !2
  %44 = load i16, ptr @"I%", !tbaa !2
  %45 = getelementptr i8, ptr @$data, i16 6
  %46 = load i16, ptr %45, !tbaa !2
  %47 = add nsw i16 %44, %46
  store i16 %47, ptr @"I%", !tbaa !2
  br label %b7

b11:
  %48 = load i16, ptr @"J%", !tbaa !2
  %49 = getelementptr i8, ptr @$data, i16 2
  %50 = load i16, ptr %49, !tbaa !2
  %51 = add nsw i16 %48, %50
  store i16 %51, ptr @"J%", !tbaa !2
  br label %b2
}

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
