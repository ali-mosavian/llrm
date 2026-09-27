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

define internal void @parse(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = alloca i8
  %6 = alloca i8
  %7 = alloca i16
  %8 = alloca i8
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i8
  %12 = alloca i8
  %13 = alloca i8
  %14 = alloca i16
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  store i8 0, ptr %5
  store i8 0, ptr %6
  store i16 0, ptr %7
  store i8 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i8 0, ptr %11
  store i8 0, ptr %12
  store i8 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %14, !tbaa !2
  br label %b2

b2:
  %15 = load i16, ptr %14, !tbaa !2
  %16 = load i16, ptr addrspace(1) %1
  %17 = icmp ult i16 %15, %16
  %18 = sext i1 %17 to i8
  store i8 %18, ptr %13, !tbaa !2
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b5, label %b6

b3:
  %20 = load i16, ptr %14, !tbaa !2
  %21 = add i16 %20, 1
  store i16 %21, ptr %14, !tbaa !2
  br label %b2

b4:
  %22 = load i16, ptr %14, !tbaa !2
  %23 = add i16 %22, 1
  %24 = load i16, ptr addrspace(1) %1
  %25 = icmp ult i16 %23, %24
  %26 = sext i1 %25 to i8
  store i8 %26, ptr %12, !tbaa !2
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b9, label %b10

b5:
  %28 = load i16, ptr %14, !tbaa !2
  %29 = load i16, ptr addrspace(1) %1
  %30 = icmp ult i16 %28, %29
  %31 = sext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b7, label %b8

b6:
  %33 = load i8, ptr %13, !tbaa !2
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b3, label %b4

b7:
  %35 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %36 = load ptr addrspace(1), ptr addrspace(1) %35
  %37 = getelementptr i8, ptr addrspace(1) %36, i16 %28
  %38 = load i8, ptr addrspace(1) %37
  %39 = icmp ne i8 %38, 61
  %40 = sext i1 %39 to i8
  store i8 %40, ptr %13, !tbaa !2
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  %41 = load i16, ptr %14, !tbaa !2
  %42 = add i16 %41, 1
  %43 = load i16, ptr addrspace(1) %1
  %44 = icmp ult i16 %42, %43
  %45 = sext i1 %44 to i8
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b11, label %b12

b10:
  %47 = load i8, ptr %12, !tbaa !2
  store i8 %47, ptr %11, !tbaa !2
  %48 = load i8, ptr %11, !tbaa !2
  %49 = icmp ne i8 %48, 0
  br i1 %49, label %b13, label %b14

b11:
  %50 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %51 = load ptr addrspace(1), ptr addrspace(1) %50
  %52 = getelementptr i8, ptr addrspace(1) %51, i16 %42
  %53 = load i8, ptr addrspace(1) %52
  %54 = icmp eq i8 %53, 45
  %55 = sext i1 %54 to i8
  store i8 %55, ptr %12, !tbaa !2
  br label %b10

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %56 = load i16, ptr %14, !tbaa !2
  %57 = add i16 %56, 2
  store i16 %57, ptr %10, !tbaa !2
  br label %b15

b14:
  %58 = load i16, ptr %14, !tbaa !2
  %59 = add i16 %58, 1
  store i16 %59, ptr %10, !tbaa !2
  br label %b15

b15:
  %60 = load i16, ptr %10, !tbaa !2
  store i16 %60, ptr %9, !tbaa !2
  %61 = load i16, ptr %14, !tbaa !2
  %62 = icmp eq i16 %61, 0
  %63 = sext i1 %62 to i8
  store i8 %63, ptr %8, !tbaa !2
  %64 = icmp ne i8 %63, 0
  br i1 %64, label %b17, label %b16

b16:
  %65 = load i16, ptr %9, !tbaa !2
  %66 = load i16, ptr addrspace(1) %1
  %67 = icmp uge i16 %65, %66
  %68 = sext i1 %67 to i8
  store i8 %68, ptr %8, !tbaa !2
  br label %b17

b17:
  %69 = load i8, ptr %8, !tbaa !2
  %70 = icmp ne i8 %69, 0
  br i1 %70, label %b18, label %b19

b18:
  store i8 1, ptr addrspace(1) %0
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %71
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %72
  ret void

b19:
  br label %b20

b20:
  store i16 0, ptr %7, !tbaa !2
  br label %b21

b21:
  %73 = load i16, ptr %9, !tbaa !2
  %74 = load i16, ptr addrspace(1) %1
  %75 = icmp ult i16 %73, %74
  %76 = sext i1 %75 to i8
  %77 = icmp ne i8 %76, 0
  br i1 %77, label %b22, label %b23

b22:
  %78 = load i16, ptr %9, !tbaa !2
  %79 = load i16, ptr addrspace(1) %1
  %80 = icmp ult i16 %78, %79
  %81 = sext i1 %80 to i8
  %82 = icmp ne i8 %81, 0
  br i1 %82, label %b24, label %b25

b23:
  %83 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %84 = load ptr addrspace(1), ptr addrspace(1) %83
  %85 = load i16, ptr addrspace(1) %1
  %86 = load i16, ptr %14, !tbaa !2
  %87 = icmp ule i16 %86, %85
  %88 = sext i1 %87 to i8
  %89 = icmp ne i8 %88, 0
  br i1 %89, label %b31, label %b32

b24:
  %90 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %91 = load ptr addrspace(1), ptr addrspace(1) %90
  %92 = getelementptr i8, ptr addrspace(1) %91, i16 %78
  %93 = load i8, ptr addrspace(1) %92
  store i8 %93, ptr %6, !tbaa !2
  %94 = load i8, ptr %6, !tbaa !2
  %95 = icmp ult i8 %94, 48
  %96 = sext i1 %95 to i8
  store i8 %96, ptr %5, !tbaa !2
  %97 = icmp ne i8 %96, 0
  br i1 %97, label %b27, label %b26

b25:
  call addrspace(1) void @N$EBND()
  unreachable

b26:
  %98 = load i8, ptr %6, !tbaa !2
  %99 = icmp ugt i8 %98, 57
  %100 = sext i1 %99 to i8
  store i8 %100, ptr %5, !tbaa !2
  br label %b27

b27:
  %101 = load i8, ptr %5, !tbaa !2
  %102 = icmp ne i8 %101, 0
  br i1 %102, label %b28, label %b29

b28:
  store i8 1, ptr addrspace(1) %0
  %103 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %103
  %104 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %104
  ret void

b29:
  br label %b30

b30:
  %105 = load i16, ptr %7, !tbaa !2
  %106 = mul i16 %105, 10
  %107 = load i8, ptr %6, !tbaa !2
  %108 = zext i8 %107 to i16
  %109 = zext i8 48 to i16
  %110 = sub i16 %108, %109
  %111 = add i16 %106, %110
  store i16 %111, ptr %7, !tbaa !2
  %112 = load i16, ptr %9, !tbaa !2
  %113 = add i16 %112, 1
  store i16 %113, ptr %9, !tbaa !2
  br label %b21

b31:
  %114 = icmp ule i16 0, %86
  %115 = sext i1 %114 to i8
  %116 = icmp ne i8 %115, 0
  br i1 %116, label %b33, label %b34

b32:
  call addrspace(1) void @N$EBND()
  unreachable

b33:
  store i16 %86, ptr %4, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %86, ptr %117, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %84, ptr %118, !tbaa !2
  %119 = addrspacecast ptr %4 to ptr addrspace(1)
  %120 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %119)
  %121 = load i8, ptr %11, !tbaa !2
  %122 = icmp ne i8 %121, 0
  br i1 %122, label %b35, label %b36

