target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [9 x i8] c"\08\00\02\00\02\00\0D\0A\00"
@$str3 = internal constant [14 x i8] c"\08\00\07\00\07\00closed \00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00 after \00"
@$str5 = internal constant [13 x i8] c"\08\00\06\00\06\00 lines\00"
@$str6 = internal constant [14 x i8] c"\08\00\07\00\07\00LOG.TXT\00"
@$str7 = internal constant [14 x i8] c"\08\00\07\00\07\00started\00"
@$str8 = internal constant [18 x i8] c"\08\00\0B\00\0B\00SCRATCH.TXT\00"
@$str9 = internal constant [16 x i8] c"\08\00\09\00\09\00temporary\00"
@$str10 = internal constant [24 x i8] c"\08\00\11\00\11\00moved, still open\00"
@$str11 = internal constant [11 x i8] c"\08\00\04\00\04\00done\00"

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
  %35 = call addrspace(1) i16 @N$OOPN(ptr addrspace(1) %9, i8 %2)
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

define internal void @Log.create(ptr addrspace(1) %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca [136 x i8]
  %4 = alloca [134 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 134, i1 false)
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  %6 = getelementptr i8, ptr %1, i16 -4
  %7 = load i16, ptr %6
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %7, ptr %2, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %7, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.create(ptr addrspace(1) %5, ptr addrspace(1) %11)
  %12 = load i8, ptr %3, !tbaa !2
  %13 = icmp eq i8 %12, 1
  br i1 %13, label %b2, label %b3

b2:
  %14 = getelementptr inbounds i8, ptr %3, i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %3, i16 4
  %17 = load i16, ptr %16, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %15, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %17, ptr addrspace(1) %19
  call addrspace(1) void @N$BDRP(ptr %1)
  ret void

b3:
  %20 = getelementptr inbounds i8, ptr %3, i16 2
  %21 = load i16, ptr %20, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %3, i16 132
  %23 = load i16, ptr %22, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %3, i16 134
  %25 = load i16, ptr %24, !tbaa !2
  store i16 %21, ptr %4, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %4, i16 2
  %27 = addrspacecast ptr %26 to ptr addrspace(1)
  %28 = getelementptr inbounds i8, ptr %3, i16 4
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  br label %b4

b4:
  %30 = phi i16 [ 0, %b3 ], [ %35, %b5 ]
  %31 = icmp slt i16 %30, 128
  br i1 %31, label %b5, label %b7

b5:
  %32 = getelementptr i8, ptr addrspace(1) %27, i16 %30
  %33 = getelementptr i8, ptr addrspace(1) %29, i16 %30
  %34 = load i8, ptr addrspace(1) %33
  store i8 %34, ptr addrspace(1) %32
  %35 = add i16 %30, 1
  br label %b4

b7:
  %36 = getelementptr inbounds i8, ptr %4, i16 130
  store i16 %23, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %4, i16 132
  store i16 %25, ptr %37, !tbaa !2
  %38 = load i16, ptr %4, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %39 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %1, ptr addrspace(1) %39
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %38, ptr addrspace(1) %40
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 6
  br label %b8

b8:
  %42 = phi i16 [ 0, %b7 ], [ %47, %b9 ]
  %43 = icmp slt i16 %42, 128
  br i1 %43, label %b9, label %b11

b9:
  %44 = getelementptr i8, ptr addrspace(1) %41, i16 %42
  %45 = getelementptr i8, ptr addrspace(1) %27, i16 %42
  %46 = load i8, ptr addrspace(1) %45
  store i8 %46, ptr addrspace(1) %44
  %47 = add i16 %42, 1
  br label %b8

b11:
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %23, ptr addrspace(1) %48
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %25, ptr addrspace(1) %49
  store i16 0, ptr %4, !tbaa !2
  br label %b12

b12:
  %50 = phi i16 [ 0, %b11 ], [ %53, %b13 ]
  %51 = icmp slt i16 %50, 128
  br i1 %51, label %b13, label %b15

b13:
  %52 = getelementptr i8, ptr addrspace(1) %27, i16 %50
  store i8 0, ptr addrspace(1) %52
  %53 = add i16 %50, 1
  br label %b12

b15:
  store i16 0, ptr %36, !tbaa !2
  store i16 0, ptr %37, !tbaa !2
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 138
  store i16 0, ptr addrspace(1) %54
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal void @Log.write(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [6 x i8]
  %3 = alloca [6 x i8]
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load i16, ptr addrspace(1) %1
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %4, ptr addrspace(1) %6, i16 %8)
  %9 = getelementptr i8, ptr @$str2, i16 6
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = addrspacecast ptr %9 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  %13 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %13, ptr addrspace(1) %4, ptr addrspace(1) %12, i16 %11)
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 136
  %15 = load i16, ptr addrspace(1) %14
  %16 = add i16 %15, 1
  store i16 %16, ptr addrspace(1) %14
  ret void
}

