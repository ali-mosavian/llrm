target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00health \00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal void @Player.hit(ptr addrspace(1) %0, i8 %1) addrspace(1) {
b1:
  %2 = load i8, ptr addrspace(1) %0
  %3 = call addrspace(1) i8 @u8.saturating_sub(i8 %2, i8 %1)
  store i8 %3, ptr addrspace(1) %0
  ret void
}

define internal void @Player.heal(ptr addrspace(1) %0, i8 %1) addrspace(1) {
b1:
  %2 = load i8, ptr addrspace(1) %0
  %3 = call addrspace(1) i8 @u8.saturating_add(i8 %2, i8 %1)
  store i8 %3, ptr addrspace(1) %0
  ret void
}

define internal i8 @Player.award(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) {
b1:
  %3 = alloca i32
  %4 = alloca [6 x i8]
  %5 = alloca i32
  %6 = alloca [6 x i8]
  store i32 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  store i32 0, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 6, i1 false)
  %7 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @i32.checked_mul(ptr addrspace(1) %7, i32 %1, i32 %2)
  %8 = load i8, ptr %6, !tbaa !2
  %9 = icmp eq i8 %8, 0
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b4, label %b3

b3:
  ret i8 0

b4:
  %12 = getelementptr inbounds i8, ptr %6, i16 2
  %13 = load i32, ptr %12, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %6, i16 2
  %15 = load i32, ptr %14, !tbaa !2
  store i32 %15, ptr %5, !tbaa !2
  %16 = addrspacecast ptr %4 to ptr addrspace(1)
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %18 = load i32, ptr addrspace(1) %17
  %19 = load i32, ptr %5, !tbaa !2
  call addrspace(1) void @i32.checked_add(ptr addrspace(1) %16, i32 %18, i32 %19)
  %20 = load i8, ptr %4, !tbaa !2
  %21 = icmp eq i8 %20, 0
  %22 = sext i1 %21 to i8
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b7, label %b6

b6:
  ret i8 0

b7:
  %24 = getelementptr inbounds i8, ptr %4, i16 2
  %25 = load i32, ptr %24, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %4, i16 2
  %27 = load i32, ptr %26, !tbaa !2
  store i32 %27, ptr %3, !tbaa !2
  %28 = load i32, ptr %3, !tbaa !2
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %28, ptr addrspace(1) %29
  ret i8 -1
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i8
  %1 = alloca i8
  %2 = alloca i8
  %3 = alloca [6 x i8]
  store i8 0, ptr %0
  store i8 0, ptr %1
  store i8 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store i8 100, ptr %3, !tbaa !2
  %4 = getelementptr inbounds i8, ptr %3, i16 2
  store i32 2000000000, ptr %4, !tbaa !2
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Player.hit(ptr addrspace(1) %5, i8 30)
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Player.heal(ptr addrspace(1) %6, i8 -56)
  %7 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %7)
  %8 = load i8, ptr %3, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %8)
  call addrspace(1) void @N$PN()
  %9 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Player.hit(ptr addrspace(1) %9, i8 -1)
  %10 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %10)
  %11 = load i8, ptr %3, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %11)
  call addrspace(1) void @N$PN()
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  %13 = call addrspace(1) i8 @Player.award(ptr addrspace(1) %12, i32 1000, i32 100000)
  store i8 %13, ptr %2, !tbaa !2
  %14 = load i8, ptr %2, !tbaa !2
  call addrspace(1) void @N$PB(i8 %14)
  %15 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %15)
  %16 = getelementptr inbounds i8, ptr %3, i16 2
  %17 = load i32, ptr %16, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %17)
  call addrspace(1) void @N$PN()
  %18 = addrspacecast ptr %3 to ptr addrspace(1)
  %19 = call addrspace(1) i8 @Player.award(ptr addrspace(1) %18, i32 100000, i32 100000)
  store i8 %19, ptr %1, !tbaa !2
  %20 = load i8, ptr %1, !tbaa !2
  call addrspace(1) void @N$PB(i8 %20)
  %21 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %21)
  %22 = getelementptr inbounds i8, ptr %3, i16 2
  %23 = load i32, ptr %22, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %23)
  call addrspace(1) void @N$PN()
  %24 = addrspacecast ptr %3 to ptr addrspace(1)
  %25 = call addrspace(1) i8 @Player.award(ptr addrspace(1) %24, i32 100000000, i32 2)
  store i8 %25, ptr %0, !tbaa !2
  %26 = load i8, ptr %0, !tbaa !2
  call addrspace(1) void @N$PB(i8 %26)
  %27 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %27)
  %28 = getelementptr inbounds i8, ptr %3, i16 2
  %29 = load i32, ptr %28, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %29)
  call addrspace(1) void @N$PN()
  ret i16 0
}

