target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @main() addrspace(1) {
b1:
  call addrspace(1) void @N$PI1(i8 -128)
  %0 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PU1(i8 -1)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PI2(i16 -32768)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PU2(i16 -1)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PI4(i32 -2147483648)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PU4(i32 -1)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PI4(i32 0)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$PI1(i8) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU1(i8) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PU4(i32) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
