target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [17 x i8] c"\08\00\0A\00\0A\00VALUES.DAT\00"
@$str3 = internal constant [17 x i8] c"\08\00\0A\00\0A\00no entries\00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00first=\00"
@$str5 = internal constant [19 x i8] c"\08\00\0C\00\0C\00, remaining=\00"
@$str6 = internal constant [24 x i8] c"\08\00\11\00\11\00 positive, width=\00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00width\00"

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

define internal cc 1005 void @parse(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = alloca i8
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i8
  %10 = alloca i8
  %11 = alloca i16
  store i16 0, ptr %11, !tbaa !2
  br label %b2

b2:
  %12 = load i16, ptr %11, !tbaa !2
  %13 = load i16, ptr addrspace(1) %1
  %14 = icmp ult i16 %12, %13
  br i1 %14, label %b5, label %b4

b3:
  %15 = load i16, ptr %11, !tbaa !2
  %16 = add i16 %15, 1
  store i16 %16, ptr %11, !tbaa !2
  br label %b2

b4:
  %17 = load i16, ptr %11, !tbaa !2
  %18 = add i16 %17, 1
  %19 = load i16, ptr addrspace(1) %1
  %20 = icmp ult i16 %18, %19
  %21 = zext i1 %20 to i8
  store i8 %21, ptr %10, !tbaa !2
  br i1 %20, label %b8, label %b9

b5:
  %22 = load i16, ptr %11, !tbaa !2
  %23 = load i16, ptr addrspace(1) %1
  %24 = icmp ult i16 %22, %23
  br i1 %24, label %b6, label %b7

b6:
  %25 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %26 = load ptr addrspace(1), ptr addrspace(1) %25
  %27 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 %22
  %28 = load i8, ptr addrspace(1) %27
  %29 = icmp ne i8 %28, 61
  br i1 %29, label %b3, label %b4

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %30 = load i16, ptr %11, !tbaa !2
  %31 = add i16 %30, 1
  %32 = load i16, ptr addrspace(1) %1
  %33 = icmp ult i16 %31, %32
  br i1 %33, label %b10, label %b11

b9:
  %34 = load i8, ptr %10, !tbaa !2, !range !6
  store i8 %34, ptr %9, !tbaa !2
  %35 = load i8, ptr %9, !tbaa !2, !range !7
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b12, label %b13

b10:
  %37 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %38 = load ptr addrspace(1), ptr addrspace(1) %37
  %39 = getelementptr inbounds i8, ptr addrspace(1) %38, i16 %31
  %40 = load i8, ptr addrspace(1) %39
  %41 = icmp eq i8 %40, 45
  %42 = zext i1 %41 to i8
  store i8 %42, ptr %10, !tbaa !2
  br label %b9

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b12:
  %43 = load i16, ptr %11, !tbaa !2
  %44 = add i16 %43, 2
  store i16 %44, ptr %8, !tbaa !2
  br label %b14

b13:
  %45 = load i16, ptr %11, !tbaa !2
  %46 = add i16 %45, 1
  store i16 %46, ptr %8, !tbaa !2
  br label %b14

b14:
  %47 = load i16, ptr %8, !tbaa !2
  store i16 %47, ptr %7, !tbaa !2
  %48 = load i16, ptr %11, !tbaa !2
  %49 = icmp eq i16 %48, 0
  br i1 %49, label %b15, label %b18

b15:
  store i8 1, ptr addrspace(1) %0
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 2, ptr addrspace(1) %50
  %51 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %51
  ret void

b16:
  br label %b17

b17:
  store i16 0, ptr %6, !tbaa !2
  br label %b19

b18:
  %52 = load i16, ptr %7, !tbaa !2
  %53 = load i16, ptr addrspace(1) %1
  %54 = icmp uge i16 %52, %53
  br i1 %54, label %b15, label %b16

b19:
  %55 = load i16, ptr %7, !tbaa !2
  %56 = load i16, ptr addrspace(1) %1
  %57 = icmp ult i16 %55, %56
  br i1 %57, label %b20, label %b21

b20:
  %58 = load i16, ptr %7, !tbaa !2
  %59 = load i16, ptr addrspace(1) %1
  %60 = icmp ult i16 %58, %59
  br i1 %60, label %b22, label %b23

b21:
  %61 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %62 = load ptr addrspace(1), ptr addrspace(1) %61
  %63 = load i16, ptr addrspace(1) %1
  %64 = load i16, ptr %11, !tbaa !2
  %65 = icmp ule i16 %64, %63
  br i1 %65, label %b28, label %b29

b22:
  %66 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %67 = load ptr addrspace(1), ptr addrspace(1) %66
  %68 = getelementptr inbounds i8, ptr addrspace(1) %67, i16 %58
  %69 = load i8, ptr addrspace(1) %68
  store i8 %69, ptr %5, !tbaa !2
  %70 = load i8, ptr %5, !tbaa !2
  %71 = icmp ult i8 %70, 48
  br i1 %71, label %b24, label %b27

b23:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b24:
  store i8 1, ptr addrspace(1) %0
  %72 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 2, ptr addrspace(1) %72
  %73 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %2, ptr addrspace(1) %73
  ret void

b25:
  br label %b26

b26:
  %74 = load i16, ptr %6, !tbaa !2
  %75 = mul i16 %74, 10
  %76 = load i8, ptr %5, !tbaa !2
  %77 = zext i8 %76 to i16
  %78 = sub i16 %77, 48
  %79 = add i16 %75, %78
  store i16 %79, ptr %6, !tbaa !2
  %80 = load i16, ptr %7, !tbaa !2
  %81 = add i16 %80, 1
  store i16 %81, ptr %7, !tbaa !2
  br label %b19

b27:
  %82 = load i8, ptr %5, !tbaa !2
  %83 = icmp ugt i8 %82, 57
  br i1 %83, label %b24, label %b25

b28:
  %84 = icmp ule i16 0, %64
  br i1 %84, label %b30, label %b31

b29:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b30:
  store i16 %64, ptr %4, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %64, ptr %85, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %62, ptr %86, !tbaa !2
  %87 = addrspacecast ptr %4 to ptr addrspace(1)
  %88 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %87)
  %89 = load i8, ptr %9, !tbaa !2, !range !8
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b32, label %b33

