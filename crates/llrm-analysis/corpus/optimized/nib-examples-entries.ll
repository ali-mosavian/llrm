target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [17 x i8] c"\08\00\0A\00\0A\00VALUES.DAT\00"
@$str3 = internal constant [17 x i8] c"\08\00\0A\00\0A\00no entries\00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00first=\00"
@$str5 = internal constant [19 x i8] c"\08\00\0C\00\0C\00, remaining=\00"
@$str6 = internal constant [24 x i8] c"\08\00\11\00\11\00 positive, width=\00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00width\00"

define internal void @std.os.write(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %0, i16 %1)
  ret void
}

define internal i32 @std.io.error(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = sub i16 0, %0
  %3 = icmp eq i16 %2, 2
  %4 = sext i1 %3 to i8
  br i1 %3, label %b3, label %b2

b2:
  %5 = icmp eq i16 %2, 3
  %6 = sext i1 %5 to i8
  br label %b3

b3:
  %7 = phi i8 [ %4, %b1 ], [ %6, %b2 ]
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b4, label %b5

b4:
  store i8 0, ptr %1, !tbaa !2
  %9 = addrspacecast ptr %1 to ptr addrspace(1)
  %10 = load i32, ptr addrspace(1) %9, !tbaa !2
  ret i32 %10

b5:
  %11 = icmp eq i16 %2, 5
  br i1 %11, label %b7, label %b9

b7:
  store i8 1, ptr %1, !tbaa !2
  %12 = addrspacecast ptr %1 to ptr addrspace(1)
  %13 = load i32, ptr addrspace(1) %12, !tbaa !2
  ret i32 %13

b9:
  store i8 2, ptr %1, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %2, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %1 to ptr addrspace(1)
  %16 = load i32, ptr addrspace(1) %15, !tbaa !2
  ret i32 %16
}

define internal void @std.io.named(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %12, %b8 ]
  %6 = phi i16 [ 0, %b1 ], [ %13, %b8 ]
  %7 = icmp ult i16 %6, %2
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %4, i16 %6
  %9 = icmp ult i16 %5, 79
  br i1 %9, label %b6, label %b8

b5:
  %10 = icmp ult i16 %5, 80
  br i1 %10, label %b11, label %b12

b6:
  %11 = icmp ult i16 %5, 80
  br i1 %11, label %b9, label %b10

b8:
  %12 = phi i16 [ %5, %b3 ], [ %16, %b9 ]
  %13 = add i16 %6, 1
  br label %b2

b9:
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 %5
  %15 = load i8, ptr addrspace(1) %8
  store i8 %15, ptr addrspace(1) %14
  %16 = add i16 %5, 1
  br label %b8

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  %17 = getelementptr i8, ptr addrspace(1) %1, i16 %5
  store i8 0, ptr addrspace(1) %17
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @std.io.opened(ptr addrspace(1) %0, i16 %1) addrspace(1) memory(argmem: write) willreturn {
b1:
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  %4 = icmp slt i16 %1, 0
  br i1 %4, label %b2, label %b3

b2:
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  %5 = sub i16 0, %1
  %6 = icmp eq i16 %5, 2
  %7 = sext i1 %6 to i8
  br i1 %6, label %11, label %8

8:
  %9 = icmp eq i16 %5, 3
  %10 = sext i1 %9 to i8
  br label %11

11:
  %12 = phi i8 [ %7, %b2 ], [ %10, %8 ]
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %14, label %17

14:
  store i8 0, ptr %2
  %15 = addrspacecast ptr %2 to ptr addrspace(1)
  %16 = load i32, ptr addrspace(1) %15
  br label %26

17:
  %18 = icmp eq i16 %5, 5
  br i1 %18, label %19, label %22

19:
  store i8 1, ptr %2
  %20 = addrspacecast ptr %2 to ptr addrspace(1)
  %21 = load i32, ptr addrspace(1) %20
  br label %26

22:
  store i8 2, ptr %2
  %23 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %5, ptr %23
  %24 = addrspacecast ptr %2 to ptr addrspace(1)
  %25 = load i32, ptr addrspace(1) %24
  br label %26

26:
  %27 = phi i32 [ %16, %14 ], [ %21, %19 ], [ %25, %22 ]
  %28 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %27, ptr addrspace(1) %28, !tbaa !2
  %29 = load i16, ptr %3, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %3, i16 2
  %31 = load i16, ptr %30, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %32 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %29, ptr addrspace(1) %32
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %31, ptr addrspace(1) %33
  ret void

b3:
  store i8 0, ptr addrspace(1) %0
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %1, ptr addrspace(1) %34
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 4
  br label %b5

b5:
  %36 = phi i16 [ 0, %b3 ], [ %39, %b6 ]
  %37 = icmp slt i16 %36, 128
  br i1 %37, label %b6, label %b8

b6:
  %38 = getelementptr i8, ptr addrspace(1) %35, i16 %36
  store i8 0, ptr addrspace(1) %38
  %39 = add i16 %36, 1
  br label %b5

b8:
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 0, ptr addrspace(1) %40
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 0, ptr addrspace(1) %41
  ret void
}

define internal void @std.io.File.open(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i8 %2) addrspace(1) {
b1:
  %3 = alloca [136 x i8]
  %4 = alloca [80 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 80, i1 false)
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %8, %b3 ]
  %6 = icmp slt i16 %5, 80
  br i1 %6, label %b3, label %b5

b3:
  %7 = getelementptr inbounds i8, ptr %4, i16 %5
  store i8 0, ptr %7, !tbaa !2
  %8 = add i16 %5, 1
  br label %b2

b5:
  %9 = addrspacecast ptr %4 to ptr addrspace(1)
  %10 = load i16, ptr addrspace(1) %1
  %11 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %12 = load ptr addrspace(1), ptr addrspace(1) %11
  br label %13

13:
  %14 = phi i16 [ 0, %b5 ], [ %25, %24 ]
  %15 = phi i16 [ 0, %b5 ], [ %26, %24 ]
  %16 = icmp ult i16 %15, %10
  br i1 %16, label %17, label %20

17:
  %18 = getelementptr i8, ptr addrspace(1) %12, i16 %15
  %19 = icmp ult i16 %14, 79
  br i1 %19, label %22, label %24

20:
  %21 = icmp ult i16 %14, 80
  br i1 %21, label %32, label %47

