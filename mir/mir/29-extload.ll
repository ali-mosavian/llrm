target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@$string3 = internal constant <{ [4 x i8], ptr, [6 x i8] }> <{ [4 x i8] c"\05\00\00\00", ptr getelementptr (i8, ptr @$string3, i16 8), [6 x i8] c"HELLO\00" }>

define internal cc 1000 void @__main() "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1000 void @llrm.qb.B$PESD(ptr @$string3)
  tail call cc 1000 void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$PESD(ptr) nocallback

declare cc 1000 void @llrm.qb.B$CEND() nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
