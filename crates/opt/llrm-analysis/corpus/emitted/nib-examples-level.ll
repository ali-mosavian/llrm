target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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

define internal cc 1005 void @std.os.write(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) %0, i16 %1)
  ret void
}

define internal cc 1005 ptr addrspace(1) @std.os.text_screen() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca ptr addrspace(1)
  %1 = inttoptr i32 -1207959552 to ptr addrspace(1)
  store ptr addrspace(1) %1, ptr %0, !tbaa !2
  %2 = load ptr addrspace(1), ptr %0, !tbaa !2
  ret ptr addrspace(1) %2
}

define internal cc 1005 i32 @std.io.error(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [4 x i8]
  %2 = icmp eq i16 %0, -2
  br i1 %2, label %b2, label %b5

b2:
  store i32 0, ptr %1, !tbaa !2
  %3 = addrspacecast ptr %1 to ptr addrspace(1)
  %4 = load i32, ptr addrspace(1) %3, !tbaa !2
  ret i32 %4

b3:
  br label %b4

b4:
  %5 = icmp eq i16 %0, -5
  br i1 %5, label %b6, label %b7

b5:
  %6 = icmp eq i16 %0, -3
  br i1 %6, label %b2, label %b3

b6:
  store i32 1, ptr %1, !tbaa !2
  %7 = addrspacecast ptr %1 to ptr addrspace(1)
  %8 = load i32, ptr addrspace(1) %7, !tbaa !2
  ret i32 %8

b7:
  br label %b8

b8:
  %9 = sub i16 0, %0
  store i16 2, ptr %1, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %9, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %1 to ptr addrspace(1)
  %12 = load i32, ptr addrspace(1) %11, !tbaa !2
  ret i32 %12
}

define internal cc 1005 void @std.io.named(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) nonnull dereferenceable(80) noalias nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  %4 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %2, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp ult i16 %5, %4
  br i1 %6, label %b3, label %b5

b3:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %8 = load ptr addrspace(1), ptr addrspace(1) %7
  %9 = getelementptr inbounds i8, ptr addrspace(1) %8, i16 %5
  %10 = load i16, ptr %3, !tbaa !2
  %11 = icmp ult i16 %10, 79
  br i1 %11, label %b6, label %b7

b4:
  %12 = load i16, ptr %2, !tbaa !2
  %13 = add i16 %12, 1
  store i16 %13, ptr %2, !tbaa !2
  br label %b2

b5:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = icmp ult i16 %14, 80
  br i1 %15, label %b11, label %b12

b6:
  %16 = load i16, ptr %3, !tbaa !2
  %17 = icmp ult i16 %16, 80
  br i1 %17, label %b9, label %b10

b7:
  br label %b8

b8:
  br label %b4

b9:
  %18 = getelementptr inbounds i8, ptr addrspace(1) %1, i16 %16
  %19 = load i8, ptr addrspace(1) %9
  store i8 %19, ptr addrspace(1) %18
  %20 = load i16, ptr %3, !tbaa !2
  %21 = add i16 %20, 1
  store i16 %21, ptr %3, !tbaa !2
  br label %b8

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  %22 = getelementptr inbounds i8, ptr addrspace(1) %1, i16 %14
  store i8 0, ptr addrspace(1) %22
  ret void

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @std.io.opened(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [4 x i8]
  %5 = icmp slt i16 %1, 0
  br i1 %5, label %b2, label %b3

b2:
  %6 = call cc 1005 addrspace(1) i32 @std.io.error(i16 %1)
  %7 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %6, ptr addrspace(1) %7, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %8, ptr %4, i16 4, i1 false)
  ret void

b3:
  br label %b4

b4:
  store i8 0, ptr addrspace(1) %0
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %1, ptr addrspace(1) %9
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 0, ptr %3, !tbaa !2
  store i16 128, ptr %2, !tbaa !2
  br label %b5

b5:
  %11 = load i16, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = icmp slt i16 %11, %12
  br i1 %13, label %b6, label %b8

b6:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = getelementptr inbounds i8, ptr addrspace(1) %10, i16 %14
  store i8 0, ptr addrspace(1) %15
  br label %b7

b7:
  %16 = load i16, ptr %3, !tbaa !2
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %3, !tbaa !2
  br label %b5

b8:
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 0, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 0, ptr addrspace(1) %19
  ret void
}

define internal cc 1005 void @std.io.File.open(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [136 x i8]
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [80 x i8]
  %9 = alloca i8
  store i8 0, ptr %9, !tbaa !2
  store i16 80, ptr %6, !tbaa !2
  store i16 80, ptr %7, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 80, ptr %4, !tbaa !2
  br label %b2

b2:
  %10 = load i16, ptr %5, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %b3, label %b5

b3:
  %13 = load i16, ptr %5, !tbaa !2
  %14 = load i8, ptr %9, !tbaa !2
  %15 = sub i16 %13, 0
  %16 = getelementptr inbounds i8, ptr %8, i16 %15
  store i8 %14, ptr %16, !tbaa !2
  br label %b4

b4:
  %17 = load i16, ptr %5, !tbaa !2
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %5, !tbaa !2
  br label %b2

b5:
  %19 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.named(ptr addrspace(1) %1, ptr addrspace(1) %19)
  %20 = addrspacecast ptr %3 to ptr addrspace(1)
  %21 = addrspacecast ptr %8 to ptr addrspace(1)
  %22 = call addrspace(1) i16 @LL$OPEN(ptr addrspace(1) %21, i8 zeroext %2)
  call cc 1005 addrspace(1) void @std.io.opened(ptr addrspace(1) %20, i16 %22)
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %0, ptr %3, i16 136, i1 false)
  ret void
}

define internal cc 1005 void @std.io.File.create(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [136 x i8]
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [80 x i8]
  %8 = alloca i8
  store i8 0, ptr %8, !tbaa !2
  store i16 80, ptr %5, !tbaa !2
  store i16 80, ptr %6, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 80, ptr %3, !tbaa !2
  br label %b2

b2:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = icmp slt i16 %9, %10
  br i1 %11, label %b3, label %b5

b3:
  %12 = load i16, ptr %4, !tbaa !2
  %13 = load i8, ptr %8, !tbaa !2
  %14 = sub i16 %12, 0
  %15 = getelementptr inbounds i8, ptr %7, i16 %14
  store i8 %13, ptr %15, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %4, !tbaa !2
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %4, !tbaa !2
  br label %b2

b5:
  %18 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.named(ptr addrspace(1) %1, ptr addrspace(1) %18)
  %19 = addrspacecast ptr %2 to ptr addrspace(1)
  %20 = addrspacecast ptr %7 to ptr addrspace(1)
  %21 = call addrspace(1) i16 @LL$CREATE(ptr addrspace(1) %20)
  call cc 1005 addrspace(1) void @std.io.opened(ptr addrspace(1) %19, i16 %21)
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %0, ptr %2, i16 136, i1 false)
  ret void
}

define internal cc 1005 void @std.io.File.write_raw(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, ptr addrspace(1) %2, i16 %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca [4 x i8]
  %5 = alloca i32
  %6 = load i16, ptr addrspace(1) %1
  %7 = call addrspace(1) i32 @LL$WRITE_FILE(i16 %6, ptr addrspace(1) %2, i16 %3)
  store i32 %7, ptr %5, !tbaa !2
  %8 = load i32, ptr %5, !tbaa !2
  %9 = icmp slt i32 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  %10 = load i32, ptr %5, !tbaa !2
  %11 = trunc i32 %10 to i16
  %12 = call cc 1005 addrspace(1) i32 @std.io.error(i16 %11)
  %13 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %12, ptr addrspace(1) %13, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %14, ptr %4, i16 4, i1 false)
  ret void

b3:
  br label %b4

b4:
  %15 = load i32, ptr %5, !tbaa !2
  %16 = trunc i32 %15 to i16
  store i8 0, ptr addrspace(1) %0
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %16, ptr addrspace(1) %17
  ret void
}

define internal cc 1005 void @std.io.File.write(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  %5 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load i16, ptr addrspace(1) %2
  call cc 1005 addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %9)
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %0, ptr %3, i16 6, i1 false)
  ret void
}

