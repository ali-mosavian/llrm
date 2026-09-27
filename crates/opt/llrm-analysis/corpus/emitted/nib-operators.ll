target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @mix(i16 %0, i16 %1, i8 %2) addrspace(1) {
b1:
  %3 = alloca i16
  store i16 0, ptr %3
  store i16 %0, ptr %3, !tbaa !2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = zext i8 %2 to i16
  %6 = icmp ult i16 %5, 16
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b2, label %b3

b2:
  %9 = zext i8 %2 to i16
  %10 = shl i16 %4, %9
  store i16 %10, ptr %3, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = or i16 %11, %1
  store i16 %12, ptr %3, !tbaa !2
  %13 = load i16, ptr %3, !tbaa !2
  %14 = xor i16 %13, 21845
  store i16 %14, ptr %3, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = and i16 %15, -256
  store i16 %16, ptr %3, !tbaa !2
  %17 = load i16, ptr %3, !tbaa !2
  %18 = zext i8 %2 to i16
  %19 = icmp ult i16 %18, 16
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b4, label %b5

b3:
  call addrspace(1) void @N$ESHF()
  unreachable

b4:
  %22 = zext i8 %2 to i16
  %23 = lshr i16 %17, %22
  store i16 %23, ptr %3, !tbaa !2
  %24 = load i16, ptr %3, !tbaa !2
  %25 = xor i16 %1, -1
  %26 = and i16 %25, %0
  %27 = xor i16 %26, %1
  %28 = or i16 %24, %27
  ret i16 %28

b5:
  call addrspace(1) void @N$ESHF()
  unreachable
}

define internal i32 @shifts(i32 %0, i16 %1) addrspace(1) {
b1:
  %2 = icmp ult i16 %1, 32
  %3 = sext i1 %2 to i8
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  %5 = zext i16 %1 to i32
  %6 = ashr i32 %0, %5
  %7 = icmp ult i16 %1, 32
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b4, label %b5

b3:
  call addrspace(1) void @N$ESHF()
  unreachable

b4:
  %10 = zext i16 %1 to i32
  %11 = shl i32 %0, %10
  %12 = add i32 %6, %11
  ret i32 %12

b5:
  call addrspace(1) void @N$ESHF()
  unreachable
}

define internal i8 @logic(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca i8
  store i8 0, ptr %2
  store i8 0, ptr %3
  %4 = icmp slt i16 %0, %1
  %5 = sext i1 %4 to i8
  store i8 %5, ptr %2, !tbaa !2
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  %7 = icmp eq i16 %0, 0
  %8 = sext i1 %7 to i8
  %9 = xor i8 %8, -1
  store i8 %9, ptr %2, !tbaa !2
  br label %b3

b3:
  %10 = load i8, ptr %2, !tbaa !2
  store i8 %10, ptr %3, !tbaa !2
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b5, label %b4

b4:
  %12 = icmp eq i16 %1, 7
  %13 = sext i1 %12 to i8
  store i8 %13, ptr %3, !tbaa !2
  br label %b5

b5:
  %14 = load i8, ptr %3, !tbaa !2
  ret i8 %14
}

define internal i32 @widen(i8 %0, i8 %1, i8 %2) addrspace(1) {
b1:
  %3 = sext i8 %0 to i32
  %4 = zext i8 %1 to i32
  %5 = add i32 %3, %4
  %6 = sext i8 %2 to i32
  %7 = and i32 %6, 1
  %8 = add i32 %5, %7
  %9 = zext i8 %0 to i16
  %10 = sext i16 %9 to i32
  %11 = add i32 %8, %10
  ret i32 %11
}

define internal i16 @narrow(i32 %0) addrspace(1) {
b1:
  %1 = trunc i32 %0 to i8
  %2 = sext i8 %1 to i16
  %3 = trunc i32 %0 to i8
  %4 = zext i8 %3 to i16
  %5 = add i16 %2, %4
  ret i16 %5
}

define internal i16 @filled(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [18 x i8]
  %8 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 18, i1 false)
  store i16 0, ptr %8
  %9 = mul i16 %0, 3
  store i16 %9, ptr %8, !tbaa !2
  store i16 9, ptr %5, !tbaa !2
  store i16 9, ptr %6, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 9, ptr %3, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = load i16, ptr %3, !tbaa !2
  %12 = icmp slt i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b3, label %b5

