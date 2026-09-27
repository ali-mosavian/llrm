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

define internal i32 @std.io.error(i16 %0) addrspace(1) {
b1:
  %1 = alloca i8
  %2 = alloca i16
  %3 = alloca [4 x i8]
  store i8 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  %4 = sub i16 0, %0
  store i16 %4, ptr %2, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp eq i16 %5, 2
  %7 = sext i1 %6 to i8
  store i8 %7, ptr %1, !tbaa !2
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b3, label %b2

b2:
  %9 = load i16, ptr %2, !tbaa !2
  %10 = icmp eq i16 %9, 3
  %11 = sext i1 %10 to i8
  store i8 %11, ptr %1, !tbaa !2
  br label %b3

b3:
  %12 = load i8, ptr %1, !tbaa !2
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b4, label %b5

b4:
  store i8 0, ptr %3, !tbaa !2
  %14 = addrspacecast ptr %3 to ptr addrspace(1)
  %15 = load i32, ptr addrspace(1) %14, !tbaa !2
  ret i32 %15

b5:
  br label %b6

b6:
  %16 = load i16, ptr %2, !tbaa !2
  %17 = icmp eq i16 %16, 5
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b7, label %b8

b7:
  store i8 1, ptr %3, !tbaa !2
  %20 = addrspacecast ptr %3 to ptr addrspace(1)
  %21 = load i32, ptr addrspace(1) %20, !tbaa !2
  ret i32 %21

b8:
  br label %b9

b9:
  %22 = load i16, ptr %2, !tbaa !2
  store i8 2, ptr %3, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %22, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %3 to ptr addrspace(1)
  %25 = load i32, ptr addrspace(1) %24, !tbaa !2
  ret i32 %25
}

define internal void @std.io.named(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %2, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp ult i16 %5, %4
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b3, label %b5

b3:
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %10 = load ptr addrspace(1), ptr addrspace(1) %9
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 %5
  %12 = load i16, ptr %3, !tbaa !2
  %13 = icmp ult i16 %12, 79
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b6, label %b7

b4:
  %16 = load i16, ptr %2, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %2, !tbaa !2
  br label %b2

b5:
  %18 = load i16, ptr %3, !tbaa !2
  %19 = icmp ult i16 %18, 80
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b11, label %b12

b6:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = icmp ult i16 %22, 80
  %24 = sext i1 %23 to i8
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b9, label %b10

b7:
  br label %b8

b8:
  br label %b4

b9:
  %26 = getelementptr i8, ptr addrspace(1) %1, i16 %22
  %27 = load i8, ptr addrspace(1) %11
  store i8 %27, ptr addrspace(1) %26
  %28 = load i16, ptr %3, !tbaa !2
  %29 = add i16 %28, 1
  store i16 %29, ptr %3, !tbaa !2
  br label %b8

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  %30 = getelementptr i8, ptr addrspace(1) %1, i16 %18
  store i8 0, ptr addrspace(1) %30
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @std.io.opened(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [4 x i8]
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  %5 = icmp slt i16 %1, 0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  %8 = call addrspace(1) i32 @std.io.error(i16 %1)
  %9 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %8, ptr addrspace(1) %9, !tbaa !2
  %10 = load i16, ptr %4, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %4, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %10, ptr addrspace(1) %13
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %12, ptr addrspace(1) %14
  ret void

b3:
  br label %b4

b4:
  store i8 0, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %1, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr %3, !tbaa !2
  store i16 128, ptr %2, !tbaa !2
  br label %b5

b5:
  %17 = load i16, ptr %3, !tbaa !2
  %18 = load i16, ptr %2, !tbaa !2
  %19 = icmp slt i16 %17, %18
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b6, label %b8

b6:
  %22 = load i16, ptr %3, !tbaa !2
  %23 = getelementptr i8, ptr addrspace(1) %16, i16 %22
  store i8 0, ptr addrspace(1) %23
  br label %b7

b7:
  %24 = load i16, ptr %3, !tbaa !2
  %25 = add i16 %24, 1
  store i16 %25, ptr %3, !tbaa !2
  br label %b5

b8:
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 0, ptr addrspace(1) %26
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 0, ptr addrspace(1) %27
  ret void
}

define internal void @std.io.File.open(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i8 %2) addrspace(1) {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca [136 x i8]
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca [80 x i8]
  %11 = alloca i8
  store i16 0, ptr %3
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 136, i1 false)
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 80, i1 false)
  store i8 0, ptr %11
  store i8 0, ptr %11, !tbaa !2
  store i16 80, ptr %8, !tbaa !2
  store i16 80, ptr %9, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 80, ptr %6, !tbaa !2
  br label %b2

b2:
  %12 = load i16, ptr %7, !tbaa !2
  %13 = load i16, ptr %6, !tbaa !2
  %14 = icmp slt i16 %12, %13
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b3, label %b5

b3:
  %17 = load i16, ptr %7, !tbaa !2
  %18 = load i8, ptr %11, !tbaa !2
  %19 = sub i16 %17, 0
  %20 = getelementptr inbounds i8, ptr %10, i16 %19
  store i8 %18, ptr %20, !tbaa !2
  br label %b4

b4:
  %21 = load i16, ptr %7, !tbaa !2
  %22 = add i16 %21, 1
  store i16 %22, ptr %7, !tbaa !2
  br label %b2

b5:
  %23 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @std.io.named(ptr addrspace(1) %1, ptr addrspace(1) %23)
  %24 = addrspacecast ptr %5 to ptr addrspace(1)
  %25 = addrspacecast ptr %10 to ptr addrspace(1)
  %26 = call addrspace(1) i16 @N$OOPN(ptr addrspace(1) %25, i8 %2)
  call addrspace(1) void @std.io.opened(ptr addrspace(1) %24, i16 %26)
  %27 = load i16, ptr %5, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %5, i16 2
  %29 = load i16, ptr %28, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %5, i16 132
  %31 = load i16, ptr %30, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %5, i16 134
  %33 = load i16, ptr %32, !tbaa !2
  store i16 %27, ptr addrspace(1) %0
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %29, ptr addrspace(1) %34
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %36 = getelementptr inbounds i8, ptr %5, i16 4
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  store i16 0, ptr %4, !tbaa !2
  store i16 64, ptr %3, !tbaa !2
  br label %b6