b34:
  call addrspace(1) void @N$EBND()
  unreachable

b35:
  %123 = load i16, ptr %7, !tbaa !2
  %124 = sub i16 0, %123
  store i16 %124, ptr %3, !tbaa !2
  br label %b37

b36:
  %125 = load i16, ptr %7, !tbaa !2
  store i16 %125, ptr %3, !tbaa !2
  br label %b37

b37:
  %126 = load i16, ptr %3, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %127 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %120, ptr addrspace(1) %127
  %128 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %126, ptr addrspace(1) %128
  ret void
}

define internal void @load_entries(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = alloca [6 x i8]
  %6 = alloca [4 x i8]
  %7 = alloca ptr
  %8 = alloca ptr
  %9 = alloca [6 x i8]
  %10 = alloca i16
  %11 = alloca ptr
  %12 = alloca i8
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca [134 x i8]
  %18 = alloca i8
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca [134 x i8]
  %22 = alloca [136 x i8]
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  store ptr null, ptr %7
  store ptr null, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 6, i1 false)
  store i16 0, ptr %10
  store ptr null, ptr %11
  store i8 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  call void @llvm.memset.p0.i16(ptr %17, i8 0, i16 134, i1 false)
  store i8 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  call void @llvm.memset.p0.i16(ptr %21, i8 0, i16 134, i1 false)
  call void @llvm.memset.p0.i16(ptr %22, i8 0, i16 136, i1 false)
  %23 = addrspacecast ptr %22 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.open(ptr addrspace(1) %23, ptr addrspace(1) %1, i8 0)
  %24 = load i8, ptr %22, !tbaa !2
  %25 = icmp eq i8 %24, 0
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b4, label %b3

b3:
  %28 = load i8, ptr %22, !tbaa !2
  %29 = icmp eq i8 %28, 1
  %30 = sext i1 %29 to i8
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b59, label %b58

b4:
  %32 = getelementptr inbounds i8, ptr %22, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %22, i16 132
  %35 = load i16, ptr %34, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %22, i16 134
  %37 = load i16, ptr %36, !tbaa !2
  store i16 %33, ptr %21, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %21, i16 2
  %39 = addrspacecast ptr %38 to ptr addrspace(1)
  %40 = getelementptr inbounds i8, ptr %22, i16 4
  %41 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 0, ptr %20, !tbaa !2
  store i16 128, ptr %19, !tbaa !2
  br label %b5

b5:
  %42 = load i16, ptr %20, !tbaa !2
  %43 = load i16, ptr %19, !tbaa !2
  %44 = icmp slt i16 %42, %43
  %45 = sext i1 %44 to i8
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b6, label %b8

b6:
  %47 = load i16, ptr %20, !tbaa !2
  %48 = getelementptr i8, ptr addrspace(1) %39, i16 %47
  %49 = load i16, ptr %20, !tbaa !2
  %50 = getelementptr i8, ptr addrspace(1) %41, i16 %49
  %51 = load i8, ptr addrspace(1) %50
  store i8 %51, ptr addrspace(1) %48
  br label %b7

b7:
  %52 = load i16, ptr %20, !tbaa !2
  %53 = add i16 %52, 1
  store i16 %53, ptr %20, !tbaa !2
  br label %b5

b8:
  %54 = getelementptr inbounds i8, ptr %21, i16 130
  store i16 %35, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %21, i16 132
  store i16 %37, ptr %55, !tbaa !2
  store i8 -1, ptr %18, !tbaa !2
  %56 = load i16, ptr %21, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %21, i16 130
  %58 = load i16, ptr %57, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %21, i16 132
  %60 = load i16, ptr %59, !tbaa !2
  store i8 0, ptr %18, !tbaa !2
  store i16 %56, ptr %17, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %17, i16 2
  %62 = addrspacecast ptr %61 to ptr addrspace(1)
  %63 = getelementptr inbounds i8, ptr %21, i16 2
  %64 = addrspacecast ptr %63 to ptr addrspace(1)
  store i16 0, ptr %16, !tbaa !2
  store i16 128, ptr %15, !tbaa !2
  br label %b9

b9:
  %65 = load i16, ptr %16, !tbaa !2
  %66 = load i16, ptr %15, !tbaa !2
  %67 = icmp slt i16 %65, %66
  %68 = sext i1 %67 to i8
  %69 = icmp ne i8 %68, 0
  br i1 %69, label %b10, label %b12

