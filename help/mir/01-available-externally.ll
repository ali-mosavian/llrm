target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$var_heap.heads = internal global [26 x i8] zeroinitializer, align 2
@$var_heap.occupied = internal global [2 x i8] zeroinitializer, align 2
@$var_heap.end = internal global [2 x i8] zeroinitializer, align 2
@$var_format.width = internal global [1 x i8] zeroinitializer
@$var_format.radix = internal global [1 x i8] c"\0A"
@$var_format.fill = internal global [1 x i8] c" "
@$var_format.left = internal global [1 x i8] zeroinitializer
@$var_format.sink = internal global [2 x i8] zeroinitializer, align 2
@$var_format.scratch = internal global [48 x i8] zeroinitializer
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00panic: \00"
@$str11 = internal constant [9 x i8] c"\08\00\02\00\02\00\0D\0A\00"
@$str12 = internal constant [35 x i8] c"\08\00\1C\00\1C\00division by zero or overflow\00"
@$str13 = internal constant [21 x i8] c"\08\00\0E\00\0E\00stack overflow\00"
@$str14 = internal constant [37 x i8] c"\08\00\1E\00\1E\00float outside the integer type\00"
@$str15 = internal constant [31 x i8] c"\08\00\18\00\18\00shift count out of range\00"
@$str16 = internal constant [26 x i8] c"\08\00\13\00\13\00index out of bounds\00"
@$str17 = internal constant [20 x i8] c"\08\00\0D\00\0D\00key not found\00"
@$str18 = internal constant [20 x i8] c"\08\00\0D\00\0D\00out of memory\00"
@$str19 = internal constant [28 x i8] c"\08\00\15\00\15\00pop from an empty vec\00"
@$str20 = internal constant [11 x i8] c"\08\00\04\00\04\00true\00"
@$str21 = internal constant [12 x i8] c"\08\00\05\00\05\00false\00"
@$str22 = internal constant [10 x i8] c"\08\00\03\00\03\00inf\00"
@$str23 = internal constant [10 x i8] c"\08\00\03\00\03\00nan\00"
@$str24 = internal constant [10 x i8] c"\08\00\03\00\03\000.0\00"
@$str25 = internal constant [9 x i8] c"\08\00\02\00\02\000.\00"

define internal cc 1005 void @os.write(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = call addrspace(1) i32 @LL$WRITE_FILE(i16 1, ptr addrspace(1) %0, i16 %1)
  ret void
}

define internal cc 1005 ptr addrspace(1) @os.text_screen() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca ptr addrspace(1)
  %1 = inttoptr i32 -1207959552 to ptr addrspace(1)
  store ptr addrspace(1) %1, ptr %0, !tbaa !2
  %2 = load ptr addrspace(1), ptr %0, !tbaa !2
  ret ptr addrspace(1) %2
}

define internal cc 1005 void @errors.panic(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
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
  call cc 1005 addrspace(1) void @errors.say(ptr addrspace(1) %9)
  call cc 1005 addrspace(1) void @errors.say(ptr addrspace(1) %0)
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
  call cc 1005 addrspace(1) void @errors.say(ptr addrspace(1) %16)
  call addrspace(1) void @LL$EXIT(i8 zeroext -1)
  ret void
}

define internal cc 1005 void @errors.say(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @os.write(ptr addrspace(1) %4, i16 %5)
  ret void
}

define cc 1005 void @N$EDIV() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
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
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define cc 1005 void @N$ESTK() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
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
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define cc 1005 void @N$ECNV() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
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
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define cc 1005 void @N$ESHF() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
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
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define cc 1005 void @N$EBND() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
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
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define cc 1005 void @N$EKEY() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = getelementptr i8, ptr @$str17, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  store i16 %3, ptr %0, !tbaa !2
  %5 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %3, ptr %5, !tbaa !2
  %6 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %4, ptr %6, !tbaa !2
  %7 = addrspacecast ptr %0 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %7)
  ret void
}

define internal cc 1005 ptr @heap.header(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  ret ptr %0
}

define internal cc 1005 i16 @heap.block_size(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = call cc 1005 addrspace(1) ptr @heap.header(ptr %0)
  %2 = load i16, ptr %1
  %3 = and i16 %2, -4
  ret i16 %3
}

define internal cc 1005 ptr @heap.next(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  ret ptr %1
}

define internal cc 1005 ptr @heap.previous(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr %0, i16 4
  ret ptr %1
}

define internal cc 1005 i16 @heap.class_of(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %2, !tbaa !2
  %3 = lshr i16 %0, 4
  store i16 %3, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr %2, !tbaa !2
  %7 = add i16 %6, 1
  store i16 %7, ptr %2, !tbaa !2
  %8 = load i16, ptr %1, !tbaa !2
  %9 = lshr i16 %8, 1
  store i16 %9, ptr %1, !tbaa !2
  br label %b2

b4:
  %10 = load i16, ptr %2, !tbaa !2
  ret i16 %10
}

define internal cc 1005 i16 @heap.rounded(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [8 x i8]
  %4 = icmp ugt i16 %0, -16
  br i1 %4, label %b2, label %b3

b2:
  %5 = getelementptr i8, ptr @$str18, i16 6
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 %7, ptr %3, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %7, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %3 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %11)
  br label %b4

b3:
  br label %b4

b4:
  %12 = add i16 %0, 2
  %13 = add i16 %12, 3
  %14 = and i16 %13, -4
  store i16 %14, ptr %2, !tbaa !2
  %15 = load i16, ptr %2, !tbaa !2
  %16 = icmp ugt i16 %15, 8
  br i1 %16, label %b5, label %b6

b5:
  %17 = load i16, ptr %2, !tbaa !2
  store i16 %17, ptr %1, !tbaa !2
  br label %b7

b6:
  store i16 8, ptr %1, !tbaa !2
  br label %b7

b7:
  %18 = load i16, ptr %1, !tbaa !2
  ret i16 %18
}

define internal cc 1005 void @heap.unlink(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  %3 = alloca ptr
  %4 = call cc 1005 addrspace(1) ptr @heap.next(ptr %0)
  %5 = load ptr, ptr %4
  store ptr %5, ptr %3, !tbaa !2
  %6 = call cc 1005 addrspace(1) ptr @heap.previous(ptr %0)
  %7 = load ptr, ptr %6
  store ptr %7, ptr %2, !tbaa !2
  %8 = load ptr, ptr %2, !tbaa !2
  %9 = icmp eq ptr %8, null
  br i1 %9, label %b2, label %b3

b2:
  %10 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %0)
  %11 = call cc 1005 addrspace(1) i16 @heap.class_of(i16 %10)
  store i16 %11, ptr %1, !tbaa !2
  %12 = load i16, ptr %1, !tbaa !2
  %13 = load ptr, ptr %3, !tbaa !2
  %14 = sub i16 %12, 0
  %15 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %14
  store ptr %13, ptr %15, !tbaa !2
  %16 = load ptr, ptr %3, !tbaa !2
  %17 = icmp eq ptr %16, null
  br i1 %17, label %b5, label %b6

b3:
  %18 = load ptr, ptr %2, !tbaa !2
  %19 = call cc 1005 addrspace(1) ptr @heap.next(ptr %18)
  %20 = load ptr, ptr %3, !tbaa !2
  store ptr %20, ptr %19
  br label %b4

b4:
  %21 = load ptr, ptr %3, !tbaa !2
  %22 = icmp eq ptr %21, null
  br i1 %22, label %b9, label %b8

b5:
  %23 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %24 = load i16, ptr %1, !tbaa !2
  %25 = shl i16 1, %24
  %26 = xor i16 %25, -1
  %27 = and i16 %23, %26
  store i16 %27, ptr @$var_heap.occupied, !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  br label %b4

b8:
  %28 = load ptr, ptr %3, !tbaa !2
  %29 = call cc 1005 addrspace(1) ptr @heap.previous(ptr %28)
  %30 = load ptr, ptr %2, !tbaa !2
  store ptr %30, ptr %29
  br label %b10

b9:
  br label %b10

b10:
  ret void
}

define internal cc 1005 void @heap.insert(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  %4 = call cc 1005 addrspace(1) ptr @heap.header(ptr %0)
  %5 = call cc 1005 addrspace(1) ptr @heap.header(ptr %0)
  %6 = load i16, ptr %5
  %7 = and i16 %6, 2
  %8 = or i16 %1, %7
  store i16 %8, ptr %4
  %9 = sub i16 %1, 2
  %10 = mul i16 %9, 1
  %11 = getelementptr i8, ptr %0, i16 %10
  %12 = call cc 1005 addrspace(1) ptr @heap.header(ptr %11)
  store i16 %1, ptr %12
  %13 = mul i16 %1, 1
  %14 = getelementptr i8, ptr %0, i16 %13
  %15 = call cc 1005 addrspace(1) ptr @heap.header(ptr %14)
  %16 = load i16, ptr %15
  %17 = and i16 %16, -3
  store i16 %17, ptr %15
  %18 = call cc 1005 addrspace(1) i16 @heap.class_of(i16 %1)
  store i16 %18, ptr %3, !tbaa !2
  %19 = load i16, ptr %3, !tbaa !2
  %20 = sub i16 %19, 0
  %21 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %20
  %22 = load ptr, ptr %21, !tbaa !2
  store ptr %22, ptr %2, !tbaa !2
  %23 = call cc 1005 addrspace(1) ptr @heap.next(ptr %0)
  %24 = load ptr, ptr %2, !tbaa !2
  store ptr %24, ptr %23
  %25 = call cc 1005 addrspace(1) ptr @heap.previous(ptr %0)
  store ptr null, ptr %25
  %26 = load ptr, ptr %2, !tbaa !2
  %27 = icmp eq ptr %26, null
  br i1 %27, label %b3, label %b2

b2:
  %28 = load ptr, ptr %2, !tbaa !2
  %29 = call cc 1005 addrspace(1) ptr @heap.previous(ptr %28)
  store ptr %0, ptr %29
  br label %b4

b3:
  br label %b4

b4:
  %30 = load i16, ptr %3, !tbaa !2
  %31 = sub i16 %30, 0
  %32 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %31
  store ptr %0, ptr %32, !tbaa !2
  %33 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %34 = load i16, ptr %3, !tbaa !2
  %35 = shl i16 1, %34
  %36 = or i16 %33, %35
  store i16 %36, ptr @$var_heap.occupied, !tbaa !2
  ret void
}

define internal cc 1005 ptr @heap.fit(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  %3 = alloca i16
  %4 = call cc 1005 addrspace(1) i16 @heap.class_of(i16 %0)
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
  br i1 %10, label %b4, label %b3

b3:
  %11 = load ptr, ptr %2, !tbaa !2
  %12 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %11)
  %13 = icmp uge i16 %12, %0
  br i1 %13, label %b5, label %b6

b4:
  %14 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %15 = load i16, ptr %3, !tbaa !2
  %16 = icmp ult i16 %15, 16
  br i1 %16, label %b8, label %b9

b5:
  %17 = load ptr, ptr %2, !tbaa !2
  ret ptr %17

b6:
  br label %b7

b7:
  %18 = load ptr, ptr %2, !tbaa !2
  %19 = call cc 1005 addrspace(1) ptr @heap.next(ptr %18)
  %20 = load ptr, ptr %19
  store ptr %20, ptr %2, !tbaa !2
  br label %b2

b8:
  %21 = shl i16 2, %15
  %22 = sub i16 %21, 1
  %23 = xor i16 %22, -1
  %24 = and i16 %14, %23
  store i16 %24, ptr %1, !tbaa !2
  %25 = load i16, ptr %1, !tbaa !2
  %26 = icmp eq i16 %25, 0
  br i1 %26, label %b10, label %b11

b9:
  call cc 1005 addrspace(1) void @N$ESHF()
  unreachable

b10:
  ret ptr null

b11:
  br label %b12

b12:
  store i16 0, ptr %3, !tbaa !2
  br label %b13

b13:
  %27 = load i16, ptr %1, !tbaa !2
  %28 = and i16 %27, 1
  %29 = icmp eq i16 %28, 0
  br i1 %29, label %b14, label %b15

b14:
  %30 = load i16, ptr %1, !tbaa !2
  %31 = lshr i16 %30, 1
  store i16 %31, ptr %1, !tbaa !2
  %32 = load i16, ptr %3, !tbaa !2
  %33 = add i16 %32, 1
  store i16 %33, ptr %3, !tbaa !2
  br label %b13

b15:
  %34 = load i16, ptr %3, !tbaa !2
  %35 = icmp ult i16 %34, 13
  br i1 %35, label %b16, label %b17

b16:
  %36 = sub i16 %34, 0
  %37 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %36
  %38 = load ptr, ptr %37, !tbaa !2
  ret ptr %38