b31:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b32:
  %91 = load i16, ptr %6, !tbaa !2
  %92 = sub i16 0, %91
  store i16 %92, ptr %3, !tbaa !2
  br label %b34

b33:
  %93 = load i16, ptr %6, !tbaa !2
  store i16 %93, ptr %3, !tbaa !2
  br label %b34

b34:
  %94 = load i16, ptr %3, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %95 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %88, ptr addrspace(1) %95
  %96 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store i16 %94, ptr addrspace(1) %96
  ret void
}

define internal cc 1005 void @load_entries(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %23 = addrspacecast ptr %22 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.open(ptr addrspace(1) %23, ptr addrspace(1) %1, i8 zeroext 0)
  %24 = load i8, ptr %22, !tbaa !2, !range !16
  %25 = icmp eq i8 %24, 0
  br i1 %25, label %b4, label %b3

b3:
  %26 = load i8, ptr %22, !tbaa !2, !range !16
  %27 = icmp eq i8 %26, 1
  br i1 %27, label %b59, label %b58

b4:
  %28 = getelementptr inbounds i8, ptr %22, i16 2
  %29 = load i16, ptr %28, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %22, i16 132
  %31 = load i16, ptr %30, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %22, i16 134
  %33 = load i16, ptr %32, !tbaa !2
  store i16 %29, ptr %21, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %21, i16 2
  %35 = addrspacecast ptr %34 to ptr addrspace(1)
  %36 = getelementptr inbounds i8, ptr %22, i16 4
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  store i16 0, ptr %20, !tbaa !2
  store i16 128, ptr %19, !tbaa !2
  br label %b5

b5:
  %38 = load i16, ptr %20, !tbaa !2
  %39 = load i16, ptr %19, !tbaa !2
  %40 = icmp slt i16 %38, %39
  br i1 %40, label %b6, label %b8

b6:
  %41 = load i16, ptr %20, !tbaa !2
  %42 = getelementptr inbounds i8, ptr addrspace(1) %35, i16 %41
  %43 = load i16, ptr %20, !tbaa !2
  %44 = getelementptr inbounds i8, ptr addrspace(1) %37, i16 %43
  %45 = load i8, ptr addrspace(1) %44
  store i8 %45, ptr addrspace(1) %42
  br label %b7

b7:
  %46 = load i16, ptr %20, !tbaa !2
  %47 = add nsw i16 %46, 1
  store i16 %47, ptr %20, !tbaa !2
  br label %b5

b8:
  %48 = getelementptr inbounds i8, ptr %21, i16 130
  store i16 %31, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %21, i16 132
  store i16 %33, ptr %49, !tbaa !2
  store i8 1, ptr %18, !tbaa !2
  %50 = load i16, ptr %21, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %21, i16 130
  %52 = load i16, ptr %51, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %21, i16 132
  %54 = load i16, ptr %53, !tbaa !2
  store i8 0, ptr %18, !tbaa !2
  store i16 %50, ptr %17, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %17, i16 2
  %56 = addrspacecast ptr %55 to ptr addrspace(1)
  %57 = getelementptr inbounds i8, ptr %21, i16 2
  %58 = addrspacecast ptr %57 to ptr addrspace(1)
  store i16 0, ptr %16, !tbaa !2
  store i16 128, ptr %15, !tbaa !2
  br label %b9

b9:
  %59 = load i16, ptr %16, !tbaa !2
  %60 = load i16, ptr %15, !tbaa !2
  %61 = icmp slt i16 %59, %60
  br i1 %61, label %b10, label %b12

b10:
  %62 = load i16, ptr %16, !tbaa !2
  %63 = getelementptr inbounds i8, ptr addrspace(1) %56, i16 %62
  %64 = load i16, ptr %16, !tbaa !2
  %65 = getelementptr inbounds i8, ptr addrspace(1) %58, i16 %64
  %66 = load i8, ptr addrspace(1) %65
  store i8 %66, ptr addrspace(1) %63
  br label %b11

b11:
  %67 = load i16, ptr %16, !tbaa !2
  %68 = add nsw i16 %67, 1
  store i16 %68, ptr %16, !tbaa !2
  br label %b9

b12:
  %69 = getelementptr inbounds i8, ptr %17, i16 130
  store i16 %52, ptr %69, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %17, i16 132
  store i16 %54, ptr %70, !tbaa !2
  store i16 0, ptr %21, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %21, i16 2
  %72 = addrspacecast ptr %71 to ptr addrspace(1)
  store i16 0, ptr %14, !tbaa !2
  store i16 128, ptr %13, !tbaa !2
  br label %b13

b13:
  %73 = load i16, ptr %14, !tbaa !2
  %74 = load i16, ptr %13, !tbaa !2
  %75 = icmp slt i16 %73, %74
  br i1 %75, label %b14, label %b16

b14:
  %76 = load i16, ptr %14, !tbaa !2
  %77 = getelementptr inbounds i8, ptr addrspace(1) %72, i16 %76
  store i8 0, ptr addrspace(1) %77
  br label %b15

b15:
  %78 = load i16, ptr %14, !tbaa !2
  %79 = add nsw i16 %78, 1
  store i16 %79, ptr %14, !tbaa !2
  br label %b13

b16:
  %80 = getelementptr inbounds i8, ptr %21, i16 130
  store i16 0, ptr %80, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %21, i16 132
  store i16 0, ptr %81, !tbaa !2
  store i8 1, ptr %12, !tbaa !2
  %82 = getelementptr i8, ptr @$str1, i16 6
  store ptr %82, ptr %11, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %83 = addrspacecast ptr %17 to ptr addrspace(1)
  br label %b18

b17:
  %84 = load ptr, ptr %11, !tbaa !2
  store ptr null, ptr %11, !tbaa !2
  store i8 0, ptr addrspace(1) %0
  %85 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr %84, ptr addrspace(1) %85
  %86 = load ptr, ptr %11, !tbaa !2
  %87 = icmp ne ptr %86, null
  br i1 %87, label %b50, label %b49

b18:
  br label %b19

b19:
  %88 = addrspacecast ptr %9 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.read_line(ptr addrspace(1) %88, ptr addrspace(1) %83)
  %89 = load i8, ptr %9, !tbaa !2, !range !16
  %90 = icmp eq i8 %89, 0
  br i1 %90, label %b23, label %b22

b21:
  br label %b18

b22:
  %91 = load i8, ptr %9, !tbaa !2, !range !16
  %92 = icmp eq i8 %91, 0
  br i1 %92, label %b42, label %b41

b23:
  %93 = getelementptr inbounds i8, ptr %9, i16 2
  %94 = load i8, ptr %93, !tbaa !2, !range !16
  %95 = icmp eq i8 %94, 0
  br i1 %95, label %b24, label %b22

b24:
  %96 = getelementptr inbounds i8, ptr %9, i16 4
  %97 = load ptr, ptr %96, !tbaa !2
  %98 = getelementptr inbounds i8, ptr %9, i16 4
  %99 = load ptr, ptr %98, !tbaa !2
  store ptr %99, ptr %8, !tbaa !2
  %100 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  store ptr %100, ptr %7, !tbaa !2
  %101 = load i16, ptr %10, !tbaa !2
  %102 = add i16 %101, 1
  store i16 %102, ptr %10, !tbaa !2
  %103 = load ptr, ptr %7, !tbaa !2
  %104 = getelementptr i8, ptr %103, i16 -4
  %105 = load i16, ptr %104
  %106 = icmp eq i16 %105, 0
  br i1 %106, label %b27, label %b26

b25:
  %107 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %107)
  br label %b21

