target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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

define internal cc 1005 void @Log.create(ptr addrspace(1) %0, ptr %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  call cc 1005 addrspace(1) void @std.io.File.create(ptr addrspace(1) %13, ptr addrspace(1) %20)
  %21 = load i8, ptr %10, !tbaa !2, !range !12
  %22 = icmp eq i8 %21, 1
  br i1 %22, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %24 = getelementptr inbounds i8, ptr %10, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %23, ptr %24, i16 4, i1 false)
  %25 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %25)
  ret void

b3:
  %26 = getelementptr inbounds i8, ptr %10, i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %10, i16 132
  %29 = load i16, ptr %28, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %10, i16 134
  %31 = load i16, ptr %30, !tbaa !2
  store i16 %27, ptr %11, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %11, i16 2
  %33 = addrspacecast ptr %32 to ptr addrspace(1)
  %34 = getelementptr inbounds i8, ptr %10, i16 4
  %35 = addrspacecast ptr %34 to ptr addrspace(1)
  store i16 0, ptr %8, !tbaa !2
  store i16 128, ptr %7, !tbaa !2
  br label %b4

b4:
  %36 = load i16, ptr %8, !tbaa !2
  %37 = load i16, ptr %7, !tbaa !2
  %38 = icmp slt i16 %36, %37
  br i1 %38, label %b5, label %b7

b5:
  %39 = load i16, ptr %8, !tbaa !2
  %40 = getelementptr inbounds i8, ptr addrspace(1) %33, i16 %39
  %41 = load i16, ptr %8, !tbaa !2
  %42 = getelementptr inbounds i8, ptr addrspace(1) %35, i16 %41
  %43 = load i8, ptr addrspace(1) %42
  store i8 %43, ptr addrspace(1) %40
  br label %b6

b6:
  %44 = load i16, ptr %8, !tbaa !2
  %45 = add nsw i16 %44, 1
  store i16 %45, ptr %8, !tbaa !2
  br label %b4

b7:
  %46 = getelementptr inbounds i8, ptr %11, i16 130
  store i16 %29, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %11, i16 132
  store i16 %31, ptr %47, !tbaa !2
  store i8 1, ptr %6, !tbaa !2
  %48 = load ptr, ptr %12, !tbaa !2
  store ptr null, ptr %12, !tbaa !2
  %49 = load i16, ptr %11, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %11, i16 130
  %51 = load i16, ptr %50, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %11, i16 132
  %53 = load i16, ptr %52, !tbaa !2
  store i8 0, ptr %6, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %48, ptr addrspace(1) %54
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %49, ptr addrspace(1) %55
  %56 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %57 = getelementptr inbounds i8, ptr %11, i16 2
  %58 = addrspacecast ptr %57 to ptr addrspace(1)
  store i16 0, ptr %5, !tbaa !2
  store i16 128, ptr %4, !tbaa !2
  br label %b8

b8:
  %59 = load i16, ptr %5, !tbaa !2
  %60 = load i16, ptr %4, !tbaa !2
  %61 = icmp slt i16 %59, %60
  br i1 %61, label %b9, label %b11

b9:
  %62 = load i16, ptr %5, !tbaa !2
  %63 = getelementptr inbounds i8, ptr addrspace(1) %56, i16 %62
  %64 = load i16, ptr %5, !tbaa !2
  %65 = getelementptr inbounds i8, ptr addrspace(1) %58, i16 %64
  %66 = load i8, ptr addrspace(1) %65
  store i8 %66, ptr addrspace(1) %63
  br label %b10

b10:
  %67 = load i16, ptr %5, !tbaa !2
  %68 = add nsw i16 %67, 1
  store i16 %68, ptr %5, !tbaa !2
  br label %b8

b11:
  %69 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %51, ptr addrspace(1) %69
  %70 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %53, ptr addrspace(1) %70
  store i16 0, ptr %11, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %11, i16 2
  %72 = addrspacecast ptr %71 to ptr addrspace(1)
  store i16 0, ptr %3, !tbaa !2
  store i16 128, ptr %2, !tbaa !2
  br label %b12

