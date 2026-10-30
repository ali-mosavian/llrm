target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"D%" = internal global [2 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"T=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 3, ptr @"A%", !tbaa !2
  store i16 7, ptr @"B%", !tbaa !2
  store i16 2, ptr @"C%", !tbaa !2
  store i16 9, ptr @"D%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  %0 = load i16, ptr @"A%", !tbaa !2
  %1 = load i16, ptr @"B%", !tbaa !2
  %2 = icmp slt i16 %0, %1
  %3 = sext i1 %2 to i16
  store i16 %3, ptr @"X%", !tbaa !2
  %4 = load i16, ptr @"T%", !tbaa !2
  %5 = load i16, ptr @"X%", !tbaa !2
  %6 = add i16 %4, %5
  store i16 %6, ptr @"T%", !tbaa !2
  %7 = load i16, ptr @"A%", !tbaa !2
  %8 = load i16, ptr @"B%", !tbaa !2
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %b2, label %b3

b2:
  %10 = load i16, ptr @"T%", !tbaa !2
  %11 = add i16 %10, 1
  store i16 %11, ptr @"T%", !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  %12 = load i16, ptr @"A%", !tbaa !2
  %13 = load i16, ptr @"B%", !tbaa !2
  %14 = icmp slt i16 %12, %13
  %15 = sext i1 %14 to i16
  %16 = load i16, ptr @"C%", !tbaa !2
  %17 = load i16, ptr @"D%", !tbaa !2
  %18 = icmp slt i16 %16, %17
  %19 = sext i1 %18 to i16
  %20 = and i16 %15, %19
  %21 = icmp ne i16 %20, 0
  br i1 %21, label %b5, label %b6

b5:
  %22 = load i16, ptr @"T%", !tbaa !2
  %23 = add i16 %22, 2
  store i16 %23, ptr @"T%", !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %24 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %24)
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
