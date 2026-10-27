target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string3 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string3, i16 4), [6 x i8] c"start\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse noreturn {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable
}

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