b6:
  %38 = load i16, ptr %4, !tbaa !2
  %39 = load i16, ptr %3, !tbaa !2
  %40 = icmp slt i16 %38, %39
  %41 = sext i1 %40 to i8
  %42 = icmp ne i8 %41, 0
  br i1 %42, label %b7, label %b9

b7:
  %43 = load i16, ptr %4, !tbaa !2
  %44 = mul i16 %43, 2
  %45 = getelementptr i8, ptr addrspace(1) %35, i16 %44
  %46 = load i16, ptr %4, !tbaa !2
  %47 = mul i16 %46, 2
  %48 = getelementptr i8, ptr addrspace(1) %37, i16 %47
  %49 = load i16, ptr addrspace(1) %48
  store i16 %49, ptr addrspace(1) %45
  br label %b8

b8:
  %50 = load i16, ptr %4, !tbaa !2
  %51 = add i16 %50, 1
  store i16 %51, ptr %4, !tbaa !2
  br label %b6

b9:
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 %31, ptr addrspace(1) %52
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %33, ptr addrspace(1) %53
  ret void
}

define internal void @std.io.File.create(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [136 x i8]
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [80 x i8]
  %10 = alloca i8
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 136, i1 false)
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 80, i1 false)
  store i8 0, ptr %10
  store i8 0, ptr %10, !tbaa !2
  store i16 80, ptr %7, !tbaa !2
  store i16 80, ptr %8, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i16 80, ptr %5, !tbaa !2
  br label %b2

b2:
  %11 = load i16, ptr %6, !tbaa !2
  %12 = load i16, ptr %5, !tbaa !2
  %13 = icmp slt i16 %11, %12
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b3, label %b5

b3:
  %16 = load i16, ptr %6, !tbaa !2
  %17 = load i8, ptr %10, !tbaa !2
  %18 = sub i16 %16, 0
  %19 = getelementptr inbounds i8, ptr %9, i16 %18
  store i8 %17, ptr %19, !tbaa !2
  br label %b4

b4:
  %20 = load i16, ptr %6, !tbaa !2
  %21 = add i16 %20, 1
  store i16 %21, ptr %6, !tbaa !2
  br label %b2

b5:
  %22 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @std.io.named(ptr addrspace(1) %1, ptr addrspace(1) %22)
  %23 = addrspacecast ptr %4 to ptr addrspace(1)
  %24 = addrspacecast ptr %9 to ptr addrspace(1)
  %25 = call addrspace(1) i16 @N$OCRE(ptr addrspace(1) %24)
  call addrspace(1) void @std.io.opened(ptr addrspace(1) %23, i16 %25)
  %26 = load i16, ptr %4, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %4, i16 2
  %28 = load i16, ptr %27, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %4, i16 132
  %30 = load i16, ptr %29, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %4, i16 134
  %32 = load i16, ptr %31, !tbaa !2
  store i16 %26, ptr addrspace(1) %0
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %28, ptr addrspace(1) %33
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %35 = getelementptr inbounds i8, ptr %4, i16 4
  %36 = addrspacecast ptr %35 to ptr addrspace(1)
  store i16 0, ptr %3, !tbaa !2
  store i16 64, ptr %2, !tbaa !2
  br label %b6

b6:
  %37 = load i16, ptr %3, !tbaa !2
  %38 = load i16, ptr %2, !tbaa !2
  %39 = icmp slt i16 %37, %38
  %40 = sext i1 %39 to i8
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b7, label %b9

b7:
  %42 = load i16, ptr %3, !tbaa !2
  %43 = mul i16 %42, 2
  %44 = getelementptr i8, ptr addrspace(1) %34, i16 %43
  %45 = load i16, ptr %3, !tbaa !2
  %46 = mul i16 %45, 2
  %47 = getelementptr i8, ptr addrspace(1) %36, i16 %46
  %48 = load i16, ptr addrspace(1) %47
  store i16 %48, ptr addrspace(1) %44
  br label %b8

b8:
  %49 = load i16, ptr %3, !tbaa !2
  %50 = add i16 %49, 1
  store i16 %50, ptr %3, !tbaa !2
  br label %b6

b9:
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 %30, ptr addrspace(1) %51
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %32, ptr addrspace(1) %52
  ret void
}

define internal void @std.io.File.write_raw(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca [4 x i8]
  %5 = alloca i16
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  store i16 0, ptr %5
  %6 = load i16, ptr addrspace(1) %1
  %7 = call addrspace(1) i16 @N$OWRI(i16 %6, ptr addrspace(1) %2, i16 %3)
  store i16 %7, ptr %5, !tbaa !2
  %8 = load i16, ptr %5, !tbaa !2
  %9 = icmp slt i16 %8, 0
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  %12 = load i16, ptr %5, !tbaa !2
  %13 = call addrspace(1) i32 @std.io.error(i16 %12)
  %14 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %13, ptr addrspace(1) %14, !tbaa !2
  %15 = load i16, ptr %4, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %4, i16 2
  %17 = load i16, ptr %16, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %15, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %17, ptr addrspace(1) %19
  ret void

b3:
  br label %b4

b4:
  %20 = load i16, ptr %5, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %20, ptr addrspace(1) %21
  ret void
}

define internal void @std.io.File.write(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store ptr addrspace(1) null, ptr %4
  %5 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load i16, ptr addrspace(1) %2
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %9)
  %10 = load i16, ptr %3, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %3, i16 4
  %14 = load i16, ptr %13, !tbaa !2
  store i16 %10, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %12, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %14, ptr addrspace(1) %16
  ret void
}