b10:
  %70 = load i16, ptr %16, !tbaa !2
  %71 = getelementptr i8, ptr addrspace(1) %62, i16 %70
  %72 = load i16, ptr %16, !tbaa !2
  %73 = getelementptr i8, ptr addrspace(1) %64, i16 %72
  %74 = load i8, ptr addrspace(1) %73
  store i8 %74, ptr addrspace(1) %71
  br label %b11

b11:
  %75 = load i16, ptr %16, !tbaa !2
  %76 = add i16 %75, 1
  store i16 %76, ptr %16, !tbaa !2
  br label %b9

b12:
  %77 = getelementptr inbounds i8, ptr %17, i16 130
  store i16 %58, ptr %77, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %17, i16 132
  store i16 %60, ptr %78, !tbaa !2
  store i16 0, ptr %21, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %21, i16 2
  %80 = addrspacecast ptr %79 to ptr addrspace(1)
  store i16 0, ptr %14, !tbaa !2
  store i16 128, ptr %13, !tbaa !2
  br label %b13

b13:
  %81 = load i16, ptr %14, !tbaa !2
  %82 = load i16, ptr %13, !tbaa !2
  %83 = icmp slt i16 %81, %82
  %84 = sext i1 %83 to i8
  %85 = icmp ne i8 %84, 0
  br i1 %85, label %b14, label %b16

b14:
  %86 = load i16, ptr %14, !tbaa !2
  %87 = getelementptr i8, ptr addrspace(1) %80, i16 %86
  store i8 0, ptr addrspace(1) %87
  br label %b15

b15:
  %88 = load i16, ptr %14, !tbaa !2
  %89 = add i16 %88, 1
  store i16 %89, ptr %14, !tbaa !2
  br label %b13

b16:
  %90 = getelementptr inbounds i8, ptr %21, i16 130
  store i16 0, ptr %90, !tbaa !2
  %91 = getelementptr inbounds i8, ptr %21, i16 132
  store i16 0, ptr %91, !tbaa !2
  store i8 -1, ptr %12, !tbaa !2
  %92 = getelementptr i8, ptr @$str1, i16 6
  store ptr %92, ptr %11, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %93 = addrspacecast ptr %17 to ptr addrspace(1)
  br label %b18

b17:
  %94 = load ptr, ptr %11, !tbaa !2
  store ptr null, ptr %11, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %95 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %94, ptr addrspace(1) %95
  %96 = load ptr, ptr %11, !tbaa !2
  %97 = icmp ne ptr %96, null
  %98 = sext i1 %97 to i8
  %99 = icmp ne i8 %98, 0
  br i1 %99, label %b50, label %b49

b18:
  br label %b19

b19:
  %100 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.read_line(ptr addrspace(1) %100, ptr addrspace(1) %93)
  %101 = load i8, ptr %9, !tbaa !2
  %102 = icmp eq i8 %101, 0
  %103 = sext i1 %102 to i8
  %104 = icmp ne i8 %103, 0
  br i1 %104, label %b23, label %b22

b21:
  br label %b18

b22:
  %105 = load i8, ptr %9, !tbaa !2
  %106 = icmp eq i8 %105, 0
  %107 = sext i1 %106 to i8
  %108 = icmp ne i8 %107, 0
  br i1 %108, label %b42, label %b41

b23:
  %109 = getelementptr inbounds i8, ptr %9, i16 2
  %110 = load i8, ptr %109, !tbaa !2
  %111 = icmp eq i8 %110, 0
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b24, label %b22

b24:
  %114 = getelementptr inbounds i8, ptr %9, i16 4
  %115 = load ptr, ptr %114, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %9, i16 4
  %117 = load ptr, ptr %116, !tbaa !2
  store ptr %117, ptr %8, !tbaa !2
  %118 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  store ptr %118, ptr %7, !tbaa !2
  %119 = load i16, ptr %10, !tbaa !2
  %120 = add i16 %119, 1
  store i16 %120, ptr %10, !tbaa !2
  %121 = load ptr, ptr %7, !tbaa !2
  %122 = getelementptr i8, ptr %121, i16 -4
  %123 = load i16, ptr %122
  %124 = icmp eq i16 %123, 0
  %125 = sext i1 %124 to i8
  %126 = xor i8 %125, -1
  %127 = icmp ne i8 %126, 0
  br i1 %127, label %b26, label %b27

b25:
  %128 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %128)
  br label %b21

b26:
  %129 = addrspacecast ptr %5 to ptr addrspace(1)
  %130 = load ptr, ptr %7, !tbaa !2
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  %133 = addrspacecast ptr %130 to ptr addrspace(1)
  store i16 %132, ptr %4, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %132, ptr %134, !tbaa !2
  %135 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %133, ptr %135, !tbaa !2
  %136 = addrspacecast ptr %4 to ptr addrspace(1)
  %137 = load i16, ptr %10, !tbaa !2
  call addrspace(1) void @parse(ptr addrspace(1) %129, ptr addrspace(1) %136, i16 %137)
  %138 = load i8, ptr %5, !tbaa !2
  %139 = icmp eq i8 %138, 1
  %140 = sext i1 %139 to i8
  %141 = icmp ne i8 %140, 0
  br i1 %141, label %b29, label %b30

b27:
  br label %b28

b28:
  %142 = load ptr, ptr %7, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %142)
  br label %b25

b29:
  %143 = getelementptr inbounds i8, ptr %5, i16 2
  %144 = load i16, ptr %143, !tbaa !2
  %145 = getelementptr inbounds i8, ptr %5, i16 4
  %146 = load i16, ptr %145, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %147 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %144, ptr addrspace(1) %147
  %148 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %146, ptr addrspace(1) %148
  %149 = load ptr, ptr %7, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %149)
  %150 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %150)
  %151 = load ptr, ptr %11, !tbaa !2
  %152 = icmp ne ptr %151, null
  %153 = sext i1 %152 to i8
  %154 = icmp ne i8 %153, 0
  br i1 %154, label %b32, label %b31

