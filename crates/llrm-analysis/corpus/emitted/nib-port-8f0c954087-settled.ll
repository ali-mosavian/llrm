target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i16 @value(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 8, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %2, !tbaa !2
  %5 = load i16, ptr %1, !tbaa !2
  %6 = icmp slt i16 %4, %5
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b3, label %b5

b3:
  %9 = load i16, ptr %3, !tbaa !2
  %10 = load i16, ptr %2, !tbaa !2
  %11 = load i16, ptr addrspace(1) %0
  %12 = icmp ult i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b6, label %b7

b4:
  %15 = load i16, ptr %2, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %2, !tbaa !2
  br label %b2

b5:
  %17 = load i16, ptr %3, !tbaa !2
  ret i16 %17

b6:
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %19 = load ptr addrspace(1), ptr addrspace(1) %18
  %20 = mul i16 %10, 2
  %21 = getelementptr i8, ptr addrspace(1) %19, i16 %20
  %22 = load i16, ptr addrspace(1) %21
  %23 = add i16 %9, %22
  store i16 %23, ptr %3, !tbaa !2
  br label %b4

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