define internal void @std.io.File.read_raw(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca [4 x i8]
  %5 = alloca i16
  %6 = alloca i8
  %7 = alloca i16
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  store i16 0, ptr %5
  store i8 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %7, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %7, !tbaa !2
  %9 = icmp ult i16 %8, %3
  %10 = sext i1 %9 to i8
  store i8 %10, ptr %6, !tbaa !2
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b5, label %b6

b3:
  %12 = load i16, ptr %7, !tbaa !2
  %13 = mul i16 %12, 1
  %14 = getelementptr i8, ptr addrspace(1) %2, i16 %13
  %15 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %16 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %17 = load i16, ptr addrspace(1) %16
  %18 = getelementptr i8, ptr addrspace(1) %15, i16 %17
  %19 = load i8, ptr addrspace(1) %18
  store i8 %19, ptr addrspace(1) %14
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %21 = load i16, ptr addrspace(1) %20
  %22 = add i16 %21, 1
  %23 = getelementptr i8, ptr addrspace(1) %1, i16 130
  store i16 %22, ptr addrspace(1) %23
  %24 = load i16, ptr %7, !tbaa !2
  %25 = add i16 %24, 1
  store i16 %25, ptr %7, !tbaa !2
  br label %b2

b4:
  %26 = load i16, ptr %7, !tbaa !2
  %27 = icmp ult i16 %26, %3
  %28 = sext i1 %27 to i8
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b7, label %b8

b5:
  %30 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %31 = load i16, ptr addrspace(1) %30
  %32 = getelementptr i8, ptr addrspace(1) %1, i16 132
  %33 = load i16, ptr addrspace(1) %32
  %34 = icmp ult i16 %31, %33
  %35 = sext i1 %34 to i8
  store i8 %35, ptr %6, !tbaa !2
  br label %b6

b6:
  %36 = load i8, ptr %6, !tbaa !2
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b3, label %b4

b7:
  %38 = load i16, ptr addrspace(1) %1
  %39 = load i16, ptr %7, !tbaa !2
  %40 = mul i16 %39, 1
  %41 = getelementptr i8, ptr addrspace(1) %2, i16 %40
  %42 = load i16, ptr %7, !tbaa !2
  %43 = sub i16 %3, %42
  %44 = call addrspace(1) i16 @N$OREA(i16 %38, ptr addrspace(1) %41, i16 %43)
  store i16 %44, ptr %5, !tbaa !2
  %45 = load i16, ptr %5, !tbaa !2
  %46 = icmp slt i16 %45, 0
  %47 = sext i1 %46 to i8
  %48 = icmp ne i8 %47, 0
  br i1 %48, label %b10, label %b11

b8:
  br label %b9

b9:
  %49 = load i16, ptr %7, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %49, ptr addrspace(1) %50
  ret void

b10:
  %51 = load i16, ptr %5, !tbaa !2
  %52 = call addrspace(1) i32 @std.io.error(i16 %51)
  %53 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %52, ptr addrspace(1) %53, !tbaa !2
  %54 = load i16, ptr %4, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %4, i16 2
  %56 = load i16, ptr %55, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %54, ptr addrspace(1) %57
  %58 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %56, ptr addrspace(1) %58
  ret void

b11:
  br label %b12

b12:
  %59 = load i16, ptr %7, !tbaa !2
  %60 = load i16, ptr %5, !tbaa !2
  %61 = add i16 %59, %60
  store i16 %61, ptr %7, !tbaa !2
  br label %b9
}

define internal void @std.io.File.read_line(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca i16
  %6 = alloca ptr addrspace(1)
  %7 = alloca i8
  %8 = alloca ptr
  store i8 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 4, i1 false)
  store i16 0, ptr %5
  store ptr addrspace(1) null, ptr %6
  store i8 0, ptr %7
  store ptr null, ptr %8
  %9 = getelementptr i8, ptr @$str1, i16 6
  store ptr %9, ptr %8, !tbaa !2
  store i8 0, ptr %7, !tbaa !2
  br label %b2

b2:
  br label %b3

b3:
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %11 = load i16, ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 132
  %13 = load i16, ptr addrspace(1) %12
  %14 = icmp eq i16 %11, %13
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b5, label %b6

b5:
  %17 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr addrspace(1) %17, ptr %6, !tbaa !2
  %18 = load i16, ptr addrspace(1) %1
  %19 = load ptr addrspace(1), ptr %6, !tbaa !2
  %20 = call addrspace(1) i16 @N$OREA(i16 %18, ptr addrspace(1) %19, i16 128)
  store i16 %20, ptr %5, !tbaa !2
  %21 = load i16, ptr %5, !tbaa !2
  %22 = icmp slt i16 %21, 0
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b8, label %b9

b6:
  br label %b7

b7:
  %25 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %26 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %27 = load i16, ptr addrspace(1) %26
  %28 = icmp ult i16 %27, 128
  %29 = sext i1 %28 to i8
  %30 = icmp ne i8 %29, 0
  br i1 %30, label %b17, label %b18

b8:
  %31 = load i16, ptr %5, !tbaa !2
  %32 = call addrspace(1) i32 @std.io.error(i16 %31)
  %33 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %32, ptr addrspace(1) %33, !tbaa !2
  %34 = load i16, ptr %4, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %4, i16 2
  %36 = load i16, ptr %35, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %37 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %34, ptr addrspace(1) %37
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %36, ptr addrspace(1) %38
  %39 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %39)
  ret void

b9:
  br label %b10

b10:
  %40 = load i16, ptr %5, !tbaa !2
  %41 = icmp eq i16 %40, 0
  %42 = sext i1 %41 to i8
  %43 = icmp ne i8 %42, 0
  br i1 %43, label %b11, label %b12

b11:
  %44 = load i8, ptr %7, !tbaa !2
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %b14, label %b15

b12:
  br label %b13