b30:
  %155 = getelementptr inbounds i8, ptr %5, i16 2
  %156 = load ptr, ptr %155, !tbaa !2
  %157 = getelementptr inbounds i8, ptr %5, i16 4
  %158 = load i16, ptr %157, !tbaa !2
  store ptr %156, ptr %6, !tbaa !2
  %159 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %158, ptr %159, !tbaa !2
  %160 = load ptr, ptr %11, !tbaa !2
  %161 = getelementptr i8, ptr %160, i16 -4
  %162 = load i16, ptr %161
  %163 = call addrspace(1) ptr @N$BGRW(ptr %160, i16 1, i16 4)
  store ptr %163, ptr %11, !tbaa !2
  %164 = mul i16 %162, 4
  %165 = getelementptr i8, ptr %163, i16 %164
  %166 = load ptr, ptr %6, !tbaa !2
  %167 = getelementptr inbounds i8, ptr %6, i16 2
  %168 = load i16, ptr %167, !tbaa !2
  store ptr %166, ptr %165
  %169 = getelementptr i8, ptr %165, i16 2
  store i16 %168, ptr %169
  store ptr null, ptr %6, !tbaa !2
  %170 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 0, ptr %170, !tbaa !2
  %171 = load ptr, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %171)
  br label %b28

b31:
  call addrspace(1) void @N$BDRP(ptr %151)
  %172 = load i8, ptr %12, !tbaa !2
  %173 = icmp ne i8 %172, 0
  %174 = sext i1 %173 to i8
  %175 = icmp ne i8 %174, 0
  br i1 %175, label %b37, label %b36

b32:
  %176 = getelementptr i8, ptr %151, i16 -4
  %177 = load i16, ptr %176
  store i16 0, ptr %3, !tbaa !2
  br label %b33

b33:
  %178 = load i16, ptr %3, !tbaa !2
  %179 = icmp ult i16 %178, %177
  %180 = sext i1 %179 to i8
  %181 = icmp ne i8 %180, 0
  br i1 %181, label %b35, label %b34

b34:
  br label %b31

b35:
  %182 = mul i16 %178, 4
  %183 = getelementptr i8, ptr %151, i16 %182
  %184 = load ptr, ptr %183
  call addrspace(1) void @N$BDRP(ptr %184)
  %185 = add i16 %178, 1
  store i16 %185, ptr %3, !tbaa !2
  br label %b33

b36:
  %186 = load i8, ptr %18, !tbaa !2
  %187 = icmp ne i8 %186, 0
  %188 = sext i1 %187 to i8
  %189 = icmp ne i8 %188, 0
  br i1 %189, label %b39, label %b38

b37:
  %190 = addrspacecast ptr %17 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %190)
  br label %b36

b38:
  ret void

b39:
  %191 = addrspacecast ptr %21 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %191)
  br label %b38

b41:
  br label %b17

b42:
  %192 = getelementptr inbounds i8, ptr %9, i16 2
  %193 = load i8, ptr %192, !tbaa !2
  %194 = icmp eq i8 %193, 0
  %195 = sext i1 %194 to i8
  %196 = icmp ne i8 %195, 0
  br i1 %196, label %b44, label %b43

b43:
  br label %b41

b44:
  %197 = getelementptr inbounds i8, ptr %9, i16 4
  %198 = load ptr, ptr %197, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %198)
  br label %b43

b49:
  call addrspace(1) void @N$BDRP(ptr %96)
  %199 = load i8, ptr %12, !tbaa !2
  %200 = icmp ne i8 %199, 0
  %201 = sext i1 %200 to i8
  %202 = icmp ne i8 %201, 0
  br i1 %202, label %b55, label %b54

b50:
  %203 = getelementptr i8, ptr %96, i16 -4
  %204 = load i16, ptr %203
  store i16 0, ptr %2, !tbaa !2
  br label %b51

b51:
  %205 = load i16, ptr %2, !tbaa !2
  %206 = icmp ult i16 %205, %204
  %207 = sext i1 %206 to i8
  %208 = icmp ne i8 %207, 0
  br i1 %208, label %b53, label %b52

b52:
  br label %b49

b53:
  %209 = mul i16 %205, 4
  %210 = getelementptr i8, ptr %96, i16 %209
  %211 = load ptr, ptr %210
  call addrspace(1) void @N$BDRP(ptr %211)
  %212 = add i16 %205, 1
  store i16 %212, ptr %2, !tbaa !2
  br label %b51

b54:
  %213 = load i8, ptr %18, !tbaa !2
  %214 = icmp ne i8 %213, 0
  %215 = sext i1 %214 to i8
  %216 = icmp ne i8 %215, 0
  br i1 %216, label %b57, label %b56

b55:
  %217 = addrspacecast ptr %17 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %217)
  br label %b54

b56:
  ret void

b57:
  %218 = addrspacecast ptr %21 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %218)
  br label %b56

b58:
  store i8 1, ptr addrspace(1) %0
  %219 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 1, ptr addrspace(1) %219
  ret void

b59:
  %220 = getelementptr inbounds i8, ptr %22, i16 2
  %221 = load i8, ptr %220, !tbaa !2
  %222 = icmp eq i8 %221, 0
  %223 = sext i1 %222 to i8
  %224 = icmp ne i8 %223, 0
  br i1 %224, label %b60, label %b58

b60:
  store i8 1, ptr addrspace(1) %0
  %225 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %225
  ret void
}

