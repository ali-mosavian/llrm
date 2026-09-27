target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [14 x i8] c"\08\00\07\00\07\00health \00"
@$str2 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal void @Player.hit(ptr addrspace(1) %0, i8 %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %2 = load i8, ptr addrspace(1) %0
  %3 = zext i8 %2 to i16
  %4 = zext i8 %1 to i16
  %5 = sub i16 %3, %4
  %6 = trunc i16 %5 to i8
  %7 = icmp slt i16 %3, %4
  br i1 %7, label %9, label %8

8:
  br label %9

9:
  %10 = phi i8 [ 0, %b1 ], [ %6, %8 ]
  store i8 %10, ptr addrspace(1) %0
  ret void
}

define internal void @Player.heal(ptr addrspace(1) %0, i8 %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %2 = load i8, ptr addrspace(1) %0
  %3 = zext i8 %2 to i16
  %4 = add i16 %3, 200
  %5 = trunc i16 %4 to i8
  %6 = zext i8 %5 to i16
  %7 = icmp slt i16 %6, %3
  br i1 %7, label %9, label %8

8:
  br label %9

9:
  %10 = phi i8 [ -1, %b1 ], [ %5, %8 ]
  store i8 %10, ptr addrspace(1) %0
  ret void
}

define internal i8 @Player.award(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = mul i32 %1, %2
  %7 = icmp eq i32 %1, -1
  br i1 %7, label %8, label %11

8:
  %9 = icmp eq i32 %2, -2147483648
  %10 = sext i1 %9 to i8
  br label %14

11:
  %12 = icmp ne i32 %1, 0
  %13 = sext i1 %12 to i8
  br i1 %12, label %17, label %31

14:
  %15 = phi i8 [ %10, %8 ], [ %32, %31 ]
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %33, label %34

17:
  %18 = sdiv i32 %6, %1
  %19 = srem i32 %6, %1
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
  br label %31

31:
  %32 = phi i8 [ %13, %11 ], [ %30, %17 ]
  br label %14

33:
  store i8 1, ptr addrspace(1) %5
  br label %36

34:
  store i8 0, ptr addrspace(1) %5
  %35 = getelementptr i8, ptr addrspace(1) %5, i16 2
  store i32 %6, ptr addrspace(1) %35
  br label %36

36:
  %37 = load i8, ptr %4, !tbaa !2
  %38 = icmp eq i8 %37, 0
  br i1 %38, label %b4, label %b3

b3:
  ret i8 0

b4:
  %39 = getelementptr inbounds i8, ptr %4, i16 2
  %40 = load i32, ptr %39, !tbaa !2
  %41 = addrspacecast ptr %3 to ptr addrspace(1)
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %43 = load i32, ptr addrspace(1) %42
  %44 = add i32 %43, %40
  %45 = xor i32 %43, %44
  %46 = xor i32 %40, %44
  %47 = and i32 %45, %46
  %48 = icmp slt i32 %47, 0
  br i1 %48, label %49, label %50

49:
  store i8 1, ptr addrspace(1) %41
  br label %52

50:
  store i8 0, ptr addrspace(1) %41
  %51 = getelementptr i8, ptr addrspace(1) %41, i16 2
  store i32 %44, ptr addrspace(1) %51
  br label %52

52:
  %53 = load i8, ptr %3, !tbaa !2
  %54 = icmp eq i8 %53, 0
  br i1 %54, label %b7, label %b6

b6:
  ret i8 0

b7:
  %55 = getelementptr inbounds i8, ptr %3, i16 2
  %56 = load i32, ptr %55, !tbaa !2
  store i32 %56, ptr addrspace(1) %42
  ret i8 -1
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 6, i1 false)
  store i8 100, ptr %0, !tbaa !2
  %1 = getelementptr inbounds i8, ptr %0, i16 2
  store i32 2000000000, ptr %1, !tbaa !2
  %2 = addrspacecast ptr %0 to ptr addrspace(1)
  store i8 70, ptr addrspace(1) %2
  store i8 -1, ptr addrspace(1) %2
  %3 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %3)
  %4 = load i8, ptr %0, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %4)
  call addrspace(1) void @N$PN()
  %5 = load i8, ptr addrspace(1) %2
  %6 = zext i8 %5 to i16
  %7 = add i16 %6, -255
  %8 = trunc i16 %7 to i8
  %9 = icmp slt i16 %6, 255
  br i1 %9, label %11, label %10

