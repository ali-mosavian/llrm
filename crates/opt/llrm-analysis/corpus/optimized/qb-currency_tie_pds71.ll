target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 3)
  ret void
}

declare i64 @llvm.lrint.i64.f80(x86_fp80) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PECY(i64) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
