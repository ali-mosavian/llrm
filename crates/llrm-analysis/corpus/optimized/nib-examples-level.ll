target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [16 x i8] c"\08\00\09\00\09\00LEVEL.DAT\00"
@$str3 = internal constant [15 x i8] c"\08\00\08\00\08\00 planes \00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00 nodes \00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00 faces, \00"
@$str6 = internal constant [13 x i8] c"\08\00\06\00\06\00 bytes\00"
@$str7 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str8 = internal constant [8 x i8] c"\08\00\01\00\01\00(\00"
@$str9 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00) leaf \00"
@$str11 = internal constant [13 x i8] c"\08\00\06\00\06\00 node \00"
@$str12 = internal constant [8 x i8] c"\08\00\01\00\01\00:\00"
@$str13 = internal constant [37 x i8] c"\08\00\1E\00\1E\00cannot read or write LEVEL.DAT\00"
@$str14 = internal constant [31 x i8] c"\08\00\18\00\18\00LEVEL.DAT is not a level\00"
@$str15 = internal constant [20 x i8] c"\08\00\0D\00\0D\00LEVEL.DAT is \00"
@$str16 = internal constant [19 x i8] c"\08\00\0C\00\0C\00 bytes short\00"

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

define internal void @read_exact(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.read_raw(ptr addrspace(1) %5, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3)
  %6 = load i8, ptr %4, !tbaa !2
  %7 = icmp eq i8 %6, 1
  br i1 %7, label %b2, label %b3

b2:
  %8 = getelementptr inbounds i8, ptr %4, i16 2
  %9 = load i16, ptr %8, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %4, i16 4
  %11 = load i16, ptr %10, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %12
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %9, ptr addrspace(1) %13
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %11, ptr addrspace(1) %14
  ret void

b3:
  %15 = getelementptr inbounds i8, ptr %4, i16 2
  %16 = load i16, ptr %15, !tbaa !2
  %17 = icmp ult i16 %16, %3
  br i1 %17, label %b4, label %b5

b4:
  %18 = sub i16 %3, %16
  store i8 1, ptr addrspace(1) %0
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %19
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %18, ptr addrspace(1) %20
  ret void

b5:
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal void @Level.save(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [6 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [6 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [136 x i8]
  %12 = alloca [134 x i8]
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 134, i1 false)
  %13 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.create(ptr addrspace(1) %13, ptr addrspace(1) %2)
  %14 = load i8, ptr %11, !tbaa !2
  %15 = icmp eq i8 %14, 1
  br i1 %15, label %b2, label %b3

b2:
  %16 = getelementptr inbounds i8, ptr %11, i16 2
  %17 = load i16, ptr %16, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %11, i16 4
  %19 = load i16, ptr %18, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %17, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %19, ptr addrspace(1) %22
  ret void

b3:
  %23 = getelementptr inbounds i8, ptr %11, i16 2
  %24 = load i16, ptr %23, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %11, i16 132
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %11, i16 134
  %28 = load i16, ptr %27, !tbaa !2
  store i16 %24, ptr %12, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %12, i16 2
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  %31 = getelementptr inbounds i8, ptr %11, i16 4
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  br label %b4

b4:
  %33 = phi i16 [ 0, %b3 ], [ %38, %b5 ]
  %34 = icmp slt i16 %33, 128
  br i1 %34, label %b5, label %b7

b5:
  %35 = getelementptr i8, ptr addrspace(1) %30, i16 %33
  %36 = getelementptr i8, ptr addrspace(1) %32, i16 %33
  %37 = load i8, ptr addrspace(1) %36
  store i8 %37, ptr addrspace(1) %35
  %38 = add i16 %33, 1
  br label %b4

b7:
  %39 = getelementptr inbounds i8, ptr %12, i16 130
  store i16 %26, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %12, i16 132
  store i16 %28, ptr %40, !tbaa !2
  %41 = load ptr, ptr addrspace(1) %1
  %42 = getelementptr i8, ptr %41, i16 -4
  %43 = load i16, ptr %42
  %44 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %45 = load ptr, ptr addrspace(1) %44
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %49 = load ptr, ptr addrspace(1) %48
  %50 = getelementptr i8, ptr %49, i16 -4
  %51 = load i16, ptr %50
  store i16 22092, ptr %10, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %43, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %10, i16 4
  store i16 %47, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %10, i16 6
  store i16 %51, ptr %54, !tbaa !2
  %55 = addrspacecast ptr %10 to ptr addrspace(1)
  %56 = addrspacecast ptr %9 to ptr addrspace(1)
  %57 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %56, ptr addrspace(1) %57, ptr addrspace(1) %55, i16 8)
  %58 = load i8, ptr %9, !tbaa !2
  %59 = icmp eq i8 %58, 1
  br i1 %59, label %b8, label %b9

b8:
  %60 = getelementptr inbounds i8, ptr %9, i16 2
  %61 = load i16, ptr %60, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %9, i16 4
  %63 = load i16, ptr %62, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %64
  %65 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %61, ptr addrspace(1) %65
  %66 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %63, ptr addrspace(1) %66
  %67 = load i16, ptr addrspace(1) %57
  %68 = call addrspace(1) i16 @N$OCLO(i16 %67)
  ret void

b9:
  %69 = addrspacecast ptr %8 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 6, i1 false)
  %70 = load ptr, ptr addrspace(1) %1
  %71 = addrspacecast ptr %70 to ptr addrspace(1)
  %72 = addrspacecast ptr %5 to ptr addrspace(1)
  %73 = getelementptr i8, ptr %70, i16 -4
  %74 = load i16, ptr %73
  %75 = shl i16 %74, 2
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %72, ptr addrspace(1) %57, ptr addrspace(1) %71, i16 %75)
  %76 = load i8, ptr %5
  %77 = icmp eq i8 %76, 1
  br i1 %77, label %78, label %86

78:
  %79 = getelementptr inbounds i8, ptr %5, i16 2
  %80 = load i16, ptr %79
  %81 = getelementptr inbounds i8, ptr %5, i16 4
  %82 = load i16, ptr %81
  store i8 1, ptr addrspace(1) %69
  %83 = getelementptr i8, ptr addrspace(1) %69, i16 2
  store i8 0, ptr addrspace(1) %83
  %84 = getelementptr i8, ptr addrspace(1) %69, i16 4
  store i16 %80, ptr addrspace(1) %84
  %85 = getelementptr i8, ptr addrspace(1) %69, i16 6
  store i16 %82, ptr addrspace(1) %85
  br label %87

86:
  store i8 0, ptr addrspace(1) %69
  br label %87

87:
  %88 = load i8, ptr %8, !tbaa !2
  %89 = icmp eq i8 %88, 1
  br i1 %89, label %b12, label %b13

