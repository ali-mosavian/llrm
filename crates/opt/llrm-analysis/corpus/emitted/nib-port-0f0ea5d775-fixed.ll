target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @scaled(i32 %0, i32 %1) addrspace(1) {
b1:
  %2 = sext i32 %0 to i64
  %3 = sext i32 %1 to i64
  %4 = mul i64 %2, %3
  %5 = ashr i64 %4, 16
  %6 = trunc i64 %5 to i32
  %7 = sext i32 %6 to i64
  %8 = sext i32 %1 to i64
  %9 = shl i64 %7, 16
  %10 = sdiv i64 %9, %8
  %11 = trunc i64 %10 to i32
  ret i32 %11
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @scaled(i32 98304, i32 147456)
  ret i16 0
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