define internal void @Log.drop(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %1)
  %2 = load ptr, ptr addrspace(1) %0
  call addrspace(1) void @N$PS(ptr %2)
  %3 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %3)
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 136
  %5 = load i16, ptr addrspace(1) %4
  call addrspace(1) void @N$PU2(i16 %5)
  %6 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %6)
  call addrspace(1) void @N$PN()
  ret void
}

define internal void @keep_open(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %2 = load ptr, ptr addrspace(1) %1
  %3 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %4 = load i16, ptr addrspace(1) %3
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 132
  %6 = load i16, ptr addrspace(1) %5
  %7 = getelementptr i8, ptr addrspace(1) %1, i16 134
  %8 = load i16, ptr addrspace(1) %7
  %9 = getelementptr i8, ptr addrspace(1) %1, i16 136
  %10 = load i16, ptr addrspace(1) %9
  store ptr %2, ptr addrspace(1) %0
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %4, ptr addrspace(1) %11
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 4
  br label %b2

b2:
  %14 = phi i16 [ 0, %b1 ], [ %19, %b3 ]
  %15 = icmp slt i16 %14, 128
  br i1 %15, label %b3, label %b5

b3:
  %16 = getelementptr i8, ptr addrspace(1) %12, i16 %14
  %17 = getelementptr i8, ptr addrspace(1) %13, i16 %14
  %18 = load i8, ptr addrspace(1) %17
  store i8 %18, ptr addrspace(1) %16
  %19 = add i16 %14, 1
  br label %b2

b5:
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 %6, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %8, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %10, ptr addrspace(1) %22
  store ptr null, ptr addrspace(1) %1
  store i16 0, ptr addrspace(1) %3
  br label %b6

b6:
  %23 = phi i16 [ 0, %b5 ], [ %26, %b7 ]
  %24 = icmp slt i16 %23, 128
  br i1 %24, label %b7, label %b9

b7:
  %25 = getelementptr i8, ptr addrspace(1) %13, i16 %23
  store i8 0, ptr addrspace(1) %25
  %26 = add i16 %23, 1
  br label %b6

b9:
  store i16 0, ptr addrspace(1) %5
  store i16 0, ptr addrspace(1) %7
  store i16 0, ptr addrspace(1) %9
  ret void
}