b12:
  %90 = getelementptr inbounds i8, ptr %8, i16 2
  %91 = load i16, ptr %90, !tbaa !2
  %92 = getelementptr inbounds i8, ptr %8, i16 4
  %93 = load i16, ptr %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %8, i16 6
  %95 = load i16, ptr %94, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %96 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %91, ptr addrspace(1) %96
  %97 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %93, ptr addrspace(1) %97
  %98 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %95, ptr addrspace(1) %98
  %99 = load i16, ptr addrspace(1) %57
  %100 = call addrspace(1) i16 @N$OCLO(i16 %99)
  ret void

b13:
  %101 = addrspacecast ptr %7 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  %102 = load ptr, ptr addrspace(1) %44
  %103 = addrspacecast ptr %102 to ptr addrspace(1)
  %104 = addrspacecast ptr %4 to ptr addrspace(1)
  %105 = getelementptr i8, ptr %102, i16 -4
  %106 = load i16, ptr %105
  %107 = mul i16 %106, 9
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %104, ptr addrspace(1) %57, ptr addrspace(1) %103, i16 %107)
  %108 = load i8, ptr %4
  %109 = icmp eq i8 %108, 1
  br i1 %109, label %110, label %118

110:
  %111 = getelementptr inbounds i8, ptr %4, i16 2
  %112 = load i16, ptr %111
  %113 = getelementptr inbounds i8, ptr %4, i16 4
  %114 = load i16, ptr %113
  store i8 1, ptr addrspace(1) %101
  %115 = getelementptr i8, ptr addrspace(1) %101, i16 2
  store i8 0, ptr addrspace(1) %115
  %116 = getelementptr i8, ptr addrspace(1) %101, i16 4
  store i16 %112, ptr addrspace(1) %116
  %117 = getelementptr i8, ptr addrspace(1) %101, i16 6
  store i16 %114, ptr addrspace(1) %117
  br label %119

118:
  store i8 0, ptr addrspace(1) %101
  br label %119

119:
  %120 = load i8, ptr %7, !tbaa !2
  %121 = icmp eq i8 %120, 1
  br i1 %121, label %b16, label %b17

b16:
  %122 = getelementptr inbounds i8, ptr %7, i16 2
  %123 = load i16, ptr %122, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %7, i16 4
  %125 = load i16, ptr %124, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %7, i16 6
  %127 = load i16, ptr %126, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %128 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %123, ptr addrspace(1) %128
  %129 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %125, ptr addrspace(1) %129
  %130 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %127, ptr addrspace(1) %130
  %131 = load i16, ptr addrspace(1) %57
  %132 = call addrspace(1) i16 @N$OCLO(i16 %131)
  ret void

b17:
  %133 = addrspacecast ptr %6 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  %134 = load ptr, ptr addrspace(1) %48
  %135 = addrspacecast ptr %134 to ptr addrspace(1)
  %136 = addrspacecast ptr %3 to ptr addrspace(1)
  %137 = getelementptr i8, ptr %134, i16 -4
  %138 = load i16, ptr %137
  %139 = mul i16 %138, 3
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %136, ptr addrspace(1) %57, ptr addrspace(1) %135, i16 %139)
  %140 = load i8, ptr %3
  %141 = icmp eq i8 %140, 1
  br i1 %141, label %142, label %150

142:
  %143 = getelementptr inbounds i8, ptr %3, i16 2
  %144 = load i16, ptr %143
  %145 = getelementptr inbounds i8, ptr %3, i16 4
  %146 = load i16, ptr %145
  store i8 1, ptr addrspace(1) %133
  %147 = getelementptr i8, ptr addrspace(1) %133, i16 2
  store i8 0, ptr addrspace(1) %147
  %148 = getelementptr i8, ptr addrspace(1) %133, i16 4
  store i16 %144, ptr addrspace(1) %148
  %149 = getelementptr i8, ptr addrspace(1) %133, i16 6
  store i16 %146, ptr addrspace(1) %149
  br label %151

150:
  store i8 0, ptr addrspace(1) %133
  br label %151

151:
  %152 = load i8, ptr %6, !tbaa !2
  %153 = icmp eq i8 %152, 1
  br i1 %153, label %b20, label %b21

b20:
  %154 = getelementptr inbounds i8, ptr %6, i16 2
  %155 = load i16, ptr %154, !tbaa !2
  %156 = getelementptr inbounds i8, ptr %6, i16 4
  %157 = load i16, ptr %156, !tbaa !2
  %158 = getelementptr inbounds i8, ptr %6, i16 6
  %159 = load i16, ptr %158, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %160 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %155, ptr addrspace(1) %160
  %161 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %157, ptr addrspace(1) %161
  %162 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %159, ptr addrspace(1) %162
  %163 = load i16, ptr addrspace(1) %57
  %164 = call addrspace(1) i16 @N$OCLO(i16 %163)
  ret void

b21:
  store i8 0, ptr addrspace(1) %0
  %165 = load i16, ptr addrspace(1) %57
  %166 = call addrspace(1) i16 @N$OCLO(i16 %165)
  ret void
}