b26:
  %108 = addrspacecast ptr %5 to ptr addrspace(1)
  %109 = load ptr, ptr %7, !tbaa !2
  %110 = getelementptr i8, ptr %109, i16 -4
  %111 = load i16, ptr %110
  %112 = addrspacecast ptr %109 to ptr addrspace(1)
  store i16 %111, ptr %4, !tbaa !2
  %113 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %111, ptr %113, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %112, ptr %114, !tbaa !2
  %115 = addrspacecast ptr %4 to ptr addrspace(1)
  %116 = load i16, ptr %10, !tbaa !2
  call cc 1005 addrspace(1) void @parse(ptr addrspace(1) %108, ptr addrspace(1) %115, i16 %116)
  %117 = load i8, ptr %5, !tbaa !2, !range !16
  %118 = icmp eq i8 %117, 1
  br i1 %118, label %b29, label %b30

b27:
  br label %b28

b28:
  %119 = load ptr, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %119)
  br label %b25

b29:
  store i8 1, ptr addrspace(1) %0
  %120 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %121 = getelementptr inbounds i8, ptr %5, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %120, ptr %121, i16 4, i1 false)
  %122 = load ptr, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %122)
  %123 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %123)
  %124 = load ptr, ptr %11, !tbaa !2
  %125 = icmp ne ptr %124, null
  br i1 %125, label %b32, label %b31

