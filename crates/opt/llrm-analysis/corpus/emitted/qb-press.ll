target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"D%" = internal global [2 x i8] zeroinitializer
@"E%" = internal global [2 x i8] zeroinitializer
@"F%" = internal global [2 x i8] zeroinitializer
@"G%" = internal global [2 x i8] zeroinitializer
@"H%" = internal global [2 x i8] zeroinitializer
@"R%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"R=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 3, ptr @"A%", !tbaa !2
  store i16 5, ptr @"B%", !tbaa !2
  store i16 7, ptr @"C%", !tbaa !2
  store i16 11, ptr @"D%", !tbaa !2
  store i16 13, ptr @"E%", !tbaa !2
  store i16 17, ptr @"F%", !tbaa !2
  store i16 19, ptr @"G%", !tbaa !2
  store i16 23, ptr @"H%", !tbaa !2
  store i16 0, ptr @"R%", !tbaa !2
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
  %10 = load i16, ptr @"R%", !tbaa !2
  %11 = load i16, ptr @"A%", !tbaa !2
  %12 = load i16, ptr @"B%", !tbaa !2
  %13 = mul i16 %11, %12
  %14 = add i16 %10, %13
  %15 = load i16, ptr @"C%", !tbaa !2
  %16 = load i16, ptr @"D%", !tbaa !2
  %17 = mul i16 %15, %16
  %18 = add i16 %14, %17
  %19 = load i16, ptr @"E%", !tbaa !2
  %20 = load i16, ptr @"F%", !tbaa !2
  %21 = mul i16 %19, %20
  %22 = add i16 %18, %21
  %23 = load i16, ptr @"G%", !tbaa !2
  %24 = load i16, ptr @"H%", !tbaa !2
  %25 = mul i16 %23, %24
  %26 = add i16 %22, %25
  store i16 %26, ptr @"R%", !tbaa !2
  %27 = load i16, ptr @"I%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 2
  %29 = load i16, ptr %28, !tbaa !2
  %30 = add nsw i16 %27, %29
  store i16 %30, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %31 = load i16, ptr @"R%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %31)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
