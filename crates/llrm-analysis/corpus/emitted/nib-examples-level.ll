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

define internal void @read_exact(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3) addrspace(1) {
b1:
  %4 = alloca i16
  %5 = alloca [6 x i8]
  store i16 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 6, i1 false)
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.read_raw(ptr addrspace(1) %6, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3)
  %7 = load i8, ptr %5, !tbaa !2
  %8 = icmp eq i8 %7, 1
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b2, label %b3

b2:
  %11 = getelementptr inbounds i8, ptr %5, i16 2
  %12 = load i16, ptr %11, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %5, i16 4
  %14 = load i16, ptr %13, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %12, ptr addrspace(1) %16
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %14, ptr addrspace(1) %17
  ret void

b3:
  %18 = getelementptr inbounds i8, ptr %5, i16 2
  %19 = load i16, ptr %18, !tbaa !2
  store i16 %19, ptr %4, !tbaa !2
  %20 = load i16, ptr %4, !tbaa !2
  %21 = icmp ult i16 %20, %3
  %22 = sext i1 %21 to i8
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b4, label %b5

b4:
  %24 = load i16, ptr %4, !tbaa !2
  %25 = sub i16 %3, %24
  store i8 1, ptr addrspace(1) %0
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %26
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %25, ptr addrspace(1) %27
  ret void

b5:
  br label %b6

b6:
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal void @Level.save(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [6 x i8]
  %7 = alloca ptr addrspace(1)
  %8 = alloca [8 x i8]
  %9 = alloca i8
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [136 x i8]
  %13 = alloca [134 x i8]
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 6, i1 false)
  store ptr addrspace(1) null, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  store i8 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 134, i1 false)
  %14 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.create(ptr addrspace(1) %14, ptr addrspace(1) %2)
  %15 = load i8, ptr %12, !tbaa !2
  %16 = icmp eq i8 %15, 1
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b2, label %b3

b2:
  %19 = getelementptr inbounds i8, ptr %12, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %12, i16 4
  %22 = load i16, ptr %21, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %23
  %24 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %20, ptr addrspace(1) %24
  %25 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %22, ptr addrspace(1) %25
  ret void

b3:
  %26 = getelementptr inbounds i8, ptr %12, i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %12, i16 132
  %29 = load i16, ptr %28, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %12, i16 134
  %31 = load i16, ptr %30, !tbaa !2
  store i16 %27, ptr %13, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %13, i16 2
  %33 = addrspacecast ptr %32 to ptr addrspace(1)
  %34 = getelementptr inbounds i8, ptr %12, i16 4
  %35 = addrspacecast ptr %34 to ptr addrspace(1)
  store i16 0, ptr %11, !tbaa !2
  store i16 128, ptr %10, !tbaa !2
  br label %b4

b4:
  %36 = load i16, ptr %11, !tbaa !2
  %37 = load i16, ptr %10, !tbaa !2
  %38 = icmp slt i16 %36, %37
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b5, label %b7

b5:
  %41 = load i16, ptr %11, !tbaa !2
  %42 = getelementptr i8, ptr addrspace(1) %33, i16 %41
  %43 = load i16, ptr %11, !tbaa !2
  %44 = getelementptr i8, ptr addrspace(1) %35, i16 %43
  %45 = load i8, ptr addrspace(1) %44
  store i8 %45, ptr addrspace(1) %42
  br label %b6

b6:
  %46 = load i16, ptr %11, !tbaa !2
  %47 = add i16 %46, 1
  store i16 %47, ptr %11, !tbaa !2
  br label %b4

b7:
  %48 = getelementptr inbounds i8, ptr %13, i16 130
  store i16 %29, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %13, i16 132
  store i16 %31, ptr %49, !tbaa !2
  store i8 -1, ptr %9, !tbaa !2
  %50 = load ptr, ptr addrspace(1) %1
  %51 = getelementptr i8, ptr %50, i16 -4
  %52 = load i16, ptr %51
  %53 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %54 = load ptr, ptr addrspace(1) %53
  %55 = getelementptr i8, ptr %54, i16 -4
  %56 = load i16, ptr %55
  %57 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %58 = load ptr, ptr addrspace(1) %57
  %59 = getelementptr i8, ptr %58, i16 -4
  %60 = load i16, ptr %59
  store i16 22092, ptr %8, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %52, ptr %61, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %8, i16 4
  store i16 %56, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %8, i16 6
  store i16 %60, ptr %63, !tbaa !2
  %64 = addrspacecast ptr %8 to ptr addrspace(1)
  store ptr addrspace(1) %64, ptr %7, !tbaa !2
  %65 = addrspacecast ptr %6 to ptr addrspace(1)
  %66 = addrspacecast ptr %13 to ptr addrspace(1)
  %67 = load ptr addrspace(1), ptr %7, !tbaa !2
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %65, ptr addrspace(1) %66, ptr addrspace(1) %67, i16 8)
  %68 = load i8, ptr %6, !tbaa !2
  %69 = icmp eq i8 %68, 1
  %70 = sext i1 %69 to i8
  %71 = icmp ne i8 %70, 0
  br i1 %71, label %b8, label %b9

b8:
  %72 = getelementptr inbounds i8, ptr %6, i16 2
  %73 = load i16, ptr %72, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %6, i16 4
  %75 = load i16, ptr %74, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %76 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %76
  %77 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %73, ptr addrspace(1) %77
  %78 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %75, ptr addrspace(1) %78
  %79 = load i8, ptr %9, !tbaa !2
  %80 = icmp ne i8 %79, 0
  %81 = sext i1 %80 to i8
  %82 = icmp ne i8 %81, 0
  br i1 %82, label %b11, label %b10

b9:
  %83 = getelementptr inbounds i8, ptr %6, i16 2
  %84 = load i16, ptr %83, !tbaa !2
  %85 = addrspacecast ptr %5 to ptr addrspace(1)
  %86 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @"written[Plane]"(ptr addrspace(1) %85, ptr addrspace(1) %86, ptr addrspace(1) %1)
  %87 = load i8, ptr %5, !tbaa !2
  %88 = icmp eq i8 %87, 1
  %89 = sext i1 %88 to i8
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b12, label %b13

b10:
  ret void

b11:
  %91 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %91)
  br label %b10

b12:
  %92 = getelementptr inbounds i8, ptr %5, i16 2
  %93 = load i16, ptr %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %5, i16 4
  %95 = load i16, ptr %94, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %5, i16 6
  %97 = load i16, ptr %96, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %98 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %93, ptr addrspace(1) %98
  %99 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %95, ptr addrspace(1) %99
  %100 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %97, ptr addrspace(1) %100
  %101 = load i8, ptr %9, !tbaa !2
  %102 = icmp ne i8 %101, 0
  %103 = sext i1 %102 to i8
  %104 = icmp ne i8 %103, 0
  br i1 %104, label %b15, label %b14

b13:
  %105 = addrspacecast ptr %4 to ptr addrspace(1)
  %106 = addrspacecast ptr %13 to ptr addrspace(1)
  %107 = getelementptr i8, ptr addrspace(1) %1, i16 2
  call addrspace(1) void @"written[Node]"(ptr addrspace(1) %105, ptr addrspace(1) %106, ptr addrspace(1) %107)
  %108 = load i8, ptr %4, !tbaa !2
  %109 = icmp eq i8 %108, 1
  %110 = sext i1 %109 to i8
  %111 = icmp ne i8 %110, 0
  br i1 %111, label %b16, label %b17

b14:
  ret void

b15:
  %112 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %112)
  br label %b14

