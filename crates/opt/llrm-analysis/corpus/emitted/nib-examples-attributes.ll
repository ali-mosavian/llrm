target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00normal \00"
@$str2 = internal constant [17 x i8] c"\08\00\0A\00\0A\00, warning \00"
@$str3 = internal constant [18 x i8] c"\08\00\0B\00\0B\00, inverted \00"
@$str4 = internal constant [34 x i8] c"\08\00\1B\00\1B\00byte waiting, ready to send\00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00errors: \00"
@$str6 = internal constant [15 x i8] c"\08\00\08\00\08\00control \00"
@$str7 = internal constant [16 x i8] c"\08\00\09\00\09\00: parity \00"
@$str8 = internal constant [16 x i8] c"\08\00\09\00\09\00, offset \00"

define internal i8 @inverted(i8 %0) addrspace(1) {
b1:
  %1 = alloca i8
  store i8 0, ptr %1
  store i8 %0, ptr %1, !tbaa !2
  %2 = load i8, ptr %1, !tbaa !2
  %3 = lshr i8 %0, 4
  %4 = and i8 %3, 7
  %5 = and i8 %4, 15
  %6 = and i8 %2, -16
  %7 = or i8 %6, %5
  store i8 %7, ptr %1, !tbaa !2
  %8 = load i8, ptr %1, !tbaa !2
  %9 = and i8 %0, 15
  %10 = zext i8 %9 to i16
  %11 = and i16 %10, 7
  %12 = trunc i16 %11 to i8
  %13 = and i8 %12, 7
  %14 = shl i8 %13, 4
  %15 = and i8 %8, -113
  %16 = or i8 %15, %14
  store i8 %16, ptr %1, !tbaa !2
  %17 = load i8, ptr %1, !tbaa !2
  ret i8 %17
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i8
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i8
  %6 = alloca i8
  %7 = alloca i8
  %8 = alloca i8
  %9 = alloca i8
  store i8 0, ptr %0
  store i8 0, ptr %1
  store i8 0, ptr %2
  store i8 0, ptr %3
  store i8 0, ptr %4
  store i8 0, ptr %5
  store i8 0, ptr %6
  store i8 0, ptr %7
  store i8 0, ptr %8
  store i8 0, ptr %9
  %10 = and i8 7, 15
  %11 = and i8 0, -16
  %12 = or i8 %11, %10
  %13 = and i8 0, 7
  %14 = shl i8 %13, 4
  %15 = and i8 %12, -113
  %16 = or i8 %15, %14
  %17 = and i8 0, 1
  %18 = shl i8 %17, 7
  %19 = and i8 %16, 127
  %20 = or i8 %19, %18
  store i8 %20, ptr %9, !tbaa !2
  %21 = and i8 14, 15
  %22 = and i8 0, -16
  %23 = or i8 %22, %21
  %24 = and i8 4, 7
  %25 = shl i8 %24, 4
  %26 = and i8 %23, -113
  %27 = or i8 %26, %25
  %28 = and i8 -1, 1
  %29 = shl i8 %28, 7
  %30 = and i8 %27, 127
  %31 = or i8 %30, %29
  store i8 %31, ptr %8, !tbaa !2
  %32 = load i8, ptr %9, !tbaa !2
  store i8 %32, ptr %7, !tbaa !2
  %33 = load i8, ptr %8, !tbaa !2
  store i8 %33, ptr %6, !tbaa !2
  %34 = load i8, ptr %9, !tbaa !2
  %35 = call addrspace(1) i8 @inverted(i8 %34)
  store i8 %35, ptr %5, !tbaa !2
  %36 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %36)
  %37 = load i8, ptr %7, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %37)
  %38 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %38)
  %39 = load i8, ptr %6, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %39)
  %40 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %40)
  %41 = load i8, ptr %5, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %41)
  call addrspace(1) void @N$PN()
  store i8 97, ptr %4, !tbaa !2
  %42 = load i8, ptr %4, !tbaa !2
  %43 = and i8 %42, 1
  %44 = icmp ne i8 %43, 0
  %45 = sext i1 %44 to i8
  store i8 %45, ptr %3, !tbaa !2
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b2, label %b3