22:
  %23 = icmp ult i16 %14, 80
  br i1 %23, label %27, label %31

24:
  %25 = phi i16 [ %14, %17 ], [ %30, %27 ]
  %26 = add i16 %15, 1
  br label %13

27:
  %28 = getelementptr i8, ptr addrspace(1) %9, i16 %14
  %29 = load i8, ptr addrspace(1) %18
  store i8 %29, ptr addrspace(1) %28
  %30 = add i16 %14, 1
  br label %24

31:
  call addrspace(1) void @N$EBND()
  unreachable

32:
  %33 = getelementptr i8, ptr addrspace(1) %9, i16 %14
  store i8 0, ptr addrspace(1) %33
  %34 = addrspacecast ptr %3 to ptr addrspace(1)
  %35 = call addrspace(1) i16 @N$OOPN(ptr addrspace(1) %9, i8 0)
  call addrspace(1) void @std.io.opened(ptr addrspace(1) %34, i16 %35)
  %36 = load i16, ptr %3, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 132
  %40 = load i16, ptr %39, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %3, i16 134
  %42 = load i16, ptr %41, !tbaa !2
  store i16 %36, ptr addrspace(1) %0
  %43 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %38, ptr addrspace(1) %43
  %44 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %45 = getelementptr inbounds i8, ptr %3, i16 4
  %46 = addrspacecast ptr %45 to ptr addrspace(1)
  br label %b6

47:
  call addrspace(1) void @N$EBND()
  unreachable

b6:
  %48 = phi i16 [ 0, %32 ], [ %54, %b7 ]
  %49 = icmp slt i16 %48, 64
  br i1 %49, label %b7, label %b9

b7:
  %50 = shl i16 %48, 1
  %51 = getelementptr i8, ptr addrspace(1) %44, i16 %50
  %52 = getelementptr i8, ptr addrspace(1) %46, i16 %50
  %53 = load i16, ptr addrspace(1) %52
  store i16 %53, ptr addrspace(1) %51
  %54 = add i16 %48, 1
  br label %b6

b9:
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 %40, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %42, ptr addrspace(1) %56
  ret void
}

define internal void @std.io.File.create(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [136 x i8]
  %3 = alloca [80 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 80, i1 false)
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %7, %b3 ]
  %5 = icmp slt i16 %4, 80
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr inbounds i8, ptr %3, i16 %4
  store i8 0, ptr %6, !tbaa !2
  %7 = add i16 %4, 1
  br label %b2

b5:
  %8 = addrspacecast ptr %3 to ptr addrspace(1)
  %9 = load i16, ptr addrspace(1) %1
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  br label %12

12:
  %13 = phi i16 [ 0, %b5 ], [ %24, %23 ]
  %14 = phi i16 [ 0, %b5 ], [ %25, %23 ]
  %15 = icmp ult i16 %14, %9
  br i1 %15, label %16, label %19

16:
  %17 = getelementptr i8, ptr addrspace(1) %11, i16 %14
  %18 = icmp ult i16 %13, 79
  br i1 %18, label %21, label %23

19:
  %20 = icmp ult i16 %13, 80
  br i1 %20, label %31, label %46

21:
  %22 = icmp ult i16 %13, 80
  br i1 %22, label %26, label %30

23:
  %24 = phi i16 [ %13, %16 ], [ %29, %26 ]
  %25 = add i16 %14, 1
  br label %12

26:
  %27 = getelementptr i8, ptr addrspace(1) %8, i16 %13
  %28 = load i8, ptr addrspace(1) %17
  store i8 %28, ptr addrspace(1) %27
  %29 = add i16 %13, 1
  br label %23

30:
  call addrspace(1) void @N$EBND()
  unreachable

31:
  %32 = getelementptr i8, ptr addrspace(1) %8, i16 %13
  store i8 0, ptr addrspace(1) %32
  %33 = addrspacecast ptr %2 to ptr addrspace(1)
  %34 = call addrspace(1) i16 @N$OCRE(ptr addrspace(1) %8)
  call addrspace(1) void @std.io.opened(ptr addrspace(1) %33, i16 %34)
  %35 = load i16, ptr %2, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %2, i16 2
  %37 = load i16, ptr %36, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %2, i16 132
  %39 = load i16, ptr %38, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %2, i16 134
  %41 = load i16, ptr %40, !tbaa !2
  store i16 %35, ptr addrspace(1) %0
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %37, ptr addrspace(1) %42
  %43 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %44 = getelementptr inbounds i8, ptr %2, i16 4
  %45 = addrspacecast ptr %44 to ptr addrspace(1)
  br label %b6

46:
  call addrspace(1) void @N$EBND()
  unreachable

b6:
  %47 = phi i16 [ 0, %31 ], [ %53, %b7 ]
  %48 = icmp slt i16 %47, 64
  br i1 %48, label %b7, label %b9

b7:
  %49 = shl i16 %47, 1
  %50 = getelementptr i8, ptr addrspace(1) %43, i16 %49
  %51 = getelementptr i8, ptr addrspace(1) %45, i16 %49
  %52 = load i16, ptr addrspace(1) %51
  store i16 %52, ptr addrspace(1) %50
  %53 = add i16 %47, 1
  br label %b6

b9:
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 %39, ptr addrspace(1) %54
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %41, ptr addrspace(1) %55
  ret void
}