b3:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = load i16, ptr %8, !tbaa !2
  %17 = sub i16 %15, 0
  %18 = getelementptr inbounds i16, ptr %7, i16 %17
  store i16 %16, ptr %18, !tbaa !2
  br label %b4

b4:
  %19 = load i16, ptr %4, !tbaa !2
  %20 = add i16 %19, 1
  store i16 %20, ptr %4, !tbaa !2
  br label %b2

b5:
  %21 = sub i16 4, 0
  %22 = getelementptr inbounds i16, ptr %7, i16 %21
  store i16 1, ptr %22, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 0, ptr %1, !tbaa !2
  br label %b6

b6:
  %23 = load i16, ptr %1, !tbaa !2
  %24 = icmp ult i16 %23, 9
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b7, label %b9

b7:
  %27 = load i16, ptr %2, !tbaa !2
  %28 = sub i16 %23, 0
  %29 = getelementptr inbounds i16, ptr %7, i16 %28
  %30 = load i16, ptr %29, !tbaa !2
  %31 = add i16 %27, %30
  store i16 %31, ptr %2, !tbaa !2
  br label %b8

b8:
  %32 = load i16, ptr %1, !tbaa !2
  %33 = add i16 %32, 1
  store i16 %33, ptr %1, !tbaa !2
  br label %b6

b9:
  %34 = load i16, ptr %2, !tbaa !2
  ret i16 %34
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i32
  %9 = alloca i32
  %10 = alloca i16
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i32 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i32 0, ptr %8
  store i32 0, ptr %9
  store i16 0, ptr %10
  %11 = call addrspace(1) i16 @mix(i16 4660, i16 -21555, i8 3)
  store i16 %11, ptr %10, !tbaa !2
  %12 = call addrspace(1) i32 @shifts(i32 -100000, i16 3)
  store i32 %12, ptr %9, !tbaa !2
  %13 = call addrspace(1) i32 @shifts(i32 100000, i16 5)
  store i32 %13, ptr %8, !tbaa !2
  %14 = load i16, ptr %10, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %14)
  %15 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %15)
  %16 = load i32, ptr %9, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %16)
  %17 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %17)
  %18 = load i32, ptr %8, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %18)
  call addrspace(1) void @N$PN()
  %19 = call addrspace(1) i8 @logic(i16 1, i16 2)
  %20 = sext i8 %19 to i16
  %21 = and i16 %20, 1
  store i16 %21, ptr %7, !tbaa !2
  %22 = call addrspace(1) i8 @logic(i16 0, i16 2)
  %23 = sext i8 %22 to i16
  %24 = and i16 %23, 1
  store i16 %24, ptr %6, !tbaa !2
  %25 = call addrspace(1) i8 @logic(i16 3, i16 7)
  %26 = sext i8 %25 to i16
  %27 = and i16 %26, 1
  store i16 %27, ptr %5, !tbaa !2
  %28 = call addrspace(1) i8 @logic(i16 3, i16 2)
  %29 = sext i8 %28 to i16
  %30 = and i16 %29, 1
  store i16 %30, ptr %4, !tbaa !2
  %31 = load i16, ptr %7, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %31)
  %32 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %32)
  %33 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %33)
  %34 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %34)
  %35 = load i16, ptr %5, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %35)
  %36 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %36)
  %37 = load i16, ptr %4, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %37)
  call addrspace(1) void @N$PN()
  %38 = call addrspace(1) i32 @widen(i8 -5, i8 -56, i8 -1)
  store i32 %38, ptr %3, !tbaa !2
  %39 = call addrspace(1) i16 @narrow(i32 -129)
  store i16 %39, ptr %2, !tbaa !2
  %40 = call addrspace(1) i16 @narrow(i32 70000)
  store i16 %40, ptr %1, !tbaa !2
  %41 = call addrspace(1) i16 @filled(i16 5)
  store i16 %41, ptr %0, !tbaa !2
  %42 = load i32, ptr %3, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %42)
  %43 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %43)
  %44 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %44)
  %45 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %45)
  %46 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %46)
  %47 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %47)
  %48 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %48)
  call addrspace(1) void @N$PN()
  ret i16 0
}

declare void @N$ESHF() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