define internal void @$main(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca i8
  %7 = alloca i16
  %8 = alloca i8
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [8 x i8]
  %12 = alloca ptr
  %13 = alloca [8 x i8]
  %14 = alloca [8 x i8]
  %15 = alloca ptr
  %16 = alloca [8 x i8]
  %17 = alloca [8 x i8]
  %18 = alloca i8
  %19 = alloca i16
  %20 = alloca i8
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca [8 x i8]
  %24 = alloca ptr
  %25 = alloca i16
  %26 = alloca ptr
  %27 = alloca ptr
  %28 = alloca [8 x i8]
  %29 = alloca [6 x i8]
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  store i8 0, ptr %6
  store i16 0, ptr %7
  store i8 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  store ptr null, ptr %12
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 8, i1 false)
  store ptr null, ptr %15
  call void @llvm.memset.p0.i16(ptr %16, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %17, i8 0, i16 8, i1 false)
  store i8 0, ptr %18
  store i16 0, ptr %19
  store i8 0, ptr %20
  store i16 0, ptr %21
  store i16 0, ptr %22
  call void @llvm.memset.p0.i16(ptr %23, i8 0, i16 8, i1 false)
  store ptr null, ptr %24
  store i16 0, ptr %25
  store ptr null, ptr %26
  store ptr null, ptr %27
  call void @llvm.memset.p0.i16(ptr %28, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %29, i8 0, i16 6, i1 false)
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  %31 = getelementptr i8, ptr @$str2, i16 6
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  %34 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 %33, ptr %28, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %28, i16 2
  store i16 %33, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %28, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %28 to ptr addrspace(1)
  call addrspace(1) void @load_entries(ptr addrspace(1) %30, ptr addrspace(1) %37)
  %38 = load i8, ptr %29, !tbaa !2
  %39 = icmp eq i8 %38, 1
  %40 = sext i1 %39 to i8
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b2, label %b3

b2:
  %42 = getelementptr inbounds i8, ptr %29, i16 2
  %43 = load i16, ptr %42, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %29, i16 4
  %45 = load i16, ptr %44, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %46 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %43, ptr addrspace(1) %46
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %45, ptr addrspace(1) %47
  ret void

b3:
  %48 = getelementptr inbounds i8, ptr %29, i16 2
  %49 = load ptr, ptr %48, !tbaa !2
  store ptr %49, ptr %27, !tbaa !2
  %50 = getelementptr i8, ptr @$str1, i16 6
  store ptr %50, ptr %26, !tbaa !2
  %51 = load ptr, ptr %27, !tbaa !2
  %52 = getelementptr i8, ptr %51, i16 -4
  %53 = load i16, ptr %52
  store i16 0, ptr %25, !tbaa !2
  br label %b4

b4:
  %54 = load i16, ptr %25, !tbaa !2
  %55 = icmp ult i16 %54, %53
  %56 = sext i1 %55 to i8
  %57 = icmp ne i8 %56, 0
  br i1 %57, label %b5, label %b7

b5:
  %58 = mul i16 %54, 4
  %59 = getelementptr i8, ptr %51, i16 %58
  %60 = getelementptr i8, ptr %59, i16 2
  %61 = load i16, ptr %60
  %62 = icmp sgt i16 %61, 0
  %63 = sext i1 %62 to i8
  %64 = icmp ne i8 %63, 0
  br i1 %64, label %b8, label %b9

b6:
  %65 = load i16, ptr %25, !tbaa !2
  %66 = add i16 %65, 1
  store i16 %66, ptr %25, !tbaa !2
  br label %b4

b7:
  %67 = load ptr, ptr %26, !tbaa !2
  store ptr null, ptr %26, !tbaa !2
  store ptr %67, ptr %15, !tbaa !2
  %68 = load ptr, ptr %27, !tbaa !2
  %69 = getelementptr i8, ptr %68, i16 -4
  %70 = load i16, ptr %69
  %71 = addrspacecast ptr %68 to ptr addrspace(1)
  store i16 %70, ptr %14, !tbaa !2
  %72 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 %70, ptr %72, !tbaa !2
  %73 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %71, ptr %73, !tbaa !2
  %74 = addrspacecast ptr %14 to ptr addrspace(1)
  %75 = getelementptr i8, ptr addrspace(1) %74, i16 4
  %76 = load ptr addrspace(1), ptr addrspace(1) %75, !tbaa !2
  %77 = load i16, ptr addrspace(1) %74, !tbaa !2
  %78 = icmp eq i16 %77, 0
  %79 = sext i1 %78 to i8
  %80 = icmp ne i8 %79, 0
  br i1 %80, label %b41, label %b40

b8:
  %81 = load ptr, ptr %26, !tbaa !2
  %82 = call addrspace(1) ptr @N$DRES(ptr %81, i16 6)
  store ptr %82, ptr %26, !tbaa !2
  %83 = load ptr, ptr %59
  %84 = call addrspace(1) ptr @N$BCLN(ptr %83, i16 1)
  store ptr %84, ptr %24, !tbaa !2
  %85 = load ptr, ptr %24, !tbaa !2
  %86 = getelementptr i8, ptr %85, i16 -4
  %87 = load i16, ptr %86
  %88 = addrspacecast ptr %85 to ptr addrspace(1)
  store i16 %87, ptr %23, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 %87, ptr %89, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %23, i16 4
  store ptr addrspace(1) %88, ptr %90, !tbaa !2
  %91 = addrspacecast ptr %23 to ptr addrspace(1)
  %92 = call addrspace(1) i16 @string.hash(ptr addrspace(1) %91)
  %93 = or i16 %92, 1
  store i16 %93, ptr %22, !tbaa !2
  store i16 0, ptr %21, !tbaa !2
  store i8 0, ptr %20, !tbaa !2
  %94 = getelementptr i8, ptr %82, i16 -4
  %95 = load i16, ptr %94
  %96 = icmp ne i16 %95, 0
  %97 = sext i1 %96 to i8
  %98 = icmp ne i8 %97, 0
  br i1 %98, label %b11, label %b12

b9:
  br label %b10

b10:
  br label %b6

b11:
  %99 = getelementptr i8, ptr %82, i16 -4
  %100 = load i16, ptr %99
  %101 = sub i16 %100, 1
  store i16 %101, ptr %19, !tbaa !2
  %102 = load i16, ptr %22, !tbaa !2
  %103 = load i16, ptr %19, !tbaa !2
  %104 = and i16 %102, %103
  store i16 %104, ptr %21, !tbaa !2
  br label %b14

b12:
  br label %b13

b13:
  %105 = load i8, ptr %20, !tbaa !2
  %106 = xor i8 %105, -1
  %107 = icmp ne i8 %106, 0
  br i1 %107, label %b28, label %b29