define internal void @std.io.File.write_raw(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca [4 x i8]
  %5 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  %6 = load i16, ptr addrspace(1) %1
  %7 = call addrspace(1) i16 @N$OWRI(i16 %6, ptr addrspace(1) %2, i16 %3)
  %8 = icmp slt i16 %7, 0
  br i1 %8, label %b2, label %b3

b2:
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  %9 = sub i16 0, %7
  %10 = icmp eq i16 %9, 2
  %11 = sext i1 %10 to i8
  br i1 %10, label %15, label %12

12:
  %13 = icmp eq i16 %9, 3
  %14 = sext i1 %13 to i8
  br label %15

15:
  %16 = phi i8 [ %11, %b2 ], [ %14, %12 ]
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %18, label %21

18:
  store i8 0, ptr %4
  %19 = addrspacecast ptr %4 to ptr addrspace(1)
  %20 = load i32, ptr addrspace(1) %19
  br label %30

21:
  %22 = icmp eq i16 %9, 5
  br i1 %22, label %23, label %26

23:
  store i8 1, ptr %4
  %24 = addrspacecast ptr %4 to ptr addrspace(1)
  %25 = load i32, ptr addrspace(1) %24
  br label %30

26:
  store i8 2, ptr %4
  %27 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %9, ptr %27
  %28 = addrspacecast ptr %4 to ptr addrspace(1)
  %29 = load i32, ptr addrspace(1) %28
  br label %30

30:
  %31 = phi i32 [ %20, %18 ], [ %25, %23 ], [ %29, %26 ]
  %32 = addrspacecast ptr %5 to ptr addrspace(1)
  store i32 %31, ptr addrspace(1) %32, !tbaa !2
  %33 = load i16, ptr %5, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %5, i16 2
  %35 = load i16, ptr %34, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %36 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %33, ptr addrspace(1) %36
  %37 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %35, ptr addrspace(1) %37
  ret void

b3:
  store i8 0, ptr addrspace(1) %0
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %7, ptr addrspace(1) %38
  ret void
}

define internal void @std.io.File.write(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  %4 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  %7 = load i16, ptr addrspace(1) %2
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %6, ptr addrspace(1) %1, ptr addrspace(1) %5, i16 %7)
  %8 = load i16, ptr %3, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %3, i16 2
  %10 = load i16, ptr %9, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  %12 = load i16, ptr %11, !tbaa !2
  store i16 %8, ptr addrspace(1) %0
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %10, ptr addrspace(1) %13
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %12, ptr addrspace(1) %14
  ret void
}

define internal void @std.io.File.read_raw(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca [4 x i8]
  %5 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %7 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 132
  br label %b2

b2:
  %9 = phi i16 [ 0, %b1 ], [ %18, %b3 ]
  %10 = icmp ult i16 %9, %3
  %11 = sext i1 %10 to i8
  br i1 %10, label %b5, label %b6

b3:
  %12 = getelementptr i8, ptr addrspace(1) %2, i16 %9
  %13 = load i16, ptr addrspace(1) %7
  %14 = getelementptr i8, ptr addrspace(1) %6, i16 %13
  %15 = load i8, ptr addrspace(1) %14
  store i8 %15, ptr addrspace(1) %12
  %16 = load i16, ptr addrspace(1) %7
  %17 = add i16 %16, 1
  store i16 %17, ptr addrspace(1) %7
  %18 = add i16 %9, 1
  br label %b2

b4:
  br i1 %10, label %b7, label %b9

b5:
  %19 = load i16, ptr addrspace(1) %7
  %20 = load i16, ptr addrspace(1) %8
  %21 = icmp ult i16 %19, %20
  %22 = sext i1 %21 to i8
  br label %b6

b6:
  %23 = phi i8 [ %11, %b2 ], [ %22, %b5 ]
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b3, label %b4

b7:
  %25 = load i16, ptr addrspace(1) %1
  %26 = getelementptr i8, ptr addrspace(1) %2, i16 %9
  %27 = sub i16 %3, %9
  %28 = call addrspace(1) i16 @N$OREA(i16 %25, ptr addrspace(1) %26, i16 %27)
  %29 = icmp slt i16 %28, 0
  br i1 %29, label %b10, label %b11

b9:
  %30 = phi i16 [ %9, %b4 ], [ %61, %b11 ]
  store i8 0, ptr addrspace(1) %0
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %30, ptr addrspace(1) %31
  ret void

b10:
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  %32 = sub i16 0, %28
  %33 = icmp eq i16 %32, 2
  %34 = sext i1 %33 to i8
  br i1 %33, label %38, label %35

35:
  %36 = icmp eq i16 %32, 3
  %37 = sext i1 %36 to i8
  br label %38

38:
  %39 = phi i8 [ %34, %b10 ], [ %37, %35 ]
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %41, label %44

41:
  store i8 0, ptr %4
  %42 = addrspacecast ptr %4 to ptr addrspace(1)
  %43 = load i32, ptr addrspace(1) %42
  br label %53

44:
  %45 = icmp eq i16 %32, 5
  br i1 %45, label %46, label %49

46:
  store i8 1, ptr %4
  %47 = addrspacecast ptr %4 to ptr addrspace(1)
  %48 = load i32, ptr addrspace(1) %47
  br label %53

49:
  store i8 2, ptr %4
  %50 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %32, ptr %50
  %51 = addrspacecast ptr %4 to ptr addrspace(1)
  %52 = load i32, ptr addrspace(1) %51
  br label %53

53:
  %54 = phi i32 [ %43, %41 ], [ %48, %46 ], [ %52, %49 ]
  %55 = addrspacecast ptr %5 to ptr addrspace(1)
  store i32 %54, ptr addrspace(1) %55, !tbaa !2
  %56 = load i16, ptr %5, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %5, i16 2
  %58 = load i16, ptr %57, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %59 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %56, ptr addrspace(1) %59
  %60 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %58, ptr addrspace(1) %60
  ret void

b11:
  %61 = add i16 %9, %28
  br label %b9
}

