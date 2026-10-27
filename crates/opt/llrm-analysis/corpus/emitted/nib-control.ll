target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i16 @step(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = add i16 %0, 1
  ret i16 %1
}

define internal cc 1005 i16 @count(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp slt i16 %2, %0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = call cc 1005 addrspace(1) i16 @step(i16 %4)
  store i16 %5, ptr %1, !tbaa !2
  %6 = load i16, ptr %1, !tbaa !2
  %7 = icmp eq i16 %6, 3
  br i1 %7, label %b5, label %b6

b4:
  %8 = load i16, ptr %1, !tbaa !2
  ret i16 %8

b5:
  br label %b2

b6:
  br label %b7

b7:
  %9 = load i16, ptr %1, !tbaa !2
  %10 = icmp sgt i16 %9, 10
  br i1 %10, label %b8, label %b9

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