define internal cc 1005 void @std.io.File.read_raw(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, ptr addrspace(1) %2, i16 %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca [4 x i8]
  %5 = alloca i32
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %6, !tbaa !2
  %8 = icmp ult i16 %7, %3
  br i1 %8, label %b5, label %b4

b3:
  %9 = load i16, ptr %6, !tbaa !2
  %10 = mul i16 %9, 1
  %11 = getelementptr i8, ptr addrspace(1) %2, i16 %10
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %14 = load i16, ptr addrspace(1) %13
  %15 = getelementptr inbounds i8, ptr addrspace(1) %12, i16 %14
  %16 = load i8, ptr addrspace(1) %15
  store i8 %16, ptr addrspace(1) %11
  %17 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %18 = load i16, ptr addrspace(1) %17
  %19 = add i16 %18, 1
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 130
  store i16 %19, ptr addrspace(1) %20
  %21 = load i16, ptr %6, !tbaa !2
  %22 = add i16 %21, 1
  store i16 %22, ptr %6, !tbaa !2
  br label %b2

b4:
  %23 = load i16, ptr %6, !tbaa !2
  %24 = icmp ult i16 %23, %3
  br i1 %24, label %b6, label %b7

b5:
  %25 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %26 = load i16, ptr addrspace(1) %25
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 132
  %28 = load i16, ptr addrspace(1) %27
  %29 = icmp ult i16 %26, %28
  br i1 %29, label %b3, label %b4

b6:
  %30 = load i16, ptr addrspace(1) %1
  %31 = load i16, ptr %6, !tbaa !2
  %32 = mul i16 %31, 1
  %33 = getelementptr i8, ptr addrspace(1) %2, i16 %32
  %34 = load i16, ptr %6, !tbaa !2
  %35 = sub i16 %3, %34
  %36 = call addrspace(1) i32 @LL$READ(i16 %30, ptr addrspace(1) %33, i16 %35)
  store i32 %36, ptr %5, !tbaa !2
  %37 = load i32, ptr %5, !tbaa !2
  %38 = icmp slt i32 %37, 0
  br i1 %38, label %b9, label %b10

b7:
  br label %b8

b8:
  %39 = load i16, ptr %6, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %39, ptr addrspace(1) %40
  ret void

b9:
  %41 = load i32, ptr %5, !tbaa !2
  %42 = trunc i32 %41 to i16
  %43 = call cc 1005 addrspace(1) i32 @std.io.error(i16 %42)
  %44 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %43, ptr addrspace(1) %44, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %45 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %45, ptr %4, i16 4, i1 false)
  ret void

b10:
  br label %b11

b11:
  %46 = load i16, ptr %6, !tbaa !2
  %47 = load i32, ptr %5, !tbaa !2
  %48 = trunc i32 %47 to i16
  %49 = add i16 %46, %48
  store i16 %49, ptr %6, !tbaa !2
  br label %b8
}

define internal cc 1005 void @std.io.File.read_line(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = alloca [4 x i8]
  %4 = alloca [4 x i8]
  %5 = alloca i32
  %6 = alloca ptr addrspace(1)
  %7 = alloca i8
  %8 = alloca ptr
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
  br i1 %14, label %b5, label %b6

b5:
  %15 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store ptr addrspace(1) %15, ptr %6, !tbaa !2
  %16 = load i16, ptr addrspace(1) %1
  %17 = load ptr addrspace(1), ptr %6, !tbaa !2
  %18 = call addrspace(1) i32 @LL$READ(i16 %16, ptr addrspace(1) %17, i16 128)
  store i32 %18, ptr %5, !tbaa !2
  %19 = load i32, ptr %5, !tbaa !2
  %20 = icmp slt i32 %19, 0
  br i1 %20, label %b8, label %b9

b6:
  br label %b7

b7:
  %21 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %22 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %23 = load i16, ptr addrspace(1) %22
  %24 = icmp ult i16 %23, 128
  br i1 %24, label %b17, label %b18

b8:
  %25 = load i32, ptr %5, !tbaa !2
  %26 = trunc i32 %25 to i16
  %27 = call cc 1005 addrspace(1) i32 @std.io.error(i16 %26)
  %28 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 %27, ptr addrspace(1) %28, !tbaa !2
  store i8 1, ptr addrspace(1) %0
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %29, ptr %4, i16 4, i1 false)
  %30 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %30)
  ret void

b9:
  br label %b10

b10:
  %31 = load i32, ptr %5, !tbaa !2
  %32 = icmp eq i32 %31, 0
  br i1 %32, label %b11, label %b12

b11:
  %33 = load i8, ptr %7, !tbaa !2, !range !5
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b14, label %b15

b12:
  br label %b13

b13:
  %35 = getelementptr i8, ptr addrspace(1) %1, i16 130
  store i16 0, ptr addrspace(1) %35
  %36 = load i32, ptr %5, !tbaa !2
  %37 = trunc i32 %36 to i16
  %38 = getelementptr i8, ptr addrspace(1) %1, i16 132
  store i16 %37, ptr addrspace(1) %38
  br label %b7

b14:
  %39 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %3, i16 2
  store ptr %39, ptr %40, !tbaa !2
  br label %b16

b15:
  store i32 1, ptr %3, !tbaa !2
  br label %b16

b16:
  store i8 0, ptr addrspace(1) %0
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %41, ptr %3, i16 4, i1 false)
  %42 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %42)
  ret void

b17:
  %43 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 %23
  %44 = load i8, ptr addrspace(1) %43
  store i8 %44, ptr %2, !tbaa !2
  %45 = getelementptr i8, ptr addrspace(1) %1, i16 130
  %46 = load i16, ptr addrspace(1) %45
  %47 = add i16 %46, 1
  %48 = getelementptr i8, ptr addrspace(1) %1, i16 130
  store i16 %47, ptr addrspace(1) %48
  store i8 1, ptr %7, !tbaa !2
  %49 = load i8, ptr %2, !tbaa !2
  %50 = zext i8 %49 to i16
  %51 = icmp eq i16 %50, 10
  br i1 %51, label %b19, label %b20

b18:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  %52 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 0, ptr addrspace(1) %53
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %52, ptr addrspace(1) %54
  %55 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %55)
  ret void

b20:
  br label %b21

b21:
  %56 = load i8, ptr %2, !tbaa !2
  %57 = zext i8 %56 to i16
  %58 = icmp ne i16 %57, 13
  br i1 %58, label %b22, label %b23

b22:
  %59 = load ptr, ptr %8, !tbaa !2
  %60 = getelementptr i8, ptr %59, i16 -4
  %61 = load i16, ptr %60
  %62 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %59, i16 1, i16 1)
  store ptr %62, ptr %8, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %62, i16 %61
  %64 = load i8, ptr %2, !tbaa !2
  store i8 %64, ptr %63
  %65 = getelementptr i8, ptr %62, i16 -4
  %66 = load i16, ptr %65
  %67 = getelementptr inbounds i8, ptr %62, i16 %66
  store i8 0, ptr %67
  br label %b24

b23:
  br label %b24

b24:
  br label %b2
}

define internal cc 1005 void @std.io.File.drop(ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = call addrspace(1) i16 @LL$CLOSE(i16 %1)
  ret void
}

define internal cc 1005 void @read_exact(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, ptr addrspace(1) %2, i16 %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca i16
  %5 = alloca [6 x i8]
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.read_raw(ptr addrspace(1) %6, ptr addrspace(1) %1, ptr addrspace(1) %2, i16 %3)
  %7 = load i8, ptr %5, !tbaa !2, !range !17
  %8 = icmp eq i8 %7, 1
  br i1 %8, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %9
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %11 = getelementptr inbounds i8, ptr %5, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %10, ptr %11, i16 4, i1 false)
  ret void

b3:
  %12 = getelementptr inbounds i8, ptr %5, i16 2
  %13 = load i16, ptr %12, !tbaa !2
  store i16 %13, ptr %4, !tbaa !2
  %14 = load i16, ptr %4, !tbaa !2
  %15 = icmp ult i16 %14, %3
  br i1 %15, label %b4, label %b5