b16:
  %113 = getelementptr inbounds i8, ptr %4, i16 2
  %114 = load i16, ptr %113, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %4, i16 4
  %116 = load i16, ptr %115, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %4, i16 6
  %118 = load i16, ptr %117, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %119 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %114, ptr addrspace(1) %119
  %120 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %116, ptr addrspace(1) %120
  %121 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %118, ptr addrspace(1) %121
  %122 = load i8, ptr %9, !tbaa !2
  %123 = icmp ne i8 %122, 0
  %124 = sext i1 %123 to i8
  %125 = icmp ne i8 %124, 0
  br i1 %125, label %b19, label %b18

b17:
  %126 = addrspacecast ptr %3 to ptr addrspace(1)
  %127 = addrspacecast ptr %13 to ptr addrspace(1)
  %128 = getelementptr i8, ptr addrspace(1) %1, i16 4
  call addrspace(1) void @"written[Face]"(ptr addrspace(1) %126, ptr addrspace(1) %127, ptr addrspace(1) %128)
  %129 = load i8, ptr %3, !tbaa !2
  %130 = icmp eq i8 %129, 1
  %131 = sext i1 %130 to i8
  %132 = icmp ne i8 %131, 0
  br i1 %132, label %b20, label %b21

b18:
  ret void

b19:
  %133 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %133)
  br label %b18

b20:
  %134 = getelementptr inbounds i8, ptr %3, i16 2
  %135 = load i16, ptr %134, !tbaa !2
  %136 = getelementptr inbounds i8, ptr %3, i16 4
  %137 = load i16, ptr %136, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %3, i16 6
  %139 = load i16, ptr %138, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %140 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %135, ptr addrspace(1) %140
  %141 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %137, ptr addrspace(1) %141
  %142 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %139, ptr addrspace(1) %142
  %143 = load i8, ptr %9, !tbaa !2
  %144 = icmp ne i8 %143, 0
  %145 = sext i1 %144 to i8
  %146 = icmp ne i8 %145, 0
  br i1 %146, label %b23, label %b22

b21:
  store i8 0, ptr addrspace(1) %0
  %147 = load i8, ptr %9, !tbaa !2
  %148 = icmp ne i8 %147, 0
  %149 = sext i1 %148 to i8
  %150 = icmp ne i8 %149, 0
  br i1 %150, label %b25, label %b24

b22:
  ret void

b23:
  %151 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %151)
  br label %b22

b24:
  ret void

b25:
  %152 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %152)
  br label %b24
}

define internal void @Level.load(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca ptr
  %3 = alloca [3 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca ptr
  %6 = alloca [9 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca ptr
  %9 = alloca [4 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca ptr addrspace(1)
  %13 = alloca [8 x i8]
  %14 = alloca i8
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca [136 x i8]
  %18 = alloca [134 x i8]
  store ptr null, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 3, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  store ptr null, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 9, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  store ptr null, ptr %8
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  store ptr addrspace(1) null, ptr %12
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 8, i1 false)
  store i8 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  call void @llvm.memset.p0.i16(ptr %17, i8 0, i16 136, i1 false)
  call void @llvm.memset.p0.i16(ptr %18, i8 0, i16 134, i1 false)
  %19 = addrspacecast ptr %17 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.open(ptr addrspace(1) %19, ptr addrspace(1) %1, i8 0)
  %20 = load i8, ptr %17, !tbaa !2
  %21 = icmp eq i8 %20, 1
  %22 = sext i1 %21 to i8
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b2, label %b3

b2:
  %24 = getelementptr inbounds i8, ptr %17, i16 2
  %25 = load i16, ptr %24, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %17, i16 4
  %27 = load i16, ptr %26, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %28 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %28
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %25, ptr addrspace(1) %29
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %27, ptr addrspace(1) %30
  ret void

b3:
  %31 = getelementptr inbounds i8, ptr %17, i16 2
  %32 = load i16, ptr %31, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %17, i16 132
  %34 = load i16, ptr %33, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %17, i16 134
  %36 = load i16, ptr %35, !tbaa !2
  store i16 %32, ptr %18, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %18, i16 2
  %38 = addrspacecast ptr %37 to ptr addrspace(1)
  %39 = getelementptr inbounds i8, ptr %17, i16 4
  %40 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 0, ptr %16, !tbaa !2
  store i16 128, ptr %15, !tbaa !2
  br label %b4

b4:
  %41 = load i16, ptr %16, !tbaa !2
  %42 = load i16, ptr %15, !tbaa !2
  %43 = icmp slt i16 %41, %42
  %44 = sext i1 %43 to i8
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %b5, label %b7

b5:
  %46 = load i16, ptr %16, !tbaa !2
  %47 = getelementptr i8, ptr addrspace(1) %38, i16 %46
  %48 = load i16, ptr %16, !tbaa !2
  %49 = getelementptr i8, ptr addrspace(1) %40, i16 %48
  %50 = load i8, ptr addrspace(1) %49
  store i8 %50, ptr addrspace(1) %47
  br label %b6

b6:
  %51 = load i16, ptr %16, !tbaa !2
  %52 = add i16 %51, 1
  store i16 %52, ptr %16, !tbaa !2
  br label %b4

b7:
  %53 = getelementptr inbounds i8, ptr %18, i16 130
  store i16 %34, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %18, i16 132
  store i16 %36, ptr %54, !tbaa !2
  store i8 -1, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 0, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %13, i16 4
  store i16 0, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %13, i16 6
  store i16 0, ptr %57, !tbaa !2
  %58 = addrspacecast ptr %13 to ptr addrspace(1)
  store ptr addrspace(1) %58, ptr %12, !tbaa !2
  %59 = addrspacecast ptr %11 to ptr addrspace(1)
  %60 = addrspacecast ptr %18 to ptr addrspace(1)
  %61 = load ptr addrspace(1), ptr %12, !tbaa !2
  call addrspace(1) void @read_exact(ptr addrspace(1) %59, ptr addrspace(1) %60, ptr addrspace(1) %61, i16 8)
  %62 = load i8, ptr %11, !tbaa !2
  %63 = icmp eq i8 %62, 1
  %64 = sext i1 %63 to i8
  %65 = icmp ne i8 %64, 0
  br i1 %65, label %b8, label %b9

b8:
  %66 = getelementptr inbounds i8, ptr %11, i16 2
  %67 = load i16, ptr %66, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %11, i16 4
  %69 = load i16, ptr %68, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %11, i16 6
  %71 = load i16, ptr %70, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %67, ptr addrspace(1) %72
  %73 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %69, ptr addrspace(1) %73
  %74 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %71, ptr addrspace(1) %74
  %75 = load i8, ptr %14, !tbaa !2
  %76 = icmp ne i8 %75, 0
  %77 = sext i1 %76 to i8
  %78 = icmp ne i8 %77, 0
  br i1 %78, label %b11, label %b10

b9:
  %79 = load i16, ptr %13, !tbaa !2
  %80 = icmp ne i16 %79, 22092
  %81 = sext i1 %80 to i8
  %82 = icmp ne i8 %81, 0
  br i1 %82, label %b12, label %b13

b10:
  ret void

b11:
  %83 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %83)
  br label %b10

b12:
  store i8 1, ptr addrspace(1) %0
  %84 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 1, ptr addrspace(1) %84
  %85 = load i8, ptr %14, !tbaa !2
  %86 = icmp ne i8 %85, 0
  %87 = sext i1 %86 to i8
  %88 = icmp ne i8 %87, 0
  br i1 %88, label %b16, label %b15

b13:
  br label %b14

b14:
  %89 = addrspacecast ptr %10 to ptr addrspace(1)
  %90 = addrspacecast ptr %18 to ptr addrspace(1)
  %91 = getelementptr inbounds i8, ptr %13, i16 2
  %92 = load i16, ptr %91, !tbaa !2
  store i8 0, ptr %9, !tbaa !2
  %93 = getelementptr inbounds i8, ptr %9, i16 1
  store i8 0, ptr %93, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 0, ptr %94, !tbaa !2
  %95 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @"records[Plane]"(ptr addrspace(1) %89, ptr addrspace(1) %90, i16 %92, ptr addrspace(1) %95)
  %96 = load i8, ptr %10, !tbaa !2
  %97 = icmp eq i8 %96, 1
  %98 = sext i1 %97 to i8
  %99 = icmp ne i8 %98, 0
  br i1 %99, label %b17, label %b18

b15:
  ret void

b16:
  %100 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %100)
  br label %b15

b17:
  %101 = getelementptr inbounds i8, ptr %10, i16 2
  %102 = load i16, ptr %101, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %10, i16 4
  %104 = load i16, ptr %103, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %10, i16 6
  %106 = load i16, ptr %105, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %107 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %102, ptr addrspace(1) %107
  %108 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %104, ptr addrspace(1) %108
  %109 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %106, ptr addrspace(1) %109
  %110 = load i8, ptr %14, !tbaa !2
  %111 = icmp ne i8 %110, 0
  %112 = sext i1 %111 to i8
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b20, label %b19

b18:
  %114 = getelementptr inbounds i8, ptr %10, i16 2
  %115 = load ptr, ptr %114, !tbaa !2
  store ptr %115, ptr %8, !tbaa !2
  %116 = addrspacecast ptr %7 to ptr addrspace(1)
  %117 = addrspacecast ptr %18 to ptr addrspace(1)
  %118 = getelementptr inbounds i8, ptr %13, i16 4
  %119 = load i16, ptr %118, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 0, ptr %120, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %6, i16 4
  store i16 0, ptr %121, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %6, i16 6
  store i16 0, ptr %122, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %6, i16 8
  store i8 0, ptr %123, !tbaa !2
  %124 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @"records[Node]"(ptr addrspace(1) %116, ptr addrspace(1) %117, i16 %119, ptr addrspace(1) %124)
  %125 = load i8, ptr %7, !tbaa !2
  %126 = icmp eq i8 %125, 1
  %127 = sext i1 %126 to i8
  %128 = icmp ne i8 %127, 0
  br i1 %128, label %b21, label %b22

b19:
  ret void

b20:
  %129 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %129)
  br label %b19

