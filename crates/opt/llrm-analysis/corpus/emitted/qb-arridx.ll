target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [42 x i8] zeroinitializer, align 2
@"A%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"A%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"A%", [6 x i8] c"\02\00\15\00\00\00" }>
@"I%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"T=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 0, ptr @"T%", !tbaa !2
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
  %12 = mul i16 %11, 3
  %13 = sub i16 %10, 0
  %14 = getelementptr inbounds i16, ptr @"A%", i16 %13
  store i16 %12, ptr %14, !tbaa !2
  %15 = load i16, ptr @"T%", !tbaa !2
  %16 = load i16, ptr @"I%", !tbaa !2
  %17 = sub i16 %16, 0
  %18 = getelementptr inbounds i16, ptr @"A%", i16 %17
  %19 = load i16, ptr %18, !tbaa !2
  %20 = add i16 %15, %19
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = sub i16 %21, 0
  %23 = getelementptr inbounds i16, ptr @"A%", i16 %22
  %24 = load i16, ptr %23, !tbaa !2
  %25 = add i16 %20, %24
  store i16 %25, ptr @"T%", !tbaa !2
  %26 = load i16, ptr @"I%", !tbaa !2
  %27 = getelementptr i8, ptr @$data, i16 2
  %28 = load i16, ptr %27, !tbaa !2
  %29 = add nsw i16 %26, %28
  store i16 %29, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %30 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %30)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
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
