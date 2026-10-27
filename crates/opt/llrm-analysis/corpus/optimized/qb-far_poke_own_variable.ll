target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X%" = internal global [2 x i8] zeroinitializer
@b$seg = external global [2 x i8]

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @"X%" to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  store volatile i8 5, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), !tbaa !2
  store volatile i8 2, ptr addrspace(1) getelementptr (i8, ptr addrspace(1) addrspacecast (ptr @"X%" to ptr addrspace(1)), i16 1), !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$DSG0()
  %0 = load i16, ptr @"X%", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  ret void
}

declare cc 1005 void @llrm.qb.B$DSG0() addrspace(1) nocallback nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