b21:
  %130 = getelementptr inbounds i8, ptr %7, i16 2
  %131 = load i16, ptr %130, !tbaa !2
  %132 = getelementptr inbounds i8, ptr %7, i16 4
  %133 = load i16, ptr %132, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %7, i16 6
  %135 = load i16, ptr %134, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %136 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %131, ptr addrspace(1) %136
  %137 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %133, ptr addrspace(1) %137
  %138 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %135, ptr addrspace(1) %138
  %139 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %139)
  %140 = load i8, ptr %14, !tbaa !2
  %141 = icmp ne i8 %140, 0
  %142 = sext i1 %141 to i8
  %143 = icmp ne i8 %142, 0
  br i1 %143, label %b24, label %b23

b22:
  %144 = getelementptr inbounds i8, ptr %7, i16 2
  %145 = load ptr, ptr %144, !tbaa !2
  store ptr %145, ptr %5, !tbaa !2
  %146 = addrspacecast ptr %4 to ptr addrspace(1)
  %147 = addrspacecast ptr %18 to ptr addrspace(1)
  %148 = getelementptr inbounds i8, ptr %13, i16 6
  %149 = load i16, ptr %148, !tbaa !2
  %150 = and i8 0, 1
  %151 = and i8 0, -2
  %152 = or i8 %151, %150
  %153 = and i8 0, 127
  %154 = shl i8 %153, 1
  %155 = and i8 %152, 1
  %156 = or i8 %155, %154
  store i16 0, ptr %3, !tbaa !2
  %157 = getelementptr inbounds i8, ptr %3, i16 2
  store i8 %156, ptr %157, !tbaa !2
  %158 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @"records[Face]"(ptr addrspace(1) %146, ptr addrspace(1) %147, i16 %149, ptr addrspace(1) %158)
  %159 = load i8, ptr %4, !tbaa !2
  %160 = icmp eq i8 %159, 1
  %161 = sext i1 %160 to i8
  %162 = icmp ne i8 %161, 0
  br i1 %162, label %b25, label %b26

b23:
  ret void

b24:
  %163 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %163)
  br label %b23

b25:
  %164 = getelementptr inbounds i8, ptr %4, i16 2
  %165 = load i16, ptr %164, !tbaa !2
  %166 = getelementptr inbounds i8, ptr %4, i16 4
  %167 = load i16, ptr %166, !tbaa !2
  %168 = getelementptr inbounds i8, ptr %4, i16 6
  %169 = load i16, ptr %168, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %170 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %165, ptr addrspace(1) %170
  %171 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %167, ptr addrspace(1) %171
  %172 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %169, ptr addrspace(1) %172
  %173 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %173)
  %174 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %174)
  %175 = load i8, ptr %14, !tbaa !2
  %176 = icmp ne i8 %175, 0
  %177 = sext i1 %176 to i8
  %178 = icmp ne i8 %177, 0
  br i1 %178, label %b28, label %b27

b26:
  %179 = getelementptr inbounds i8, ptr %4, i16 2
  %180 = load ptr, ptr %179, !tbaa !2
  store ptr %180, ptr %2, !tbaa !2
  %181 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  %182 = load ptr, ptr %5, !tbaa !2
  store ptr null, ptr %5, !tbaa !2
  %183 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %184 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %181, ptr addrspace(1) %184
  %185 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %182, ptr addrspace(1) %185
  %186 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store ptr %183, ptr addrspace(1) %186
  %187 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %187)
  %188 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %188)
  %189 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %189)
  %190 = load i8, ptr %14, !tbaa !2
  %191 = icmp ne i8 %190, 0
  %192 = sext i1 %191 to i8
  %193 = icmp ne i8 %192, 0
  br i1 %193, label %b30, label %b29

b27:
  ret void

b28:
  %194 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %194)
  br label %b27

b29:
  ret void

b30:
  %195 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @std.io.File.drop(ptr addrspace(1) %195)
  br label %b29
}

define internal i32 @Level.leaf(ptr addrspace(1) %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [4 x i8]
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 4, i1 false)
  store i16 0, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %6, !tbaa !2
  %9 = icmp sge i16 %8, 0
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b3, label %b4

b3:
  %12 = load i16, ptr %6, !tbaa !2
  store i16 %12, ptr %5, !tbaa !2
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %14 = load ptr, ptr addrspace(1) %13
  %15 = load i16, ptr %5, !tbaa !2
  %16 = getelementptr i8, ptr %14, i16 -4
  %17 = load i16, ptr %16
  %18 = icmp ult i16 %15, %17
  %19 = sext i1 %18 to i8
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b5, label %b6

b4:
  %21 = load i16, ptr %6, !tbaa !2
  %22 = sub i16 -1, %21
  %23 = load i16, ptr %5, !tbaa !2
  store i16 %22, ptr %7, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %23, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %7 to ptr addrspace(1)
  %26 = load i32, ptr addrspace(1) %25, !tbaa !2
  ret i32 %26

b5:
  %27 = mul i16 %15, 9
  %28 = getelementptr i8, ptr %14, i16 %27
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  %30 = load ptr, ptr addrspace(1) %0
  %31 = load i16, ptr addrspace(1) %29
  %32 = getelementptr i8, ptr %30, i16 -4
  %33 = load i16, ptr %32
  %34 = icmp ult i16 %31, %33
  %35 = sext i1 %34 to i8
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b7, label %b8

b6:
  call addrspace(1) void @N$EBND()
  unreachable

