target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$var_heap.heads = internal global [26 x i8] zeroinitializer
@$var_heap.occupied = internal global [2 x i8] zeroinitializer
@$var_heap.end = internal global [2 x i8] zeroinitializer
@$var_format.width = internal global [1 x i8] zeroinitializer
@$var_format.radix = internal global [1 x i8] c"\0A"
@$var_format.fill = internal global [1 x i8] c" "
@$var_format.left = internal global [1 x i8] zeroinitializer
@$var_format.sink = internal global [2 x i8] zeroinitializer
@$var_format.scratch = internal global [48 x i8] zeroinitializer
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00panic: \00"
@$str11 = internal constant [9 x i8] c"\08\00\02\00\02\00\0D\0A\00"
@$str12 = internal constant [35 x i8] c"\08\00\1C\00\1C\00division by zero or overflow\00"
@$str13 = internal constant [37 x i8] c"\08\00\1E\00\1E\00float outside the integer type\00"
@$str14 = internal constant [31 x i8] c"\08\00\18\00\18\00shift count out of range\00"
@$str15 = internal constant [26 x i8] c"\08\00\13\00\13\00index out of bounds\00"
@$str16 = internal constant [20 x i8] c"\08\00\0D\00\0D\00key not found\00"
@$str17 = internal constant [20 x i8] c"\08\00\0D\00\0D\00out of memory\00"
@$str18 = internal constant [28 x i8] c"\08\00\15\00\15\00pop from an empty vec\00"
@$str19 = internal constant [11 x i8] c"\08\00\04\00\04\00true\00"
@$str20 = internal constant [12 x i8] c"\08\00\05\00\05\00false\00"
@$str21 = internal constant [10 x i8] c"\08\00\03\00\03\00inf\00"
@$str22 = internal constant [10 x i8] c"\08\00\03\00\03\00nan\00"
@$str23 = internal constant [10 x i8] c"\08\00\03\00\03\000.0\00"
@$str24 = internal constant [9 x i8] c"\08\00\02\00\02\000.\00"

define internal void @os.write(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %0, i16 %1)
  ret void
}

define internal void @errors.panic(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %3 = getelementptr i8, ptr @$str10, i16 6
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 %5, ptr %2, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %5, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %6, ptr %8, !tbaa !2
  %9 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.say(ptr addrspace(1) %9)
  call addrspace(1) void @errors.say(ptr addrspace(1) %0)
  %10 = getelementptr i8, ptr @$str11, i16 6
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 %12, ptr %1, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %12, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @errors.say(ptr addrspace(1) %16)
  call addrspace(1) void @N$OEXT(i8 -1)
  ret void
}

define internal void @errors.say(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  store ptr addrspace(1) null, ptr %1
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @os.write(ptr addrspace(1) %4, i16 %5)
  ret void
}

define void @N$EDIV() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr i8, ptr @$str12, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %3, ptr %0, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define void @N$ECNV() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr i8, ptr @$str13, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %3, ptr %0, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define void @N$ESHF() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr i8, ptr @$str14, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %3, ptr %0, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define void @N$EBND() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr i8, ptr @$str15, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %3, ptr %0, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define void @N$EKEY() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr i8, ptr @$str16, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %3, ptr %0, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define internal ptr @heap.header(ptr %0) addrspace(1) {
b1:
  ret ptr %0
}

define internal i16 @heap.size_of(ptr %0) addrspace(1) {
b1:
  %1 = call addrspace(1) ptr @heap.header(ptr %0)
  %2 = load i16, ptr %1
  %3 = xor i16 3, -1
  %4 = and i16 %2, %3
  ret i16 %4
}

define internal ptr @heap.next(ptr %0) addrspace(1) {
b1:
  %1 = mul i16 2, 1
  %2 = getelementptr i8, ptr %0, i16 %1
  ret ptr %2
}

define internal ptr @heap.previous(ptr %0) addrspace(1) {
b1:
  %1 = mul i16 4, 1
  %2 = getelementptr i8, ptr %0, i16 %1
  ret ptr %2
}

define internal i16 @heap.class_of(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 0, ptr %2, !tbaa !2
  %3 = lshr i16 %0, 4
  store i16 %3, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ne i16 %4, 0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %2, !tbaa !2
  %9 = add i16 %8, 1
  store i16 %9, ptr %2, !tbaa !2
  %10 = load i16, ptr %1, !tbaa !2
  %11 = lshr i16 %10, 1
  store i16 %11, ptr %1, !tbaa !2
  br label %b2

b4:
  %12 = load i16, ptr %2, !tbaa !2
  ret i16 %12
}

define internal i16 @heap.rounded(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  %4 = icmp ugt i16 %0, -16
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  %7 = getelementptr i8, ptr @$str17, i16 6
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 %9, ptr %3, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %9, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %10, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %13)
  br label %b4

b3:
  br label %b4

b4:
  %14 = add i16 %0, 5
  %15 = xor i16 3, -1
  %16 = and i16 %14, %15
  store i16 %16, ptr %2, !tbaa !2
  %17 = load i16, ptr %2, !tbaa !2
  %18 = icmp ugt i16 %17, 8
  %19 = sext i1 %18 to i8
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b5, label %b6

b5:
  %21 = load i16, ptr %2, !tbaa !2
  store i16 %21, ptr %1, !tbaa !2
  br label %b7

b6:
  store i16 8, ptr %1, !tbaa !2
  br label %b7

b7:
  %22 = load i16, ptr %1, !tbaa !2
  ret i16 %22
}

define internal void @heap.unlink(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  %3 = alloca ptr
  store i16 0, ptr %1
  store ptr null, ptr %2
  store ptr null, ptr %3
  %4 = call addrspace(1) ptr @heap.next(ptr %0)
  %5 = load ptr, ptr %4
  store ptr %5, ptr %3, !tbaa !2
  %6 = call addrspace(1) ptr @heap.previous(ptr %0)
  %7 = load ptr, ptr %6
  store ptr %7, ptr %2, !tbaa !2
  %8 = load ptr, ptr %2, !tbaa !2
  %9 = icmp eq ptr %8, null
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  %12 = call addrspace(1) i16 @heap.size_of(ptr %0)
  %13 = call addrspace(1) i16 @heap.class_of(i16 %12)
  store i16 %13, ptr %1, !tbaa !2
  %14 = load i16, ptr %1, !tbaa !2
  %15 = load ptr, ptr %3, !tbaa !2
  %16 = sub i16 %14, 0
  %17 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %16
  store ptr %15, ptr %17, !tbaa !2
  %18 = load ptr, ptr %3, !tbaa !2
  %19 = icmp eq ptr %18, null
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b5, label %b6

b3:
  %22 = load ptr, ptr %2, !tbaa !2
  %23 = call addrspace(1) ptr @heap.next(ptr %22)
  %24 = load ptr, ptr %3, !tbaa !2
  store ptr %24, ptr %23
  br label %b4

b4:
  %25 = load ptr, ptr %3, !tbaa !2
  %26 = icmp eq ptr %25, null
  %27 = sext i1 %26 to i8
  %28 = xor i8 %27, -1
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b8, label %b9

b5:
  %30 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %31 = load i16, ptr %1, !tbaa !2
  %32 = shl i16 1, %31
  %33 = xor i16 %32, -1
  %34 = and i16 %30, %33
  store i16 %34, ptr @$var_heap.occupied, !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  br label %b4

b8:
  %35 = load ptr, ptr %3, !tbaa !2
  %36 = call addrspace(1) ptr @heap.previous(ptr %35)
  %37 = load ptr, ptr %2, !tbaa !2
  store ptr %37, ptr %36
  br label %b10

b9:
  br label %b10

b10:
  ret void
}

define internal void @heap.insert(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  store ptr null, ptr %2
  store i16 0, ptr %3
  %4 = call addrspace(1) ptr @heap.header(ptr %0)
  %5 = call addrspace(1) ptr @heap.header(ptr %0)
  %6 = load i16, ptr %5
  %7 = and i16 %6, 2
  %8 = or i16 %1, %7
  store i16 %8, ptr %4
  %9 = sub i16 %1, 2
  %10 = mul i16 %9, 1
  %11 = getelementptr i8, ptr %0, i16 %10
  %12 = call addrspace(1) ptr @heap.header(ptr %11)
  store i16 %1, ptr %12
  %13 = mul i16 %1, 1
  %14 = getelementptr i8, ptr %0, i16 %13
  %15 = call addrspace(1) ptr @heap.header(ptr %14)
  %16 = load i16, ptr %15
  %17 = xor i16 2, -1
  %18 = and i16 %16, %17
  store i16 %18, ptr %15
  %19 = call addrspace(1) i16 @heap.class_of(i16 %1)
  store i16 %19, ptr %3, !tbaa !2
  %20 = load i16, ptr %3, !tbaa !2
  %21 = sub i16 %20, 0
  %22 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %21
  %23 = load ptr, ptr %22, !tbaa !2
  store ptr %23, ptr %2, !tbaa !2
  %24 = call addrspace(1) ptr @heap.next(ptr %0)
  %25 = load ptr, ptr %2, !tbaa !2
  store ptr %25, ptr %24
  %26 = call addrspace(1) ptr @heap.previous(ptr %0)
  store ptr null, ptr %26
  %27 = load ptr, ptr %2, !tbaa !2
  %28 = icmp eq ptr %27, null
  %29 = sext i1 %28 to i8
  %30 = xor i8 %29, -1
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b2, label %b3

b2:
  %32 = load ptr, ptr %2, !tbaa !2
  %33 = call addrspace(1) ptr @heap.previous(ptr %32)
  store ptr %0, ptr %33
  br label %b4

b3:
  br label %b4

b4:
  %34 = load i16, ptr %3, !tbaa !2
  %35 = sub i16 %34, 0
  %36 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %35
  store ptr %0, ptr %36, !tbaa !2
  %37 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %38 = load i16, ptr %3, !tbaa !2
  %39 = shl i16 1, %38
  %40 = or i16 %37, %39
  store i16 %40, ptr @$var_heap.occupied, !tbaa !2
  ret void
}

define internal ptr @heap.fit(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  %3 = alloca i16
  store i16 0, ptr %1
  store ptr null, ptr %2
  store i16 0, ptr %3
  %4 = call addrspace(1) i16 @heap.class_of(i16 %0)
  store i16 %4, ptr %3, !tbaa !2
  %5 = load i16, ptr %3, !tbaa !2
  %6 = sub i16 %5, 0
  %7 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %6
  %8 = load ptr, ptr %7, !tbaa !2
  store ptr %8, ptr %2, !tbaa !2
  br label %b2

b2:
  %9 = load ptr, ptr %2, !tbaa !2
  %10 = icmp eq ptr %9, null
  %11 = sext i1 %10 to i8
  %12 = xor i8 %11, -1
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b3, label %b4

b3:
  %14 = load ptr, ptr %2, !tbaa !2
  %15 = call addrspace(1) i16 @heap.size_of(ptr %14)
  %16 = icmp uge i16 %15, %0
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b5, label %b6

b4:
  %19 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %20 = load i16, ptr %3, !tbaa !2
  %21 = icmp ult i16 %20, 16
  %22 = sext i1 %21 to i8
  %23 = icmp ne i8 %22, 0
  br i1 %23, label %b8, label %b9

b5:
  %24 = load ptr, ptr %2, !tbaa !2
  ret ptr %24

b6:
  br label %b7

b7:
  %25 = load ptr, ptr %2, !tbaa !2
  %26 = call addrspace(1) ptr @heap.next(ptr %25)
  %27 = load ptr, ptr %26
  store ptr %27, ptr %2, !tbaa !2
  br label %b2

b8:
  %28 = shl i16 2, %20
  %29 = sub i16 %28, 1
  %30 = xor i16 %29, -1
  %31 = and i16 %19, %30
  store i16 %31, ptr %1, !tbaa !2
  %32 = load i16, ptr %1, !tbaa !2
  %33 = icmp eq i16 %32, 0
  %34 = sext i1 %33 to i8
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b10, label %b11

b9:
  call addrspace(1) void @N$ESHF()
  unreachable

b10:
  ret ptr null

b11:
  br label %b12

b12:
  store i16 0, ptr %3, !tbaa !2
  br label %b13

b13:
  %36 = load i16, ptr %1, !tbaa !2
  %37 = and i16 %36, 1
  %38 = icmp eq i16 %37, 0
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b14, label %b15

b14:
  %41 = load i16, ptr %1, !tbaa !2
  %42 = lshr i16 %41, 1
  store i16 %42, ptr %1, !tbaa !2
  %43 = load i16, ptr %3, !tbaa !2
  %44 = add i16 %43, 1
  store i16 %44, ptr %3, !tbaa !2
  br label %b13

b15:
  %45 = load i16, ptr %3, !tbaa !2
  %46 = icmp ult i16 %45, 13
  %47 = sext i1 %46 to i8
  %48 = icmp ne i8 %47, 0
  br i1 %48, label %b16, label %b17

b16:
  %49 = sub i16 %45, 0
  %50 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %49
  %51 = load ptr, ptr %50, !tbaa !2
  ret ptr %51

b17:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @heap.grow(i16 %0) addrspace(1) {
b1:
  %1 = alloca ptr
  %2 = alloca ptr
  %3 = alloca ptr
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i8
  %7 = alloca i16
  %8 = alloca i16
  store ptr null, ptr %1
  store ptr null, ptr %2
  store ptr null, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i8 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  %9 = icmp ugt i16 %0, 1024
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  store i16 %0, ptr %8, !tbaa !2
  br label %b4

b3:
  store i16 1024, ptr %8, !tbaa !2
  br label %b4

b4:
  %12 = load i16, ptr %8, !tbaa !2
  store i16 %12, ptr %7, !tbaa !2
  %13 = load ptr, ptr @$var_heap.end, !tbaa !2
  %14 = icmp eq ptr %13, null
  %15 = sext i1 %14 to i8
  store i8 %15, ptr %6, !tbaa !2
  %16 = load i8, ptr %6, !tbaa !2
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %b5, label %b6

b5:
  store i16 2, ptr %5, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %5, !tbaa !2
  br label %b7

b7:
  %18 = load i16, ptr %5, !tbaa !2
  store i16 %18, ptr %4, !tbaa !2
  %19 = load i16, ptr %7, !tbaa !2
  %20 = load i16, ptr %4, !tbaa !2
  %21 = add i16 %19, %20
  %22 = call addrspace(1) ptr @N$OMEM(i16 %21)
  store ptr %22, ptr %3, !tbaa !2
  %23 = load ptr, ptr %3, !tbaa !2
  %24 = icmp eq ptr %23, null
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b8, label %b9

b8:
  store i16 %0, ptr %7, !tbaa !2
  %27 = load i16, ptr %7, !tbaa !2
  %28 = load i16, ptr %4, !tbaa !2
  %29 = add i16 %27, %28
  %30 = call addrspace(1) ptr @N$OMEM(i16 %29)
  store ptr %30, ptr %3, !tbaa !2
  %31 = load ptr, ptr %3, !tbaa !2
  %32 = icmp eq ptr %31, null
  %33 = sext i1 %32 to i8
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b11, label %b12

b9:
  br label %b10

b10:
  %35 = load i8, ptr %6, !tbaa !2
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b14, label %b15

b11:
  ret void

b12:
  br label %b13

b13:
  br label %b10

b14:
  %37 = load ptr, ptr %3, !tbaa !2
  store ptr %37, ptr %2, !tbaa !2
  br label %b16

b15:
  %38 = load ptr, ptr @$var_heap.end, !tbaa !2
  store ptr %38, ptr %2, !tbaa !2
  br label %b16

b16:
  %39 = load ptr, ptr %2, !tbaa !2
  store ptr %39, ptr %1, !tbaa !2
  %40 = load i8, ptr %6, !tbaa !2
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b17, label %b18

b17:
  %42 = load ptr, ptr %1, !tbaa !2
  %43 = call addrspace(1) ptr @heap.header(ptr %42)
  store i16 2, ptr %43
  br label %b19

b18:
  br label %b19

b19:
  %44 = load ptr, ptr %1, !tbaa !2
  %45 = load i16, ptr %7, !tbaa !2
  %46 = mul i16 %45, 1
  %47 = getelementptr i8, ptr %44, i16 %46
  store ptr %47, ptr @$var_heap.end, !tbaa !2
  %48 = load ptr, ptr @$var_heap.end, !tbaa !2
  %49 = call addrspace(1) ptr @heap.header(ptr %48)
  %50 = or i16 1, 2
  store i16 %50, ptr %49
  %51 = load ptr, ptr %1, !tbaa !2
  %52 = call addrspace(1) ptr @heap.header(ptr %51)
  %53 = load i16, ptr %7, !tbaa !2
  %54 = or i16 %53, 1
  %55 = load ptr, ptr %1, !tbaa !2
  %56 = call addrspace(1) ptr @heap.header(ptr %55)
  %57 = load i16, ptr %56
  %58 = and i16 %57, 2
  %59 = or i16 %54, %58
  store i16 %59, ptr %52
  %60 = load ptr, ptr %1, !tbaa !2
  %61 = mul i16 2, 1
  %62 = getelementptr i8, ptr %60, i16 %61
  call addrspace(1) void @heap.release(ptr %62)
  ret void
}

define internal void @heap.trim(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  store ptr null, ptr %2
  store i16 0, ptr %3
  %4 = call addrspace(1) i16 @heap.size_of(ptr %0)
  store i16 %4, ptr %3, !tbaa !2
  %5 = load i16, ptr %3, !tbaa !2
  %6 = sub i16 %5, %1
  %7 = icmp ult i16 %6, 8
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  ret void

b3:
  br label %b4

b4:
  %10 = mul i16 %1, 1
  %11 = getelementptr i8, ptr %0, i16 %10
  store ptr %11, ptr %2, !tbaa !2
  %12 = load ptr, ptr %2, !tbaa !2
  %13 = call addrspace(1) ptr @heap.header(ptr %12)
  %14 = load i16, ptr %3, !tbaa !2
  %15 = sub i16 %14, %1
  %16 = or i16 %15, 1
  %17 = or i16 %16, 2
  store i16 %17, ptr %13
  %18 = call addrspace(1) ptr @heap.header(ptr %0)
  %19 = call addrspace(1) ptr @heap.header(ptr %0)
  %20 = load i16, ptr %19
  %21 = and i16 %20, 3
  %22 = or i16 %1, %21
  store i16 %22, ptr %18
  %23 = load ptr, ptr %2, !tbaa !2
  %24 = mul i16 2, 1
  %25 = getelementptr i8, ptr %23, i16 %24
  call addrspace(1) void @heap.release(ptr %25)
  ret void
}

define internal ptr @heap.allocate(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca ptr
  %4 = alloca i16
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  store ptr null, ptr %3
  store i16 0, ptr %4
  %5 = call addrspace(1) i16 @heap.rounded(i16 %0)
  store i16 %5, ptr %4, !tbaa !2
  %6 = load i16, ptr %4, !tbaa !2
  %7 = call addrspace(1) ptr @heap.fit(i16 %6)
  store ptr %7, ptr %3, !tbaa !2
  %8 = load ptr, ptr %3, !tbaa !2
  %9 = icmp eq ptr %8, null
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  %12 = load i16, ptr %4, !tbaa !2
  call addrspace(1) void @heap.grow(i16 %12)
  %13 = load i16, ptr %4, !tbaa !2
  %14 = call addrspace(1) ptr @heap.fit(i16 %13)
  store ptr %14, ptr %3, !tbaa !2
  %15 = load ptr, ptr %3, !tbaa !2
  %16 = icmp eq ptr %15, null
  %17 = sext i1 %16 to i8
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b5, label %b6

b3:
  br label %b4

b4:
  %19 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @heap.unlink(ptr %19)
  %20 = load ptr, ptr %3, !tbaa !2
  %21 = call addrspace(1) i16 @heap.size_of(ptr %20)
  store i16 %21, ptr %1, !tbaa !2
  %22 = load ptr, ptr %3, !tbaa !2
  %23 = call addrspace(1) ptr @heap.header(ptr %22)
  %24 = load i16, ptr %1, !tbaa !2
  %25 = or i16 %24, 1
  %26 = load ptr, ptr %3, !tbaa !2
  %27 = call addrspace(1) ptr @heap.header(ptr %26)
  %28 = load i16, ptr %27
  %29 = and i16 %28, 2
  %30 = or i16 %25, %29
  store i16 %30, ptr %23
  %31 = load ptr, ptr %3, !tbaa !2
  %32 = load i16, ptr %1, !tbaa !2
  %33 = mul i16 %32, 1
  %34 = getelementptr i8, ptr %31, i16 %33
  %35 = call addrspace(1) ptr @heap.header(ptr %34)
  %36 = load i16, ptr %35
  %37 = or i16 %36, 2
  store i16 %37, ptr %35
  %38 = load ptr, ptr %3, !tbaa !2
  %39 = load i16, ptr %4, !tbaa !2
  call addrspace(1) void @heap.trim(ptr %38, i16 %39)
  %40 = load ptr, ptr %3, !tbaa !2
  %41 = mul i16 2, 1
  %42 = getelementptr i8, ptr %40, i16 %41
  ret ptr %42

b5:
  %43 = getelementptr i8, ptr @$str17, i16 6
  %44 = getelementptr i8, ptr %43, i16 -4
  %45 = load i16, ptr %44
  %46 = addrspacecast ptr %43 to ptr addrspace(1)
  store i16 %45, ptr %2, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %45, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %46, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %49)
  br label %b7

b6:
  br label %b7

b7:
  br label %b4
}

