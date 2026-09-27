target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i8
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  store i8 0, ptr %0
  store i16 0, ptr %1
  store i32 0, ptr %2
  store i16 0, ptr %3
  store i16 3, ptr %3, !tbaa !2
  %4 = sub i32 0, 704
  store i32 %4, ptr %2, !tbaa !2
  %5 = load i32, ptr %2, !tbaa !2
  %6 = add i32 1408, %5
  %7 = zext i8 31 to i32
  %8 = ashr i32 %6, %7
  %9 = and i32 %8, 15
  %10 = add i32 %6, %9
  %11 = zext i8 4 to i32
  %12 = ashr i32 %10, %11
  %13 = trunc i32 %12 to i16
  store i16 %13, ptr %1, !tbaa !2
  store i8 -56, ptr %0, !tbaa !2
  %14 = load i16, ptr %3, !tbaa !2
  %15 = sext i16 %14 to i32
  %16 = zext i8 8 to i32
  %17 = shl i32 %15, %16
  call addrspace(1) void @N$PQ4(i32 %17, i8 8)
  %18 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %18)
  %19 = load i32, ptr %2, !tbaa !2
  %20 = zext i8 31 to i32
  %21 = ashr i32 %19, %20
  %22 = and i32 %21, 255
  %23 = add i32 %19, %22
  %24 = zext i8 8 to i32
  %25 = ashr i32 %23, %24
  %26 = trunc i32 %25 to i16
  call addrspace(1) void @N$PI2(i16 %26)
  %27 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  %28 = sub i32 0, 128
  %29 = zext i8 31 to i32
  %30 = ashr i32 %28, %29
  %31 = and i32 %30, 255
  %32 = add i32 %28, %31
  %33 = zext i8 8 to i32
  %34 = ashr i32 %32, %33
  call addrspace(1) void @N$PI4(i32 %34)
  %35 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %35)
  %36 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PQ2(i16 %36, i8 4)
  %37 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %37)
  %38 = load i8, ptr %0, !tbaa !2
  %39 = zext i8 %38 to i32
  %40 = zext i8 8 to i32
  %41 = shl i32 %39, %40
  call addrspace(1) void @N$PQ4(i32 %41, i8 8)
  %42 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %42)
  %43 = zext i8 15 to i16
  %44 = ashr i16 127, %43
  %45 = and i16 %44, 15
  %46 = add i16 127, %45
  %47 = zext i8 4 to i16
  %48 = ashr i16 %46, %47
  call addrspace(1) void @N$PI2(i16 %48)
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
