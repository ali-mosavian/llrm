target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"N\00" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"THEN" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"ELSE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 0, ptr @"A%", !tbaa !2
  store i16 1, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b5

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %1 = load i16, ptr @"A%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %1)
  br label %b4

b3:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %2 = load i16, ptr @"A%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
  br label %b4

b4:
  %3 = load i16, ptr @"A%", !tbaa !2
  %4 = getelementptr i8, ptr @$data, i16 10
  %5 = load i16, ptr %4, !tbaa !2
  %6 = add nsw i16 %3, %5
  store i16 %6, ptr @"A%", !tbaa !2
  br label %b21

b5:
  %7 = getelementptr i8, ptr @$data, i16 2
  %8 = load i16, ptr %7, !tbaa !2
  %9 = icmp sge i16 %8, 0
  br i1 %9, label %b6, label %b7

b6:
  %10 = load i16, ptr @"A%", !tbaa !2
  %11 = load i16, ptr @$data, !tbaa !2
  %12 = icmp sle i16 %10, %11
  br i1 %12, label %b8, label %b9

b7:
  %13 = load i16, ptr @"A%", !tbaa !2
  %14 = load i16, ptr @$data, !tbaa !2
  %15 = icmp sge i16 %13, %14
  br i1 %15, label %b8, label %b9

b8:
  store i16 0, ptr @"B%", !tbaa !2
  %16 = getelementptr i8, ptr @$data, i16 4
  store i16 1, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %17, !tbaa !2
  br label %b10

b9:
  store i16 0, ptr @"A%", !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 8
  store i16 1, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %19, !tbaa !2
  br label %b21

b10:
  %20 = getelementptr i8, ptr @$data, i16 6
  %21 = load i16, ptr %20, !tbaa !2
  %22 = icmp sge i16 %21, 0
  br i1 %22, label %b11, label %b12

b11:
  %23 = load i16, ptr @"B%", !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 4
  %25 = load i16, ptr %24, !tbaa !2
  %26 = icmp sle i16 %23, %25
  br i1 %26, label %b13, label %b14

b12:
  %27 = load i16, ptr @"B%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 4
  %29 = load i16, ptr %28, !tbaa !2
  %30 = icmp sge i16 %27, %29
  br i1 %30, label %b13, label %b14

b13:
  %31 = load i16, ptr @"A%", !tbaa !2
  %32 = icmp ne i16 %31, 0
  br i1 %32, label %b15, label %b16

b14:
  %33 = load i16, ptr @"A%", !tbaa !2
  %34 = getelementptr i8, ptr @$data, i16 2
  %35 = load i16, ptr %34, !tbaa !2
  %36 = add nsw i16 %33, %35
  store i16 %36, ptr @"A%", !tbaa !2
  br label %b5

b15:
  %37 = load i16, ptr @"B%", !tbaa !2
  %38 = icmp ne i16 %37, 0
  br i1 %38, label %b18, label %b19

b16:
  store i16 3, ptr @"X%", !tbaa !2
  br label %b17

b17:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %39 = load i16, ptr @"A%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %39)
  %40 = load i16, ptr @"B%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %40)
  %41 = load i16, ptr @"X%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %41)
  %42 = load i16, ptr @"B%", !tbaa !2
  %43 = getelementptr i8, ptr @$data, i16 6
  %44 = load i16, ptr %43, !tbaa !2
  %45 = add nsw i16 %42, %44
  store i16 %45, ptr @"B%", !tbaa !2
  br label %b10

b18:
  store i16 1, ptr @"X%", !tbaa !2
  br label %b20

b19:
  store i16 2, ptr @"X%", !tbaa !2
  br label %b20

b20:
  br label %b17

b21:
  %46 = getelementptr i8, ptr @$data, i16 10
  %47 = load i16, ptr %46, !tbaa !2
  %48 = icmp sge i16 %47, 0
  br i1 %48, label %b22, label %b23

b22:
  %49 = load i16, ptr @"A%", !tbaa !2
  %50 = getelementptr i8, ptr @$data, i16 8
  %51 = load i16, ptr %50, !tbaa !2
  %52 = icmp sle i16 %49, %51
  br i1 %52, label %b24, label %b25

b23:
  %53 = load i16, ptr @"A%", !tbaa !2
  %54 = getelementptr i8, ptr @$data, i16 8
  %55 = load i16, ptr %54, !tbaa !2
  %56 = icmp sge i16 %53, %55
  br i1 %56, label %b24, label %b25

b24:
  %57 = load i16, ptr @"A%", !tbaa !2
  %58 = icmp ne i16 %57, 0
  br i1 %58, label %b26, label %b27

b25:
  ret void

b26:
  br label %b2

b27:
  br label %b3
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