b30:
  %126 = getelementptr inbounds i8, ptr %5, i16 2
  %127 = load ptr, ptr %126, !tbaa !2
  %128 = getelementptr inbounds i8, ptr %5, i16 4
  %129 = load i16, ptr %128, !tbaa !2
  store ptr %127, ptr %6, !tbaa !2
  %130 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %129, ptr %130, !tbaa !2
  %131 = load ptr, ptr %11, !tbaa !2
  %132 = getelementptr i8, ptr %131, i16 -4
  %133 = load i16, ptr %132
  %134 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %131, i16 1, i16 4)
  store ptr %134, ptr %11, !tbaa !2
  %135 = mul i16 %133, 4
  %136 = getelementptr inbounds i8, ptr %134, i16 %135
  %137 = load ptr, ptr %6, !tbaa !2
  %138 = getelementptr inbounds i8, ptr %6, i16 2
  %139 = load i16, ptr %138, !tbaa !2
  store ptr %137, ptr %136
  %140 = getelementptr i8, ptr %136, i16 2
  store i16 %139, ptr %140
  store ptr null, ptr %6, !tbaa !2
  %141 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 0, ptr %141, !tbaa !2
  %142 = load ptr, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %142)
  br label %b28

b31:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %124)
  %143 = load i8, ptr %12, !tbaa !2, !range !9
  %144 = icmp ne i8 %143, 0
  br i1 %144, label %b37, label %b36

b32:
  %145 = getelementptr i8, ptr %124, i16 -4
  %146 = load i16, ptr %145
  store i16 0, ptr %3, !tbaa !2
  br label %b33

b33:
  %147 = load i16, ptr %3, !tbaa !2
  %148 = icmp ult i16 %147, %146
  br i1 %148, label %b35, label %b34

b34:
  br label %b31

b35:
  %149 = mul i16 %147, 4
  %150 = getelementptr inbounds i8, ptr %124, i16 %149
  %151 = load ptr, ptr %150
  call cc 1005 addrspace(1) void @N$BDRP(ptr %151)
  %152 = add i16 %147, 1
  store i16 %152, ptr %3, !tbaa !2
  br label %b33

b36:
  %153 = load i8, ptr %18, !tbaa !2, !range !10
  %154 = icmp ne i8 %153, 0
  br i1 %154, label %b39, label %b38

b37:
  %155 = addrspacecast ptr %17 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %155)
  br label %b36

b38:
  ret void

b39:
  %156 = addrspacecast ptr %21 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %156)
  br label %b38

b41:
  br label %b17

b42:
  %157 = getelementptr inbounds i8, ptr %9, i16 2
  %158 = load i8, ptr %157, !tbaa !2, !range !16
  %159 = icmp eq i8 %158, 0
  br i1 %159, label %b44, label %b43

b43:
  br label %b41

b44:
  %160 = getelementptr inbounds i8, ptr %9, i16 4
  %161 = load ptr, ptr %160, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %161)
  br label %b43

b49:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %86)
  %162 = load i8, ptr %12, !tbaa !2, !range !11
  %163 = icmp ne i8 %162, 0
  br i1 %163, label %b55, label %b54

b50:
  %164 = getelementptr i8, ptr %86, i16 -4
  %165 = load i16, ptr %164
  store i16 0, ptr %2, !tbaa !2
  br label %b51

b51:
  %166 = load i16, ptr %2, !tbaa !2
  %167 = icmp ult i16 %166, %165
  br i1 %167, label %b53, label %b52

b52:
  br label %b49

b53:
  %168 = mul i16 %166, 4
  %169 = getelementptr inbounds i8, ptr %86, i16 %168
  %170 = load ptr, ptr %169
  call cc 1005 addrspace(1) void @N$BDRP(ptr %170)
  %171 = add i16 %166, 1
  store i16 %171, ptr %2, !tbaa !2
  br label %b51

b54:
  %172 = load i8, ptr %18, !tbaa !2, !range !12
  %173 = icmp ne i8 %172, 0
  br i1 %173, label %b57, label %b56

b55:
  %174 = addrspacecast ptr %17 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %174)
  br label %b54

b56:
  ret void

b57:
  %175 = addrspacecast ptr %21 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @std.io.File.drop(ptr addrspace(1) %175)
  br label %b56

b58:
  store i8 1, ptr addrspace(1) %0
  %176 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 1, ptr addrspace(1) %176
  ret void

b59:
  %177 = getelementptr inbounds i8, ptr %22, i16 2
  %178 = load i8, ptr %177, !tbaa !2, !range !17
  %179 = icmp eq i8 %178, 0
  br i1 %179, label %b60, label %b58

b60:
  store i8 1, ptr addrspace(1) %0
  %180 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i32 0, ptr addrspace(1) %180
  ret void
}

define internal cc 1005 void @$main(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca i8
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca [8 x i8]
  %11 = alloca ptr
  %12 = alloca [8 x i8]
  %13 = alloca [8 x i8]
  %14 = alloca ptr
  %15 = alloca [8 x i8]
  %16 = alloca [8 x i8]
  %17 = alloca i16
  %18 = alloca i8
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca [8 x i8]
  %22 = alloca ptr
  %23 = alloca i16
  %24 = alloca ptr
  %25 = alloca ptr
  %26 = alloca [8 x i8]
  %27 = alloca [6 x i8]
  %28 = addrspacecast ptr %27 to ptr addrspace(1)
  %29 = getelementptr i8, ptr @$str2, i16 6
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 %31, ptr %26, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %26, i16 2
  store i16 %31, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %26, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %26 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @load_entries(ptr addrspace(1) %28, ptr addrspace(1) %35)
  %36 = load i8, ptr %27, !tbaa !2, !range !18
  %37 = icmp eq i8 %36, 1
  br i1 %37, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %39 = getelementptr inbounds i8, ptr %27, i16 2
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %38, ptr %39, i16 4, i1 false)
  ret void