10:
  br label %11

11:
  %12 = phi i8 [ 0, %b1 ], [ %8, %10 ]
  store i8 %12, ptr addrspace(1) %2
  call addrspace(1) void @N$PS(ptr %3)
  %13 = load i8, ptr %0, !tbaa !2
  call addrspace(1) void @N$PU1(i8 %13)
  call addrspace(1) void @N$PN()
  %14 = call addrspace(1) i8 @Player.award(ptr addrspace(1) %2, i32 1000, i32 100000)
  call addrspace(1) void @N$PB(i8 %14)
  %15 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %15)
  %16 = load i32, ptr %1, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %16)
  call addrspace(1) void @N$PN()
  %17 = call addrspace(1) i8 @Player.award(ptr addrspace(1) %2, i32 100000, i32 100000)
  call addrspace(1) void @N$PB(i8 %17)
  call addrspace(1) void @N$PS(ptr %15)
  %18 = load i32, ptr %1, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %18)
  call addrspace(1) void @N$PN()
  %19 = call addrspace(1) i8 @Player.award(ptr addrspace(1) %2, i32 100000000, i32 2)
  call addrspace(1) void @N$PB(i8 %19)
  call addrspace(1) void @N$PS(ptr %15)
  %20 = load i32, ptr %1, !tbaa !2
  call addrspace(1) void @N$PI4(i32 %20)
  call addrspace(1) void @N$PN()
  ret i16 0
}

define internal void @i32.checked_add(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) memory(argmem: write) willreturn {
b1:
  %3 = add i32 %1, %2
  %4 = xor i32 %1, %3
  %5 = xor i32 %2, %3
  %6 = and i32 %4, %5
  %7 = icmp slt i32 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  store i8 0, ptr addrspace(1) %0
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %3, ptr addrspace(1) %8
  ret void
}

define internal void @i32.checked_mul(ptr addrspace(1) %0, i32 %1, i32 %2) addrspace(1) memory(argmem: write) willreturn {
b1:
  %3 = mul i32 %1, %2
  %4 = icmp eq i32 %1, -1
  br i1 %4, label %b2, label %b3

b2:
  %5 = icmp eq i32 %2, -2147483648
  %6 = sext i1 %5 to i8
  br label %b4

b3:
  %7 = icmp ne i32 %1, 0
  %8 = sext i1 %7 to i8
  br i1 %7, label %b5, label %b6

b4:
  %9 = phi i8 [ %6, %b2 ], [ %24, %b6 ]
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b7, label %b8

b5:
  %11 = sdiv i32 %3, %1
  %12 = srem i32 %3, %1
  %13 = icmp ne i32 %12, 0
  %14 = sext i1 %13 to i8
  %15 = xor i32 %12, %1
  %16 = icmp slt i32 %15, 0
  %17 = sext i1 %16 to i8
  %18 = and i8 %14, %17
  %19 = sext i8 %18 to i32
  %20 = and i32 %19, 1
  %21 = sub i32 %11, %20
  %22 = icmp ne i32 %21, %2
  %23 = sext i1 %22 to i8
  br label %b6

b6:
  %24 = phi i8 [ %8, %b3 ], [ %23, %b5 ]
  br label %b4

b7:
  store i8 1, ptr addrspace(1) %0
  ret void

b8:
  store i8 0, ptr addrspace(1) %0
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 %3, ptr addrspace(1) %25
  ret void
}

define internal i8 @u8.saturating_add(i8 %0, i8 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = zext i8 %0 to i16
  %3 = zext i8 %1 to i16
  %4 = add i16 %2, %3
  %5 = trunc i16 %4 to i8
  %6 = zext i8 %5 to i16
  %7 = icmp slt i16 %6, %2
  br i1 %7, label %b2, label %b3

b2:
  ret i8 -1

b3:
  ret i8 %5
}

define internal i8 @u8.saturating_sub(i8 %0, i8 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = zext i8 %0 to i16
  %3 = zext i8 %1 to i16
  %4 = sub i16 %2, %3
  %5 = trunc i16 %4 to i8
  %6 = icmp slt i16 %2, %3
  br i1 %6, label %b2, label %b3

b2:
  ret i8 0

b3:
  ret i8 %5
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