b4:
  %16 = load i16, ptr %4, !tbaa !2
  %17 = sub i16 %3, %16
  store i8 1, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 2, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %17, ptr addrspace(1) %19
  ret void

b5:
  br label %b6

b6:
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 void @Level.save(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %14 = addrspacecast ptr %12 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.create(ptr addrspace(1) %14, ptr addrspace(1) %2)
  %15 = load i8, ptr %12, !tbaa !2, !range !18
  %16 = icmp eq i8 %15, 1
  br i1 %16, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %17
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %19 = getelementptr inbounds i8, ptr %12, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %18, ptr %19, i16 4, i1 false)
  ret void

b3:
  %20 = getelementptr inbounds i8, ptr %12, i16 2
  %21 = load i16, ptr %20, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %12, i16 132
  %23 = load i16, ptr %22, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %12, i16 134
  %25 = load i16, ptr %24, !tbaa !2
  store i16 %21, ptr %13, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %13, i16 2
  %27 = addrspacecast ptr %26 to ptr addrspace(1)
  %28 = getelementptr inbounds i8, ptr %12, i16 4
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 0, ptr %11, !tbaa !2
  store i16 128, ptr %10, !tbaa !2
  br label %b4

b4:
  %30 = load i16, ptr %11, !tbaa !2
  %31 = load i16, ptr %10, !tbaa !2
  %32 = icmp slt i16 %30, %31
  br i1 %32, label %b5, label %b7

b5:
  %33 = load i16, ptr %11, !tbaa !2
  %34 = getelementptr inbounds i8, ptr addrspace(1) %27, i16 %33
  %35 = load i16, ptr %11, !tbaa !2
  %36 = getelementptr inbounds i8, ptr addrspace(1) %29, i16 %35
  %37 = load i8, ptr addrspace(1) %36
  store i8 %37, ptr addrspace(1) %34
  br label %b6

b6:
  %38 = load i16, ptr %11, !tbaa !2
  %39 = add nsw i16 %38, 1
  store i16 %39, ptr %11, !tbaa !2
  br label %b4

b7:
  %40 = getelementptr inbounds i8, ptr %13, i16 130
  store i16 %23, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %13, i16 132
  store i16 %25, ptr %41, !tbaa !2
  store i8 1, ptr %9, !tbaa !2
  %42 = load ptr, ptr addrspace(1) %1
  %43 = getelementptr i8, ptr %42, i16 -4
  %44 = load i16, ptr %43
  %45 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %46 = load ptr, ptr addrspace(1) %45
  %47 = getelementptr i8, ptr %46, i16 -4
  %48 = load i16, ptr %47
  %49 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %50 = load ptr, ptr addrspace(1) %49
  %51 = getelementptr i8, ptr %50, i16 -4
  %52 = load i16, ptr %51
  store i16 22092, ptr %8, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %44, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %8, i16 4
  store i16 %48, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %8, i16 6
  store i16 %52, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %8 to ptr addrspace(1)
  store ptr addrspace(1) %56, ptr %7, !tbaa !2
  %57 = addrspacecast ptr %6 to ptr addrspace(1)
  %58 = addrspacecast ptr %13 to ptr addrspace(1)
  %59 = load ptr addrspace(1), ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %57, ptr addrspace(1) %58, ptr addrspace(1) %59, i16 8)
  %60 = load i8, ptr %6, !tbaa !2, !range !18
  %61 = icmp eq i8 %60, 1
  br i1 %61, label %b8, label %b9

b8:
  store i8 1, ptr addrspace(1) %0
  %62 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %62
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %64 = getelementptr inbounds i8, ptr %6, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %63, ptr %64, i16 4, i1 false)
  %65 = load i8, ptr %9, !tbaa !2, !range !6
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b11, label %b10

b9:
  %67 = getelementptr inbounds i8, ptr %6, i16 2
  %68 = load i16, ptr %67, !tbaa !2
  %69 = addrspacecast ptr %5 to ptr addrspace(1)
  %70 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @"written[Plane]"(ptr addrspace(1) %69, ptr addrspace(1) %70, ptr addrspace(1) %1)
  %71 = load i8, ptr %5, !tbaa !2, !range !18
  %72 = icmp eq i8 %71, 1
  br i1 %72, label %b12, label %b13

b10:
  ret void

b11:
  %73 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %73)
  br label %b10

b12:
  store i8 1, ptr addrspace(1) %0
  %74 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %75 = getelementptr inbounds i8, ptr %5, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %74, ptr %75, i16 6, i1 false)
  %76 = load i8, ptr %9, !tbaa !2, !range !7
  %77 = icmp ne i8 %76, 0
  br i1 %77, label %b15, label %b14

b13:
  %78 = addrspacecast ptr %4 to ptr addrspace(1)
  %79 = addrspacecast ptr %13 to ptr addrspace(1)
  %80 = getelementptr i8, ptr addrspace(1) %1, i16 2
  call cc 1005 addrspace(1) void @"written[Node]"(ptr addrspace(1) %78, ptr addrspace(1) %79, ptr addrspace(1) %80)
  %81 = load i8, ptr %4, !tbaa !2, !range !18
  %82 = icmp eq i8 %81, 1
  br i1 %82, label %b16, label %b17

b14:
  ret void

b15:
  %83 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %83)
  br label %b14

b16:
  store i8 1, ptr addrspace(1) %0
  %84 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %85 = getelementptr inbounds i8, ptr %4, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %84, ptr %85, i16 6, i1 false)
  %86 = load i8, ptr %9, !tbaa !2, !range !8
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b19, label %b18

b17:
  %88 = addrspacecast ptr %3 to ptr addrspace(1)
  %89 = addrspacecast ptr %13 to ptr addrspace(1)
  %90 = getelementptr i8, ptr addrspace(1) %1, i16 4
  call cc 1005 addrspace(1) void @"written[Face]"(ptr addrspace(1) %88, ptr addrspace(1) %89, ptr addrspace(1) %90)
  %91 = load i8, ptr %3, !tbaa !2, !range !18
  %92 = icmp eq i8 %91, 1
  br i1 %92, label %b20, label %b21

b18:
  ret void

b19:
  %93 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %93)
  br label %b18

b20:
  store i8 1, ptr addrspace(1) %0
  %94 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %95 = getelementptr inbounds i8, ptr %3, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %94, ptr %95, i16 6, i1 false)
  %96 = load i8, ptr %9, !tbaa !2, !range !9
  %97 = icmp ne i8 %96, 0
  br i1 %97, label %b23, label %b22

b21:
  store i8 0, ptr addrspace(1) %0
  %98 = load i8, ptr %9, !tbaa !2, !range !10
  %99 = icmp ne i8 %98, 0
  br i1 %99, label %b25, label %b24

b22:
  ret void

b23:
  %100 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %100)
  br label %b22

b24:
  ret void

b25:
  %101 = addrspacecast ptr %13 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %101)
  br label %b24
}

define internal cc 1005 void @Level.load(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %19 = addrspacecast ptr %17 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.open(ptr addrspace(1) %19, ptr addrspace(1) %1, i8 zeroext 0)
  %20 = load i8, ptr %17, !tbaa !2, !range !19
  %21 = icmp eq i8 %20, 1
  br i1 %21, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %22 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %22
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %24 = getelementptr inbounds i8, ptr %17, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %23, ptr %24, i16 4, i1 false)
  ret void

b3:
  %25 = getelementptr inbounds i8, ptr %17, i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %17, i16 132
  %28 = load i16, ptr %27, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %17, i16 134
  %30 = load i16, ptr %29, !tbaa !2
  store i16 %26, ptr %18, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %18, i16 2
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  %33 = getelementptr inbounds i8, ptr %17, i16 4
  %34 = addrspacecast ptr %33 to ptr addrspace(1)
  store i16 0, ptr %16, !tbaa !2
  store i16 128, ptr %15, !tbaa !2
  br label %b4

b4:
  %35 = load i16, ptr %16, !tbaa !2
  %36 = load i16, ptr %15, !tbaa !2
  %37 = icmp slt i16 %35, %36
  br i1 %37, label %b5, label %b7

