target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = tail call double @llvm.log2.f64(double 1.000000e+01)
  %1 = fmul double %0, 3.080000e+02
  %2 = fcmp oge double %1, 3.276750e+04
  br i1 %2, label %b11, label %b12

b11:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b12:
  %3 = tail call double @llvm.exp2.f64(double %1)
  %4 = fpround double %3 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %4)
  %5 = tail call float @llvm.log2.f32(float 1.000000e+01)
  %6 = fmul float %5, 3.800000e+01
  %7 = fcmp oge float %6, 3.276750e+04
  br i1 %7, label %b26, label %b27

b26:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b27:
  %8 = tail call float @llvm.exp2.f32(float %6)
  %9 = fpround float %8 to float
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER4(float %9)
  %10 = fmul double %0, 2.200000e+01
  %11 = fcmp oge double %10, 3.276750e+04
  br i1 %11, label %b41, label %b42

b41:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b42:
  %12 = tail call double @llvm.exp2.f64(double %10)
  %13 = fpround double %12 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %13)
  %14 = tail call double @llvm.log2.f64(double 2.000000e+00)
  %15 = fmul double %14, 5.000000e-01
  %16 = fcmp oge double %15, 3.276750e+04
  br i1 %16, label %b56, label %b57

b56:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b57:
  %17 = tail call double @llvm.exp2.f64(double %15)
  %18 = fpround double %17 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %18)
  ret void
}

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.log2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PER4(float) addrspace(1) nocallback

declare float @llvm.exp2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