b17:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @heap.grow(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  %3 = alloca ptr
  %4 = alloca i8
  %5 = alloca i8
  %6 = alloca i8
  %7 = alloca ptr
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i8
  %11 = alloca i16
  %12 = alloca i16
  %13 = add i16 %0, 4
  %14 = icmp ugt i16 %13, 1024
  br i1 %14, label %b2, label %b3

b2:
  %15 = add i16 %0, 4
  store i16 %15, ptr %12, !tbaa !2
  br label %b4

b3:
  store i16 1024, ptr %12, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %12, !tbaa !2
  store i16 %16, ptr %11, !tbaa !2
  %17 = load ptr, ptr @$var_heap.end, !tbaa !2
  %18 = icmp eq ptr %17, null
  %19 = zext i1 %18 to i8
  store i8 %19, ptr %10, !tbaa !2
  %20 = load i8, ptr %10, !tbaa !2, !range !5
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b5, label %b6

b5:
  store i16 2, ptr %9, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %9, !tbaa !2
  br label %b7

b7:
  %22 = load i16, ptr %9, !tbaa !2
  store i16 %22, ptr %8, !tbaa !2
  %23 = load i16, ptr %11, !tbaa !2
  %24 = load i16, ptr %8, !tbaa !2
  %25 = add i16 %23, %24
  %26 = call addrspace(1) ptr @LL$MORE(i16 %25)
  store ptr %26, ptr %7, !tbaa !2
  %27 = load ptr, ptr %7, !tbaa !2
  %28 = icmp eq ptr %27, null
  br i1 %28, label %b8, label %b9

b8:
  %29 = add i16 %0, 4
  store i16 %29, ptr %11, !tbaa !2
  %30 = load i16, ptr %11, !tbaa !2
  %31 = load i16, ptr %8, !tbaa !2
  %32 = add i16 %30, %31
  %33 = call addrspace(1) ptr @LL$MORE(i16 %32)
  store ptr %33, ptr %7, !tbaa !2
  %34 = load ptr, ptr %7, !tbaa !2
  %35 = icmp eq ptr %34, null
  br i1 %35, label %b11, label %b12

b9:
  br label %b10

b10:
  %36 = load i8, ptr %10, !tbaa !2, !range !6
  %37 = xor i8 %36, 1
  store i8 %37, ptr %6, !tbaa !2
  %38 = icmp ne i8 %37, 0
  br i1 %38, label %b14, label %b15

b11:
  ret void

b12:
  br label %b13

b13:
  br label %b10

b14:
  %39 = load ptr, ptr %7, !tbaa !2
  %40 = load ptr, ptr @$var_heap.end, !tbaa !2
  %41 = getelementptr i8, ptr %40, i16 2
  %42 = icmp ne ptr %39, %41
  %43 = zext i1 %42 to i8
  store i8 %43, ptr %6, !tbaa !2
  br label %b15

b15:
  %44 = load i8, ptr %6, !tbaa !2, !range !7
  store i8 %44, ptr %5, !tbaa !2
  %45 = load i8, ptr %10, !tbaa !2, !range !8
  store i8 %45, ptr %4, !tbaa !2
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b17, label %b16

b16:
  %47 = load i8, ptr %5, !tbaa !2, !range !9
  store i8 %47, ptr %4, !tbaa !2
  br label %b17

b17:
  %48 = load i8, ptr %4, !tbaa !2, !range !10
  %49 = icmp ne i8 %48, 0
  br i1 %49, label %b18, label %b19

b18:
  %50 = load ptr, ptr %7, !tbaa !2
  store ptr %50, ptr %3, !tbaa !2
  br label %b20

b19:
  %51 = load ptr, ptr @$var_heap.end, !tbaa !2
  store ptr %51, ptr %3, !tbaa !2
  br label %b20

b20:
  %52 = load ptr, ptr %3, !tbaa !2
  store ptr %52, ptr %2, !tbaa !2
  %53 = load i16, ptr %11, !tbaa !2
  store i16 %53, ptr %1, !tbaa !2
  %54 = load i8, ptr %10, !tbaa !2, !range !11
  %55 = icmp ne i8 %54, 0
  br i1 %55, label %b21, label %b24

b21:
  %56 = load ptr, ptr %2, !tbaa !2
  %57 = call cc 1005 addrspace(1) ptr @heap.header(ptr %56)
  store i16 2, ptr %57
  br label %b23

b22:
  br label %b23

b23:
  %58 = load i8, ptr %5, !tbaa !2, !range !12
  %59 = icmp ne i8 %58, 0
  br i1 %59, label %b25, label %b26

b24:
  %60 = load i8, ptr %5, !tbaa !2, !range !13
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b21, label %b22

b25:
  %62 = load i16, ptr %11, !tbaa !2
  %63 = sub i16 %62, 4
  store i16 %63, ptr %1, !tbaa !2
  br label %b27

b26:
  br label %b27

b27:
  %64 = load ptr, ptr %2, !tbaa !2
  %65 = load i16, ptr %1, !tbaa !2
  %66 = mul i16 %65, 1
  %67 = getelementptr i8, ptr %64, i16 %66
  store ptr %67, ptr @$var_heap.end, !tbaa !2
  %68 = load ptr, ptr @$var_heap.end, !tbaa !2
  %69 = call cc 1005 addrspace(1) ptr @heap.header(ptr %68)
  store i16 3, ptr %69
  %70 = load ptr, ptr %2, !tbaa !2
  %71 = call cc 1005 addrspace(1) ptr @heap.header(ptr %70)
  %72 = load i16, ptr %1, !tbaa !2
  %73 = or i16 %72, 1
  %74 = load ptr, ptr %2, !tbaa !2
  %75 = call cc 1005 addrspace(1) ptr @heap.header(ptr %74)
  %76 = load i16, ptr %75
  %77 = and i16 %76, 2
  %78 = or i16 %73, %77
  store i16 %78, ptr %71
  %79 = load ptr, ptr %2, !tbaa !2
  %80 = getelementptr i8, ptr %79, i16 2
  call cc 1005 addrspace(1) void @heap.release(ptr %80)
  ret void
}

define internal cc 1005 void @heap.trim(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  %4 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %0)
  store i16 %4, ptr %3, !tbaa !2
  %5 = load i16, ptr %3, !tbaa !2
  %6 = sub i16 %5, %1
  %7 = icmp ult i16 %6, 8
  br i1 %7, label %b2, label %b3

b2:
  ret void

b3:
  br label %b4

b4:
  %8 = mul i16 %1, 1
  %9 = getelementptr i8, ptr %0, i16 %8
  store ptr %9, ptr %2, !tbaa !2
  %10 = load ptr, ptr %2, !tbaa !2
  %11 = call cc 1005 addrspace(1) ptr @heap.header(ptr %10)
  %12 = load i16, ptr %3, !tbaa !2
  %13 = sub i16 %12, %1
  %14 = or i16 %13, 1
  %15 = or i16 %14, 2
  store i16 %15, ptr %11
  %16 = call cc 1005 addrspace(1) ptr @heap.header(ptr %0)
  %17 = call cc 1005 addrspace(1) ptr @heap.header(ptr %0)
  %18 = load i16, ptr %17
  %19 = and i16 %18, 3
  %20 = or i16 %1, %19
  store i16 %20, ptr %16
  %21 = load ptr, ptr %2, !tbaa !2
  %22 = getelementptr i8, ptr %21, i16 2
  call cc 1005 addrspace(1) void @heap.release(ptr %22)
  ret void
}

define internal cc 1005 ptr @heap.allocate(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca ptr
  %4 = alloca i16
  %5 = call cc 1005 addrspace(1) i16 @heap.rounded(i16 %0)
  store i16 %5, ptr %4, !tbaa !2
  %6 = load i16, ptr %4, !tbaa !2
  %7 = call cc 1005 addrspace(1) ptr @heap.fit(i16 %6)
  store ptr %7, ptr %3, !tbaa !2
  %8 = load ptr, ptr %3, !tbaa !2
  %9 = icmp eq ptr %8, null
  br i1 %9, label %b2, label %b3

b2:
  %10 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @heap.grow(i16 %10)
  %11 = load i16, ptr %4, !tbaa !2
  %12 = call cc 1005 addrspace(1) ptr @heap.fit(i16 %11)
  store ptr %12, ptr %3, !tbaa !2
  %13 = load ptr, ptr %3, !tbaa !2
  %14 = icmp eq ptr %13, null
  br i1 %14, label %b5, label %b6

b3:
  br label %b4

b4:
  %15 = load ptr, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @heap.unlink(ptr %15)
  %16 = load ptr, ptr %3, !tbaa !2
  %17 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %16)
  store i16 %17, ptr %1, !tbaa !2
  %18 = load ptr, ptr %3, !tbaa !2
  %19 = call cc 1005 addrspace(1) ptr @heap.header(ptr %18)
  %20 = load i16, ptr %1, !tbaa !2
  %21 = or i16 %20, 1
  %22 = load ptr, ptr %3, !tbaa !2
  %23 = call cc 1005 addrspace(1) ptr @heap.header(ptr %22)
  %24 = load i16, ptr %23
  %25 = and i16 %24, 2
  %26 = or i16 %21, %25
  store i16 %26, ptr %19
  %27 = load ptr, ptr %3, !tbaa !2
  %28 = load i16, ptr %1, !tbaa !2
  %29 = mul i16 %28, 1
  %30 = getelementptr i8, ptr %27, i16 %29
  %31 = call cc 1005 addrspace(1) ptr @heap.header(ptr %30)
  %32 = load i16, ptr %31
  %33 = or i16 %32, 2
  store i16 %33, ptr %31
  %34 = load ptr, ptr %3, !tbaa !2
  %35 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @heap.trim(ptr %34, i16 %35)
  %36 = load ptr, ptr %3, !tbaa !2
  %37 = getelementptr i8, ptr %36, i16 2
  ret ptr %37

b5:
  %38 = getelementptr i8, ptr @$str18, i16 6
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %2, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %40, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %41, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %44)
  br label %b7

b6:
  br label %b7

b7:
  br label %b4
}

define internal cc 1005 void @heap.release(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca ptr
  %3 = alloca i16
  %4 = alloca ptr
  %5 = icmp eq ptr %0, null
  br i1 %5, label %b2, label %b3

b2:
  ret void

b3:
  br label %b4

b4:
  %6 = getelementptr i8, ptr %0, i16 -2
  store ptr %6, ptr %4, !tbaa !2
  %7 = load ptr, ptr %4, !tbaa !2
  %8 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %7)
  store i16 %8, ptr %3, !tbaa !2
  %9 = load ptr, ptr %4, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = mul i16 %10, 1
  %12 = getelementptr i8, ptr %9, i16 %11
  store ptr %12, ptr %2, !tbaa !2
  %13 = load ptr, ptr %2, !tbaa !2
  %14 = call cc 1005 addrspace(1) ptr @heap.header(ptr %13)
  %15 = load i16, ptr %14
  %16 = and i16 %15, 1
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %b5, label %b6

b5:
  %18 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @heap.unlink(ptr %18)
  %19 = load i16, ptr %3, !tbaa !2
  %20 = load ptr, ptr %2, !tbaa !2
  %21 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %20)
  %22 = add i16 %19, %21
  store i16 %22, ptr %3, !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  %23 = load ptr, ptr %4, !tbaa !2
  %24 = call cc 1005 addrspace(1) ptr @heap.header(ptr %23)
  %25 = load i16, ptr %24
  %26 = and i16 %25, 2
  %27 = icmp eq i16 %26, 0
  br i1 %27, label %b8, label %b9

b8:
  %28 = load ptr, ptr %4, !tbaa !2
  %29 = getelementptr i8, ptr %28, i16 -2
  %30 = call cc 1005 addrspace(1) ptr @heap.header(ptr %29)
  %31 = load i16, ptr %30
  store i16 %31, ptr %1, !tbaa !2
  %32 = load ptr, ptr %4, !tbaa !2
  %33 = load i16, ptr %1, !tbaa !2
  %34 = sub i16 0, %33
  %35 = mul i16 %34, 1
  %36 = getelementptr i8, ptr %32, i16 %35
  store ptr %36, ptr %4, !tbaa !2
  %37 = load ptr, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @heap.unlink(ptr %37)
  %38 = load i16, ptr %3, !tbaa !2
  %39 = load i16, ptr %1, !tbaa !2
  %40 = add i16 %38, %39
  store i16 %40, ptr %3, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  %41 = load ptr, ptr %4, !tbaa !2
  %42 = load i16, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @heap.insert(ptr %41, i16 %42)
  ret void
}

define internal cc 1005 i8 @heap.resize(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca ptr
  %6 = getelementptr i8, ptr %0, i16 -2
  store ptr %6, ptr %5, !tbaa !2
  %7 = call cc 1005 addrspace(1) i16 @heap.rounded(i16 %1)
  store i16 %7, ptr %4, !tbaa !2
  %8 = load ptr, ptr %5, !tbaa !2
  %9 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %8)
  store i16 %9, ptr %3, !tbaa !2
  %10 = load i16, ptr %3, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = icmp ult i16 %10, %11
  br i1 %12, label %b2, label %b3

b2:
  %13 = load ptr, ptr %5, !tbaa !2
  %14 = load i16, ptr %3, !tbaa !2
  %15 = mul i16 %14, 1
  %16 = getelementptr i8, ptr %13, i16 %15
  store ptr %16, ptr %2, !tbaa !2
  %17 = load ptr, ptr %2, !tbaa !2
  %18 = call cc 1005 addrspace(1) ptr @heap.header(ptr %17)
  %19 = load i16, ptr %18
  %20 = and i16 %19, 1
  %21 = icmp ne i16 %20, 0
  br i1 %21, label %b5, label %b8

b3:
  br label %b4

b4:
  %22 = load ptr, ptr %5, !tbaa !2
  %23 = load i16, ptr %4, !tbaa !2
  call cc 1005 addrspace(1) void @heap.trim(ptr %22, i16 %23)
  ret i8 1

b5:
  ret i8 0

b6:
  br label %b7

b7:
  %24 = load ptr, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @heap.unlink(ptr %24)
  %25 = load i16, ptr %3, !tbaa !2
  %26 = load ptr, ptr %2, !tbaa !2
  %27 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %26)
  %28 = add i16 %25, %27
  store i16 %28, ptr %3, !tbaa !2
  %29 = load ptr, ptr %5, !tbaa !2
  %30 = call cc 1005 addrspace(1) ptr @heap.header(ptr %29)
  %31 = load i16, ptr %3, !tbaa !2
  %32 = load ptr, ptr %5, !tbaa !2
  %33 = call cc 1005 addrspace(1) ptr @heap.header(ptr %32)
  %34 = load i16, ptr %33
  %35 = and i16 %34, 3
  %36 = or i16 %31, %35
  store i16 %36, ptr %30
  %37 = load ptr, ptr %5, !tbaa !2
  %38 = load i16, ptr %3, !tbaa !2
  %39 = mul i16 %38, 1
  %40 = getelementptr i8, ptr %37, i16 %39
  %41 = call cc 1005 addrspace(1) ptr @heap.header(ptr %40)
  %42 = load i16, ptr %41
  %43 = or i16 %42, 2
  store i16 %43, ptr %41
  br label %b4

b8:
  %44 = load i16, ptr %3, !tbaa !2
  %45 = load ptr, ptr %2, !tbaa !2
  %46 = call cc 1005 addrspace(1) i16 @heap.block_size(ptr %45)
  %47 = add i16 %44, %46
  %48 = load i16, ptr %4, !tbaa !2
  %49 = icmp ult i16 %47, %48
  br i1 %49, label %b5, label %b6
}

define internal cc 1005 ptr @buffers.flags(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr %0, i16 -6
  ret ptr %1
}

define internal cc 1005 ptr @buffers.length(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr %0, i16 -4
  ret ptr %1
}

define internal cc 1005 ptr @buffers.capacity(ptr %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = getelementptr i8, ptr %0, i16 -2
  ret ptr %1
}

