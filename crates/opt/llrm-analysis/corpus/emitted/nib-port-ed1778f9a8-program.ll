target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = alloca i16
  store i16 0, ptr %0
  store i16 1, ptr %0, !tbaa !2
  %1 = load i16, ptr %0, !tbaa !2
  %2 = shl i16 %1, 4
  store i16 %2, ptr %0, !tbaa !2
  %3 = load i16, ptr %0, !tbaa !2
  %4 = or i16 %3, 3
  store i16 %4, ptr %0, !tbaa !2
  %5 = load i16, ptr %0, !tbaa !2
  %6 = xor i16 %5, 1
  store i16 %6, ptr %0, !tbaa !2
  %7 = load i16, ptr %0, !tbaa !2
  %8 = and i16 %7, 255
  store i16 %8, ptr %0, !tbaa !2
  %9 = load i16, ptr %0, !tbaa !2
  %10 = lshr i16 %9, 1
  store i16 %10, ptr %0, !tbaa !2
  %11 = load i16, ptr %0, !tbaa !2
  ret i16 %11
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