b5:
  %38 = load i16, ptr %16, !tbaa !2
  %39 = getelementptr inbounds i8, ptr addrspace(1) %32, i16 %38
  %40 = load i16, ptr %16, !tbaa !2
  %41 = getelementptr inbounds i8, ptr addrspace(1) %34, i16 %40
  %42 = load i8, ptr addrspace(1) %41
  store i8 %42, ptr addrspace(1) %39
  br label %b6

b6:
  %43 = load i16, ptr %16, !tbaa !2
  %44 = add nsw i16 %43, 1
  store i16 %44, ptr %16, !tbaa !2
  br label %b4

b7:
  %45 = getelementptr inbounds i8, ptr %18, i16 130
  store i16 %28, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %18, i16 132
  store i16 %30, ptr %46, !tbaa !2
  store i8 1, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 0, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %13, i16 4
  store i16 0, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %13, i16 6
  store i16 0, ptr %49, !tbaa !2
  %50 = addrspacecast ptr %13 to ptr addrspace(1)
  store ptr addrspace(1) %50, ptr %12, !tbaa !2
  %51 = addrspacecast ptr %11 to ptr addrspace(1)
  %52 = addrspacecast ptr %18 to ptr addrspace(1)
  %53 = load ptr addrspace(1), ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @read_exact(ptr addrspace(1) %51, ptr addrspace(1) %52, ptr addrspace(1) %53, i16 8)
  %54 = load i8, ptr %11, !tbaa !2, !range !19
  %55 = icmp eq i8 %54, 1
  br i1 %55, label %b8, label %b9

b8:
  store i8 1, ptr addrspace(1) %0
  %56 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %57 = getelementptr inbounds i8, ptr %11, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %56, ptr %57, i16 6, i1 false)
  %58 = load i8, ptr %14, !tbaa !2, !range !11
  %59 = icmp ne i8 %58, 0
  br i1 %59, label %b11, label %b10

b9:
  %60 = load i16, ptr %13, !tbaa !2
  %61 = icmp ne i16 %60, 22092
  br i1 %61, label %b12, label %b13

b10:
  ret void

b11:
  %62 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %62)
  br label %b10

b12:
  store i8 1, ptr addrspace(1) %0
  %63 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 1, ptr addrspace(1) %63
  %64 = load i8, ptr %14, !tbaa !2, !range !12
  %65 = icmp ne i8 %64, 0
  br i1 %65, label %b16, label %b15

b13:
  br label %b14

b14:
  %66 = addrspacecast ptr %10 to ptr addrspace(1)
  %67 = addrspacecast ptr %18 to ptr addrspace(1)
  %68 = getelementptr inbounds i8, ptr %13, i16 2
  %69 = load i16, ptr %68, !tbaa !2
  store i8 0, ptr %9, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %9, i16 1
  store i8 0, ptr %70, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 0, ptr %71, !tbaa !2
  %72 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @"records[Plane]"(ptr addrspace(1) %66, ptr addrspace(1) %67, i16 %69, ptr addrspace(1) %72)
  %73 = load i8, ptr %10, !tbaa !2, !range !19
  %74 = icmp eq i8 %73, 1
  br i1 %74, label %b17, label %b18

b15:
  ret void

b16:
  %75 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %75)
  br label %b15

b17:
  store i8 1, ptr addrspace(1) %0
  %76 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %77 = getelementptr inbounds i8, ptr %10, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %76, ptr %77, i16 6, i1 false)
  %78 = load i8, ptr %14, !tbaa !2, !range !13
  %79 = icmp ne i8 %78, 0
  br i1 %79, label %b20, label %b19

b18:
  %80 = getelementptr inbounds i8, ptr %10, i16 2
  %81 = load ptr, ptr %80, !tbaa !2
  store ptr %81, ptr %8, !tbaa !2
  %82 = addrspacecast ptr %7 to ptr addrspace(1)
  %83 = addrspacecast ptr %18 to ptr addrspace(1)
  %84 = getelementptr inbounds i8, ptr %13, i16 4
  %85 = load i16, ptr %84, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 0, ptr %86, !tbaa !2
  %87 = getelementptr inbounds i8, ptr %6, i16 4
  store i16 0, ptr %87, !tbaa !2
  %88 = getelementptr inbounds i8, ptr %6, i16 6
  store i16 0, ptr %88, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %6, i16 8
  store i8 0, ptr %89, !tbaa !2
  %90 = addrspacecast ptr %6 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @"records[Node]"(ptr addrspace(1) %82, ptr addrspace(1) %83, i16 %85, ptr addrspace(1) %90)
  %91 = load i8, ptr %7, !tbaa !2, !range !19
  %92 = icmp eq i8 %91, 1
  br i1 %92, label %b21, label %b22

b19:
  ret void

b20:
  %93 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %93)
  br label %b19

b21:
  store i8 1, ptr addrspace(1) %0
  %94 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %95 = getelementptr inbounds i8, ptr %7, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %94, ptr %95, i16 6, i1 false)
  %96 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %96)
  %97 = load i8, ptr %14, !tbaa !2, !range !14
  %98 = icmp ne i8 %97, 0
  br i1 %98, label %b24, label %b23

b22:
  %99 = getelementptr inbounds i8, ptr %7, i16 2
  %100 = load ptr, ptr %99, !tbaa !2
  store ptr %100, ptr %5, !tbaa !2
  %101 = addrspacecast ptr %4 to ptr addrspace(1)
  %102 = addrspacecast ptr %18 to ptr addrspace(1)
  %103 = getelementptr inbounds i8, ptr %13, i16 6
  %104 = load i16, ptr %103, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %3, i16 2
  store i8 0, ptr %105, !tbaa !2
  %106 = addrspacecast ptr %3 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @"records[Face]"(ptr addrspace(1) %101, ptr addrspace(1) %102, i16 %104, ptr addrspace(1) %106)
  %107 = load i8, ptr %4, !tbaa !2, !range !19
  %108 = icmp eq i8 %107, 1
  br i1 %108, label %b25, label %b26

b23:
  ret void

b24:
  %109 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %109)
  br label %b23

b25:
  store i8 1, ptr addrspace(1) %0
  %110 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %111 = getelementptr inbounds i8, ptr %4, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %110, ptr %111, i16 6, i1 false)
  %112 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %112)
  %113 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %113)
  %114 = load i8, ptr %14, !tbaa !2, !range !15
  %115 = icmp ne i8 %114, 0
  br i1 %115, label %b28, label %b27

b26:
  %116 = getelementptr inbounds i8, ptr %4, i16 2
  %117 = load ptr, ptr %116, !tbaa !2
  store ptr %117, ptr %2, !tbaa !2
  %118 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  %119 = load ptr, ptr %5, !tbaa !2
  store ptr null, ptr %5, !tbaa !2
  %120 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %121 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %118, ptr addrspace(1) %121
  %122 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %119, ptr addrspace(1) %122
  %123 = getelementptr i8, ptr addrspace(1) %0, i16 6
  store ptr %120, ptr addrspace(1) %123
  %124 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %124)
  %125 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %125)
  %126 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %126)
  %127 = load i8, ptr %14, !tbaa !2, !range !16
  %128 = icmp ne i8 %127, 0
  br i1 %128, label %b30, label %b29

b27:
  ret void

b28:
  %129 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %129)
  br label %b27

b29:
  ret void

b30:
  %130 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %130)
  br label %b29
}

