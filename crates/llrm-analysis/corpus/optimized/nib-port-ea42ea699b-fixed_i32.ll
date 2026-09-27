target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal i32 @product(i32 %0, i32 %1) addrspace(1) memory(none) willreturn {
b1:
  ret i32 25600000
}

define internal i32 @quotient(i32 %0, i32 %1) addrspace(1) memory(none) willreturn {
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
  call addrspace(1) void @N$PQ4(i32 262144, i8 9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PQ4(i32 -262144, i8 9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PQ4(i32 -512, i8 9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PQ4(i32 -2147483648, i8 9)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$PQ4(i32 25600000, i8 9)
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