define internal void @heap.release(ptr %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  %3 = alloca i16
  %4 = alloca ptr
  store i16 0, ptr %1
  store ptr null, ptr %2
  store i16 0, ptr %3
  store ptr null, ptr %4
  %5 = icmp eq ptr %0, null
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  ret void

b3:
  br label %b4

b4:
  %8 = mul i16 -2, 1
  %9 = getelementptr i8, ptr %0, i16 %8
  store ptr %9, ptr %4, !tbaa !2
  %10 = load ptr, ptr %4, !tbaa !2
  %11 = call addrspace(1) i16 @heap.size_of(ptr %10)
  store i16 %11, ptr %3, !tbaa !2
  %12 = load ptr, ptr %4, !tbaa !2
  %13 = load i16, ptr %3, !tbaa !2
  %14 = mul i16 %13, 1
  %15 = getelementptr i8, ptr %12, i16 %14
  store ptr %15, ptr %2, !tbaa !2
  %16 = load ptr, ptr %2, !tbaa !2
  %17 = call addrspace(1) ptr @heap.header(ptr %16)
  %18 = load i16, ptr %17
  %19 = and i16 %18, 1
  %20 = icmp eq i16 %19, 0
  %21 = sext i1 %20 to i8
  %22 = icmp ne i8 %21, 0
  br i1 %22, label %b5, label %b6

b5:
  %23 = load ptr, ptr %2, !tbaa !2
  call addrspace(1) void @heap.unlink(ptr %23)
  %24 = load i16, ptr %3, !tbaa !2
  %25 = load ptr, ptr %2, !tbaa !2
  %26 = call addrspace(1) i16 @heap.size_of(ptr %25)
  %27 = add i16 %24, %26
  store i16 %27, ptr %3, !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  %28 = load ptr, ptr %4, !tbaa !2
  %29 = call addrspace(1) ptr @heap.header(ptr %28)
  %30 = load i16, ptr %29
  %31 = and i16 %30, 2
  %32 = icmp eq i16 %31, 0
  %33 = sext i1 %32 to i8
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b8, label %b9

b8:
  %35 = load ptr, ptr %4, !tbaa !2
  %36 = mul i16 -2, 1
  %37 = getelementptr i8, ptr %35, i16 %36
  %38 = call addrspace(1) ptr @heap.header(ptr %37)
  %39 = load i16, ptr %38
  store i16 %39, ptr %1, !tbaa !2
  %40 = load ptr, ptr %4, !tbaa !2
  %41 = load i16, ptr %1, !tbaa !2
  %42 = sub i16 0, %41
  %43 = mul i16 %42, 1
  %44 = getelementptr i8, ptr %40, i16 %43
  store ptr %44, ptr %4, !tbaa !2
  %45 = load ptr, ptr %4, !tbaa !2
  call addrspace(1) void @heap.unlink(ptr %45)
  %46 = load i16, ptr %3, !tbaa !2
  %47 = load i16, ptr %1, !tbaa !2
  %48 = add i16 %46, %47
  store i16 %48, ptr %3, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  %49 = load ptr, ptr %4, !tbaa !2
  %50 = load i16, ptr %3, !tbaa !2
  call addrspace(1) void @heap.insert(ptr %49, i16 %50)
  ret void
}

define internal i8 @heap.resize(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca ptr
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca ptr
  store i8 0, ptr %2
  store ptr null, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store ptr null, ptr %6
  %7 = mul i16 -2, 1
  %8 = getelementptr i8, ptr %0, i16 %7
  store ptr %8, ptr %6, !tbaa !2
  %9 = call addrspace(1) i16 @heap.rounded(i16 %1)
  store i16 %9, ptr %5, !tbaa !2
  %10 = load ptr, ptr %6, !tbaa !2
  %11 = call addrspace(1) i16 @heap.size_of(ptr %10)
  store i16 %11, ptr %4, !tbaa !2
  %12 = load i16, ptr %4, !tbaa !2
  %13 = load i16, ptr %5, !tbaa !2
  %14 = icmp ult i16 %12, %13
  %15 = sext i1 %14 to i8
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b2, label %b3

b2:
  %17 = load ptr, ptr %6, !tbaa !2
  %18 = load i16, ptr %4, !tbaa !2
  %19 = mul i16 %18, 1
  %20 = getelementptr i8, ptr %17, i16 %19
  store ptr %20, ptr %3, !tbaa !2
  %21 = load ptr, ptr %3, !tbaa !2
  %22 = call addrspace(1) ptr @heap.header(ptr %21)
  %23 = load i16, ptr %22
  %24 = and i16 %23, 1
  %25 = icmp ne i16 %24, 0
  %26 = sext i1 %25 to i8
  store i8 %26, ptr %2, !tbaa !2
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b6, label %b5

b3:
  br label %b4

b4:
  %28 = load ptr, ptr %6, !tbaa !2
  %29 = load i16, ptr %5, !tbaa !2
  call addrspace(1) void @heap.trim(ptr %28, i16 %29)
  ret i8 -1

b5:
  %30 = load i16, ptr %4, !tbaa !2
  %31 = load ptr, ptr %3, !tbaa !2
  %32 = call addrspace(1) i16 @heap.size_of(ptr %31)
  %33 = add i16 %30, %32
  %34 = load i16, ptr %5, !tbaa !2
  %35 = icmp ult i16 %33, %34
  %36 = sext i1 %35 to i8
  store i8 %36, ptr %2, !tbaa !2
  br label %b6

b6:
  %37 = load i8, ptr %2, !tbaa !2
  %38 = icmp ne i8 %37, 0
  br i1 %38, label %b7, label %b8

b7:
  ret i8 0

b8:
  br label %b9

b9:
  %39 = load ptr, ptr %3, !tbaa !2
  call addrspace(1) void @heap.unlink(ptr %39)
  %40 = load i16, ptr %4, !tbaa !2
  %41 = load ptr, ptr %3, !tbaa !2
  %42 = call addrspace(1) i16 @heap.size_of(ptr %41)
  %43 = add i16 %40, %42
  store i16 %43, ptr %4, !tbaa !2
  %44 = load ptr, ptr %6, !tbaa !2
  %45 = call addrspace(1) ptr @heap.header(ptr %44)
  %46 = load i16, ptr %4, !tbaa !2
  %47 = load ptr, ptr %6, !tbaa !2
  %48 = call addrspace(1) ptr @heap.header(ptr %47)
  %49 = load i16, ptr %48
  %50 = and i16 %49, 3
  %51 = or i16 %46, %50
  store i16 %51, ptr %45
  %52 = load ptr, ptr %6, !tbaa !2
  %53 = load i16, ptr %4, !tbaa !2
  %54 = mul i16 %53, 1
  %55 = getelementptr i8, ptr %52, i16 %54
  %56 = call addrspace(1) ptr @heap.header(ptr %55)
  %57 = load i16, ptr %56
  %58 = or i16 %57, 2
  store i16 %58, ptr %56
  br label %b4
}

define internal ptr @buffers.flags(ptr %0) addrspace(1) {
b1:
  %1 = mul i16 -6, 1
  %2 = getelementptr i8, ptr %0, i16 %1
  ret ptr %2
}

define internal ptr @buffers.length(ptr %0) addrspace(1) {
b1:
  %1 = mul i16 -4, 1
  %2 = getelementptr i8, ptr %0, i16 %1
  ret ptr %2
}

define internal ptr @buffers.capacity(ptr %0) addrspace(1) {
b1:
  %1 = mul i16 -2, 1
  %2 = getelementptr i8, ptr %0, i16 %1
  ret ptr %2
}

define internal void @buffers.copy(ptr %0, ptr addrspace(1) %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %4, !tbaa !2
  store i16 %2, ptr %3, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %4, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = icmp ult i16 %5, %6
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b3, label %b5

b3:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = mul i16 %10, 1
  %12 = getelementptr i8, ptr %0, i16 %11
  %13 = load i16, ptr %4, !tbaa !2
  %14 = mul i16 %13, 1
  %15 = getelementptr i8, ptr addrspace(1) %1, i16 %14
  %16 = load i8, ptr addrspace(1) %15
  store i8 %16, ptr %12
  br label %b4

b4:
  %17 = load i16, ptr %4, !tbaa !2
  %18 = add i16 %17, 1
  store i16 %18, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal i8 @buffers.fits(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i8
  store i8 0, ptr %2
  %3 = icmp eq i16 %1, 0
  %4 = sext i1 %3 to i8
  store i8 %4, ptr %2, !tbaa !2
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b3, label %b2

b2:
  %6 = sub i16 -16, 6
  %7 = sub i16 %6, 1
  %8 = udiv i16 %7, %1
  %9 = icmp ule i16 %0, %8
  %10 = sext i1 %9 to i8
  store i8 %10, ptr %2, !tbaa !2
  br label %b3

b3:
  %11 = load i8, ptr %2, !tbaa !2
  ret i8 %11
}

define internal ptr @buffers.allocate(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca ptr
  %3 = alloca [8 x i8]
  store ptr null, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  %4 = call addrspace(1) i8 @buffers.fits(i16 %0, i16 %1)
  %5 = xor i8 %4, -1
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  %7 = getelementptr i8, ptr @$str17, i16 6
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 %9, ptr %3, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %9, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %10, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %13)
  br label %b4

b3:
  br label %b4

b4:
  %14 = mul i16 %0, %1
  %15 = add i16 %14, 6
  %16 = add i16 %15, 1
  %17 = call addrspace(1) ptr @heap.allocate(i16 %16)
  %18 = mul i16 6, 1
  %19 = getelementptr i8, ptr %17, i16 %18
  store ptr %19, ptr %2, !tbaa !2
  %20 = load ptr, ptr %2, !tbaa !2
  %21 = call addrspace(1) ptr @buffers.flags(ptr %20)
  store i8 1, ptr %21
  %22 = load ptr, ptr %2, !tbaa !2
  %23 = call addrspace(1) ptr @buffers.flags(ptr %22)
  %24 = mul i16 1, 1
  %25 = getelementptr i8, ptr %23, i16 %24
  store i8 0, ptr %25
  %26 = load ptr, ptr %2, !tbaa !2
  %27 = call addrspace(1) ptr @buffers.length(ptr %26)
  store i16 0, ptr %27
  %28 = load ptr, ptr %2, !tbaa !2
  %29 = call addrspace(1) ptr @buffers.capacity(ptr %28)
  store i16 %0, ptr %29
  %30 = load ptr, ptr %2, !tbaa !2
  store i8 0, ptr %30
  %31 = load ptr, ptr %2, !tbaa !2
  ret ptr %31
}