define internal void @$main(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [138 x i8]
  %3 = alloca [138 x i8]
  %4 = alloca [138 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [140 x i8]
  %7 = alloca [138 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [140 x i8]
  %10 = alloca [138 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 140, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 140, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 138, i1 false)
  %11 = addrspacecast ptr %9 to ptr addrspace(1)
  %12 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @Log.create(ptr addrspace(1) %11, ptr %12)
  %13 = load i8, ptr %9, !tbaa !2
  %14 = icmp eq i8 %13, 1
  br i1 %14, label %b2, label %b3

b2:
  %15 = getelementptr inbounds i8, ptr %9, i16 2
  %16 = load i16, ptr %15, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %9, i16 4
  %18 = load i16, ptr %17, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %16, ptr addrspace(1) %19
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %18, ptr addrspace(1) %20
  ret void

b3:
  %21 = getelementptr inbounds i8, ptr %9, i16 2
  %22 = load ptr, ptr %21, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %9, i16 4
  %24 = load i16, ptr %23, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %9, i16 134
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %9, i16 136
  %28 = load i16, ptr %27, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %9, i16 138
  %30 = load i16, ptr %29, !tbaa !2
  store ptr %22, ptr %10, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %24, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %10, i16 4
  %33 = addrspacecast ptr %32 to ptr addrspace(1)
  %34 = getelementptr inbounds i8, ptr %9, i16 6
  %35 = addrspacecast ptr %34 to ptr addrspace(1)
  br label %b4

b4:
  %36 = phi i16 [ 0, %b3 ], [ %41, %b5 ]
  %37 = icmp slt i16 %36, 128
  br i1 %37, label %b5, label %b7

b5:
  %38 = getelementptr i8, ptr addrspace(1) %33, i16 %36
  %39 = getelementptr i8, ptr addrspace(1) %35, i16 %36
  %40 = load i8, ptr addrspace(1) %39
  store i8 %40, ptr addrspace(1) %38
  %41 = add i16 %36, 1
  br label %b4

b7:
  %42 = getelementptr inbounds i8, ptr %10, i16 132
  store i16 %26, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %10, i16 134
  store i16 %28, ptr %43, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %10, i16 136
  store i16 %30, ptr %44, !tbaa !2
  %45 = addrspacecast ptr %10 to ptr addrspace(1)
  %46 = getelementptr i8, ptr @$str7, i16 6
  %47 = getelementptr i8, ptr %46, i16 -4
  %48 = load i16, ptr %47
  %49 = addrspacecast ptr %46 to ptr addrspace(1)
  store i16 %48, ptr %8, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %48, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %49, ptr %51, !tbaa !2
  %52 = addrspacecast ptr %8 to ptr addrspace(1)
  call addrspace(1) void @Log.write(ptr addrspace(1) %45, ptr addrspace(1) %52)
  %53 = addrspacecast ptr %6 to ptr addrspace(1)
  %54 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @Log.create(ptr addrspace(1) %53, ptr %54)
  %55 = load i8, ptr %6, !tbaa !2
  %56 = icmp eq i8 %55, 1
  br i1 %56, label %b8, label %b9

b8:
  %57 = getelementptr inbounds i8, ptr %6, i16 2
  %58 = load i16, ptr %57, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %6, i16 4
  %60 = load i16, ptr %59, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %61 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %58, ptr addrspace(1) %61
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %60, ptr addrspace(1) %62
  %63 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %63)
  %64 = load ptr, ptr addrspace(1) %45
  call addrspace(1) void @N$PS(ptr %64)
  %65 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %65)
  %66 = getelementptr i8, ptr addrspace(1) %45, i16 136
  %67 = load i16, ptr addrspace(1) %66
  call addrspace(1) void @N$PU2(i16 %67)
  %68 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  call addrspace(1) void @N$PN()
  %69 = addrspacecast ptr %31 to ptr addrspace(1)
  %70 = load i16, ptr addrspace(1) %69
  %71 = call addrspace(1) i16 @N$OCLO(i16 %70)
  %72 = load ptr, ptr %10, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %72)
  ret void

b9:
  %73 = getelementptr inbounds i8, ptr %6, i16 2
  %74 = load ptr, ptr %73, !tbaa !2
  %75 = getelementptr inbounds i8, ptr %6, i16 4
  %76 = load i16, ptr %75, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %6, i16 134
  %78 = load i16, ptr %77, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %6, i16 136
  %80 = load i16, ptr %79, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %6, i16 138
  %82 = load i16, ptr %81, !tbaa !2
  store ptr %74, ptr %7, !tbaa !2
  %83 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %76, ptr %83, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %7, i16 4
  %85 = addrspacecast ptr %84 to ptr addrspace(1)
  %86 = getelementptr inbounds i8, ptr %6, i16 6
  %87 = addrspacecast ptr %86 to ptr addrspace(1)
  br label %b12

