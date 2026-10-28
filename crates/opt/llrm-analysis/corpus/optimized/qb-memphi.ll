target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"INPUTVALUE%" = internal global [2 x i8] zeroinitializer
@"BRANCHCHOICE%" = internal global [2 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE%" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  %0 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %1 = icmp eq i16 %0, 0
  %2 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %3 = select i1 %1, i16 2, i16 1
  %4 = add i16 %2, 3
  %5 = add i16 %4, %3
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE%" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  %6 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %7 = icmp eq i16 %6, 0
  %8 = select i1 %7, i16 2, i16 1
  %9 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %10 = add i16 %9, 3
  %11 = add i16 %10, %8
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
