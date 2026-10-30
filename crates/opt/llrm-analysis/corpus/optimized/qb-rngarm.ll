target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"FACTOR%" = internal global [2 x i8] zeroinitializer
@"TOTAL%" = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"FACTOR%" to ptr addrspace(1)))
  %1 = load i16, ptr @"FACTOR%", !tbaa !2
  %promoted = load i16, ptr @"TOTAL%", align 1
  %2 = add i16 %1, 3
  %3 = shl i16 %1, 2
  %4 = add i16 %3, %promoted
  store i16 %4, ptr @"TOTAL%", align 1
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
