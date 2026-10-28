target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [202 x i8] zeroinitializer, align 2
@"A%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"A%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"A%", [6 x i8] c"\02\00e\00\00\00" }>
@"B&" = internal global [404 x i8] zeroinitializer, align 2
@B$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"B&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"B&", [6 x i8] c"\04\00e\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@"U&" = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"T=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"U=" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 0, ptr @"T%", !tbaa !2
  store i32 0, ptr @"U&", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 20, ptr @$data, !tbaa !2
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
  %10 = load i16, ptr @"I%", !tbaa !2
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = sub i16 %10, 0
  %13 = getelementptr inbounds i16, ptr @"A%", i16 %12
  store i16 %11, ptr %13, !tbaa !2
  %14 = load i16, ptr @"T%", !tbaa !2
  %15 = load i16, ptr @"I%", !tbaa !2
  %16 = sub i16 %15, 0
  %17 = getelementptr inbounds i16, ptr @"A%", i16 %16
  %18 = load i16, ptr %17, !tbaa !2
  %19 = add i16 %14, %18
  %20 = load i16, ptr @"I%", !tbaa !2
  %21 = add i16 %20, 1
  %22 = sub i16 %21, 0
  %23 = getelementptr inbounds i16, ptr @"A%", i16 %22
  %24 = load i16, ptr %23, !tbaa !2
  %25 = add i16 %19, %24
  store i16 %25, ptr @"T%", !tbaa !2
  %26 = load i16, ptr @"I%", !tbaa !2
  %27 = load i16, ptr @"I%", !tbaa !2
  %28 = sext i16 %27 to i32
  %29 = sub i16 %26, 0
  %30 = getelementptr inbounds i32, ptr @"B&", i16 %29
  store i32 %28, ptr %30, !tbaa !2
  %31 = load i32, ptr @"U&", !tbaa !2
  %32 = load i16, ptr @"I%", !tbaa !2
  %33 = sub i16 %32, 0
  %34 = getelementptr inbounds i32, ptr @"B&", i16 %33
  %35 = load i32, ptr %34, !tbaa !2
  %36 = add i32 %31, %35
  store i32 %36, ptr @"U&", !tbaa !2
  %37 = load i16, ptr @"I%", !tbaa !2
  %38 = getelementptr i8, ptr @$data, i16 2
  %39 = load i16, ptr %38, !tbaa !2
  %40 = add nsw i16 %37, %39
  store i16 %40, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %41 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %41)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %42 = load i32, ptr @"U&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %42)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
