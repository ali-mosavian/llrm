target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define internal cc 1005 i8 @boom(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = sdiv i16 1, %0
  %2 = srem i16 1, %0
  %3 = icmp ne i16 %2, 0
  %4 = zext i1 %3 to i8
  %5 = xor i16 %2, %0
  %6 = icmp slt i16 %5, 0
  %7 = zext i1 %6 to i8
  %8 = and i8 %4, %7
  %9 = zext i8 %8 to i16
  %10 = and i16 %9, 1
  %11 = sub i16 %1, %10
  %12 = icmp eq i16 %11, 0
  %13 = zext i1 %12 to i8
  ret i8 %13
}

define internal cc 1005 i16 @value() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i8
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
  store i8 0, ptr %1, !tbaa !2
  br i1 false, label %b2, label %b3

b2:
  %3 = load i16, ptr %2, !tbaa !2
  %4 = call cc 1005 addrspace(1) i8 @boom(i16 %3)
  store i8 %4, ptr %1, !tbaa !2
  br label %b3

b3:
  %5 = load i8, ptr %1, !tbaa !2, !range !5
  %6 = zext i8 %5 to i16
  %7 = and i16 %6, 1
  store i8 1, ptr %0, !tbaa !2
  br i1 true, label %b5, label %b4

b4:
  %8 = load i16, ptr %2, !tbaa !2
  %9 = call cc 1005 addrspace(1) i8 @boom(i16 %8)
  store i8 %9, ptr %0, !tbaa !2
  br label %b5

b5:
  %10 = load i8, ptr %0, !tbaa !2, !range !6
  %11 = zext i8 %10 to i16
  %12 = and i16 %11, 1
  %13 = add i16 %7, %12
  ret i16 %13
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
