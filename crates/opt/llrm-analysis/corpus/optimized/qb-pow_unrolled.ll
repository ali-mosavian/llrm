target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = tail call double @llvm.log2.f64(double 1.000000e+01)
  br label %b5

b5:
  %1 = phi i16 [ -7, %b1 ], [ %8, %b8 ]
  %2 = sitofp i16 %1 to double
  %3 = fmul double %0, %2
  %4 = fcmp oge double %3, 3.276750e+04
  br i1 %4, label %b7, label %b8

b6:
  ret void

b7:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b8:
  %5 = tail call double @llvm.exp2.f64(double %3)
  %6 = fmul double %5, 0x42D6BCC41E8FFFFD
  %7 = fpround double %6 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %7)
  %8 = add nsw i16 %1, 1
  %exitcond = icmp eq i16 %8, 8
  br i1 %exitcond, label %b6, label %b5
}

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
