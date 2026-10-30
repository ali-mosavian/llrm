target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A%" = internal global [46 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 5, i16 2, i16 257, ptr @"A%")
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 2), !tbaa !2
  %1 = inttoptr i16 %0 to ptr addrspace(2)
  %2 = addrspacecast ptr addrspace(2) %1 to ptr addrspace(1)
  %3 = getelementptr i8, ptr addrspace(1) %2, i16 2
  store i16 7, ptr addrspace(1) %3, !tbaa !6
  %4 = getelementptr i8, ptr addrspace(1) %2, i16 10
  store i16 9, ptr addrspace(1) %4, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDMP(i16 0, i16 10, i16 2, i16 257, ptr @"A%")
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 2), !tbaa !2
  %6 = inttoptr i16 %5 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 2
  %9 = load i16, ptr addrspace(1) %8, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %9)
  %10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 2), !tbaa !2
  %11 = inttoptr i16 %10 to ptr addrspace(2)
  %12 = addrspacecast ptr addrspace(2) %11 to ptr addrspace(1)
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 10
  %14 = load i16, ptr addrspace(1) %13, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %14)
  %15 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 2), !tbaa !2
  %16 = inttoptr i16 %15 to ptr addrspace(2)
  %17 = addrspacecast ptr addrspace(2) %16 to ptr addrspace(1)
  %18 = getelementptr i8, ptr addrspace(1) %17, i16 20
  %19 = load i16, ptr addrspace(1) %18, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %19)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$RDMP(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