b13:
  %46 = getelementptr i8, ptr addrspace(1) %1, i16 130
  store i16 0, ptr addrspace(1) %46
  %47 = load i16, ptr %5, !tbaa !2
  %48 = getelementptr i8, ptr addrspace(1) %1, i16 132
  store i16 %47, ptr addrspace(1) %48
  br label %b7

b14:
  %49 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  store i8 0, ptr %3, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %3, i16 2
  store ptr %49, ptr %50, !tbaa !2
  br label %b16

b15:
  store i8 1, ptr %3, !tbaa !2
  br label %b16

b16:
  %51 = load i16, ptr %3, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %3, i16 2
  %53 = load i16, ptr %52, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %51, ptr addrspace(1) %54
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %53, ptr addrspace(1) %55
  %56 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %56)
  ret void

b17:
  %57 = getelementptr i8, ptr addrspace(1) %25, i16 %27
  %58 = load i8, ptr addrspace(1) %57
  store i8 %58, ptr %2, !tbaa !2
  %59 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %60 = load i16, ptr addrspace(1) %59
  %61 = add i16 %60, 1
  %62 = getelementptr i8, ptr addrspace(1) %1, i16 130
  store i16 %61, ptr addrspace(1) %62
  store i8 -1, ptr %7, !tbaa !2
  %63 = load i8, ptr %2, !tbaa !2
  %64 = zext i8 %63 to i16
  %65 = icmp eq i16 %64, 10
  %66 = sext i1 %65 to i8
  %67 = icmp ne i8 %66, 0
  br i1 %67, label %b19, label %b20

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %68 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %69 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %69
  %70 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %68, ptr addrspace(1) %70
  %71 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %71)
  ret void

b20:
  br label %b21

b21:
  %72 = load i8, ptr %2, !tbaa !2
  %73 = zext i8 %72 to i16
  %74 = icmp ne i16 %73, 13
  %75 = sext i1 %74 to i8
  %76 = icmp ne i8 %75, 0
  br i1 %76, label %b22, label %b23

b22:
  %77 = load ptr, ptr %8, !tbaa !2
  %78 = getelementptr i8, ptr %77, i16 -4
  %79 = load i16, ptr %78
  %80 = call addrspace(1) ptr @N$BGRW(ptr %77, i16 1, i16 1)
  store ptr %80, ptr %8, !tbaa !2
  %81 = getelementptr i8, ptr %80, i16 %79
  %82 = load i8, ptr %2, !tbaa !2
  store i8 %82, ptr %81
  %83 = getelementptr i8, ptr %80, i16 -4
  %84 = load i16, ptr %83
  %85 = getelementptr i8, ptr %80, i16 %84
  store i8 0, ptr %85
  br label %b24

b23:
  br label %b24

b24:
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
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i8
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [8 x i8]
  %10 = alloca [136 x i8]
  %11 = alloca [134 x i8]
  %12 = alloca ptr
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i8 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 134, i1 false)
  store ptr null, ptr %12
  store ptr %1, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  %14 = load ptr, ptr %12, !tbaa !2
  %15 = getelementptr i8, ptr %14, i16 -4
  %16 = load i16, ptr %15
  %17 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 %16, ptr %9, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %16, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %17, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.create(ptr addrspace(1) %13, ptr addrspace(1) %20)
  %21 = load i8, ptr %10, !tbaa !2
  %22 = icmp eq i8 %21, 1
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b2, label %b3

b2:
  %25 = getelementptr inbounds i8, ptr %10, i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %10, i16 4
  %28 = load i16, ptr %27, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %26, ptr addrspace(1) %29
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %28, ptr addrspace(1) %30
  %31 = load ptr, ptr %12, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %31)
  ret void

b3:
  %32 = getelementptr inbounds i8, ptr %10, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %10, i16 132
  %35 = load i16, ptr %34, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %10, i16 134
  %37 = load i16, ptr %36, !tbaa !2
  store i16 %33, ptr %11, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %11, i16 2
  %39 = addrspacecast ptr %38 to ptr addrspace(1)
  %40 = getelementptr inbounds i8, ptr %10, i16 4
  %41 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 0, ptr %8, !tbaa !2
  store i16 128, ptr %7, !tbaa !2
  br label %b4

b4:
  %42 = load i16, ptr %8, !tbaa !2
  %43 = load i16, ptr %7, !tbaa !2
  %44 = icmp slt i16 %42, %43
  %45 = sext i1 %44 to i8
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b5, label %b7

b5:
  %47 = load i16, ptr %8, !tbaa !2
  %48 = getelementptr i8, ptr addrspace(1) %39, i16 %47
  %49 = load i16, ptr %8, !tbaa !2
  %50 = getelementptr i8, ptr addrspace(1) %41, i16 %49
  %51 = load i8, ptr addrspace(1) %50
  store i8 %51, ptr addrspace(1) %48
  br label %b6

b6:
  %52 = load i16, ptr %8, !tbaa !2
  %53 = add i16 %52, 1
  store i16 %53, ptr %8, !tbaa !2
  br label %b4

b7:
  %54 = getelementptr inbounds i8, ptr %11, i16 130
  store i16 %35, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %11, i16 132
  store i16 %37, ptr %55, !tbaa !2
  store i8 -1, ptr %6, !tbaa !2
  %56 = load ptr, ptr %12, !tbaa !2
  store ptr null, ptr %12, !tbaa !2
  %57 = load i16, ptr %11, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %11, i16 130
  %59 = load i16, ptr %58, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %11, i16 132
  %61 = load i16, ptr %60, !tbaa !2
  store i8 0, ptr %6, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %56, ptr addrspace(1) %62
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %57, ptr addrspace(1) %63
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %65 = getelementptr inbounds i8, ptr %11, i16 2
  %66 = addrspacecast ptr %65 to ptr addrspace(1)
  store i16 0, ptr %5, !tbaa !2
  store i16 128, ptr %4, !tbaa !2
  br label %b8