define internal void @Level.load(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [6 x i8]
  %3 = alloca [3 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [9 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [4 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [136 x i8]
  %12 = alloca [134 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 3, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 9, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 134, i1 false)
  %13 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.open(ptr addrspace(1) %13, ptr addrspace(1) %1, i8 0)
  %14 = load i8, ptr %11, !tbaa !2
  %15 = icmp eq i8 %14, 1
  br i1 %15, label %b2, label %b3

b2:
  %16 = getelementptr inbounds i8, ptr %11, i16 2
  %17 = load i16, ptr %16, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %11, i16 4
  %19 = load i16, ptr %18, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %17, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %19, ptr addrspace(1) %22
  ret void

b3:
  %23 = getelementptr inbounds i8, ptr %11, i16 2
  %24 = load i16, ptr %23, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %11, i16 132
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %11, i16 134
  %28 = load i16, ptr %27, !tbaa !2
  store i16 %24, ptr %12, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %12, i16 2
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  %31 = getelementptr inbounds i8, ptr %11, i16 4
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  br label %b4

b4:
  %33 = phi i16 [ 0, %b3 ], [ %38, %b5 ]
  %34 = icmp slt i16 %33, 128
  br i1 %34, label %b5, label %b7

b5:
  %35 = getelementptr i8, ptr addrspace(1) %30, i16 %33
  %36 = getelementptr i8, ptr addrspace(1) %32, i16 %33
  %37 = load i8, ptr addrspace(1) %36
  store i8 %37, ptr addrspace(1) %35
  %38 = add i16 %33, 1
  br label %b4

b7:
  %39 = getelementptr inbounds i8, ptr %12, i16 130
  store i16 %26, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %12, i16 132
  store i16 %28, ptr %40, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 0, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %10, i16 4
  store i16 0, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %10, i16 6
  store i16 0, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %10 to ptr addrspace(1)
  %45 = addrspacecast ptr %9 to ptr addrspace(1)
  %46 = addrspacecast ptr %12 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 6, i1 false)
  %47 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.read_raw(ptr addrspace(1) %47, ptr addrspace(1) %46, ptr addrspace(1) %44, i16 8)
  %48 = load i8, ptr %2
  %49 = icmp eq i8 %48, 1
  br i1 %49, label %50, label %58

50:
  %51 = getelementptr inbounds i8, ptr %2, i16 2
  %52 = load i16, ptr %51
  %53 = getelementptr inbounds i8, ptr %2, i16 4
  %54 = load i16, ptr %53
  store i8 1, ptr addrspace(1) %45
  %55 = getelementptr i8, ptr addrspace(1) %45, i16 2
  store i8 0, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %45, i16 4
  store i16 %52, ptr addrspace(1) %56
  %57 = getelementptr i8, ptr addrspace(1) %45, i16 6
  store i16 %54, ptr addrspace(1) %57
  br label %67

58:
  %59 = getelementptr inbounds i8, ptr %2, i16 2
  %60 = load i16, ptr %59
  %61 = icmp ult i16 %60, 8
  br i1 %61, label %62, label %66

62:
  %63 = sub i16 8, %60
  store i8 1, ptr addrspace(1) %45
  %64 = getelementptr i8, ptr addrspace(1) %45, i16 2
  store i8 2, ptr addrspace(1) %64
  %65 = getelementptr i8, ptr addrspace(1) %45, i16 4
  store i16 %63, ptr addrspace(1) %65
  br label %67

66:
  store i8 0, ptr addrspace(1) %45
  br label %67

67:
  %68 = load i8, ptr %9, !tbaa !2
  %69 = icmp eq i8 %68, 1
  br i1 %69, label %b8, label %b9

b8:
  %70 = getelementptr inbounds i8, ptr %9, i16 2
  %71 = load i16, ptr %70, !tbaa !2
  %72 = getelementptr inbounds i8, ptr %9, i16 4
  %73 = load i16, ptr %72, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %9, i16 6
  %75 = load i16, ptr %74, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %76 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %71, ptr addrspace(1) %76
  %77 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %73, ptr addrspace(1) %77
  %78 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %75, ptr addrspace(1) %78
  %79 = load i16, ptr addrspace(1) %46
  %80 = call addrspace(1) i16 @N$OCLO(i16 %79)
  ret void

b9:
  %81 = load i16, ptr %10, !tbaa !2
  %82 = icmp ne i16 %81, 22092
  br i1 %82, label %b12, label %b14

b12:
  store i8 1, ptr addrspace(1) %0
  %83 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 1, ptr addrspace(1) %83
  %84 = load i16, ptr addrspace(1) %46
  %85 = call addrspace(1) i16 @N$OCLO(i16 %84)
  ret void

b14:
  %86 = addrspacecast ptr %8 to ptr addrspace(1)
  %87 = load i16, ptr %41, !tbaa !2
  store i8 0, ptr %7, !tbaa !2
  %88 = getelementptr inbounds i8, ptr %7, i16 1
  store i8 0, ptr %88, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 0, ptr %89, !tbaa !2
  %90 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @"records[Plane]"(ptr addrspace(1) %86, ptr addrspace(1) %46, i16 %87, ptr addrspace(1) %90)
  %91 = load i8, ptr %8, !tbaa !2
  %92 = icmp eq i8 %91, 1
  br i1 %92, label %b17, label %b18

b17:
  %93 = getelementptr inbounds i8, ptr %8, i16 2
  %94 = load i16, ptr %93, !tbaa !2
  %95 = getelementptr inbounds i8, ptr %8, i16 4
  %96 = load i16, ptr %95, !tbaa !2
  %97 = getelementptr inbounds i8, ptr %8, i16 6
  %98 = load i16, ptr %97, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %99 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %94, ptr addrspace(1) %99
  %100 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %96, ptr addrspace(1) %100
  %101 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %98, ptr addrspace(1) %101
  %102 = load i16, ptr addrspace(1) %46
  %103 = call addrspace(1) i16 @N$OCLO(i16 %102)
  ret void

b18:
  %104 = getelementptr inbounds i8, ptr %8, i16 2
  %105 = load ptr, ptr %104, !tbaa !2
  %106 = addrspacecast ptr %6 to ptr addrspace(1)
  %107 = load i16, ptr %42, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 0, ptr %108, !tbaa !2
  %109 = getelementptr inbounds i8, ptr %5, i16 4
  store i16 0, ptr %109, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %5, i16 6
  store i16 0, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %5, i16 8
  store i8 0, ptr %111, !tbaa !2
  %112 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @"records[Node]"(ptr addrspace(1) %106, ptr addrspace(1) %46, i16 %107, ptr addrspace(1) %112)
  %113 = load i8, ptr %6, !tbaa !2
  %114 = icmp eq i8 %113, 1
  br i1 %114, label %b21, label %b22

b21:
  %115 = getelementptr inbounds i8, ptr %6, i16 2
  %116 = load i16, ptr %115, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %6, i16 4
  %118 = load i16, ptr %117, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %6, i16 6
  %120 = load i16, ptr %119, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %121 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %116, ptr addrspace(1) %121
  %122 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %118, ptr addrspace(1) %122
  %123 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %120, ptr addrspace(1) %123
  call addrspace(1) void @N$BDRP(ptr %105)
  %124 = load i16, ptr addrspace(1) %46
  %125 = call addrspace(1) i16 @N$OCLO(i16 %124)
  ret void

b22:
  %126 = getelementptr inbounds i8, ptr %6, i16 2
  %127 = load ptr, ptr %126, !tbaa !2
  %128 = addrspacecast ptr %4 to ptr addrspace(1)
  %129 = load i16, ptr %43, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  %130 = getelementptr inbounds i8, ptr %3, i16 2
  store i8 0, ptr %130, !tbaa !2
  %131 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @"records[Face]"(ptr addrspace(1) %128, ptr addrspace(1) %46, i16 %129, ptr addrspace(1) %131)
  %132 = load i8, ptr %4, !tbaa !2
  %133 = icmp eq i8 %132, 1
  br i1 %133, label %b25, label %b26

b25:
  %134 = getelementptr inbounds i8, ptr %4, i16 2
  %135 = load i16, ptr %134, !tbaa !2
  %136 = getelementptr inbounds i8, ptr %4, i16 4
  %137 = load i16, ptr %136, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %4, i16 6
  %139 = load i16, ptr %138, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %140 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %135, ptr addrspace(1) %140
  %141 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %137, ptr addrspace(1) %141
  %142 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %139, ptr addrspace(1) %142
  call addrspace(1) void @N$BDRP(ptr %127)
  call addrspace(1) void @N$BDRP(ptr %105)
  %143 = load i16, ptr addrspace(1) %46
  %144 = call addrspace(1) i16 @N$OCLO(i16 %143)
  ret void

b26:
  %145 = getelementptr inbounds i8, ptr %4, i16 2
  %146 = load ptr, ptr %145, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %147 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %105, ptr addrspace(1) %147
  %148 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %127, ptr addrspace(1) %148
  %149 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store ptr %146, ptr addrspace(1) %149
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr null)
  %150 = load i16, ptr addrspace(1) %46
  %151 = call addrspace(1) i16 @N$OCLO(i16 %150)
  ret void
}

define internal i32 @Level.leaf(ptr addrspace(1) %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %45, %b12 ]
  %6 = phi i16 [ 0, %b1 ], [ %5, %b12 ]
  %7 = icmp sge i16 %5, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load ptr, ptr addrspace(1) %4
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = icmp ult i16 %5, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = sub i16 -1, %5
  store i16 %12, ptr %3, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %6, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %3 to ptr addrspace(1)
  %15 = load i32, ptr addrspace(1) %14, !tbaa !2
  ret i32 %15

b5:
  %16 = mul i16 %5, 9
  %17 = getelementptr i8, ptr %8, i16 %16
  %18 = addrspacecast ptr %17 to ptr addrspace(1)
  %19 = load ptr, ptr addrspace(1) %0
  %20 = load i16, ptr addrspace(1) %18
  %21 = getelementptr i8, ptr %19, i16 -4
  %22 = load i16, ptr %21
  %23 = icmp ult i16 %20, %22
  br i1 %23, label %b7, label %b8

b6:
  call addrspace(1) void @N$EBND()
  unreachable

b7:
  %24 = shl i16 %20, 2
  %25 = getelementptr i8, ptr %19, i16 %24
  %26 = addrspacecast ptr %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 0
  %28 = load i8, ptr addrspace(1) %27
  %29 = sext i8 %28 to i16
  %30 = mul i16 %29, %1
  %31 = getelementptr i8, ptr addrspace(1) %26, i16 1
  %32 = load i8, ptr addrspace(1) %31
  %33 = sext i8 %32 to i16
  %34 = mul i16 %33, %2
  %35 = add i16 %30, %34
  %36 = getelementptr i8, ptr addrspace(1) %26, i16 2
  %37 = load i16, ptr addrspace(1) %36
  %38 = sub i16 %35, %37
  %39 = getelementptr i8, ptr addrspace(1) %18, i16 2
  %40 = icmp sge i16 %38, 0
  br i1 %40, label %b11, label %b10

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b10:
  br label %b11

b11:
  %41 = phi i16 [ 0, %b7 ], [ 1, %b10 ]
  %42 = icmp ult i16 %41, 2
  br i1 %42, label %b12, label %b13

b12:
  %43 = shl i16 %41, 1
  %44 = getelementptr i8, ptr addrspace(1) %39, i16 %43
  %45 = load i16, ptr addrspace(1) %44
  br label %b2

b13:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @built(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr @$str1, i16 6
  %2 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 3, i16 4)
  %3 = getelementptr i8, ptr %2, i16 0
  store i8 1, ptr %3
  %4 = getelementptr i8, ptr %3, i16 1
  store i8 0, ptr %4
  %5 = getelementptr i8, ptr %3, i16 2
  store i16 32, ptr %5
  %6 = getelementptr i8, ptr %2, i16 4
  store i8 0, ptr %6
  %7 = getelementptr i8, ptr %6, i16 1
  store i8 1, ptr %7
  %8 = getelementptr i8, ptr %6, i16 2
  store i16 32, ptr %8
  %9 = getelementptr i8, ptr %2, i16 8
  store i8 0, ptr %9
  %10 = getelementptr i8, ptr %9, i16 1
  store i8 1, ptr %10
  %11 = getelementptr i8, ptr %9, i16 2
  store i16 16, ptr %11
  %12 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 3, i16 9)
  %13 = getelementptr i8, ptr %12, i16 0
  store i16 0, ptr %13
  %14 = getelementptr i8, ptr %13, i16 2
  store i16 1, ptr %14
  %15 = getelementptr i8, ptr %13, i16 4
  store i16 2, ptr %15
  %16 = getelementptr i8, ptr %13, i16 6
  store i16 0, ptr %16
  %17 = getelementptr i8, ptr %13, i16 8
  store i8 1, ptr %17
  %18 = getelementptr i8, ptr %12, i16 9
  store i16 1, ptr %18
  %19 = getelementptr i8, ptr %18, i16 2
  store i16 -1, ptr %19
  %20 = getelementptr i8, ptr %18, i16 4
  store i16 -2, ptr %20
  %21 = getelementptr i8, ptr %18, i16 6
  store i16 1, ptr %21
  %22 = getelementptr i8, ptr %18, i16 8
  store i8 2, ptr %22
  %23 = getelementptr i8, ptr %12, i16 18
  store i16 2, ptr %23
  %24 = getelementptr i8, ptr %23, i16 2
  store i16 -3, ptr %24
  %25 = getelementptr i8, ptr %23, i16 4
  store i16 -4, ptr %25
  %26 = getelementptr i8, ptr %23, i16 6
  store i16 3, ptr %26
  %27 = getelementptr i8, ptr %23, i16 8
  store i8 2, ptr %27
  %28 = call addrspace(1) ptr @N$BGRW(ptr %1, i16 5, i16 3)
  %29 = getelementptr i8, ptr %28, i16 0
  store i16 0, ptr %29
  %30 = getelementptr i8, ptr %29, i16 2
  store i8 2, ptr %30
  %31 = getelementptr i8, ptr %28, i16 3
  store i16 1, ptr %31
  %32 = getelementptr i8, ptr %31, i16 2
  store i8 4, ptr %32
  %33 = getelementptr i8, ptr %28, i16 6
  store i16 1, ptr %33
  %34 = getelementptr i8, ptr %33, i16 2
  store i8 5, ptr %34
  %35 = getelementptr i8, ptr %28, i16 9
  store i16 2, ptr %35
  %36 = getelementptr i8, ptr %35, i16 2
  store i8 6, ptr %36
  %37 = getelementptr i8, ptr %28, i16 12
  store i16 2, ptr %37
  %38 = getelementptr i8, ptr %37, i16 2
  store i8 9, ptr %38
  store ptr %2, ptr addrspace(1) %0
  %39 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %12, ptr addrspace(1) %39
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %28, ptr addrspace(1) %40
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal void @run(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [6 x i8]
  %7 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @built(ptr addrspace(1) %9)
  %10 = getelementptr i8, ptr @$str2, i16 6
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 %12, ptr %5, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %12, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @Level.save(ptr addrspace(1) %8, ptr addrspace(1) %9, ptr addrspace(1) %16)
  %17 = load i8, ptr %7, !tbaa !2
  %18 = icmp eq i8 %17, 1
  br i1 %18, label %b2, label %b3

b2:
  %19 = getelementptr inbounds i8, ptr %7, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %7, i16 4
  %22 = load i16, ptr %21, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %7, i16 6
  %24 = load i16, ptr %23, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %20, ptr addrspace(1) %25
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %22, ptr addrspace(1) %26
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %24, ptr addrspace(1) %27
  %28 = getelementptr inbounds i8, ptr %6, i16 4
  %29 = load ptr, ptr %28, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %29)
  %30 = getelementptr inbounds i8, ptr %6, i16 2
  %31 = load ptr, ptr %30, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %31)
  %32 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %32)
  ret void