b14:
  %108 = load i16, ptr %21, !tbaa !2
  %109 = getelementptr i8, ptr %82, i16 -4
  %110 = load i16, ptr %109
  %111 = icmp ult i16 %108, %110
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b17, label %b18

b15:
  %114 = load i16, ptr %21, !tbaa !2
  %115 = getelementptr i8, ptr %82, i16 -4
  %116 = load i16, ptr %115
  %117 = icmp ult i16 %114, %116
  %118 = sext i1 %117 to i8
  %119 = icmp ne i8 %118, 0
  br i1 %119, label %b19, label %b20

b16:
  br label %b13

b17:
  %120 = mul i16 %108, 6
  %121 = getelementptr i8, ptr %82, i16 %120
  %122 = load i16, ptr %121
  %123 = icmp ne i16 %122, 0
  %124 = sext i1 %123 to i8
  %125 = icmp ne i8 %124, 0
  br i1 %125, label %b15, label %b16

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %126 = mul i16 %114, 6
  %127 = getelementptr i8, ptr %82, i16 %126
  %128 = load i16, ptr %127
  %129 = load i16, ptr %22, !tbaa !2
  %130 = icmp eq i16 %128, %129
  %131 = sext i1 %130 to i8
  store i8 %131, ptr %18, !tbaa !2
  %132 = icmp ne i8 %131, 0
  br i1 %132, label %b21, label %b22

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  %133 = load i16, ptr %21, !tbaa !2
  %134 = getelementptr i8, ptr %82, i16 -4
  %135 = load i16, ptr %134
  %136 = icmp ult i16 %133, %135
  %137 = sext i1 %136 to i8
  %138 = icmp ne i8 %137, 0
  br i1 %138, label %b23, label %b24

b22:
  %139 = load i8, ptr %18, !tbaa !2
  %140 = icmp ne i8 %139, 0
  br i1 %140, label %b25, label %b26

b23:
  %141 = mul i16 %133, 6
  %142 = getelementptr i8, ptr %82, i16 %141
  %143 = getelementptr i8, ptr %142, i16 2
  %144 = load ptr, ptr %143
  %145 = getelementptr i8, ptr %144, i16 -4
  %146 = load i16, ptr %145
  %147 = addrspacecast ptr %144 to ptr addrspace(1)
  store i16 %146, ptr %17, !tbaa !2
  %148 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 %146, ptr %148, !tbaa !2
  %149 = getelementptr inbounds i8, ptr %17, i16 4
  store ptr addrspace(1) %147, ptr %149, !tbaa !2
  %150 = addrspacecast ptr %17 to ptr addrspace(1)
  %151 = load ptr, ptr %24, !tbaa !2
  %152 = getelementptr i8, ptr %151, i16 -4
  %153 = load i16, ptr %152
  %154 = addrspacecast ptr %151 to ptr addrspace(1)
  store i16 %153, ptr %16, !tbaa !2
  %155 = getelementptr inbounds i8, ptr %16, i16 2
  store i16 %153, ptr %155, !tbaa !2
  %156 = getelementptr inbounds i8, ptr %16, i16 4
  store ptr addrspace(1) %154, ptr %156, !tbaa !2
  %157 = addrspacecast ptr %16 to ptr addrspace(1)
  %158 = call addrspace(1) i8 @string.eq(ptr addrspace(1) %150, ptr addrspace(1) %157)
  store i8 %158, ptr %18, !tbaa !2
  br label %b22

b24:
  call addrspace(1) void @N$EBND()
  unreachable

b25:
  store i8 -1, ptr %20, !tbaa !2
  br label %b16

b26:
  br label %b27

b27:
  %159 = load i16, ptr %21, !tbaa !2
  %160 = add i16 %159, 1
  %161 = load i16, ptr %19, !tbaa !2
  %162 = and i16 %160, %161
  store i16 %162, ptr %21, !tbaa !2
  br label %b14

b28:
  %163 = load i16, ptr %21, !tbaa !2
  %164 = getelementptr i8, ptr %82, i16 -4
  %165 = load i16, ptr %164
  %166 = icmp ult i16 %163, %165
  %167 = sext i1 %166 to i8
  %168 = icmp ne i8 %167, 0
  br i1 %168, label %b31, label %b32

b29:
  br label %b30

b30:
  %169 = load i8, ptr %20, !tbaa !2
  %170 = icmp ne i8 %169, 0
  br i1 %170, label %b36, label %b35

b31:
  %171 = mul i16 %163, 6
  %172 = getelementptr i8, ptr %82, i16 %171
  %173 = load i16, ptr %22, !tbaa !2
  store i16 %173, ptr %172
  %174 = load i16, ptr %21, !tbaa !2
  %175 = getelementptr i8, ptr %82, i16 -4
  %176 = load i16, ptr %175
  %177 = icmp ult i16 %174, %176
  %178 = sext i1 %177 to i8
  %179 = icmp ne i8 %178, 0
  br i1 %179, label %b33, label %b34

b32:
  call addrspace(1) void @N$EBND()
  unreachable

b33:
  %180 = mul i16 %174, 6
  %181 = getelementptr i8, ptr %82, i16 %180
  %182 = load ptr, ptr %24, !tbaa !2
  store ptr null, ptr %24, !tbaa !2
  %183 = getelementptr i8, ptr %181, i16 2
  %184 = load ptr, ptr %183
  call addrspace(1) void @N$BDRP(ptr %184)
  %185 = getelementptr i8, ptr %181, i16 2
  store ptr %182, ptr %185
  br label %b30

b34:
  call addrspace(1) void @N$EBND()
  unreachable

b35:
  %186 = getelementptr i8, ptr %82, i16 -2
  %187 = load i16, ptr %186
  %188 = add i16 %187, 1
  %189 = getelementptr i8, ptr %82, i16 -2
  store i16 %188, ptr %189
  br label %b36

b36:
  %190 = load i16, ptr %21, !tbaa !2
  %191 = getelementptr i8, ptr %82, i16 -4
  %192 = load i16, ptr %191
  %193 = icmp ult i16 %190, %192
  %194 = sext i1 %193 to i8
  %195 = icmp ne i8 %194, 0
  br i1 %195, label %b37, label %b38