b8:
  %67 = load i16, ptr %5, !tbaa !2
  %68 = load i16, ptr %4, !tbaa !2
  %69 = icmp slt i16 %67, %68
  %70 = sext i1 %69 to i8
  %71 = icmp ne i8 %70, 0
  br i1 %71, label %b9, label %b11

b9:
  %72 = load i16, ptr %5, !tbaa !2
  %73 = getelementptr i8, ptr addrspace(1) %64, i16 %72
  %74 = load i16, ptr %5, !tbaa !2
  %75 = getelementptr i8, ptr addrspace(1) %66, i16 %74
  %76 = load i8, ptr addrspace(1) %75
  store i8 %76, ptr addrspace(1) %73
  br label %b10

b10:
  %77 = load i16, ptr %5, !tbaa !2
  %78 = add i16 %77, 1
  store i16 %78, ptr %5, !tbaa !2
  br label %b8

b11:
  %79 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %59, ptr addrspace(1) %79
  %80 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %61, ptr addrspace(1) %80
  store i16 0, ptr %11, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %11, i16 2
  %82 = addrspacecast ptr %81 to ptr addrspace(1)
  store i16 0, ptr %3, !tbaa !2
  store i16 128, ptr %2, !tbaa !2
  br label %b12

b12:
  %83 = load i16, ptr %3, !tbaa !2
  %84 = load i16, ptr %2, !tbaa !2
  %85 = icmp slt i16 %83, %84
  %86 = sext i1 %85 to i8
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b13, label %b15

b13:
  %88 = load i16, ptr %3, !tbaa !2
  %89 = getelementptr i8, ptr addrspace(1) %82, i16 %88
  store i8 0, ptr addrspace(1) %89
  br label %b14

b14:
  %90 = load i16, ptr %3, !tbaa !2
  %91 = add i16 %90, 1
  store i16 %91, ptr %3, !tbaa !2
  br label %b12

b15:
  %92 = getelementptr inbounds i8, ptr %11, i16 130
  store i16 0, ptr %92, !tbaa !2
  %93 = getelementptr inbounds i8, ptr %11, i16 132
  store i16 0, ptr %93, !tbaa !2
  %94 = getelementptr i8, ptr addrspace(1) %0, i16 138
  store i16 0, ptr addrspace(1) %94
  %95 = load i8, ptr %6, !tbaa !2
  %96 = icmp ne i8 %95, 0
  %97 = sext i1 %96 to i8
  %98 = icmp ne i8 %97, 0
  br i1 %98, label %b17, label %b16

b16:
  %99 = load ptr, ptr %12, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %99)
  ret void

b17:
  %100 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %100)
  br label %b16
}

define internal void @Log.write(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca [6 x i8]
  %4 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 6, i1 false)
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call addrspace(1) void @std.io.File.write(ptr addrspace(1) %5, ptr addrspace(1) %6, ptr addrspace(1) %1)
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %9 = getelementptr i8, ptr @$str2, i16 6
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 %11, ptr %2, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %11, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.write(ptr addrspace(1) %7, ptr addrspace(1) %8, ptr addrspace(1) %15)
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 136
  %17 = load i16, ptr addrspace(1) %16
  %18 = add i16 %17, 1
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %18, ptr addrspace(1) %19
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

define internal void @keep_open(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i8
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i8 0, ptr %6
  store i8 -1, ptr %6, !tbaa !2
  %7 = load ptr, ptr addrspace(1) %1
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 132
  %11 = load i16, ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 134
  %13 = load i16, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 136
  %15 = load i16, ptr addrspace(1) %14
  store i8 0, ptr %6, !tbaa !2
  store ptr %7, ptr addrspace(1) %0
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %9, ptr addrspace(1) %16
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %18 = getelementptr i8, ptr addrspace(1) %1, i16 4
  store i16 0, ptr %5, !tbaa !2
  store i16 128, ptr %4, !tbaa !2
  br label %b2

b2:
  %19 = load i16, ptr %5, !tbaa !2
  %20 = load i16, ptr %4, !tbaa !2
  %21 = icmp slt i16 %19, %20
  %22 = sext i1 %21 to i8
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b3, label %b5

b3:
  %24 = load i16, ptr %5, !tbaa !2
  %25 = getelementptr i8, ptr addrspace(1) %17, i16 %24
  %26 = load i16, ptr %5, !tbaa !2
  %27 = getelementptr i8, ptr addrspace(1) %18, i16 %26
  %28 = load i8, ptr addrspace(1) %27
  store i8 %28, ptr addrspace(1) %25
  br label %b4

b4:
  %29 = load i16, ptr %5, !tbaa !2
  %30 = add i16 %29, 1
  store i16 %30, ptr %5, !tbaa !2
  br label %b2

b5:
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 %11, ptr addrspace(1) %31
  %32 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %13, ptr addrspace(1) %32
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %15, ptr addrspace(1) %33
  store ptr null, ptr addrspace(1) %1
  %34 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store i16 0, ptr addrspace(1) %34
  %35 = getelementptr i8, ptr addrspace(1) %1, i16 4
  store i16 0, ptr %3, !tbaa !2
  store i16 128, ptr %2, !tbaa !2
  br label %b6

b6:
  %36 = load i16, ptr %3, !tbaa !2
  %37 = load i16, ptr %2, !tbaa !2
  %38 = icmp slt i16 %36, %37
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b7, label %b9

b7:
  %41 = load i16, ptr %3, !tbaa !2
  %42 = getelementptr i8, ptr addrspace(1) %35, i16 %41
  store i8 0, ptr addrspace(1) %42
  br label %b8

b8:
  %43 = load i16, ptr %3, !tbaa !2
  %44 = add i16 %43, 1
  store i16 %44, ptr %3, !tbaa !2
  br label %b6

b9:
  %45 = getelementptr i8, ptr addrspace(1) %1, i16 132
  store i16 0, ptr addrspace(1) %45
  %46 = getelementptr i8, ptr addrspace(1) %1, i16 134
  store i16 0, ptr addrspace(1) %46
  %47 = getelementptr i8, ptr addrspace(1) %1, i16 136
  store i16 0, ptr addrspace(1) %47
  %48 = load i8, ptr %6, !tbaa !2
  %49 = icmp ne i8 %48, 0
  %50 = sext i1 %49 to i8
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %b11, label %b10

b10:
  ret void

b11:
  call addrspace(1) void @Log.drop(ptr addrspace(1) %1)
  %52 = getelementptr i8, ptr addrspace(1) %1, i16 2
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %52)
  %53 = load ptr, ptr addrspace(1) %1
  call addrspace(1) void @N$BDRP(ptr %53)
  br label %b10
}