define ptr @N$BRES(ptr %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca ptr
  %4 = alloca i8
  %5 = alloca i8
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i8
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i8
  %12 = alloca i8
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  store ptr null, ptr %3
  store i8 0, ptr %4
  store i8 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i8 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i8 0, ptr %11
  store i8 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  %17 = call addrspace(1) ptr @buffers.length(ptr %0)
  %18 = load i16, ptr %17
  store i16 %18, ptr %16, !tbaa !2
  %19 = call addrspace(1) ptr @buffers.capacity(ptr %0)
  %20 = load i16, ptr %19
  store i16 %20, ptr %15, !tbaa !2
  %21 = load i16, ptr %16, !tbaa !2
  %22 = icmp ult i16 %1, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b2, label %b3

b2:
  %25 = load i16, ptr %16, !tbaa !2
  store i16 %25, ptr %14, !tbaa !2
  br label %b4

b3:
  store i16 %1, ptr %14, !tbaa !2
  br label %b4

b4:
  %26 = load i16, ptr %14, !tbaa !2
  store i16 %26, ptr %13, !tbaa !2
  %27 = call addrspace(1) ptr @buffers.flags(ptr %0)
  %28 = load i8, ptr %27
  %29 = or i16 1, 8
  %30 = zext i8 %28 to i16
  %31 = and i16 %30, %29
  %32 = icmp eq i16 %31, 1
  %33 = sext i1 %32 to i8
  store i8 %33, ptr %12, !tbaa !2
  %34 = load i8, ptr %12, !tbaa !2
  store i8 %34, ptr %11, !tbaa !2
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b5, label %b6

b5:
  %36 = load i16, ptr %15, !tbaa !2
  %37 = load i16, ptr %13, !tbaa !2
  %38 = icmp uge i16 %36, %37
  %39 = sext i1 %38 to i8
  store i8 %39, ptr %11, !tbaa !2
  br label %b6

b6:
  %40 = load i8, ptr %11, !tbaa !2
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b7, label %b8

b7:
  ret ptr %0

b8:
  br label %b9

b9:
  %42 = load i16, ptr %15, !tbaa !2
  %43 = icmp ult i16 %42, -32768
  %44 = sext i1 %43 to i8
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %b10, label %b11

b10:
  %46 = load i16, ptr %15, !tbaa !2
  %47 = mul i16 %46, 2
  store i16 %47, ptr %10, !tbaa !2
  br label %b12

b11:
  %48 = load i16, ptr %15, !tbaa !2
  store i16 %48, ptr %10, !tbaa !2
  br label %b12

b12:
  %49 = load i16, ptr %10, !tbaa !2
  store i16 %49, ptr %9, !tbaa !2
  %50 = load i16, ptr %13, !tbaa !2
  %51 = load i16, ptr %9, !tbaa !2
  %52 = icmp ult i16 %50, %51
  %53 = sext i1 %52 to i8
  store i8 %53, ptr %8, !tbaa !2
  %54 = icmp ne i8 %53, 0
  br i1 %54, label %b13, label %b14

b13:
  %55 = load i16, ptr %9, !tbaa !2
  %56 = call addrspace(1) i8 @buffers.fits(i16 %55, i16 %2)
  store i8 %56, ptr %8, !tbaa !2
  br label %b14

b14:
  %57 = load i8, ptr %8, !tbaa !2
  %58 = icmp ne i8 %57, 0
  br i1 %58, label %b15, label %b16

b15:
  %59 = load i16, ptr %9, !tbaa !2
  store i16 %59, ptr %7, !tbaa !2
  br label %b17

b16:
  %60 = load i16, ptr %13, !tbaa !2
  store i16 %60, ptr %7, !tbaa !2
  br label %b17

b17:
  %61 = load i16, ptr %7, !tbaa !2
  store i16 %61, ptr %6, !tbaa !2
  %62 = load i8, ptr %12, !tbaa !2
  store i8 %62, ptr %4, !tbaa !2
  %63 = icmp ne i8 %62, 0
  br i1 %63, label %b18, label %b19

b18:
  %64 = load i16, ptr %6, !tbaa !2
  %65 = call addrspace(1) i8 @buffers.fits(i16 %64, i16 %2)
  store i8 %65, ptr %4, !tbaa !2
  br label %b19

b19:
  %66 = load i8, ptr %4, !tbaa !2
  store i8 %66, ptr %5, !tbaa !2
  %67 = icmp ne i8 %66, 0
  br i1 %67, label %b20, label %b21

b20:
  %68 = mul i16 -6, 1
  %69 = getelementptr i8, ptr %0, i16 %68
  %70 = load i16, ptr %6, !tbaa !2
  %71 = mul i16 %70, %2
  %72 = add i16 %71, 6
  %73 = add i16 %72, 1
  %74 = call addrspace(1) i8 @heap.resize(ptr %69, i16 %73)
  store i8 %74, ptr %5, !tbaa !2
  br label %b21

b21:
  %75 = load i8, ptr %5, !tbaa !2
  %76 = icmp ne i8 %75, 0
  br i1 %76, label %b22, label %b23

b22:
  %77 = call addrspace(1) ptr @buffers.capacity(ptr %0)
  %78 = load i16, ptr %6, !tbaa !2
  store i16 %78, ptr %77
  ret ptr %0

b23:
  br label %b24

b24:
  %79 = load i16, ptr %6, !tbaa !2
  %80 = call addrspace(1) ptr @buffers.allocate(i16 %79, i16 %2)
  store ptr %80, ptr %3, !tbaa !2
  %81 = load ptr, ptr %3, !tbaa !2
  %82 = addrspacecast ptr %0 to ptr addrspace(1)
  %83 = load i16, ptr %16, !tbaa !2
  %84 = mul i16 %83, %2
  %85 = add i16 %84, 1
  call addrspace(1) void @buffers.copy(ptr %81, ptr addrspace(1) %82, i16 %85)
  %86 = load ptr, ptr %3, !tbaa !2
  %87 = call addrspace(1) ptr @buffers.length(ptr %86)
  %88 = load i16, ptr %16, !tbaa !2
  store i16 %88, ptr %87
  call addrspace(1) void @N$BDRP(ptr %0)
  %89 = load ptr, ptr %3, !tbaa !2
  ret ptr %89
}

define void @N$BDRP(ptr %0) addrspace(1) {
b1:
  %1 = alloca i8
  store i8 0, ptr %1
  %2 = icmp eq ptr %0, null
  %3 = sext i1 %2 to i8
  %4 = xor i8 %3, -1
  store i8 %4, ptr %1, !tbaa !2
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  %6 = call addrspace(1) ptr @buffers.flags(ptr %0)
  %7 = load i8, ptr %6
  %8 = zext i8 %7 to i16
  %9 = and i16 %8, 1
  %10 = icmp ne i16 %9, 0
  %11 = sext i1 %10 to i8
  store i8 %11, ptr %1, !tbaa !2
  br label %b3

b3:
  %12 = load i8, ptr %1, !tbaa !2
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b4, label %b5

b4:
  %14 = mul i16 -6, 1
  %15 = getelementptr i8, ptr %0, i16 %14
  call addrspace(1) void @heap.release(ptr %15)
  br label %b6

b5:
  br label %b6

b6:
  ret void
}

define ptr @N$BGRW(ptr %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca ptr
  %4 = alloca i16
  store ptr null, ptr %3
  store i16 0, ptr %4
  %5 = call addrspace(1) ptr @buffers.length(ptr %0)
  %6 = load i16, ptr %5
  store i16 %6, ptr %4, !tbaa !2
  %7 = load i16, ptr %4, !tbaa !2
  %8 = add i16 %7, %1
  %9 = call addrspace(1) ptr @N$BRES(ptr %0, i16 %8, i16 %2)
  store ptr %9, ptr %3, !tbaa !2
  %10 = load ptr, ptr %3, !tbaa !2
  %11 = call addrspace(1) ptr @buffers.length(ptr %10)
  %12 = load i16, ptr %4, !tbaa !2
  %13 = add i16 %12, %1
  store i16 %13, ptr %11
  %14 = load ptr, ptr %3, !tbaa !2
  ret ptr %14
}

define i16 @N$BSHR(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca i16
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  store i16 0, ptr %3
  %4 = call addrspace(1) ptr @buffers.length(ptr %0)
  %5 = load i16, ptr %4
  store i16 %5, ptr %3, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = icmp ult i16 %6, %1
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  %10 = getelementptr i8, ptr @$str18, i16 6
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 %12, ptr %2, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %12, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %16)
  br label %b4

b3:
  br label %b4

b4:
  %17 = call addrspace(1) ptr @buffers.length(ptr %0)
  %18 = load i16, ptr %3, !tbaa !2
  %19 = sub i16 %18, %1
  store i16 %19, ptr %17
  %20 = load i16, ptr %3, !tbaa !2
  %21 = sub i16 %20, %1
  ret i16 %21
}

define ptr @N$BCLN(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  store ptr null, ptr %2
  store i16 0, ptr %3
  %4 = call addrspace(1) ptr @buffers.length(ptr %0)
  %5 = load i16, ptr %4
  store i16 %5, ptr %3, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = call addrspace(1) ptr @buffers.allocate(i16 %6, i16 %1)
  store ptr %7, ptr %2, !tbaa !2
  %8 = load ptr, ptr %2, !tbaa !2
  %9 = addrspacecast ptr %0 to ptr addrspace(1)
  %10 = load i16, ptr %3, !tbaa !2
  %11 = mul i16 %10, %1
  %12 = add i16 %11, 1
  call addrspace(1) void @buffers.copy(ptr %8, ptr addrspace(1) %9, i16 %12)
  %13 = load ptr, ptr %2, !tbaa !2
  %14 = call addrspace(1) ptr @buffers.length(ptr %13)
  %15 = load i16, ptr %3, !tbaa !2
  store i16 %15, ptr %14
  %16 = load ptr, ptr %2, !tbaa !2
  ret ptr %16
}

define ptr @N$DRES(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca ptr
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca ptr
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
  store ptr null, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store ptr null, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  %15 = call addrspace(1) ptr @buffers.length(ptr %0)
  %16 = load i16, ptr %15
  store i16 %16, ptr %14, !tbaa !2
  %17 = call addrspace(1) ptr @buffers.capacity(ptr %0)
  %18 = load i16, ptr %17
  store i16 %18, ptr %13, !tbaa !2
  %19 = load i16, ptr %13, !tbaa !2
  %20 = add i16 %19, 1
  %21 = mul i16 %20, 4
  %22 = load i16, ptr %14, !tbaa !2
  %23 = mul i16 %22, 3
  %24 = icmp ule i16 %21, %23
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b2, label %b3

b2:
  ret ptr %0

b3:
  br label %b4

b4:
  %27 = load i16, ptr %14, !tbaa !2
  %28 = icmp ult i16 %27, 8
  %29 = sext i1 %28 to i8
  %30 = icmp ne i8 %29, 0
  br i1 %30, label %b5, label %b6

b5:
  store i16 8, ptr %12, !tbaa !2
  br label %b7

b6:
  %31 = load i16, ptr %14, !tbaa !2
  %32 = mul i16 %31, 2
  store i16 %32, ptr %12, !tbaa !2
  br label %b7

b7:
  %33 = load i16, ptr %12, !tbaa !2
  store i16 %33, ptr %11, !tbaa !2
  %34 = load i16, ptr %11, !tbaa !2
  %35 = sub i16 %34, 1
  store i16 %35, ptr %10, !tbaa !2
  %36 = load i16, ptr %11, !tbaa !2
  %37 = call addrspace(1) ptr @buffers.allocate(i16 %36, i16 %1)
  store ptr %37, ptr %9, !tbaa !2
  %38 = load i16, ptr %11, !tbaa !2
  %39 = mul i16 %38, %1
  store i16 0, ptr %8, !tbaa !2
  store i16 %39, ptr %7, !tbaa !2
  br label %b8

b8:
  %40 = load i16, ptr %8, !tbaa !2
  %41 = load i16, ptr %7, !tbaa !2
  %42 = icmp ult i16 %40, %41
  %43 = sext i1 %42 to i8
  %44 = icmp ne i8 %43, 0
  br i1 %44, label %b9, label %b11

b9:
  %45 = load ptr, ptr %9, !tbaa !2
  %46 = load i16, ptr %8, !tbaa !2
  %47 = mul i16 %46, 1
  %48 = getelementptr i8, ptr %45, i16 %47
  store i8 0, ptr %48
  br label %b10

b10:
  %49 = load i16, ptr %8, !tbaa !2
  %50 = add i16 %49, 1
  store i16 %50, ptr %8, !tbaa !2
  br label %b8

b11:
  %51 = load i16, ptr %14, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i16 %51, ptr %5, !tbaa !2
  br label %b12

b12:
  %52 = load i16, ptr %6, !tbaa !2
  %53 = load i16, ptr %5, !tbaa !2
  %54 = icmp ult i16 %52, %53
  %55 = sext i1 %54 to i8
  %56 = icmp ne i8 %55, 0
  br i1 %56, label %b13, label %b15

b13:
  %57 = load i16, ptr %6, !tbaa !2
  %58 = mul i16 %57, %1
  %59 = mul i16 %58, 1
  %60 = getelementptr i8, ptr %0, i16 %59
  store ptr %60, ptr %4, !tbaa !2
  %61 = load ptr, ptr %4, !tbaa !2
  %62 = load i16, ptr %61
  store i16 %62, ptr %3, !tbaa !2
  %63 = load i16, ptr %3, !tbaa !2
  %64 = icmp eq i16 %63, 0
  %65 = sext i1 %64 to i8
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b16, label %b17

b14:
  %67 = load i16, ptr %6, !tbaa !2
  %68 = add i16 %67, 1
  store i16 %68, ptr %6, !tbaa !2
  br label %b12

b15:
  %69 = load ptr, ptr %9, !tbaa !2
  %70 = call addrspace(1) ptr @buffers.length(ptr %69)
  %71 = load i16, ptr %11, !tbaa !2
  store i16 %71, ptr %70
  %72 = load ptr, ptr %9, !tbaa !2
  %73 = call addrspace(1) ptr @buffers.capacity(ptr %72)
  %74 = load i16, ptr %13, !tbaa !2
  store i16 %74, ptr %73
  call addrspace(1) void @N$BDRP(ptr %0)
  %75 = load ptr, ptr %9, !tbaa !2
  ret ptr %75

b16:
  br label %b14

b17:
  br label %b18

b18:
  %76 = load i16, ptr %3, !tbaa !2
  %77 = load i16, ptr %10, !tbaa !2
  %78 = and i16 %76, %77
  store i16 %78, ptr %2, !tbaa !2
  br label %b19

b19:
  %79 = load ptr, ptr %9, !tbaa !2
  %80 = load i16, ptr %2, !tbaa !2
  %81 = mul i16 %80, %1
  %82 = mul i16 %81, 1
  %83 = getelementptr i8, ptr %79, i16 %82
  %84 = load i16, ptr %83
  %85 = icmp ne i16 %84, 0
  %86 = sext i1 %85 to i8
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b20, label %b21

b20:
  %88 = load i16, ptr %2, !tbaa !2
  %89 = add i16 %88, 1
  %90 = load i16, ptr %10, !tbaa !2
  %91 = and i16 %89, %90
  store i16 %91, ptr %2, !tbaa !2
  br label %b19

b21:
  %92 = load ptr, ptr %9, !tbaa !2
  %93 = load i16, ptr %2, !tbaa !2
  %94 = mul i16 %93, %1
  %95 = mul i16 %94, 1
  %96 = getelementptr i8, ptr %92, i16 %95
  %97 = load ptr, ptr %4, !tbaa !2
  %98 = addrspacecast ptr %97 to ptr addrspace(1)
  call addrspace(1) void @buffers.copy(ptr %96, ptr addrspace(1) %98, i16 %1)
  br label %b14
}