b3:
  %40 = getelementptr inbounds i8, ptr %27, i16 2
  %41 = load ptr, ptr %40, !tbaa !2
  store ptr %41, ptr %25, !tbaa !2
  %42 = getelementptr i8, ptr @$str1, i16 6
  store ptr %42, ptr %24, !tbaa !2
  %43 = load ptr, ptr %25, !tbaa !2
  %44 = getelementptr i8, ptr %43, i16 -4
  %45 = load i16, ptr %44
  store i16 0, ptr %23, !tbaa !2
  br label %b4

b4:
  %46 = load i16, ptr %23, !tbaa !2
  %47 = icmp ult i16 %46, %45
  br i1 %47, label %b5, label %b7

b5:
  %48 = mul i16 %46, 4
  %49 = getelementptr inbounds i8, ptr %43, i16 %48
  %50 = getelementptr i8, ptr %49, i16 2
  %51 = load i16, ptr %50
  %52 = icmp sgt i16 %51, 0
  br i1 %52, label %b8, label %b9

b6:
  %53 = load i16, ptr %23, !tbaa !2
  %54 = add i16 %53, 1
  store i16 %54, ptr %23, !tbaa !2
  br label %b4

b7:
  %55 = load ptr, ptr %24, !tbaa !2
  store ptr null, ptr %24, !tbaa !2
  store ptr %55, ptr %14, !tbaa !2
  %56 = load ptr, ptr %25, !tbaa !2
  %57 = getelementptr i8, ptr %56, i16 -4
  %58 = load i16, ptr %57
  %59 = addrspacecast ptr %56 to ptr addrspace(1)
  store i16 %58, ptr %13, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %58, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %59, ptr %61, !tbaa !2
  %62 = addrspacecast ptr %13 to ptr addrspace(1)
  %63 = getelementptr i8, ptr addrspace(1) %62, i16 4
  %64 = load ptr addrspace(1), ptr addrspace(1) %63, !tbaa !2
  %65 = load i16, ptr addrspace(1) %62, !tbaa !2
  %66 = icmp eq i16 %65, 0
  br i1 %66, label %b40, label %b39

b8:
  %67 = load ptr, ptr %24, !tbaa !2
  %68 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %67, i16 6)
  store ptr %68, ptr %24, !tbaa !2
  %69 = load ptr, ptr %49
  %70 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %69, i16 1)
  store ptr %70, ptr %22, !tbaa !2
  %71 = load ptr, ptr %22, !tbaa !2
  %72 = getelementptr i8, ptr %71, i16 -4
  %73 = load i16, ptr %72
  %74 = addrspacecast ptr %71 to ptr addrspace(1)
  store i16 %73, ptr %21, !tbaa !2
  %75 = getelementptr inbounds i8, ptr %21, i16 2
  store i16 %73, ptr %75, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %21, i16 4
  store ptr addrspace(1) %74, ptr %76, !tbaa !2
  %77 = addrspacecast ptr %21 to ptr addrspace(1)
  %78 = call cc 1005 addrspace(1) i16 @string.hash(ptr addrspace(1) %77)
  %79 = or i16 %78, 1
  store i16 %79, ptr %20, !tbaa !2
  store i16 0, ptr %19, !tbaa !2
  store i8 0, ptr %18, !tbaa !2
  %80 = getelementptr i8, ptr %68, i16 -4
  %81 = load i16, ptr %80
  %82 = icmp ne i16 %81, 0
  br i1 %82, label %b11, label %b12

b9:
  br label %b10

b10:
  br label %b6

b11:
  %83 = getelementptr i8, ptr %68, i16 -4
  %84 = load i16, ptr %83
  %85 = sub i16 %84, 1
  store i16 %85, ptr %17, !tbaa !2
  %86 = load i16, ptr %20, !tbaa !2
  %87 = load i16, ptr %17, !tbaa !2
  %88 = and i16 %86, %87
  store i16 %88, ptr %19, !tbaa !2
  br label %b14

b12:
  br label %b13

b13:
  %89 = load i8, ptr %18, !tbaa !2, !range !13
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b28, label %b27

b14:
  %91 = load i16, ptr %19, !tbaa !2
  %92 = getelementptr i8, ptr %68, i16 -4
  %93 = load i16, ptr %92
  %94 = icmp ult i16 %91, %93
  br i1 %94, label %b17, label %b18

b15:
  %95 = load i16, ptr %19, !tbaa !2
  %96 = getelementptr i8, ptr %68, i16 -4
  %97 = load i16, ptr %96
  %98 = icmp ult i16 %95, %97
  br i1 %98, label %b23, label %b24

b16:
  br label %b13

b17:
  %99 = mul i16 %91, 6
  %100 = getelementptr inbounds i8, ptr %68, i16 %99
  %101 = load i16, ptr %100
  %102 = icmp ne i16 %101, 0
  br i1 %102, label %b15, label %b16

