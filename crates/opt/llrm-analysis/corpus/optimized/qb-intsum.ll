target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@DONE$ = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSR4(float 2.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSR4(float 1.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER4(float 1.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSR4(float 0.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSR4(float -1.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER4(float -1.000000e+00)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string5, ptr @DONE$)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @DONE$)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PSR4(float) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PER4(float) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
