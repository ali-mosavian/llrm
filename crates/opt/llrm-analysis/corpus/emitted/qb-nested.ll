target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"B%" = internal global [122 x i8] zeroinitializer, align 2
@"B%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"B%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"B%", [6 x i8] c"\02\00=\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"J%" = internal global [2 x i8] zeroinitializer
@"W%" = internal global [2 x i8] zeroinitializer
@"H%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"T=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 6, ptr @"W%", !tbaa !2
  store i16 5, ptr @"H%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  store i16 0, ptr @"I%", !tbaa !2
  %0 = load i16, ptr @"H%", !tbaa !2
  %1 = sub i16 %0, 1
  store i16 %1, ptr @$data, !tbaa !2
  %2 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %2, !tbaa !2
  br label %b2

b2:
  %3 = getelementptr i8, ptr @$data, i16 2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = icmp sge i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr @"I%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sle i16 %6, %7
  br i1 %8, label %b5, label %b6

b4:
  %9 = load i16, ptr @"I%", !tbaa !2
  %10 = load i16, ptr @$data, !tbaa !2
  %11 = icmp sge i16 %9, %10
  br i1 %11, label %b5, label %b6

b5:
  store i16 0, ptr @"J%", !tbaa !2
  %12 = load i16, ptr @"W%", !tbaa !2
  %13 = sub i16 %12, 1
  %14 = getelementptr i8, ptr @$data, i16 4
  store i16 %13, ptr %14, !tbaa !2
  %15 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %15, !tbaa !2
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %16 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %16)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void

b7:
  %17 = getelementptr i8, ptr @$data, i16 6
  %18 = load i16, ptr %17, !tbaa !2
  %19 = icmp sge i16 %18, 0
  br i1 %19, label %b8, label %b9

b8:
  %20 = load i16, ptr @"J%", !tbaa !2
  %21 = getelementptr i8, ptr @$data, i16 4
  %22 = load i16, ptr %21, !tbaa !2
  %23 = icmp sle i16 %20, %22
  br i1 %23, label %b10, label %b11

b9:
  %24 = load i16, ptr @"J%", !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 4
  %26 = load i16, ptr %25, !tbaa !2
  %27 = icmp sge i16 %24, %26
  br i1 %27, label %b10, label %b11

b10:
  %28 = load i16, ptr @"I%", !tbaa !2
  %29 = load i16, ptr @"W%", !tbaa !2
  %30 = mul i16 %28, %29
  %31 = load i16, ptr @"J%", !tbaa !2
  %32 = add i16 %30, %31
  %33 = load i16, ptr @"I%", !tbaa !2
  %34 = mul i16 %33, 10
  %35 = load i16, ptr @"J%", !tbaa !2
  %36 = add i16 %34, %35
  %37 = sub i16 %32, 0
  %38 = getelementptr inbounds i16, ptr @"B%", i16 %37
  store i16 %36, ptr %38, !tbaa !2
  %39 = load i16, ptr @"T%", !tbaa !2
  %40 = load i16, ptr @"I%", !tbaa !2
  %41 = load i16, ptr @"W%", !tbaa !2
  %42 = mul i16 %40, %41
  %43 = load i16, ptr @"J%", !tbaa !2
  %44 = add i16 %42, %43
  %45 = sub i16 %44, 0
  %46 = getelementptr inbounds i16, ptr @"B%", i16 %45
  %47 = load i16, ptr %46, !tbaa !2
  %48 = add i16 %39, %47
  store i16 %48, ptr @"T%", !tbaa !2
  %49 = load i16, ptr @"J%", !tbaa !2
  %50 = getelementptr i8, ptr @$data, i16 6
  %51 = load i16, ptr %50, !tbaa !2
  %52 = add nsw i16 %49, %51
  store i16 %52, ptr @"J%", !tbaa !2
  br label %b7

b11:
  %53 = load i16, ptr @"I%", !tbaa !2
  %54 = getelementptr i8, ptr @$data, i16 2
  %55 = load i16, ptr %54, !tbaa !2
  %56 = add nsw i16 %53, %55
  store i16 %56, ptr @"I%", !tbaa !2
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
