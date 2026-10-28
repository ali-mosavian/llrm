target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X#" = internal global [8 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
  %0 = load double, ptr @"X#", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
  %1 = load double, ptr @"X#", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
  %2 = load double, ptr @"X#", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
  %3 = load double, ptr @"X#", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
  %4 = load double, ptr @"X#", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 1.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 0x41E0000000000000)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER4(float 0x423CBE9920000000)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 0x41E0000000000000)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 0x423CBE991A140000)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 0x43118B54F22AEB00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 1.000000e+20)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDR8(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PER4(float) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
