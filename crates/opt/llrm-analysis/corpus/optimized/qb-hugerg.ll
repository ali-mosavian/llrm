target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"ITEMS%" = internal global [22 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 200, i16 -2, i16 198, i16 2, i16 258, ptr @"ITEMS%")
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %1 = inttoptr i16 %0 to ptr addrspace(2)
  br label %b5

b5:
  %2 = phi i16 [ %12, %b5 ], [ 1934, %b1 ]
  %3 = phi i16 [ %11, %b5 ], [ 789, %b1 ]
  %4 = phi i16 [ %10, %b5 ], [ -394, %b1 ]
  %5 = addrspacecast ptr addrspace(2) %1 to ptr addrspace(1)
  %6 = getelementptr i8, ptr addrspace(1) %5, i16 %4
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 392
  %8 = add i16 %3, -333
  store i16 %8, ptr addrspace(1) %7, !tbaa !6
  %9 = getelementptr i8, ptr addrspace(1) %5, i16 %2
  store i16 %3, ptr addrspace(1) %9, !tbaa !6
  %10 = add nsw i16 %4, 2
  %11 = add nuw nsw i16 %3, 1
  %12 = add i16 %2, 402
  %exitcond = icmp eq i16 %10, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %14 = inttoptr i16 %13 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr addrspace(1) %15, i16 -2
  %17 = load i16, ptr addrspace(1) %16, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %17)
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %19 = inttoptr i16 %18 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = load i16, ptr addrspace(1) %20, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %21)
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %23 = inttoptr i16 %22 to ptr addrspace(2)
  %24 = addrspacecast ptr addrspace(2) %23 to ptr addrspace(1)
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 1934
  %26 = load i16, ptr addrspace(1) %25, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %26)
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %28 = inttoptr i16 %27 to ptr addrspace(2)
  %29 = addrspacecast ptr addrspace(2) %28 to ptr addrspace(1)
  %30 = getelementptr i8, ptr addrspace(1) %29, i16 2336
  %31 = load i16, ptr addrspace(1) %30, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %31)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  ret void
}

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