b37:
  %196 = mul i16 %190, 6
  %197 = getelementptr i8, ptr %82, i16 %196
  %198 = getelementptr i8, ptr %59, i16 2
  %199 = load i16, ptr %198
  %200 = getelementptr i8, ptr %197, i16 4
  store i16 %199, ptr %200
  %201 = load ptr, ptr %24, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %201)
  br label %b10

b38:
  call addrspace(1) void @N$EBND()
  unreachable

b39:
  %202 = load ptr, ptr %15, !tbaa !2
  %203 = getelementptr i8, ptr %202, i16 -2
  %204 = load i16, ptr %203
  call addrspace(1) void @N$PU2(i16 %204)
  %205 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %205)
  %206 = load ptr, ptr %15, !tbaa !2
  %207 = getelementptr i8, ptr @$str7, i16 6
  store ptr %207, ptr %12, !tbaa !2
  %208 = load ptr, ptr %12, !tbaa !2
  %209 = getelementptr i8, ptr %208, i16 -4
  %210 = load i16, ptr %209
  %211 = addrspacecast ptr %208 to ptr addrspace(1)
  store i16 %210, ptr %11, !tbaa !2
  %212 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %210, ptr %212, !tbaa !2
  %213 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %211, ptr %213, !tbaa !2
  %214 = addrspacecast ptr %11 to ptr addrspace(1)
  %215 = call addrspace(1) i16 @string.hash(ptr addrspace(1) %214)
  %216 = or i16 %215, 1
  store i16 %216, ptr %10, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i8 0, ptr %8, !tbaa !2
  %217 = getelementptr i8, ptr %206, i16 -4
  %218 = load i16, ptr %217
  %219 = icmp ne i16 %218, 0
  %220 = sext i1 %219 to i8
  %221 = icmp ne i8 %220, 0
  br i1 %221, label %b43, label %b44

b40:
  %222 = getelementptr i8, ptr addrspace(1) %76, i16 0
  %223 = getelementptr i8, ptr addrspace(1) %76, i16 4
  %224 = sub i16 %77, 1
  store i16 %224, ptr %13, !tbaa !2
  %225 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %224, ptr %225, !tbaa !2
  %226 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %223, ptr %226, !tbaa !2
  %227 = addrspacecast ptr %13 to ptr addrspace(1)
  %228 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %228)
  %229 = load ptr, ptr addrspace(1) %222
  call addrspace(1) void @N$PS(ptr %229)
  %230 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %230)
  %231 = load i16, ptr addrspace(1) %227
  call addrspace(1) void @N$PU2(i16 %231)
  call addrspace(1) void @N$PN()
  br label %b39

b41:
  %232 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %232)
  call addrspace(1) void @N$PN()
  br label %b39

b43:
  %233 = getelementptr i8, ptr %206, i16 -4
  %234 = load i16, ptr %233
  %235 = sub i16 %234, 1
  store i16 %235, ptr %7, !tbaa !2
  %236 = load i16, ptr %10, !tbaa !2
  %237 = load i16, ptr %7, !tbaa !2
  %238 = and i16 %236, %237
  store i16 %238, ptr %9, !tbaa !2
  br label %b46

b44:
  br label %b45

b45:
  %239 = load i8, ptr %8, !tbaa !2
  %240 = icmp ne i8 %239, 0
  br i1 %240, label %b60, label %b61

b46:
  %241 = load i16, ptr %9, !tbaa !2
  %242 = getelementptr i8, ptr %206, i16 -4
  %243 = load i16, ptr %242
  %244 = icmp ult i16 %241, %243
  %245 = sext i1 %244 to i8
  %246 = icmp ne i8 %245, 0
  br i1 %246, label %b49, label %b50

b47:
  %247 = load i16, ptr %9, !tbaa !2
  %248 = getelementptr i8, ptr %206, i16 -4
  %249 = load i16, ptr %248
  %250 = icmp ult i16 %247, %249
  %251 = sext i1 %250 to i8
  %252 = icmp ne i8 %251, 0
  br i1 %252, label %b51, label %b52

b48:
  br label %b45

b49:
  %253 = mul i16 %241, 6
  %254 = getelementptr i8, ptr %206, i16 %253
  %255 = load i16, ptr %254
  %256 = icmp ne i16 %255, 0
  %257 = sext i1 %256 to i8
  %258 = icmp ne i8 %257, 0
  br i1 %258, label %b47, label %b48

b50:
  call addrspace(1) void @N$EBND()
  unreachable

b51:
  %259 = mul i16 %247, 6
  %260 = getelementptr i8, ptr %206, i16 %259
  %261 = load i16, ptr %260
  %262 = load i16, ptr %10, !tbaa !2
  %263 = icmp eq i16 %261, %262
  %264 = sext i1 %263 to i8
  store i8 %264, ptr %6, !tbaa !2
  %265 = icmp ne i8 %264, 0
  br i1 %265, label %b53, label %b54

b52:
  call addrspace(1) void @N$EBND()
  unreachable

b53:
  %266 = load i16, ptr %9, !tbaa !2
  %267 = getelementptr i8, ptr %206, i16 -4
  %268 = load i16, ptr %267
  %269 = icmp ult i16 %266, %268
  %270 = sext i1 %269 to i8
  %271 = icmp ne i8 %270, 0
  br i1 %271, label %b55, label %b56

b54:
  %272 = load i8, ptr %6, !tbaa !2
  %273 = icmp ne i8 %272, 0
  br i1 %273, label %b57, label %b58