b7:
  %37 = mul i16 %31, 4
  %38 = getelementptr i8, ptr %30, i16 %37
  %39 = addrspacecast ptr %38 to ptr addrspace(1)
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 0
  %41 = load i8, ptr addrspace(1) %40
  %42 = sext i8 %41 to i16
  %43 = mul i16 %42, %1
  %44 = getelementptr i8, ptr addrspace(1) %39, i16 1
  %45 = load i8, ptr addrspace(1) %44
  %46 = sext i8 %45 to i16
  %47 = mul i16 %46, %2
  %48 = add i16 %43, %47
  %49 = getelementptr i8, ptr addrspace(1) %39, i16 2
  %50 = load i16, ptr addrspace(1) %49
  %51 = sub i16 %48, %50
  store i16 %51, ptr %4, !tbaa !2
  %52 = getelementptr i8, ptr addrspace(1) %29, i16 2
  %53 = load i16, ptr %4, !tbaa !2
  %54 = icmp sge i16 %53, 0
  %55 = sext i1 %54 to i8
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %b9, label %b10

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  store i16 0, ptr %3, !tbaa !2
  br label %b11

b10:
  store i16 1, ptr %3, !tbaa !2
  br label %b11

b11:
  %57 = load i16, ptr %3, !tbaa !2
  %58 = icmp ult i16 %57, 2
  %59 = sext i1 %58 to i8
  %60 = icmp ne i8 %59, 0
  br i1 %60, label %b12, label %b13

b12:
  %61 = mul i16 %57, 2
  %62 = getelementptr i8, ptr addrspace(1) %52, i16 %61
  %63 = load i16, ptr addrspace(1) %62
  store i16 %63, ptr %6, !tbaa !2
  br label %b2

b13:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @built(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca ptr
  %2 = alloca ptr
  %3 = alloca ptr
  store ptr null, ptr %1
  store ptr null, ptr %2
  store ptr null, ptr %3
  %4 = getelementptr i8, ptr @$str1, i16 6
  %5 = call addrspace(1) ptr @N$BGRW(ptr %4, i16 3, i16 4)
  %6 = getelementptr i8, ptr %5, i16 0
  store i8 1, ptr %6
  %7 = getelementptr i8, ptr %6, i16 1
  store i8 0, ptr %7
  %8 = getelementptr i8, ptr %6, i16 2
  store i16 32, ptr %8
  %9 = getelementptr i8, ptr %5, i16 4
  store i8 0, ptr %9
  %10 = getelementptr i8, ptr %9, i16 1
  store i8 1, ptr %10
  %11 = getelementptr i8, ptr %9, i16 2
  store i16 32, ptr %11
  %12 = getelementptr i8, ptr %5, i16 8
  store i8 0, ptr %12
  %13 = getelementptr i8, ptr %12, i16 1
  store i8 1, ptr %13
  %14 = getelementptr i8, ptr %12, i16 2
  store i16 16, ptr %14
  store ptr %5, ptr %3, !tbaa !2
  %15 = getelementptr i8, ptr @$str1, i16 6
  %16 = call addrspace(1) ptr @N$BGRW(ptr %15, i16 3, i16 9)
  %17 = getelementptr i8, ptr %16, i16 0
  store i16 0, ptr %17
  %18 = getelementptr i8, ptr %17, i16 2
  store i16 1, ptr %18
  %19 = getelementptr i8, ptr %17, i16 4
  store i16 2, ptr %19
  %20 = getelementptr i8, ptr %17, i16 6
  store i16 0, ptr %20
  %21 = getelementptr i8, ptr %17, i16 8
  store i8 1, ptr %21
  %22 = getelementptr i8, ptr %16, i16 9
  store i16 1, ptr %22
  %23 = getelementptr i8, ptr %22, i16 2
  store i16 -1, ptr %23
  %24 = getelementptr i8, ptr %22, i16 4
  store i16 -2, ptr %24
  %25 = getelementptr i8, ptr %22, i16 6
  store i16 1, ptr %25
  %26 = getelementptr i8, ptr %22, i16 8
  store i8 2, ptr %26
  %27 = getelementptr i8, ptr %16, i16 18
  store i16 2, ptr %27
  %28 = getelementptr i8, ptr %27, i16 2
  store i16 -3, ptr %28
  %29 = getelementptr i8, ptr %27, i16 4
  store i16 -4, ptr %29
  %30 = getelementptr i8, ptr %27, i16 6
  store i16 3, ptr %30
  %31 = getelementptr i8, ptr %27, i16 8
  store i8 2, ptr %31
  store ptr %16, ptr %2, !tbaa !2
  %32 = getelementptr i8, ptr @$str1, i16 6
  %33 = call addrspace(1) ptr @N$BGRW(ptr %32, i16 5, i16 3)
  %34 = getelementptr i8, ptr %33, i16 0
  %35 = and i8 0, 1
  %36 = and i8 0, -2
  %37 = or i8 %36, %35
  %38 = and i8 1, 127
  %39 = shl i8 %38, 1
  %40 = and i8 %37, 1
  %41 = or i8 %40, %39
  store i16 0, ptr %34
  %42 = getelementptr i8, ptr %34, i16 2
  store i8 %41, ptr %42
  %43 = getelementptr i8, ptr %33, i16 3
  %44 = and i8 0, 1
  %45 = and i8 0, -2
  %46 = or i8 %45, %44
  %47 = and i8 2, 127
  %48 = shl i8 %47, 1
  %49 = and i8 %46, 1
  %50 = or i8 %49, %48
  store i16 1, ptr %43
  %51 = getelementptr i8, ptr %43, i16 2
  store i8 %50, ptr %51
  %52 = getelementptr i8, ptr %33, i16 6
  %53 = and i8 -1, 1
  %54 = and i8 0, -2
  %55 = or i8 %54, %53
  %56 = and i8 2, 127
  %57 = shl i8 %56, 1
  %58 = and i8 %55, 1
  %59 = or i8 %58, %57
  store i16 1, ptr %52
  %60 = getelementptr i8, ptr %52, i16 2
  store i8 %59, ptr %60
  %61 = getelementptr i8, ptr %33, i16 9
  %62 = and i8 0, 1
  %63 = and i8 0, -2
  %64 = or i8 %63, %62
  %65 = and i8 3, 127
  %66 = shl i8 %65, 1
  %67 = and i8 %64, 1
  %68 = or i8 %67, %66
  store i16 2, ptr %61
  %69 = getelementptr i8, ptr %61, i16 2
  store i8 %68, ptr %69
  %70 = getelementptr i8, ptr %33, i16 12
  %71 = and i8 -1, 1
  %72 = and i8 0, -2
  %73 = or i8 %72, %71
  %74 = and i8 4, 127
  %75 = shl i8 %74, 1
  %76 = and i8 %73, 1
  %77 = or i8 %76, %75
  store i16 2, ptr %70
  %78 = getelementptr i8, ptr %70, i16 2
  store i8 %77, ptr %78
  store ptr %33, ptr %1, !tbaa !2
  %79 = load ptr, ptr %3, !tbaa !2
  store ptr null, ptr %3, !tbaa !2
  %80 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  %81 = load ptr, ptr %1, !tbaa !2
  store ptr null, ptr %1, !tbaa !2
  store ptr %79, ptr addrspace(1) %0
  %82 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %80, ptr addrspace(1) %82
  %83 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %81, ptr addrspace(1) %83
  %84 = load ptr, ptr %1, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %84)
  %85 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %85)
  %86 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %86)
  ret void
}