define internal void @$main(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca [138 x i8]
  %10 = alloca [138 x i8]
  %11 = alloca [138 x i8]
  %12 = alloca [8 x i8]
  %13 = alloca i8
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca [140 x i8]
  %17 = alloca [138 x i8]
  %18 = alloca [8 x i8]
  %19 = alloca i8
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca [140 x i8]
  %23 = alloca [138 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store i8 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 8, i1 false)
  store i8 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  call void @llvm.memset.p0.i16(ptr %16, i8 0, i16 140, i1 false)
  call void @llvm.memset.p0.i16(ptr %17, i8 0, i16 138, i1 false)
  call void @llvm.memset.p0.i16(ptr %18, i8 0, i16 8, i1 false)
  store i8 0, ptr %19
  store i16 0, ptr %20
  store i16 0, ptr %21
  call void @llvm.memset.p0.i16(ptr %22, i8 0, i16 140, i1 false)
  call void @llvm.memset.p0.i16(ptr %23, i8 0, i16 138, i1 false)
  %24 = addrspacecast ptr %22 to ptr addrspace(1)
  %25 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @Log.create(ptr addrspace(1) %24, ptr %25)
  %26 = load i8, ptr %22, !tbaa !2
  %27 = icmp eq i8 %26, 1
  %28 = sext i1 %27 to i8
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b2, label %b3

b2:
  %30 = getelementptr inbounds i8, ptr %22, i16 2
  %31 = load i16, ptr %30, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %22, i16 4
  %33 = load i16, ptr %32, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %34 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %31, ptr addrspace(1) %34
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %33, ptr addrspace(1) %35
  ret void

b3:
  %36 = getelementptr inbounds i8, ptr %22, i16 2
  %37 = load ptr, ptr %36, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %22, i16 4
  %39 = load i16, ptr %38, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %22, i16 134
  %41 = load i16, ptr %40, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %22, i16 136
  %43 = load i16, ptr %42, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %22, i16 138
  %45 = load i16, ptr %44, !tbaa !2
  store ptr %37, ptr %23, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 %39, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %23, i16 4
  %48 = addrspacecast ptr %47 to ptr addrspace(1)
  %49 = getelementptr inbounds i8, ptr %22, i16 6
  %50 = addrspacecast ptr %49 to ptr addrspace(1)
  store i16 0, ptr %21, !tbaa !2
  store i16 128, ptr %20, !tbaa !2
  br label %b4

b4:
  %51 = load i16, ptr %21, !tbaa !2
  %52 = load i16, ptr %20, !tbaa !2
  %53 = icmp slt i16 %51, %52
  %54 = sext i1 %53 to i8
  %55 = icmp ne i8 %54, 0
  br i1 %55, label %b5, label %b7

b5:
  %56 = load i16, ptr %21, !tbaa !2
  %57 = getelementptr i8, ptr addrspace(1) %48, i16 %56
  %58 = load i16, ptr %21, !tbaa !2
  %59 = getelementptr i8, ptr addrspace(1) %50, i16 %58
  %60 = load i8, ptr addrspace(1) %59
  store i8 %60, ptr addrspace(1) %57
  br label %b6

b6:
  %61 = load i16, ptr %21, !tbaa !2
  %62 = add i16 %61, 1
  store i16 %62, ptr %21, !tbaa !2
  br label %b4

b7:
  %63 = getelementptr inbounds i8, ptr %23, i16 132
  store i16 %41, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %23, i16 134
  store i16 %43, ptr %64, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %23, i16 136
  store i16 %45, ptr %65, !tbaa !2
  store i8 -1, ptr %19, !tbaa !2
  %66 = addrspacecast ptr %23 to ptr addrspace(1)
  %67 = getelementptr i8, ptr @$str7, i16 6
  %68 = getelementptr i8, ptr %67, i16 -4
  %69 = load i16, ptr %68
  %70 = addrspacecast ptr %67 to ptr addrspace(1)
  store i16 %69, ptr %18, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %18, i16 2
  store i16 %69, ptr %71, !tbaa !2
  %72 = getelementptr inbounds i8, ptr %18, i16 4
  store ptr addrspace(1) %70, ptr %72, !tbaa !2
  %73 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @Log.write(ptr addrspace(1) %66, ptr addrspace(1) %73)
  %74 = addrspacecast ptr %16 to ptr addrspace(1)
  %75 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @Log.create(ptr addrspace(1) %74, ptr %75)
  %76 = load i8, ptr %16, !tbaa !2
  %77 = icmp eq i8 %76, 1
  %78 = sext i1 %77 to i8
  %79 = icmp ne i8 %78, 0
  br i1 %79, label %b8, label %b9

b8:
  %80 = getelementptr inbounds i8, ptr %16, i16 2
  %81 = load i16, ptr %80, !tbaa !2
  %82 = getelementptr inbounds i8, ptr %16, i16 4
  %83 = load i16, ptr %82, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %84 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %81, ptr addrspace(1) %84
  %85 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %83, ptr addrspace(1) %85
  %86 = load i8, ptr %19, !tbaa !2
  %87 = icmp ne i8 %86, 0
  %88 = sext i1 %87 to i8
  %89 = icmp ne i8 %88, 0
  br i1 %89, label %b11, label %b10

b9:
  %90 = getelementptr inbounds i8, ptr %16, i16 2
  %91 = load ptr, ptr %90, !tbaa !2
  %92 = getelementptr inbounds i8, ptr %16, i16 4
  %93 = load i16, ptr %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %16, i16 134
  %95 = load i16, ptr %94, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %16, i16 136
  %97 = load i16, ptr %96, !tbaa !2
  %98 = getelementptr inbounds i8, ptr %16, i16 138
  %99 = load i16, ptr %98, !tbaa !2
  store ptr %91, ptr %17, !tbaa !2
  %100 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 %93, ptr %100, !tbaa !2
  %101 = getelementptr inbounds i8, ptr %17, i16 4
  %102 = addrspacecast ptr %101 to ptr addrspace(1)
  %103 = getelementptr inbounds i8, ptr %16, i16 6
  %104 = addrspacecast ptr %103 to ptr addrspace(1)
  store i16 0, ptr %15, !tbaa !2
  store i16 128, ptr %14, !tbaa !2
  br label %b12

b10:
  ret void

b11:
  %105 = addrspacecast ptr %23 to ptr addrspace(1)
  call addrspace(1) void @Log.drop(ptr addrspace(1) %105)
  %106 = getelementptr inbounds i8, ptr %23, i16 2
  %107 = addrspacecast ptr %106 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %107)
  %108 = load ptr, ptr %23, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %108)
  br label %b10

