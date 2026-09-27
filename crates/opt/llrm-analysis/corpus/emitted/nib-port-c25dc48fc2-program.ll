target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value() addrspace(1) {
b1:
  %0 = alloca i8
  %1 = alloca i8
  store i8 0, ptr %0
  store i8 0, ptr %1
  store i8 -1, ptr %1, !tbaa !2
  %2 = icmp ne i8 -1, 0
  br i1 %2, label %b3, label %b2

b2:
  store i8 0, ptr %0, !tbaa !2
  %3 = icmp ne i8 0, 0
  br i1 %3, label %b4, label %b5

b3:
  %4 = load i8, ptr %1, !tbaa !2
  %5 = sext i8 %4 to i16
  %6 = and i16 %5, 1
  ret i16 %6

b4:
  store i8 0, ptr %0, !tbaa !2
  br label %b5

b5:
  %7 = load i8, ptr %0, !tbaa !2
  store i8 %7, ptr %1, !tbaa !2
  br label %b3
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
