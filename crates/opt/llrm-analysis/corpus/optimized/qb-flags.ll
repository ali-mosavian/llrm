target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string4 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0D\00", ptr getelementptr (i8, ptr @$string4, i16 4), [14 x i8] c"SPLIT=nonzero\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0D\00", ptr getelementptr (i8, ptr @$string6, i16 4), [14 x i8] c"FUSED=nonzero\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0E\00", ptr getelementptr (i8, ptr @$string8, i16 4), [14 x i8] c"MIRROR=nonzero" }>
@$string9 = internal constant <{ [2 x i8], ptr, [10 x i8] }> <{ [2 x i8] c"\09\00", ptr getelementptr (i8, ptr @$string9, i16 4), [10 x i8] c"BOTH=zero\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [8 x i8] }> <{ [2 x i8] c"\08\00", ptr getelementptr (i8, ptr @$string12, i16 4), [8 x i8] c"SIGN=pos" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string13)
  ret void
}

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