b12:
  %109 = load i16, ptr %15, !tbaa !2
  %110 = load i16, ptr %14, !tbaa !2
  %111 = icmp slt i16 %109, %110
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b13, label %b15

b13:
  %114 = load i16, ptr %15, !tbaa !2
  %115 = getelementptr i8, ptr addrspace(1) %102, i16 %114
  %116 = load i16, ptr %15, !tbaa !2
  %117 = getelementptr i8, ptr addrspace(1) %104, i16 %116
  %118 = load i8, ptr addrspace(1) %117
  store i8 %118, ptr addrspace(1) %115
  br label %b14

b14:
  %119 = load i16, ptr %15, !tbaa !2
  %120 = add i16 %119, 1
  store i16 %120, ptr %15, !tbaa !2
  br label %b12

b15:
  %121 = getelementptr inbounds i8, ptr %17, i16 132
  store i16 %95, ptr %121, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %17, i16 134
  store i16 %97, ptr %122, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %17, i16 136
  store i16 %99, ptr %123, !tbaa !2
  store i8 -1, ptr %13, !tbaa !2
  %124 = addrspacecast ptr %17 to ptr addrspace(1)
  %125 = getelementptr i8, ptr @$str9, i16 6
  %126 = getelementptr i8, ptr %125, i16 -4
  %127 = load i16, ptr %126
  %128 = addrspacecast ptr %125 to ptr addrspace(1)
  store i16 %127, ptr %12, !tbaa !2
  %129 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 %127, ptr %129, !tbaa !2
  %130 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %128, ptr %130, !tbaa !2
  %131 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @Log.write(ptr addrspace(1) %124, ptr addrspace(1) %131)
  %132 = load i8, ptr %13, !tbaa !2
  %133 = icmp ne i8 %132, 0
  %134 = sext i1 %133 to i8
  %135 = icmp ne i8 %134, 0
  br i1 %135, label %b17, label %b16

b16:
  %136 = addrspacecast ptr %10 to ptr addrspace(1)
  %137 = load ptr, ptr %23, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %23, i16 2
  %139 = load i16, ptr %138, !tbaa !2
  %140 = getelementptr inbounds i8, ptr %23, i16 132
  %141 = load i16, ptr %140, !tbaa !2
  %142 = getelementptr inbounds i8, ptr %23, i16 134
  %143 = load i16, ptr %142, !tbaa !2
  %144 = getelementptr inbounds i8, ptr %23, i16 136
  %145 = load i16, ptr %144, !tbaa !2
  store i8 0, ptr %19, !tbaa !2
  store ptr %137, ptr %9, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %139, ptr %146, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %9, i16 4
  %148 = addrspacecast ptr %147 to ptr addrspace(1)
  %149 = getelementptr inbounds i8, ptr %23, i16 4
  %150 = addrspacecast ptr %149 to ptr addrspace(1)
  store i16 0, ptr %8, !tbaa !2
  store i16 128, ptr %7, !tbaa !2
  br label %b18

b17:
  %151 = addrspacecast ptr %17 to ptr addrspace(1)
  call addrspace(1) void @Log.drop(ptr addrspace(1) %151)
  %152 = getelementptr inbounds i8, ptr %17, i16 2
  %153 = addrspacecast ptr %152 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %153)
  %154 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %154)
  br label %b16

b18:
  %155 = load i16, ptr %8, !tbaa !2
  %156 = load i16, ptr %7, !tbaa !2
  %157 = icmp slt i16 %155, %156
  %158 = sext i1 %157 to i8
  %159 = icmp ne i8 %158, 0
  br i1 %159, label %b19, label %b21

b19:
  %160 = load i16, ptr %8, !tbaa !2
  %161 = getelementptr i8, ptr addrspace(1) %148, i16 %160
  %162 = load i16, ptr %8, !tbaa !2
  %163 = getelementptr i8, ptr addrspace(1) %150, i16 %162
  %164 = load i8, ptr addrspace(1) %163
  store i8 %164, ptr addrspace(1) %161
  br label %b20

b20:
  %165 = load i16, ptr %8, !tbaa !2
  %166 = add i16 %165, 1
  store i16 %166, ptr %8, !tbaa !2
  br label %b18

b21:
  %167 = getelementptr inbounds i8, ptr %9, i16 132
  store i16 %141, ptr %167, !tbaa !2
  %168 = getelementptr inbounds i8, ptr %9, i16 134
  store i16 %143, ptr %168, !tbaa !2
  %169 = getelementptr inbounds i8, ptr %9, i16 136
  store i16 %145, ptr %169, !tbaa !2
  store ptr null, ptr %23, !tbaa !2
  %170 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 0, ptr %170, !tbaa !2
  %171 = getelementptr inbounds i8, ptr %23, i16 4
  %172 = addrspacecast ptr %171 to ptr addrspace(1)
  store i16 0, ptr %6, !tbaa !2
  store i16 128, ptr %5, !tbaa !2
  br label %b22