b12:
  %73 = load i16, ptr %3, !tbaa !2
  %74 = load i16, ptr %2, !tbaa !2
  %75 = icmp slt i16 %73, %74
  br i1 %75, label %b13, label %b15

b13:
  %76 = load i16, ptr %3, !tbaa !2
  %77 = getelementptr inbounds i8, ptr addrspace(1) %72, i16 %76
  store i8 0, ptr addrspace(1) %77
  br label %b14

b14:
  %78 = load i16, ptr %3, !tbaa !2
  %79 = add nsw i16 %78, 1
  store i16 %79, ptr %3, !tbaa !2
  br label %b12

b15:
  %80 = getelementptr inbounds i8, ptr %11, i16 130
  store i16 0, ptr %80, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %11, i16 132
  store i16 0, ptr %81, !tbaa !2
  %82 = getelementptr i8, ptr addrspace(1) %0, i16 138
  store i16 0, ptr addrspace(1) %82
  %83 = load i8, ptr %6, !tbaa !2, !range !6
  %84 = icmp ne i8 %83, 0
  br i1 %84, label %b17, label %b16

b16:
  %85 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %85)
  ret void

b17:
  %86 = addrspacecast ptr %11 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %86)
  br label %b16
}

define internal cc 1005 void @Log.write(ptr addrspace(1) nonnull dereferenceable(138) noalias nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca [6 x i8]
  %4 = alloca [6 x i8]
  %5 = addrspacecast ptr %4 to ptr addrspace(1)
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  call cc 1005 addrspace(1) void @std.io.File.write(ptr addrspace(1) %5, ptr addrspace(1) %6, ptr addrspace(1) %1)
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
  call cc 1005 addrspace(1) void @std.io.File.write(ptr addrspace(1) %7, ptr addrspace(1) %8, ptr addrspace(1) %15)
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 136
  %17 = load i16, ptr addrspace(1) %16
  %18 = add i16 %17, 1
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %18, ptr addrspace(1) %19
  ret void
}

define internal cc 1005 void @Log.drop(ptr addrspace(1) nonnull dereferenceable(138) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %1)
  %2 = load ptr, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @N$PS(ptr %2)
  %3 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %3)
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 136
  %5 = load i16, ptr addrspace(1) %4
  call cc 1005 addrspace(1) void @N$PU2(i16 %5)
  %6 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %6)
  call cc 1005 addrspace(1) void @N$PN()
  ret void
}

define internal cc 1005 void @keep_open(ptr addrspace(1) %0, ptr addrspace(1) noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i8
  store i8 1, ptr %6, !tbaa !2
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
  br i1 %21, label %b3, label %b5

b3:
  %22 = load i16, ptr %5, !tbaa !2
  %23 = getelementptr inbounds i8, ptr addrspace(1) %17, i16 %22
  %24 = load i16, ptr %5, !tbaa !2
  %25 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 %24
  %26 = load i8, ptr addrspace(1) %25
  store i8 %26, ptr addrspace(1) %23
  br label %b4

b4:
  %27 = load i16, ptr %5, !tbaa !2
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %5, !tbaa !2
  br label %b2

b5:
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 132
  store i16 %11, ptr addrspace(1) %29
  %30 = getelementptr i8, ptr addrspace(1) %0, i16 134
  store i16 %13, ptr addrspace(1) %30
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 136
  store i16 %15, ptr addrspace(1) %31
  store ptr null, ptr addrspace(1) %1
  %32 = getelementptr i8, ptr addrspace(1) %1, i16 2
  store i16 0, ptr addrspace(1) %32
  %33 = getelementptr i8, ptr addrspace(1) %1, i16 4
  store i16 0, ptr %3, !tbaa !2
  store i16 128, ptr %2, !tbaa !2
  br label %b6

b6:
  %34 = load i16, ptr %3, !tbaa !2
  %35 = load i16, ptr %2, !tbaa !2
  %36 = icmp slt i16 %34, %35
  br i1 %36, label %b7, label %b9

b7:
  %37 = load i16, ptr %3, !tbaa !2
  %38 = getelementptr inbounds i8, ptr addrspace(1) %33, i16 %37
  store i8 0, ptr addrspace(1) %38
  br label %b8

b8:
  %39 = load i16, ptr %3, !tbaa !2
  %40 = add nsw i16 %39, 1
  store i16 %40, ptr %3, !tbaa !2
  br label %b6

b9:
  %41 = getelementptr i8, ptr addrspace(1) %1, i16 132
  store i16 0, ptr addrspace(1) %41
  %42 = getelementptr i8, ptr addrspace(1) %1, i16 134
  store i16 0, ptr addrspace(1) %42
  %43 = getelementptr i8, ptr addrspace(1) %1, i16 136
  store i16 0, ptr addrspace(1) %43
  %44 = load i8, ptr %6, !tbaa !2, !range !7
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %b11, label %b10

b10:
  ret void

b11:
  call cc 1005 addrspace(1) void @Log.drop(ptr addrspace(1) %1)
  %46 = getelementptr i8, ptr addrspace(1) %1, i16 2
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %46)
  %47 = load ptr, ptr addrspace(1) %1
  call cc 1005 addrspace(1) void @N$BDRP(ptr %47)
  br label %b10
}