define internal ptr @strings.append_bytes(ptr %0, ptr addrspace(1) %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca ptr
  %4 = alloca i16
  store ptr null, ptr %3
  store i16 0, ptr %4
  %5 = call addrspace(1) ptr @buffers.length(ptr %0)
  %6 = load i16, ptr %5
  store i16 %6, ptr %4, !tbaa !2
  %7 = load i16, ptr %4, !tbaa !2
  %8 = add i16 %7, %2
  %9 = call addrspace(1) ptr @N$BRES(ptr %0, i16 %8, i16 1)
  store ptr %9, ptr %3, !tbaa !2
  %10 = load ptr, ptr %3, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = mul i16 %11, 1
  %13 = getelementptr i8, ptr %10, i16 %12
  call addrspace(1) void @buffers.copy(ptr %13, ptr addrspace(1) %1, i16 %2)
  %14 = load ptr, ptr %3, !tbaa !2
  %15 = call addrspace(1) ptr @buffers.length(ptr %14)
  %16 = load i16, ptr %4, !tbaa !2
  %17 = add i16 %16, %2
  store i16 %17, ptr %15
  %18 = load ptr, ptr %3, !tbaa !2
  %19 = load i16, ptr %4, !tbaa !2
  %20 = add i16 %19, %2
  %21 = mul i16 %20, 1
  %22 = getelementptr i8, ptr %18, i16 %21
  store i8 0, ptr %22
  %23 = load ptr, ptr %3, !tbaa !2
  ret ptr %23
}

define ptr @N$TAPP(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  %3 = call addrspace(1) ptr @buffers.length(ptr %1)
  %4 = load i16, ptr %3
  %5 = call addrspace(1) ptr @strings.append_bytes(ptr %0, ptr addrspace(1) %2, i16 %4)
  ret ptr %5
}

define ptr @N$TCAT(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  store ptr null, ptr %2
  store i16 0, ptr %3
  %4 = call addrspace(1) ptr @buffers.length(ptr %0)
  %5 = load i16, ptr %4
  store i16 %5, ptr %3, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = call addrspace(1) ptr @buffers.length(ptr %1)
  %8 = load i16, ptr %7
  %9 = add i16 %6, %8
  %10 = call addrspace(1) ptr @buffers.allocate(i16 %9, i16 1)
  store ptr %10, ptr %2, !tbaa !2
  %11 = load ptr, ptr %2, !tbaa !2
  %12 = addrspacecast ptr %0 to ptr addrspace(1)
  %13 = load i16, ptr %3, !tbaa !2
  call addrspace(1) void @buffers.copy(ptr %11, ptr addrspace(1) %12, i16 %13)
  %14 = load ptr, ptr %2, !tbaa !2
  %15 = call addrspace(1) ptr @buffers.length(ptr %14)
  %16 = load i16, ptr %3, !tbaa !2
  store i16 %16, ptr %15
  %17 = load ptr, ptr %2, !tbaa !2
  %18 = call addrspace(1) ptr @N$TAPP(ptr %17, ptr %1)
  ret ptr %18
}

define ptr @N$VCPY(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = alloca ptr
  store ptr addrspace(1) null, ptr %1
  store ptr null, ptr %2
  %3 = load i16, ptr addrspace(1) %0
  %4 = call addrspace(1) ptr @buffers.allocate(i16 %3, i16 1)
  store ptr %4, ptr %2, !tbaa !2
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  store ptr addrspace(1) %6, ptr %1, !tbaa !2
  %7 = load ptr, ptr %2, !tbaa !2
  %8 = load ptr addrspace(1), ptr %1, !tbaa !2
  %9 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @buffers.copy(ptr %7, ptr addrspace(1) %8, i16 %9)
  %10 = load ptr, ptr %2, !tbaa !2
  %11 = load i16, ptr addrspace(1) %0
  %12 = mul i16 %11, 1
  %13 = getelementptr i8, ptr %10, i16 %12
  store i8 0, ptr %13
  %14 = load ptr, ptr %2, !tbaa !2
  %15 = call addrspace(1) ptr @buffers.length(ptr %14)
  %16 = load i16, ptr addrspace(1) %0
  store i16 %16, ptr %15
  %17 = load ptr, ptr %2, !tbaa !2
  ret ptr %17
}

define i8 @N$VCMP(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca ptr addrspace(1)
  %7 = alloca ptr addrspace(1)
  %8 = alloca i16
  %9 = alloca i16
  store i8 0, ptr %2
  store i8 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store ptr addrspace(1) null, ptr %6
  store ptr addrspace(1) null, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  %10 = load i16, ptr addrspace(1) %0
  %11 = load i16, ptr addrspace(1) %1
  %12 = icmp ult i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b2, label %b3

b2:
  %15 = load i16, ptr addrspace(1) %0
  store i16 %15, ptr %9, !tbaa !2
  br label %b4

b3:
  %16 = load i16, ptr addrspace(1) %1
  store i16 %16, ptr %9, !tbaa !2
  br label %b4

b4:
  %17 = load i16, ptr %9, !tbaa !2
  store i16 %17, ptr %8, !tbaa !2
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %19 = load ptr addrspace(1), ptr addrspace(1) %18
  store ptr addrspace(1) %19, ptr %7, !tbaa !2
  %20 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %21 = load ptr addrspace(1), ptr addrspace(1) %20
  store ptr addrspace(1) %21, ptr %6, !tbaa !2
  %22 = load i16, ptr %8, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 %22, ptr %4, !tbaa !2
  br label %b5

b5:
  %23 = load i16, ptr %5, !tbaa !2
  %24 = load i16, ptr %4, !tbaa !2
  %25 = icmp ult i16 %23, %24
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b6, label %b8

b6:
  %28 = load ptr addrspace(1), ptr %7, !tbaa !2
  %29 = load i16, ptr %5, !tbaa !2
  %30 = mul i16 %29, 1
  %31 = getelementptr i8, ptr addrspace(1) %28, i16 %30
  %32 = load i8, ptr addrspace(1) %31
  %33 = load ptr addrspace(1), ptr %6, !tbaa !2
  %34 = load i16, ptr %5, !tbaa !2
  %35 = mul i16 %34, 1
  %36 = getelementptr i8, ptr addrspace(1) %33, i16 %35
  %37 = load i8, ptr addrspace(1) %36
  %38 = icmp ne i8 %32, %37
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b9, label %b10

b7:
  %41 = load i16, ptr %5, !tbaa !2
  %42 = add i16 %41, 1
  store i16 %42, ptr %5, !tbaa !2
  br label %b5

b8:
  %43 = load i16, ptr addrspace(1) %0
  %44 = load i16, ptr addrspace(1) %1
  %45 = icmp eq i16 %43, %44
  %46 = sext i1 %45 to i8
  %47 = icmp ne i8 %46, 0
  br i1 %47, label %b15, label %b16

b9:
  %48 = load ptr addrspace(1), ptr %7, !tbaa !2
  %49 = load i16, ptr %5, !tbaa !2
  %50 = mul i16 %49, 1
  %51 = getelementptr i8, ptr addrspace(1) %48, i16 %50
  %52 = load i8, ptr addrspace(1) %51
  %53 = load ptr addrspace(1), ptr %6, !tbaa !2
  %54 = load i16, ptr %5, !tbaa !2
  %55 = mul i16 %54, 1
  %56 = getelementptr i8, ptr addrspace(1) %53, i16 %55
  %57 = load i8, ptr addrspace(1) %56
  %58 = icmp ult i8 %52, %57
  %59 = sext i1 %58 to i8
  %60 = icmp ne i8 %59, 0
  br i1 %60, label %b12, label %b13

b10:
  br label %b11

b11:
  br label %b7

b12:
  store i8 -1, ptr %3, !tbaa !2
  br label %b14

b13:
  store i8 1, ptr %3, !tbaa !2
  br label %b14

b14:
  %61 = load i8, ptr %3, !tbaa !2
  ret i8 %61

b15:
  ret i8 0

b16:
  br label %b17

b17:
  %62 = load i16, ptr addrspace(1) %0
  %63 = load i16, ptr addrspace(1) %1
  %64 = icmp ult i16 %62, %63
  %65 = sext i1 %64 to i8
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b18, label %b19

b18:
  store i8 -1, ptr %2, !tbaa !2
  br label %b20

b19:
  store i8 1, ptr %2, !tbaa !2
  br label %b20

b20:
  %67 = load i8, ptr %2, !tbaa !2
  ret i8 %67
}

define internal ptr @format.scratch_at(i16 %0) addrspace(1) {
b1:
  %1 = alloca ptr
  store ptr null, ptr %1
  store ptr @$var_format.scratch, ptr %1, !tbaa !2
  %2 = load ptr, ptr %1, !tbaa !2
  %3 = mul i16 %0, 1
  %4 = getelementptr i8, ptr %2, i16 %3
  ret ptr %4
}

define internal void @format.put(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = load ptr, ptr @$var_format.sink, !tbaa !2
  %3 = icmp eq ptr %2, null
  %4 = sext i1 %3 to i8
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b2, label %b3

b2:
  call addrspace(1) void @os.write(ptr addrspace(1) %0, i16 %1)
  br label %b4

b3:
  %6 = load ptr, ptr @$var_format.sink, !tbaa !2
  %7 = call addrspace(1) ptr @strings.append_bytes(ptr %6, ptr addrspace(1) %0, i16 %1)
  store ptr %7, ptr @$var_format.sink, !tbaa !2
  br label %b4

b4:
  ret void
}

define internal void @format.put_text(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  store ptr addrspace(1) null, ptr %1
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @format.put(ptr addrspace(1) %4, i16 %5)
  ret void
}