define internal cc 1005 i32 @Level.leaf(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias nocapture %0, i16 %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [4 x i8]
  store i16 0, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %6, !tbaa !2
  %9 = icmp sge i16 %8, 0
  br i1 %9, label %b3, label %b4

b3:
  %10 = load i16, ptr %6, !tbaa !2
  store i16 %10, ptr %5, !tbaa !2
  %11 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %12 = load ptr, ptr addrspace(1) %11
  %13 = load i16, ptr %5, !tbaa !2
  %14 = getelementptr i8, ptr %12, i16 -4
  %15 = load i16, ptr %14
  %16 = icmp ult i16 %13, %15
  br i1 %16, label %b5, label %b6

b4:
  %17 = load i16, ptr %6, !tbaa !2
  %18 = sub i16 -1, %17
  %19 = load i16, ptr %5, !tbaa !2
  store i16 %18, ptr %7, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %19, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %7 to ptr addrspace(1)
  %22 = load i32, ptr addrspace(1) %21, !tbaa !2
  ret i32 %22

b5:
  %23 = mul i16 %13, 9
  %24 = getelementptr inbounds i8, ptr %12, i16 %23
  %25 = addrspacecast ptr %24 to ptr addrspace(1)
  %26 = load ptr, ptr addrspace(1) %0
  %27 = load i16, ptr addrspace(1) %25
  %28 = getelementptr i8, ptr %26, i16 -4
  %29 = load i16, ptr %28
  %30 = icmp ult i16 %27, %29
  br i1 %30, label %b7, label %b8

b6:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b7:
  %31 = mul i16 %27, 4
  %32 = getelementptr inbounds i8, ptr %26, i16 %31
  %33 = addrspacecast ptr %32 to ptr addrspace(1)
  %34 = getelementptr inbounds i8, ptr addrspace(1) %33, i16 0
  %35 = load i8, ptr addrspace(1) %34
  %36 = sext i8 %35 to i16
  %37 = mul i16 %36, %1
  %38 = getelementptr inbounds i8, ptr addrspace(1) %33, i16 1
  %39 = load i8, ptr addrspace(1) %38
  %40 = sext i8 %39 to i16
  %41 = mul i16 %40, %2
  %42 = add i16 %37, %41
  %43 = getelementptr i8, ptr addrspace(1) %33, i16 2
  %44 = load i16, ptr addrspace(1) %43
  %45 = sub i16 %42, %44
  store i16 %45, ptr %4, !tbaa !2
  %46 = getelementptr i8, ptr addrspace(1) %25, i16 2
  %47 = load i16, ptr %4, !tbaa !2
  %48 = icmp sge i16 %47, 0
  br i1 %48, label %b9, label %b10

b8:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b9:
  store i16 0, ptr %3, !tbaa !2
  br label %b11

b10:
  store i16 1, ptr %3, !tbaa !2
  br label %b11

b11:
  %49 = load i16, ptr %3, !tbaa !2
  %50 = icmp ult i16 %49, 2
  br i1 %50, label %b12, label %b13

b12:
  %51 = mul i16 %49, 2
  %52 = getelementptr inbounds i8, ptr addrspace(1) %46, i16 %51
  %53 = load i16, ptr addrspace(1) %52
  store i16 %53, ptr %6, !tbaa !2
  br label %b2

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @built(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr
  %2 = alloca ptr
  %3 = alloca ptr
  %4 = getelementptr i8, ptr @$str1, i16 6
  %5 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %4, i16 3, i16 4)
  %6 = getelementptr inbounds i8, ptr %5, i16 0
  store i8 1, ptr %6
  %7 = getelementptr i8, ptr %6, i16 1
  store i8 0, ptr %7
  %8 = getelementptr i8, ptr %6, i16 2
  store i16 32, ptr %8
  %9 = getelementptr inbounds i8, ptr %5, i16 4
  store i8 0, ptr %9
  %10 = getelementptr i8, ptr %9, i16 1
  store i8 1, ptr %10
  %11 = getelementptr i8, ptr %9, i16 2
  store i16 32, ptr %11
  %12 = getelementptr inbounds i8, ptr %5, i16 8
  store i8 0, ptr %12
  %13 = getelementptr i8, ptr %12, i16 1
  store i8 1, ptr %13
  %14 = getelementptr i8, ptr %12, i16 2
  store i16 16, ptr %14
  store ptr %5, ptr %3, !tbaa !2
  %15 = getelementptr i8, ptr @$str1, i16 6
  %16 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %15, i16 3, i16 9)
  %17 = getelementptr inbounds i8, ptr %16, i16 0
  store i16 0, ptr %17
  %18 = getelementptr i8, ptr %17, i16 2
  store i16 1, ptr %18
  %19 = getelementptr i8, ptr %17, i16 4
  store i16 2, ptr %19
  %20 = getelementptr i8, ptr %17, i16 6
  store i16 0, ptr %20
  %21 = getelementptr i8, ptr %17, i16 8
  store i8 1, ptr %21
  %22 = getelementptr inbounds i8, ptr %16, i16 9
  store i16 1, ptr %22
  %23 = getelementptr i8, ptr %22, i16 2
  store i16 -1, ptr %23
  %24 = getelementptr i8, ptr %22, i16 4
  store i16 -2, ptr %24
  %25 = getelementptr i8, ptr %22, i16 6
  store i16 1, ptr %25
  %26 = getelementptr i8, ptr %22, i16 8
  store i8 2, ptr %26
  %27 = getelementptr inbounds i8, ptr %16, i16 18
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
  %33 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %32, i16 5, i16 3)
  %34 = getelementptr inbounds i8, ptr %33, i16 0
  store i16 0, ptr %34
  %35 = getelementptr i8, ptr %34, i16 2
  store i8 2, ptr %35
  %36 = getelementptr inbounds i8, ptr %33, i16 3
  store i16 1, ptr %36
  %37 = getelementptr i8, ptr %36, i16 2
  store i8 4, ptr %37
  %38 = getelementptr inbounds i8, ptr %33, i16 6
  store i16 1, ptr %38
  %39 = getelementptr i8, ptr %38, i16 2
  store i8 5, ptr %39
  %40 = getelementptr inbounds i8, ptr %33, i16 9
  store i16 2, ptr %40
  %41 = getelementptr i8, ptr %40, i16 2
  store i8 6, ptr %41
  %42 = getelementptr inbounds i8, ptr %33, i16 12
  store i16 2, ptr %42
  %43 = getelementptr i8, ptr %42, i16 2
  store i8 9, ptr %43
  store ptr %33, ptr %1, !tbaa !2
  %44 = load ptr, ptr %3, !tbaa !2
  store ptr null, ptr %3, !tbaa !2
  %45 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  %46 = load ptr, ptr %1, !tbaa !2
  store ptr null, ptr %1, !tbaa !2
  store ptr %44, ptr addrspace(1) %0
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %45, ptr addrspace(1) %47
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr %46, ptr addrspace(1) %48
  %49 = load ptr, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %49)
  %50 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %50)
  %51 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %51)
  ret void
}

define internal cc 1005 void @run(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %20 = addrspacecast ptr %19 to ptr addrspace(1)
  %21 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @built(ptr addrspace(1) %21)
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
  call cc 1005 addrspace(1) void @Level.save(ptr addrspace(1) %20, ptr addrspace(1) %22, ptr addrspace(1) %29)
  %30 = load i8, ptr %19, !tbaa !2, !range !20
  %31 = icmp eq i8 %30, 1
  br i1 %31, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %32 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %33 = getelementptr inbounds i8, ptr %19, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %32, ptr %33, i16 6, i1 false)
  %34 = getelementptr inbounds i8, ptr %18, i16 4
  %35 = load ptr, ptr %34, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %35)
  %36 = getelementptr inbounds i8, ptr %18, i16 2
  %37 = load ptr, ptr %36, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %37)
  %38 = load ptr, ptr %18, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %38)
  ret void

b3:
  %39 = getelementptr inbounds i8, ptr %18, i16 4
  %40 = load ptr, ptr %39, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %40)
  %41 = getelementptr inbounds i8, ptr %18, i16 2
  %42 = load ptr, ptr %41, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %42)
  %43 = load ptr, ptr %18, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %43)
  %44 = addrspacecast ptr %15 to ptr addrspace(1)
  %45 = getelementptr i8, ptr @$str2, i16 6
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %14, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 %47, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %48, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %14 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Level.load(ptr addrspace(1) %44, ptr addrspace(1) %51)
  %52 = load i8, ptr %15, !tbaa !2, !range !20
  %53 = icmp eq i8 %52, 1
  br i1 %53, label %b4, label %b5

b4:
  store i8 1, ptr addrspace(1) %0
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %55 = getelementptr inbounds i8, ptr %15, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %54, ptr %55, i16 6, i1 false)
  ret void

