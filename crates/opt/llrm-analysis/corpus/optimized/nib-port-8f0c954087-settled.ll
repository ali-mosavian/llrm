target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %11, %b6 ]
  %5 = phi i16 [ 0, %b1 ], [ %12, %b6 ]
  %6 = icmp slt i16 %5, 8
  br i1 %6, label %b3, label %b5

b3:
  %7 = icmp ult i16 %5, %1
  br i1 %7, label %b6, label %b7

b5:
  ret i16 %4

b6:
  %8 = shl i16 %5, 1
  %9 = getelementptr i8, ptr addrspace(1) %3, i16 %8
  %10 = load i16, ptr addrspace(1) %9
  %11 = add i16 %4, %10
  %12 = add i16 %5, 1
  br label %b2

b7:
  call addrspace(1) void @N$EBND()
  unreachable
}

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