define internal cc 1005 void @buffers.copy(ptr %0, ptr addrspace(1) %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %4, !tbaa !2
  store i16 %2, ptr %3, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %4, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = icmp ult i16 %5, %6
  br i1 %7, label %b3, label %b5

b3:
  %8 = load i16, ptr %4, !tbaa !2
  %9 = mul i16 %8, 1
  %10 = getelementptr i8, ptr %0, i16 %9
  %11 = load i16, ptr %4, !tbaa !2
  %12 = mul i16 %11, 1
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 %12
  %14 = load i8, ptr addrspace(1) %13
  store i8 %14, ptr %10
  br label %b4

b4:
  %15 = load i16, ptr %4, !tbaa !2
  %16 = add nuw i16 %15, 1
  store i16 %16, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 i8 @buffers.fits(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = icmp eq i16 %1, 0
  %4 = zext i1 %3 to i8
  store i8 %4, ptr %2, !tbaa !2
  br i1 %3, label %b3, label %b2

b2:
  %5 = udiv i16 -23, %1
  %6 = icmp ule i16 %0, %5
  %7 = zext i1 %6 to i8
  store i8 %7, ptr %2, !tbaa !2
  br label %b3

b3:
  %8 = load i8, ptr %2, !tbaa !2, !range !14
  ret i8 %8
}

define internal cc 1005 ptr @buffers.allocate(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  %3 = alloca [8 x i8]
  %4 = call cc 1005 addrspace(1) i8 @buffers.fits(i16 %0, i16 %1)
  %5 = icmp ne i8 %4, 0
  br i1 %5, label %b3, label %b2

b2:
  %6 = getelementptr i8, ptr @$str18, i16 6
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr %3, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %8, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %3 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %12)
  br label %b4

b3:
  br label %b4

b4:
  %13 = mul i16 %0, %1
  %14 = add i16 %13, 6
  %15 = add i16 %14, 1
  %16 = call cc 1005 addrspace(1) ptr @heap.allocate(i16 %15)
  %17 = getelementptr i8, ptr %16, i16 6
  store ptr %17, ptr %2, !tbaa !2
  %18 = load ptr, ptr %2, !tbaa !2
  %19 = call cc 1005 addrspace(1) ptr @buffers.flags(ptr %18)
  store i8 1, ptr %19
  %20 = load ptr, ptr %2, !tbaa !2
  %21 = call cc 1005 addrspace(1) ptr @buffers.flags(ptr %20)
  %22 = getelementptr i8, ptr %21, i16 1
  store i8 0, ptr %22
  %23 = load ptr, ptr %2, !tbaa !2
  %24 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %23)
  store i16 0, ptr %24
  %25 = load ptr, ptr %2, !tbaa !2
  %26 = call cc 1005 addrspace(1) ptr @buffers.capacity(ptr %25)
  store i16 %0, ptr %26
  %27 = load ptr, ptr %2, !tbaa !2
  store i8 0, ptr %27
  %28 = load ptr, ptr %2, !tbaa !2
  ret ptr %28
}

define cc 1005 ptr @N$BRES(ptr %0, i16 %1, i16 %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca ptr
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i8
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i8
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %15 = load i16, ptr %14
  store i16 %15, ptr %13, !tbaa !2
  %16 = call cc 1005 addrspace(1) ptr @buffers.capacity(ptr %0)
  %17 = load i16, ptr %16
  store i16 %17, ptr %12, !tbaa !2
  %18 = load i16, ptr %13, !tbaa !2
  %19 = icmp ult i16 %1, %18
  br i1 %19, label %b2, label %b3

b2:
  %20 = load i16, ptr %13, !tbaa !2
  store i16 %20, ptr %11, !tbaa !2
  br label %b4

b3:
  store i16 %1, ptr %11, !tbaa !2
  br label %b4

b4:
  %21 = load i16, ptr %11, !tbaa !2
  store i16 %21, ptr %10, !tbaa !2
  %22 = call cc 1005 addrspace(1) ptr @buffers.flags(ptr %0)
  %23 = load i8, ptr %22
  %24 = zext i8 %23 to i16
  %25 = and i16 %24, 9
  %26 = icmp eq i16 %25, 1
  %27 = zext i1 %26 to i8
  store i8 %27, ptr %9, !tbaa !2
  %28 = load i8, ptr %9, !tbaa !2, !range !15
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b8, label %b6

b5:
  ret ptr %0

b6:
  br label %b7

b7:
  %30 = load i16, ptr %12, !tbaa !2
  %31 = icmp ult i16 %30, -32768
  br i1 %31, label %b9, label %b10

b8:
  %32 = load i16, ptr %12, !tbaa !2
  %33 = load i16, ptr %10, !tbaa !2
  %34 = icmp uge i16 %32, %33
  br i1 %34, label %b5, label %b6

b9:
  %35 = load i16, ptr %12, !tbaa !2
  %36 = mul i16 %35, 2
  store i16 %36, ptr %8, !tbaa !2
  br label %b11

b10:
  %37 = load i16, ptr %12, !tbaa !2
  store i16 %37, ptr %8, !tbaa !2
  br label %b11

b11:
  %38 = load i16, ptr %8, !tbaa !2
  store i16 %38, ptr %7, !tbaa !2
  %39 = load i16, ptr %10, !tbaa !2
  %40 = load i16, ptr %7, !tbaa !2
  %41 = icmp ult i16 %39, %40
  %42 = zext i1 %41 to i8
  store i8 %42, ptr %6, !tbaa !2
  br i1 %41, label %b12, label %b13

b12:
  %43 = load i16, ptr %7, !tbaa !2
  %44 = call cc 1005 addrspace(1) i8 @buffers.fits(i16 %43, i16 %2)
  store i8 %44, ptr %6, !tbaa !2
  br label %b13

b13:
  %45 = load i8, ptr %6, !tbaa !2, !range !16
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b14, label %b15

b14:
  %47 = load i16, ptr %7, !tbaa !2
  store i16 %47, ptr %5, !tbaa !2
  br label %b16

b15:
  %48 = load i16, ptr %10, !tbaa !2
  store i16 %48, ptr %5, !tbaa !2
  br label %b16

b16:
  %49 = load i16, ptr %5, !tbaa !2
  store i16 %49, ptr %4, !tbaa !2
  %50 = load i8, ptr %9, !tbaa !2, !range !17
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %b21, label %b18

b17:
  %52 = call cc 1005 addrspace(1) ptr @buffers.capacity(ptr %0)
  %53 = load i16, ptr %4, !tbaa !2
  store i16 %53, ptr %52
  ret ptr %0

b18:
  br label %b19

b19:
  %54 = load i16, ptr %4, !tbaa !2
  %55 = call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %54, i16 %2)
  store ptr %55, ptr %3, !tbaa !2
  %56 = load ptr, ptr %3, !tbaa !2
  %57 = addrspacecast ptr %0 to ptr addrspace(1)
  %58 = load i16, ptr %13, !tbaa !2
  %59 = mul i16 %58, %2
  %60 = add i16 %59, 1
  call cc 1005 addrspace(1) void @buffers.copy(ptr %56, ptr addrspace(1) %57, i16 %60)
  %61 = load ptr, ptr %3, !tbaa !2
  %62 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %61)
  %63 = load i16, ptr %13, !tbaa !2
  store i16 %63, ptr %62
  call cc 1005 addrspace(1) void @N$BDRP(ptr %0)
  %64 = load ptr, ptr %3, !tbaa !2
  ret ptr %64

b20:
  %65 = getelementptr i8, ptr %0, i16 -6
  %66 = load i16, ptr %4, !tbaa !2
  %67 = mul i16 %66, %2
  %68 = add i16 %67, 6
  %69 = add i16 %68, 1
  %70 = call cc 1005 addrspace(1) i8 @heap.resize(ptr %65, i16 %69)
  %71 = icmp ne i8 %70, 0
  br i1 %71, label %b17, label %b18

b21:
  %72 = load i16, ptr %4, !tbaa !2
  %73 = call cc 1005 addrspace(1) i8 @buffers.fits(i16 %72, i16 %2)
  %74 = icmp ne i8 %73, 0
  br i1 %74, label %b20, label %b18
}

define cc 1005 void @N$BDRP(ptr %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = icmp eq ptr %0, null
  br i1 %1, label %b3, label %b5

b2:
  %2 = getelementptr i8, ptr %0, i16 -6
  call cc 1005 addrspace(1) void @heap.release(ptr %2)
  br label %b4

b3:
  br label %b4

b4:
  ret void

b5:
  %3 = call cc 1005 addrspace(1) ptr @buffers.flags(ptr %0)
  %4 = load i8, ptr %3
  %5 = zext i8 %4 to i16
  %6 = and i16 %5, 1
  %7 = icmp ne i16 %6, 0
  br i1 %7, label %b2, label %b3
}

define cc 1005 ptr @N$BGRW(ptr %0, i16 %1, i16 %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca ptr
  %4 = alloca i16
  %5 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %6 = load i16, ptr %5
  store i16 %6, ptr %4, !tbaa !2
  %7 = load i16, ptr %4, !tbaa !2
  %8 = add i16 %7, %1
  %9 = call cc 1005 addrspace(1) ptr @N$BRES(ptr %0, i16 %8, i16 %2)
  store ptr %9, ptr %3, !tbaa !2
  %10 = load ptr, ptr %3, !tbaa !2
  %11 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %10)
  %12 = load i16, ptr %4, !tbaa !2
  %13 = add i16 %12, %1
  store i16 %13, ptr %11
  %14 = load ptr, ptr %3, !tbaa !2
  ret ptr %14
}

define cc 1005 i16 @N$BSHR(ptr %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %5 = load i16, ptr %4
  store i16 %5, ptr %3, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = icmp ult i16 %6, %1
  br i1 %7, label %b2, label %b3

b2:
  %8 = getelementptr i8, ptr @$str19, i16 6
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 %10, ptr %2, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %10, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %11, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @errors.panic(ptr addrspace(1) %14)
  br label %b4

b3:
  br label %b4

b4:
  %15 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %16 = load i16, ptr %3, !tbaa !2
  %17 = sub i16 %16, %1
  store i16 %17, ptr %15
  %18 = load i16, ptr %3, !tbaa !2
  %19 = sub i16 %18, %1
  ret i16 %19
}

define cc 1005 ptr @N$BCLN(ptr %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  %4 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %5 = load i16, ptr %4
  store i16 %5, ptr %3, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %6, i16 %1)
  store ptr %7, ptr %2, !tbaa !2
  %8 = load ptr, ptr %2, !tbaa !2
  %9 = addrspacecast ptr %0 to ptr addrspace(1)
  %10 = load i16, ptr %3, !tbaa !2
  %11 = mul i16 %10, %1
  %12 = add i16 %11, 1
  call cc 1005 addrspace(1) void @buffers.copy(ptr %8, ptr addrspace(1) %9, i16 %12)
  %13 = load ptr, ptr %2, !tbaa !2
  %14 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %13)
  %15 = load i16, ptr %3, !tbaa !2
  store i16 %15, ptr %14
  %16 = load ptr, ptr %2, !tbaa !2
  ret ptr %16
}

define cc 1005 ptr @N$DRES(ptr %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
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
  %15 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %16 = load i16, ptr %15
  store i16 %16, ptr %14, !tbaa !2
  %17 = call cc 1005 addrspace(1) ptr @buffers.capacity(ptr %0)
  %18 = load i16, ptr %17
  store i16 %18, ptr %13, !tbaa !2
  %19 = load i16, ptr %13, !tbaa !2
  %20 = add i16 %19, 1
  %21 = mul i16 %20, 4
  %22 = load i16, ptr %14, !tbaa !2
  %23 = mul i16 %22, 3
  %24 = icmp ule i16 %21, %23
  br i1 %24, label %b2, label %b3

b2:
  ret ptr %0

b3:
  br label %b4

b4:
  %25 = load i16, ptr %14, !tbaa !2
  %26 = icmp ult i16 %25, 8
  br i1 %26, label %b5, label %b6

b5:
  store i16 8, ptr %12, !tbaa !2
  br label %b7

b6:
  %27 = load i16, ptr %14, !tbaa !2
  %28 = mul i16 %27, 2
  store i16 %28, ptr %12, !tbaa !2
  br label %b7

b7:
  %29 = load i16, ptr %12, !tbaa !2
  store i16 %29, ptr %11, !tbaa !2
  %30 = load i16, ptr %11, !tbaa !2
  %31 = sub i16 %30, 1
  store i16 %31, ptr %10, !tbaa !2
  %32 = load i16, ptr %11, !tbaa !2
  %33 = call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %32, i16 %1)
  store ptr %33, ptr %9, !tbaa !2
  %34 = load i16, ptr %11, !tbaa !2
  %35 = mul i16 %34, %1
  store i16 0, ptr %8, !tbaa !2
  store i16 %35, ptr %7, !tbaa !2
  br label %b8

b8:
  %36 = load i16, ptr %8, !tbaa !2
  %37 = load i16, ptr %7, !tbaa !2
  %38 = icmp ult i16 %36, %37
  br i1 %38, label %b9, label %b11

b9:
  %39 = load ptr, ptr %9, !tbaa !2
  %40 = load i16, ptr %8, !tbaa !2
  %41 = mul i16 %40, 1
  %42 = getelementptr i8, ptr %39, i16 %41
  store i8 0, ptr %42
  br label %b10

b10:
  %43 = load i16, ptr %8, !tbaa !2
  %44 = add nuw i16 %43, 1
  store i16 %44, ptr %8, !tbaa !2
  br label %b8

b11:
  %45 = load i16, ptr %14, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i16 %45, ptr %5, !tbaa !2
  br label %b12

b12:
  %46 = load i16, ptr %6, !tbaa !2
  %47 = load i16, ptr %5, !tbaa !2
  %48 = icmp ult i16 %46, %47
  br i1 %48, label %b13, label %b15

b13:
  %49 = load i16, ptr %6, !tbaa !2
  %50 = mul i16 %49, %1
  %51 = mul i16 %50, 1
  %52 = getelementptr i8, ptr %0, i16 %51
  store ptr %52, ptr %4, !tbaa !2
  %53 = load ptr, ptr %4, !tbaa !2
  %54 = load i16, ptr %53
  store i16 %54, ptr %3, !tbaa !2
  %55 = load i16, ptr %3, !tbaa !2
  %56 = icmp eq i16 %55, 0
  br i1 %56, label %b16, label %b17

b14:
  %57 = load i16, ptr %6, !tbaa !2
  %58 = add nuw i16 %57, 1
  store i16 %58, ptr %6, !tbaa !2
  br label %b12