b12:
  %88 = phi i16 [ 0, %b9 ], [ %93, %b13 ]
  %89 = icmp slt i16 %88, 128
  br i1 %89, label %b13, label %b15

b13:
  %90 = getelementptr i8, ptr addrspace(1) %85, i16 %88
  %91 = getelementptr i8, ptr addrspace(1) %87, i16 %88
  %92 = load i8, ptr addrspace(1) %91
  store i8 %92, ptr addrspace(1) %90
  %93 = add i16 %88, 1
  br label %b12

b15:
  %94 = getelementptr inbounds i8, ptr %7, i16 132
  store i16 %78, ptr %94, !tbaa !2
  %95 = getelementptr inbounds i8, ptr %7, i16 134
  store i16 %80, ptr %95, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %7, i16 136
  store i16 %82, ptr %96, !tbaa !2
  %97 = addrspacecast ptr %7 to ptr addrspace(1)
  %98 = getelementptr i8, ptr @$str9, i16 6
  %99 = getelementptr i8, ptr %98, i16 -4
  %100 = load i16, ptr %99
  %101 = addrspacecast ptr %98 to ptr addrspace(1)
  store i16 %100, ptr %5, !tbaa !2
  %102 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %100, ptr %102, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %101, ptr %103, !tbaa !2
  %104 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @Log.write(ptr addrspace(1) %97, ptr addrspace(1) %104)
  %105 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %105)
  %106 = load ptr, ptr addrspace(1) %97
  call addrspace(1) void @N$PS(ptr %106)
  %107 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %107)
  %108 = getelementptr i8, ptr addrspace(1) %97, i16 136
  %109 = load i16, ptr addrspace(1) %108
  call addrspace(1) void @N$PU2(i16 %109)
  %110 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %110)
  call addrspace(1) void @N$PN()
  %111 = addrspacecast ptr %83 to ptr addrspace(1)
  %112 = load i16, ptr addrspace(1) %111
  %113 = call addrspace(1) i16 @N$OCLO(i16 %112)
  %114 = load ptr, ptr %7, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %114)
  %115 = addrspacecast ptr %3 to ptr addrspace(1)
  %116 = load ptr, ptr %10, !tbaa !2
  %117 = load i16, ptr %31, !tbaa !2
  %118 = load i16, ptr %42, !tbaa !2
  %119 = load i16, ptr %43, !tbaa !2
  %120 = load i16, ptr %44, !tbaa !2
  store ptr %116, ptr %2, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %117, ptr %121, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %2, i16 4
  %123 = addrspacecast ptr %122 to ptr addrspace(1)
  br label %b18

b18:
  %124 = phi i16 [ 0, %b15 ], [ %129, %b19 ]
  %125 = icmp slt i16 %124, 128
  br i1 %125, label %b19, label %b21

b19:
  %126 = getelementptr i8, ptr addrspace(1) %123, i16 %124
  %127 = getelementptr i8, ptr addrspace(1) %33, i16 %124
  %128 = load i8, ptr addrspace(1) %127
  store i8 %128, ptr addrspace(1) %126
  %129 = add i16 %124, 1
  br label %b18

b21:
  %130 = getelementptr inbounds i8, ptr %2, i16 132
  store i16 %118, ptr %130, !tbaa !2
  %131 = getelementptr inbounds i8, ptr %2, i16 134
  store i16 %119, ptr %131, !tbaa !2
  %132 = getelementptr inbounds i8, ptr %2, i16 136
  store i16 %120, ptr %132, !tbaa !2
  store ptr null, ptr %10, !tbaa !2
  store i16 0, ptr %31, !tbaa !2
  br label %b22

b22:
  %133 = phi i16 [ 0, %b21 ], [ %136, %b23 ]
  %134 = icmp slt i16 %133, 128
  br i1 %134, label %b23, label %b25

b23:
  %135 = getelementptr i8, ptr addrspace(1) %33, i16 %133
  store i8 0, ptr addrspace(1) %135
  %136 = add i16 %133, 1
  br label %b22