b5:
  %56 = getelementptr inbounds i8, ptr %15, i16 2
  %57 = load ptr, ptr %56, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %15, i16 4
  %59 = load ptr, ptr %58, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %15, i16 6
  %61 = load ptr, ptr %60, !tbaa !2
  store ptr %57, ptr %16, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %16, i16 2
  store ptr %59, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %16, i16 4
  store ptr %61, ptr %63, !tbaa !2
  %64 = load ptr, ptr %16, !tbaa !2
  %65 = getelementptr i8, ptr %64, i16 -4
  %66 = load i16, ptr %65
  store i16 %66, ptr %13, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %16, i16 2
  %68 = load ptr, ptr %67, !tbaa !2
  %69 = getelementptr i8, ptr %68, i16 -4
  %70 = load i16, ptr %69
  store i16 %70, ptr %12, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %16, i16 4
  %72 = load ptr, ptr %71, !tbaa !2
  %73 = getelementptr i8, ptr %72, i16 -4
  %74 = load i16, ptr %73
  store i16 %74, ptr %11, !tbaa !2
  %75 = load ptr, ptr %16, !tbaa !2
  %76 = getelementptr i8, ptr %75, i16 -4
  %77 = load i16, ptr %76
  %78 = mul i16 %77, 4
  %79 = add i16 8, %78
  %80 = getelementptr inbounds i8, ptr %16, i16 2
  %81 = load ptr, ptr %80, !tbaa !2
  %82 = getelementptr i8, ptr %81, i16 -4
  %83 = load i16, ptr %82
  %84 = mul i16 %83, 9
  %85 = add i16 %79, %84
  %86 = getelementptr inbounds i8, ptr %16, i16 4
  %87 = load ptr, ptr %86, !tbaa !2
  %88 = getelementptr i8, ptr %87, i16 -4
  %89 = load i16, ptr %88
  %90 = mul i16 %89, 3
  %91 = add i16 %85, %90
  store i16 %91, ptr %10, !tbaa !2
  %92 = load i16, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %92)
  %93 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %93)
  %94 = load i16, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %94)
  %95 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %95)
  %96 = load i16, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %96)
  %97 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %97)
  %98 = load i16, ptr %10, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %98)
  %99 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %99)
  call cc 1005 addrspace(1) void @N$PN()
  %100 = getelementptr i8, ptr @$str1, i16 6
  %101 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %100, i16 4, i16 4)
  %102 = getelementptr inbounds i8, ptr %101, i16 0
  store i16 40, ptr %102
  %103 = getelementptr i8, ptr %102, i16 2
  store i16 50, ptr %103
  %104 = getelementptr inbounds i8, ptr %101, i16 4
  store i16 40, ptr %104
  %105 = getelementptr i8, ptr %104, i16 2
  store i16 10, ptr %105
  %106 = getelementptr inbounds i8, ptr %101, i16 8
  store i16 8, ptr %106
  %107 = getelementptr i8, ptr %106, i16 2
  store i16 20, ptr %107
  %108 = getelementptr inbounds i8, ptr %101, i16 12
  store i16 8, ptr %108
  %109 = getelementptr i8, ptr %108, i16 2
  store i16 4, ptr %109
  store ptr %101, ptr %9, !tbaa !2
  %110 = load ptr, ptr %9, !tbaa !2
  %111 = getelementptr i8, ptr %110, i16 -4
  %112 = load i16, ptr %111
  store i16 0, ptr %8, !tbaa !2
  br label %b6

b6:
  %113 = load i16, ptr %8, !tbaa !2
  %114 = icmp ult i16 %113, %112
  br i1 %114, label %b7, label %b9

b7:
  %115 = mul i16 %113, 4
  %116 = getelementptr inbounds i8, ptr %110, i16 %115
  %117 = load i16, ptr %116
  %118 = getelementptr i8, ptr %116, i16 2
  %119 = load i16, ptr %118
  %120 = getelementptr i8, ptr %116, i16 2
  %121 = addrspacecast ptr %120 to ptr addrspace(1)
  %122 = addrspacecast ptr %16 to ptr addrspace(1)
  %123 = load i16, ptr %116
  %124 = load i16, ptr addrspace(1) %121
  %125 = call cc 1005 addrspace(1) i32 @Level.leaf(ptr addrspace(1) %122, i16 %123, i16 %124)
  %126 = addrspacecast ptr %7 to ptr addrspace(1)
  store i32 %125, ptr addrspace(1) %126, !tbaa !2
  %127 = load i16, ptr %7, !tbaa !2
  %128 = getelementptr inbounds i8, ptr %7, i16 2
  %129 = load i16, ptr %128, !tbaa !2
  store i16 %127, ptr %6, !tbaa !2
  store i16 %129, ptr %5, !tbaa !2
  %130 = getelementptr inbounds i8, ptr %16, i16 2
  %131 = load ptr, ptr %130, !tbaa !2
  %132 = load i16, ptr %5, !tbaa !2
  %133 = getelementptr i8, ptr %131, i16 -4
  %134 = load i16, ptr %133
  %135 = icmp ult i16 %132, %134
  br i1 %135, label %b10, label %b11

b8:
  %136 = load i16, ptr %8, !tbaa !2
  %137 = add i16 %136, 1
  store i16 %137, ptr %8, !tbaa !2
  br label %b6

b9:
  store i8 0, ptr addrspace(1) %0
  %138 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %138)
  %139 = getelementptr inbounds i8, ptr %16, i16 4
  %140 = load ptr, ptr %139, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %140)
  %141 = getelementptr inbounds i8, ptr %16, i16 2
  %142 = load ptr, ptr %141, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %142)
  %143 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %143)
  ret void

b10:
  %144 = mul i16 %132, 9
  %145 = getelementptr inbounds i8, ptr %131, i16 %144
  %146 = addrspacecast ptr %145 to ptr addrspace(1)
  %147 = getelementptr i8, ptr @$str1, i16 6
  store ptr %147, ptr %4, !tbaa !2
  %148 = getelementptr i8, ptr addrspace(1) %146, i16 6
  %149 = load i16, ptr addrspace(1) %148
  %150 = getelementptr i8, ptr addrspace(1) %146, i16 6
  %151 = load i16, ptr addrspace(1) %150
  %152 = getelementptr i8, ptr addrspace(1) %146, i16 8
  %153 = load i8, ptr addrspace(1) %152
  %154 = zext i8 %153 to i16
  %155 = add i16 %151, %154
  store i16 %149, ptr %3, !tbaa !2
  store i16 %155, ptr %2, !tbaa !2
  br label %b12

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %156 = load i16, ptr %3, !tbaa !2
  %157 = load i16, ptr %2, !tbaa !2
  %158 = icmp ult i16 %156, %157
  br i1 %158, label %b13, label %b15

b13:
  %159 = getelementptr inbounds i8, ptr %16, i16 4
  %160 = load ptr, ptr %159, !tbaa !2
  %161 = load i16, ptr %3, !tbaa !2
  %162 = getelementptr i8, ptr %160, i16 -4
  %163 = load i16, ptr %162
  %164 = icmp ult i16 %161, %163
  br i1 %164, label %b16, label %b17

b14:
  %165 = load i16, ptr %3, !tbaa !2
  %166 = add nuw i16 %165, 1
  store i16 %166, ptr %3, !tbaa !2
  br label %b12

b15:
  %167 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %167)
  %168 = load i16, ptr %116
  call cc 1005 addrspace(1) void @N$PI2(i16 %168)
  %169 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %169)
  %170 = load i16, ptr addrspace(1) %121
  call cc 1005 addrspace(1) void @N$PI2(i16 %170)
  %171 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %171)
  %172 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %172)
  %173 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %173)
  %174 = load i16, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %174)
  %175 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %175)
  %176 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %176)
  call cc 1005 addrspace(1) void @N$PN()
  %177 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %177)
  br label %b8