define internal void @std.io.File.read_line(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca [4 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  %5 = getelementptr i8, ptr @$str1, i16 6
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %7 = getelementptr i8, ptr addrspace(1) %1, i16 132
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 2
  br label %b2

b2:
  %9 = phi ptr [ %5, %b1 ], [ %72, %b24 ]
  %10 = phi i8 [ 0, %b1 ], [ -1, %b24 ]
  %11 = load i16, ptr addrspace(1) %6
  %12 = load i16, ptr addrspace(1) %7
  %13 = icmp eq i16 %11, %12
  br i1 %13, label %b5, label %b7

b5:
  %14 = load i16, ptr addrspace(1) %1
  %15 = call addrspace(1) i16 @N$OREA(i16 %14, ptr addrspace(1) %8, i16 128)
  %16 = icmp slt i16 %15, 0
  br i1 %16, label %b8, label %b9

b7:
  %17 = load i16, ptr addrspace(1) %6
  %18 = icmp ult i16 %17, 128
  br i1 %18, label %b17, label %b18

b8:
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  %19 = sub i16 0, %15
  %20 = icmp eq i16 %19, 2
  %21 = sext i1 %20 to i8
  br i1 %20, label %25, label %22

22:
  %23 = icmp eq i16 %19, 3
  %24 = sext i1 %23 to i8
  br label %25

25:
  %26 = phi i8 [ %21, %b8 ], [ %24, %22 ]
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %28, label %31

28:
  store i8 0, ptr %2
  %29 = addrspacecast ptr %2 to ptr addrspace(1)
  %30 = load i32, ptr addrspace(1) %29
  br label %40

31:
  %32 = icmp eq i16 %19, 5
  br i1 %32, label %33, label %36

33:
  store i8 1, ptr %2
  %34 = addrspacecast ptr %2 to ptr addrspace(1)
  %35 = load i32, ptr addrspace(1) %34
  br label %40

36:
  store i8 2, ptr %2
  %37 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %19, ptr %37
  %38 = addrspacecast ptr %2 to ptr addrspace(1)
  %39 = load i32, ptr addrspace(1) %38
  br label %40

40:
  %41 = phi i32 [ %30, %28 ], [ %35, %33 ], [ %39, %36 ]
  %42 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %41, ptr addrspace(1) %42, !tbaa !2
  %43 = load i16, ptr %4, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %4, i16 2
  %45 = load i16, ptr %44, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %46 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %43, ptr addrspace(1) %46
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %45, ptr addrspace(1) %47
  call addrspace(1) void @N$BDRP(ptr %9)
  ret void

b9:
  %48 = icmp eq i16 %15, 0
  br i1 %48, label %b11, label %b13

b11:
  %49 = icmp ne i8 %10, 0
  br i1 %49, label %b14, label %b15

b13:
  store i16 0, ptr addrspace(1) %6
  store i16 %15, ptr addrspace(1) %7
  br label %b7

b14:
  store i8 0, ptr %3, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %3, i16 2
  store ptr %9, ptr %50, !tbaa !2
  br label %b16

b15:
  store i8 1, ptr %3, !tbaa !2
  br label %b16

b16:
  %51 = phi ptr [ null, %b14 ], [ %9, %b15 ]
  %52 = load i16, ptr %3, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %3, i16 2
  %54 = load i16, ptr %53, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %52, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %54, ptr addrspace(1) %56
  call addrspace(1) void @N$BDRP(ptr %51)
  ret void

b17:
  %57 = getelementptr i8, ptr addrspace(1) %8, i16 %17
  %58 = load i8, ptr addrspace(1) %57
  %59 = add i16 %17, 1
  store i16 %59, ptr addrspace(1) %6
  %60 = zext i8 %58 to i16
  %61 = icmp eq i16 %60, 10
  br i1 %61, label %b19, label %b20

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  store i8 0, ptr addrspace(1) %0
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %62
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %9, ptr addrspace(1) %63
  call addrspace(1) void @N$BDRP(ptr null)
  ret void

b20:
  %64 = icmp ne i16 %60, 13
  br i1 %64, label %b22, label %b24

b22:
  %65 = getelementptr i8, ptr %9, i16 -4
  %66 = load i16, ptr %65
  %67 = call addrspace(1) ptr @N$BGRW(ptr %9, i16 1, i16 1)
  %68 = getelementptr i8, ptr %67, i16 %66
  store i8 %58, ptr %68
  %69 = getelementptr i8, ptr %67, i16 -4
  %70 = load i16, ptr %69
  %71 = getelementptr i8, ptr %67, i16 %70
  store i8 0, ptr %71
  br label %b24

b24:
  %72 = phi ptr [ %67, %b22 ], [ %9, %b20 ]
  br label %b2
}

define internal void @std.io.File.drop(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = call addrspace(1) i16 @N$OCLO(i16 %1)
  ret void
}

define internal void @parse(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  %4 = load i16, ptr addrspace(1) %1
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  br label %b2

b2:
  %7 = phi i16 [ 0, %b1 ], [ %10, %b3 ]
  %8 = icmp ult i16 %7, %4
  %9 = sext i1 %8 to i8
  br i1 %8, label %b5, label %b6

b3:
  %10 = add i16 %7, 1
  br label %b2

b4:
  %11 = add i16 %7, 1
  %12 = icmp ult i16 %11, %4
  %13 = sext i1 %12 to i8
  br i1 %12, label %b11, label %b10

b5:
  %14 = getelementptr i8, ptr addrspace(1) %6, i16 %7
  %15 = load i8, ptr addrspace(1) %14
  %16 = icmp ne i8 %15, 61
  %17 = sext i1 %16 to i8
  br label %b6

b6:
  %18 = phi i8 [ %9, %b2 ], [ %17, %b5 ]
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b3, label %b4

b10:
  %20 = phi i8 [ %13, %b4 ], [ %25, %b11 ]
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b13, label %b15

b11:
  %22 = getelementptr i8, ptr addrspace(1) %6, i16 %11
  %23 = load i8, ptr addrspace(1) %22
  %24 = icmp eq i8 %23, 45
  %25 = sext i1 %24 to i8
  br label %b10

b13:
  %26 = add i16 %7, 2
  br label %b15

b15:
  %27 = phi i16 [ %26, %b13 ], [ %11, %b10 ]
  %28 = icmp eq i16 %7, 0
  %29 = sext i1 %28 to i8
  br i1 %28, label %b17, label %b16

b16:
  %30 = icmp uge i16 %27, %4
  %31 = sext i1 %30 to i8
  br label %b17

b17:
  %32 = phi i8 [ %29, %b15 ], [ %31, %b16 ]
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %b18, label %b19

b18:
  store i8 1, ptr addrspace(1) %0
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %34
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %35
  ret void

b19:
  br label %b21

b21:
  %36 = phi i16 [ %27, %b19 ], [ %54, %b30 ]
  %37 = phi i16 [ 0, %b19 ], [ %53, %b30 ]
  %38 = icmp ult i16 %36, %4
  br i1 %38, label %b22, label %b23

b22:
  %39 = getelementptr i8, ptr addrspace(1) %6, i16 %36
  %40 = load i8, ptr addrspace(1) %39
  %41 = icmp ult i8 %40, 48
  %42 = sext i1 %41 to i8
  br i1 %41, label %b27, label %b26

b23:
  %43 = icmp ule i16 %7, %4
  br i1 %43, label %b31, label %b32

b26:
  %44 = icmp ugt i8 %40, 57
  %45 = sext i1 %44 to i8
  br label %b27

b27:
  %46 = phi i8 [ %42, %b22 ], [ %45, %b26 ]
  %47 = icmp ne i8 %46, 0
  br i1 %47, label %b28, label %b30

b28:
  store i8 1, ptr addrspace(1) %0
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %48
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %49
  ret void

b30:
  %50 = mul i16 %37, 10
  %51 = zext i8 %40 to i16
  %52 = add i16 %51, -48
  %53 = add i16 %50, %52
  %54 = add i16 %36, 1
  br label %b21

b31:
  store i16 %7, ptr %3, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %7, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %6, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %3 to ptr addrspace(1)
  %58 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %57)
  br i1 %21, label %b35, label %b37

b32:
  call addrspace(1) void @N$EBND()
  unreachable

b35:
  %59 = sub i16 0, %37
  br label %b37

b37:
  %60 = phi i16 [ %59, %b35 ], [ %37, %b31 ]
  store i8 0, ptr addrspace(1) %0
  %61 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %58, ptr addrspace(1) %61
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %60, ptr addrspace(1) %62
  ret void
}