define internal void @run(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i8
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca ptr
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [4 x i8]
  %8 = alloca i16
  %9 = alloca ptr
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca [8 x i8]
  %15 = alloca [8 x i8]
  %16 = alloca [6 x i8]
  %17 = alloca [8 x i8]
  %18 = alloca [6 x i8]
  %19 = alloca [8 x i8]
  store i8 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %3
  store ptr null, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 4, i1 false)
  store i16 0, ptr %8
  store ptr null, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %15, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %16, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %17, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %18, i8 0, i16 6, i1 false)
  call void @llvm.memset.p0.i16(ptr %19, i8 0, i16 8, i1 false)
  %20 = addrspacecast ptr %19 to ptr addrspace(1)
  %21 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @built(ptr addrspace(1) %21)
  %22 = addrspacecast ptr %18 to ptr addrspace(1)
  %23 = getelementptr i8, ptr @$str2, i16 6
  %24 = getelementptr i8, ptr %23, i16 -4
  %25 = load i16, ptr %24
  %26 = addrspacecast ptr %23 to ptr addrspace(1)
  store i16 %25, ptr %17, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 %25, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %17, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %17 to ptr addrspace(1)
  call addrspace(1) void @Level.save(ptr addrspace(1) %20, ptr addrspace(1) %22, ptr addrspace(1) %29)
  %30 = load i8, ptr %19, !tbaa !2
  %31 = icmp eq i8 %30, 1
  %32 = sext i1 %31 to i8
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %b2, label %b3

b2:
  %34 = getelementptr inbounds i8, ptr %19, i16 2
  %35 = load i16, ptr %34, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %19, i16 4
  %37 = load i16, ptr %36, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %19, i16 6
  %39 = load i16, ptr %38, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %35, ptr addrspace(1) %40
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %37, ptr addrspace(1) %41
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %39, ptr addrspace(1) %42
  %43 = getelementptr inbounds i8, ptr %18, i16 4
  %44 = load ptr, ptr %43, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %44)
  %45 = getelementptr inbounds i8, ptr %18, i16 2
  %46 = load ptr, ptr %45, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %46)
  %47 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %47)
  ret void

b3:
  %48 = getelementptr inbounds i8, ptr %18, i16 4
  %49 = load ptr, ptr %48, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %49)
  %50 = getelementptr inbounds i8, ptr %18, i16 2
  %51 = load ptr, ptr %50, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %51)
  %52 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %52)
  %53 = addrspacecast ptr %15 to ptr addrspace(1)
  %54 = getelementptr i8, ptr @$str2, i16 6
  %55 = getelementptr i8, ptr %54, i16 -4
  %56 = load i16, ptr %55
  %57 = addrspacecast ptr %54 to ptr addrspace(1)
  store i16 %56, ptr %14, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 %56, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %57, ptr %59, !tbaa !2
  %60 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @Level.load(ptr addrspace(1) %53, ptr addrspace(1) %60)
  %61 = load i8, ptr %15, !tbaa !2
  %62 = icmp eq i8 %61, 1
  %63 = sext i1 %62 to i8
  %64 = icmp ne i8 %63, 0
  br i1 %64, label %b4, label %b5

b4:
  %65 = getelementptr inbounds i8, ptr %15, i16 2
  %66 = load i16, ptr %65, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %15, i16 4
  %68 = load i16, ptr %67, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %15, i16 6
  %70 = load i16, ptr %69, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %66, ptr addrspace(1) %71
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %68, ptr addrspace(1) %72
  %73 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %70, ptr addrspace(1) %73
  ret void

b5:
  %74 = getelementptr inbounds i8, ptr %15, i16 2
  %75 = load ptr, ptr %74, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %15, i16 4
  %77 = load ptr, ptr %76, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %15, i16 6
  %79 = load ptr, ptr %78, !tbaa !2
  store ptr %75, ptr %16, !tbaa !2
  %80 = getelementptr inbounds i8, ptr %16, i16 2
  store ptr %77, ptr %80, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %16, i16 4
  store ptr %79, ptr %81, !tbaa !2
  %82 = load ptr, ptr %16, !tbaa !2
  %83 = getelementptr i8, ptr %82, i16 -4
  %84 = load i16, ptr %83
  store i16 %84, ptr %13, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %16, i16 2
  %86 = load ptr, ptr %85, !tbaa !2
  %87 = getelementptr i8, ptr %86, i16 -4
  %88 = load i16, ptr %87
  store i16 %88, ptr %12, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %16, i16 4
  %90 = load ptr, ptr %89, !tbaa !2
  %91 = getelementptr i8, ptr %90, i16 -4
  %92 = load i16, ptr %91
  store i16 %92, ptr %11, !tbaa !2
  %93 = load ptr, ptr %16, !tbaa !2
  %94 = getelementptr i8, ptr %93, i16 -4
  %95 = load i16, ptr %94
  %96 = mul i16 %95, 4
  %97 = add i16 8, %96
  %98 = getelementptr inbounds i8, ptr %16, i16 2
  %99 = load ptr, ptr %98, !tbaa !2
  %100 = getelementptr i8, ptr %99, i16 -4
  %101 = load i16, ptr %100
  %102 = mul i16 %101, 9
  %103 = add i16 %97, %102
  %104 = getelementptr inbounds i8, ptr %16, i16 4
  %105 = load ptr, ptr %104, !tbaa !2
  %106 = getelementptr i8, ptr %105, i16 -4
  %107 = load i16, ptr %106
  %108 = mul i16 %107, 3
  %109 = add i16 %103, %108
  store i16 %109, ptr %10, !tbaa !2
  %110 = load i16, ptr %13, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %110)
  %111 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %111)
  %112 = load i16, ptr %12, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %112)
  %113 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %113)
  %114 = load i16, ptr %11, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %114)
  %115 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %115)
  %116 = load i16, ptr %10, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %116)
  %117 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %117)
  call addrspace(1) void @N$PN()
  %118 = getelementptr i8, ptr @$str1, i16 6
  %119 = call addrspace(1) ptr @N$BGRW(ptr %118, i16 4, i16 4)
  %120 = getelementptr i8, ptr %119, i16 0
  store i16 40, ptr %120
  %121 = getelementptr i8, ptr %120, i16 2
  store i16 50, ptr %121
  %122 = getelementptr i8, ptr %119, i16 4
  store i16 40, ptr %122
  %123 = getelementptr i8, ptr %122, i16 2
  store i16 10, ptr %123
  %124 = getelementptr i8, ptr %119, i16 8
  store i16 8, ptr %124
  %125 = getelementptr i8, ptr %124, i16 2
  store i16 20, ptr %125
  %126 = getelementptr i8, ptr %119, i16 12
  store i16 8, ptr %126
  %127 = getelementptr i8, ptr %126, i16 2
  store i16 4, ptr %127
  store ptr %119, ptr %9, !tbaa !2
  %128 = load ptr, ptr %9, !tbaa !2
  %129 = getelementptr i8, ptr %128, i16 -4
  %130 = load i16, ptr %129
  store i16 0, ptr %8, !tbaa !2
  br label %b6

b6:
  %131 = load i16, ptr %8, !tbaa !2
  %132 = icmp ult i16 %131, %130
  %133 = sext i1 %132 to i8
  %134 = icmp ne i8 %133, 0
  br i1 %134, label %b7, label %b9

b7:
  %135 = mul i16 %131, 4
  %136 = getelementptr i8, ptr %128, i16 %135
  %137 = load i16, ptr %136
  %138 = getelementptr i8, ptr %136, i16 2
  %139 = load i16, ptr %138
  %140 = getelementptr i8, ptr %136, i16 2
  %141 = addrspacecast ptr %140 to ptr addrspace(1)
  %142 = addrspacecast ptr %16 to ptr addrspace(1)
  %143 = load i16, ptr %136
  %144 = load i16, ptr addrspace(1) %141
  %145 = call addrspace(1) i32 @Level.leaf(ptr addrspace(1) %142, i16 %143, i16 %144)
  %146 = addrspacecast ptr %7 to ptr addrspace(1)
  store i32 %145, ptr addrspace(1) %146, !tbaa !2
  %147 = load i16, ptr %7, !tbaa !2
  %148 = getelementptr inbounds i8, ptr %7, i16 2
  %149 = load i16, ptr %148, !tbaa !2
  store i16 %147, ptr %6, !tbaa !2
  store i16 %149, ptr %5, !tbaa !2
  %150 = getelementptr inbounds i8, ptr %16, i16 2
  %151 = load ptr, ptr %150, !tbaa !2
  %152 = load i16, ptr %5, !tbaa !2
  %153 = getelementptr i8, ptr %151, i16 -4
  %154 = load i16, ptr %153
  %155 = icmp ult i16 %152, %154
  %156 = sext i1 %155 to i8
  %157 = icmp ne i8 %156, 0
  br i1 %157, label %b10, label %b11

