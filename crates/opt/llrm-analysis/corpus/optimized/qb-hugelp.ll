target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"ITEMS%" = internal global [22 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 200, i16 -2, i16 198, i16 2, i16 258, ptr @"ITEMS%")
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %1 = inttoptr i16 %0 to ptr addrspace(2)
  %2 = addrspacecast ptr addrspace(2) %1 to ptr addrspace(1)
  %3 = getelementptr i8, ptr addrspace(1) %2, i16 -2
  store i16 456, ptr addrspace(1) %3, !tbaa !6
  %4 = getelementptr i8, ptr addrspace(1) %2, i16 1934
  store i16 789, ptr addrspace(1) %4, !tbaa !6
  store i16 457, ptr addrspace(1) %2, !tbaa !6
  %5 = getelementptr i8, ptr addrspace(1) %2, i16 2336
  store i16 790, ptr addrspace(1) %5, !tbaa !6
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %7 = inttoptr i16 %6 to ptr addrspace(2)
  %8 = addrspacecast ptr addrspace(2) %7 to ptr addrspace(1)
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 -2
  %10 = load i16, ptr addrspace(1) %9, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %10)
  %11 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %12 = inttoptr i16 %11 to ptr addrspace(2)
  %13 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %14 = load i16, ptr addrspace(1) %13, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %14)
  %15 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2)
  %17 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 1934
  %19 = load i16, ptr addrspace(1) %18, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %19)
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %21 = inttoptr i16 %20 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 2336
  %24 = load i16, ptr addrspace(1) %23, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %24)
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
