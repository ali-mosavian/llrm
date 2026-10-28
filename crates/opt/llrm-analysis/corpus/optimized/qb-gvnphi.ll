target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"INPUTVALUE&" = internal global [4 x i8] zeroinitializer
@"BRANCHCHOICE%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE&" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  %0 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %1 = icmp eq i16 %0, 0
  %2 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %3 = select i1 %1, i32 2, i32 1
  %4 = select i1 %1, i32 -1, i32 1
  %5 = add i32 %2, %3
  store i32 %5, ptr @"INPUTVALUE&", !tbaa !2
  %6 = mul i32 %5, %5
  %7 = add i32 %6, %4
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE&" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  %8 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %9 = icmp eq i16 %8, 0
  %10 = select i1 %9, i32 2, i32 1
  %11 = select i1 %9, i32 -1, i32 1
  %12 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %13 = add i32 %12, %10
  store i32 %13, ptr @"INPUTVALUE&", !tbaa !2
  %14 = mul i32 %13, %13
  %15 = add i32 %14, %11
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %15)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDI4(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
