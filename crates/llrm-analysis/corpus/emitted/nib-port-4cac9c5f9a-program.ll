target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = sext i16 127 to i32
  %1 = zext i8 4 to i32
  %2 = shl i32 %0, %1
  %3 = sext i32 %2 to i64
  %4 = sext i32 4096 to i64
  %5 = mul i64 %3, %4
  %6 = ashr i64 %5, 8
  %7 = trunc i64 %6 to i32
  %8 = zext i8 31 to i32
  %9 = ashr i32 %7, %8
  %10 = and i32 %9, 255
  %11 = add i32 %7, %10
  %12 = zext i8 8 to i32
  %13 = ashr i32 %11, %12
  %14 = trunc i32 %13 to i16
  ret i16 %14
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
