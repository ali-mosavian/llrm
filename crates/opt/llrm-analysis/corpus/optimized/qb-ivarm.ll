target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"BRANCHCHOICE&" = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>
@"VALUES%.0" = internal global i16 0, align 2
@"VALUES%.1" = internal global i16 0, align 2

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE&" to ptr addrspace(1)))
  %0 = load i32, ptr @"BRANCHCHOICE&", !tbaa !2
  %1 = icmp eq i32 %0, 0
  br i1 %1, label %2, label %4

2:
  store i16 34, ptr @"VALUES%.1", !tbaa !2
  %3 = load i16, ptr @"VALUES%.0", !tbaa !2
  br label %5

4:
  store i16 34, ptr @"VALUES%.0", !tbaa !2
  br label %5

5:
  %6 = phi i16 [ 34, %4 ], [ %3, %2 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %6)
  %7 = load i16, ptr @"VALUES%.1", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %7)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 37)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDI4(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