b3:
  %33 = getelementptr inbounds i8, ptr %6, i16 4
  %34 = load ptr, ptr %33, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %34)
  %35 = getelementptr inbounds i8, ptr %6, i16 2
  %36 = load ptr, ptr %35, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %36)
  %37 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %37)
  %38 = addrspacecast ptr %3 to ptr addrspace(1)
  %39 = load i16, ptr %11
  store i16 %39, ptr %2, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %39, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %13, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Level.load(ptr addrspace(1) %38, ptr addrspace(1) %42)
  %43 = load i8, ptr %3, !tbaa !2
  %44 = icmp eq i8 %43, 1
  br i1 %44, label %b4, label %b5

b4:
  %45 = getelementptr inbounds i8, ptr %3, i16 2
  %46 = load i16, ptr %45, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %3, i16 4
  %48 = load i16, ptr %47, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %3, i16 6
  %50 = load i16, ptr %49, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %46, ptr addrspace(1) %51
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %48, ptr addrspace(1) %52
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %50, ptr addrspace(1) %53
  ret void

b5:
  %54 = getelementptr inbounds i8, ptr %3, i16 2
  %55 = load ptr, ptr %54, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %3, i16 4
  %57 = load ptr, ptr %56, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %3, i16 6
  %59 = load ptr, ptr %58, !tbaa !2
  store ptr %55, ptr %4, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %4, i16 2
  store ptr %57, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr %59, ptr %61, !tbaa !2
  %62 = getelementptr i8, ptr %55, i16 -4
  %63 = load i16, ptr %62
  %64 = getelementptr i8, ptr %57, i16 -4
  %65 = load i16, ptr %64
  %66 = getelementptr i8, ptr %59, i16 -4
  %67 = load i16, ptr %66
  %68 = shl i16 %63, 2
  %69 = add i16 %68, 8
  %70 = mul i16 %65, 9
  %71 = add i16 %69, %70
  %72 = mul i16 %67, 3
  %73 = add i16 %71, %72
  call addrspace(1) void @N$PU2(i16 %63)
  %74 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %74)
  call addrspace(1) void @N$PU2(i16 %65)
  %75 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %75)
  call addrspace(1) void @N$PU2(i16 %67)
  %76 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  call addrspace(1) void @N$PU2(i16 %73)
  %77 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %77)
  call addrspace(1) void @N$PN()
  %78 = getelementptr i8, ptr @$str1, i16 6
  %79 = call addrspace(1) ptr @N$BGRW(ptr %78, i16 4, i16 4)
  %80 = getelementptr i8, ptr %79, i16 0
  store i16 40, ptr %80
  %81 = getelementptr i8, ptr %80, i16 2
  store i16 50, ptr %81
  %82 = getelementptr i8, ptr %79, i16 4
  store i16 40, ptr %82
  %83 = getelementptr i8, ptr %82, i16 2
  store i16 10, ptr %83
  %84 = getelementptr i8, ptr %79, i16 8
  store i16 8, ptr %84
  %85 = getelementptr i8, ptr %84, i16 2
  store i16 20, ptr %85
  %86 = getelementptr i8, ptr %79, i16 12
  store i16 8, ptr %86
  %87 = getelementptr i8, ptr %86, i16 2
  store i16 4, ptr %87
  %88 = getelementptr i8, ptr %79, i16 -4
  %89 = load i16, ptr %88
  %90 = addrspacecast ptr %4 to ptr addrspace(1)
  %91 = addrspacecast ptr %1 to ptr addrspace(1)
  %92 = getelementptr inbounds i8, ptr %1, i16 2
  %93 = getelementptr i8, ptr @$str7, i16 6
  %94 = getelementptr i8, ptr @$str8, i16 6
  %95 = getelementptr i8, ptr @$str9, i16 6
  %96 = getelementptr i8, ptr @$str10, i16 6
  %97 = getelementptr i8, ptr @$str11, i16 6
  %98 = getelementptr i8, ptr @$str12, i16 6
  br label %b6

