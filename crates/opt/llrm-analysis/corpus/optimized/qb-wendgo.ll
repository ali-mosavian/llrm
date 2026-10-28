target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"PA%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"PA=\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>
@"KON%.0" = internal global i16 0, align 2
@"KON%.1" = internal global i16 0, align 2

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  store i16 1, ptr @"KON%.0", !tbaa !2
  %0 = load i16, ptr @"KON%.1", !tbaa !2
  %1 = icmp eq i16 %0, 1
  %2 = select i1 %1, i16 355, i16 360
  %3 = icmp samesign ugt i16 %2, 359
  %4 = select i1 %3, i16 -359, i16 1
  %5 = add nsw i16 %2, %4
  %6 = icmp slt i16 %5, 1
  %7 = select i1 %6, i16 359, i16 -1
  %8 = select i1 %1, i16 %7, i16 0
  %9 = add nsw i16 %5, %8
  %10 = icmp sgt i16 %9, 359
  %11 = select i1 %10, i16 -359, i16 1
  %12 = add nsw i16 %9, %11
  %13 = icmp slt i16 %12, 1
  %14 = select i1 %13, i16 359, i16 -1
  %15 = select i1 %1, i16 %14, i16 0
  %16 = add nsw i16 %12, %15
  %17 = icmp sgt i16 %16, 359
  %18 = select i1 %17, i16 -359, i16 1
  %19 = add nsw i16 %16, %18
  %20 = icmp slt i16 %19, 1
  %21 = select i1 %20, i16 359, i16 -1
  %22 = select i1 %1, i16 %21, i16 0
  %23 = add nsw i16 %19, %22
  %24 = icmp sgt i16 %23, 359
  %25 = select i1 %24, i16 -359, i16 1
  %26 = add nsw i16 %23, %25
  %27 = icmp slt i16 %26, 1
  %28 = select i1 %27, i16 359, i16 -1
  %29 = select i1 %1, i16 %28, i16 0
  %30 = add nsw i16 %26, %29
  %31 = icmp sgt i16 %30, 359
  %32 = select i1 %31, i16 -359, i16 1
  %33 = add nsw i16 %30, %32
  %34 = icmp slt i16 %33, 1
  %35 = select i1 %34, i16 359, i16 -1
  %36 = select i1 %1, i16 %35, i16 0
  %37 = add nsw i16 %33, %36
  store i16 %37, ptr @"PA%", align 1, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %38 = load i16, ptr @"PA%", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %38)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @TURN() addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: none, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {
b1:
  %0 = load i16, ptr @"KON%.0", !tbaa !2
  %1 = icmp eq i16 %0, 1
  br i1 %1, label %b5, label %b11

b3:
  ret void

b5:
  %2 = load i16, ptr @"PA%", !tbaa !2
  %3 = add i16 %2, 1
  store i16 %3, ptr @"PA%", !tbaa !2
  %4 = icmp sgt i16 %3, 360
  br i1 %4, label %b7, label %b11

b7:
  %5 = add nsw i16 %2, -359
  store i16 %5, ptr @"PA%", !tbaa !2
  br label %b11

b11:
  %6 = load i16, ptr @"KON%.1", !tbaa !2
  %7 = icmp eq i16 %6, 1
  br i1 %7, label %b12, label %b3

b12:
  %8 = load i16, ptr @"PA%", !tbaa !2
  %9 = add i16 %8, -1
  store i16 %9, ptr @"PA%", !tbaa !2
  %10 = icmp slt i16 %9, 0
  br i1 %10, label %b14, label %b3

b14:
  %11 = add nsw i16 %8, 359
  store i16 %11, ptr @"PA%", !tbaa !2
  br label %b3
}

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