define internal void @format.pad(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca ptr
  store i16 0, ptr %1
  store i16 0, ptr %2
  store ptr null, ptr %3
  store ptr @$var_format.fill, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 %0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %2, !tbaa !2
  %5 = load i16, ptr %1, !tbaa !2
  %6 = icmp ult i16 %4, %5
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b3, label %b5

b3:
  %9 = load ptr, ptr %3, !tbaa !2
  %10 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @format.put(ptr addrspace(1) %10, i16 1)
  br label %b4

b4:
  %11 = load i16, ptr %2, !tbaa !2
  %12 = add i16 %11, 1
  store i16 %12, ptr %2, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal i16 @format.open(i16 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  %3 = load i8, ptr @$var_format.width, !tbaa !2
  %4 = zext i8 %3 to i16
  %5 = icmp ugt i16 %4, %0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b2, label %b3

b2:
  %8 = load i8, ptr @$var_format.width, !tbaa !2
  %9 = zext i8 %8 to i16
  %10 = sub i16 %9, %0
  store i16 %10, ptr %2, !tbaa !2
  br label %b4

b3:
  store i16 0, ptr %2, !tbaa !2
  br label %b4

b4:
  %11 = load i16, ptr %2, !tbaa !2
  store i16 %11, ptr %1, !tbaa !2
  store i8 0, ptr @$var_format.width, !tbaa !2
  store i8 10, ptr @$var_format.radix, !tbaa !2
  %12 = load i8, ptr @$var_format.left, !tbaa !2
  %13 = xor i8 %12, -1
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i16, ptr %1, !tbaa !2
  call addrspace(1) void @format.pad(i16 %15)
  br label %b7

b6:
  br label %b7

b7:
  %16 = load i16, ptr %1, !tbaa !2
  ret i16 %16
}

define internal void @format.close(i16 %0) addrspace(1) {
b1:
  %1 = load i8, ptr @$var_format.left, !tbaa !2
  %2 = icmp ne i8 %1, 0
  br i1 %2, label %b2, label %b3

b2:
  call addrspace(1) void @format.pad(i16 %0)
  br label %b4

b3:
  br label %b4

b4:
  store i8 32, ptr @$var_format.fill, !tbaa !2
  store i8 0, ptr @$var_format.left, !tbaa !2
  ret void
}

define internal void @format.field(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  store i16 0, ptr %2
  %3 = call addrspace(1) i16 @format.open(i16 %1)
  store i16 %3, ptr %2, !tbaa !2
  call addrspace(1) void @format.put(ptr addrspace(1) %0, i16 %1)
  %4 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @format.close(i16 %4)
  ret void
}

define internal void @format.word(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  store ptr addrspace(1) null, ptr %1
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @format.field(ptr addrspace(1) %4, i16 %5)
  ret void
}

define internal void @format.number(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i16
  %6 = alloca ptr
  store i8 0, ptr %2
  store i8 0, ptr %3
  store i8 0, ptr %4
  store i16 0, ptr %5
  store ptr null, ptr %6
  store ptr %0, ptr %6, !tbaa !2
  store i16 %1, ptr %5, !tbaa !2
  %7 = load i8, ptr @$var_format.fill, !tbaa !2
  %8 = zext i8 %7 to i16
  %9 = icmp eq i16 %8, 48
  %10 = sext i1 %9 to i8
  store i8 %10, ptr %2, !tbaa !2
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b2, label %b3

b2:
  %12 = load i8, ptr @$var_format.left, !tbaa !2
  %13 = xor i8 %12, -1
  store i8 %13, ptr %2, !tbaa !2
  br label %b3

b3:
  %14 = load i8, ptr %2, !tbaa !2
  store i8 %14, ptr %3, !tbaa !2
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b4, label %b5

b4:
  %16 = load i16, ptr %5, !tbaa !2
  %17 = icmp ne i16 %16, 0
  %18 = sext i1 %17 to i8
  store i8 %18, ptr %3, !tbaa !2
  br label %b5

b5:
  %19 = load i8, ptr %3, !tbaa !2
  store i8 %19, ptr %4, !tbaa !2
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b6, label %b7

b6:
  %21 = load ptr, ptr %6, !tbaa !2
  %22 = load i8, ptr %21
  %23 = zext i8 %22 to i16
  %24 = icmp eq i16 %23, 45
  %25 = sext i1 %24 to i8
  store i8 %25, ptr %4, !tbaa !2
  br label %b7

b7:
  %26 = load i8, ptr %4, !tbaa !2
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b8, label %b9

b8:
  %28 = load ptr, ptr %6, !tbaa !2
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  call addrspace(1) void @format.put(ptr addrspace(1) %29, i16 1)
  %30 = load ptr, ptr %6, !tbaa !2
  %31 = mul i16 1, 1
  %32 = getelementptr i8, ptr %30, i16 %31
  store ptr %32, ptr %6, !tbaa !2
  %33 = load i16, ptr %5, !tbaa !2
  %34 = sub i16 %33, 1
  store i16 %34, ptr %5, !tbaa !2
  %35 = load i8, ptr @$var_format.width, !tbaa !2
  %36 = zext i8 %35 to i16
  %37 = icmp ne i16 %36, 0
  %38 = sext i1 %37 to i8
  %39 = icmp ne i8 %38, 0
  br i1 %39, label %b11, label %b12

b9:
  br label %b10

b10:
  %40 = load ptr, ptr %6, !tbaa !2
  %41 = addrspacecast ptr %40 to ptr addrspace(1)
  %42 = load i16, ptr %5, !tbaa !2
  call addrspace(1) void @format.field(ptr addrspace(1) %41, i16 %42)
  ret void

b11:
  %43 = load i8, ptr @$var_format.width, !tbaa !2
  %44 = zext i8 %43 to i16
  %45 = sub i16 %44, 1
  %46 = trunc i16 %45 to i8
  store i8 %46, ptr @$var_format.width, !tbaa !2
  br label %b13

b12:
  br label %b13

b13:
  br label %b10
}

define internal i16 @format.written(ptr %0, i32 %1, i8 %2) addrspace(1) {
b1:
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i16
  %6 = alloca i32
  store i8 0, ptr %3
  store i8 0, ptr %4
  store i16 0, ptr %5
  store i32 0, ptr %6
  store i32 %1, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  br label %b2

b2:
  br label %b3

b3:
  %7 = load i32, ptr %6, !tbaa !2
  %8 = zext i8 %2 to i32
  %9 = urem i32 %7, %8
  %10 = trunc i32 %9 to i8
  store i8 %10, ptr %4, !tbaa !2
  %11 = load i16, ptr %5, !tbaa !2
  %12 = add i16 %11, 1
  store i16 %12, ptr %5, !tbaa !2
  %13 = load i16, ptr %5, !tbaa !2
  %14 = sub i16 0, %13
  %15 = mul i16 %14, 1
  %16 = getelementptr i8, ptr %0, i16 %15
  %17 = load i8, ptr %4, !tbaa !2
  %18 = zext i8 %17 to i16
  %19 = icmp slt i16 %18, 10
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b5, label %b6

b5:
  %22 = load i8, ptr %4, !tbaa !2
  %23 = zext i8 %22 to i16
  %24 = add i16 48, %23
  %25 = trunc i16 %24 to i8
  store i8 %25, ptr %3, !tbaa !2
  br label %b7

b6:
  %26 = load i8, ptr %4, !tbaa !2
  %27 = zext i8 %26 to i16
  %28 = add i16 87, %27
  %29 = trunc i16 %28 to i8
  store i8 %29, ptr %3, !tbaa !2
  br label %b7

b7:
  %30 = load i8, ptr %3, !tbaa !2
  store i8 %30, ptr %16
  %31 = load i32, ptr %6, !tbaa !2
  %32 = zext i8 %2 to i32
  %33 = udiv i32 %31, %32
  store i32 %33, ptr %6, !tbaa !2
  %34 = load i32, ptr %6, !tbaa !2
  %35 = icmp eq i32 %34, 0
  %36 = sext i1 %35 to i8
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b8, label %b9

b8:
  %38 = load i16, ptr %5, !tbaa !2
  ret i16 %38

b9:
  br label %b10

b10:
  br label %b2
}

define internal void @format.integer(i32 %0, i8 %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca ptr
  store i16 0, ptr %2
  store ptr null, ptr %3
  %4 = call addrspace(1) ptr @format.scratch_at(i16 16)
  store ptr %4, ptr %3, !tbaa !2
  %5 = load ptr, ptr %3, !tbaa !2
  %6 = load i8, ptr @$var_format.radix, !tbaa !2
  %7 = call addrspace(1) i16 @format.written(ptr %5, i32 %0, i8 %6)
  store i16 %7, ptr %2, !tbaa !2
  %8 = icmp ne i8 %1, 0
  br i1 %8, label %b2, label %b3

b2:
  %9 = load i16, ptr %2, !tbaa !2
  %10 = add i16 %9, 1
  store i16 %10, ptr %2, !tbaa !2
  %11 = load ptr, ptr %3, !tbaa !2
  %12 = load i16, ptr %2, !tbaa !2
  %13 = sub i16 0, %12
  %14 = mul i16 %13, 1
  %15 = getelementptr i8, ptr %11, i16 %14
  store i8 45, ptr %15
  br label %b4

b3:
  br label %b4

b4:
  %16 = load ptr, ptr %3, !tbaa !2
  %17 = load i16, ptr %2, !tbaa !2
  %18 = sub i16 0, %17
  %19 = mul i16 %18, 1
  %20 = getelementptr i8, ptr %16, i16 %19
  %21 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @format.number(ptr %20, i16 %21)
  ret void
}

define internal void @format.signed(i32 %0) addrspace(1) {
b1:
  %1 = alloca i32
  store i32 0, ptr %1
  %2 = icmp slt i32 %0, 0
  %3 = sext i1 %2 to i8
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  %5 = sub i32 0, %0
  store i32 %5, ptr %1, !tbaa !2
  br label %b4

b3:
  store i32 %0, ptr %1, !tbaa !2
  br label %b4

b4:
  %6 = load i32, ptr %1, !tbaa !2
  %7 = icmp slt i32 %0, 0
  %8 = sext i1 %7 to i8
  call addrspace(1) void @format.integer(i32 %6, i8 %8)
  ret void
}

define void @N$PI1(i8 %0) addrspace(1) {
b1:
  %1 = sext i8 %0 to i32
  call addrspace(1) void @format.signed(i32 %1)
  ret void
}

define void @N$PU1(i8 %0) addrspace(1) {
b1:
  %1 = zext i8 %0 to i32
  call addrspace(1) void @format.integer(i32 %1, i8 0)
  ret void
}

define void @N$PI2(i16 %0) addrspace(1) {
b1:
  %1 = sext i16 %0 to i32
  call addrspace(1) void @format.signed(i32 %1)
  ret void
}

define void @N$PU2(i16 %0) addrspace(1) {
b1:
  %1 = zext i16 %0 to i32
  call addrspace(1) void @format.integer(i32 %1, i8 0)
  ret void
}

define void @N$PI4(i32 %0) addrspace(1) {
b1:
  call addrspace(1) void @format.signed(i32 %0)
  ret void
}

define void @N$PU4(i32 %0) addrspace(1) {
b1:
  call addrspace(1) void @format.integer(i32 %0, i8 0)
  ret void
}

define void @N$PQ4(i32 %0, i8 %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i32
  %7 = alloca i32
  %8 = alloca i32
  %9 = alloca i16
  %10 = alloca ptr
  %11 = alloca i32
  %12 = alloca i16
  store i8 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i32 0, ptr %6
  store i32 0, ptr %7
  store i32 0, ptr %8
  store i16 0, ptr %9
  store ptr null, ptr %10
  store i32 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %12, !tbaa !2
  store i32 %0, ptr %11, !tbaa !2
  %13 = icmp slt i32 %0, 0
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b2, label %b3

b2:
  %16 = call addrspace(1) ptr @format.scratch_at(i16 0)
  store i8 45, ptr %16
  store i16 1, ptr %12, !tbaa !2
  %17 = sub i32 0, %0
  store i32 %17, ptr %11, !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  %18 = call addrspace(1) ptr @format.scratch_at(i16 16)
  store ptr %18, ptr %10, !tbaa !2
  %19 = load ptr, ptr %10, !tbaa !2
  %20 = load i32, ptr %11, !tbaa !2
  %21 = zext i8 %1 to i16
  %22 = zext i16 %21 to i32
  %23 = lshr i32 %20, %22
  %24 = call addrspace(1) i16 @format.written(ptr %19, i32 %23, i8 10)
  store i16 %24, ptr %9, !tbaa !2
  %25 = load i16, ptr %12, !tbaa !2
  %26 = call addrspace(1) ptr @format.scratch_at(i16 %25)
  %27 = load ptr, ptr %10, !tbaa !2
  %28 = load i16, ptr %9, !tbaa !2
  %29 = sub i16 0, %28
  %30 = mul i16 %29, 1
  %31 = getelementptr i8, ptr %27, i16 %30
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  %33 = load i16, ptr %9, !tbaa !2
  call addrspace(1) void @buffers.copy(ptr %26, ptr addrspace(1) %32, i16 %33)
  %34 = load i16, ptr %12, !tbaa !2
  %35 = load i16, ptr %9, !tbaa !2
  %36 = add i16 %34, %35
  store i16 %36, ptr %12, !tbaa !2
  %37 = load i16, ptr %12, !tbaa !2
  %38 = call addrspace(1) ptr @format.scratch_at(i16 %37)
  store i8 46, ptr %38
  %39 = load i16, ptr %12, !tbaa !2
  %40 = add i16 %39, 1
  store i16 %40, ptr %12, !tbaa !2
  %41 = zext i8 %1 to i16
  %42 = zext i16 %41 to i32
  %43 = shl i32 1, %42
  store i32 %43, ptr %8, !tbaa !2
  %44 = load i32, ptr %11, !tbaa !2
  %45 = load i32, ptr %8, !tbaa !2
  %46 = sub i32 %45, 1
  %47 = and i32 %44, %46
  store i32 %47, ptr %7, !tbaa !2
  %48 = load i32, ptr %7, !tbaa !2
  %49 = icmp eq i32 %48, 0
  %50 = sext i1 %49 to i8
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %b5, label %b6

b5:
  %52 = load i16, ptr %12, !tbaa !2
  %53 = call addrspace(1) ptr @format.scratch_at(i16 %52)
  store i8 48, ptr %53
  %54 = load i16, ptr %12, !tbaa !2
  %55 = add i16 %54, 1
  store i16 %55, ptr %12, !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  %56 = load i32, ptr %8, !tbaa !2
  %57 = udiv i32 %56, 10
  store i32 %57, ptr %6, !tbaa !2
  %58 = load i32, ptr %8, !tbaa !2
  %59 = urem i32 %58, 10
  %60 = trunc i32 %59 to i16
  store i16 %60, ptr %5, !tbaa !2
  br label %b8

b8:
  %61 = load i32, ptr %7, !tbaa !2
  %62 = icmp ne i32 %61, 0
  %63 = sext i1 %62 to i8
  %64 = icmp ne i8 %63, 0
  br i1 %64, label %b9, label %b10

b9:
  store i16 0, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b11

b10:
  %65 = call addrspace(1) ptr @format.scratch_at(i16 0)
  %66 = load i16, ptr %12, !tbaa !2
  call addrspace(1) void @format.number(ptr %65, i16 %66)
  ret void

b11:
  %67 = load i16, ptr %3, !tbaa !2
  %68 = icmp ule i16 %67, 9
  %69 = sext i1 %68 to i8
  store i8 %69, ptr %2, !tbaa !2
  %70 = icmp ne i8 %69, 0
  br i1 %70, label %b14, label %b15

b12:
  %71 = load i16, ptr %3, !tbaa !2
  store i16 %71, ptr %4, !tbaa !2
  %72 = load i16, ptr %3, !tbaa !2
  %73 = add i16 %72, 1
  store i16 %73, ptr %3, !tbaa !2
  br label %b11

b13:
  %74 = load i32, ptr %7, !tbaa !2
  %75 = load i16, ptr %4, !tbaa !2
  %76 = zext i16 %75 to i32
  %77 = load i32, ptr %6, !tbaa !2
  %78 = mul i32 %76, %77
  %79 = sub i32 %74, %78
  %80 = mul i32 %79, 10
  %81 = load i16, ptr %4, !tbaa !2
  %82 = load i16, ptr %5, !tbaa !2
  %83 = mul i16 %81, %82
  %84 = zext i16 %83 to i32
  %85 = sub i32 %80, %84
  store i32 %85, ptr %7, !tbaa !2
  %86 = load i16, ptr %12, !tbaa !2
  %87 = call addrspace(1) ptr @format.scratch_at(i16 %86)
  %88 = load i16, ptr %4, !tbaa !2
  %89 = trunc i16 %88 to i8
  %90 = zext i8 %89 to i16
  %91 = add i16 48, %90
  %92 = trunc i16 %91 to i8
  store i8 %92, ptr %87
  %93 = load i16, ptr %12, !tbaa !2
  %94 = add i16 %93, 1
  store i16 %94, ptr %12, !tbaa !2
  br label %b8

b14:
  %95 = load i32, ptr %7, !tbaa !2
  %96 = load i16, ptr %3, !tbaa !2
  %97 = zext i16 %96 to i32
  %98 = load i32, ptr %6, !tbaa !2
  %99 = mul i32 %97, %98
  %100 = load i16, ptr %3, !tbaa !2
  %101 = load i16, ptr %5, !tbaa !2
  %102 = mul i16 %100, %101
  %103 = add i16 %102, 9
  %104 = udiv i16 %103, 10
  %105 = zext i16 %104 to i32
  %106 = add i32 %99, %105
  %107 = icmp uge i32 %95, %106
  %108 = sext i1 %107 to i8
  store i8 %108, ptr %2, !tbaa !2
  br label %b15

b15:
  %109 = load i8, ptr %2, !tbaa !2
  %110 = icmp ne i8 %109, 0
  br i1 %110, label %b12, label %b13
}

define void @N$PQ2(i16 %0, i8 %1) addrspace(1) {
b1:
  %2 = sext i16 %0 to i32
  call addrspace(1) void @N$PQ4(i32 %2, i8 %1)
  ret void
}

define void @N$PB(i8 %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca ptr
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  store ptr null, ptr %2
  %3 = icmp ne i8 %0, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr @$str19, i16 6
  store ptr %4, ptr %2, !tbaa !2
  br label %b4

b3:
  %5 = getelementptr i8, ptr @$str20, i16 6
  store ptr %5, ptr %2, !tbaa !2
  br label %b4

b4:
  %6 = load ptr, ptr %2, !tbaa !2
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr %1, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %8, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @format.word(ptr addrspace(1) %12)
  ret void
}

define void @N$PC(i8 %0) addrspace(1) {
b1:
  %1 = call addrspace(1) ptr @format.scratch_at(i16 0)
  store i8 %0, ptr %1
  %2 = call addrspace(1) ptr @format.scratch_at(i16 0)
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @format.field(ptr addrspace(1) %3, i16 1)
  ret void
}

define void @N$PS(ptr %0) addrspace(1) {
b1:
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = call addrspace(1) ptr @buffers.length(ptr %0)
  %3 = load i16, ptr %2
  call addrspace(1) void @format.field(ptr addrspace(1) %1, i16 %3)
  ret void
}

define void @N$PV(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  store ptr addrspace(1) null, ptr %1
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @format.field(ptr addrspace(1) %4, i16 %5)
  ret void
}

define void @N$PN() addrspace(1) {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr i8, ptr @$str11, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %3, ptr %0, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @format.put_text(ptr addrspace(1) %7)
  ret void
}

define void @N$PFLD(i8 %0, i8 %1, i8 %2, i8 %3) addrspace(1) {
b1:
  store i8 %0, ptr @$var_format.width, !tbaa !2
  store i8 %1, ptr @$var_format.radix, !tbaa !2
  store i8 %2, ptr @$var_format.fill, !tbaa !2
  %4 = zext i8 %3 to i16
  %5 = icmp ne i16 %4, 0
  %6 = sext i1 %5 to i8
  store i8 %6, ptr @$var_format.left, !tbaa !2
  ret void
}

define void @N$PBEG() addrspace(1) {
b1:
  %0 = call addrspace(1) ptr @buffers.allocate(i16 16, i16 1)
  store ptr %0, ptr @$var_format.sink, !tbaa !2
  ret void
}

define ptr @N$PEND() addrspace(1) {
b1:
  %0 = alloca ptr
  store ptr null, ptr %0
  %1 = load ptr, ptr @$var_format.sink, !tbaa !2
  store ptr %1, ptr %0, !tbaa !2
  store ptr null, ptr @$var_format.sink, !tbaa !2
  %2 = load ptr, ptr %0, !tbaa !2
  ret ptr %2
}

define internal void @floats.assign(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %3, !tbaa !2
  store i16 72, ptr %2, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %3, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp slt i16 %4, %5
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b3, label %b5

b3:
  %9 = load i16, ptr %3, !tbaa !2
  %10 = mul i16 %9, 2
  %11 = getelementptr i8, ptr %0, i16 %10
  store i16 0, ptr %11
  br label %b4

b4:
  %12 = load i16, ptr %3, !tbaa !2
  %13 = add i16 %12, 1
  store i16 %13, ptr %3, !tbaa !2
  br label %b2

b5:
  %14 = mul i16 0, 2
  %15 = getelementptr i8, ptr %0, i16 %14
  store i16 %1, ptr %15
  ret void
}

define internal void @floats.copy(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %3, !tbaa !2
  store i16 72, ptr %2, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %3, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp slt i16 %4, %5
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b3, label %b5

b3:
  %9 = load i16, ptr %3, !tbaa !2
  %10 = mul i16 %9, 2
  %11 = getelementptr i8, ptr %0, i16 %10
  %12 = load i16, ptr %3, !tbaa !2
  %13 = mul i16 %12, 2
  %14 = getelementptr i8, ptr %1, i16 %13
  %15 = load i16, ptr %14
  store i16 %15, ptr %11
  br label %b4

b4:
  %16 = load i16, ptr %3, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %3, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal void @floats.shift(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  %6 = lshr i16 %1, 4
  store i16 %6, ptr %5, !tbaa !2
  %7 = and i16 %1, 15
  store i16 %7, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = icmp ne i16 %8, 0
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b3, label %b4

b3:
  %12 = load i16, ptr %3, !tbaa !2
  %13 = sub i16 %12, 1
  store i16 %13, ptr %3, !tbaa !2
  %14 = load i16, ptr %3, !tbaa !2
  %15 = mul i16 %14, 2
  %16 = getelementptr i8, ptr %0, i16 %15
  %17 = load i16, ptr %3, !tbaa !2
  %18 = load i16, ptr %5, !tbaa !2
  %19 = icmp uge i16 %17, %18
  %20 = sext i1 %19 to i8
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b5, label %b6

b4:
  %22 = load i16, ptr %4, !tbaa !2
  %23 = icmp ne i16 %22, 0
  %24 = sext i1 %23 to i8
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b8, label %b9

b5:
  %26 = load i16, ptr %3, !tbaa !2
  %27 = load i16, ptr %5, !tbaa !2
  %28 = sub i16 %26, %27
  %29 = mul i16 %28, 2
  %30 = getelementptr i8, ptr %0, i16 %29
  %31 = load i16, ptr %30
  store i16 %31, ptr %2, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %2, !tbaa !2
  br label %b7

b7:
  %32 = load i16, ptr %2, !tbaa !2
  store i16 %32, ptr %16
  br label %b2

b8:
  store i16 72, ptr %3, !tbaa !2
  br label %b11

b9:
  br label %b10

b10:
  ret void

b11:
  %33 = load i16, ptr %3, !tbaa !2
  %34 = icmp ugt i16 %33, 1
  %35 = sext i1 %34 to i8
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b12, label %b13

b12:
  %37 = load i16, ptr %3, !tbaa !2
  %38 = sub i16 %37, 1
  store i16 %38, ptr %3, !tbaa !2
  %39 = load i16, ptr %3, !tbaa !2
  %40 = mul i16 %39, 2
  %41 = getelementptr i8, ptr %0, i16 %40
  %42 = load i16, ptr %3, !tbaa !2
  %43 = mul i16 %42, 2
  %44 = getelementptr i8, ptr %0, i16 %43
  %45 = load i16, ptr %44
  %46 = load i16, ptr %4, !tbaa !2
  %47 = shl i16 %45, %46
  %48 = load i16, ptr %3, !tbaa !2
  %49 = sub i16 %48, 1
  %50 = mul i16 %49, 2
  %51 = getelementptr i8, ptr %0, i16 %50
  %52 = load i16, ptr %51
  %53 = load i16, ptr %4, !tbaa !2
  %54 = sub i16 16, %53
  %55 = lshr i16 %52, %54
  %56 = or i16 %47, %55
  store i16 %56, ptr %41
  br label %b11

b13:
  %57 = mul i16 0, 2
  %58 = getelementptr i8, ptr %0, i16 %57
  %59 = mul i16 0, 2
  %60 = getelementptr i8, ptr %0, i16 %59
  %61 = load i16, ptr %60
  %62 = load i16, ptr %4, !tbaa !2
  %63 = shl i16 %61, %62
  store i16 %63, ptr %58
  br label %b10
}

define internal void @floats.multiply(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  store i32 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i32 0, ptr %5
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  %8 = icmp slt i16 %6, %7
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b3, label %b5

b3:
  %11 = load i16, ptr %4, !tbaa !2
  %12 = mul i16 %11, 2
  %13 = getelementptr i8, ptr %0, i16 %12
  %14 = load i16, ptr %13
  %15 = zext i16 %14 to i32
  %16 = zext i16 %1 to i32
  %17 = mul i32 %15, %16
  %18 = load i32, ptr %5, !tbaa !2
  %19 = add i32 %17, %18
  store i32 %19, ptr %2, !tbaa !2
  %20 = load i16, ptr %4, !tbaa !2
  %21 = mul i16 %20, 2
  %22 = getelementptr i8, ptr %0, i16 %21
  %23 = load i32, ptr %2, !tbaa !2
  %24 = trunc i32 %23 to i16
  store i16 %24, ptr %22
  %25 = load i32, ptr %2, !tbaa !2
  %26 = zext i16 16 to i32
  %27 = lshr i32 %25, %26
  store i32 %27, ptr %5, !tbaa !2
  br label %b4

b4:
  %28 = load i16, ptr %4, !tbaa !2
  %29 = add i16 %28, 1
  store i16 %29, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal void @floats.add(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  store i32 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i32 0, ptr %5
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  %8 = icmp slt i16 %6, %7
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b3, label %b5

b3:
  %11 = load i16, ptr %4, !tbaa !2
  %12 = mul i16 %11, 2
  %13 = getelementptr i8, ptr %0, i16 %12
  %14 = load i16, ptr %13
  %15 = zext i16 %14 to i32
  %16 = load i16, ptr %4, !tbaa !2
  %17 = mul i16 %16, 2
  %18 = getelementptr i8, ptr %1, i16 %17
  %19 = load i16, ptr %18
  %20 = zext i16 %19 to i32
  %21 = add i32 %15, %20
  %22 = load i32, ptr %5, !tbaa !2
  %23 = add i32 %21, %22
  store i32 %23, ptr %2, !tbaa !2
  %24 = load i16, ptr %4, !tbaa !2
  %25 = mul i16 %24, 2
  %26 = getelementptr i8, ptr %0, i16 %25
  %27 = load i32, ptr %2, !tbaa !2
  %28 = trunc i32 %27 to i16
  store i16 %28, ptr %26
  %29 = load i32, ptr %2, !tbaa !2
  %30 = zext i16 16 to i32
  %31 = lshr i32 %29, %30
  store i32 %31, ptr %5, !tbaa !2
  br label %b4

b4:
  %32 = load i16, ptr %4, !tbaa !2
  %33 = add i16 %32, 1
  store i16 %33, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal void @floats.subtract(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  store i32 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i32 0, ptr %5
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  %8 = icmp slt i16 %6, %7
  %9 = sext i1 %8 to i8
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b3, label %b5

b3:
  %11 = load i16, ptr %4, !tbaa !2
  %12 = mul i16 %11, 2
  %13 = getelementptr i8, ptr %0, i16 %12
  %14 = load i16, ptr %13
  %15 = zext i16 %14 to i32
  %16 = load i16, ptr %4, !tbaa !2
  %17 = mul i16 %16, 2
  %18 = getelementptr i8, ptr %1, i16 %17
  %19 = load i16, ptr %18
  %20 = zext i16 %19 to i32
  %21 = sub i32 %15, %20
  %22 = load i32, ptr %5, !tbaa !2
  %23 = sub i32 %21, %22
  store i32 %23, ptr %2, !tbaa !2
  %24 = load i16, ptr %4, !tbaa !2
  %25 = mul i16 %24, 2
  %26 = getelementptr i8, ptr %0, i16 %25
  %27 = load i32, ptr %2, !tbaa !2
  %28 = trunc i32 %27 to i16
  store i16 %28, ptr %26
  %29 = load i32, ptr %2, !tbaa !2
  %30 = zext i16 16 to i32
  %31 = lshr i32 %29, %30
  %32 = and i32 %31, 1
  store i32 %32, ptr %5, !tbaa !2
  br label %b4

b4:
  %33 = load i16, ptr %4, !tbaa !2
  %34 = add i16 %33, 1
  store i16 %34, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal i8 @floats.compare(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = alloca i8
  %3 = alloca i16
  store i8 0, ptr %2
  store i16 0, ptr %3
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %3, !tbaa !2
  %5 = icmp ne i16 %4, 0
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = sub i16 %8, 1
  store i16 %9, ptr %3, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = mul i16 %10, 2
  %12 = getelementptr i8, ptr %0, i16 %11
  %13 = load i16, ptr %12
  %14 = load i16, ptr %3, !tbaa !2
  %15 = mul i16 %14, 2
  %16 = getelementptr i8, ptr %1, i16 %15
  %17 = load i16, ptr %16
  %18 = icmp ne i16 %13, %17
  %19 = sext i1 %18 to i8
  %20 = icmp ne i8 %19, 0
  br i1 %20, label %b5, label %b6

b4:
  ret i8 0

b5:
  %21 = load i16, ptr %3, !tbaa !2
  %22 = mul i16 %21, 2
  %23 = getelementptr i8, ptr %0, i16 %22
  %24 = load i16, ptr %23
  %25 = load i16, ptr %3, !tbaa !2
  %26 = mul i16 %25, 2
  %27 = getelementptr i8, ptr %1, i16 %26
  %28 = load i16, ptr %27
  %29 = icmp ult i16 %24, %28
  %30 = sext i1 %29 to i8
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b8, label %b9

b6:
  br label %b7

b7:
  br label %b2

b8:
  store i8 -1, ptr %2, !tbaa !2
  br label %b10

b9:
  store i8 1, ptr %2, !tbaa !2
  br label %b10

b10:
  %32 = load i8, ptr %2, !tbaa !2
  ret i8 %32
}

define internal i8 @floats.beyond(ptr %0, ptr %1, i8 %2) addrspace(1) {
b1:
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i8
  store i8 0, ptr %3
  store i8 0, ptr %4
  store i8 0, ptr %5
  %6 = call addrspace(1) i8 @floats.compare(ptr %0, ptr %1)
  store i8 %6, ptr %5, !tbaa !2
  %7 = load i8, ptr %5, !tbaa !2
  %8 = sext i8 %7 to i16
  %9 = icmp sgt i16 %8, 0
  %10 = sext i1 %9 to i8
  store i8 %10, ptr %4, !tbaa !2
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b3, label %b2

b2:
  store i8 %2, ptr %3, !tbaa !2
  %12 = icmp ne i8 %2, 0
  br i1 %12, label %b4, label %b5

b3:
  %13 = load i8, ptr %4, !tbaa !2
  ret i8 %13

b4:
  %14 = load i8, ptr %5, !tbaa !2
  %15 = sext i8 %14 to i16
  %16 = icmp eq i16 %15, 0
  %17 = sext i1 %16 to i8
  store i8 %17, ptr %3, !tbaa !2
  br label %b5

b5:
  %18 = load i8, ptr %3, !tbaa !2
  store i8 %18, ptr %4, !tbaa !2
  br label %b3
}

define internal i16 @floats.put(ptr %0, i16 %1, i8 %2) addrspace(1) {
b1:
  %3 = mul i16 %1, 1
  %4 = getelementptr i8, ptr %0, i16 %3
  store i8 %2, ptr %4
  %5 = add i16 %1, 1
  ret i16 %5
}

define internal i16 @floats.put_text(ptr %0, i16 %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) {
b1:
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 %1, ptr %4, !tbaa !2
  %5 = load i16, ptr addrspace(1) %2
  store i16 0, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %3, !tbaa !2
  %7 = icmp ult i16 %6, %5
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b3, label %b5

b3:
  %10 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 %6
  %13 = load i16, ptr %4, !tbaa !2
  %14 = load i8, ptr addrspace(1) %12
  %15 = call addrspace(1) i16 @floats.put(ptr %0, i16 %13, i8 %14)
  store i16 %15, ptr %4, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %3, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %3, !tbaa !2
  br label %b2

b5:
  %18 = load i16, ptr %4, !tbaa !2
  ret i16 %18
}

define internal i32 @floats.digits(ptr %0, i16 %1, i8 %2, i8 %3, ptr %4) addrspace(1) {
b1:
  %5 = alloca i8
  %6 = alloca i8
  %7 = alloca i8
  %8 = alloca i8
  %9 = alloca i8
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca ptr
  %15 = alloca ptr
  %16 = alloca ptr
  %17 = alloca ptr
  %18 = alloca ptr
  %19 = alloca [4 x i8]
  store i8 0, ptr %5
  store i8 0, ptr %6
  store i8 0, ptr %7
  store i8 0, ptr %8
  store i8 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store ptr null, ptr %14
  store ptr null, ptr %15
  store ptr null, ptr %16
  store ptr null, ptr %17
  store ptr null, ptr %18
  call void @llvm.memset.p0.i16(ptr %19, i8 0, i16 4, i1 false)
  store ptr %0, ptr %18, !tbaa !2
  %20 = mul i16 72, 2
  %21 = getelementptr i8, ptr %0, i16 %20
  store ptr %21, ptr %17, !tbaa !2
  %22 = mul i16 144, 2
  %23 = getelementptr i8, ptr %0, i16 %22
  store ptr %23, ptr %16, !tbaa !2
  %24 = mul i16 216, 2
  %25 = getelementptr i8, ptr %0, i16 %24
  store ptr %25, ptr %15, !tbaa !2
  %26 = mul i16 288, 2
  %27 = getelementptr i8, ptr %0, i16 %26
  store ptr %27, ptr %14, !tbaa !2
  %28 = icmp ne i8 %2, 0
  br i1 %28, label %b2, label %b3

b2:
  store i16 1, ptr %13, !tbaa !2
  br label %b4

b3:
  store i16 0, ptr %13, !tbaa !2
  br label %b4

b4:
  %29 = load i16, ptr %13, !tbaa !2
  store i16 %29, ptr %12, !tbaa !2
  %30 = icmp sge i16 %1, 0
  %31 = sext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b5, label %b6

b5:
  %33 = load ptr, ptr %18, !tbaa !2
  %34 = add i16 %1, 1
  %35 = load i16, ptr %12, !tbaa !2
  %36 = add i16 %34, %35
  call addrspace(1) void @floats.shift(ptr %33, i16 %36)
  %37 = load ptr, ptr %17, !tbaa !2
  %38 = load i16, ptr %12, !tbaa !2
  %39 = icmp ult i16 %38, 16
  %40 = sext i1 %39 to i8
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b8, label %b9

b6:
  %42 = load ptr, ptr %18, !tbaa !2
  %43 = load i16, ptr %12, !tbaa !2
  %44 = add i16 1, %43
  call addrspace(1) void @floats.shift(ptr %42, i16 %44)
  %45 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @floats.assign(ptr %45, i16 1)
  %46 = load ptr, ptr %17, !tbaa !2
  %47 = sub i16 0, %1
  %48 = add i16 %47, 1
  %49 = load i16, ptr %12, !tbaa !2
  %50 = add i16 %48, %49
  call addrspace(1) void @floats.shift(ptr %46, i16 %50)
  %51 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @floats.assign(ptr %51, i16 1)
  br label %b7

b7:
  %52 = load ptr, ptr %16, !tbaa !2
  %53 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @floats.copy(ptr %52, ptr %53)
  %54 = load ptr, ptr %16, !tbaa !2
  %55 = load i16, ptr %12, !tbaa !2
  call addrspace(1) void @floats.shift(ptr %54, i16 %55)
  store i16 0, ptr %11, !tbaa !2
  br label %b10

b8:
  %56 = shl i16 2, %38
  call addrspace(1) void @floats.assign(ptr %37, i16 %56)
  %57 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @floats.assign(ptr %57, i16 1)
  %58 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @floats.shift(ptr %58, i16 %1)
  br label %b7

b9:
  call addrspace(1) void @N$ESHF()
  unreachable

b10:
  br label %b11

b11:
  %59 = load ptr, ptr %14, !tbaa !2
  %60 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @floats.copy(ptr %59, ptr %60)
  %61 = load ptr, ptr %14, !tbaa !2
  %62 = load ptr, ptr %16, !tbaa !2
  call addrspace(1) void @floats.add(ptr %61, ptr %62)
  %63 = load ptr, ptr %14, !tbaa !2
  %64 = load ptr, ptr %17, !tbaa !2
  %65 = xor i8 %3, -1
  %66 = call addrspace(1) i8 @floats.beyond(ptr %63, ptr %64, i8 %65)
  %67 = xor i8 %66, -1
  %68 = icmp ne i8 %67, 0
  br i1 %68, label %b13, label %b14

b12:
  br label %b16

b13:
  br label %b12

b14:
  br label %b15

b15:
  %69 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %69, i16 10)
  %70 = load i16, ptr %11, !tbaa !2
  %71 = add i16 %70, 1
  store i16 %71, ptr %11, !tbaa !2
  br label %b10

b16:
  br label %b17

b17:
  %72 = load ptr, ptr %14, !tbaa !2
  %73 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @floats.copy(ptr %72, ptr %73)
  %74 = load ptr, ptr %14, !tbaa !2
  %75 = load ptr, ptr %16, !tbaa !2
  call addrspace(1) void @floats.add(ptr %74, ptr %75)
  %76 = load ptr, ptr %14, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %76, i16 10)
  %77 = load ptr, ptr %14, !tbaa !2
  %78 = load ptr, ptr %17, !tbaa !2
  %79 = xor i8 %3, -1
  %80 = call addrspace(1) i8 @floats.beyond(ptr %77, ptr %78, i8 %79)
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b19, label %b20

b18:
  store i16 0, ptr %10, !tbaa !2
  br label %b22

b19:
  br label %b18

b20:
  br label %b21

b21:
  %82 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %82, i16 10)
  %83 = load ptr, ptr %16, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %83, i16 10)
  %84 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %84, i16 10)
  %85 = load i16, ptr %11, !tbaa !2
  %86 = sub i16 %85, 1
  store i16 %86, ptr %11, !tbaa !2
  br label %b16

b22:
  br label %b23

b23:
  %87 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %87, i16 10)
  %88 = load ptr, ptr %16, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %88, i16 10)
  %89 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @floats.multiply(ptr %89, i16 10)
  store i8 0, ptr %9, !tbaa !2
  br label %b25