define internal void @load_entries(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca [6 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [134 x i8]
  %6 = alloca [134 x i8]
  %7 = alloca [136 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 134, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 134, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 136, i1 false)
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.open(ptr addrspace(1) %8, ptr addrspace(1) %1, i8 0)
  %9 = load i8, ptr %7, !tbaa !2
  %10 = icmp eq i8 %9, 0
  br i1 %10, label %b4, label %b3

b3:
  %11 = icmp eq i8 %9, 1
  br i1 %11, label %b59, label %b58

b4:
  %12 = getelementptr inbounds i8, ptr %7, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %7, i16 132
  %15 = load i16, ptr %14, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %7, i16 134
  %17 = load i16, ptr %16, !tbaa !2
  store i16 %13, ptr %6, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %6, i16 2
  %19 = addrspacecast ptr %18 to ptr addrspace(1)
  %20 = getelementptr inbounds i8, ptr %7, i16 4
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  br label %b5

b5:
  %22 = phi i16 [ 0, %b4 ], [ %27, %b6 ]
  %23 = icmp slt i16 %22, 128
  br i1 %23, label %b6, label %b8

b6:
  %24 = getelementptr i8, ptr addrspace(1) %19, i16 %22
  %25 = getelementptr i8, ptr addrspace(1) %21, i16 %22
  %26 = load i8, ptr addrspace(1) %25
  store i8 %26, ptr addrspace(1) %24
  %27 = add i16 %22, 1
  br label %b5

b8:
  %28 = getelementptr inbounds i8, ptr %6, i16 130
  store i16 %15, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %6, i16 132
  store i16 %17, ptr %29, !tbaa !2
  %30 = load i16, ptr %6, !tbaa !2
  store i16 %30, ptr %5, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %5, i16 2
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  br label %b9

b9:
  %33 = phi i16 [ 0, %b8 ], [ %38, %b10 ]
  %34 = icmp slt i16 %33, 128
  br i1 %34, label %b10, label %b12

b10:
  %35 = getelementptr i8, ptr addrspace(1) %32, i16 %33
  %36 = getelementptr i8, ptr addrspace(1) %19, i16 %33
  %37 = load i8, ptr addrspace(1) %36
  store i8 %37, ptr addrspace(1) %35
  %38 = add i16 %33, 1
  br label %b9

b12:
  %39 = getelementptr inbounds i8, ptr %5, i16 130
  store i16 %15, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %5, i16 132
  store i16 %17, ptr %40, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b13

b13:
  %41 = phi i16 [ 0, %b12 ], [ %44, %b14 ]
  %42 = icmp slt i16 %41, 128
  br i1 %42, label %b14, label %b16

b14:
  %43 = getelementptr i8, ptr addrspace(1) %19, i16 %41
  store i8 0, ptr addrspace(1) %43
  %44 = add i16 %41, 1
  br label %b13

b16:
  store i16 0, ptr %28, !tbaa !2
  store i16 0, ptr %29, !tbaa !2
  %45 = getelementptr i8, ptr @$str1, i16 6
  %46 = addrspacecast ptr %5 to ptr addrspace(1)
  %47 = addrspacecast ptr %4 to ptr addrspace(1)
  %48 = getelementptr inbounds i8, ptr %4, i16 2
  %49 = getelementptr inbounds i8, ptr %4, i16 4
  %50 = addrspacecast ptr %3 to ptr addrspace(1)
  %51 = getelementptr inbounds i8, ptr %2, i16 2
  %52 = getelementptr inbounds i8, ptr %2, i16 4
  %53 = addrspacecast ptr %2 to ptr addrspace(1)
  %54 = getelementptr inbounds i8, ptr %3, i16 2
  %55 = getelementptr inbounds i8, ptr %3, i16 4
  br label %b18

b18:
  %56 = phi ptr [ %45, %b16 ], [ %75, %b28 ]
  %57 = phi i16 [ 0, %b16 ], [ %65, %b28 ]
  call addrspace(1) void @std.io.File.read_line(ptr addrspace(1) %47, ptr addrspace(1) %46)
  %58 = load i8, ptr %4, !tbaa !2
  %59 = icmp eq i8 %58, 0
  br i1 %59, label %b23, label %b22

b22:
  %60 = load i8, ptr %4, !tbaa !2
  %61 = icmp eq i8 %60, 0
  br i1 %61, label %b42, label %b41

b23:
  %62 = load i8, ptr %48, !tbaa !2
  %63 = icmp eq i8 %62, 0
  br i1 %63, label %b24, label %b22

b24:
  %64 = load ptr, ptr %49, !tbaa !2
  %65 = add i16 %57, 1
  %66 = getelementptr i8, ptr %64, i16 -4
  %67 = load i16, ptr %66
  %68 = icmp eq i16 %67, 0
  %69 = sext i1 %68 to i8
  %70 = xor i8 %69, -1
  %71 = icmp ne i8 %70, 0
  br i1 %71, label %b26, label %b28

b26:
  %72 = addrspacecast ptr %64 to ptr addrspace(1)
  store i16 %67, ptr %2, !tbaa !2
  store i16 %67, ptr %51, !tbaa !2
  store ptr addrspace(1) %72, ptr %52, !tbaa !2
  call addrspace(1) void @parse(ptr addrspace(1) %50, ptr addrspace(1) %53, i16 %65)
  %73 = load i8, ptr %3, !tbaa !2
  %74 = icmp eq i8 %73, 1
  br i1 %74, label %b29, label %b30

b28:
  %75 = phi ptr [ %56, %b24 ], [ %85, %b30 ]
  call addrspace(1) void @N$BDRP(ptr %64)
  call addrspace(1) void @N$BDRP(ptr null)
  br label %b18

b29:
  %76 = load i16, ptr %54, !tbaa !2
  %77 = load i16, ptr %55, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %78 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %76, ptr addrspace(1) %78
  %79 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %77, ptr addrspace(1) %79
  call addrspace(1) void @N$BDRP(ptr %64)
  call addrspace(1) void @N$BDRP(ptr null)
  %80 = icmp ne ptr %56, null
  br i1 %80, label %b32, label %b31

b30:
  %81 = load ptr, ptr %54, !tbaa !2
  %82 = load i16, ptr %55, !tbaa !2
  %83 = getelementptr i8, ptr %56, i16 -4
  %84 = load i16, ptr %83
  %85 = call addrspace(1) ptr @N$BGRW(ptr %56, i16 1, i16 4)
  %86 = shl i16 %84, 2
  %87 = getelementptr i8, ptr %85, i16 %86
  store ptr %81, ptr %87
  %88 = getelementptr i8, ptr %87, i16 2
  store i16 %82, ptr %88
  call addrspace(1) void @N$BDRP(ptr null)
  br label %b28

b31:
  call addrspace(1) void @N$BDRP(ptr %56)
  %89 = load i16, ptr addrspace(1) %46
  %90 = call addrspace(1) i16 @N$OCLO(i16 %89)
  ret void

b32:
  %91 = getelementptr i8, ptr %56, i16 -4
  %92 = load i16, ptr %91
  br label %b33

b33:
  %93 = phi i16 [ 0, %b32 ], [ %98, %b35 ]
  %94 = icmp ult i16 %93, %92
  br i1 %94, label %b35, label %b31

b35:
  %95 = shl i16 %93, 2
  %96 = getelementptr i8, ptr %56, i16 %95
  %97 = load ptr, ptr %96
  call addrspace(1) void @N$BDRP(ptr %97)
  %98 = add i16 %93, 1
  br label %b33

b41:
  store i8 0, ptr addrspace(1) %0
  %99 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %56, ptr addrspace(1) %99
  call addrspace(1) void @N$BDRP(ptr null)
  %100 = load i16, ptr addrspace(1) %46
  %101 = call addrspace(1) i16 @N$OCLO(i16 %100)
  ret void

b42:
  %102 = load i8, ptr %48, !tbaa !2
  %103 = icmp eq i8 %102, 0
  br i1 %103, label %b44, label %b41

b44:
  %104 = load ptr, ptr %49, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %104)
  br label %b41

b58:
  store i8 1, ptr addrspace(1) %0
  %105 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 1, ptr addrspace(1) %105
  ret void

b59:
  %106 = getelementptr inbounds i8, ptr %7, i16 2
  %107 = load i8, ptr %106, !tbaa !2
  %108 = icmp eq i8 %107, 0
  br i1 %108, label %b60, label %b58

b60:
  store i8 1, ptr addrspace(1) %0
  %109 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %109
  ret void
}

