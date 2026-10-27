target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [46 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 5, i16 2, i16 257, ptr @"A%")
  %0 = getelementptr i8, ptr @"A%", i16 2
  %1 = load i16, ptr %0, !tbaa !2
  %2 = inttoptr i16 %1 to ptr addrspace(2)
  %3 = addrspacecast ptr addrspace(2) %2 to ptr addrspace(1)
  %4 = getelementptr i8, ptr addrspace(1) %3, i16 2
  store i16 7, ptr addrspace(1) %4, !tbaa !6
  %5 = getelementptr i8, ptr @"A%", i16 2
  %6 = load i16, ptr %5, !tbaa !2
  %7 = inttoptr i16 %6 to ptr addrspace(2)
  %8 = addrspacecast ptr addrspace(2) %7 to ptr addrspace(1)
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 10
  store i16 9, ptr addrspace(1) %9, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$RDMP(i16 0, i16 10, i16 2, i16 257, ptr @"A%")
  %10 = getelementptr i8, ptr @"A%", i16 2
  %11 = load i16, ptr %10, !tbaa !2
  %12 = inttoptr i16 %11 to ptr addrspace(2)
  %13 = addrspacecast ptr addrspace(2) %12 to ptr addrspace(1)
  %14 = getelementptr i8, ptr addrspace(1) %13, i16 2
  %15 = load i16, ptr addrspace(1) %14, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %15)
  %16 = getelementptr i8, ptr @"A%", i16 2
  %17 = load i16, ptr %16, !tbaa !2
  %18 = inttoptr i16 %17 to ptr addrspace(2)
  %19 = addrspacecast ptr addrspace(2) %18 to ptr addrspace(1)
  %20 = getelementptr i8, ptr addrspace(1) %19, i16 10
  %21 = load i16, ptr addrspace(1) %20, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %21)
  %22 = getelementptr i8, ptr @"A%", i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = inttoptr i16 %23 to ptr addrspace(2)
  %25 = addrspacecast ptr addrspace(2) %24 to ptr addrspace(1)
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 20
  %27 = load i16, ptr addrspace(1) %26, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %27)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDMP(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