b6:
  %99 = phi i16 [ 0, %b5 ], [ %136, %b15 ]
  %100 = icmp ult i16 %99, %89
  br i1 %100, label %b7, label %b9

b7:
  %101 = shl i16 %99, 2
  %102 = getelementptr i8, ptr %79, i16 %101
  %103 = getelementptr i8, ptr %102, i16 2
  %104 = addrspacecast ptr %103 to ptr addrspace(1)
  %105 = load i16, ptr %102
  %106 = load i16, ptr addrspace(1) %104
  %107 = call addrspace(1) i32 @Level.leaf(ptr addrspace(1) %90, i16 %105, i16 %106)
  store i32 %107, ptr addrspace(1) %91, !tbaa !2
  %108 = load i16, ptr %1, !tbaa !2
  %109 = load i16, ptr %92, !tbaa !2
  %110 = load ptr, ptr %60, !tbaa !2
  %111 = getelementptr i8, ptr %110, i16 -4
  %112 = load i16, ptr %111
  %113 = icmp ult i16 %109, %112
  br i1 %113, label %b10, label %b11

b9:
  store i8 0, ptr addrspace(1) %0
  call addrspace(1) void @N$BDRP(ptr %79)
  %114 = load ptr, ptr %61, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %114)
  %115 = load ptr, ptr %60, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %115)
  %116 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %116)
  ret void

b10:
  %117 = mul i16 %109, 9
  %118 = getelementptr i8, ptr %110, i16 %117
  %119 = addrspacecast ptr %118 to ptr addrspace(1)
  %120 = getelementptr i8, ptr addrspace(1) %119, i16 6
  %121 = load i16, ptr addrspace(1) %120
  %122 = load i16, ptr addrspace(1) %120
  %123 = getelementptr i8, ptr addrspace(1) %119, i16 8
  %124 = load i8, ptr addrspace(1) %123
  %125 = zext i8 %124 to i16
  %126 = add i16 %122, %125
  br label %b12