define internal void @i32.checked_add(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) {
b1:
  %3 = alloca i32
  store i32 0, ptr %3
  %4 = add i32 %1, %2
  store i32 %4, ptr %3, !tbaa !2
  %5 = load i32, ptr %3, !tbaa !2
  %6 = xor i32 %1, %5
  %7 = load i32, ptr %3, !tbaa !2
  %8 = xor i32 %2, %7
  %9 = and i32 %6, %8
  %10 = icmp slt i32 %9, 0
  %11 = sext i1 %10 to i8
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  br label %b4

b4:
  %13 = load i32, ptr %3, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %13, ptr addrspace(1) %14
  ret void
}

define internal void @i32.checked_mul(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) {
b1:
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i32
  store i8 0, ptr %3
  store i8 0, ptr %4
  store i32 0, ptr %5
  %6 = mul i32 %1, %2
  store i32 %6, ptr %5, !tbaa !2
  %7 = icmp eq i32 %1, -1
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  %10 = icmp eq i32 %2, -2147483648
  %11 = sext i1 %10 to i8
  store i8 %11, ptr %4, !tbaa !2
  br label %b4

b3:
  %12 = icmp ne i32 %1, 0
  %13 = sext i1 %12 to i8
  store i8 %13, ptr %3, !tbaa !2
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b5, label %b6

b4:
  %15 = load i8, ptr %4, !tbaa !2
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b7, label %b8

b5:
  %17 = load i32, ptr %5, !tbaa !2
  %18 = sdiv i32 %17, %1
  %19 = srem i32 %17, %1
  %20 = icmp ne i32 %19, 0
  %21 = sext i1 %20 to i8
  %22 = xor i32 %19, %1
  %23 = icmp slt i32 %22, 0
  %24 = sext i1 %23 to i8
  %25 = and i8 %21, %24
  %26 = sext i8 %25 to i32
  %27 = and i32 %26, 1
  %28 = sub i32 %18, %27
  %29 = icmp ne i32 %28, %2
  %30 = sext i1 %29 to i8
  store i8 %30, ptr %3, !tbaa !2
  br label %b6

b6:
  %31 = load i8, ptr %3, !tbaa !2
  store i8 %31, ptr %4, !tbaa !2
  br label %b4

b7:
  store i8 1, ptr addrspace(1) %0
  ret void

b8:
  br label %b9

b9:
  %32 = load i32, ptr %5, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %32, ptr addrspace(1) %33
  ret void
}

define internal i8 @u8.saturating_add(i8 %0, i8 %1) addrspace(1) {
b1:
  %2 = alloca i8
  store i8 0, ptr %2
  %3 = zext i8 %0 to i16
  %4 = zext i8 %1 to i16
  %5 = add i16 %3, %4
  %6 = trunc i16 %5 to i8
  store i8 %6, ptr %2, !tbaa !2
  %7 = load i8, ptr %2, !tbaa !2
  %8 = zext i8 %7 to i16
  %9 = zext i8 %0 to i16
  %10 = icmp slt i16 %8, %9
  %11 = sext i1 %10 to i8
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b2, label %b3

b2:
  ret i8 -1

b3:
  br label %b4

b4:
  %13 = load i8, ptr %2, !tbaa !2
  ret i8 %13
}

define internal i8 @u8.saturating_sub(i8 %0, i8 %1) addrspace(1) {
b1:
  %2 = alloca i8
  store i8 0, ptr %2
  %3 = zext i8 %0 to i16
  %4 = zext i8 %1 to i16
  %5 = sub i16 %3, %4
  %6 = trunc i16 %5 to i8
  store i8 %6, ptr %2, !tbaa !2
  %7 = zext i8 %0 to i16
  %8 = zext i8 %1 to i16
  %9 = icmp slt i16 %7, %8
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  ret i8 0

b3:
  br label %b4

b4:
  %12 = load i8, ptr %2, !tbaa !2
  ret i8 %12
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU1(i8) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare void @N$PI4(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