b16:
  %178 = mul i16 %161, 3
  %179 = getelementptr inbounds i8, ptr %160, i16 %178
  %180 = addrspacecast ptr %179 to ptr addrspace(1)
  %181 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$PBEG()
  %182 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %182)
  %183 = getelementptr i8, ptr addrspace(1) %180, i16 2
  %184 = load i8, ptr addrspace(1) %183
  %185 = and i8 %184, 1
  %186 = icmp ne i8 %185, 0
  br i1 %186, label %b18, label %b19

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b18:
  store i8 98, ptr %1, !tbaa !2
  br label %b20

b19:
  store i8 102, ptr %1, !tbaa !2
  br label %b20

b20:
  %187 = load i8, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @N$PC(i8 zeroext %187)
  %188 = getelementptr i8, ptr addrspace(1) %180, i16 2
  %189 = load i8, ptr addrspace(1) %188
  %190 = lshr i8 %189, 1
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %190)
  %191 = call cc 1005 addrspace(1) ptr @N$PEND()
  %192 = call cc 1005 addrspace(1) ptr @N$TCAT(ptr %181, ptr %191)
  %193 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %193)
  store ptr %192, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %191)
  br label %b14
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @run(ptr addrspace(1) %2)
  %3 = load i8, ptr %1, !tbaa !2, !range !21
  %4 = icmp eq i8 %3, 0
  br i1 %4, label %b4, label %b3

b2:
  ret i16 1

b3:
  %5 = load i8, ptr %1, !tbaa !2, !range !21
  %6 = icmp eq i8 %5, 1
  br i1 %6, label %b6, label %b5

b4:
  ret i16 0

b5:
  %7 = load i8, ptr %1, !tbaa !2, !range !21
  %8 = icmp eq i8 %7, 1
  br i1 %8, label %b9, label %b8

b6:
  %9 = getelementptr inbounds i8, ptr %1, i16 2
  %10 = load i8, ptr %9, !tbaa !2, !range !22
  %11 = icmp eq i8 %10, 0
  br i1 %11, label %b7, label %b5

b7:
  %12 = getelementptr i8, ptr @$str13, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %12)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b8:
  %13 = getelementptr inbounds i8, ptr %1, i16 4
  %14 = load i16, ptr %13, !tbaa !2
  store i16 %14, ptr %0, !tbaa !2
  %15 = getelementptr i8, ptr @$str15, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %15)
  %16 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %16)
  %17 = getelementptr i8, ptr @$str16, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %17)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b9:
  %18 = getelementptr inbounds i8, ptr %1, i16 2
  %19 = load i8, ptr %18, !tbaa !2, !range !22
  %20 = icmp eq i8 %19, 1
  br i1 %20, label %b10, label %b8

b10:
  %21 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %21)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

define internal cc 1005 void @"records[Face]"(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, i16 %2, ptr addrspace(1) noalias %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = alloca [8 x i8]
  %5 = alloca ptr addrspace(1)
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca [3 x i8]
  %9 = alloca ptr
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
  br i1 %17, label %b3, label %b5

b3:
  %18 = addrspacecast ptr %8 to ptr addrspace(1)
  store ptr addrspace(1) %18, ptr %5, !tbaa !2
  %19 = addrspacecast ptr %4 to ptr addrspace(1)
  %20 = load ptr addrspace(1), ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @read_exact(ptr addrspace(1) %19, ptr addrspace(1) %1, ptr addrspace(1) %20, i16 3)
  %21 = load i8, ptr %4, !tbaa !2, !range !23
  %22 = icmp eq i8 %21, 1
  br i1 %22, label %b6, label %b7

b4:
  %23 = load i16, ptr %7, !tbaa !2
  %24 = add nuw i16 %23, 1
  store i16 %24, ptr %7, !tbaa !2
  br label %b2

b5:
  %25 = load ptr, ptr %9, !tbaa !2
  store ptr null, ptr %9, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %25, ptr addrspace(1) %26
  %27 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %27)
  ret void

b6:
  store i8 1, ptr addrspace(1) %0
  %28 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %29 = getelementptr inbounds i8, ptr %4, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %28, ptr %29, i16 6, i1 false)
  %30 = load ptr, ptr %9, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %30)
  ret void

b7:
  %31 = load ptr, ptr %9, !tbaa !2
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  %34 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %31, i16 1, i16 3)
  store ptr %34, ptr %9, !tbaa !2
  %35 = mul i16 %33, 3
  %36 = getelementptr inbounds i8, ptr %34, i16 %35
  %37 = load i16, ptr %8, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %8, i16 2
  %39 = load i8, ptr %38, !tbaa !2
  store i16 %37, ptr %36
  %40 = getelementptr i8, ptr %36, i16 2
  store i8 %39, ptr %40
  br label %b4
}

define internal cc 1005 void @"records[Node]"(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, i16 %2, ptr addrspace(1) noalias %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  br i1 %25, label %b3, label %b5

b3:
  %26 = load i16, ptr %11, !tbaa !2
  %27 = mul i16 %26, 2
  %28 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 %27
  %29 = load i16, ptr %11, !tbaa !2
  %30 = mul i16 %29, 2
  %31 = getelementptr inbounds i8, ptr addrspace(1) %22, i16 %30
  %32 = load i16, ptr addrspace(1) %31
  store i16 %32, ptr addrspace(1) %28
  br label %b4

b4:
  %33 = load i16, ptr %11, !tbaa !2
  %34 = add nsw i16 %33, 1
  store i16 %34, ptr %11, !tbaa !2
  br label %b2

b5:
  %35 = getelementptr inbounds i8, ptr %12, i16 6
  store i16 %17, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %12, i16 8
  store i8 %19, ptr %36, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i16 %2, ptr %8, !tbaa !2
  br label %b6

b6:
  %37 = load i16, ptr %9, !tbaa !2
  %38 = load i16, ptr %8, !tbaa !2
  %39 = icmp ult i16 %37, %38
  br i1 %39, label %b7, label %b9

b7:
  %40 = addrspacecast ptr %12 to ptr addrspace(1)
  store ptr addrspace(1) %40, ptr %7, !tbaa !2
  %41 = addrspacecast ptr %6 to ptr addrspace(1)
  %42 = load ptr addrspace(1), ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @read_exact(ptr addrspace(1) %41, ptr addrspace(1) %1, ptr addrspace(1) %42, i16 9)
  %43 = load i8, ptr %6, !tbaa !2, !range !24
  %44 = icmp eq i8 %43, 1
  br i1 %44, label %b10, label %b11

b8:
  %45 = load i16, ptr %9, !tbaa !2
  %46 = add nuw i16 %45, 1
  store i16 %46, ptr %9, !tbaa !2
  br label %b6

b9:
  %47 = load ptr, ptr %13, !tbaa !2
  store ptr null, ptr %13, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %48 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %47, ptr addrspace(1) %48
  %49 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %49)
  ret void

b10:
  store i8 1, ptr addrspace(1) %0
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %51 = getelementptr inbounds i8, ptr %6, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %50, ptr %51, i16 6, i1 false)
  %52 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %52)
  ret void

b11:
  %53 = load ptr, ptr %13, !tbaa !2
  %54 = getelementptr i8, ptr %53, i16 -4
  %55 = load i16, ptr %54
  %56 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %53, i16 1, i16 9)
  store ptr %56, ptr %13, !tbaa !2
  %57 = mul i16 %55, 9
  %58 = getelementptr inbounds i8, ptr %56, i16 %57
  %59 = load i16, ptr %12, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %12, i16 6
  %61 = load i16, ptr %60, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %12, i16 8
  %63 = load i8, ptr %62, !tbaa !2
  store i16 %59, ptr %58
  %64 = getelementptr i8, ptr %58, i16 2
  %65 = addrspacecast ptr %64 to ptr addrspace(1)
  %66 = getelementptr inbounds i8, ptr %12, i16 2
  %67 = addrspacecast ptr %66 to ptr addrspace(1)
  store i16 0, ptr %5, !tbaa !2
  store i16 2, ptr %4, !tbaa !2
  br label %b12

b12:
  %68 = load i16, ptr %5, !tbaa !2
  %69 = load i16, ptr %4, !tbaa !2
  %70 = icmp slt i16 %68, %69
  br i1 %70, label %b13, label %b15