b11:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  %127 = phi ptr [ %78, %b10 ], [ %148, %b20 ]
  %128 = phi i16 [ %121, %b10 ], [ %149, %b20 ]
  %129 = icmp ult i16 %128, %126
  br i1 %129, label %b13, label %b15

b13:
  %130 = load ptr, ptr %61, !tbaa !2
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  %133 = icmp ult i16 %128, %132
  br i1 %133, label %b16, label %b17

b15:
  call addrspace(1) void @N$PS(ptr %94)
  %134 = load i16, ptr %102
  call addrspace(1) void @N$PI2(i16 %134)
  call addrspace(1) void @N$PS(ptr %95)
  %135 = load i16, ptr addrspace(1) %104
  call addrspace(1) void @N$PI2(i16 %135)
  call addrspace(1) void @N$PS(ptr %96)
  call addrspace(1) void @N$PU2(i16 %108)
  call addrspace(1) void @N$PS(ptr %97)
  call addrspace(1) void @N$PU2(i16 %109)
  call addrspace(1) void @N$PS(ptr %98)
  call addrspace(1) void @N$PS(ptr %127)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %127)
  %136 = add i16 %99, 1
  br label %b6

b16:
  %137 = mul i16 %128, 3
  %138 = getelementptr i8, ptr %130, i16 %137
  %139 = addrspacecast ptr %138 to ptr addrspace(1)
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PS(ptr %93)
  %140 = getelementptr i8, ptr addrspace(1) %139, i16 2
  %141 = load i8, ptr addrspace(1) %140
  %142 = and i8 %141, 1
  %143 = icmp ne i8 %142, 0
  br i1 %143, label %b20, label %b19

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  br label %b20

b20:
  %144 = phi i8 [ 98, %b16 ], [ 102, %b19 ]
  call addrspace(1) void @N$PC(i8 %144)
  %145 = load i8, ptr addrspace(1) %140
  %146 = lshr i8 %145, 1
  call addrspace(1) void @N$PU1(i8 %146)
  %147 = call addrspace(1) ptr @N$PEND()
  %148 = call addrspace(1) ptr @N$TCAT(ptr %127, ptr %147)
  call addrspace(1) void @N$BDRP(ptr %127)
  call addrspace(1) void @N$BDRP(ptr %147)
  %149 = add i16 %128, 1
  br label %b12
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @run(ptr addrspace(1) %1)
  %2 = load i8, ptr %0, !tbaa !2
  %3 = icmp eq i8 %2, 0
  br i1 %3, label %b4, label %b3

b2:
  ret i16 1

b3:
  %4 = icmp eq i8 %2, 1
  br i1 %4, label %b6, label %b5

b4:
  ret i16 0

b5:
  %5 = load i8, ptr %0, !tbaa !2
  %6 = icmp eq i8 %5, 1
  br i1 %6, label %b9, label %b8

b6:
  %7 = getelementptr inbounds i8, ptr %0, i16 2
  %8 = load i8, ptr %7, !tbaa !2
  %9 = icmp eq i8 %8, 0
  br i1 %9, label %b7, label %b5

b7:
  %10 = getelementptr i8, ptr @$str13, i16 6
  call addrspace(1) void @N$PS(ptr %10)
  call addrspace(1) void @N$PN()
  br label %b2

b8:
  %11 = getelementptr inbounds i8, ptr %0, i16 4
  %12 = load i16, ptr %11, !tbaa !2
  %13 = getelementptr i8, ptr @$str15, i16 6
  call addrspace(1) void @N$PS(ptr %13)
  call addrspace(1) void @N$PU2(i16 %12)
  %14 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %14)
  call addrspace(1) void @N$PN()
  br label %b2

b9:
  %15 = getelementptr inbounds i8, ptr %0, i16 2
  %16 = load i8, ptr %15, !tbaa !2
  %17 = icmp eq i8 %16, 1
  br i1 %17, label %b10, label %b8

b10:
  %18 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %18)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal void @"records[Face]"(ptr addrspace(1) %0, ptr addrspace(1) %1, i16 %2, ptr addrspace(1) %3) addrspace(1) {
b1:
  %4 = alloca [6 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [3 x i8]
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 3, i1 false)
  %7 = getelementptr i8, ptr @$str1, i16 6
  %8 = load i16, ptr addrspace(1) %3
  %9 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %10 = load i8, ptr addrspace(1) %9
  store i16 %8, ptr %6, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %6, i16 2
  store i8 %10, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %6 to ptr addrspace(1)
  %13 = addrspacecast ptr %5 to ptr addrspace(1)
  %14 = addrspacecast ptr %4 to ptr addrspace(1)
  %15 = getelementptr inbounds i8, ptr %4, i16 2
  %16 = getelementptr i8, ptr addrspace(1) %13, i16 2
  %17 = getelementptr i8, ptr addrspace(1) %13, i16 4
  %18 = getelementptr inbounds i8, ptr %4, i16 4
  %19 = getelementptr i8, ptr addrspace(1) %13, i16 6
  br label %b2

b2:
  %20 = phi ptr [ %7, %b1 ], [ %49, %b7 ]
  %21 = phi i16 [ 0, %b1 ], [ %55, %b7 ]
  %22 = icmp ult i16 %21, %2
  br i1 %22, label %b3, label %b5

b3:
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  call addrspace(1) void @std.io.File.read_raw(ptr addrspace(1) %14, ptr addrspace(1) %1, ptr addrspace(1) %12, i16 3)
  %23 = load i8, ptr %4
  %24 = icmp eq i8 %23, 1
  br i1 %24, label %25, label %28

25:
  %26 = load i16, ptr %15
  %27 = load i16, ptr %18
  store i8 1, ptr addrspace(1) %13
  store i8 0, ptr addrspace(1) %16
  store i16 %26, ptr addrspace(1) %17
  store i16 %27, ptr addrspace(1) %19
  br label %34

28:
  %29 = load i16, ptr %15
  %30 = icmp ult i16 %29, 3
  br i1 %30, label %31, label %33

31:
  %32 = sub i16 3, %29
  store i8 1, ptr addrspace(1) %13
  store i8 2, ptr addrspace(1) %16
  store i16 %32, ptr addrspace(1) %17
  br label %34

33:
  store i8 0, ptr addrspace(1) %13
  br label %34

34:
  %35 = load i8, ptr %5, !tbaa !2
  %36 = icmp eq i8 %35, 1
  br i1 %36, label %b6, label %b7

b5:
  store i8 0, ptr addrspace(1) %0
  %37 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %20, ptr addrspace(1) %37
  call addrspace(1) void @N$BDRP(ptr null)
  ret void

b6:
  %38 = getelementptr inbounds i8, ptr %5, i16 2
  %39 = load i16, ptr %38, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %5, i16 4
  %41 = load i16, ptr %40, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %5, i16 6
  %43 = load i16, ptr %42, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %44 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %39, ptr addrspace(1) %44
  %45 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %41, ptr addrspace(1) %45
  %46 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %43, ptr addrspace(1) %46
  call addrspace(1) void @N$BDRP(ptr %20)
  ret void

b7:
  %47 = getelementptr i8, ptr %20, i16 -4
  %48 = load i16, ptr %47
  %49 = call addrspace(1) ptr @N$BGRW(ptr %20, i16 1, i16 3)
  %50 = mul i16 %48, 3
  %51 = getelementptr i8, ptr %49, i16 %50
  %52 = load i16, ptr %6, !tbaa !2
  %53 = load i8, ptr %11, !tbaa !2
  store i16 %52, ptr %51
  %54 = getelementptr i8, ptr %51, i16 2
  store i8 %53, ptr %54
  %55 = add i16 %21, 1
  br label %b2
}