b8:
  %158 = load i16, ptr %8, !tbaa !2
  %159 = add i16 %158, 1
  store i16 %159, ptr %8, !tbaa !2
  br label %b6

b9:
  store i8 0, ptr addrspace(1) %0
  %160 = load ptr, ptr %9, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %160)
  %161 = getelementptr inbounds i8, ptr %16, i16 4
  %162 = load ptr, ptr %161, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %162)
  %163 = getelementptr inbounds i8, ptr %16, i16 2
  %164 = load ptr, ptr %163, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %164)
  %165 = load ptr, ptr %16, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %165)
  ret void

b10:
  %166 = mul i16 %152, 9
  %167 = getelementptr i8, ptr %151, i16 %166
  %168 = addrspacecast ptr %167 to ptr addrspace(1)
  %169 = getelementptr i8, ptr @$str1, i16 6
  store ptr %169, ptr %4, !tbaa !2
  %170 = getelementptr i8, ptr addrspace(1) %168, i16 6
  %171 = load i16, ptr addrspace(1) %170
  %172 = getelementptr i8, ptr addrspace(1) %168, i16 6
  %173 = load i16, ptr addrspace(1) %172
  %174 = getelementptr i8, ptr addrspace(1) %168, i16 8
  %175 = load i8, ptr addrspace(1) %174
  %176 = zext i8 %175 to i16
  %177 = add i16 %173, %176
  store i16 %171, ptr %3, !tbaa !2
  store i16 %177, ptr %2, !tbaa !2
  br label %b12

b11:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  %178 = load i16, ptr %3, !tbaa !2
  %179 = load i16, ptr %2, !tbaa !2
  %180 = icmp ult i16 %178, %179
  %181 = sext i1 %180 to i8
  %182 = icmp ne i8 %181, 0
  br i1 %182, label %b13, label %b15

b13:
  %183 = getelementptr inbounds i8, ptr %16, i16 4
  %184 = load ptr, ptr %183, !tbaa !2
  %185 = load i16, ptr %3, !tbaa !2
  %186 = getelementptr i8, ptr %184, i16 -4
  %187 = load i16, ptr %186
  %188 = icmp ult i16 %185, %187
  %189 = sext i1 %188 to i8
  %190 = icmp ne i8 %189, 0
  br i1 %190, label %b16, label %b17

b14:
  %191 = load i16, ptr %3, !tbaa !2
  %192 = add i16 %191, 1
  store i16 %192, ptr %3, !tbaa !2
  br label %b12

b15:
  %193 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %193)
  %194 = load i16, ptr %136
  call addrspace(1) void @N$PI2(i16 %194)
  %195 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %195)
  %196 = load i16, ptr addrspace(1) %141
  call addrspace(1) void @N$PI2(i16 %196)
  %197 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %197)
  %198 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %198)
  %199 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %199)
  %200 = load i16, ptr %5, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %200)
  %201 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %201)
  %202 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$PS(ptr %202)
  call addrspace(1) void @N$PN()
  %203 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %203)
  br label %b8

b16:
  %204 = mul i16 %185, 3
  %205 = getelementptr i8, ptr %184, i16 %204
  %206 = addrspacecast ptr %205 to ptr addrspace(1)
  %207 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$PBEG()
  %208 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %208)
  %209 = getelementptr i8, ptr addrspace(1) %206, i16 2
  %210 = load i8, ptr addrspace(1) %209
  %211 = and i8 %210, 1
  %212 = icmp ne i8 %211, 0
  %213 = sext i1 %212 to i8
  %214 = icmp ne i8 %213, 0
  br i1 %214, label %b18, label %b19

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  store i8 98, ptr %1, !tbaa !2
  br label %b20

b19:
  store i8 102, ptr %1, !tbaa !2
  br label %b20

b20:
  %215 = load i8, ptr %1, !tbaa !2
  call addrspace(1) void @N$PC(i8 %215)
  %216 = getelementptr i8, ptr addrspace(1) %206, i16 2
  %217 = load i8, ptr addrspace(1) %216
  %218 = lshr i8 %217, 1
  call addrspace(1) void @N$PU1(i8 %218)
  %219 = call addrspace(1) ptr @N$PEND()
  %220 = call addrspace(1) ptr @N$TCAT(ptr %207, ptr %219)
  %221 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %221)
  store ptr %220, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %219)
  br label %b14
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @run(ptr addrspace(1) %2)
  %3 = load i8, ptr %1, !tbaa !2
  %4 = icmp eq i8 %3, 0
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b4, label %b3

b2:
  ret i16 1

b3:
  %7 = load i8, ptr %1, !tbaa !2
  %8 = icmp eq i8 %7, 1
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b6, label %b5

b4:
  ret i16 0

b5:
  %11 = load i8, ptr %1, !tbaa !2
  %12 = icmp eq i8 %11, 1
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b9, label %b8

b6:
  %15 = getelementptr inbounds i8, ptr %1, i16 2
  %16 = load i8, ptr %15, !tbaa !2
  %17 = icmp eq i8 %16, 0
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b7, label %b5

b7:
  %20 = getelementptr i8, ptr @$str13, i16 6
  call addrspace(1) void @N$PS(ptr %20)
  call addrspace(1) void @N$PN()
  br label %b2

b8:
  %21 = getelementptr inbounds i8, ptr %1, i16 4
  %22 = load i16, ptr %21, !tbaa !2
  store i16 %22, ptr %0, !tbaa !2
  %23 = getelementptr i8, ptr @$str15, i16 6
  call addrspace(1) void @N$PS(ptr %23)
  %24 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %24)
  %25 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %25)
  call addrspace(1) void @N$PN()
  br label %b2

b9:
  %26 = getelementptr inbounds i8, ptr %1, i16 2
  %27 = load i8, ptr %26, !tbaa !2
  %28 = icmp eq i8 %27, 1
  %29 = sext i1 %28 to i8
  %30 = icmp ne i8 %29, 0
  br i1 %30, label %b10, label %b8

b10:
  %31 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %31)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal void @"records[Face]"(ptr addrspace(1) %0, ptr addrspace(1) %1, i16 %2, ptr addrspace(1) %3) addrspace(1) {
b1:
  %4 = alloca [8 x i8]
  %5 = alloca ptr addrspace(1)
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [3 x i8]
  %9 = alloca ptr
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  store ptr addrspace(1) null, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 3, i1 false)
  store ptr null, ptr %9
  %10 = getelementptr i8, ptr @$str1, i16 6
  store ptr %10, ptr %9, !tbaa !2
  %11 = load i16, ptr addrspace(1) %3
  %12 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %13 = load i8, ptr addrspace(1) %12
  store i16 %11, ptr %8, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %8, i16 2
  store i8 %13, ptr %14, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 %2, ptr %6, !tbaa !2
  br label %b2

b2:
  %15 = load i16, ptr %7, !tbaa !2
  %16 = load i16, ptr %6, !tbaa !2
  %17 = icmp ult i16 %15, %16
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b3, label %b5

b3:
  %20 = addrspacecast ptr %8 to ptr addrspace(1)
  store ptr addrspace(1) %20, ptr %5, !tbaa !2
  %21 = addrspacecast ptr %4 to ptr addrspace(1)
  %22 = load ptr addrspace(1), ptr %5, !tbaa !2
  call addrspace(1) void @read_exact(ptr addrspace(1) %21, ptr addrspace(1) %1, ptr addrspace(1) %22, i16 3)
  %23 = load i8, ptr %4, !tbaa !2
  %24 = icmp eq i8 %23, 1
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b6, label %b7

