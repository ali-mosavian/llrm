target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A&" = internal global [4 x i8] zeroinitializer
@"B&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"ON" }>
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"=\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"CASE" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 305419896, ptr @"A&", !tbaa !2
  store i32 252645135, ptr @"B&", !tbaa !2
  store i16 1, ptr @"K%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b6

b2:
  %1 = load i32, ptr @"A&", !tbaa !2
  %2 = load i32, ptr @"B&", !tbaa !2
  %3 = and i32 %1, %2
  store i32 %3, ptr @"R&", !tbaa !2
  br label %b5

b3:
  %4 = load i32, ptr @"A&", !tbaa !2
  %5 = load i32, ptr @"B&", !tbaa !2
  %6 = or i32 %4, %5
  store i32 %6, ptr @"R&", !tbaa !2
  br label %b5

b4:
  %7 = load i32, ptr @"A&", !tbaa !2
  %8 = load i32, ptr @"B&", !tbaa !2
  %9 = xor i32 %7, %8
  store i32 %9, ptr @"R&", !tbaa !2
  br label %b5

b5:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %10 = load i16, ptr @"K%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %10)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %11 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  %12 = load i16, ptr @"K%", !tbaa !2
  %13 = icmp eq i16 %12, 1
  br i1 %13, label %b28, label %b29

b6:
  %14 = getelementptr i8, ptr @$data, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = icmp sge i16 %15, 0
  br i1 %16, label %b7, label %b8

b7:
  %17 = load i16, ptr @"K%", !tbaa !2
  %18 = load i16, ptr @$data, !tbaa !2
  %19 = icmp sle i16 %17, %18
  br i1 %19, label %b9, label %b10

b8:
  %20 = load i16, ptr @"K%", !tbaa !2
  %21 = load i16, ptr @$data, !tbaa !2
  %22 = icmp sge i16 %20, %21
  br i1 %22, label %b9, label %b10

b9:
  %23 = load i16, ptr @"K%", !tbaa !2
  %24 = icmp slt i16 %23, 0
  br i1 %24, label %b11, label %b12

b10:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b12:
  %25 = icmp sgt i16 %23, 255
  br i1 %25, label %b13, label %b14

b13:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b14:
  %26 = icmp eq i16 %23, 1
  br i1 %26, label %b16, label %b17

b15:
  br label %b2

b16:
  br label %b2

b17:
  %27 = icmp eq i16 %23, 2
  br i1 %27, label %b19, label %b20

b19:
  br label %b3

b20:
  %28 = icmp eq i16 %23, 3
  br i1 %28, label %b22, label %b23

b22:
  br label %b4

b23:
  br label %b15

b27:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %29 = load i16, ptr @"K%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %29)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %30 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %30)
  %31 = load i16, ptr @"K%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = add nsw i16 %31, %33
  store i16 %34, ptr @"K%", !tbaa !2
  br label %b6

b28:
  %35 = load i32, ptr @"A&", !tbaa !2
  %36 = load i32, ptr @"B&", !tbaa !2
  %37 = add i32 %35, %36
  store i32 %37, ptr @"R&", !tbaa !2
  br label %b27

b29:
  %38 = icmp eq i16 %12, 2
  br i1 %38, label %b30, label %b31

b30:
  %39 = load i32, ptr @"A&", !tbaa !2
  %40 = load i32, ptr @"B&", !tbaa !2
  %41 = sub i32 %39, %40
  store i32 %41, ptr @"R&", !tbaa !2
  br label %b27

b31:
  %42 = load i32, ptr @"A&", !tbaa !2
  %43 = sub i32 0, %42
  store i32 %43, ptr @"R&", !tbaa !2
  br label %b27
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