define internal cc 1005 void @$main(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %24 = addrspacecast ptr %22 to ptr addrspace(1)
  %25 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @Log.create(ptr addrspace(1) %24, ptr %25)
  %26 = load i8, ptr %22, !tbaa !2, !range !13
  %27 = icmp eq i8 %26, 1
  br i1 %27, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %28 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %29 = getelementptr inbounds i8, ptr %22, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %28, ptr %29, i16 4, i1 false)
  ret void

b3:
  %30 = getelementptr inbounds i8, ptr %22, i16 2
  %31 = load ptr, ptr %30, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %22, i16 4
  %33 = load i16, ptr %32, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %22, i16 134
  %35 = load i16, ptr %34, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %22, i16 136
  %37 = load i16, ptr %36, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %22, i16 138
  %39 = load i16, ptr %38, !tbaa !2
  store ptr %31, ptr %23, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 %33, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %23, i16 4
  %42 = addrspacecast ptr %41 to ptr addrspace(1)
  %43 = getelementptr inbounds i8, ptr %22, i16 6
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  store i16 0, ptr %21, !tbaa !2
  store i16 128, ptr %20, !tbaa !2
  br label %b4

b4:
  %45 = load i16, ptr %21, !tbaa !2
  %46 = load i16, ptr %20, !tbaa !2
  %47 = icmp slt i16 %45, %46
  br i1 %47, label %b5, label %b7

b5:
  %48 = load i16, ptr %21, !tbaa !2
  %49 = getelementptr inbounds i8, ptr addrspace(1) %42, i16 %48
  %50 = load i16, ptr %21, !tbaa !2
  %51 = getelementptr inbounds i8, ptr addrspace(1) %44, i16 %50
  %52 = load i8, ptr addrspace(1) %51
  store i8 %52, ptr addrspace(1) %49
  br label %b6

b6:
  %53 = load i16, ptr %21, !tbaa !2
  %54 = add nsw i16 %53, 1
  store i16 %54, ptr %21, !tbaa !2
  br label %b4

b7:
  %55 = getelementptr inbounds i8, ptr %23, i16 132
  store i16 %35, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %23, i16 134
  store i16 %37, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %23, i16 136
  store i16 %39, ptr %57, !tbaa !2
  store i8 1, ptr %19, !tbaa !2
  %58 = addrspacecast ptr %23 to ptr addrspace(1)
  %59 = getelementptr i8, ptr @$str7, i16 6
  %60 = getelementptr i8, ptr %59, i16 -4
  %61 = load i16, ptr %60
  %62 = addrspacecast ptr %59 to ptr addrspace(1)
  store i16 %61, ptr %18, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %18, i16 2
  store i16 %61, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %18, i16 4
  store ptr addrspace(1) %62, ptr %64, !tbaa !2
  %65 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Log.write(ptr addrspace(1) %58, ptr addrspace(1) %65)
  %66 = addrspacecast ptr %16 to ptr addrspace(1)
  %67 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @Log.create(ptr addrspace(1) %66, ptr %67)
  %68 = load i8, ptr %16, !tbaa !2, !range !13
  %69 = icmp eq i8 %68, 1
  br i1 %69, label %b8, label %b9