b2:
  %47 = load i8, ptr %4, !tbaa !2
  %48 = lshr i8 %47, 5
  %49 = and i8 %48, 1
  %50 = icmp ne i8 %49, 0
  %51 = sext i1 %50 to i8
  store i8 %51, ptr %3, !tbaa !2
  br label %b3

b3:
  %52 = load i8, ptr %3, !tbaa !2
  %53 = icmp ne i8 %52, 0
  br i1 %53, label %b4, label %b5

b4:
  %54 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %54)
  call addrspace(1) void @N$PN()
  br label %b6

b5:
  br label %b6

b6:
  %55 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %55)
  %56 = load i8, ptr %4, !tbaa !2
  %57 = lshr i8 %56, 1
  %58 = and i8 %57, 1
  %59 = icmp ne i8 %58, 0
  %60 = sext i1 %59 to i8
  store i8 %60, ptr %1, !tbaa !2
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b8, label %b7

b7:
  %62 = load i8, ptr %4, !tbaa !2
  %63 = lshr i8 %62, 2
  %64 = and i8 %63, 1
  %65 = icmp ne i8 %64, 0
  %66 = sext i1 %65 to i8
  store i8 %66, ptr %1, !tbaa !2
  br label %b8

b8:
  %67 = load i8, ptr %1, !tbaa !2
  store i8 %67, ptr %2, !tbaa !2
  %68 = icmp ne i8 %67, 0
  br i1 %68, label %b10, label %b9

b9:
  %69 = load i8, ptr %4, !tbaa !2
  %70 = lshr i8 %69, 3
  %71 = and i8 %70, 1
  %72 = icmp ne i8 %71, 0
  %73 = sext i1 %72 to i8
  store i8 %73, ptr %2, !tbaa !2
  br label %b10

b10:
  %74 = load i8, ptr %2, !tbaa !2
  call addrspace(1) void @N$PB(i8 %74)
  call addrspace(1) void @N$PN()
  %75 = and i8 3, 3
  %76 = and i8 0, -4
  %77 = or i8 %76, %75
  %78 = and i8 0, 1
  %79 = shl i8 %78, 2
  %80 = and i8 %77, -5
  %81 = or i8 %80, %79
  %82 = and i8 2, 3
  %83 = shl i8 %82, 3
  %84 = and i8 %81, -25
  %85 = or i8 %84, %83
  %86 = and i8 -2, 7
  %87 = shl i8 %86, 5
  %88 = and i8 %85, 31
  %89 = or i8 %88, %87
  store i8 %89, ptr %0, !tbaa !2
  %90 = load i8, ptr %0, !tbaa !2
  %91 = and i8 1, 1
  %92 = shl i8 %91, 2
  %93 = and i8 %90, -5
  %94 = or i8 %93, %92
  store i8 %94, ptr %0, !tbaa !2
  %95 = load i8, ptr %0, !tbaa !2
  %96 = ashr i8 %95, 5
  %97 = sext i8 %96 to i16
  %98 = add i16 %97, 3
  %99 = trunc i16 %98 to i8
  %100 = and i8 %99, 7
  %101 = shl i8 %100, 5
  %102 = and i8 %95, 31
  %103 = or i8 %102, %101
  store i8 %103, ptr %0, !tbaa !2
  %104 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %104)
  %105 = load i8, ptr %0, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %105)
  %106 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %106)
  %107 = load i8, ptr %0, !tbaa !2
  %108 = lshr i8 %107, 3
  %109 = and i8 %108, 3
  call addrspace(1) void @N$PU1(i8 %109)
  %110 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %110)
  %111 = load i8, ptr %0, !tbaa !2
  %112 = ashr i8 %111, 5
  call addrspace(1) void @N$PI1(i8 %112)
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
