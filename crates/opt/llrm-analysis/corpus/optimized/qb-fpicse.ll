target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"INPUTVALUE&" = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"VALUE=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE&" to ptr addrspace(1)))
  %0 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %1 = sitofp i32 %0 to double
  %2 = fpround double %1 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %3 = tail call i32 @llvm.lrint.i32.f64(double %2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE&" to ptr addrspace(1)))
  %4 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %5 = sitofp i32 %4 to double
  %6 = fpround double %5 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %7 = tail call i32 @llvm.lrint.i32.f64(double %6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE&" to ptr addrspace(1)))
  %8 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %9 = sitofp i32 %8 to double
  %10 = fpround double %9 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %11 = tail call i32 @llvm.lrint.i32.f64(double %10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 void @llrm.qb.B$RDI4(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