b8:
  store i8 1, ptr addrspace(1) %0
  %70 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %71 = getelementptr inbounds i8, ptr %16, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %70, ptr %71, i16 4, i1 false)
  %72 = load i8, ptr %19, !tbaa !2, !range !8
  %73 = icmp ne i8 %72, 0
  br i1 %73, label %b11, label %b10

b9:
  %74 = getelementptr inbounds i8, ptr %16, i16 2
  %75 = load ptr, ptr %74, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %16, i16 4
  %77 = load i16, ptr %76, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %16, i16 134
  %79 = load i16, ptr %78, !tbaa !2
  %80 = getelementptr inbounds i8, ptr %16, i16 136
  %81 = load i16, ptr %80, !tbaa !2
  %82 = getelementptr inbounds i8, ptr %16, i16 138
  %83 = load i16, ptr %82, !tbaa !2
  store ptr %75, ptr %17, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 %77, ptr %84, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %17, i16 4
  %86 = addrspacecast ptr %85 to ptr addrspace(1)
  %87 = getelementptr inbounds i8, ptr %16, i16 6
  %88 = addrspacecast ptr %87 to ptr addrspace(1)
  store i16 0, ptr %15, !tbaa !2
  store i16 128, ptr %14, !tbaa !2
  br label %b12

b10:
  ret void

b11:
  %89 = addrspacecast ptr %23 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Log.drop(ptr addrspace(1) %89)
  %90 = getelementptr inbounds i8, ptr %23, i16 2
  %91 = addrspacecast ptr %90 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %91)
  %92 = load ptr, ptr %23, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %92)
  br label %b10

b12:
  %93 = load i16, ptr %15, !tbaa !2
  %94 = load i16, ptr %14, !tbaa !2
  %95 = icmp slt i16 %93, %94
  br i1 %95, label %b13, label %b15

b13:
  %96 = load i16, ptr %15, !tbaa !2
  %97 = getelementptr inbounds i8, ptr addrspace(1) %86, i16 %96
  %98 = load i16, ptr %15, !tbaa !2
  %99 = getelementptr inbounds i8, ptr addrspace(1) %88, i16 %98
  %100 = load i8, ptr addrspace(1) %99
  store i8 %100, ptr addrspace(1) %97
  br label %b14

b14:
  %101 = load i16, ptr %15, !tbaa !2
  %102 = add nsw i16 %101, 1
  store i16 %102, ptr %15, !tbaa !2
  br label %b12

b15:
  %103 = getelementptr inbounds i8, ptr %17, i16 132
  store i16 %79, ptr %103, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %17, i16 134
  store i16 %81, ptr %104, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %17, i16 136
  store i16 %83, ptr %105, !tbaa !2
  store i8 1, ptr %13, !tbaa !2
  %106 = addrspacecast ptr %17 to ptr addrspace(1)
  %107 = getelementptr i8, ptr @$str9, i16 6
  %108 = getelementptr i8, ptr %107, i16 -4
  %109 = load i16, ptr %108
  %110 = addrspacecast ptr %107 to ptr addrspace(1)
  store i16 %109, ptr %12, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 %109, ptr %111, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %110, ptr %112, !tbaa !2
  %113 = addrspacecast ptr %12 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Log.write(ptr addrspace(1) %106, ptr addrspace(1) %113)
  %114 = load i8, ptr %13, !tbaa !2, !range !9
  %115 = icmp ne i8 %114, 0
  br i1 %115, label %b17, label %b16

