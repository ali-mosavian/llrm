target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DIV=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"FADD=\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"FSUB=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"MOD=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"FMUL=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"FDIV=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"FHALF=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string11, i16 4), [6 x i8] c"FSQR=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string12, i16 4), [6 x i8] c"SADD=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string13, i16 4), [6 x i8] c"SMUL=\00" }>
@$string14 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string14, i16 4), [4 x i8] c"AND=" }>
@$string15 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string15, i16 4), [6 x i8] c"FCMP=\00" }>
@$string16 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string16, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 1048576)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 1049600)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 1047552)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 1073741824)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 1024)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
  %0 = tail call double @llvm.sqrt.f64(double 1.048576e+06)
  %1 = tail call i32 @llvm.lrint.i32.f64(double %0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 1048580)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 4194304)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 1073741824)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 -1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string16)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.sqrt.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