define internal void @$main(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 6, i1 false)
  %7 = addrspacecast ptr %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr @$str2, i16 6
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 %10, ptr %5, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %10, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %11, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @load_entries(ptr addrspace(1) %7, ptr addrspace(1) %14)
  %15 = load i8, ptr %6, !tbaa !2
  %16 = icmp eq i8 %15, 1
  br i1 %16, label %b2, label %b3

b2:
  %17 = getelementptr inbounds i8, ptr %6, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %6, i16 4
  %20 = load i16, ptr %19, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %18, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %20, ptr addrspace(1) %22
  ret void

b3:
  %23 = getelementptr inbounds i8, ptr %6, i16 2
  %24 = load ptr, ptr %23, !tbaa !2
  %25 = getelementptr i8, ptr @$str1, i16 6
  %26 = getelementptr i8, ptr %24, i16 -4
  %27 = load i16, ptr %26
  %28 = getelementptr inbounds i8, ptr %4, i16 2
  %29 = getelementptr inbounds i8, ptr %4, i16 4
  %30 = addrspacecast ptr %4 to ptr addrspace(1)
  %31 = getelementptr inbounds i8, ptr %3, i16 2
  %32 = getelementptr inbounds i8, ptr %3, i16 4
  %33 = addrspacecast ptr %3 to ptr addrspace(1)
  br label %b4

b4:
  %34 = phi ptr [ %25, %b3 ], [ %67, %b10 ]
  %35 = phi i16 [ 0, %b3 ], [ %68, %b10 ]
  %36 = icmp ult i16 %35, %27
  br i1 %36, label %b5, label %b7

b5:
  %37 = shl i16 %35, 2
  %38 = getelementptr i8, ptr %24, i16 %37
  %39 = getelementptr i8, ptr %38, i16 2
  %40 = load i16, ptr %39
  %41 = icmp sgt i16 %40, 0
  br i1 %41, label %b8, label %b10

b7:
  %42 = load i16, ptr %26
  %43 = addrspacecast ptr %24 to ptr addrspace(1)
  %44 = icmp eq i16 %42, 0
  br i1 %44, label %b41, label %b40

b8:
  %45 = call addrspace(1) ptr @N$DRES(ptr %34, i16 6)
  %46 = load ptr, ptr %38
  %47 = call addrspace(1) ptr @N$BCLN(ptr %46, i16 1)
  %48 = getelementptr i8, ptr %47, i16 -4
  %49 = load i16, ptr %48
  %50 = addrspacecast ptr %47 to ptr addrspace(1)
  br label %51

51:
  %52 = phi i16 [ 5381, %b8 ], [ %60, %55 ]
  %53 = phi i16 [ 0, %b8 ], [ %61, %55 ]
  %54 = icmp ult i16 %53, %49
  br i1 %54, label %55, label %62

55:
  %56 = getelementptr i8, ptr addrspace(1) %50, i16 %53
  %57 = mul i16 %52, 33
  %58 = load i8, ptr addrspace(1) %56
  %59 = zext i8 %58 to i16
  %60 = xor i16 %57, %59
  %61 = add i16 %53, 1
  br label %51