b55:
  %274 = mul i16 %266, 6
  %275 = getelementptr i8, ptr %206, i16 %274
  %276 = getelementptr i8, ptr %275, i16 2
  %277 = load ptr, ptr %276
  %278 = getelementptr i8, ptr %277, i16 -4
  %279 = load i16, ptr %278
  %280 = addrspacecast ptr %277 to ptr addrspace(1)
  store i16 %279, ptr %5, !tbaa !2
  %281 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %279, ptr %281, !tbaa !2
  %282 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %280, ptr %282, !tbaa !2
  %283 = addrspacecast ptr %5 to ptr addrspace(1)
  %284 = load ptr, ptr %12, !tbaa !2
  %285 = getelementptr i8, ptr %284, i16 -4
  %286 = load i16, ptr %285
  %287 = addrspacecast ptr %284 to ptr addrspace(1)
  store i16 %286, ptr %4, !tbaa !2
  %288 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %286, ptr %288, !tbaa !2
  %289 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %287, ptr %289, !tbaa !2
  %290 = addrspacecast ptr %4 to ptr addrspace(1)
  %291 = call addrspace(1) i8 @string.eq(ptr addrspace(1) %283, ptr addrspace(1) %290)
  store i8 %291, ptr %6, !tbaa !2
  br label %b54

b56:
  call addrspace(1) void @N$EBND()
  unreachable

b57:
  store i8 -1, ptr %8, !tbaa !2
  br label %b48

b58:
  br label %b59

b59:
  %292 = load i16, ptr %9, !tbaa !2
  %293 = add i16 %292, 1
  %294 = load i16, ptr %7, !tbaa !2
  %295 = and i16 %293, %294
  store i16 %295, ptr %9, !tbaa !2
  br label %b46

b60:
  %296 = load i16, ptr %9, !tbaa !2
  %297 = getelementptr i8, ptr %206, i16 -4
  %298 = load i16, ptr %297
  %299 = icmp ult i16 %296, %298
  %300 = sext i1 %299 to i8
  %301 = icmp ne i8 %300, 0
  br i1 %301, label %b62, label %b63

b61:
  call addrspace(1) void @N$EKEY()
  unreachable

b62:
  %302 = mul i16 %296, 6
  %303 = getelementptr i8, ptr %206, i16 %302
  %304 = getelementptr i8, ptr %303, i16 4
  %305 = load i16, ptr %304
  call addrspace(1) void @N$PI2(i16 %305)
  call addrspace(1) void @N$PN()
  store i8 0, ptr addrspace(1) %0
  %306 = load ptr, ptr %12, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %306)
  %307 = load ptr, ptr %15, !tbaa !2
  %308 = icmp ne ptr %307, null
  %309 = sext i1 %308 to i8
  %310 = icmp ne i8 %309, 0
  br i1 %310, label %b65, label %b64

b63:
  call addrspace(1) void @N$EBND()
  unreachable

b64:
  call addrspace(1) void @N$BDRP(ptr %307)
  %311 = load ptr, ptr %26, !tbaa !2
  %312 = icmp ne ptr %311, null
  %313 = sext i1 %312 to i8
  %314 = icmp ne i8 %313, 0
  br i1 %314, label %b70, label %b69

b65:
  %315 = getelementptr i8, ptr %307, i16 -4
  %316 = load i16, ptr %315
  store i16 0, ptr %3, !tbaa !2
  br label %b66

b66:
  %317 = load i16, ptr %3, !tbaa !2
  %318 = icmp ult i16 %317, %316
  %319 = sext i1 %318 to i8
  %320 = icmp ne i8 %319, 0
  br i1 %320, label %b68, label %b67

b67:
  br label %b64

b68:
  %321 = mul i16 %317, 6
  %322 = getelementptr i8, ptr %307, i16 %321
  %323 = getelementptr i8, ptr %322, i16 2
  %324 = load ptr, ptr %323
  call addrspace(1) void @N$BDRP(ptr %324)
  %325 = add i16 %317, 1
  store i16 %325, ptr %3, !tbaa !2
  br label %b66

b69:
  call addrspace(1) void @N$BDRP(ptr %311)
  %326 = load ptr, ptr %27, !tbaa !2
  %327 = icmp ne ptr %326, null
  %328 = sext i1 %327 to i8
  %329 = icmp ne i8 %328, 0
  br i1 %329, label %b75, label %b74

b70:
  %330 = getelementptr i8, ptr %311, i16 -4
  %331 = load i16, ptr %330
  store i16 0, ptr %2, !tbaa !2
  br label %b71

b71:
  %332 = load i16, ptr %2, !tbaa !2
  %333 = icmp ult i16 %332, %331
  %334 = sext i1 %333 to i8
  %335 = icmp ne i8 %334, 0
  br i1 %335, label %b73, label %b72

b72:
  br label %b69

b73:
  %336 = mul i16 %332, 6
  %337 = getelementptr i8, ptr %311, i16 %336
  %338 = getelementptr i8, ptr %337, i16 2
  %339 = load ptr, ptr %338
  call addrspace(1) void @N$BDRP(ptr %339)
  %340 = add i16 %332, 1
  store i16 %340, ptr %2, !tbaa !2
  br label %b71

b74:
  call addrspace(1) void @N$BDRP(ptr %326)
  ret void

b75:
  %341 = getelementptr i8, ptr %326, i16 -4
  %342 = load i16, ptr %341
  store i16 0, ptr %1, !tbaa !2
  br label %b76

b76:
  %343 = load i16, ptr %1, !tbaa !2
  %344 = icmp ult i16 %343, %342
  %345 = sext i1 %344 to i8
  %346 = icmp ne i8 %345, 0
  br i1 %346, label %b78, label %b77

b77:
  br label %b74

b78:
  %347 = mul i16 %343, 4
  %348 = getelementptr i8, ptr %326, i16 %347
  %349 = load ptr, ptr %348
  call addrspace(1) void @N$BDRP(ptr %349)
  %350 = add i16 %343, 1
  store i16 %350, ptr %1, !tbaa !2
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
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b4, label %b3

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

define internal i16 @string.hash(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 5381, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 %4
  %11 = load i16, ptr %2, !tbaa !2
  %12 = mul i16 %11, 33
  %13 = load i8, ptr addrspace(1) %10
  %14 = zext i8 %13 to i16
  %15 = xor i16 %12, %14
  store i16 %15, ptr %2, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b2

b5:
  %18 = load i16, ptr %2, !tbaa !2
  ret i16 %18
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
