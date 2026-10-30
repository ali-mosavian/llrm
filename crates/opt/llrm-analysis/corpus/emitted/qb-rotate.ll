target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"N%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"T=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 0, ptr @"T%", !tbaa !2
  store i16 10, ptr @"N%", !tbaa !2
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
  %10 = load i16, ptr @"T%", !tbaa !2
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = add i16 %10, %11
  store i16 %12, ptr @"T%", !tbaa !2
  %13 = load i16, ptr @"I%", !tbaa !2
  %14 = getelementptr i8, ptr @$data, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = add nsw i16 %13, %15
  store i16 %16, ptr @"I%", !tbaa !2
  br label %b2

b6:
  store i16 1, ptr @"I%", !tbaa !2
  %17 = load i16, ptr @"N%", !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 4
  store i16 %17, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %19, !tbaa !2
  br label %b7

b7:
  %20 = getelementptr i8, ptr @$data, i16 6
  %21 = load i16, ptr %20, !tbaa !2
  %22 = icmp sge i16 %21, 0
  br i1 %22, label %b8, label %b9

b8:
  %23 = load i16, ptr @"I%", !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 4
  %25 = load i16, ptr %24, !tbaa !2
  %26 = icmp sle i16 %23, %25
  br i1 %26, label %b10, label %b11

b9:
  %27 = load i16, ptr @"I%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 4
  %29 = load i16, ptr %28, !tbaa !2
  %30 = icmp sge i16 %27, %29
  br i1 %30, label %b10, label %b11

b10:
  %31 = load i16, ptr @"T%", !tbaa !2
  %32 = load i16, ptr @"I%", !tbaa !2
  %33 = add i16 %31, %32
  store i16 %33, ptr @"T%", !tbaa !2
  %34 = load i16, ptr @"I%", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 6
  %36 = load i16, ptr %35, !tbaa !2
  %37 = add nsw i16 %34, %36
  store i16 %37, ptr @"I%", !tbaa !2
  br label %b7

b11:
  store i16 1, ptr @"I%", !tbaa !2
  %38 = getelementptr i8, ptr @$data, i16 8
  store i16 0, ptr %38, !tbaa !2
  %39 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %39, !tbaa !2
  br label %b12

b12:
  %40 = getelementptr i8, ptr @$data, i16 10
  %41 = load i16, ptr %40, !tbaa !2
  %42 = icmp sge i16 %41, 0
  br i1 %42, label %b13, label %b14

b13:
  %43 = load i16, ptr @"I%", !tbaa !2
  %44 = getelementptr i8, ptr @$data, i16 8
  %45 = load i16, ptr %44, !tbaa !2
  %46 = icmp sle i16 %43, %45
  br i1 %46, label %b15, label %b16

b14:
  %47 = load i16, ptr @"I%", !tbaa !2
  %48 = getelementptr i8, ptr @$data, i16 8
  %49 = load i16, ptr %48, !tbaa !2
  %50 = icmp sge i16 %47, %49
  br i1 %50, label %b15, label %b16

b15:
  %51 = load i16, ptr @"T%", !tbaa !2
  %52 = add i16 %51, 999
  store i16 %52, ptr @"T%", !tbaa !2
  %53 = load i16, ptr @"I%", !tbaa !2
  %54 = getelementptr i8, ptr @$data, i16 10
  %55 = load i16, ptr %54, !tbaa !2
  %56 = add nsw i16 %53, %55
  store i16 %56, ptr @"I%", !tbaa !2
  br label %b12

b16:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %57 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %57)
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
