target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = zext i8 8 to i32
  %1 = shl i32 200, %0
  %2 = zext i8 31 to i32
  %3 = ashr i32 %1, %2
  %4 = and i32 %3, 255
  %5 = add i32 %1, %4
  %6 = zext i8 8 to i32
  %7 = ashr i32 %5, %6
  %8 = trunc i32 %7 to i16
  ret i16 %8
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