b18:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b19:
  store i8 1, ptr %18, !tbaa !2
  br label %b16

b20:
  br label %b21

b21:
  %103 = load i16, ptr %19, !tbaa !2
  %104 = add i16 %103, 1
  %105 = load i16, ptr %17, !tbaa !2
  %106 = and i16 %104, %105
  store i16 %106, ptr %19, !tbaa !2
  br label %b14

b22:
  %107 = load i16, ptr %19, !tbaa !2
  %108 = getelementptr i8, ptr %68, i16 -4
  %109 = load i16, ptr %108
  %110 = icmp ult i16 %107, %109
  br i1 %110, label %b25, label %b26

b23:
  %111 = mul i16 %95, 6
  %112 = getelementptr inbounds i8, ptr %68, i16 %111
  %113 = load i16, ptr %112
  %114 = load i16, ptr %20, !tbaa !2
  %115 = icmp eq i16 %113, %114
  br i1 %115, label %b22, label %b20

b24:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b25:
  %116 = mul i16 %107, 6
  %117 = getelementptr inbounds i8, ptr %68, i16 %116
  %118 = getelementptr i8, ptr %117, i16 2
  %119 = load ptr, ptr %118
  %120 = getelementptr i8, ptr %119, i16 -4
  %121 = load i16, ptr %120
  %122 = addrspacecast ptr %119 to ptr addrspace(1)
  store i16 %121, ptr %16, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %16, i16 2
  store i16 %121, ptr %123, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %16, i16 4
  store ptr addrspace(1) %122, ptr %124, !tbaa !2
  %125 = addrspacecast ptr %16 to ptr addrspace(1)
  %126 = load ptr, ptr %22, !tbaa !2
  %127 = getelementptr i8, ptr %126, i16 -4
  %128 = load i16, ptr %127
  %129 = addrspacecast ptr %126 to ptr addrspace(1)
  store i16 %128, ptr %15, !tbaa !2
  %130 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 %128, ptr %130, !tbaa !2
  %131 = getelementptr inbounds i8, ptr %15, i16 4
  store ptr addrspace(1) %129, ptr %131, !tbaa !2
  %132 = addrspacecast ptr %15 to ptr addrspace(1)
  %133 = call cc 1005 addrspace(1) i8 @string.eq(ptr addrspace(1) %125, ptr addrspace(1) %132)
  %134 = icmp ne i8 %133, 0
  br i1 %134, label %b19, label %b20

b26:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b27:
  %135 = load i16, ptr %19, !tbaa !2
  %136 = getelementptr i8, ptr %68, i16 -4
  %137 = load i16, ptr %136
  %138 = icmp ult i16 %135, %137
  br i1 %138, label %b30, label %b31

b28:
  br label %b29

b29:
  %139 = load i8, ptr %18, !tbaa !2, !range !14
  %140 = icmp ne i8 %139, 0
  br i1 %140, label %b35, label %b34

b30:
  %141 = mul i16 %135, 6
  %142 = getelementptr inbounds i8, ptr %68, i16 %141
  %143 = load i16, ptr %20, !tbaa !2
  store i16 %143, ptr %142
  %144 = load i16, ptr %19, !tbaa !2
  %145 = getelementptr i8, ptr %68, i16 -4
  %146 = load i16, ptr %145
  %147 = icmp ult i16 %144, %146
  br i1 %147, label %b32, label %b33

b31:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b32:
  %148 = mul i16 %144, 6
  %149 = getelementptr inbounds i8, ptr %68, i16 %148
  %150 = load ptr, ptr %22, !tbaa !2
  store ptr null, ptr %22, !tbaa !2
  %151 = getelementptr i8, ptr %149, i16 2
  %152 = load ptr, ptr %151
  call cc 1005 addrspace(1) void @N$BDRP(ptr %152)
  %153 = getelementptr i8, ptr %149, i16 2
  store ptr %150, ptr %153
  br label %b29

b33:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b34:
  %154 = getelementptr i8, ptr %68, i16 -2
  %155 = load i16, ptr %154
  %156 = add i16 %155, 1
  %157 = getelementptr i8, ptr %68, i16 -2
  store i16 %156, ptr %157
  br label %b35

b35:
  %158 = load i16, ptr %19, !tbaa !2
  %159 = getelementptr i8, ptr %68, i16 -4
  %160 = load i16, ptr %159
  %161 = icmp ult i16 %158, %160
  br i1 %161, label %b36, label %b37

b36:
  %162 = mul i16 %158, 6
  %163 = getelementptr inbounds i8, ptr %68, i16 %162
  %164 = getelementptr i8, ptr %49, i16 2
  %165 = load i16, ptr %164
  %166 = getelementptr i8, ptr %163, i16 4
  store i16 %165, ptr %166
  %167 = load ptr, ptr %22, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %167)
  br label %b10