b13:
  %71 = load i16, ptr %5, !tbaa !2
  %72 = mul i16 %71, 2
  %73 = getelementptr inbounds i8, ptr addrspace(1) %65, i16 %72
  %74 = load i16, ptr %5, !tbaa !2
  %75 = mul i16 %74, 2
  %76 = getelementptr inbounds i8, ptr addrspace(1) %67, i16 %75
  %77 = load i16, ptr addrspace(1) %76
  store i16 %77, ptr addrspace(1) %73
  br label %b14

b14:
  %78 = load i16, ptr %5, !tbaa !2
  %79 = add nsw i16 %78, 1
  store i16 %79, ptr %5, !tbaa !2
  br label %b12

b15:
  %80 = getelementptr i8, ptr %58, i16 6
  store i16 %61, ptr %80
  %81 = getelementptr i8, ptr %58, i16 8
  store i8 %63, ptr %81
  br label %b8
}

define internal cc 1005 void @"records[Plane]"(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, i16 %2, ptr addrspace(1) noalias %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  br i1 %20, label %b3, label %b5

b3:
  %21 = load i16, ptr %11, !tbaa !2
  %22 = getelementptr inbounds i8, ptr addrspace(1) %17, i16 %21
  %23 = load i16, ptr %11, !tbaa !2
  %24 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 %23
  %25 = load i8, ptr addrspace(1) %24
  store i8 %25, ptr addrspace(1) %22
  br label %b4

b4:
  %26 = load i16, ptr %11, !tbaa !2
  %27 = add nsw i16 %26, 1
  store i16 %27, ptr %11, !tbaa !2
  br label %b2

b5:
  %28 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 %16, ptr %28, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i16 %2, ptr %8, !tbaa !2
  br label %b6

b6:
  %29 = load i16, ptr %9, !tbaa !2
  %30 = load i16, ptr %8, !tbaa !2
  %31 = icmp ult i16 %29, %30
  br i1 %31, label %b7, label %b9

b7:
  %32 = addrspacecast ptr %12 to ptr addrspace(1)
  store ptr addrspace(1) %32, ptr %7, !tbaa !2
  %33 = addrspacecast ptr %6 to ptr addrspace(1)
  %34 = load ptr addrspace(1), ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @read_exact(ptr addrspace(1) %33, ptr addrspace(1) %1, ptr addrspace(1) %34, i16 4)
  %35 = load i8, ptr %6, !tbaa !2, !range !25
  %36 = icmp eq i8 %35, 1
  br i1 %36, label %b10, label %b11

b8:
  %37 = load i16, ptr %9, !tbaa !2
  %38 = add nuw i16 %37, 1
  store i16 %38, ptr %9, !tbaa !2
  br label %b6

b9:
  %39 = load ptr, ptr %13, !tbaa !2
  store ptr null, ptr %13, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %39, ptr addrspace(1) %40
  %41 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %41)
  ret void

b10:
  store i8 1, ptr addrspace(1) %0
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %43 = getelementptr inbounds i8, ptr %6, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %42, ptr %43, i16 6, i1 false)
  %44 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %44)
  ret void

b11:
  %45 = load ptr, ptr %13, !tbaa !2
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %45, i16 1, i16 4)
  store ptr %48, ptr %13, !tbaa !2
  %49 = mul i16 %47, 4
  %50 = getelementptr inbounds i8, ptr %48, i16 %49
  %51 = getelementptr inbounds i8, ptr %12, i16 2
  %52 = load i16, ptr %51, !tbaa !2
  %53 = addrspacecast ptr %50 to ptr addrspace(1)
  %54 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 0, ptr %5, !tbaa !2
  store i16 2, ptr %4, !tbaa !2
  br label %b12

b12:
  %55 = load i16, ptr %5, !tbaa !2
  %56 = load i16, ptr %4, !tbaa !2
  %57 = icmp slt i16 %55, %56
  br i1 %57, label %b13, label %b15

b13:
  %58 = load i16, ptr %5, !tbaa !2
  %59 = getelementptr inbounds i8, ptr addrspace(1) %53, i16 %58
  %60 = load i16, ptr %5, !tbaa !2
  %61 = getelementptr inbounds i8, ptr addrspace(1) %54, i16 %60
  %62 = load i8, ptr addrspace(1) %61
  store i8 %62, ptr addrspace(1) %59
  br label %b14

b14:
  %63 = load i16, ptr %5, !tbaa !2
  %64 = add nsw i16 %63, 1
  store i16 %64, ptr %5, !tbaa !2
  br label %b12

b15:
  %65 = getelementptr i8, ptr %50, i16 2
  store i16 %52, ptr %65
  br label %b8
}

define internal cc 1005 void @"written[Face]"(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, ptr addrspace(1) %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  %5 = load ptr, ptr addrspace(1) %2
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load ptr, ptr addrspace(1) %2
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = mul i16 %11, 3
  call cc 1005 addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %12)
  %13 = load i8, ptr %3, !tbaa !2, !range !26
  %14 = icmp eq i8 %13, 1
  br i1 %14, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %17 = getelementptr inbounds i8, ptr %3, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %16, ptr %17, i16 4, i1 false)
  ret void

b3:
  %18 = getelementptr inbounds i8, ptr %3, i16 2
  %19 = load i16, ptr %18, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 void @"written[Node]"(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, ptr addrspace(1) %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  %5 = load ptr, ptr addrspace(1) %2
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load ptr, ptr addrspace(1) %2
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = mul i16 %11, 9
  call cc 1005 addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %12)
  %13 = load i8, ptr %3, !tbaa !2, !range !27
  %14 = icmp eq i8 %13, 1
  br i1 %14, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %17 = getelementptr inbounds i8, ptr %3, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %16, ptr %17, i16 4, i1 false)
  ret void

b3:
  %18 = getelementptr inbounds i8, ptr %3, i16 2
  %19 = load i16, ptr %18, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  ret void
}

define internal cc 1005 void @"written[Plane]"(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(134) noalias nocapture %1, ptr addrspace(1) %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [6 x i8]
  %4 = alloca ptr addrspace(1)
  %5 = load ptr, ptr addrspace(1) %2
  %6 = addrspacecast ptr %5 to ptr addrspace(1)
  store ptr addrspace(1) %6, ptr %4, !tbaa !2
  %7 = addrspacecast ptr %3 to ptr addrspace(1)
  %8 = load ptr addrspace(1), ptr %4, !tbaa !2
  %9 = load ptr, ptr addrspace(1) %2
  %10 = getelementptr i8, ptr %9, i16 -4
  %11 = load i16, ptr %10
  %12 = mul i16 %11, 4
  call cc 1005 addrspace(1) void @std.io.File.write_raw(ptr addrspace(1) %7, ptr addrspace(1) %1, ptr addrspace(1) %8, i16 %12)
  %13 = load i8, ptr %3, !tbaa !2, !range !28
  %14 = icmp eq i8 %13, 1
  br i1 %14, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i8 0, ptr addrspace(1) %15
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %17 = getelementptr inbounds i8, ptr %3, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %16, ptr %17, i16 4, i1 false)
  ret void

b3:
  %18 = getelementptr inbounds i8, ptr %3, i16 2
  %19 = load i16, ptr %18, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  ret void
}

declare i32 @LL$WRITE_FILE(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare i16 @LL$OPEN(ptr addrspace(1), i8) addrspace(1)

declare i16 @LL$CREATE(ptr addrspace(1)) addrspace(1)

declare i32 @LL$READ(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare i16 @LL$CLOSE(i16) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PC(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 ptr @N$TCAT(ptr, ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
!11 = !{i8 0, i8 2}
!12 = !{i8 0, i8 2}
!13 = !{i8 0, i8 2}
!14 = !{i8 0, i8 2}
!15 = !{i8 0, i8 2}
!16 = !{i8 0, i8 2}
!17 = !{i8 0, i8 2}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
!20 = !{i8 0, i8 2}
!21 = !{i8 0, i8 2}
!22 = !{i8 0, i8 3}
!23 = !{i8 0, i8 2}
!24 = !{i8 0, i8 2}
!25 = !{i8 0, i8 2}
!26 = !{i8 0, i8 2}
!27 = !{i8 0, i8 2}
!28 = !{i8 0, i8 2}