b15:
  %59 = load ptr, ptr %9, !tbaa !2
  %60 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %59)
  %61 = load i16, ptr %11, !tbaa !2
  store i16 %61, ptr %60
  %62 = load ptr, ptr %9, !tbaa !2
  %63 = call cc 1005 addrspace(1) ptr @buffers.capacity(ptr %62)
  %64 = load i16, ptr %13, !tbaa !2
  store i16 %64, ptr %63
  call cc 1005 addrspace(1) void @N$BDRP(ptr %0)
  %65 = load ptr, ptr %9, !tbaa !2
  ret ptr %65

b16:
  br label %b14

b17:
  br label %b18

b18:
  %66 = load i16, ptr %3, !tbaa !2
  %67 = load i16, ptr %10, !tbaa !2
  %68 = and i16 %66, %67
  store i16 %68, ptr %2, !tbaa !2
  br label %b19

b19:
  %69 = load ptr, ptr %9, !tbaa !2
  %70 = load i16, ptr %2, !tbaa !2
  %71 = mul i16 %70, %1
  %72 = mul i16 %71, 1
  %73 = getelementptr i8, ptr %69, i16 %72
  %74 = load i16, ptr %73
  %75 = icmp ne i16 %74, 0
  br i1 %75, label %b20, label %b21

b20:
  %76 = load i16, ptr %2, !tbaa !2
  %77 = add i16 %76, 1
  %78 = load i16, ptr %10, !tbaa !2
  %79 = and i16 %77, %78
  store i16 %79, ptr %2, !tbaa !2
  br label %b19

b21:
  %80 = load ptr, ptr %9, !tbaa !2
  %81 = load i16, ptr %2, !tbaa !2
  %82 = mul i16 %81, %1
  %83 = mul i16 %82, 1
  %84 = getelementptr i8, ptr %80, i16 %83
  %85 = load ptr, ptr %4, !tbaa !2
  %86 = addrspacecast ptr %85 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @buffers.copy(ptr %84, ptr addrspace(1) %86, i16 %1)
  br label %b14
}

define internal cc 1005 ptr @strings.append_bytes(ptr %0, ptr addrspace(1) %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca ptr
  %4 = alloca i16
  %5 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %6 = load i16, ptr %5
  store i16 %6, ptr %4, !tbaa !2
  %7 = load i16, ptr %4, !tbaa !2
  %8 = add i16 %7, %2
  %9 = call cc 1005 addrspace(1) ptr @N$BRES(ptr %0, i16 %8, i16 1)
  store ptr %9, ptr %3, !tbaa !2
  %10 = load ptr, ptr %3, !tbaa !2
  %11 = load i16, ptr %4, !tbaa !2
  %12 = mul i16 %11, 1
  %13 = getelementptr i8, ptr %10, i16 %12
  call cc 1005 addrspace(1) void @buffers.copy(ptr %13, ptr addrspace(1) %1, i16 %2)
  %14 = load ptr, ptr %3, !tbaa !2
  %15 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %14)
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

define cc 1005 ptr @N$TAPP(ptr %0, ptr %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  %3 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %1)
  %4 = load i16, ptr %3
  %5 = call cc 1005 addrspace(1) ptr @strings.append_bytes(ptr %0, ptr addrspace(1) %2, i16 %4)
  ret ptr %5
}

define cc 1005 ptr @N$TCAT(ptr %0, ptr %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr
  %3 = alloca i16
  %4 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %5 = load i16, ptr %4
  store i16 %5, ptr %3, !tbaa !2
  %6 = load i16, ptr %3, !tbaa !2
  %7 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %1)
  %8 = load i16, ptr %7
  %9 = add i16 %6, %8
  %10 = call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %9, i16 1)
  store ptr %10, ptr %2, !tbaa !2
  %11 = load ptr, ptr %2, !tbaa !2
  %12 = addrspacecast ptr %0 to ptr addrspace(1)
  %13 = load i16, ptr %3, !tbaa !2
  call cc 1005 addrspace(1) void @buffers.copy(ptr %11, ptr addrspace(1) %12, i16 %13)
  %14 = load ptr, ptr %2, !tbaa !2
  %15 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %14)
  %16 = load i16, ptr %3, !tbaa !2
  store i16 %16, ptr %15
  %17 = load ptr, ptr %2, !tbaa !2
  %18 = call cc 1005 addrspace(1) ptr @N$TAPP(ptr %17, ptr %1)
  ret ptr %18
}

define cc 1005 ptr @N$VCPY(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = alloca ptr
  %3 = load i16, ptr addrspace(1) %0
  %4 = call cc 1005 addrspace(1) ptr @buffers.allocate(i16 %3, i16 1)
  store ptr %4, ptr %2, !tbaa !2
  %5 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %6 = load ptr addrspace(1), ptr addrspace(1) %5
  store ptr addrspace(1) %6, ptr %1, !tbaa !2
  %7 = load ptr, ptr %2, !tbaa !2
  %8 = load ptr addrspace(1), ptr %1, !tbaa !2
  %9 = load i16, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @buffers.copy(ptr %7, ptr addrspace(1) %8, i16 %9)
  %10 = load ptr, ptr %2, !tbaa !2
  %11 = load i16, ptr addrspace(1) %0
  %12 = mul i16 %11, 1
  %13 = getelementptr i8, ptr %10, i16 %12
  store i8 0, ptr %13
  %14 = load ptr, ptr %2, !tbaa !2
  %15 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %14)
  %16 = load i16, ptr addrspace(1) %0
  store i16 %16, ptr %15
  %17 = load ptr, ptr %2, !tbaa !2
  ret ptr %17
}

define cc 1005 i8 @N$VCMP(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca ptr addrspace(1)
  %7 = alloca ptr addrspace(1)
  %8 = alloca i16
  %9 = alloca i16
  %10 = load i16, ptr addrspace(1) %0
  %11 = load i16, ptr addrspace(1) %1
  %12 = icmp ult i16 %10, %11
  br i1 %12, label %b2, label %b3

b2:
  %13 = load i16, ptr addrspace(1) %0
  store i16 %13, ptr %9, !tbaa !2
  br label %b4

b3:
  %14 = load i16, ptr addrspace(1) %1
  store i16 %14, ptr %9, !tbaa !2
  br label %b4

b4:
  %15 = load i16, ptr %9, !tbaa !2
  store i16 %15, ptr %8, !tbaa !2
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %17 = load ptr addrspace(1), ptr addrspace(1) %16
  store ptr addrspace(1) %17, ptr %7, !tbaa !2
  %18 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %19 = load ptr addrspace(1), ptr addrspace(1) %18
  store ptr addrspace(1) %19, ptr %6, !tbaa !2
  %20 = load i16, ptr %8, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 %20, ptr %4, !tbaa !2
  br label %b5

b5:
  %21 = load i16, ptr %5, !tbaa !2
  %22 = load i16, ptr %4, !tbaa !2
  %23 = icmp ult i16 %21, %22
  br i1 %23, label %b6, label %b8

b6:
  %24 = load ptr addrspace(1), ptr %7, !tbaa !2
  %25 = load i16, ptr %5, !tbaa !2
  %26 = mul i16 %25, 1
  %27 = getelementptr i8, ptr addrspace(1) %24, i16 %26
  %28 = load i8, ptr addrspace(1) %27
  %29 = load ptr addrspace(1), ptr %6, !tbaa !2
  %30 = load i16, ptr %5, !tbaa !2
  %31 = mul i16 %30, 1
  %32 = getelementptr i8, ptr addrspace(1) %29, i16 %31
  %33 = load i8, ptr addrspace(1) %32
  %34 = icmp ne i8 %28, %33
  br i1 %34, label %b9, label %b10

b7:
  %35 = load i16, ptr %5, !tbaa !2
  %36 = add nuw i16 %35, 1
  store i16 %36, ptr %5, !tbaa !2
  br label %b5

b8:
  %37 = load i16, ptr addrspace(1) %0
  %38 = load i16, ptr addrspace(1) %1
  %39 = icmp eq i16 %37, %38
  br i1 %39, label %b15, label %b16

b9:
  %40 = load ptr addrspace(1), ptr %7, !tbaa !2
  %41 = load i16, ptr %5, !tbaa !2
  %42 = mul i16 %41, 1
  %43 = getelementptr i8, ptr addrspace(1) %40, i16 %42
  %44 = load i8, ptr addrspace(1) %43
  %45 = load ptr addrspace(1), ptr %6, !tbaa !2
  %46 = load i16, ptr %5, !tbaa !2
  %47 = mul i16 %46, 1
  %48 = getelementptr i8, ptr addrspace(1) %45, i16 %47
  %49 = load i8, ptr addrspace(1) %48
  %50 = icmp ult i8 %44, %49
  br i1 %50, label %b12, label %b13

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
  %51 = load i8, ptr %3, !tbaa !2
  ret i8 %51

b15:
  ret i8 0

b16:
  br label %b17

b17:
  %52 = load i16, ptr addrspace(1) %0
  %53 = load i16, ptr addrspace(1) %1
  %54 = icmp ult i16 %52, %53
  br i1 %54, label %b18, label %b19

b18:
  store i8 -1, ptr %2, !tbaa !2
  br label %b20

b19:
  store i8 1, ptr %2, !tbaa !2
  br label %b20

b20:
  %55 = load i8, ptr %2, !tbaa !2
  ret i8 %55
}

define internal cc 1005 ptr @format.scratch_at(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr
  store ptr @$var_format.scratch, ptr %1, !tbaa !2
  %2 = load ptr, ptr %1, !tbaa !2
  %3 = mul i16 %0, 1
  %4 = getelementptr i8, ptr %2, i16 %3
  ret ptr %4
}

define internal cc 1005 void @format.put(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = load ptr, ptr @$var_format.sink, !tbaa !2
  %3 = icmp eq ptr %2, null
  br i1 %3, label %b2, label %b3

b2:
  call cc 1005 addrspace(1) void @os.write(ptr addrspace(1) %0, i16 %1)
  br label %b4

b3:
  %4 = load ptr, ptr @$var_format.sink, !tbaa !2
  %5 = call cc 1005 addrspace(1) ptr @strings.append_bytes(ptr %4, ptr addrspace(1) %0, i16 %1)
  store ptr %5, ptr @$var_format.sink, !tbaa !2
  br label %b4

b4:
  ret void
}

define internal cc 1005 void @format.put_text(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @format.put(ptr addrspace(1) %4, i16 %5)
  ret void
}

define internal cc 1005 void @format.pad(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca ptr
  store ptr @$var_format.fill, ptr %3, !tbaa !2
  store i16 0, ptr %2, !tbaa !2
  store i16 %0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %2, !tbaa !2
  %5 = load i16, ptr %1, !tbaa !2
  %6 = icmp ult i16 %4, %5
  br i1 %6, label %b3, label %b5

b3:
  %7 = load ptr, ptr %3, !tbaa !2
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @format.put(ptr addrspace(1) %8, i16 1)
  br label %b4

b4:
  %9 = load i16, ptr %2, !tbaa !2
  %10 = add nuw i16 %9, 1
  store i16 %10, ptr %2, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 i16 @format.open(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = load i8, ptr @$var_format.width, !tbaa !2
  %4 = zext i8 %3 to i16
  %5 = icmp ugt i16 %4, %0
  br i1 %5, label %b2, label %b3

b2:
  %6 = load i8, ptr @$var_format.width, !tbaa !2
  %7 = zext i8 %6 to i16
  %8 = sub i16 %7, %0
  store i16 %8, ptr %2, !tbaa !2
  br label %b4

b3:
  store i16 0, ptr %2, !tbaa !2
  br label %b4

b4:
  %9 = load i16, ptr %2, !tbaa !2
  store i16 %9, ptr %1, !tbaa !2
  store i8 0, ptr @$var_format.width, !tbaa !2
  store i8 10, ptr @$var_format.radix, !tbaa !2
  %10 = load i8, ptr @$var_format.left, !tbaa !2, !range !18
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b6, label %b5

b5:
  %12 = load i16, ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @format.pad(i16 %12)
  br label %b7

b6:
  br label %b7

b7:
  %13 = load i16, ptr %1, !tbaa !2
  ret i16 %13
}

define internal cc 1005 void @format.close(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = load i8, ptr @$var_format.left, !tbaa !2, !range !19
  %2 = icmp ne i8 %1, 0
  br i1 %2, label %b2, label %b3

b2:
  call cc 1005 addrspace(1) void @format.pad(i16 %0)
  br label %b4

b3:
  br label %b4

b4:
  store i8 32, ptr @$var_format.fill, !tbaa !2
  store i8 0, ptr @$var_format.left, !tbaa !2
  ret void
}

define internal cc 1005 void @format.field(ptr addrspace(1) %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = call cc 1005 addrspace(1) i16 @format.open(i16 %1)
  store i16 %3, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @format.put(ptr addrspace(1) %0, i16 %1)
  %4 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @format.close(i16 %4)
  ret void
}

define internal cc 1005 void @format.word(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @format.field(ptr addrspace(1) %4, i16 %5)
  ret void
}

define internal cc 1005 void @format.number(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca ptr
  store ptr %0, ptr %3, !tbaa !2
  store i16 %1, ptr %2, !tbaa !2
  %4 = load i8, ptr @$var_format.fill, !tbaa !2
  %5 = zext i8 %4 to i16
  %6 = icmp eq i16 %5, 48
  br i1 %6, label %b7, label %b3

b2:
  %7 = load ptr, ptr %3, !tbaa !2
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @format.put(ptr addrspace(1) %8, i16 1)
  %9 = load ptr, ptr %3, !tbaa !2
  %10 = getelementptr i8, ptr %9, i16 1
  store ptr %10, ptr %3, !tbaa !2
  %11 = load i16, ptr %2, !tbaa !2
  %12 = sub i16 %11, 1
  store i16 %12, ptr %2, !tbaa !2
  %13 = load i8, ptr @$var_format.width, !tbaa !2
  %14 = zext i8 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b8, label %b9

b3:
  br label %b4

b4:
  %16 = load ptr, ptr %3, !tbaa !2
  %17 = addrspacecast ptr %16 to ptr addrspace(1)
  %18 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @format.field(ptr addrspace(1) %17, i16 %18)
  ret void

b5:
  %19 = load ptr, ptr %3, !tbaa !2
  %20 = load i8, ptr %19
  %21 = zext i8 %20 to i16
  %22 = icmp eq i16 %21, 45
  br i1 %22, label %b2, label %b3

b6:
  %23 = load i16, ptr %2, !tbaa !2
  %24 = icmp ne i16 %23, 0
  br i1 %24, label %b5, label %b3

b7:
  %25 = load i8, ptr @$var_format.left, !tbaa !2, !range !20
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b3, label %b6

b8:
  %27 = load i8, ptr @$var_format.width, !tbaa !2
  %28 = zext i8 %27 to i16
  %29 = sub i16 %28, 1
  %30 = trunc i16 %29 to i8
  store i8 %30, ptr @$var_format.width, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  br label %b4
}

define internal cc 1005 i16 @format.written(ptr %0, i32 %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i16
  %6 = alloca i32
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
  br i1 %19, label %b5, label %b6

b5:
  %20 = load i8, ptr %4, !tbaa !2
  %21 = zext i8 %20 to i16
  %22 = add i16 48, %21
  %23 = trunc i16 %22 to i8
  store i8 %23, ptr %3, !tbaa !2
  br label %b7

b6:
  %24 = load i8, ptr %4, !tbaa !2
  %25 = zext i8 %24 to i16
  %26 = add i16 87, %25
  %27 = trunc i16 %26 to i8
  store i8 %27, ptr %3, !tbaa !2
  br label %b7

b7:
  %28 = load i8, ptr %3, !tbaa !2
  store i8 %28, ptr %16
  %29 = load i32, ptr %6, !tbaa !2
  %30 = zext i8 %2 to i32
  %31 = udiv i32 %29, %30
  store i32 %31, ptr %6, !tbaa !2
  %32 = load i32, ptr %6, !tbaa !2
  %33 = icmp eq i32 %32, 0
  br i1 %33, label %b8, label %b9

b8:
  %34 = load i16, ptr %5, !tbaa !2
  ret i16 %34

b9:
  br label %b10

b10:
  br label %b2
}

define internal cc 1005 void @format.integer(i32 %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca ptr
  %4 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 16)
  store ptr %4, ptr %3, !tbaa !2
  %5 = load ptr, ptr %3, !tbaa !2
  %6 = load i8, ptr @$var_format.radix, !tbaa !2
  %7 = call cc 1005 addrspace(1) i16 @format.written(ptr %5, i32 %0, i8 zeroext %6)
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
  call cc 1005 addrspace(1) void @format.number(ptr %20, i16 %21)
  ret void
}

