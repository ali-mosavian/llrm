target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00normal \00"
@$str2 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, warning \00"
@$str3 = internal constant [18 x i8] c"\08\00\0B\00\0B\00, inverted \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00byte waiting, ready to send\00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00errors: \00"
@$str6 = internal constant [15 x i8] c"\08\00\08\00\08\00control \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00: parity \00"
@$str8 = internal constant [16 x i8] c"\08\00\09\00\09\00, offset \00"

define internal i8 @inverted(i8 %0) addrspace(1) memory(none) willreturn {
b1:
  ret i8 112
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %0)
  call addrspace(1) void @N$PU1(i8 7)
  %1 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %1)
  call addrspace(1) void @N$PU1(i8 -50)
  %2 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %2)
  call addrspace(1) void @N$PU1(i8 112)
  call addrspace(1) void @N$PN()
  %3 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %3)
  call addrspace(1) void @N$PN()
  %4 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %4)
  call addrspace(1) void @N$PB(i8 0)
  call addrspace(1) void @N$PN()
  %5 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %5)
  call addrspace(1) void @N$PU1(i8 55)
  %6 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %6)
  call addrspace(1) void @N$PU1(i8 2)
  %7 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %7)
  call addrspace(1) void @N$PI1(i8 1)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU1(i8) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare void @N$PI1(i8) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