b25:
  %90 = load ptr, ptr %18, !tbaa !2
  %91 = load ptr, ptr %17, !tbaa !2
  %92 = call addrspace(1) i8 @floats.compare(ptr %90, ptr %91)
  %93 = sext i8 %92 to i16
  %94 = icmp sge i16 %93, 0
  %95 = sext i1 %94 to i8
  %96 = icmp ne i8 %95, 0
  br i1 %96, label %b26, label %b27

b26:
  %97 = load ptr, ptr %18, !tbaa !2
  %98 = load ptr, ptr %17, !tbaa !2
  call addrspace(1) void @floats.subtract(ptr %97, ptr %98)
  %99 = load i8, ptr %9, !tbaa !2
  %100 = zext i8 %99 to i16
  %101 = add i16 %100, 1
  %102 = trunc i16 %101 to i8
  store i8 %102, ptr %9, !tbaa !2
  br label %b25

b27:
  %103 = load ptr, ptr %18, !tbaa !2
  %104 = load ptr, ptr %15, !tbaa !2
  %105 = xor i8 %3, -1
  %106 = call addrspace(1) i8 @floats.beyond(ptr %103, ptr %104, i8 %105)
  %107 = xor i8 %106, -1
  store i8 %107, ptr %8, !tbaa !2
  %108 = load ptr, ptr %14, !tbaa !2
  %109 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @floats.copy(ptr %108, ptr %109)
  %110 = load ptr, ptr %14, !tbaa !2
  %111 = load ptr, ptr %16, !tbaa !2
  call addrspace(1) void @floats.add(ptr %110, ptr %111)
  %112 = load ptr, ptr %14, !tbaa !2
  %113 = load ptr, ptr %17, !tbaa !2
  %114 = call addrspace(1) i8 @floats.beyond(ptr %112, ptr %113, i8 %3)
  store i8 %114, ptr %7, !tbaa !2
  %115 = load i8, ptr %8, !tbaa !2
  %116 = xor i8 %115, -1
  store i8 %116, ptr %6, !tbaa !2
  %117 = icmp ne i8 %116, 0
  br i1 %117, label %b28, label %b29