b22:
  %173 = load i16, ptr %6, !tbaa !2
  %174 = load i16, ptr %5, !tbaa !2
  %175 = icmp slt i16 %173, %174
  %176 = sext i1 %175 to i8
  %177 = icmp ne i8 %176, 0
  br i1 %177, label %b23, label %b25

b23:
  %178 = load i16, ptr %6, !tbaa !2
  %179 = getelementptr i8, ptr addrspace(1) %172, i16 %178
  store i8 0, ptr addrspace(1) %179
  br label %b24

b24:
  %180 = load i16, ptr %6, !tbaa !2
  %181 = add i16 %180, 1
  store i16 %181, ptr %6, !tbaa !2
  br label %b22

b25:
  %182 = getelementptr inbounds i8, ptr %23, i16 132
  store i16 0, ptr %182, !tbaa !2
  %183 = getelementptr inbounds i8, ptr %23, i16 134
  store i16 0, ptr %183, !tbaa !2
  %184 = getelementptr inbounds i8, ptr %23, i16 136
  store i16 0, ptr %184, !tbaa !2
  %185 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @keep_open(ptr addrspace(1) %136, ptr addrspace(1) %185)
  %186 = load ptr, ptr %10, !tbaa !2
  %187 = getelementptr inbounds i8, ptr %10, i16 2
  %188 = load i16, ptr %187, !tbaa !2
  %189 = getelementptr inbounds i8, ptr %10, i16 132
  %190 = load i16, ptr %189, !tbaa !2
  %191 = getelementptr inbounds i8, ptr %10, i16 134
  %192 = load i16, ptr %191, !tbaa !2
  %193 = getelementptr inbounds i8, ptr %10, i16 136
  %194 = load i16, ptr %193, !tbaa !2
  store ptr %186, ptr %11, !tbaa !2
  %195 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %188, ptr %195, !tbaa !2
  %196 = getelementptr inbounds i8, ptr %11, i16 4
  %197 = addrspacecast ptr %196 to ptr addrspace(1)
  %198 = getelementptr inbounds i8, ptr %10, i16 4
  %199 = addrspacecast ptr %198 to ptr addrspace(1)
  store i16 0, ptr %4, !tbaa !2
  store i16 128, ptr %3, !tbaa !2
  br label %b26

b26:
  %200 = load i16, ptr %4, !tbaa !2
  %201 = load i16, ptr %3, !tbaa !2
  %202 = icmp slt i16 %200, %201
  %203 = sext i1 %202 to i8
  %204 = icmp ne i8 %203, 0
  br i1 %204, label %b27, label %b29

b27:
  %205 = load i16, ptr %4, !tbaa !2
  %206 = getelementptr i8, ptr addrspace(1) %197, i16 %205
  %207 = load i16, ptr %4, !tbaa !2
  %208 = getelementptr i8, ptr addrspace(1) %199, i16 %207
  %209 = load i8, ptr addrspace(1) %208
  store i8 %209, ptr addrspace(1) %206
  br label %b28

b28:
  %210 = load i16, ptr %4, !tbaa !2
  %211 = add i16 %210, 1
  store i16 %211, ptr %4, !tbaa !2
  br label %b26

b29:
  %212 = getelementptr inbounds i8, ptr %11, i16 132
  store i16 %190, ptr %212, !tbaa !2
  %213 = getelementptr inbounds i8, ptr %11, i16 134
  store i16 %192, ptr %213, !tbaa !2
  %214 = getelementptr inbounds i8, ptr %11, i16 136
  store i16 %194, ptr %214, !tbaa !2
  store i8 -1, ptr %2, !tbaa !2
  %215 = addrspacecast ptr %11 to ptr addrspace(1)
  %216 = getelementptr i8, ptr @$str10, i16 6
  %217 = getelementptr i8, ptr %216, i16 -4
  %218 = load i16, ptr %217
  %219 = addrspacecast ptr %216 to ptr addrspace(1)
  store i16 %218, ptr %1, !tbaa !2
  %220 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %218, ptr %220, !tbaa !2
  %221 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %219, ptr %221, !tbaa !2
  %222 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Log.write(ptr addrspace(1) %215, ptr addrspace(1) %222)
  %223 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %223)
  call addrspace(1) void @N$PN()
  store i8 0, ptr addrspace(1) %0
  %224 = load i8, ptr %2, !tbaa !2
  %225 = icmp ne i8 %224, 0
  %226 = sext i1 %225 to i8
  %227 = icmp ne i8 %226, 0
  br i1 %227, label %b31, label %b30

b30:
  %228 = load i8, ptr %19, !tbaa !2
  %229 = icmp ne i8 %228, 0
  %230 = sext i1 %229 to i8
  %231 = icmp ne i8 %230, 0
  br i1 %231, label %b33, label %b32

b31:
  %232 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @Log.drop(ptr addrspace(1) %232)
  %233 = getelementptr inbounds i8, ptr %11, i16 2
  %234 = addrspacecast ptr %233 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %234)
  %235 = load ptr, ptr %11, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %235)
  br label %b30

b32:
  ret void

b33:
  %236 = addrspacecast ptr %23 to ptr addrspace(1)
  call addrspace(1) void @Log.drop(ptr addrspace(1) %236)
  %237 = getelementptr inbounds i8, ptr %23, i16 2
  %238 = addrspacecast ptr %237 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %238)
  %239 = load ptr, ptr %23, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %239)
  br label %b32
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [6 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 6, i1 false)
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @$main(ptr addrspace(1) %1)
  %2 = load i8, ptr %0, !tbaa !2
  %3 = icmp eq i8 %2, 0
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b4, label %b3

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
