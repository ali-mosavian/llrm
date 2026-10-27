target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"INPUTVALUE%" = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"VALUE=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE%" to ptr addrspace(1)))
  %0 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %1 = add i16 %0, 1
  %2 = sitofp i16 %1 to double
  %3 = fpround double %2 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %4 = tail call i32 @llvm.lrint.i32.f64(double %3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE%" to ptr addrspace(1)))
  %5 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %6 = add i16 %5, 1
  %7 = sitofp i16 %6 to double
  %8 = fpround double %7 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %9 = tail call i32 @llvm.lrint.i32.f64(double %8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE%" to ptr addrspace(1)))
  %10 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %11 = add i16 %10, 1
  %12 = sitofp i16 %11 to double
  %13 = fpround double %12 to double
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %14 = tail call i32 @llvm.lrint.i32.f64(double %13)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

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