b4:
  %27 = load i16, ptr %7, !tbaa !2
  %28 = add i16 %27, 1
  store i16 %28, ptr %7, !tbaa !2
  br label %b2

b5:
  %29 = load ptr, ptr %9, !tbaa !2
  store ptr null, ptr %9, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %29, ptr addrspace(1) %30
  %31 = load ptr, ptr %9, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %31)
  ret void

b6:
  %32 = getelementptr inbounds i8, ptr %4, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %4, i16 4
  %35 = load i16, ptr %34, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %4, i16 6
  %37 = load i16, ptr %36, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %33, ptr addrspace(1) %38
  %39 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %35, ptr addrspace(1) %39
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %37, ptr addrspace(1) %40
  %41 = load ptr, ptr %9, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %41)
  ret void

b7:
  %42 = load ptr, ptr %9, !tbaa !2
  %43 = getelementptr i8, ptr %42, i16 -4
  %44 = load i16, ptr %43
  %45 = call addrspace(1) ptr @N$BGRW(ptr %42, i16 1, i16 3)
  store ptr %45, ptr %9, !tbaa !2
  %46 = mul i16 %44, 3
  %47 = getelementptr i8, ptr %45, i16 %46
  %48 = load i16, ptr %8, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %8, i16 2
  %50 = load i8, ptr %49, !tbaa !2
  store i16 %48, ptr %47
  %51 = getelementptr i8, ptr %47, i16 2
  store i8 %50, ptr %51
  br label %b4
}

define internal void @"records[Node]"(ptr addrspace(1) %0, ptr addrspace(1) %1, i16 %2, ptr addrspace(1) %3) addrspace(1) {
b1:
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [8 x i8]
  %7 = alloca ptr addrspace(1)
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [9 x i8]
  %13 = alloca ptr
  store i16 0, ptr %4
  store i16 0, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  store ptr addrspace(1) null, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 9, i1 false)
  store ptr null, ptr %13
  %14 = getelementptr i8, ptr @$str1, i16 6
  store ptr %14, ptr %13, !tbaa !2
  %15 = load i16, ptr addrspace(1) %3
  %16 = getelementptr i8, ptr addrspace(1) %3, i16 6
  %17 = load i16, ptr addrspace(1) %16
  %18 = getelementptr i8, ptr addrspace(1) %3, i16 8
  %19 = load i8, ptr addrspace(1) %18
  store i16 %15, ptr %12, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %12, i16 2
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  %22 = getelementptr i8, ptr addrspace(1) %3, i16 2
  store i16 0, ptr %11, !tbaa !2
  store i16 2, ptr %10, !tbaa !2
  br label %b2

b2:
  %23 = load i16, ptr %11, !tbaa !2
  %24 = load i16, ptr %10, !tbaa !2
  %25 = icmp slt i16 %23, %24
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b3, label %b5

b3:
  %28 = load i16, ptr %11, !tbaa !2
  %29 = mul i16 %28, 2
  %30 = getelementptr i8, ptr addrspace(1) %21, i16 %29
  %31 = load i16, ptr %11, !tbaa !2
  %32 = mul i16 %31, 2
  %33 = getelementptr i8, ptr addrspace(1) %22, i16 %32
  %34 = load i16, ptr addrspace(1) %33
  store i16 %34, ptr addrspace(1) %30
  br label %b4

b4:
  %35 = load i16, ptr %11, !tbaa !2
  %36 = add i16 %35, 1
  store i16 %36, ptr %11, !tbaa !2
  br label %b2

b5:
  %37 = getelementptr inbounds i8, ptr %12, i16 6
  store i16 %17, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %12, i16 8
  store i8 %19, ptr %38, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i16 %2, ptr %8, !tbaa !2
  br label %b6

b6:
  %39 = load i16, ptr %9, !tbaa !2
  %40 = load i16, ptr %8, !tbaa !2
  %41 = icmp ult i16 %39, %40
  %42 = sext i1 %41 to i8
  %43 = icmp ne i8 %42, 0
  br i1 %43, label %b7, label %b9

b7:
  %44 = addrspacecast ptr %12 to ptr addrspace(1)
  store ptr addrspace(1) %44, ptr %7, !tbaa !2
  %45 = addrspacecast ptr %6 to ptr addrspace(1)
  %46 = load ptr addrspace(1), ptr %7, !tbaa !2
  call addrspace(1) void @read_exact(ptr addrspace(1) %45, ptr addrspace(1) %1, ptr addrspace(1) %46, i16 9)
  %47 = load i8, ptr %6, !tbaa !2
  %48 = icmp eq i8 %47, 1
  %49 = sext i1 %48 to i8
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b10, label %b11

b8:
  %51 = load i16, ptr %9, !tbaa !2
  %52 = add i16 %51, 1
  store i16 %52, ptr %9, !tbaa !2
  br label %b6

b9:
  %53 = load ptr, ptr %13, !tbaa !2
  store ptr null, ptr %13, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %53, ptr addrspace(1) %54
  %55 = load ptr, ptr %13, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %55)
  ret void

b10:
  %56 = getelementptr inbounds i8, ptr %6, i16 2
  %57 = load i16, ptr %56, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %6, i16 4
  %59 = load i16, ptr %58, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %6, i16 6
  %61 = load i16, ptr %60, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %57, ptr addrspace(1) %62
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %59, ptr addrspace(1) %63
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %61, ptr addrspace(1) %64
  %65 = load ptr, ptr %13, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %65)
  ret void

b11:
  %66 = load ptr, ptr %13, !tbaa !2
  %67 = getelementptr i8, ptr %66, i16 -4
  %68 = load i16, ptr %67
  %69 = call addrspace(1) ptr @N$BGRW(ptr %66, i16 1, i16 9)
  store ptr %69, ptr %13, !tbaa !2
  %70 = mul i16 %68, 9
  %71 = getelementptr i8, ptr %69, i16 %70
  %72 = load i16, ptr %12, !tbaa !2
  %73 = getelementptr inbounds i8, ptr %12, i16 6
  %74 = load i16, ptr %73, !tbaa !2
  %75 = getelementptr inbounds i8, ptr %12, i16 8
  %76 = load i8, ptr %75, !tbaa !2
  store i16 %72, ptr %71
  %77 = getelementptr i8, ptr %71, i16 2
  %78 = addrspacecast ptr %77 to ptr addrspace(1)
  %79 = getelementptr inbounds i8, ptr %12, i16 2
  %80 = addrspacecast ptr %79 to ptr addrspace(1)
  store i16 0, ptr %5, !tbaa !2
  store i16 2, ptr %4, !tbaa !2
  br label %b12

b12:
  %81 = load i16, ptr %5, !tbaa !2
  %82 = load i16, ptr %4, !tbaa !2
  %83 = icmp slt i16 %81, %82
  %84 = sext i1 %83 to i8
  %85 = icmp ne i8 %84, 0
  br i1 %85, label %b13, label %b15

b13:
  %86 = load i16, ptr %5, !tbaa !2
  %87 = mul i16 %86, 2
  %88 = getelementptr i8, ptr addrspace(1) %78, i16 %87
  %89 = load i16, ptr %5, !tbaa !2
  %90 = mul i16 %89, 2
  %91 = getelementptr i8, ptr addrspace(1) %80, i16 %90
  %92 = load i16, ptr addrspace(1) %91
  store i16 %92, ptr addrspace(1) %88
  br label %b14

b14:
  %93 = load i16, ptr %5, !tbaa !2
  %94 = add i16 %93, 1
  store i16 %94, ptr %5, !tbaa !2
  br label %b12

b15:
  %95 = getelementptr i8, ptr %71, i16 6
  store i16 %74, ptr %95
  %96 = getelementptr i8, ptr %71, i16 8
  store i8 %76, ptr %96
  br label %b8
}