define internal cc 1005 void @format.signed(i32 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i32
  %2 = icmp slt i32 %0, 0
  br i1 %2, label %b2, label %b3

b2:
  %3 = sub i32 0, %0
  store i32 %3, ptr %1, !tbaa !2
  br label %b4

b3:
  store i32 %0, ptr %1, !tbaa !2
  br label %b4

b4:
  %4 = load i32, ptr %1, !tbaa !2
  %5 = icmp slt i32 %0, 0
  %6 = zext i1 %5 to i8
  call cc 1005 addrspace(1) void @format.integer(i32 %4, i8 %6)
  ret void
}

define cc 1005 void @N$PI1(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = sext i8 %0 to i32
  call cc 1005 addrspace(1) void @format.signed(i32 %1)
  ret void
}

define cc 1005 void @N$PU1(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = zext i8 %0 to i32
  call cc 1005 addrspace(1) void @format.integer(i32 %1, i8 0)
  ret void
}

define cc 1005 void @N$PI2(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = sext i16 %0 to i32
  call cc 1005 addrspace(1) void @format.signed(i32 %1)
  ret void
}

define cc 1005 void @N$PU2(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = zext i16 %0 to i32
  call cc 1005 addrspace(1) void @format.integer(i32 %1, i8 0)
  ret void
}

define cc 1005 void @N$PI4(i32 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1005 addrspace(1) void @format.signed(i32 %0)
  ret void
}

define cc 1005 void @N$PU4(i32 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1005 addrspace(1) void @format.integer(i32 %0, i8 0)
  ret void
}

define cc 1005 void @N$PQ4(i32 %0, i8 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  %6 = alloca i32
  %7 = alloca i32
  %8 = alloca i16
  %9 = alloca ptr
  %10 = alloca i32
  %11 = alloca i16
  store i16 0, ptr %11, !tbaa !2
  store i32 %0, ptr %10, !tbaa !2
  %12 = icmp slt i32 %0, 0
  br i1 %12, label %b2, label %b3

b2:
  %13 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 0)
  store i8 45, ptr %13
  store i16 1, ptr %11, !tbaa !2
  %14 = sub i32 0, %0
  store i32 %14, ptr %10, !tbaa !2
  br label %b4

b3:
  br label %b4

b4:
  %15 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 16)
  store ptr %15, ptr %9, !tbaa !2
  %16 = load ptr, ptr %9, !tbaa !2
  %17 = load i32, ptr %10, !tbaa !2
  %18 = zext i8 %1 to i16
  %19 = zext i16 %18 to i32
  %20 = lshr i32 %17, %19
  %21 = call cc 1005 addrspace(1) i16 @format.written(ptr %16, i32 %20, i8 zeroext 10)
  store i16 %21, ptr %8, !tbaa !2
  %22 = load i16, ptr %11, !tbaa !2
  %23 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 %22)
  %24 = load ptr, ptr %9, !tbaa !2
  %25 = load i16, ptr %8, !tbaa !2
  %26 = sub i16 0, %25
  %27 = mul i16 %26, 1
  %28 = getelementptr i8, ptr %24, i16 %27
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  %30 = load i16, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @buffers.copy(ptr %23, ptr addrspace(1) %29, i16 %30)
  %31 = load i16, ptr %11, !tbaa !2
  %32 = load i16, ptr %8, !tbaa !2
  %33 = add i16 %31, %32
  store i16 %33, ptr %11, !tbaa !2
  %34 = load i16, ptr %11, !tbaa !2
  %35 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 %34)
  store i8 46, ptr %35
  %36 = load i16, ptr %11, !tbaa !2
  %37 = add i16 %36, 1
  store i16 %37, ptr %11, !tbaa !2
  %38 = zext i8 %1 to i16
  %39 = zext i16 %38 to i32
  %40 = shl i32 1, %39
  store i32 %40, ptr %7, !tbaa !2
  %41 = load i32, ptr %10, !tbaa !2
  %42 = load i32, ptr %7, !tbaa !2
  %43 = sub i32 %42, 1
  %44 = and i32 %41, %43
  store i32 %44, ptr %6, !tbaa !2
  %45 = load i32, ptr %6, !tbaa !2
  %46 = icmp eq i32 %45, 0
  br i1 %46, label %b5, label %b6

b5:
  %47 = load i16, ptr %11, !tbaa !2
  %48 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 %47)
  store i8 48, ptr %48
  %49 = load i16, ptr %11, !tbaa !2
  %50 = add i16 %49, 1
  store i16 %50, ptr %11, !tbaa !2
  br label %b7

b6:
  br label %b7

b7:
  %51 = load i32, ptr %7, !tbaa !2
  %52 = udiv i32 %51, 10
  store i32 %52, ptr %5, !tbaa !2
  %53 = load i32, ptr %7, !tbaa !2
  %54 = urem i32 %53, 10
  %55 = trunc i32 %54 to i16
  store i16 %55, ptr %4, !tbaa !2
  br label %b8

b8:
  %56 = load i32, ptr %6, !tbaa !2
  %57 = icmp ne i32 %56, 0
  br i1 %57, label %b9, label %b10

b9:
  store i16 0, ptr %3, !tbaa !2
  store i16 1, ptr %2, !tbaa !2
  br label %b11

b10:
  %58 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 0)
  %59 = load i16, ptr %11, !tbaa !2
  call cc 1005 addrspace(1) void @format.number(ptr %58, i16 %59)
  ret void

b11:
  %60 = load i16, ptr %2, !tbaa !2
  %61 = icmp ule i16 %60, 9
  br i1 %61, label %b14, label %b13

b12:
  %62 = load i16, ptr %2, !tbaa !2
  store i16 %62, ptr %3, !tbaa !2
  %63 = load i16, ptr %2, !tbaa !2
  %64 = add i16 %63, 1
  store i16 %64, ptr %2, !tbaa !2
  br label %b11

b13:
  %65 = load i32, ptr %6, !tbaa !2
  %66 = load i16, ptr %3, !tbaa !2
  %67 = zext i16 %66 to i32
  %68 = load i32, ptr %5, !tbaa !2
  %69 = mul i32 %67, %68
  %70 = sub i32 %65, %69
  %71 = mul i32 %70, 10
  %72 = load i16, ptr %3, !tbaa !2
  %73 = load i16, ptr %4, !tbaa !2
  %74 = mul i16 %72, %73
  %75 = zext i16 %74 to i32
  %76 = sub i32 %71, %75
  store i32 %76, ptr %6, !tbaa !2
  %77 = load i16, ptr %11, !tbaa !2
  %78 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 %77)
  %79 = load i16, ptr %3, !tbaa !2
  %80 = trunc i16 %79 to i8
  %81 = zext i8 %80 to i16
  %82 = add i16 48, %81
  %83 = trunc i16 %82 to i8
  store i8 %83, ptr %78
  %84 = load i16, ptr %11, !tbaa !2
  %85 = add i16 %84, 1
  store i16 %85, ptr %11, !tbaa !2
  br label %b8

b14:
  %86 = load i32, ptr %6, !tbaa !2
  %87 = load i16, ptr %2, !tbaa !2
  %88 = zext i16 %87 to i32
  %89 = load i32, ptr %5, !tbaa !2
  %90 = mul i32 %88, %89
  %91 = load i16, ptr %2, !tbaa !2
  %92 = load i16, ptr %4, !tbaa !2
  %93 = mul i16 %91, %92
  %94 = add i16 %93, 9
  %95 = udiv i16 %94, 10
  %96 = zext i16 %95 to i32
  %97 = add i32 %90, %96
  %98 = icmp uge i32 %86, %97
  br i1 %98, label %b12, label %b13
}

define cc 1005 void @N$PQ2(i16 %0, i8 %1) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %2 = sext i16 %0 to i32
  call cc 1005 addrspace(1) void @N$PQ4(i32 %2, i8 zeroext %1)
  ret void
}

define cc 1005 void @N$PB(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca ptr
  %3 = icmp ne i8 %0, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr @$str20, i16 6
  store ptr %4, ptr %2, !tbaa !2
  br label %b4

b3:
  %5 = getelementptr i8, ptr @$str21, i16 6
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
  call cc 1005 addrspace(1) void @format.word(ptr addrspace(1) %12)
  ret void
}

define cc 1005 void @N$PC(i8 %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 0)
  store i8 %0, ptr %1
  %2 = call cc 1005 addrspace(1) ptr @format.scratch_at(i16 0)
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @format.field(ptr addrspace(1) %3, i16 1)
  ret void
}

define cc 1005 void @N$PS(ptr %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = call cc 1005 addrspace(1) ptr @buffers.length(ptr %0)
  %3 = load i16, ptr %2
  call cc 1005 addrspace(1) void @format.field(ptr addrspace(1) %1, i16 %3)
  ret void
}

define cc 1005 void @N$PV(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  call cc 1005 addrspace(1) void @format.field(ptr addrspace(1) %4, i16 %5)
  ret void
}

define cc 1005 void @N$PN() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
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
  call cc 1005 addrspace(1) void @format.put_text(ptr addrspace(1) %7)
  ret void
}

define cc 1005 void @N$PFLD(i8 %0, i8 %1, i8 %2, i8 %3) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i8 %0, ptr @$var_format.width, !tbaa !2
  store i8 %1, ptr @$var_format.radix, !tbaa !2
  store i8 %2, ptr @$var_format.fill, !tbaa !2
  %4 = zext i8 %3 to i16
  %5 = icmp ne i16 %4, 0
  %6 = zext i1 %5 to i8
  store i8 %6, ptr @$var_format.left, !tbaa !2
  ret void
}

define cc 1005 void @N$PBEG() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1005 addrspace(1) ptr @buffers.allocate(i16 16, i16 1)
  store ptr %0, ptr @$var_format.sink, !tbaa !2
  ret void
}

define cc 1005 ptr @N$PEND() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca ptr
  %1 = load ptr, ptr @$var_format.sink, !tbaa !2
  store ptr %1, ptr %0, !tbaa !2
  store ptr null, ptr @$var_format.sink, !tbaa !2
  %2 = load ptr, ptr %0, !tbaa !2
  ret ptr %2
}

define internal cc 1005 void @floats.assign(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  store i16 72, ptr %2, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %3, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp slt i16 %4, %5
  br i1 %6, label %b3, label %b5

b3:
  %7 = load i16, ptr %3, !tbaa !2
  %8 = mul i16 %7, 2
  %9 = getelementptr i8, ptr %0, i16 %8
  store i16 0, ptr %9
  br label %b4

b4:
  %10 = load i16, ptr %3, !tbaa !2
  %11 = add nsw i16 %10, 1
  store i16 %11, ptr %3, !tbaa !2
  br label %b2

b5:
  %12 = getelementptr i8, ptr %0, i16 0
  store i16 %1, ptr %12
  ret void
}