b37:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b38:
  %168 = load ptr, ptr %14, !tbaa !2
  %169 = getelementptr i8, ptr %168, i16 -2
  %170 = load i16, ptr %169
  call cc 1005 addrspace(1) void @N$PU2(i16 %170)
  %171 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %171)
  %172 = load ptr, ptr %14, !tbaa !2
  %173 = getelementptr i8, ptr @$str7, i16 6
  store ptr %173, ptr %11, !tbaa !2
  %174 = load ptr, ptr %11, !tbaa !2
  %175 = getelementptr i8, ptr %174, i16 -4
  %176 = load i16, ptr %175
  %177 = addrspacecast ptr %174 to ptr addrspace(1)
  store i16 %176, ptr %10, !tbaa !2
  %178 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %176, ptr %178, !tbaa !2
  %179 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %177, ptr %179, !tbaa !2
  %180 = addrspacecast ptr %10 to ptr addrspace(1)
  %181 = call cc 1005 addrspace(1) i16 @string.hash(ptr addrspace(1) %180)
  %182 = or i16 %181, 1
  store i16 %182, ptr %9, !tbaa !2
  store i16 0, ptr %8, !tbaa !2
  store i8 0, ptr %7, !tbaa !2
  %183 = getelementptr i8, ptr %172, i16 -4
  %184 = load i16, ptr %183
  %185 = icmp ne i16 %184, 0
  br i1 %185, label %b42, label %b43

b39:
  %186 = getelementptr inbounds i8, ptr addrspace(1) %64, i16 0
  %187 = getelementptr inbounds i8, ptr addrspace(1) %64, i16 4
  %188 = sub i16 %65, 1
  store i16 %188, ptr %12, !tbaa !2
  %189 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 %188, ptr %189, !tbaa !2
  %190 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %187, ptr %190, !tbaa !2
  %191 = addrspacecast ptr %12 to ptr addrspace(1)
  %192 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %192)
  %193 = load ptr, ptr addrspace(1) %186
  call cc 1005 addrspace(1) void @N$PS(ptr %193)
  %194 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %194)
  %195 = load i16, ptr addrspace(1) %191
  call cc 1005 addrspace(1) void @N$PU2(i16 %195)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b38

b40:
  %196 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %196)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b38

b42:
  %197 = getelementptr i8, ptr %172, i16 -4
  %198 = load i16, ptr %197
  %199 = sub i16 %198, 1
  store i16 %199, ptr %6, !tbaa !2
  %200 = load i16, ptr %9, !tbaa !2
  %201 = load i16, ptr %6, !tbaa !2
  %202 = and i16 %200, %201
  store i16 %202, ptr %8, !tbaa !2
  br label %b45

b43:
  br label %b44

b44:
  %203 = load i8, ptr %7, !tbaa !2, !range !15
  %204 = icmp ne i8 %203, 0
  br i1 %204, label %b58, label %b59

b45:
  %205 = load i16, ptr %8, !tbaa !2
  %206 = getelementptr i8, ptr %172, i16 -4
  %207 = load i16, ptr %206
  %208 = icmp ult i16 %205, %207
  br i1 %208, label %b48, label %b49

b46:
  %209 = load i16, ptr %8, !tbaa !2
  %210 = getelementptr i8, ptr %172, i16 -4
  %211 = load i16, ptr %210
  %212 = icmp ult i16 %209, %211
  br i1 %212, label %b54, label %b55

b47:
  br label %b44

b48:
  %213 = mul i16 %205, 6
  %214 = getelementptr inbounds i8, ptr %172, i16 %213
  %215 = load i16, ptr %214
  %216 = icmp ne i16 %215, 0
  br i1 %216, label %b46, label %b47

b49:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b50:
  store i8 1, ptr %7, !tbaa !2
  br label %b47

b51:
  br label %b52

b52:
  %217 = load i16, ptr %8, !tbaa !2
  %218 = add i16 %217, 1
  %219 = load i16, ptr %6, !tbaa !2
  %220 = and i16 %218, %219
  store i16 %220, ptr %8, !tbaa !2
  br label %b45

b53:
  %221 = load i16, ptr %8, !tbaa !2
  %222 = getelementptr i8, ptr %172, i16 -4
  %223 = load i16, ptr %222
  %224 = icmp ult i16 %221, %223
  br i1 %224, label %b56, label %b57

b54:
  %225 = mul i16 %209, 6
  %226 = getelementptr inbounds i8, ptr %172, i16 %225
  %227 = load i16, ptr %226
  %228 = load i16, ptr %9, !tbaa !2
  %229 = icmp eq i16 %227, %228
  br i1 %229, label %b53, label %b51

b55:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b56:
  %230 = mul i16 %221, 6
  %231 = getelementptr inbounds i8, ptr %172, i16 %230
  %232 = getelementptr i8, ptr %231, i16 2
  %233 = load ptr, ptr %232
  %234 = getelementptr i8, ptr %233, i16 -4
  %235 = load i16, ptr %234
  %236 = addrspacecast ptr %233 to ptr addrspace(1)
  store i16 %235, ptr %5, !tbaa !2
  %237 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %235, ptr %237, !tbaa !2
  %238 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %236, ptr %238, !tbaa !2
  %239 = addrspacecast ptr %5 to ptr addrspace(1)
  %240 = load ptr, ptr %11, !tbaa !2
  %241 = getelementptr i8, ptr %240, i16 -4
  %242 = load i16, ptr %241
  %243 = addrspacecast ptr %240 to ptr addrspace(1)
  store i16 %242, ptr %4, !tbaa !2
  %244 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %242, ptr %244, !tbaa !2
  %245 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %243, ptr %245, !tbaa !2
  %246 = addrspacecast ptr %4 to ptr addrspace(1)
  %247 = call cc 1005 addrspace(1) i8 @string.eq(ptr addrspace(1) %239, ptr addrspace(1) %246)
  %248 = icmp ne i8 %247, 0
  br i1 %248, label %b50, label %b51