define internal void @"records[Node]"(ptr addrspace(1) %0, ptr addrspace(1) %1, i16 %2, ptr addrspace(1) %3) addrspace(1) {
b1:
  %4 = alloca [6 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [9 x i8]
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 9, i1 false)
  %7 = getelementptr i8, ptr @$str1, i16 6
  %8 = load i16, ptr addrspace(1) %3
  %9 = getelementptr i8, ptr addrspace(1) %3, i16 6
  %10 = load i16, ptr addrspace(1) %9
  %11 = getelementptr i8, ptr addrspace(1) %3, i16 8
  %12 = load i8, ptr addrspace(1) %11
  store i16 %8, ptr %6, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %6, i16 2
  %14 = addrspacecast ptr %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %3, i16 2
  br label %b2

b2:
  %16 = phi i16 [ 0, %b1 ], [ %22, %b3 ]
  %17 = icmp slt i16 %16, 2
  br i1 %17, label %b3, label %b5

b3:
  %18 = shl i16 %16, 1
  %19 = getelementptr i8, ptr addrspace(1) %14, i16 %18
  %20 = getelementptr i8, ptr addrspace(1) %15, i16 %18
  %21 = load i16, ptr addrspace(1) %20
  store i16 %21, ptr addrspace(1) %19
  %22 = add i16 %16, 1
  br label %b2

b5:
  %23 = getelementptr inbounds i8, ptr %6, i16 6
  store i16 %10, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %6, i16 8
  store i8 %12, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %6 to ptr addrspace(1)
  %26 = addrspacecast ptr %5 to ptr addrspace(1)
  %27 = addrspacecast ptr %4 to ptr addrspace(1)
  %28 = getelementptr inbounds i8, ptr %4, i16 2
  %29 = getelementptr i8, ptr addrspace(1) %26, i16 2
  %30 = getelementptr i8, ptr addrspace(1) %26, i16 4
  %31 = getelementptr inbounds i8, ptr %4, i16 4
  %32 = getelementptr i8, ptr addrspace(1) %26, i16 6
  br label %b6

b6:
  %33 = phi ptr [ %7, %b5 ], [ %62, %b15 ]
  %34 = phi i16 [ 0, %b5 ], [ %79, %b15 ]
  %35 = icmp ult i16 %34, %2
  br i1 %35, label %b7, label %b9

b7:
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  call addrspace(1) void @std.io.File.read_raw(ptr addrspace(1) %27, ptr addrspace(1) %1, ptr addrspace(1) %25, i16 9)
  %36 = load i8, ptr %4
  %37 = icmp eq i8 %36, 1
  br i1 %37, label %38, label %41

38:
  %39 = load i16, ptr %28
  %40 = load i16, ptr %31
  store i8 1, ptr addrspace(1) %26
  store i8 0, ptr addrspace(1) %29
  store i16 %39, ptr addrspace(1) %30
  store i16 %40, ptr addrspace(1) %32
  br label %47

41:
  %42 = load i16, ptr %28
  %43 = icmp ult i16 %42, 9
  br i1 %43, label %44, label %46

44:
  %45 = sub i16 9, %42
  store i8 1, ptr addrspace(1) %26
  store i8 2, ptr addrspace(1) %29
  store i16 %45, ptr addrspace(1) %30
  br label %47

46:
  store i8 0, ptr addrspace(1) %26
  br label %47

47:
  %48 = load i8, ptr %5, !tbaa !2
  %49 = icmp eq i8 %48, 1
  br i1 %49, label %b10, label %b11

b9:
  store i8 0, ptr addrspace(1) %0
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %33, ptr addrspace(1) %50
  call addrspace(1) void @N$BDRP(ptr null)
  ret void

b10:
  %51 = getelementptr inbounds i8, ptr %5, i16 2
  %52 = load i16, ptr %51, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %5, i16 4
  %54 = load i16, ptr %53, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %5, i16 6
  %56 = load i16, ptr %55, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %52, ptr addrspace(1) %57
  %58 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %54, ptr addrspace(1) %58
  %59 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %56, ptr addrspace(1) %59
  call addrspace(1) void @N$BDRP(ptr %33)
  ret void

b11:
  %60 = getelementptr i8, ptr %33, i16 -4
  %61 = load i16, ptr %60
  %62 = call addrspace(1) ptr @N$BGRW(ptr %33, i16 1, i16 9)
  %63 = mul i16 %61, 9
  %64 = getelementptr i8, ptr %62, i16 %63
  %65 = load i16, ptr %6, !tbaa !2
  %66 = load i16, ptr %23, !tbaa !2
  %67 = load i8, ptr %24, !tbaa !2
  store i16 %65, ptr %64
  %68 = getelementptr i8, ptr %64, i16 2
  %69 = addrspacecast ptr %68 to ptr addrspace(1)
  br label %b12

b12:
  %70 = phi i16 [ 0, %b11 ], [ %76, %b13 ]
  %71 = icmp slt i16 %70, 2
  br i1 %71, label %b13, label %b15

b13:
  %72 = shl i16 %70, 1
  %73 = getelementptr i8, ptr addrspace(1) %69, i16 %72
  %74 = getelementptr i8, ptr addrspace(1) %14, i16 %72
  %75 = load i16, ptr addrspace(1) %74
  store i16 %75, ptr addrspace(1) %73
  %76 = add i16 %70, 1
  br label %b12

b15:
  %77 = getelementptr i8, ptr %64, i16 6
  store i16 %66, ptr %77
  %78 = getelementptr i8, ptr %64, i16 8
  store i8 %67, ptr %78
  %79 = add i16 %34, 1
  br label %b6
}