define internal cc 1005 void @floats.copy(ptr %0, ptr %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  store i16 0, ptr %3, !tbaa !2
  store i16 72, ptr %2, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %3, !tbaa !2
  %5 = load i16, ptr %2, !tbaa !2
  %6 = icmp slt i16 %4, %5
  br i1 %6, label %b3, label %b5

b3:
  %7 = load i16, ptr %3, !tbaa !2
  %8 = mul i16 %7, 2
  %9 = getelementptr i8, ptr %0, i16 %8
  %10 = load i16, ptr %3, !tbaa !2
  %11 = mul i16 %10, 2
  %12 = getelementptr i8, ptr %1, i16 %11
  %13 = load i16, ptr %12
  store i16 %13, ptr %9
  br label %b4

b4:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = add nsw i16 %14, 1
  store i16 %15, ptr %3, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 void @floats.shift(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = lshr i16 %1, 4
  store i16 %6, ptr %5, !tbaa !2
  %7 = and i16 %1, 15
  store i16 %7, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !2
  %9 = icmp ne i16 %8, 0
  br i1 %9, label %b3, label %b4

b3:
  %10 = load i16, ptr %3, !tbaa !2
  %11 = sub i16 %10, 1
  store i16 %11, ptr %3, !tbaa !2
  %12 = load i16, ptr %3, !tbaa !2
  %13 = mul i16 %12, 2
  %14 = getelementptr i8, ptr %0, i16 %13
  %15 = load i16, ptr %3, !tbaa !2
  %16 = load i16, ptr %5, !tbaa !2
  %17 = icmp uge i16 %15, %16
  br i1 %17, label %b5, label %b6

b4:
  %18 = load i16, ptr %4, !tbaa !2
  %19 = icmp ne i16 %18, 0
  br i1 %19, label %b8, label %b9

b5:
  %20 = load i16, ptr %3, !tbaa !2
  %21 = load i16, ptr %5, !tbaa !2
  %22 = sub i16 %20, %21
  %23 = mul i16 %22, 2
  %24 = getelementptr i8, ptr %0, i16 %23
  %25 = load i16, ptr %24
  store i16 %25, ptr %2, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr %2, !tbaa !2
  br label %b7

b7:
  %26 = load i16, ptr %2, !tbaa !2
  store i16 %26, ptr %14
  br label %b2

b8:
  store i16 72, ptr %3, !tbaa !2
  br label %b11

b9:
  br label %b10

b10:
  ret void

b11:
  %27 = load i16, ptr %3, !tbaa !2
  %28 = icmp ugt i16 %27, 1
  br i1 %28, label %b12, label %b13

b12:
  %29 = load i16, ptr %3, !tbaa !2
  %30 = sub i16 %29, 1
  store i16 %30, ptr %3, !tbaa !2
  %31 = load i16, ptr %3, !tbaa !2
  %32 = mul i16 %31, 2
  %33 = getelementptr i8, ptr %0, i16 %32
  %34 = load i16, ptr %3, !tbaa !2
  %35 = mul i16 %34, 2
  %36 = getelementptr i8, ptr %0, i16 %35
  %37 = load i16, ptr %36
  %38 = load i16, ptr %4, !tbaa !2
  %39 = shl i16 %37, %38
  %40 = load i16, ptr %3, !tbaa !2
  %41 = sub i16 %40, 1
  %42 = mul i16 %41, 2
  %43 = getelementptr i8, ptr %0, i16 %42
  %44 = load i16, ptr %43
  %45 = load i16, ptr %4, !tbaa !2
  %46 = sub i16 16, %45
  %47 = lshr i16 %44, %46
  %48 = or i16 %39, %47
  store i16 %48, ptr %33
  br label %b11

b13:
  %49 = getelementptr i8, ptr %0, i16 0
  %50 = getelementptr i8, ptr %0, i16 0
  %51 = load i16, ptr %50
  %52 = load i16, ptr %4, !tbaa !2
  %53 = shl i16 %51, %52
  store i16 %53, ptr %49
  br label %b10
}

define internal cc 1005 void @floats.multiply(ptr %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  %8 = icmp slt i16 %6, %7
  br i1 %8, label %b3, label %b5

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = mul i16 %9, 2
  %11 = getelementptr i8, ptr %0, i16 %10
  %12 = load i16, ptr %11
  %13 = zext i16 %12 to i32
  %14 = zext i16 %1 to i32
  %15 = mul i32 %13, %14
  %16 = load i32, ptr %5, !tbaa !2
  %17 = add i32 %15, %16
  store i32 %17, ptr %2, !tbaa !2
  %18 = load i16, ptr %4, !tbaa !2
  %19 = mul i16 %18, 2
  %20 = getelementptr i8, ptr %0, i16 %19
  %21 = load i32, ptr %2, !tbaa !2
  %22 = trunc i32 %21 to i16
  store i16 %22, ptr %20
  %23 = load i32, ptr %2, !tbaa !2
  %24 = lshr i32 %23, 16
  store i32 %24, ptr %5, !tbaa !2
  br label %b4

b4:
  %25 = load i16, ptr %4, !tbaa !2
  %26 = add nsw i16 %25, 1
  store i16 %26, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 void @floats.add(ptr %0, ptr %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  %8 = icmp slt i16 %6, %7
  br i1 %8, label %b3, label %b5

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = mul i16 %9, 2
  %11 = getelementptr i8, ptr %0, i16 %10
  %12 = load i16, ptr %11
  %13 = zext i16 %12 to i32
  %14 = load i16, ptr %4, !tbaa !2
  %15 = mul i16 %14, 2
  %16 = getelementptr i8, ptr %1, i16 %15
  %17 = load i16, ptr %16
  %18 = zext i16 %17 to i32
  %19 = add i32 %13, %18
  %20 = load i32, ptr %5, !tbaa !2
  %21 = add i32 %19, %20
  store i32 %21, ptr %2, !tbaa !2
  %22 = load i16, ptr %4, !tbaa !2
  %23 = mul i16 %22, 2
  %24 = getelementptr i8, ptr %0, i16 %23
  %25 = load i32, ptr %2, !tbaa !2
  %26 = trunc i32 %25 to i16
  store i16 %26, ptr %24
  %27 = load i32, ptr %2, !tbaa !2
  %28 = lshr i32 %27, 16
  store i32 %28, ptr %5, !tbaa !2
  br label %b4

b4:
  %29 = load i16, ptr %4, !tbaa !2
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 void @floats.subtract(ptr %0, ptr %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i32
  store i32 0, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %4, !tbaa !2
  %7 = load i16, ptr %3, !tbaa !2
  %8 = icmp slt i16 %6, %7
  br i1 %8, label %b3, label %b5

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = mul i16 %9, 2
  %11 = getelementptr i8, ptr %0, i16 %10
  %12 = load i16, ptr %11
  %13 = zext i16 %12 to i32
  %14 = load i16, ptr %4, !tbaa !2
  %15 = mul i16 %14, 2
  %16 = getelementptr i8, ptr %1, i16 %15
  %17 = load i16, ptr %16
  %18 = zext i16 %17 to i32
  %19 = sub i32 %13, %18
  %20 = load i32, ptr %5, !tbaa !2
  %21 = sub i32 %19, %20
  store i32 %21, ptr %2, !tbaa !2
  %22 = load i16, ptr %4, !tbaa !2
  %23 = mul i16 %22, 2
  %24 = getelementptr i8, ptr %0, i16 %23
  %25 = load i32, ptr %2, !tbaa !2
  %26 = trunc i32 %25 to i16
  store i16 %26, ptr %24
  %27 = load i32, ptr %2, !tbaa !2
  %28 = lshr i32 %27, 16
  %29 = and i32 %28, 1
  store i32 %29, ptr %5, !tbaa !2
  br label %b4

b4:
  %30 = load i16, ptr %4, !tbaa !2
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %4, !tbaa !2
  br label %b2

b5:
  ret void
}

define internal cc 1005 i8 @floats.compare(ptr %0, ptr %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = alloca i16
  store i16 72, ptr %3, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %3, !tbaa !2
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr %3, !tbaa !2
  %7 = sub i16 %6, 1
  store i16 %7, ptr %3, !tbaa !2
  %8 = load i16, ptr %3, !tbaa !2
  %9 = mul i16 %8, 2
  %10 = getelementptr i8, ptr %0, i16 %9
  %11 = load i16, ptr %10
  %12 = load i16, ptr %3, !tbaa !2
  %13 = mul i16 %12, 2
  %14 = getelementptr i8, ptr %1, i16 %13
  %15 = load i16, ptr %14
  %16 = icmp ne i16 %11, %15
  br i1 %16, label %b5, label %b6

b4:
  ret i8 0

b5:
  %17 = load i16, ptr %3, !tbaa !2
  %18 = mul i16 %17, 2
  %19 = getelementptr i8, ptr %0, i16 %18
  %20 = load i16, ptr %19
  %21 = load i16, ptr %3, !tbaa !2
  %22 = mul i16 %21, 2
  %23 = getelementptr i8, ptr %1, i16 %22
  %24 = load i16, ptr %23
  %25 = icmp ult i16 %20, %24
  br i1 %25, label %b8, label %b9

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
  %26 = load i8, ptr %2, !tbaa !2
  ret i8 %26
}

define internal cc 1005 i8 @floats.beyond(ptr %0, ptr %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i8
  %6 = call cc 1005 addrspace(1) i8 @floats.compare(ptr %0, ptr %1)
  store i8 %6, ptr %5, !tbaa !2
  %7 = load i8, ptr %5, !tbaa !2
  %8 = sext i8 %7 to i16
  %9 = icmp sgt i16 %8, 0
  %10 = zext i1 %9 to i8
  store i8 %10, ptr %4, !tbaa !2
  br i1 %9, label %b3, label %b2

b2:
  store i8 %2, ptr %3, !tbaa !2
  %11 = icmp ne i8 %2, 0
  br i1 %11, label %b4, label %b5

b3:
  %12 = load i8, ptr %4, !tbaa !2, !range !21
  ret i8 %12

b4:
  %13 = load i8, ptr %5, !tbaa !2
  %14 = sext i8 %13 to i16
  %15 = icmp eq i16 %14, 0
  %16 = zext i1 %15 to i8
  store i8 %16, ptr %3, !tbaa !2
  br label %b5

b5:
  %17 = load i8, ptr %3, !tbaa !2, !range !22
  store i8 %17, ptr %4, !tbaa !2
  br label %b3
}

define internal cc 1005 i16 @floats.put(ptr %0, i16 %1, i8 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = mul i16 %1, 1
  %4 = getelementptr i8, ptr %0, i16 %3
  store i8 %2, ptr %4
  %5 = add i16 %1, 1
  ret i16 %5
}

define internal cc 1005 i16 @floats.put_text(ptr %0, i16 %1, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = alloca i16
  store i16 %1, ptr %4, !tbaa !2
  %5 = load i16, ptr addrspace(1) %2
  store i16 0, ptr %3, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %3, !tbaa !2
  %7 = icmp ult i16 %6, %5
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = getelementptr inbounds i8, ptr addrspace(1) %9, i16 %6
  %11 = load i16, ptr %4, !tbaa !2
  %12 = load i8, ptr addrspace(1) %10
  %13 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %11, i8 zeroext %12)
  store i16 %13, ptr %4, !tbaa !2
  br label %b4

b4:
  %14 = load i16, ptr %3, !tbaa !2
  %15 = add i16 %14, 1
  store i16 %15, ptr %3, !tbaa !2
  br label %b2

b5:
  %16 = load i16, ptr %4, !tbaa !2
  ret i16 %16
}

define internal cc 1005 i32 @floats.digits(ptr %0, i16 %1, i8 %2, i8 %3, ptr %4) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %5 = alloca i8
  %6 = alloca i8
  %7 = alloca i8
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca ptr
  %13 = alloca ptr
  %14 = alloca ptr
  %15 = alloca ptr
  %16 = alloca ptr
  %17 = alloca [4 x i8]
  store ptr %0, ptr %16, !tbaa !2
  %18 = getelementptr i8, ptr %0, i16 144
  store ptr %18, ptr %15, !tbaa !2
  %19 = getelementptr i8, ptr %0, i16 288
  store ptr %19, ptr %14, !tbaa !2
  %20 = getelementptr i8, ptr %0, i16 432
  store ptr %20, ptr %13, !tbaa !2
  %21 = getelementptr i8, ptr %0, i16 576
  store ptr %21, ptr %12, !tbaa !2
  %22 = icmp ne i8 %2, 0
  br i1 %22, label %b2, label %b3

b2:
  store i16 1, ptr %11, !tbaa !2
  br label %b4

b3:
  store i16 0, ptr %11, !tbaa !2
  br label %b4

b4:
  %23 = load i16, ptr %11, !tbaa !2
  store i16 %23, ptr %10, !tbaa !2
  %24 = icmp sge i16 %1, 0
  br i1 %24, label %b5, label %b6

b5:
  %25 = load ptr, ptr %16, !tbaa !2
  %26 = add i16 %1, 1
  %27 = load i16, ptr %10, !tbaa !2
  %28 = add i16 %26, %27
  call cc 1005 addrspace(1) void @floats.shift(ptr %25, i16 %28)
  %29 = load ptr, ptr %15, !tbaa !2
  %30 = load i16, ptr %10, !tbaa !2
  %31 = icmp ult i16 %30, 16
  br i1 %31, label %b8, label %b9

b6:
  %32 = load ptr, ptr %16, !tbaa !2
  %33 = load i16, ptr %10, !tbaa !2
  %34 = add i16 1, %33
  call cc 1005 addrspace(1) void @floats.shift(ptr %32, i16 %34)
  %35 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @floats.assign(ptr %35, i16 1)
  %36 = load ptr, ptr %15, !tbaa !2
  %37 = sub i16 0, %1
  %38 = add i16 %37, 1
  %39 = load i16, ptr %10, !tbaa !2
  %40 = add i16 %38, %39
  call cc 1005 addrspace(1) void @floats.shift(ptr %36, i16 %40)
  %41 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @floats.assign(ptr %41, i16 1)
  br label %b7

b7:
  %42 = load ptr, ptr %14, !tbaa !2
  %43 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @floats.copy(ptr %42, ptr %43)
  %44 = load ptr, ptr %14, !tbaa !2
  %45 = load i16, ptr %10, !tbaa !2
  call cc 1005 addrspace(1) void @floats.shift(ptr %44, i16 %45)
  store i16 0, ptr %9, !tbaa !2
  br label %b10

b8:
  %46 = shl i16 2, %30
  call cc 1005 addrspace(1) void @floats.assign(ptr %29, i16 %46)
  %47 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @floats.assign(ptr %47, i16 1)
  %48 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @floats.shift(ptr %48, i16 %1)
  br label %b7

b9:
  call cc 1005 addrspace(1) void @N$ESHF()
  unreachable

b10:
  br label %b11

b11:
  %49 = load ptr, ptr %12, !tbaa !2
  %50 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @floats.copy(ptr %49, ptr %50)
  %51 = load ptr, ptr %12, !tbaa !2
  %52 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @floats.add(ptr %51, ptr %52)
  %53 = load ptr, ptr %12, !tbaa !2
  %54 = load ptr, ptr %15, !tbaa !2
  %55 = xor i8 %3, 1
  %56 = call cc 1005 addrspace(1) i8 @floats.beyond(ptr %53, ptr %54, i8 %55)
  %57 = icmp ne i8 %56, 0
  br i1 %57, label %b14, label %b13

b12:
  br label %b16

b13:
  br label %b12

b14:
  br label %b15

b15:
  %58 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %58, i16 10)
  %59 = load i16, ptr %9, !tbaa !2
  %60 = add i16 %59, 1
  store i16 %60, ptr %9, !tbaa !2
  br label %b10

b16:
  br label %b17

b17:
  %61 = load ptr, ptr %12, !tbaa !2
  %62 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @floats.copy(ptr %61, ptr %62)
  %63 = load ptr, ptr %12, !tbaa !2
  %64 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @floats.add(ptr %63, ptr %64)
  %65 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %65, i16 10)
  %66 = load ptr, ptr %12, !tbaa !2
  %67 = load ptr, ptr %15, !tbaa !2
  %68 = xor i8 %3, 1
  %69 = call cc 1005 addrspace(1) i8 @floats.beyond(ptr %66, ptr %67, i8 %68)
  %70 = icmp ne i8 %69, 0
  br i1 %70, label %b19, label %b20

