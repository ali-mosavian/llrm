target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @step(i16 %0) addrspace(1) {
b1:
  %1 = add i16 %0, 1
  ret i16 %1
}

define internal i16 @count(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp slt i16 %2, %0
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr %1, !tbaa !2
  %7 = call addrspace(1) i16 @step(i16 %6)
  store i16 %7, ptr %1, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = icmp eq i16 %8, 3
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %1, !tbaa !2
  ret i16 %12

b5:
  br label %b2

b6:
  br label %b7

b7:
  %13 = load i16, ptr %1, !tbaa !2
  %14 = icmp sgt i16 %13, 10
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b8, label %b9

b8:
  br label %b4

b9:
  br label %b10

b10:
  br label %b2
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
