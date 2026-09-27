target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = sub i32 1408, 704
  %1 = zext i8 31 to i32
  %2 = ashr i32 %0, %1
  %3 = and i32 %2, 15
  %4 = add i32 %0, %3
  %5 = zext i8 4 to i32
  %6 = ashr i32 %4, %5
  %7 = trunc i32 %6 to i16
  %8 = sext i16 %7 to i32
  %9 = sext i16 64 to i32
  %10 = mul i32 %8, %9
  %11 = zext i8 4 to i32
  %12 = ashr i32 %10, %11
  %13 = trunc i32 %12 to i16
  %14 = zext i8 15 to i16
  %15 = ashr i16 %13, %14
  %16 = and i16 %15, 15
  %17 = add i16 %13, %16
  %18 = zext i8 4 to i16
  %19 = ashr i16 %17, %18
  ret i16 %19
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