b18:
  store i16 0, ptr %8, !tbaa !2
  br label %b22

b19:
  br label %b18

b20:
  br label %b21

b21:
  %71 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %71, i16 10)
  %72 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %72, i16 10)
  %73 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %73, i16 10)
  %74 = load i16, ptr %9, !tbaa !2
  %75 = sub i16 %74, 1
  store i16 %75, ptr %9, !tbaa !2
  br label %b16

b22:
  br label %b23

b23:
  %76 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %76, i16 10)
  %77 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %77, i16 10)
  %78 = load ptr, ptr %13, !tbaa !2
  call cc 1005 addrspace(1) void @floats.multiply(ptr %78, i16 10)
  store i8 0, ptr %7, !tbaa !2
  br label %b25

b25:
  %79 = load ptr, ptr %16, !tbaa !2
  %80 = load ptr, ptr %15, !tbaa !2
  %81 = call cc 1005 addrspace(1) i8 @floats.compare(ptr %79, ptr %80)
  %82 = sext i8 %81 to i16
  %83 = icmp sge i16 %82, 0
  br i1 %83, label %b26, label %b27

b26:
  %84 = load ptr, ptr %16, !tbaa !2
  %85 = load ptr, ptr %15, !tbaa !2
  call cc 1005 addrspace(1) void @floats.subtract(ptr %84, ptr %85)
  %86 = load i8, ptr %7, !tbaa !2
  %87 = zext i8 %86 to i16
  %88 = add i16 %87, 1
  %89 = trunc i16 %88 to i8
  store i8 %89, ptr %7, !tbaa !2
  br label %b25

b27:
  %90 = load ptr, ptr %16, !tbaa !2
  %91 = load ptr, ptr %13, !tbaa !2
  %92 = xor i8 %3, 1
  %93 = call cc 1005 addrspace(1) i8 @floats.beyond(ptr %90, ptr %91, i8 %92)
  %94 = xor i8 %93, 1
  store i8 %94, ptr %6, !tbaa !2
  %95 = load ptr, ptr %12, !tbaa !2
  %96 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @floats.copy(ptr %95, ptr %96)
  %97 = load ptr, ptr %12, !tbaa !2
  %98 = load ptr, ptr %14, !tbaa !2
  call cc 1005 addrspace(1) void @floats.add(ptr %97, ptr %98)
  %99 = load ptr, ptr %12, !tbaa !2
  %100 = load ptr, ptr %15, !tbaa !2
  %101 = call cc 1005 addrspace(1) i8 @floats.beyond(ptr %99, ptr %100, i8 %3)
  store i8 %101, ptr %5, !tbaa !2
  %102 = load i8, ptr %6, !tbaa !2, !range !23
  %103 = icmp ne i8 %102, 0
  br i1 %103, label %b29, label %b31

b28:
  %104 = load i16, ptr %8, !tbaa !2
  %105 = load i8, ptr %7, !tbaa !2
  %106 = zext i8 %105 to i16
  %107 = add i16 48, %106
  %108 = trunc i16 %107 to i8
  %109 = call cc 1005 addrspace(1) i16 @floats.put(ptr %4, i16 %104, i8 zeroext %108)
  store i16 %109, ptr %8, !tbaa !2
  br label %b22

b29:
  br label %b30

b30:
  %110 = load i8, ptr %6, !tbaa !2, !range !24
  %111 = icmp ne i8 %110, 0
  br i1 %111, label %b35, label %b33

b31:
  %112 = load i8, ptr %5, !tbaa !2, !range !25
  %113 = icmp ne i8 %112, 0
  br i1 %113, label %b29, label %b28

b32:
  %114 = load ptr, ptr %12, !tbaa !2
  %115 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @floats.copy(ptr %114, ptr %115)
  %116 = load ptr, ptr %12, !tbaa !2
  call cc 1005 addrspace(1) void @floats.shift(ptr %116, i16 1)
  %117 = load ptr, ptr %12, !tbaa !2
  %118 = load ptr, ptr %15, !tbaa !2
  %119 = call cc 1005 addrspace(1) i8 @floats.compare(ptr %117, ptr %118)
  %120 = sext i8 %119 to i16
  %121 = icmp sge i16 %120, 0
  br i1 %121, label %b36, label %b37

b33:
  %122 = load i8, ptr %5, !tbaa !2, !range !26
  %123 = icmp ne i8 %122, 0
  br i1 %123, label %b39, label %b40

b34:
  %124 = load i16, ptr %8, !tbaa !2
  %125 = load i8, ptr %7, !tbaa !2
  %126 = zext i8 %125 to i16
  %127 = add i16 48, %126
  %128 = trunc i16 %127 to i8
  %129 = call cc 1005 addrspace(1) i16 @floats.put(ptr %4, i16 %124, i8 zeroext %128)
  store i16 %129, ptr %8, !tbaa !2
  %130 = load i16, ptr %8, !tbaa !2
  %131 = load i16, ptr %9, !tbaa !2
  store i16 %130, ptr %17, !tbaa !2
  %132 = getelementptr inbounds i8, ptr %17, i16 2
  store i16 %131, ptr %132, !tbaa !2
  %133 = addrspacecast ptr %17 to ptr addrspace(1)
  %134 = load i32, ptr addrspace(1) %133, !tbaa !2
  ret i32 %134

b35:
  %135 = load i8, ptr %5, !tbaa !2, !range !27
  %136 = icmp ne i8 %135, 0
  br i1 %136, label %b32, label %b33

b36:
  %137 = load i8, ptr %7, !tbaa !2
  %138 = zext i8 %137 to i16
  %139 = add i16 %138, 1
  %140 = trunc i16 %139 to i8
  store i8 %140, ptr %7, !tbaa !2
  br label %b38

b37:
  br label %b38

b38:
  br label %b34

b39:
  %141 = load i8, ptr %7, !tbaa !2
  %142 = zext i8 %141 to i16
  %143 = add i16 %142, 1
  %144 = trunc i16 %143 to i8
  store i8 %144, ptr %7, !tbaa !2
  br label %b41

b40:
  br label %b41

b41:
  br label %b34
}

define internal cc 1005 void @floats.print_float(ptr addrspace(1) %0, i16 %1, i16 %2, i16 %3, i16 %4) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %16 = alloca [8 x i8]
  %17 = alloca ptr
  %18 = alloca i8
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i8
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca ptr
  %27 = alloca ptr
  %28 = alloca ptr
  %29 = call cc 1005 addrspace(1) ptr @heap.allocate(i16 752)
  store ptr %29, ptr %28, !tbaa !2
  %30 = load ptr, ptr %28, !tbaa !2
  store ptr %30, ptr %27, !tbaa !2
  %31 = load ptr, ptr %28, !tbaa !2
  %32 = getelementptr i8, ptr %31, i16 720
  store ptr %32, ptr %26, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  %33 = sub i16 %1, 1
  %34 = mul i16 %33, 2
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 %34
  %36 = load i16, ptr addrspace(1) %35
  store i16 %36, ptr %24, !tbaa !2
  %37 = load i16, ptr %24, !tbaa !2
  %38 = lshr i16 %37, %2
  %39 = and i16 %38, %3
  store i16 %39, ptr %23, !tbaa !2
  %40 = load i16, ptr %24, !tbaa !2
  %41 = shl i16 1, %2
  %42 = sub i16 %41, 1
  %43 = and i16 %40, %42
  store i16 %43, ptr %22, !tbaa !2
  %44 = load i16, ptr %22, !tbaa !2
  %45 = icmp eq i16 %44, 0
  %46 = zext i1 %45 to i8
  store i8 %46, ptr %21, !tbaa !2
  %47 = sub i16 %1, 1
  store i16 0, ptr %20, !tbaa !2
  store i16 %47, ptr %19, !tbaa !2
  br label %b2

b2:
  %48 = load i16, ptr %20, !tbaa !2
  %49 = load i16, ptr %19, !tbaa !2
  %50 = icmp ult i16 %48, %49
  br i1 %50, label %b3, label %b5

b3:
  %51 = load i8, ptr %21, !tbaa !2, !range !28
  store i8 %51, ptr %18, !tbaa !2
  %52 = icmp ne i8 %51, 0
  br i1 %52, label %b6, label %b7

b4:
  %53 = load i16, ptr %20, !tbaa !2
  %54 = add nuw i16 %53, 1
  store i16 %54, ptr %20, !tbaa !2
  br label %b2

b5:
  %55 = load i16, ptr %24, !tbaa !2
  %56 = lshr i16 %55, 15
  %57 = icmp ne i16 %56, 0
  br i1 %57, label %b11, label %b9

b6:
  %58 = load i16, ptr %20, !tbaa !2
  %59 = mul i16 %58, 2
  %60 = getelementptr i8, ptr addrspace(1) %0, i16 %59
  %61 = load i16, ptr addrspace(1) %60
  %62 = icmp eq i16 %61, 0
  %63 = zext i1 %62 to i8
  store i8 %63, ptr %18, !tbaa !2
  br label %b7

b7:
  %64 = load i8, ptr %18, !tbaa !2, !range !29
  store i8 %64, ptr %21, !tbaa !2
  br label %b4

b8:
  %65 = load ptr, ptr %26, !tbaa !2
  %66 = load i16, ptr %25, !tbaa !2
  %67 = call cc 1005 addrspace(1) i16 @floats.put(ptr %65, i16 %66, i8 zeroext 45)
  store i16 %67, ptr %25, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  %68 = load i16, ptr %23, !tbaa !2
  %69 = icmp eq i16 %68, %3
  br i1 %69, label %b13, label %b14

b11:
  %70 = load i16, ptr %23, !tbaa !2
  %71 = icmp eq i16 %70, %3
  br i1 %71, label %b12, label %b8

b12:
  %72 = load i8, ptr %21, !tbaa !2, !range !30
  %73 = icmp ne i8 %72, 0
  br i1 %73, label %b8, label %b9

b13:
  %74 = load ptr, ptr %26, !tbaa !2
  %75 = load i16, ptr %25, !tbaa !2
  %76 = load i8, ptr %21, !tbaa !2, !range !31
  %77 = icmp ne i8 %76, 0
  br i1 %77, label %b16, label %b17

b14:
  %78 = load i16, ptr %23, !tbaa !2
  %79 = icmp eq i16 %78, 0
  br i1 %79, label %b22, label %b20

b15:
  %80 = load ptr, ptr %26, !tbaa !2
  %81 = load i16, ptr %25, !tbaa !2
  call cc 1005 addrspace(1) void @format.number(ptr %80, i16 %81)
  %82 = load ptr, ptr %28, !tbaa !2
  call cc 1005 addrspace(1) void @heap.release(ptr %82)
  ret void

b16:
  %83 = getelementptr i8, ptr @$str22, i16 6
  store ptr %83, ptr %17, !tbaa !2
  br label %b18

b17:
  %84 = getelementptr i8, ptr @$str23, i16 6
  store ptr %84, ptr %17, !tbaa !2
  br label %b18

b18:
  %85 = load ptr, ptr %17, !tbaa !2
  %86 = getelementptr i8, ptr %85, i16 -4
  %87 = load i16, ptr %86
  %88 = addrspacecast ptr %85 to ptr addrspace(1)
  store i16 %87, ptr %16, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %16, i16 2
  store i16 %87, ptr %89, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %16, i16 4
  store ptr addrspace(1) %88, ptr %90, !tbaa !2
  %91 = addrspacecast ptr %16 to ptr addrspace(1)
  %92 = call cc 1005 addrspace(1) i16 @floats.put_text(ptr %74, i16 %75, ptr addrspace(1) %91)
  store i16 %92, ptr %25, !tbaa !2
  br label %b15

b19:
  %93 = load ptr, ptr %26, !tbaa !2
  %94 = load i16, ptr %25, !tbaa !2
  %95 = getelementptr i8, ptr @$str24, i16 6
  %96 = getelementptr i8, ptr %95, i16 -4
  %97 = load i16, ptr %96
  %98 = addrspacecast ptr %95 to ptr addrspace(1)
  store i16 %97, ptr %15, !tbaa !2
  %99 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 %97, ptr %99, !tbaa !2
  %100 = getelementptr inbounds i8, ptr %15, i16 4
  store ptr addrspace(1) %98, ptr %100, !tbaa !2
  %101 = addrspacecast ptr %15 to ptr addrspace(1)
  %102 = call cc 1005 addrspace(1) i16 @floats.put_text(ptr %93, i16 %94, ptr addrspace(1) %101)
  store i16 %102, ptr %25, !tbaa !2
  br label %b21

b20:
  %103 = load ptr, ptr %27, !tbaa !2
  call cc 1005 addrspace(1) void @floats.assign(ptr %103, i16 0)
  %104 = sub i16 %1, 1
  store i16 0, ptr %14, !tbaa !2
  store i16 %104, ptr %13, !tbaa !2
  br label %b23

b21:
  br label %b15

b22:
  %105 = load i8, ptr %21, !tbaa !2, !range !32
  %106 = icmp ne i8 %105, 0
  br i1 %106, label %b19, label %b20

b23:
  %107 = load i16, ptr %14, !tbaa !2
  %108 = load i16, ptr %13, !tbaa !2
  %109 = icmp ult i16 %107, %108
  br i1 %109, label %b24, label %b26

b24:
  %110 = load ptr, ptr %27, !tbaa !2
  %111 = load i16, ptr %14, !tbaa !2
  %112 = mul i16 %111, 2
  %113 = getelementptr i8, ptr %110, i16 %112
  %114 = load i16, ptr %14, !tbaa !2
  %115 = mul i16 %114, 2
  %116 = getelementptr i8, ptr addrspace(1) %0, i16 %115
  %117 = load i16, ptr addrspace(1) %116
  store i16 %117, ptr %113
  br label %b25

b25:
  %118 = load i16, ptr %14, !tbaa !2
  %119 = add nuw i16 %118, 1
  store i16 %119, ptr %14, !tbaa !2
  br label %b23

b26:
  %120 = load ptr, ptr %27, !tbaa !2
  %121 = sub i16 %1, 1
  %122 = mul i16 %121, 2
  %123 = getelementptr i8, ptr %120, i16 %122
  %124 = load i16, ptr %22, !tbaa !2
  %125 = load i16, ptr %23, !tbaa !2
  %126 = icmp ne i16 %125, 0
  br i1 %126, label %b27, label %b28