62:
  %63 = or i16 %52, 1
  %64 = getelementptr i8, ptr %45, i16 -4
  %65 = load i16, ptr %64
  %66 = icmp ne i16 %65, 0
  br i1 %66, label %b11, label %b13

b10:
  %67 = phi ptr [ %34, %b5 ], [ %45, %b37 ]
  %68 = add i16 %35, 1
  br label %b4

b11:
  %69 = add i16 %65, -1
  %70 = and i16 %63, %69
  br label %b14

b13:
  %71 = phi i16 [ 0, %62 ], [ %75, %b16 ]
  %72 = phi i8 [ 0, %62 ], [ %81, %b16 ]
  %73 = xor i8 %72, -1
  %74 = icmp ne i8 %73, 0
  br i1 %74, label %b28, label %b30

b14:
  %75 = phi i16 [ %70, %b11 ], [ %98, %b26 ]
  %76 = load i16, ptr %64
  %77 = icmp ult i16 %75, %76
  br i1 %77, label %b17, label %b18

b15:
  %78 = load i16, ptr %83
  %79 = icmp eq i16 %78, %63
  %80 = sext i1 %79 to i8
  br i1 %79, label %b23, label %b22

b16:
  %81 = phi i8 [ 0, %b17 ], [ -1, %b25 ]
  br label %b13

b17:
  %82 = mul i16 %75, 6
  %83 = getelementptr i8, ptr %45, i16 %82
  %84 = load i16, ptr %83
  %85 = icmp ne i16 %84, 0
  br i1 %85, label %b15, label %b16

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b22:
  %86 = phi i8 [ %80, %b15 ], [ %96, %b23 ]
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b25, label %b26

b23:
  %88 = getelementptr i8, ptr %83, i16 2
  %89 = load ptr, ptr %88
  %90 = getelementptr i8, ptr %89, i16 -4
  %91 = load i16, ptr %90
  %92 = addrspacecast ptr %89 to ptr addrspace(1)
  store i16 %91, ptr %4, !tbaa !2
  store i16 %91, ptr %28, !tbaa !2
  store ptr addrspace(1) %92, ptr %29, !tbaa !2
  %93 = load i16, ptr %48
  store i16 %93, ptr %3, !tbaa !2
  store i16 %93, ptr %31, !tbaa !2
  store ptr addrspace(1) %50, ptr %32, !tbaa !2
  %94 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %30, ptr addrspace(1) %33)
  %95 = icmp eq i8 %94, 0
  %96 = sext i1 %95 to i8
  br label %b22

b25:
  br label %b16

b26:
  %97 = add i16 %75, 1
  %98 = and i16 %97, %69
  br label %b14

b28:
  %99 = load i16, ptr %64
  %100 = icmp ult i16 %71, %99
  br i1 %100, label %b31, label %b32

b30:
  %101 = phi ptr [ %47, %b13 ], [ null, %b33 ]
  %102 = icmp ne i8 %72, 0
  br i1 %102, label %b36, label %b35

b31:
  %103 = mul i16 %71, 6
  %104 = getelementptr i8, ptr %45, i16 %103
  store i16 %63, ptr %104
  %105 = load i16, ptr %64
  %106 = icmp ult i16 %71, %105
  br i1 %106, label %b33, label %b34

b32:
  call addrspace(1) void @N$EBND()
  unreachable

b33:
  %107 = getelementptr i8, ptr %104, i16 2
  %108 = load ptr, ptr %107
  call addrspace(1) void @N$BDRP(ptr %108)
  store ptr %47, ptr %107
  br label %b30

b34:
  call addrspace(1) void @N$EBND()
  unreachable

b35:
  %109 = getelementptr i8, ptr %45, i16 -2
  %110 = load i16, ptr %109
  %111 = add i16 %110, 1
  store i16 %111, ptr %109
  br label %b36

b36:
  %112 = load i16, ptr %64
  %113 = icmp ult i16 %71, %112
  br i1 %113, label %b37, label %b38

b37:
  %114 = mul i16 %71, 6
  %115 = getelementptr i8, ptr %45, i16 %114
  %116 = load i16, ptr %39
  %117 = getelementptr i8, ptr %115, i16 4
  store i16 %116, ptr %117
  call addrspace(1) void @N$BDRP(ptr %101)
  br label %b10

b38:
  call addrspace(1) void @N$EBND()
  unreachable

b39:
  %118 = getelementptr i8, ptr %34, i16 -2
  %119 = load i16, ptr %118
  call addrspace(1) void @N$PU2(i16 %119)
  %120 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %120)
  %121 = getelementptr i8, ptr @$str7, i16 6
  %122 = getelementptr i8, ptr %121, i16 -4
  %123 = load i16, ptr %122
  %124 = addrspacecast ptr %121 to ptr addrspace(1)
  br label %125

125:
  %126 = phi i16 [ 5381, %b39 ], [ %134, %129 ]
  %127 = phi i16 [ 0, %b39 ], [ %135, %129 ]
  %128 = icmp ult i16 %127, %123
  br i1 %128, label %129, label %136

129:
  %130 = getelementptr i8, ptr addrspace(1) %124, i16 %127
  %131 = mul i16 %126, 33
  %132 = load i8, ptr addrspace(1) %130
  %133 = zext i8 %132 to i16
  %134 = xor i16 %131, %133
  %135 = add i16 %127, 1
  br label %125

136:
  %137 = or i16 %126, 1
  %138 = getelementptr i8, ptr %34, i16 -4
  %139 = load i16, ptr %138
  %140 = icmp ne i16 %139, 0
  br i1 %140, label %b43, label %b45

b40:
  %141 = getelementptr i8, ptr addrspace(1) %43, i16 0
  %142 = add i16 %42, -1
  %143 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %143)
  %144 = load ptr, ptr addrspace(1) %141
  call addrspace(1) void @N$PS(ptr %144)
  %145 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %145)
  call addrspace(1) void @N$PU2(i16 %142)
  call addrspace(1) void @N$PN()
  br label %b39

b41:
  %146 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %146)
  call addrspace(1) void @N$PN()
  br label %b39

