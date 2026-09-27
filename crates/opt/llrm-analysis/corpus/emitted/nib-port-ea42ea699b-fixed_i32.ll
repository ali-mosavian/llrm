target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @product(i32 %0, i32 %1) addrspace(1) {
b1:
  %2 = sext i32 %0 to i64
  %3 = sext i32 %1 to i64
  %4 = mul i64 %2, %3
  %5 = ashr i64 %4, 9
  %6 = trunc i64 %5 to i32
  ret i32 %6
}

define internal i32 @quotient(i32 %0, i32 %1) addrspace(1) {
b1:
  %2 = sext i32 %0 to i64
  %3 = sext i32 %1 to i64
  %4 = shl i64 %2, 9
  %5 = sdiv i64 %4, %3
  %6 = trunc i64 %5 to i32
  ret i32 %6
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i32
  %1 = alloca i32
  %2 = alloca i32
  %3 = alloca i32
  %4 = alloca i32
  store i32 0, ptr %0
  store i32 0, ptr %1
  store i32 0, ptr %2
  store i32 0, ptr %3
  store i32 0, ptr %4
  %5 = call addrspace(1) i32 @quotient(i32 512, i32 1)
  store i32 %5, ptr %4, !tbaa !2
  %6 = load i32, ptr %4, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %6, i8 9)
  call addrspace(1) void @N$PN()
  %7 = call addrspace(1) i32 @quotient(i32 -512, i32 1)
  store i32 %7, ptr %3, !tbaa !2
  %8 = load i32, ptr %3, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %8, i8 9)
  call addrspace(1) void @N$PN()
  %9 = call addrspace(1) i32 @quotient(i32 2147483647, i32 1)
  store i32 %9, ptr %2, !tbaa !2
  %10 = load i32, ptr %2, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %10, i8 9)
  call addrspace(1) void @N$PN()
  %11 = call addrspace(1) i32 @quotient(i32 -2147483648, i32 -512)
  store i32 %11, ptr %1, !tbaa !2
  %12 = load i32, ptr %1, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %12, i8 9)
  call addrspace(1) void @N$PN()
  %13 = call addrspace(1) i32 @product(i32 51200000, i32 256)
  store i32 %13, ptr %0, !tbaa !2
  %14 = load i32, ptr %0, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %14, i8 9)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