define internal void @"records[Plane]"(ptr addrspace(1) %0, ptr addrspace(1) %1, i16 %2, ptr addrspace(1) %3) addrspace(1) {
b1:
  %4 = alloca [6 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  %7 = getelementptr i8, ptr @$str1, i16 6
  %8 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %9 = load i16, ptr addrspace(1) %8
  %10 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %b2

b2:
  %11 = phi i16 [ 0, %b1 ], [ %16, %b3 ]
  %12 = icmp slt i16 %11, 2
  br i1 %12, label %b3, label %b5

b3:
  %13 = getelementptr i8, ptr addrspace(1) %10, i16 %11
  %14 = getelementptr i8, ptr addrspace(1) %3, i16 %11
  %15 = load i8, ptr addrspace(1) %14
  store i8 %15, ptr addrspace(1) %13
  %16 = add i16 %11, 1
  br label %b2

b5:
  %17 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %9, ptr %17, !tbaa !2
  %18 = addrspacecast ptr %5 to ptr addrspace(1)
  %19 = addrspacecast ptr %4 to ptr addrspace(1)
  %20 = getelementptr inbounds i8, ptr %4, i16 2
  %21 = getelementptr i8, ptr addrspace(1) %18, i16 2
  %22 = getelementptr i8, ptr addrspace(1) %18, i16 4
  %23 = getelementptr inbounds i8, ptr %4, i16 4
  %24 = getelementptr i8, ptr addrspace(1) %18, i16 6
  br label %b6

b6:
  %25 = phi ptr [ %7, %b5 ], [ %54, %b15 ]
  %26 = phi i16 [ 0, %b5 ], [ %66, %b15 ]
  %27 = icmp ult i16 %26, %2
  br i1 %27, label %b7, label %b9

b7:
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  call addrspace(1) void @std.io.File.read_raw(ptr addrspace(1) %19, ptr addrspace(1) %1, ptr addrspace(1) %10, i16 4)
  %28 = load i8, ptr %4
  %29 = icmp eq i8 %28, 1
  br i1 %29, label %30, label %33

30:
  %31 = load i16, ptr %20
  %32 = load i16, ptr %23
  store i8 1, ptr addrspace(1) %18
  store i8 0, ptr addrspace(1) %21
  store i16 %31, ptr addrspace(1) %22
  store i16 %32, ptr addrspace(1) %24
  br label %39

33:
  %34 = load i16, ptr %20
  %35 = icmp ult i16 %34, 4
  br i1 %35, label %36, label %38

36:
  %37 = sub i16 4, %34
  store i8 1, ptr addrspace(1) %18
  store i8 2, ptr addrspace(1) %21
  store i16 %37, ptr addrspace(1) %22
  br label %39

38:
  store i8 0, ptr addrspace(1) %18
  br label %39

39:
  %40 = load i8, ptr %5, !tbaa !2
  %41 = icmp eq i8 %40, 1
  br i1 %41, label %b10, label %b11

b9:
  store i8 0, ptr addrspace(1) %0
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %25, ptr addrspace(1) %42
  call addrspace(1) void @N$BDRP(ptr null)
  ret void

b10:
  %43 = getelementptr inbounds i8, ptr %5, i16 2
  %44 = load i16, ptr %43, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %5, i16 4
  %46 = load i16, ptr %45, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %5, i16 6
  %48 = load i16, ptr %47, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %44, ptr addrspace(1) %49
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %46, ptr addrspace(1) %50
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %48, ptr addrspace(1) %51
  call addrspace(1) void @N$BDRP(ptr %25)
  ret void

b11:
  %52 = getelementptr i8, ptr %25, i16 -4
  %53 = load i16, ptr %52
  %54 = call addrspace(1) ptr @N$BGRW(ptr %25, i16 1, i16 4)
  %55 = shl i16 %53, 2
  %56 = getelementptr i8, ptr %54, i16 %55
  %57 = load i16, ptr %17, !tbaa !2
  %58 = addrspacecast ptr %56 to ptr addrspace(1)
  br label %b12

b12:
  %59 = phi i16 [ 0, %b11 ], [ %64, %b13 ]
  %60 = icmp slt i16 %59, 2
  br i1 %60, label %b13, label %b15

b13:
  %61 = getelementptr i8, ptr addrspace(1) %58, i16 %59
  %62 = getelementptr i8, ptr addrspace(1) %10, i16 %59
  %63 = load i8, ptr addrspace(1) %62
  store i8 %63, ptr addrspace(1) %61
  %64 = add i16 %59, 1
  br label %b12

b15:
  %65 = getelementptr i8, ptr %56, i16 2
  store i16 %57, ptr %65
  %66 = add i16 %26, 1
  br label %b6
}

define internal void @"written[Face]"(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  %4 = load ptr, ptr addrspace(1) %2
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  %7 = getelementptr i8, ptr %4, i16 -4
  %8 = load i16, ptr %7
  %9 = mul i16 %8, 3
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %6, ptr addrspace(1) %1, ptr addrspace(1) %5, i16 %9)
  %10 = load i8, ptr %3, !tbaa !2
  %11 = icmp eq i8 %10, 1
  br i1 %11, label %b2, label %b3

b2:
  %12 = getelementptr inbounds i8, ptr %3, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %3, i16 4
  %15 = load i16, ptr %14, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %16
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %13, ptr addrspace(1) %17
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %15, ptr addrspace(1) %18
  ret void

b3:
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal void @"written[Node]"(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  %4 = load ptr, ptr addrspace(1) %2
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  %7 = getelementptr i8, ptr %4, i16 -4
  %8 = load i16, ptr %7
  %9 = mul i16 %8, 9
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %6, ptr addrspace(1) %1, ptr addrspace(1) %5, i16 %9)
  %10 = load i8, ptr %3, !tbaa !2
  %11 = icmp eq i8 %10, 1
  br i1 %11, label %b2, label %b3

b2:
  %12 = getelementptr inbounds i8, ptr %3, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %3, i16 4
  %15 = load i16, ptr %14, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %16
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %13, ptr addrspace(1) %17
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %15, ptr addrspace(1) %18
  ret void

b3:
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal void @"written[Plane]"(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  %4 = load ptr, ptr addrspace(1) %2
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  %7 = getelementptr i8, ptr %4, i16 -4
  %8 = load i16, ptr %7
  %9 = shl i16 %8, 2
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %6, ptr addrspace(1) %1, ptr addrspace(1) %5, i16 %9)
  %10 = load i8, ptr %3, !tbaa !2
  %11 = icmp eq i8 %10, 1
  br i1 %11, label %b2, label %b3

b2:
  %12 = getelementptr inbounds i8, ptr %3, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %3, i16 4
  %15 = load i16, ptr %14, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %16
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %13, ptr addrspace(1) %17
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %15, ptr addrspace(1) %18
  ret void

b3:
  store i8 0, ptr addrspace(1) %0
  ret void
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

declare void @N$PU2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$PC(i8) addrspace(1)

declare void @N$PU1(i8) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare ptr @N$TCAT(ptr, ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
