target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  %0 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$DTSR(i16 1992, i16 2, i16 29)
  %1 = load double, ptr %0
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %1)
  %2 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$DTSR(i16 1899, i16 12, i16 30)
  %3 = load double, ptr %2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 ptr @llrm.qb.B$DTSR(i16, i16, i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