b43:
  %147 = add i16 %139, -1
  %148 = and i16 %137, %147
  %149 = getelementptr inbounds i8, ptr %2, i16 2
  %150 = getelementptr inbounds i8, ptr %2, i16 4
  %151 = addrspacecast ptr %2 to ptr addrspace(1)
  %152 = getelementptr inbounds i8, ptr %1, i16 2
  %153 = getelementptr inbounds i8, ptr %1, i16 4
  %154 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %b46

b45:
  %155 = phi i16 [ 0, %136 ], [ %158, %b48 ]
  %156 = phi i8 [ 0, %136 ], [ %164, %b48 ]
  %157 = icmp ne i8 %156, 0
  br i1 %157, label %b60, label %b61

b46:
  %158 = phi i16 [ %148, %b43 ], [ %181, %b58 ]
  %159 = load i16, ptr %138
  %160 = icmp ult i16 %158, %159
  br i1 %160, label %b49, label %b50

b47:
  %161 = load i16, ptr %166
  %162 = icmp eq i16 %161, %137
  %163 = sext i1 %162 to i8
  br i1 %162, label %b55, label %b54

b48:
  %164 = phi i8 [ 0, %b49 ], [ -1, %b57 ]
  br label %b45

b49:
  %165 = mul i16 %158, 6
  %166 = getelementptr i8, ptr %34, i16 %165
  %167 = load i16, ptr %166
  %168 = icmp ne i16 %167, 0
  br i1 %168, label %b47, label %b48

b50:
  call addrspace(1) void @N$EBND()
  unreachable

b54:
  %169 = phi i8 [ %163, %b47 ], [ %179, %b55 ]
  %170 = icmp ne i8 %169, 0
  br i1 %170, label %b57, label %b58

b55:
  %171 = getelementptr i8, ptr %166, i16 2
  %172 = load ptr, ptr %171
  %173 = getelementptr i8, ptr %172, i16 -4
  %174 = load i16, ptr %173
  %175 = addrspacecast ptr %172 to ptr addrspace(1)
  store i16 %174, ptr %2, !tbaa !2
  store i16 %174, ptr %149, !tbaa !2
  store ptr addrspace(1) %175, ptr %150, !tbaa !2
  %176 = load i16, ptr %122
  store i16 %176, ptr %1, !tbaa !2
  store i16 %176, ptr %152, !tbaa !2
  store ptr addrspace(1) %124, ptr %153, !tbaa !2
  %177 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %151, ptr addrspace(1) %154)
  %178 = icmp eq i8 %177, 0
  %179 = sext i1 %178 to i8
  br label %b54

b57:
  br label %b48

b58:
  %180 = add i16 %158, 1
  %181 = and i16 %180, %147
  br label %b46

b60:
  %182 = load i16, ptr %138
  %183 = icmp ult i16 %155, %182
  br i1 %183, label %b62, label %b63

b61:
  call addrspace(1) void @N$EKEY()
  unreachable

b62:
  %184 = mul i16 %155, 6
  %185 = getelementptr i8, ptr %34, i16 %184
  %186 = getelementptr i8, ptr %185, i16 4
  %187 = load i16, ptr %186
  call addrspace(1) void @N$PI2(i16 %187)
  call addrspace(1) void @N$PN()
  store i8 0, ptr addrspace(1) %0
  call addrspace(1) void @N$BDRP(ptr %121)
  %188 = icmp ne ptr %34, null
  br i1 %188, label %b65, label %b64

b63:
  call addrspace(1) void @N$EBND()
  unreachable

b64:
  call addrspace(1) void @N$BDRP(ptr %34)
  call addrspace(1) void @N$BDRP(ptr null)
  %189 = icmp ne ptr %24, null
  br i1 %189, label %b75, label %b74

b65:
  %190 = load i16, ptr %138
  br label %b66

b66:
  %191 = phi i16 [ 0, %b65 ], [ %197, %b68 ]
  %192 = icmp ult i16 %191, %190
  br i1 %192, label %b68, label %b64

b68:
  %193 = mul i16 %191, 6
  %194 = getelementptr i8, ptr %34, i16 %193
  %195 = getelementptr i8, ptr %194, i16 2
  %196 = load ptr, ptr %195
  call addrspace(1) void @N$BDRP(ptr %196)
  %197 = add i16 %191, 1
  br label %b66

b74:
  call addrspace(1) void @N$BDRP(ptr %24)
  ret void

b75:
  %198 = load i16, ptr %26
  br label %b76

b76:
  %199 = phi i16 [ 0, %b75 ], [ %204, %b78 ]
  %200 = icmp ult i16 %199, %198
  br i1 %200, label %b78, label %b74

b78:
  %201 = shl i16 %199, 2
  %202 = getelementptr i8, ptr %24, i16 %201
  %203 = load ptr, ptr %202
  call addrspace(1) void @N$BDRP(ptr %203)
  %204 = add i16 %199, 1
  br label %b76
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 6, i1 false)
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @$main(ptr addrspace(1) %1)
  %2 = load i8, ptr %0, !tbaa !2
  %3 = icmp eq i8 %2, 0
  br i1 %3, label %b4, label %b3

b3:
  ret i16 1

b4:
  ret i16 0
}

define internal i8 @string.eq(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %0, ptr addrspace(1) %1)
  %3 = icmp eq i8 %2, 0
  %4 = sext i1 %3 to i8
  ret i8 %4
}

define internal i16 @string.hash(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i16 [ 5381, %b1 ], [ %11, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %12, %b3 ]
  %6 = icmp ult i16 %5, %1
  br i1 %6, label %b3, label %b5

b3:
  %7 = getelementptr i8, ptr addrspace(1) %3, i16 %5
  %8 = mul i16 %4, 33
  %9 = load i8, ptr addrspace(1) %7
  %10 = zext i8 %9 to i16
  %11 = xor i16 %8, %10
  %12 = add i16 %5, 1
  br label %b2

b5:
  ret i16 %4
}

declare i16 @N$OWRI(i16, ptr addrspace(1), i16) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare i16 @N$OOPN(ptr addrspace(1), i8) addrspace(1)

declare i16 @N$OCRE(ptr addrspace(1)) addrspace(1)

declare i16 @N$OREA(i16, ptr addrspace(1), i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare i16 @N$OCLO(i16) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare ptr @N$DRES(ptr, i16) addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$EKEY() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
