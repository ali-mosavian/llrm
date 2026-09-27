target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = sext i8 -1 to i16
  %1 = and i16 %0, 1
  %2 = sext i8 0 to i16
  %3 = and i16 %2, 1
  %4 = add i16 %1, %3
  ret i16 %4
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
