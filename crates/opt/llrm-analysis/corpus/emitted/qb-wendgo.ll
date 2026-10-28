target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"KON%" = internal global [202 x i8] zeroinitializer, align 2
@"KON%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"KON%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"KON%", [6 x i8] c"\02\00e\00\00\00" }>
@"PA%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"PA=\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 355, ptr @"PA%", !tbaa !2
  %0 = getelementptr inbounds i16, ptr @"KON%", i16 75
  store i16 1, ptr %0, !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = getelementptr i8, ptr @$data, i16 2
  %3 = load i16, ptr %2, !tbaa !2
  %4 = icmp sge i16 %3, 0
  br i1 %4, label %b3, label %b4

b3:
  %5 = load i16, ptr @"I%", !tbaa !2
  %6 = load i16, ptr @$data, !tbaa !2
  %7 = icmp sle i16 %5, %6
  br i1 %7, label %b5, label %b6

b4:
  %8 = load i16, ptr @"I%", !tbaa !2
  %9 = load i16, ptr @$data, !tbaa !2
  %10 = icmp sge i16 %8, %9
  br i1 %10, label %b5, label %b6

b5:
  call cc 1000 addrspace(1) void @TURN()
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = getelementptr i8, ptr @$data, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = add nsw i16 %11, %13
  store i16 %14, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %15 = load i16, ptr @"PA%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %15)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @TURN() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  br label %b4

b2:
  br label %b11

b3:
  ret void

b4:
  %0 = getelementptr inbounds i16, ptr @"KON%", i16 75
  %1 = load i16, ptr %0, !tbaa !2
  %2 = icmp eq i16 %1, 1
  br i1 %2, label %b5, label %b6

b5:
  %3 = load i16, ptr @"PA%", !tbaa !2
  %4 = add i16 %3, 1
  store i16 %4, ptr @"PA%", !tbaa !2
  %5 = load i16, ptr @"PA%", !tbaa !2
  %6 = icmp sgt i16 %5, 360
  br i1 %6, label %b7, label %b8

b6:
  br label %b2

b7:
  %7 = load i16, ptr @"PA%", !tbaa !2
  %8 = sub i16 %7, 360
  %9 = add i16 0, %8
  store i16 %9, ptr @"PA%", !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  br label %b2

b11:
  %10 = getelementptr inbounds i16, ptr @"KON%", i16 77
  %11 = load i16, ptr %10, !tbaa !2
  %12 = icmp eq i16 %11, 1
  br i1 %12, label %b12, label %b13

b12:
  %13 = load i16, ptr @"PA%", !tbaa !2
  %14 = sub i16 %13, 1
  store i16 %14, ptr @"PA%", !tbaa !2
  %15 = load i16, ptr @"PA%", !tbaa !2
  %16 = icmp slt i16 %15, 0
  br i1 %16, label %b14, label %b15

b13:
  br label %b3

b14:
  %17 = load i16, ptr @"PA%", !tbaa !2
  %18 = add i16 360, %17
  store i16 %18, ptr @"PA%", !tbaa !2
  br label %b16

b15:
  br label %b16

b16:
  br label %b3
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
