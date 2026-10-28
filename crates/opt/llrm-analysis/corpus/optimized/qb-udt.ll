target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 305419896)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 252645135)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
