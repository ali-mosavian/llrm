target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X%" = internal global [2 x i8] zeroinitializer
@b$seg = external global [2 x i8]

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = addrspacecast ptr @"X%" to ptr addrspace(1)
  %1 = addrspacecast ptr addrspace(1) %0 to ptr addrspace(2)
  %2 = ptrtoint ptr addrspace(2) %1 to i16
  store i16 %2, ptr @b$seg, !tbaa !2
  %3 = addrspacecast ptr @"X%" to ptr addrspace(1)
  %4 = ptrtoint ptr addrspace(1) %3 to i16
  %5 = load i16, ptr @b$seg, !tbaa !2
  %6 = inttoptr i16 %5 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 %4
  store volatile i8 5, ptr addrspace(1) %3, !tbaa !2
  %9 = addrspacecast ptr @"X%" to ptr addrspace(1)
  %10 = ptrtoint ptr addrspace(1) %9 to i16
  %11 = add i16 %10, 1
  %12 = load i16, ptr @b$seg, !tbaa !2
  %13 = inttoptr i16 %12 to ptr addrspace(2)
  %14 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 %11
  %16 = getelementptr i8, ptr addrspace(1) %9, i16 1
  store volatile i8 2, ptr addrspace(1) %16, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  %17 = load i16, ptr @"X%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %17)
  ret void
}

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
