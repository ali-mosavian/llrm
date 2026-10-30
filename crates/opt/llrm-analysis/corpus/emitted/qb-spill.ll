target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"H1%" = internal global [2 x i8] zeroinitializer
@"H2%" = internal global [2 x i8] zeroinitializer
@"H3%" = internal global [2 x i8] zeroinitializer
@"O1%" = internal global [2 x i8] zeroinitializer
@"O2%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"J%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"T=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"O=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 3, ptr @"H1%", !tbaa !2
  store i16 5, ptr @"H2%", !tbaa !2
  store i16 7, ptr @"H3%", !tbaa !2
  store i16 0, ptr @"O1%", !tbaa !2
  store i16 0, ptr @"O2%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"I%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = load i16, ptr @"O1%", !tbaa !2
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = add i16 %10, %11
  store i16 %12, ptr @"O1%", !tbaa !2
  %13 = load i16, ptr @"O2%", !tbaa !2
  %14 = load i16, ptr @"O1%", !tbaa !2
  %15 = add i16 %13, %14
  store i16 %15, ptr @"O2%", !tbaa !2
  store i16 1, ptr @"J%", !tbaa !2
  %16 = getelementptr i8, ptr @$data, i16 4
  store i16 10, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %17, !tbaa !2
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %18 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %18)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %19 = load i16, ptr @"O2%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %19)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void

b7:
  %20 = getelementptr i8, ptr @$data, i16 6
  %21 = load i16, ptr %20, !tbaa !2
  %22 = icmp sge i16 %21, 0
  br i1 %22, label %b8, label %b9

b8:
  %23 = load i16, ptr @"J%", !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 4
  %25 = load i16, ptr %24, !tbaa !2
  %26 = icmp sle i16 %23, %25
  br i1 %26, label %b10, label %b11

b9:
  %27 = load i16, ptr @"J%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 4
  %29 = load i16, ptr %28, !tbaa !2
  %30 = icmp sge i16 %27, %29
  br i1 %30, label %b10, label %b11

b10:
  %31 = load i16, ptr @"T%", !tbaa !2
  %32 = load i16, ptr @"H1%", !tbaa !2
  %33 = load i16, ptr @"H2%", !tbaa !2
  %34 = mul i16 %32, %33
  %35 = add i16 %31, %34
  %36 = load i16, ptr @"H3%", !tbaa !2
  %37 = add i16 %35, %36
  store i16 %37, ptr @"T%", !tbaa !2
  %38 = load i16, ptr @"J%", !tbaa !2
  %39 = getelementptr i8, ptr @$data, i16 6
  %40 = load i16, ptr %39, !tbaa !2
  %41 = add nsw i16 %38, %40
  store i16 %41, ptr @"J%", !tbaa !2
  br label %b7

b11:
  %42 = load i16, ptr @"I%", !tbaa !2
  %43 = getelementptr i8, ptr @$data, i16 2
  %44 = load i16, ptr %43, !tbaa !2
  %45 = add nsw i16 %42, %44
  store i16 %45, ptr @"I%", !tbaa !2
  br label %b2
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
