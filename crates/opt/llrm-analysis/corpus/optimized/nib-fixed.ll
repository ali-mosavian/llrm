target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [13 x i8] c"\08\00\06\00\06\00fixed=\00"

define internal i16 @product(i16 %0, i16 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = sext i16 %0 to i32
  %3 = sext i16 %1 to i32
  %4 = mul i32 %2, %3
  %5 = ashr i32 %4, 8
  %6 = trunc i32 %5 to i16
  ret i16 %6
}

define internal i32 @quotient(i32 %0, i32 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = sext i32 %0 to i64
  %3 = sext i32 %1 to i64
  %4 = shl i64 %2, 16
  %5 = sdiv i64 %4, %3
  %6 = trunc i64 %5 to i32
  ret i32 %6
}

define internal i32 @fixed_literals() addrspace(1) {
b1:
  call addrspace(1) void @N$PQ4(i32 147456, i8 16)
  call addrspace(1) void @N$PN()
  %0 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PQ4(i32 147456, i8 16)
  call addrspace(1) void @N$PN()
  ret i32 147456
}

define internal i16 @main() addrspace(1) {
b1:
  call addrspace(1) void @N$PQ4(i32 147456, i8 16)
  call addrspace(1) void @N$PN()
  %0 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PQ4(i32 147456, i8 16)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PS(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