b27:
  %127 = shl i16 1, %2
  store i16 %127, ptr %12, !tbaa !2
  br label %b29

b28:
  store i16 0, ptr %12, !tbaa !2
  br label %b29

b29:
  %128 = load i16, ptr %12, !tbaa !2
  %129 = or i16 %124, %128
  store i16 %129, ptr %123
  %130 = load i16, ptr %23, !tbaa !2
  %131 = icmp ne i16 %130, 0
  br i1 %131, label %b30, label %b31

b30:
  %132 = load i16, ptr %23, !tbaa !2
  store i16 %132, ptr %11, !tbaa !2
  br label %b32

b31:
  store i16 1, ptr %11, !tbaa !2
  br label %b32

b32:
  %133 = load i16, ptr %11, !tbaa !2
  %134 = sub i16 %133, %4
  store i16 %134, ptr %10, !tbaa !2
  %135 = load ptr, ptr %26, !tbaa !2
  %136 = getelementptr i8, ptr %135, i16 16
  store ptr %136, ptr %9, !tbaa !2
  %137 = load ptr, ptr %27, !tbaa !2
  %138 = load i16, ptr %10, !tbaa !2
  %139 = load i16, ptr %23, !tbaa !2
  %140 = icmp ugt i16 %139, 1
  %141 = zext i1 %140 to i8
  store i8 %141, ptr %7, !tbaa !2
  br i1 %140, label %b33, label %b34

b33:
  %142 = load i8, ptr %21, !tbaa !2, !range !33
  store i8 %142, ptr %7, !tbaa !2
  br label %b34

b34:
  %143 = load i8, ptr %7, !tbaa !2, !range !34
  %144 = getelementptr i8, ptr addrspace(1) %0, i16 0
  %145 = load i16, ptr addrspace(1) %144
  %146 = and i16 %145, 1
  %147 = icmp eq i16 %146, 0
  %148 = zext i1 %147 to i8
  %149 = load ptr, ptr %9, !tbaa !2
  %150 = call cc 1005 addrspace(1) i32 @floats.digits(ptr %137, i16 %138, i8 %143, i8 %148, ptr %149)
  %151 = addrspacecast ptr %8 to ptr addrspace(1)
  store i32 %150, ptr addrspace(1) %151, !tbaa !2
  %152 = load i16, ptr %8, !tbaa !2
  %153 = getelementptr inbounds i8, ptr %8, i16 2
  %154 = load i16, ptr %153, !tbaa !2
  store i16 %152, ptr %6, !tbaa !2
  store i16 %154, ptr %5, !tbaa !2
  %155 = load ptr, ptr %26, !tbaa !2
  %156 = load i16, ptr %25, !tbaa !2
  %157 = load ptr, ptr %9, !tbaa !2
  %158 = load i16, ptr %6, !tbaa !2
  %159 = load i16, ptr %5, !tbaa !2
  %160 = sub i16 %159, 1
  %161 = call cc 1005 addrspace(1) i16 @floats.decimal(ptr %155, i16 %156, ptr %157, i16 %158, i16 %160)
  store i16 %161, ptr %25, !tbaa !2
  br label %b21
}

define internal cc 1005 i16 @floats.decimal(ptr %0, i16 %1, ptr %2, i16 %3, i16 %4) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  %21 = alloca i16
  store i16 %1, ptr %21, !tbaa !2
  %22 = icmp sge i16 %4, -4
  br i1 %22, label %b5, label %b3

b2:
  %23 = icmp slt i16 %4, 0
  br i1 %23, label %b6, label %b7

b3:
  br label %b4

b4:
  %24 = load i16, ptr %21, !tbaa !2
  %25 = getelementptr i8, ptr %2, i16 0
  %26 = load i8, ptr %25
  %27 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %24, i8 zeroext %26)
  store i16 %27, ptr %21, !tbaa !2
  %28 = icmp ugt i16 %3, 1
  br i1 %28, label %b31, label %b32

b5:
  %29 = icmp slt i16 %4, 16
  br i1 %29, label %b2, label %b3

b6:
  %30 = load i16, ptr %21, !tbaa !2
  %31 = getelementptr i8, ptr @$str25, i16 6
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  %34 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 %33, ptr %20, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %20, i16 2
  store i16 %33, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %20, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %20 to ptr addrspace(1)
  %38 = call cc 1005 addrspace(1) i16 @floats.put_text(ptr %0, i16 %30, ptr addrspace(1) %37)
  store i16 %38, ptr %21, !tbaa !2
  %39 = sub i16 0, %4
  %40 = sub i16 %39, 1
  store i16 0, ptr %19, !tbaa !2
  store i16 %40, ptr %18, !tbaa !2
  br label %b9

b7:
  br label %b8

b8:
  %41 = add i16 %4, 1
  store i16 %41, ptr %15, !tbaa !2
  %42 = load i16, ptr %15, !tbaa !2
  store i16 0, ptr %14, !tbaa !2
  store i16 %42, ptr %13, !tbaa !2
  br label %b17

b9:
  %43 = load i16, ptr %19, !tbaa !2
  %44 = load i16, ptr %18, !tbaa !2
  %45 = icmp slt i16 %43, %44
  br i1 %45, label %b10, label %b12

b10:
  %46 = load i16, ptr %21, !tbaa !2
  %47 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %46, i8 zeroext 48)
  store i16 %47, ptr %21, !tbaa !2
  br label %b11

b11:
  %48 = load i16, ptr %19, !tbaa !2
  %49 = add nsw i16 %48, 1
  store i16 %49, ptr %19, !tbaa !2
  br label %b9

b12:
  store i16 0, ptr %17, !tbaa !2
  store i16 %3, ptr %16, !tbaa !2
  br label %b13

b13:
  %50 = load i16, ptr %17, !tbaa !2
  %51 = load i16, ptr %16, !tbaa !2
  %52 = icmp ult i16 %50, %51
  br i1 %52, label %b14, label %b16

b14:
  %53 = load i16, ptr %21, !tbaa !2
  %54 = load i16, ptr %17, !tbaa !2
  %55 = mul i16 %54, 1
  %56 = getelementptr i8, ptr %2, i16 %55
  %57 = load i8, ptr %56
  %58 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %53, i8 zeroext %57)
  store i16 %58, ptr %21, !tbaa !2
  br label %b15

b15:
  %59 = load i16, ptr %17, !tbaa !2
  %60 = add nuw i16 %59, 1
  store i16 %60, ptr %17, !tbaa !2
  br label %b13

b16:
  %61 = load i16, ptr %21, !tbaa !2
  ret i16 %61

b17:
  %62 = load i16, ptr %14, !tbaa !2
  %63 = load i16, ptr %13, !tbaa !2
  %64 = icmp ult i16 %62, %63
  br i1 %64, label %b18, label %b20

b18:
  %65 = load i16, ptr %21, !tbaa !2
  %66 = load i16, ptr %14, !tbaa !2
  %67 = icmp ult i16 %66, %3
  br i1 %67, label %b21, label %b22

b19:
  %68 = load i16, ptr %14, !tbaa !2
  %69 = add nuw i16 %68, 1
  store i16 %69, ptr %14, !tbaa !2
  br label %b17

b20:
  %70 = load i16, ptr %21, !tbaa !2
  %71 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %70, i8 zeroext 46)
  store i16 %71, ptr %21, !tbaa !2
  %72 = load i16, ptr %15, !tbaa !2
  %73 = icmp ule i16 %3, %72
  br i1 %73, label %b24, label %b25

b21:
  %74 = load i16, ptr %14, !tbaa !2
  %75 = mul i16 %74, 1
  %76 = getelementptr i8, ptr %2, i16 %75
  %77 = load i8, ptr %76
  store i8 %77, ptr %12, !tbaa !2
  br label %b23

b22:
  store i8 48, ptr %12, !tbaa !2
  br label %b23

b23:
  %78 = load i8, ptr %12, !tbaa !2
  %79 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %65, i8 zeroext %78)
  store i16 %79, ptr %21, !tbaa !2
  br label %b19

b24:
  %80 = load i16, ptr %21, !tbaa !2
  %81 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %80, i8 zeroext 48)
  ret i16 %81

b25:
  br label %b26

b26:
  %82 = load i16, ptr %15, !tbaa !2
  store i16 %82, ptr %11, !tbaa !2
  store i16 %3, ptr %10, !tbaa !2
  br label %b27

b27:
  %83 = load i16, ptr %11, !tbaa !2
  %84 = load i16, ptr %10, !tbaa !2
  %85 = icmp ult i16 %83, %84
  br i1 %85, label %b28, label %b30

b28:
  %86 = load i16, ptr %21, !tbaa !2
  %87 = load i16, ptr %11, !tbaa !2
  %88 = mul i16 %87, 1
  %89 = getelementptr i8, ptr %2, i16 %88
  %90 = load i8, ptr %89
  %91 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %86, i8 zeroext %90)
  store i16 %91, ptr %21, !tbaa !2
  br label %b29

b29:
  %92 = load i16, ptr %11, !tbaa !2
  %93 = add nuw i16 %92, 1
  store i16 %93, ptr %11, !tbaa !2
  br label %b27

b30:
  %94 = load i16, ptr %21, !tbaa !2
  ret i16 %94

b31:
  %95 = load i16, ptr %21, !tbaa !2
  %96 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %95, i8 zeroext 46)
  store i16 %96, ptr %21, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  store i16 %3, ptr %8, !tbaa !2
  br label %b34

b32:
  br label %b33

b33:
  %97 = load i16, ptr %21, !tbaa !2
  %98 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %97, i8 zeroext 101)
  store i16 %98, ptr %21, !tbaa !2
  %99 = load i16, ptr %21, !tbaa !2
  %100 = icmp slt i16 %4, 0
  br i1 %100, label %b38, label %b39

b34:
  %101 = load i16, ptr %9, !tbaa !2
  %102 = load i16, ptr %8, !tbaa !2
  %103 = icmp ult i16 %101, %102
  br i1 %103, label %b35, label %b37

b35:
  %104 = load i16, ptr %21, !tbaa !2
  %105 = load i16, ptr %9, !tbaa !2
  %106 = mul i16 %105, 1
  %107 = getelementptr i8, ptr %2, i16 %106
  %108 = load i8, ptr %107
  %109 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %104, i8 zeroext %108)
  store i16 %109, ptr %21, !tbaa !2
  br label %b36

b36:
  %110 = load i16, ptr %9, !tbaa !2
  %111 = add nuw i16 %110, 1
  store i16 %111, ptr %9, !tbaa !2
  br label %b34

b37:
  br label %b33

b38:
  store i8 45, ptr %7, !tbaa !2
  br label %b40

b39:
  store i8 43, ptr %7, !tbaa !2
  br label %b40

b40:
  %112 = load i8, ptr %7, !tbaa !2
  %113 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %99, i8 zeroext %112)
  store i16 %113, ptr %21, !tbaa !2
  %114 = icmp slt i16 %4, 0
  br i1 %114, label %b41, label %b42

b41:
  %115 = sub i16 0, %4
  store i16 %115, ptr %6, !tbaa !2
  br label %b43

b42:
  store i16 %4, ptr %6, !tbaa !2
  br label %b43

b43:
  %116 = load i16, ptr %6, !tbaa !2
  store i16 %116, ptr %5, !tbaa !2
  %117 = load i16, ptr %5, !tbaa !2
  %118 = icmp uge i16 %117, 100
  br i1 %118, label %b44, label %b45

b44:
  %119 = load i16, ptr %21, !tbaa !2
  %120 = load i16, ptr %5, !tbaa !2
  %121 = udiv i16 %120, 100
  %122 = trunc i16 %121 to i8
  %123 = zext i8 %122 to i16
  %124 = add i16 48, %123
  %125 = trunc i16 %124 to i8
  %126 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %119, i8 zeroext %125)
  store i16 %126, ptr %21, !tbaa !2
  br label %b46

b45:
  br label %b46

b46:
  %127 = load i16, ptr %21, !tbaa !2
  %128 = load i16, ptr %5, !tbaa !2
  %129 = udiv i16 %128, 10
  %130 = urem i16 %129, 10
  %131 = trunc i16 %130 to i8
  %132 = zext i8 %131 to i16
  %133 = add i16 48, %132
  %134 = trunc i16 %133 to i8
  %135 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %127, i8 zeroext %134)
  store i16 %135, ptr %21, !tbaa !2
  %136 = load i16, ptr %21, !tbaa !2
  %137 = load i16, ptr %5, !tbaa !2
  %138 = urem i16 %137, 10
  %139 = trunc i16 %138 to i8
  %140 = zext i8 %139 to i16
  %141 = add i16 48, %140
  %142 = trunc i16 %141 to i8
  %143 = call cc 1005 addrspace(1) i16 @floats.put(ptr %0, i16 %136, i8 zeroext %142)
  ret i16 %143
}

define cc 1005 void @N$PR8(double %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = alloca double
  store double %0, ptr %2, !tbaa !2
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @floats.print_float(ptr addrspace(1) %4, i16 4, i16 4, i16 2047, i16 1075)
  ret void
}

define cc 1005 void @N$PR4(float %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca ptr addrspace(1)
  %2 = alloca float
  store float %0, ptr %2, !tbaa !2
  %3 = addrspacecast ptr %2 to ptr addrspace(1)
  store ptr addrspace(1) %3, ptr %1, !tbaa !2
  %4 = load ptr addrspace(1), ptr %1, !tbaa !2
  call cc 1005 addrspace(1) void @floats.print_float(ptr addrspace(1) %4, i16 2, i16 7, i16 255, i16 150)
  ret void
}

declare i32 @LL$WRITE_FILE(i16, ptr addrspace(1), i16) addrspace(1)

declare void @LL$EXIT(i8) addrspace(1)

declare ptr @LL$MORE(i16) addrspace(1)

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
!22 = !{i8 0, i8 2}
!23 = !{i8 0, i8 2}
!24 = !{i8 0, i8 2}
!25 = !{i8 0, i8 2}
!26 = !{i8 0, i8 2}
!27 = !{i8 0, i8 2}
!28 = !{i8 0, i8 2}
!29 = !{i8 0, i8 2}
!30 = !{i8 0, i8 2}
!31 = !{i8 0, i8 2}
!32 = !{i8 0, i8 2}
!33 = !{i8 0, i8 2}
!34 = !{i8 0, i8 2}