b57:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b58:
  %249 = load i16, ptr %8, !tbaa !2
  %250 = getelementptr i8, ptr %172, i16 -4
  %251 = load i16, ptr %250
  %252 = icmp ult i16 %249, %251
  br i1 %252, label %b60, label %b61

b59:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

b60:
  %253 = mul i16 %249, 6
  %254 = getelementptr inbounds i8, ptr %172, i16 %253
  %255 = getelementptr i8, ptr %254, i16 4
  %256 = load i16, ptr %255
  call cc 1005 addrspace(1) void @N$PI2(i16 %256)
  call cc 1005 addrspace(1) void @N$PN()
  store i8 0, ptr addrspace(1) %0
  %257 = load ptr, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %257)
  %258 = load ptr, ptr %14, !tbaa !2
  %259 = icmp ne ptr %258, null
  br i1 %259, label %b63, label %b62

b61:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b62:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %258)
  %260 = load ptr, ptr %24, !tbaa !2
  %261 = icmp ne ptr %260, null
  br i1 %261, label %b68, label %b67

b63:
  %262 = getelementptr i8, ptr %258, i16 -4
  %263 = load i16, ptr %262
  store i16 0, ptr %3, !tbaa !2
  br label %b64

b64:
  %264 = load i16, ptr %3, !tbaa !2
  %265 = icmp ult i16 %264, %263
  br i1 %265, label %b66, label %b65

b65:
  br label %b62

b66:
  %266 = mul i16 %264, 6
  %267 = getelementptr inbounds i8, ptr %258, i16 %266
  %268 = getelementptr i8, ptr %267, i16 2
  %269 = load ptr, ptr %268
  call cc 1005 addrspace(1) void @N$BDRP(ptr %269)
  %270 = add i16 %264, 1
  store i16 %270, ptr %3, !tbaa !2
  br label %b64

b67:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %260)
  %271 = load ptr, ptr %25, !tbaa !2
  %272 = icmp ne ptr %271, null
  br i1 %272, label %b73, label %b72

b68:
  %273 = getelementptr i8, ptr %260, i16 -4
  %274 = load i16, ptr %273
  store i16 0, ptr %2, !tbaa !2
  br label %b69

b69:
  %275 = load i16, ptr %2, !tbaa !2
  %276 = icmp ult i16 %275, %274
  br i1 %276, label %b71, label %b70

b70:
  br label %b67

b71:
  %277 = mul i16 %275, 6
  %278 = getelementptr inbounds i8, ptr %260, i16 %277
  %279 = getelementptr i8, ptr %278, i16 2
  %280 = load ptr, ptr %279
  call cc 1005 addrspace(1) void @N$BDRP(ptr %280)
  %281 = add i16 %275, 1
  store i16 %281, ptr %2, !tbaa !2
  br label %b69

b72:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %271)
  ret void

b73:
  %282 = getelementptr i8, ptr %271, i16 -4
  %283 = load i16, ptr %282
  store i16 0, ptr %1, !tbaa !2
  br label %b74

b74:
  %284 = load i16, ptr %1, !tbaa !2
  %285 = icmp ult i16 %284, %283
  br i1 %285, label %b76, label %b75

b75:
  br label %b72

b76:
  %286 = mul i16 %284, 4
  %287 = getelementptr inbounds i8, ptr %271, i16 %286
  %288 = load ptr, ptr %287
  call cc 1005 addrspace(1) void @N$BDRP(ptr %288)
  %289 = add i16 %284, 1
  store i16 %289, ptr %1, !tbaa !2
  br label %b74
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [6 x i8]
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @$main(ptr addrspace(1) %1)
  %2 = load i8, ptr %0, !tbaa !2, !range !19
  %3 = icmp eq i8 %2, 0
  br i1 %3, label %b4, label %b3

b3:
  ret i16 1

b4:
  ret i16 0
}

define internal cc 1005 i8 @string.eq(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %0, ptr addrspace(1) %1)
  %3 = icmp eq i8 %2, 0
  %4 = zext i1 %3 to i8
  ret i8 %4
}

define internal cc 1005 i16 @string.hash(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 5381, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %4
  %9 = load i16, ptr %2, !tbaa !2
  %10 = mul i16 %9, 33
  %11 = load i8, ptr addrspace(1) %8
  %12 = zext i8 %11 to i16
  %13 = xor i16 %10, %12
  store i16 %13, ptr %2, !tbaa !2
  br label %b4

b4:
  %14 = load i16, ptr %1, !tbaa !2
  %15 = add i16 %14, 1
  store i16 %15, ptr %1, !tbaa !2
  br label %b2

b5:
  %16 = load i16, ptr %2, !tbaa !2
  ret i16 %16
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

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 ptr @N$DRES(ptr, i16) addrspace(1)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EKEY() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

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
!17 = !{i8 0, i8 3}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
