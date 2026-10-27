target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A%" = internal global [202 x i8] zeroinitializer, align 2
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"T=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"U=" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  br label %b5

b5:
  %0 = phi i16 [ %14, %b5 ], [ 0, %b1 ]
  %1 = phi i16 [ %13, %b5 ], [ -40, %b1 ]
  %2 = phi i16 [ 0, %b1 ], [ %11, %b5 ]
  %3 = phi i16 [ 1, %b1 ], [ %12, %b5 ]
  %4 = add i16 %0, 1
  %5 = getelementptr i8, ptr @"A%", i16 %1
  %6 = getelementptr i8, ptr %5, i16 42
  store i16 %4, ptr %6, !tbaa !2
  %7 = add i16 %2, %3
  %8 = getelementptr i8, ptr @"A%", i16 %1
  %9 = getelementptr i8, ptr %8, i16 44
  %10 = load i16, ptr %9, !tbaa !2
  %11 = add i16 %7, %10
  %12 = add nuw nsw i16 %3, 1
  %13 = add nsw i16 %1, 2
  %14 = add nuw nsw i16 %0, 1
  %exitcond = icmp eq i16 %13, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %15 = add i16 %2, %10
  %16 = add i16 20, %15
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %16)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 210)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void
}

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