b28:
  %118 = load i8, ptr %7, !tbaa !2
  %119 = xor i8 %118, -1
  store i8 %119, ptr %6, !tbaa !2
  br label %b29

b29:
  %120 = load i8, ptr %6, !tbaa !2
  %121 = icmp ne i8 %120, 0
  br i1 %121, label %b30, label %b31

b30:
  %122 = load i16, ptr %10, !tbaa !2
  %123 = load i8, ptr %9, !tbaa !2
  %124 = zext i8 %123 to i16
  %125 = add i16 48, %124
  %126 = trunc i16 %125 to i8
  %127 = call addrspace(1) i16 @floats.put(ptr %4, i16 %122, i8 %126)
  store i16 %127, ptr %10, !tbaa !2
  br label %b22

b31:
  br label %b32

b32:
  %128 = load i8, ptr %8, !tbaa !2
  store i8 %128, ptr %5, !tbaa !2
  %129 = icmp ne i8 %128, 0
  br i1 %129, label %b33, label %b34

b33:
  %130 = load i8, ptr %7, !tbaa !2
  store i8 %130, ptr %5, !tbaa !2
  br label %b34

b34:
  %131 = load i8, ptr %5, !tbaa !2
  %132 = icmp ne i8 %131, 0
  br i1 %132, label %b35, label %b36

b35:
  %133 = load ptr, ptr %14, !tbaa !2
  %134 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @floats.copy(ptr %133, ptr %134)
  %135 = load ptr, ptr %14, !tbaa !2
  call addrspace(1) void @floats.shift(ptr %135, i16 1)
  %136 = load ptr, ptr %14, !tbaa !2
  %137 = load ptr, ptr %17, !tbaa !2
  %138 = call addrspace(1) i8 @floats.compare(ptr %136, ptr %137)
  %139 = sext i8 %138 to i16
  %140 = icmp sge i16 %139, 0
  %141 = sext i1 %140 to i8
  %142 = icmp ne i8 %141, 0
  br i1 %142, label %b38, label %b39

b36:
  %143 = load i8, ptr %7, !tbaa !2
  %144 = icmp ne i8 %143, 0
  br i1 %144, label %b41, label %b42

b37:
  %145 = load i16, ptr %10, !tbaa !2
  %146 = load i8, ptr %9, !tbaa !2
  %147 = zext i8 %146 to i16
  %148 = add i16 48, %147
  %149 = trunc i16 %148 to i8
  %150 = call addrspace(1) i16 @floats.put(ptr %4, i16 %145, i8 %149)
  store i16 %150, ptr %10, !tbaa !2
  %151 = load i16, ptr %10, !tbaa !2
  %152 = load i16, ptr %11, !tbaa !2
  store i16 %151, ptr %19, !tbaa !2
  %153 = getelementptr inbounds i8, ptr %19, i16 2
  store i16 %152, ptr %153, !tbaa !2
  %154 = addrspacecast ptr %19 to ptr addrspace(1)
  %155 = load i32, ptr addrspace(1) %154, !tbaa !2
  ret i32 %155

b38:
  %156 = load i8, ptr %9, !tbaa !2
  %157 = zext i8 %156 to i16
  %158 = add i16 %157, 1
  %159 = trunc i16 %158 to i8
  store i8 %159, ptr %9, !tbaa !2
  br label %b40

b39:
  br label %b40

b40:
  br label %b37

b41:
  %160 = load i8, ptr %9, !tbaa !2
  %161 = zext i8 %160 to i16
  %162 = add i16 %161, 1
  %163 = trunc i16 %162 to i8
  store i8 %163, ptr %9, !tbaa !2
  br label %b43

b42:
  br label %b43

b43:
  br label %b37
}

define internal void @floats.print_float(ptr addrspace(1) %0, i16 %1, i16 %2, i16 %3, i16 %4) addrspace(1) {
b1:
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i8
  %8 = alloca [4 x i8]
  %9 = alloca ptr
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca [8 x i8]
  %16 = alloca i8
  %17 = alloca [8 x i8]
  %18 = alloca ptr
  %19 = alloca i8
  %20 = alloca i8
  %21 = alloca i8
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i8
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca ptr
  %30 = alloca ptr
  %31 = alloca ptr
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i8 0, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 4, i1 false)
  store ptr null, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  call void @llvm.memset.p0.i16(ptr %15, i8 0, i16 8, i1 false)
  store i8 0, ptr %16
  call void @llvm.memset.p0.i16(ptr %17, i8 0, i16 8, i1 false)
  store ptr null, ptr %18
  store i8 0, ptr %19
  store i8 0, ptr %20
  store i8 0, ptr %21
  store i16 0, ptr %22
  store i16 0, ptr %23
  store i8 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store ptr null, ptr %29
  store ptr null, ptr %30
  store ptr null, ptr %31
  %32 = mul i16 5, 144
  %33 = add i16 %32, 32
  %34 = call addrspace(1) ptr @heap.allocate(i16 %33)
  store ptr %34, ptr %31, !tbaa !2
  %35 = load ptr, ptr %31, !tbaa !2
  store ptr %35, ptr %30, !tbaa !2
  %36 = load ptr, ptr %31, !tbaa !2
  %37 = mul i16 5, 144
  %38 = mul i16 %37, 1
  %39 = getelementptr i8, ptr %36, i16 %38
  store ptr %39, ptr %29, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  %40 = sub i16 %1, 1
  %41 = mul i16 %40, 2
  %42 = getelementptr i8, ptr addrspace(1) %0, i16 %41
  %43 = load i16, ptr addrspace(1) %42
  store i16 %43, ptr %27, !tbaa !2
  %44 = load i16, ptr %27, !tbaa !2
  %45 = lshr i16 %44, %2
  %46 = and i16 %45, %3
  store i16 %46, ptr %26, !tbaa !2
  %47 = load i16, ptr %27, !tbaa !2
  %48 = shl i16 1, %2
  %49 = sub i16 %48, 1
  %50 = and i16 %47, %49
  store i16 %50, ptr %25, !tbaa !2
  %51 = load i16, ptr %25, !tbaa !2
  %52 = icmp eq i16 %51, 0
  %53 = sext i1 %52 to i8
  store i8 %53, ptr %24, !tbaa !2
  %54 = sub i16 %1, 1
  store i16 0, ptr %23, !tbaa !2
  store i16 %54, ptr %22, !tbaa !2
  br label %b2

b2:
  %55 = load i16, ptr %23, !tbaa !2
  %56 = load i16, ptr %22, !tbaa !2
  %57 = icmp ult i16 %55, %56
  %58 = sext i1 %57 to i8
  %59 = icmp ne i8 %58, 0
  br i1 %59, label %b3, label %b5

b3:
  %60 = load i8, ptr %24, !tbaa !2
  store i8 %60, ptr %21, !tbaa !2
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b6, label %b7

b4:
  %62 = load i16, ptr %23, !tbaa !2
  %63 = add i16 %62, 1
  store i16 %63, ptr %23, !tbaa !2
  br label %b2

b5:
  %64 = load i16, ptr %27, !tbaa !2
  %65 = lshr i16 %64, 15
  %66 = icmp ne i16 %65, 0
  %67 = sext i1 %66 to i8
  store i8 %67, ptr %20, !tbaa !2
  %68 = icmp ne i8 %67, 0
  br i1 %68, label %b8, label %b9

b6:
  %69 = load i16, ptr %23, !tbaa !2
  %70 = mul i16 %69, 2
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 %70
  %72 = load i16, ptr addrspace(1) %71
  %73 = icmp eq i16 %72, 0
  %74 = sext i1 %73 to i8
  store i8 %74, ptr %21, !tbaa !2
  br label %b7

b7:
  %75 = load i8, ptr %21, !tbaa !2
  store i8 %75, ptr %24, !tbaa !2
  br label %b4

b8:
  %76 = load i16, ptr %26, !tbaa !2
  %77 = icmp eq i16 %76, %3
  %78 = sext i1 %77 to i8
  store i8 %78, ptr %19, !tbaa !2
  %79 = icmp ne i8 %78, 0
  br i1 %79, label %b10, label %b11

b9:
  %80 = load i8, ptr %20, !tbaa !2
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b12, label %b13

b10:
  %82 = load i8, ptr %24, !tbaa !2
  %83 = xor i8 %82, -1
  store i8 %83, ptr %19, !tbaa !2
  br label %b11

b11:
  %84 = load i8, ptr %19, !tbaa !2
  %85 = xor i8 %84, -1
  store i8 %85, ptr %20, !tbaa !2
  br label %b9

b12:
  %86 = load ptr, ptr %29, !tbaa !2
  %87 = load i16, ptr %28, !tbaa !2
  %88 = call addrspace(1) i16 @floats.put(ptr %86, i16 %87, i8 45)
  store i16 %88, ptr %28, !tbaa !2
  br label %b14

b13:
  br label %b14

b14:
  %89 = load i16, ptr %26, !tbaa !2
  %90 = icmp eq i16 %89, %3
  %91 = sext i1 %90 to i8
  %92 = icmp ne i8 %91, 0
  br i1 %92, label %b15, label %b16

b15:
  %93 = load ptr, ptr %29, !tbaa !2
  %94 = load i16, ptr %28, !tbaa !2
  %95 = load i8, ptr %24, !tbaa !2
  %96 = icmp ne i8 %95, 0
  br i1 %96, label %b18, label %b19

b16:
  %97 = load i16, ptr %26, !tbaa !2
  %98 = icmp eq i16 %97, 0
  %99 = sext i1 %98 to i8
  store i8 %99, ptr %16, !tbaa !2
  %100 = icmp ne i8 %99, 0
  br i1 %100, label %b21, label %b22

b17:
  %101 = load ptr, ptr %29, !tbaa !2
  %102 = load i16, ptr %28, !tbaa !2
  call addrspace(1) void @format.number(ptr %101, i16 %102)
  %103 = load ptr, ptr %31, !tbaa !2
  call addrspace(1) void @heap.release(ptr %103)
  ret void

b18:
  %104 = getelementptr i8, ptr @$str21, i16 6
  store ptr %104, ptr %18, !tbaa !2
  br label %b20

b19:
  %105 = getelementptr i8, ptr @$str22, i16 6
  store ptr %105, ptr %18, !tbaa !2
  br label %b20

b20:
  %106 = load ptr, ptr %18, !tbaa !2
  %107 = getelementptr i8, ptr %106, i16 -4
  %108 = load i16, ptr %107
  %109 = addrspacecast ptr %106 to ptr addrspace(1)
  store i16 %108, ptr %17, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 %108, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %17, i16 4
  store ptr addrspace(1) %109, ptr %111, !tbaa !2
  %112 = addrspacecast ptr %17 to ptr addrspace(1)
  %113 = call addrspace(1) i16 @floats.put_text(ptr %93, i16 %94, ptr addrspace(1) %112)
  store i16 %113, ptr %28, !tbaa !2
  br label %b17

b21:
  %114 = load i8, ptr %24, !tbaa !2
  store i8 %114, ptr %16, !tbaa !2
  br label %b22

b22:
  %115 = load i8, ptr %16, !tbaa !2
  %116 = icmp ne i8 %115, 0
  br i1 %116, label %b23, label %b24

b23:
  %117 = load ptr, ptr %29, !tbaa !2
  %118 = load i16, ptr %28, !tbaa !2
  %119 = getelementptr i8, ptr @$str23, i16 6
  %120 = getelementptr i8, ptr %119, i16 -4
  %121 = load i16, ptr %120
  %122 = addrspacecast ptr %119 to ptr addrspace(1)
  store i16 %121, ptr %15, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 %121, ptr %123, !tbaa !2
  %124 = getelementptr inbounds i8, ptr %15, i16 4
  store ptr addrspace(1) %122, ptr %124, !tbaa !2
  %125 = addrspacecast ptr %15 to ptr addrspace(1)
  %126 = call addrspace(1) i16 @floats.put_text(ptr %117, i16 %118, ptr addrspace(1) %125)
  store i16 %126, ptr %28, !tbaa !2
  br label %b25

b24:
  %127 = load ptr, ptr %30, !tbaa !2
  call addrspace(1) void @floats.assign(ptr %127, i16 0)
  %128 = sub i16 %1, 1
  store i16 0, ptr %14, !tbaa !2
  store i16 %128, ptr %13, !tbaa !2
  br label %b26

b25:
  br label %b17

b26:
  %129 = load i16, ptr %14, !tbaa !2
  %130 = load i16, ptr %13, !tbaa !2
  %131 = icmp ult i16 %129, %130
  %132 = sext i1 %131 to i8
  %133 = icmp ne i8 %132, 0
  br i1 %133, label %b27, label %b29

b27:
  %134 = load ptr, ptr %30, !tbaa !2
  %135 = load i16, ptr %14, !tbaa !2
  %136 = mul i16 %135, 2
  %137 = getelementptr i8, ptr %134, i16 %136
  %138 = load i16, ptr %14, !tbaa !2
  %139 = mul i16 %138, 2
  %140 = getelementptr i8, ptr addrspace(1) %0, i16 %139
  %141 = load i16, ptr addrspace(1) %140
  store i16 %141, ptr %137
  br label %b28

b28:
  %142 = load i16, ptr %14, !tbaa !2
  %143 = add i16 %142, 1
  store i16 %143, ptr %14, !tbaa !2
  br label %b26

b29:
  %144 = load ptr, ptr %30, !tbaa !2
  %145 = sub i16 %1, 1
  %146 = mul i16 %145, 2
  %147 = getelementptr i8, ptr %144, i16 %146
  %148 = load i16, ptr %25, !tbaa !2
  %149 = load i16, ptr %26, !tbaa !2
  %150 = icmp ne i16 %149, 0
  %151 = sext i1 %150 to i8
  %152 = icmp ne i8 %151, 0
  br i1 %152, label %b30, label %b31

