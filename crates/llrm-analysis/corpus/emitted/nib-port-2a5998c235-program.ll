target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @value() addrspace(1) {
b1:
  %0 = sub i32 0, 128
  %1 = zext i8 31 to i32
  %2 = ashr i32 %0, %1
  %3 = and i32 %2, 255
  %4 = add i32 %0, %3
  %5 = zext i8 8 to i32
  %6 = ashr i32 %4, %5
  ret i32 %6
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
