target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @main() addrspace(1) {
b1:
  call addrspace(1) void @N$PQ4(i32 768, i8 8)
  %0 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PI2(i16 -2)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PI4(i32 0)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PQ2(i16 44, i8 4)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PQ4(i32 51200, i8 8)
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PI2(i16 7)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PQ2(i16, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