define internal void @"records[Plane]"(ptr addrspace(1) %0, ptr addrspace(1) %1, i16 %2, ptr addrspace(1) %3) addrspace(1) {
b1:
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca [8 x i8]
  %7 = alloca ptr addrspace(1)
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca [4 x i8]
  %13 = alloca ptr
  store i16 0, ptr %4
  store i16 0, ptr %5
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  store ptr addrspace(1) null, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  call void @llvm.memset.p0.i16(ptr %12, i8 0, i16 4, i1 false)
  store ptr null, ptr %13
  %14 = getelementptr i8, ptr @$str1, i16 6
  store ptr %14, ptr %13, !tbaa !2
  %15 = getelementptr i8, ptr addrspace(1) %3, i16 2
  %16 = load i16, ptr addrspace(1) %15
  %17 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 0, ptr %11, !tbaa !2
  store i16 2, ptr %10, !tbaa !2
  br label %b2

b2:
  %18 = load i16, ptr %11, !tbaa !2
  %19 = load i16, ptr %10, !tbaa !2
  %20 = icmp slt i16 %18, %19
  %21 = sext i1 %20 to i8
  %22 = icmp ne i8 %21, 0
  br i1 %22, label %b3, label %b5

b3:
  %23 = load i16, ptr %11, !tbaa !2
  %24 = getelementptr i8, ptr addrspace(1) %17, i16 %23
  %25 = load i16, ptr %11, !tbaa !2
  %26 = getelementptr i8, ptr addrspace(1) %3, i16 %25
  %27 = load i8, ptr addrspace(1) %26
  store i8 %27, ptr addrspace(1) %24
  br label %b4

b4:
  %28 = load i16, ptr %11, !tbaa !2
  %29 = add i16 %28, 1
  store i16 %29, ptr %11, !tbaa !2
  br label %b2

b5:
  %30 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 %16, ptr %30, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i16 %2, ptr %8, !tbaa !2
  br label %b6

b6:
  %31 = load i16, ptr %9, !tbaa !2
  %32 = load i16, ptr %8, !tbaa !2
  %33 = icmp ult i16 %31, %32
  %34 = sext i1 %33 to i8
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b7, label %b9

b7:
  %36 = addrspacecast ptr %12 to ptr addrspace(1)
  store ptr addrspace(1) %36, ptr %7, !tbaa !2
  %37 = addrspacecast ptr %6 to ptr addrspace(1)
  %38 = load ptr addrspace(1), ptr %7, !tbaa !2
  call addrspace(1) void @read_exact(ptr addrspace(1) %37, ptr addrspace(1) %1, ptr addrspace(1) %38, i16 4)
  %39 = load i8, ptr %6, !tbaa !2
  %40 = icmp eq i8 %39, 1
  %41 = sext i1 %40 to i8
  %42 = icmp ne i8 %41, 0
  br i1 %42, label %b10, label %b11

b8:
  %43 = load i16, ptr %9, !tbaa !2
  %44 = add i16 %43, 1
  store i16 %44, ptr %9, !tbaa !2
  br label %b6

b9:
  %45 = load ptr, ptr %13, !tbaa !2
  store ptr null, ptr %13, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %46 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %45, ptr addrspace(1) %46
  %47 = load ptr, ptr %13, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %47)
  ret void

b10:
  %48 = getelementptr inbounds i8, ptr %6, i16 2
  %49 = load i16, ptr %48, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %6, i16 4
  %51 = load i16, ptr %50, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %6, i16 6
  %53 = load i16, ptr %52, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %49, ptr addrspace(1) %54
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %51, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %53, ptr addrspace(1) %56
  %57 = load ptr, ptr %13, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %57)
  ret void

b11:
  %58 = load ptr, ptr %13, !tbaa !2
  %59 = getelementptr i8, ptr %58, i16 -4
  %60 = load i16, ptr %59
  %61 = call addrspace(1) ptr @N$BGRW(ptr %58, i16 1, i16 4)
  store ptr %61, ptr %13, !tbaa !2
  %62 = mul i16 %60, 4
  %63 = getelementptr i8, ptr %61, i16 %62
  %64 = getelementptr inbounds i8, ptr %12, i16 2
  %65 = load i16, ptr %64, !tbaa !2
  %66 = addrspacecast ptr %63 to ptr addrspace(1)
  %67 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 0, ptr %5, !tbaa !2
  store i16 2, ptr %4, !tbaa !2
  br label %b12

b12:
  %68 = load i16, ptr %5, !tbaa !2
  %69 = load i16, ptr %4, !tbaa !2
  %70 = icmp slt i16 %68, %69
  %71 = sext i1 %70 to i8
  %72 = icmp ne i8 %71, 0
  br i1 %72, label %b13, label %b15

b13:
  %73 = load i16, ptr %5, !tbaa !2
  %74 = getelementptr i8, ptr addrspace(1) %66, i16 %73
  %75 = load i16, ptr %5, !tbaa !2
  %76 = getelementptr i8, ptr addrspace(1) %67, i16 %75
  %77 = load i8, ptr addrspace(1) %76
  store i8 %77, ptr addrspace(1) %74
  br label %b14

b14:
  %78 = load i16, ptr %5, !tbaa !2
  %79 = add i16 %78, 1
  store i16 %79, ptr %5, !tbaa !2
  br label %b12

b15:
  %80 = getelementptr i8, ptr %63, i16 2
  store i16 %65, ptr %80
  br label %b8
}

define internal void @"written[Face]"(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store ptr addrspace(1) null, ptr %4
  %5 = load ptr, ptr addrspace(1) %2
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load ptr, ptr addrspace(1) %2
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = mul i16 %11, 3
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %12)
  %13 = load i8, ptr %3, !tbaa !2
  %14 = icmp eq i8 %13, 1
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b2, label %b3

b2:
  %17 = getelementptr inbounds i8, ptr %3, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %3, i16 4
  %20 = load i16, ptr %19, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %18, ptr addrspace(1) %22
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %20, ptr addrspace(1) %23
  ret void

b3:
  %24 = getelementptr inbounds i8, ptr %3, i16 2
  %25 = load i16, ptr %24, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal void @"written[Node]"(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store ptr addrspace(1) null, ptr %4
  %5 = load ptr, ptr addrspace(1) %2
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load ptr, ptr addrspace(1) %2
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = mul i16 %11, 9
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %12)
  %13 = load i8, ptr %3, !tbaa !2
  %14 = icmp eq i8 %13, 1
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b2, label %b3

b2:
  %17 = getelementptr inbounds i8, ptr %3, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %3, i16 4
  %20 = load i16, ptr %19, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %18, ptr addrspace(1) %22
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %20, ptr addrspace(1) %23
  ret void

b3:
  %24 = getelementptr inbounds i8, ptr %3, i16 2
  %25 = load i16, ptr %24, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal void @"written[Plane]"(ptr addrspace(1) %0, ptr addrspace(1) %1, ptr addrspace(1) %2) addrspace(1) {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 6, i1 false)
  store ptr addrspace(1) null, ptr %4
  %5 = load ptr, ptr addrspace(1) %2
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load ptr, ptr addrspace(1) %2
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = mul i16 %11, 4
  call addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %12)
  %13 = load i8, ptr %3, !tbaa !2
  %14 = icmp eq i8 %13, 1
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b2, label %b3

b2:
  %17 = getelementptr inbounds i8, ptr %3, i16 2
  %18 = load i16, ptr %17, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %3, i16 4
  %20 = load i16, ptr %19, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %21
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %18, ptr addrspace(1) %22
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store i16 %20, ptr addrspace(1) %23
  ret void

b3:
  %24 = getelementptr inbounds i8, ptr %3, i16 2
  %25 = load i16, ptr %24, !tbaa !2
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
