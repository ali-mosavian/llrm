target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @step(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = add i16 %0, 1
  ret i16 %1
}

define internal i16 @count(i16 %0) addrspace(1) memory(none) {
b1:
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %3, %b5 ], [ %3, %b10 ]
  %2 = icmp slt i16 %1, %0
  br i1 %2, label %b3, label %b4

b3:
  %3 = add i16 %1, 1
  %4 = icmp eq i16 %3, 3
  br i1 %4, label %b5, label %b6

b4:
  %5 = phi i16 [ %1, %b2 ], [ %3, %b8 ]
  ret i16 %5

b5:
  br label %b2

b6:
  %6 = icmp sgt i16 %3, 10
  br i1 %6, label %b8, label %b10

b8:
  br label %b4

b10:
  br label %b2
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
