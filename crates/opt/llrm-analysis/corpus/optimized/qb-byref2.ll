target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER4(float 2.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 1.600000e+01)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  ret void
}

define cc 1000 ptr @"HALF!"(ptr dereferenceable(4) nocapture readonly %0, ptr writeonly initializes((0, 4)) %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: readwrite) willreturn nounwind nofree nosync norecurse {
b1:
  %2 = load float, ptr %0
  %3 = fmul float %2, 5.000000e-01
  %4 = fpround float %3 to float
  store float %4, ptr %1
  ret ptr %1
}

define cc 1000 ptr @"DOUBLED#"(ptr dereferenceable(8) nocapture readonly %0, ptr writeonly initializes((0, 8)) %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: readwrite) willreturn nounwind nofree nosync norecurse {
b1:
  %2 = load double, ptr %0
  %3 = fmul double %2, 2.000000e+00
  %4 = fpround double %3 to double
  store double %4, ptr %1
  ret ptr %1
}

declare cc 1005 void @llrm.qb.B$PER4(float) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