b16:
  %116 = addrspacecast ptr %10 to ptr addrspace(1)
  %117 = load ptr, ptr %23, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %23, i16 2
  %119 = load i16, ptr %118, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %23, i16 132
  %121 = load i16, ptr %120, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %23, i16 134
  %123 = load i16, ptr %122, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %23, i16 136
  %125 = load i16, ptr %124, !tbaa !2
  store i8 0, ptr %19, !tbaa !2
  store ptr %117, ptr %9, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 %119, ptr %126, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %9, i16 4
  %128 = addrspacecast ptr %127 to ptr addrspace(1)
  %129 = getelementptr inbounds i8, ptr %23, i16 4
  %130 = addrspacecast ptr %129 to ptr addrspace(1)
  store i16 0, ptr %8, !tbaa !2
  store i16 128, ptr %7, !tbaa !2
  br label %b18

b17:
  %131 = addrspacecast ptr %17 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Log.drop(ptr addrspace(1) %131)
  %132 = getelementptr inbounds i8, ptr %17, i16 2
  %133 = addrspacecast ptr %132 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %133)
  %134 = load ptr, ptr %17, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %134)
  br label %b16

b18:
  %135 = load i16, ptr %8, !tbaa !2
  %136 = load i16, ptr %7, !tbaa !2
  %137 = icmp slt i16 %135, %136
  br i1 %137, label %b19, label %b21

b19:
  %138 = load i16, ptr %8, !tbaa !2
  %139 = getelementptr inbounds i8, ptr addrspace(1) %128, i16 %138
  %140 = load i16, ptr %8, !tbaa !2
  %141 = getelementptr inbounds i8, ptr addrspace(1) %130, i16 %140
  %142 = load i8, ptr addrspace(1) %141
  store i8 %142, ptr addrspace(1) %139
  br label %b20

b20:
  %143 = load i16, ptr %8, !tbaa !2
  %144 = add nsw i16 %143, 1
  store i16 %144, ptr %8, !tbaa !2
  br label %b18

b21:
  %145 = getelementptr inbounds i8, ptr %9, i16 132
  store i16 %121, ptr %145, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %9, i16 134
  store i16 %123, ptr %146, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %9, i16 136
  store i16 %125, ptr %147, !tbaa !2
  store ptr null, ptr %23, !tbaa !2
  %148 = getelementptr inbounds i8, ptr %23, i16 2
  store i16 0, ptr %148, !tbaa !2
  %149 = getelementptr inbounds i8, ptr %23, i16 4
  %150 = addrspacecast ptr %149 to ptr addrspace(1)
  store i16 0, ptr %6, !tbaa !2
  store i16 128, ptr %5, !tbaa !2
  br label %b22

b22:
  %151 = load i16, ptr %6, !tbaa !2
  %152 = load i16, ptr %5, !tbaa !2
  %153 = icmp slt i16 %151, %152
  br i1 %153, label %b23, label %b25

b23:
  %154 = load i16, ptr %6, !tbaa !2
  %155 = getelementptr inbounds i8, ptr addrspace(1) %150, i16 %154
  store i8 0, ptr addrspace(1) %155
  br label %b24

b24:
  %156 = load i16, ptr %6, !tbaa !2
  %157 = add nsw i16 %156, 1
  store i16 %157, ptr %6, !tbaa !2
  br label %b22

b25:
  %158 = getelementptr inbounds i8, ptr %23, i16 132
  store i16 0, ptr %158, !tbaa !2
  %159 = getelementptr inbounds i8, ptr %23, i16 134
  store i16 0, ptr %159, !tbaa !2
  %160 = getelementptr inbounds i8, ptr %23, i16 136
  store i16 0, ptr %160, !tbaa !2
  %161 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @keep_open(ptr addrspace(1) %116, ptr addrspace(1) %161)
  %162 = load ptr, ptr %10, !tbaa !2
  %163 = getelementptr inbounds i8, ptr %10, i16 2
  %164 = load i16, ptr %163, !tbaa !2
  %165 = getelementptr inbounds i8, ptr %10, i16 132
  %166 = load i16, ptr %165, !tbaa !2
  %167 = getelementptr inbounds i8, ptr %10, i16 134
  %168 = load i16, ptr %167, !tbaa !2
  %169 = getelementptr inbounds i8, ptr %10, i16 136
  %170 = load i16, ptr %169, !tbaa !2
  store ptr %162, ptr %11, !tbaa !2
  %171 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %164, ptr %171, !tbaa !2
  %172 = getelementptr inbounds i8, ptr %11, i16 4
  %173 = addrspacecast ptr %172 to ptr addrspace(1)
  %174 = getelementptr inbounds i8, ptr %10, i16 4
  %175 = addrspacecast ptr %174 to ptr addrspace(1)
  store i16 0, ptr %4, !tbaa !2
  store i16 128, ptr %3, !tbaa !2
  br label %b26

