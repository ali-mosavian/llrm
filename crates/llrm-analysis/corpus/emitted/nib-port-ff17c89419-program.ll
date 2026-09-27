target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = zext i8 31 to i32
  %1 = ashr i32 768, %0
  %2 = and i32 %1, 255
  %3 = add i32 768, %2
  %4 = zext i8 8 to i32
  %5 = ashr i32 %3, %4
  %6 = trunc i32 %5 to i16
  %7 = zext i8 31 to i32
  %8 = ashr i32 704, %7
  %9 = and i32 %8, 255
  %10 = add i32 704, %9
  %11 = zext i8 8 to i32
  %12 = ashr i32 %10, %11
  %13 = trunc i32 %12 to i16
  %14 = add i16 %6, %13
  ret i16 %14
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
