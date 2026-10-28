target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"DT100%" = internal global [722 x i8] zeroinitializer, align 2
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"SUM=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1000 addrspace(1) void @RAMP()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @RAMP() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  br label %b5

b5:
  %0 = phi i16 [ 64, %b1 ], [ %18, %b5 ]
  %1 = phi i16 [ 0, %b1 ], [ %17, %b5 ]
  %2 = phi i16 [ -1, %b1 ], [ %spec.store.select1, %b5 ]
  %3 = lshr i16 %0, 6
  %4 = shl nuw nsw i16 %3, 6
  %5 = add nsw i16 %4, -1
  %6 = sub i16 %0, %4
  %7 = add i16 %1, %5
  %8 = shl nsw i16 %1, 1
  %9 = getelementptr i8, ptr @"DT100%", i16 %8
  %10 = getelementptr i8, ptr %9, i16 358
  %11 = icmp eq i16 %2, 1
  %spec.store.select4 = select i1 %11, i16 %7, i16 %6
  store i16 %spec.store.select4, ptr %10
  %12 = icmp eq i16 %spec.store.select4, 0
  %13 = and i1 %11, %12
  %spec.store.select = select i1 %13, i16 -1, i16 %2
  %14 = icmp eq i16 %spec.store.select4, 63
  %15 = icmp eq i16 %spec.store.select, -1
  %16 = select i1 %14, i1 %15, i1 false
  %spec.store.select1 = select i1 %16, i16 1, i16 %spec.store.select
  %17 = add nsw i16 %1, -1
  %18 = add nuw nsw i16 %0, 1
  %19 = icmp samesign ugt i16 %17, -156
  br i1 %19, label %b5, label %20

20:
  br label %b19

b19:
  %21 = phi i16 [ 63, %20 ], [ %46, %b23 ]
  %22 = phi i16 [ -314, %20 ], [ %45, %b23 ]
  %23 = phi i16 [ %spec.store.select3, %b23 ], [ 1, %20 ]
  %24 = phi i16 [ %44, %b23 ], [ 0, %20 ]
  %25 = lshr i16 %24, 6
  %26 = shl nuw nsw i16 %25, 6
  %27 = sub i16 %24, %26
  %28 = add i16 %21, %26
  %29 = and i16 %24, 192
  %30 = sub nsw i16 %24, %29
  %31 = getelementptr i8, ptr @"DT100%", i16 %22
  %32 = getelementptr i8, ptr %31, i16 674
  store i16 %27, ptr %32, !tbaa !2
  %33 = icmp eq i16 %23, -1
  br i1 %33, label %b21, label %b23

b21:
  %34 = sub nsw i16 63, %30
  %35 = getelementptr i8, ptr @"DT100%", i16 %22
  %36 = getelementptr i8, ptr %35, i16 674
  store i16 %28, ptr %36, !tbaa !2
  br label %b23

b23:
  %37 = phi i16 [ %34, %b21 ], [ %27, %b19 ]
  %38 = icmp eq i16 %37, 63
  %39 = icmp eq i16 %23, 1
  %40 = and i1 %38, %39
  %spec.store.select2 = select i1 %40, i16 -1, i16 %23
  %41 = icmp eq i16 %37, 0
  %42 = icmp eq i16 %spec.store.select2, -1
  %43 = select i1 %41, i1 %42, i1 false
  %spec.store.select3 = select i1 %43, i16 1, i16 %spec.store.select2
  %44 = add nuw nsw i16 %24, 1
  %45 = add nsw i16 %22, 2
  %46 = add nsw i16 %21, -1
  %exitcond1 = icmp eq i16 %45, 0
  br i1 %exitcond1, label %47, label %b19

47:
  br label %b33

b33:
  %48 = phi i16 [ 44, %47 ], [ %57, %b33 ]
  %49 = phi i16 [ 48, %47 ], [ %56, %b33 ]
  %50 = phi i32 [ %55, %b33 ], [ 0, %47 ]
  %51 = getelementptr i8, ptr @"DT100%", i16 %49
  %52 = load i16, ptr %51, !tbaa !2
  %53 = mul i16 %52, %48
  %54 = sext i16 %53 to i32
  %55 = add i32 %50, %54
  %56 = add nuw nsw i16 %49, 2
  %57 = add nuw nsw i16 %48, 1
  %exitcond = icmp eq i16 %56, 674
  br i1 %exitcond, label %b34, label %b33

b34:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %55)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