b26:
  %176 = load i16, ptr %4, !tbaa !2
  %177 = load i16, ptr %3, !tbaa !2
  %178 = icmp slt i16 %176, %177
  br i1 %178, label %b27, label %b29

b27:
  %179 = load i16, ptr %4, !tbaa !2
  %180 = getelementptr inbounds i8, ptr addrspace(1) %173, i16 %179
  %181 = load i16, ptr %4, !tbaa !2
  %182 = getelementptr inbounds i8, ptr addrspace(1) %175, i16 %181
  %183 = load i8, ptr addrspace(1) %182
  store i8 %183, ptr addrspace(1) %180
  br label %b28

b28:
  %184 = load i16, ptr %4, !tbaa !2
  %185 = add nsw i16 %184, 1
  store i16 %185, ptr %4, !tbaa !2
  br label %b26

b29:
  %186 = getelementptr inbounds i8, ptr %11, i16 132
  store i16 %166, ptr %186, !tbaa !2
  %187 = getelementptr inbounds i8, ptr %11, i16 134
  store i16 %168, ptr %187, !tbaa !2
  %188 = getelementptr inbounds i8, ptr %11, i16 136
  store i16 %170, ptr %188, !tbaa !2
  store i8 1, ptr %2, !tbaa !2
  %189 = addrspacecast ptr %11 to ptr addrspace(1)
  %190 = getelementptr i8, ptr @$str10, i16 6
  %191 = getelementptr i8, ptr %190, i16 -4
  %192 = load i16, ptr %191
  %193 = addrspacecast ptr %190 to ptr addrspace(1)
  store i16 %192, ptr %1, !tbaa !2
  %194 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %192, ptr %194, !tbaa !2
  %195 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %193, ptr %195, !tbaa !2
  %196 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Log.write(ptr addrspace(1) %189, ptr addrspace(1) %196)
  %197 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %197)
  call cc 1005 addrspace(1) void @N$PN()
  store i8 0, ptr addrspace(1) %0
  %198 = load i8, ptr %2, !tbaa !2, !range !10
  %199 = icmp ne i8 %198, 0
  br i1 %199, label %b31, label %b30

b30:
  %200 = load i8, ptr %19, !tbaa !2, !range !11
  %201 = icmp ne i8 %200, 0
  br i1 %201, label %b33, label %b32

b31:
  %202 = addrspacecast ptr %11 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Log.drop(ptr addrspace(1) %202)
  %203 = getelementptr inbounds i8, ptr %11, i16 2
  %204 = addrspacecast ptr %203 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %204)
  %205 = load ptr, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %205)
  br label %b30

b32:
  ret void

b33:
  %206 = addrspacecast ptr %23 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Log.drop(ptr addrspace(1) %206)
  %207 = getelementptr inbounds i8, ptr %23, i16 2
  %208 = addrspacecast ptr %207 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %208)
  %209 = load ptr, ptr %23, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %209)
  br label %b32
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [6 x i8]
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @$main(ptr addrspace(1) %1)
  %2 = load i8, ptr %0, !tbaa !2, !range !14
  %3 = icmp eq i8 %2, 0
  br i1 %3, label %b4, label %b3

b3:
  ret i16 1

b4:
  ret i16 0
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

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

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