b25:
  store i16 0, ptr %42, !tbaa !2
  store i16 0, ptr %43, !tbaa !2
  store i16 0, ptr %44, !tbaa !2
  %137 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @keep_open(ptr addrspace(1) %115, ptr addrspace(1) %137)
  %138 = load ptr, ptr %3, !tbaa !2
  %139 = getelementptr inbounds i8, ptr %3, i16 2
  %140 = load i16, ptr %139, !tbaa !2
  %141 = getelementptr inbounds i8, ptr %3, i16 132
  %142 = load i16, ptr %141, !tbaa !2
  %143 = getelementptr inbounds i8, ptr %3, i16 134
  %144 = load i16, ptr %143, !tbaa !2
  %145 = getelementptr inbounds i8, ptr %3, i16 136
  %146 = load i16, ptr %145, !tbaa !2
  store ptr %138, ptr %4, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %140, ptr %147, !tbaa !2
  %148 = getelementptr inbounds i8, ptr %4, i16 4
  %149 = addrspacecast ptr %148 to ptr addrspace(1)
  %150 = getelementptr inbounds i8, ptr %3, i16 4
  %151 = addrspacecast ptr %150 to ptr addrspace(1)
  br label %b26

b26:
  %152 = phi i16 [ 0, %b25 ], [ %157, %b27 ]
  %153 = icmp slt i16 %152, 128
  br i1 %153, label %b27, label %b29

b27:
  %154 = getelementptr i8, ptr addrspace(1) %149, i16 %152
  %155 = getelementptr i8, ptr addrspace(1) %151, i16 %152
  %156 = load i8, ptr addrspace(1) %155
  store i8 %156, ptr addrspace(1) %154
  %157 = add i16 %152, 1
  br label %b26

b29:
  %158 = getelementptr inbounds i8, ptr %4, i16 132
  store i16 %142, ptr %158, !tbaa !2
  %159 = getelementptr inbounds i8, ptr %4, i16 134
  store i16 %144, ptr %159, !tbaa !2
  %160 = getelementptr inbounds i8, ptr %4, i16 136
  store i16 %146, ptr %160, !tbaa !2
  %161 = addrspacecast ptr %4 to ptr addrspace(1)
  %162 = getelementptr i8, ptr @$str10, i16 6
  %163 = getelementptr i8, ptr %162, i16 -4
  %164 = load i16, ptr %163
  %165 = addrspacecast ptr %162 to ptr addrspace(1)
  store i16 %164, ptr %1, !tbaa !2
  %166 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %164, ptr %166, !tbaa !2
  %167 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %165, ptr %167, !tbaa !2
  %168 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Log.write(ptr addrspace(1) %161, ptr addrspace(1) %168)
  %169 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %169)
  call addrspace(1) void @N$PN()
  store i8 0, ptr addrspace(1) %0
  call addrspace(1) void @N$PS(ptr %105)
  %170 = load ptr, ptr addrspace(1) %161
  call addrspace(1) void @N$PS(ptr %170)
  call addrspace(1) void @N$PS(ptr %107)
  %171 = getelementptr i8, ptr addrspace(1) %161, i16 136
  %172 = load i16, ptr addrspace(1) %171
  call addrspace(1) void @N$PU2(i16 %172)
  call addrspace(1) void @N$PS(ptr %110)
  call addrspace(1) void @N$PN()
  %173 = addrspacecast ptr %147 to ptr addrspace(1)
  %174 = load i16, ptr addrspace(1) %173
  %175 = call addrspace(1) i16 @N$OCLO(i16 %174)
  %176 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %176)
  ret void
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

declare i16 @N$OWRI(i16, ptr addrspace(1), i16) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare i16 @N$OOPN(ptr addrspace(1), i8) addrspace(1)

declare i16 @N$OCRE(ptr addrspace(1)) addrspace(1)

declare i16 @N$OREA(i16, ptr addrspace(1), i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare i16 @N$OCLO(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
