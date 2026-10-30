target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i8
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  store i16 3, ptr %3, !tbaa !2
  store i32 -704, ptr %2, !tbaa !2
  %4 = load i32, ptr %2, !tbaa !2
  %5 = add i32 1408, %4
  %6 = ashr i32 %5, 31
  %7 = and i32 %6, 15
  %8 = add i32 %5, %7
  %9 = ashr i32 %8, 4
  %10 = trunc i32 %9 to i16
  store i16 %10, ptr %1, !tbaa !2
  store i8 -56, ptr %0, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = sext i16 %11 to i32
  %13 = shl i32 %12, 8
  call cc 1005 addrspace(1) void @N$PQ4(i32 %13, i8 zeroext 8)
  %14 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %14)
  %15 = load i32, ptr %2, !tbaa !2
  %16 = ashr i32 %15, 31
  %17 = and i32 %16, 255
  %18 = add i32 %15, %17
  %19 = ashr i32 %18, 8
  %20 = trunc i32 %19 to i16
  call cc 1005 addrspace(1) void @N$PI2(i16 %20)
  %21 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  call cc 1005 addrspace(1) void @N$PI4(i32 0)
  %22 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %22)
  %23 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PQ2(i16 %23, i8 zeroext 4)
  %24 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %24)
  %25 = load i8, ptr %0, !tbaa !2
  %26 = zext i8 %25 to i32
  %27 = shl i32 %26, 8
  call cc 1005 addrspace(1) void @N$PQ4(i32 %27, i8 zeroext 8)
  %28 = getelementptr i8, ptr @$str1, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %28)
  call cc 1005 addrspace(1) void @N$PI2(i16 7)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0
}

declare cc 1005 void @N$PQ4(i32, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI4(i32) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PQ2(i16, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
