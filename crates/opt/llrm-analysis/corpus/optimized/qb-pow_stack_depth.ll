target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = tail call double @llvm.log2.f64(double 1.000000e+01)
  %1 = fmul double 4.400000e+00, 5.500000e+00
  %2 = fmul double %1, 1.000000e-06
  %3 = fmul double 8.800000e+00, 1.000000e-03
  br label %b5

b5:
  %4 = phi i16 [ %30, %b8 ], [ -4, %b1 ]
  %5 = phi double [ 1.111000e+01, %b1 ], [ %29, %b8 ]
  %6 = phi double [ 1.010000e+01, %b1 ], [ %27, %b8 ]
  %7 = phi double [ 9.900000e+00, %b1 ], [ %24, %b8 ]
  %8 = phi double [ 2.200000e+00, %b1 ], [ %22, %b8 ]
  %9 = phi double [ 1.100000e+00, %b1 ], [ %17, %b8 ]
  %10 = add i16 %4, 5
  %11 = sitofp i16 %4 to double
  %12 = fmul double %0, %11
  %13 = fcmp oge double %12, 3.276750e+04
  br i1 %13, label %b7, label %b8

b6:
  ret void

b7:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b8:
  %14 = tail call double @llvm.exp2.f64(double %12)
  %15 = fmul double %9, %14
  %16 = fadd double %8, %15
  %17 = fpround double %16 to double
  %18 = uitofp nneg i16 %10 to double
  %19 = fadd double %18, 3.300000e+00
  %20 = fmul double %19, 4.400000e+00
  %21 = fadd double %8, %20
  %22 = fpround double %21 to double
  %23 = fadd double %2, %7
  %24 = fpround double %23 to double
  %25 = fmul double %6, 1.000100e+00
  %26 = fadd double %25, 7.700000e+00
  %27 = fpround double %26 to double
  %28 = fadd double %3, %5
  %29 = fpround double %28 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double %17)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double %22)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 3.300000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double 4.400000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double 5.500000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 6.600000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double 7.700000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double 8.800000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double %24)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PCR8(double %27)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %29)
  %30 = add nsw i16 %4, 1
  %exitcond = icmp eq i16 %30, 5
  br i1 %exitcond, label %b6, label %b5
}

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PCR8(double) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