b30:
  %153 = shl i16 1, %2
  store i16 %153, ptr %12, !tbaa !2
  br label %b32

b31:
  store i16 0, ptr %12, !tbaa !2
  br label %b32

b32:
  %154 = load i16, ptr %12, !tbaa !2
  %155 = or i16 %148, %154
  store i16 %155, ptr %147
  %156 = load i16, ptr %26, !tbaa !2
  %157 = icmp ne i16 %156, 0
  %158 = sext i1 %157 to i8
  %159 = icmp ne i8 %158, 0
  br i1 %159, label %b33, label %b34

b33:
  %160 = load i16, ptr %26, !tbaa !2
  store i16 %160, ptr %11, !tbaa !2
  br label %b35

b34:
  store i16 1, ptr %11, !tbaa !2
  br label %b35

b35:
  %161 = load i16, ptr %11, !tbaa !2
  %162 = sub i16 %161, %4
  store i16 %162, ptr %10, !tbaa !2
  %163 = load ptr, ptr %29, !tbaa !2
  %164 = mul i16 16, 1
  %165 = getelementptr i8, ptr %163, i16 %164
  store ptr %165, ptr %9, !tbaa !2
  %166 = load ptr, ptr %30, !tbaa !2
  %167 = load i16, ptr %10, !tbaa !2
  %168 = load i16, ptr %26, !tbaa !2
  %169 = icmp ugt i16 %168, 1
  %170 = sext i1 %169 to i8
  store i8 %170, ptr %7, !tbaa !2
  %171 = icmp ne i8 %170, 0
  br i1 %171, label %b36, label %b37

b36:
  %172 = load i8, ptr %24, !tbaa !2
  store i8 %172, ptr %7, !tbaa !2
  br label %b37

b37:
  %173 = load i8, ptr %7, !tbaa !2
  %174 = mul i16 0, 2
  %175 = getelementptr i8, ptr addrspace(1) %0, i16 %174
  %176 = load i16, ptr addrspace(1) %175
  %177 = and i16 %176, 1
  %178 = icmp eq i16 %177, 0
  %179 = sext i1 %178 to i8
  %180 = load ptr, ptr %9, !tbaa !2
  %181 = call addrspace(1) i32 @floats.digits(ptr %166, i16 %167, i8 %173, i8 %179, ptr %180)
  %182 = addrspacecast ptr %8 to ptr addrspace(1)
  store i32 %181, ptr addrspace(1) %182, !tbaa !2
  %183 = load i16, ptr %8, !tbaa !2
  %184 = getelementptr inbounds i8, ptr %8, i16 2
  %185 = load i16, ptr %184, !tbaa !2
  store i16 %183, ptr %6, !tbaa !2
  store i16 %185, ptr %5, !tbaa !2
  %186 = load ptr, ptr %29, !tbaa !2
  %187 = load i16, ptr %28, !tbaa !2
  %188 = load ptr, ptr %9, !tbaa !2
  %189 = load i16, ptr %6, !tbaa !2
  %190 = load i16, ptr %5, !tbaa !2
  %191 = sub i16 %190, 1
  %192 = call addrspace(1) i16 @floats.decimal(ptr %186, i16 %187, ptr %188, i16 %189, i16 %191)
  store i16 %192, ptr %28, !tbaa !2
  br label %b25
}

define internal i16 @floats.decimal(ptr %0, i16 %1, ptr %2, i16 %3, i16 %4) addrspace(1) {
b1:
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i8
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i8
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca [8 x i8]
  %21 = alloca i8
  %22 = alloca i16
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i8 0, ptr %7
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i8 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  call void @llvm.memset.p0.i16(ptr %20, i8 0, i16 8, i1 false)
  store i8 0, ptr %21
  store i16 0, ptr %22
  store i16 %1, ptr %22, !tbaa !2
  %23 = icmp sge i16 %4, -4
  %24 = sext i1 %23 to i8
  store i8 %24, ptr %21, !tbaa !2
  %25 = icmp ne i8 %24, 0
  br i1 %25, label %b2, label %b3

b2:
  %26 = icmp slt i16 %4, 16
  %27 = sext i1 %26 to i8
  store i8 %27, ptr %21, !tbaa !2
  br label %b3

b3:
  %28 = load i8, ptr %21, !tbaa !2
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b4, label %b5

b4:
  %30 = icmp slt i16 %4, 0
  %31 = sext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b7, label %b8

b5:
  br label %b6

b6:
  %33 = load i16, ptr %22, !tbaa !2
  %34 = mul i16 0, 1
  %35 = getelementptr i8, ptr %2, i16 %34
  %36 = load i8, ptr %35
  %37 = call addrspace(1) i16 @floats.put(ptr %0, i16 %33, i8 %36)
  store i16 %37, ptr %22, !tbaa !2
  %38 = icmp ugt i16 %3, 1
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b32, label %b33

b7:
  %41 = load i16, ptr %22, !tbaa !2
  %42 = getelementptr i8, ptr @$str24, i16 6
  %43 = getelementptr i8, ptr %42, i16 -4
  %44 = load i16, ptr %43
  %45 = addrspacecast ptr %42 to ptr addrspace(1)
  store i16 %44, ptr %20, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %20, i16 2
  store i16 %44, ptr %46, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %20, i16 4
  store ptr addrspace(1) %45, ptr %47, !tbaa !2
  %48 = addrspacecast ptr %20 to ptr addrspace(1)
  %49 = call addrspace(1) i16 @floats.put_text(ptr %0, i16 %41, ptr addrspace(1) %48)
  store i16 %49, ptr %22, !tbaa !2
  %50 = sub i16 0, %4
  %51 = sub i16 %50, 1
  store i16 0, ptr %19, !tbaa !2
  store i16 %51, ptr %18, !tbaa !2
  br label %b10

b8:
  br label %b9

b9:
  %52 = add i16 %4, 1
  store i16 %52, ptr %15, !tbaa !2
  %53 = load i16, ptr %15, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  store i16 %53, ptr %13, !tbaa !2
  br label %b18

b10:
  %54 = load i16, ptr %19, !tbaa !2
  %55 = load i16, ptr %18, !tbaa !2
  %56 = icmp slt i16 %54, %55
  %57 = sext i1 %56 to i8
  %58 = icmp ne i8 %57, 0
  br i1 %58, label %b11, label %b13

b11:
  %59 = load i16, ptr %22, !tbaa !2
  %60 = call addrspace(1) i16 @floats.put(ptr %0, i16 %59, i8 48)
  store i16 %60, ptr %22, !tbaa !2
  br label %b12

b12:
  %61 = load i16, ptr %19, !tbaa !2
  %62 = add i16 %61, 1
  store i16 %62, ptr %19, !tbaa !2
  br label %b10

b13:
  store i16 0, ptr %17, !tbaa !2
  store i16 %3, ptr %16, !tbaa !2
  br label %b14

b14:
  %63 = load i16, ptr %17, !tbaa !2
  %64 = load i16, ptr %16, !tbaa !2
  %65 = icmp ult i16 %63, %64
  %66 = sext i1 %65 to i8
  %67 = icmp ne i8 %66, 0
  br i1 %67, label %b15, label %b17

b15:
  %68 = load i16, ptr %22, !tbaa !2
  %69 = load i16, ptr %17, !tbaa !2
  %70 = mul i16 %69, 1
  %71 = getelementptr i8, ptr %2, i16 %70
  %72 = load i8, ptr %71
  %73 = call addrspace(1) i16 @floats.put(ptr %0, i16 %68, i8 %72)
  store i16 %73, ptr %22, !tbaa !2
  br label %b16

b16:
  %74 = load i16, ptr %17, !tbaa !2
  %75 = add i16 %74, 1
  store i16 %75, ptr %17, !tbaa !2
  br label %b14

b17:
  %76 = load i16, ptr %22, !tbaa !2
  ret i16 %76

b18:
  %77 = load i16, ptr %14, !tbaa !2
  %78 = load i16, ptr %13, !tbaa !2
  %79 = icmp ult i16 %77, %78
  %80 = sext i1 %79 to i8
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b19, label %b21

b19:
  %82 = load i16, ptr %22, !tbaa !2
  %83 = load i16, ptr %14, !tbaa !2
  %84 = icmp ult i16 %83, %3
  %85 = sext i1 %84 to i8
  %86 = icmp ne i8 %85, 0
  br i1 %86, label %b22, label %b23

b20:
  %87 = load i16, ptr %14, !tbaa !2
  %88 = add i16 %87, 1
  store i16 %88, ptr %14, !tbaa !2
  br label %b18

b21:
  %89 = load i16, ptr %22, !tbaa !2
  %90 = call addrspace(1) i16 @floats.put(ptr %0, i16 %89, i8 46)
  store i16 %90, ptr %22, !tbaa !2
  %91 = load i16, ptr %15, !tbaa !2
  %92 = icmp ule i16 %3, %91
  %93 = sext i1 %92 to i8
  %94 = icmp ne i8 %93, 0
  br i1 %94, label %b25, label %b26

b22:
  %95 = load i16, ptr %14, !tbaa !2
  %96 = mul i16 %95, 1
  %97 = getelementptr i8, ptr %2, i16 %96
  %98 = load i8, ptr %97
  store i8 %98, ptr %12, !tbaa !2
  br label %b24

b23:
  store i8 48, ptr %12, !tbaa !2
  br label %b24

b24:
  %99 = load i8, ptr %12, !tbaa !2
  %100 = call addrspace(1) i16 @floats.put(ptr %0, i16 %82, i8 %99)
  store i16 %100, ptr %22, !tbaa !2
  br label %b20

b25:
  %101 = load i16, ptr %22, !tbaa !2
  %102 = call addrspace(1) i16 @floats.put(ptr %0, i16 %101, i8 48)
  ret i16 %102

b26:
  br label %b27

b27:
  %103 = load i16, ptr %15, !tbaa !2
  store i16 %103, ptr %11, !tbaa !2
  store i16 %3, ptr %10, !tbaa !2
  br label %b28

b28:
  %104 = load i16, ptr %11, !tbaa !2
  %105 = load i16, ptr %10, !tbaa !2
  %106 = icmp ult i16 %104, %105
  %107 = sext i1 %106 to i8
  %108 = icmp ne i8 %107, 0
  br i1 %108, label %b29, label %b31

b29:
  %109 = load i16, ptr %22, !tbaa !2
  %110 = load i16, ptr %11, !tbaa !2
  %111 = mul i16 %110, 1
  %112 = getelementptr i8, ptr %2, i16 %111
  %113 = load i8, ptr %112
  %114 = call addrspace(1) i16 @floats.put(ptr %0, i16 %109, i8 %113)
  store i16 %114, ptr %22, !tbaa !2
  br label %b30

b30:
  %115 = load i16, ptr %11, !tbaa !2
  %116 = add i16 %115, 1
  store i16 %116, ptr %11, !tbaa !2
  br label %b28

b31:
  %117 = load i16, ptr %22, !tbaa !2
  ret i16 %117

b32:
  %118 = load i16, ptr %22, !tbaa !2
  %119 = call addrspace(1) i16 @floats.put(ptr %0, i16 %118, i8 46)
  store i16 %119, ptr %22, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  store i16 %3, ptr %8, !tbaa !2
  br label %b35

b33:
  br label %b34

b34:
  %120 = load i16, ptr %22, !tbaa !2
  %121 = call addrspace(1) i16 @floats.put(ptr %0, i16 %120, i8 101)
  store i16 %121, ptr %22, !tbaa !2
  %122 = load i16, ptr %22, !tbaa !2
  %123 = icmp slt i16 %4, 0
  %124 = sext i1 %123 to i8
  %125 = icmp ne i8 %124, 0
  br i1 %125, label %b39, label %b40

b35:
  %126 = load i16, ptr %9, !tbaa !2
  %127 = load i16, ptr %8, !tbaa !2
  %128 = icmp ult i16 %126, %127
  %129 = sext i1 %128 to i8
  %130 = icmp ne i8 %129, 0
  br i1 %130, label %b36, label %b38

b36:
  %131 = load i16, ptr %22, !tbaa !2
  %132 = load i16, ptr %9, !tbaa !2
  %133 = mul i16 %132, 1
  %134 = getelementptr i8, ptr %2, i16 %133
  %135 = load i8, ptr %134
  %136 = call addrspace(1) i16 @floats.put(ptr %0, i16 %131, i8 %135)
  store i16 %136, ptr %22, !tbaa !2
  br label %b37

b37:
  %137 = load i16, ptr %9, !tbaa !2
  %138 = add i16 %137, 1
  store i16 %138, ptr %9, !tbaa !2
  br label %b35

b38:
  br label %b34

b39:
  store i8 45, ptr %7, !tbaa !2
  br label %b41

b40:
  store i8 43, ptr %7, !tbaa !2
  br label %b41

b41:
  %139 = load i8, ptr %7, !tbaa !2
  %140 = call addrspace(1) i16 @floats.put(ptr %0, i16 %122, i8 %139)
  store i16 %140, ptr %22, !tbaa !2
  %141 = icmp slt i16 %4, 0
  %142 = sext i1 %141 to i8
  %143 = icmp ne i8 %142, 0
  br i1 %143, label %b42, label %b43

b42:
  %144 = sub i16 0, %4
  store i16 %144, ptr %6, !tbaa !2
  br label %b44

b43:
  store i16 %4, ptr %6, !tbaa !2
  br label %b44

b44:
  %145 = load i16, ptr %6, !tbaa !2
  store i16 %145, ptr %5, !tbaa !2
  %146 = load i16, ptr %5, !tbaa !2
  %147 = icmp uge i16 %146, 100
  %148 = sext i1 %147 to i8
  %149 = icmp ne i8 %148, 0
  br i1 %149, label %b45, label %b46

b45:
  %150 = load i16, ptr %22, !tbaa !2
  %151 = load i16, ptr %5, !tbaa !2
  %152 = udiv i16 %151, 100
  %153 = trunc i16 %152 to i8
  %154 = zext i8 %153 to i16
  %155 = add i16 48, %154
  %156 = trunc i16 %155 to i8
  %157 = call addrspace(1) i16 @floats.put(ptr %0, i16 %150, i8 %156)
  store i16 %157, ptr %22, !tbaa !2
  br label %b47

b46:
  br label %b47

b47:
  %158 = load i16, ptr %22, !tbaa !2
  %159 = load i16, ptr %5, !tbaa !2
  %160 = udiv i16 %159, 10
  %161 = urem i16 %160, 10
  %162 = trunc i16 %161 to i8
  %163 = zext i8 %162 to i16
  %164 = add i16 48, %163
  %165 = trunc i16 %164 to i8
  %166 = call addrspace(1) i16 @floats.put(ptr %0, i16 %158, i8 %165)
  store i16 %166, ptr %22, !tbaa !2
  %167 = load i16, ptr %22, !tbaa !2
  %168 = load i16, ptr %5, !tbaa !2
  %169 = urem i16 %168, 10
  %170 = trunc i16 %169 to i8
  %171 = zext i8 %170 to i16
  %172 = add i16 48, %171
  %173 = trunc i16 %172 to i8
  %174 = call addrspace(1) i16 @floats.put(ptr %0, i16 %167, i8 %173)
  ret i16 %174
}

define void @N$PR8(double %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = alloca double
  store ptr addrspace(1) null, ptr %1
  store double 0.000000e+00, ptr %2
  store double %0, ptr %2, !tbaa !2
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  call addrspace(1) void @floats.print_float(ptr addrspace(1) %4, i16 4, i16 4, i16 2047, i16 1075)
  ret void
}

define void @N$PR4(float %0) addrspace(1) {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = alloca float
  store ptr addrspace(1) null, ptr %1
  store float 0.000000e+00, ptr %2
  store float %0, ptr %2, !tbaa !2
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  call addrspace(1) void @floats.print_float(ptr addrspace(1) %4, i16 2, i16 7, i16 255, i16 150)
  ret void
}

declare i16 @N$OWRI(i16, ptr addrspace(1), i16) addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$OEXT(i8) addrspace(1)

declare ptr @N$OMEM(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
