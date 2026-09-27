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
  %1 = getelementptr i8, ptr @$str10, i16 6
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  %5 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %4, i16 %3)
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = load i16, ptr addrspace(1) %0
  %9 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %7, i16 %8)
  %10 = getelementptr i8, ptr @$str11, i16 6
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  %14 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %13, i16 %12)
  call addrspace(1) void @N$OEXT(i8 -1)
  ret void
}

define internal void @errors.say(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  %3 = load i16, ptr addrspace(1) %0
  %4 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %2, i16 %3)
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

define internal ptr @heap.header(ptr %0) addrspace(1) memory(none) willreturn {
b1:
  ret ptr %0
}

define internal i16 @heap.size_of(ptr %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = and i16 %1, -4
  ret i16 %2
}

define internal ptr @heap.next(ptr %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  ret ptr %1
}

define internal ptr @heap.previous(ptr %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 4
  ret ptr %1
}

define internal i16 @heap.class_of(i16 %0) addrspace(1) memory(none) {
b1:
  %1 = lshr i16 %0, 4
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %5, %b3 ]
  %3 = phi i16 [ %1, %b1 ], [ %6, %b3 ]
  %4 = icmp ne i16 %3, 0
  br i1 %4, label %b3, label %b4

b3:
  %5 = add i16 %2, 1
  %6 = lshr i16 %3, 1
  br label %b2

b4:
  ret i16 %2
}

define internal i16 @heap.rounded(i16 %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %2 = icmp ugt i16 %0, -16
  br i1 %2, label %b2, label %b4

b2:
  %3 = getelementptr i8, ptr @$str17, i16 6
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 %5, ptr %1, !tbaa !2
  %7 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %5, ptr %7, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %6, ptr %8, !tbaa !2
  %9 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %9)
  br label %b4

b4:
  %10 = add i16 %0, 5
  %11 = and i16 %10, -4
  %12 = icmp ugt i16 %11, 8
  br i1 %12, label %b7, label %b6

b6:
  br label %b7

b7:
  %13 = phi i16 [ %11, %b4 ], [ 8, %b6 ]
  ret i16 %13
}

define internal void @heap.unlink(ptr %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr %0, i16 2
  %2 = load ptr, ptr %1
  %3 = getelementptr i8, ptr %0, i16 4
  %4 = load ptr, ptr %3
  %5 = icmp eq ptr %4, null
  br i1 %5, label %6, label %b3

6:
  %7 = load i16, ptr %0
  %8 = and i16 %7, -4
  %9 = lshr i16 %8, 4
  br label %10

10:
  %11 = phi i16 [ 0, %6 ], [ %15, %14 ]
  %12 = phi i16 [ %9, %6 ], [ %16, %14 ]
  %13 = icmp ne i16 %12, 0
  br i1 %13, label %14, label %17

14:
  %15 = add i16 %11, 1
  %16 = lshr i16 %12, 1
  br label %10

17:
  %18 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %11
  store ptr %2, ptr %18, !tbaa !2
  %19 = icmp eq ptr %2, null
  br i1 %19, label %b5, label %b4

b3:
  %20 = getelementptr i8, ptr %4, i16 2
  store ptr %2, ptr %20
  br label %b4

b4:
  %21 = icmp eq ptr %2, null
  %22 = sext i1 %21 to i8
  %23 = xor i8 %22, -1
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %29, label %b10

b5:
  %25 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %26 = shl i16 1, %11
  %27 = xor i16 %26, -1
  %28 = and i16 %25, %27
  store i16 %28, ptr @$var_heap.occupied, !tbaa !2
  br label %b4

29:
  %30 = getelementptr i8, ptr %2, i16 4
  store ptr %4, ptr %30
  br label %b10

b10:
  ret void
}

define internal void @heap.insert(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = load i16, ptr %0
  %3 = and i16 %2, 2
  %4 = or i16 %1, %3
  store i16 %4, ptr %0
  %5 = add i16 %1, -2
  %6 = getelementptr i8, ptr %0, i16 %5
  store i16 %1, ptr %6
  %7 = getelementptr i8, ptr %0, i16 %1
  %8 = load i16, ptr %7
  %9 = and i16 %8, -3
  store i16 %9, ptr %7
  %10 = lshr i16 %1, 4
  br label %11

11:
  %12 = phi i16 [ 0, %b1 ], [ %16, %15 ]
  %13 = phi i16 [ %10, %b1 ], [ %17, %15 ]
  %14 = icmp ne i16 %13, 0
  br i1 %14, label %15, label %18

15:
  %16 = add i16 %12, 1
  %17 = lshr i16 %13, 1
  br label %11

18:
  %19 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %12
  %20 = load ptr, ptr %19, !tbaa !2
  %21 = getelementptr i8, ptr %0, i16 2
  store ptr %20, ptr %21
  %22 = getelementptr i8, ptr %0, i16 4
  store ptr null, ptr %22
  %23 = icmp eq ptr %20, null
  %24 = sext i1 %23 to i8
  %25 = xor i8 %24, -1
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %27, label %b4

27:
  %28 = getelementptr i8, ptr %20, i16 4
  store ptr %0, ptr %28
  br label %b4

b4:
  store ptr %0, ptr %19, !tbaa !2
  %29 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %30 = shl i16 1, %12
  %31 = or i16 %29, %30
  store i16 %31, ptr @$var_heap.occupied, !tbaa !2
  ret void
}

define internal ptr @heap.fit(i16 %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = lshr i16 %0, 4
  br label %4

4:
  %5 = phi i16 [ 0, %b1 ], [ %9, %8 ]
  %6 = phi i16 [ %3, %b1 ], [ %10, %8 ]
  %7 = icmp ne i16 %6, 0
  br i1 %7, label %8, label %11

8:
  %9 = add i16 %5, 1
  %10 = lshr i16 %6, 1
  br label %4

11:
  %12 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %5
  %13 = load ptr, ptr %12, !tbaa !2
  br label %b2

b2:
  %14 = phi ptr [ %13, %11 ], [ %27, %25 ]
  %15 = icmp eq ptr %14, null
  %16 = sext i1 %15 to i8
  %17 = xor i8 %16, -1
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %19, label %b4

19:
  %20 = load i16, ptr %14
  %21 = and i16 %20, -4
  %22 = icmp uge i16 %21, %0
  br i1 %22, label %b5, label %25

b4:
  %23 = load i16, ptr @$var_heap.occupied, !tbaa !2
  %24 = icmp ult i16 %5, 16
  br i1 %24, label %b8, label %33

b5:
  ret ptr %14

25:
  %26 = getelementptr i8, ptr %14, i16 2
  %27 = load ptr, ptr %26
  br label %b2

b8:
  %28 = shl i16 2, %5
  %29 = add i16 %28, -1
  %30 = xor i16 %29, -1
  %31 = and i16 %23, %30
  %32 = icmp eq i16 %31, 0
  br i1 %32, label %b10, label %b12

33:
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %34 = getelementptr i8, ptr @$str14, i16 6
  %35 = getelementptr i8, ptr %34, i16 -4
  %36 = load i16, ptr %35
  %37 = addrspacecast ptr %34 to ptr addrspace(1)
  store i16 %36, ptr %2
  %38 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %36, ptr %38
  %39 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %37, ptr %39
  %40 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %40)
  unreachable

b10:
  ret ptr null

b12:
  br label %b13

b13:
  %41 = phi i16 [ 0, %b12 ], [ %46, %b14 ]
  %42 = phi i16 [ %31, %b12 ], [ %45, %b14 ]
  %43 = and i16 %42, 1
  %44 = icmp eq i16 %43, 0
  br i1 %44, label %b14, label %b15

b14:
  %45 = lshr i16 %42, 1
  %46 = add i16 %41, 1
  br label %b13

b15:
  %47 = icmp ult i16 %41, 13
  br i1 %47, label %b16, label %50

b16:
  %48 = getelementptr inbounds ptr, ptr @$var_heap.heads, i16 %41
  %49 = load ptr, ptr %48, !tbaa !2
  ret ptr %49

50:
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %51 = getelementptr i8, ptr @$str15, i16 6
  %52 = getelementptr i8, ptr %51, i16 -4
  %53 = load i16, ptr %52
  %54 = addrspacecast ptr %51 to ptr addrspace(1)
  store i16 %53, ptr %1
  %55 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %53, ptr %55
  %56 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %54, ptr %56
  %57 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %57)
  unreachable
}

define internal void @heap.grow(i16 %0) addrspace(1) {
b1:
  %1 = icmp ugt i16 %0, 1024
  br i1 %1, label %b4, label %b3

b3:
  br label %b4

b4:
  %2 = phi i16 [ %0, %b1 ], [ 1024, %b3 ]
  %3 = load ptr, ptr @$var_heap.end, !tbaa !2
  %4 = icmp eq ptr %3, null
  br i1 %4, label %b7, label %b6

b6:
  br label %b7

b7:
  %5 = phi i16 [ 2, %b4 ], [ 0, %b6 ]
  %6 = add i16 %2, %5
  %7 = call addrspace(1) ptr @N$OMEM(i16 %6)
  %8 = icmp eq ptr %7, null
  br i1 %8, label %b8, label %b10

b8:
  %9 = add i16 %0, %5
  %10 = call addrspace(1) ptr @N$OMEM(i16 %9)
  %11 = icmp eq ptr %10, null
  br i1 %11, label %b11, label %b10

b10:
  %12 = phi i16 [ %2, %b7 ], [ %0, %b8 ]
  %13 = phi ptr [ %7, %b7 ], [ %10, %b8 ]
  br i1 %4, label %b16, label %b15

b11:
  ret void

b15:
  %14 = load ptr, ptr @$var_heap.end, !tbaa !2
  br label %b16

b16:
  %15 = phi ptr [ %13, %b10 ], [ %14, %b15 ]
  br i1 %4, label %16, label %b19

16:
  store i16 2, ptr %15
  br label %b19

b19:
  %17 = getelementptr i8, ptr %15, i16 %12
  store ptr %17, ptr @$var_heap.end, !tbaa !2
  store i16 3, ptr %17
  %18 = or i16 %12, 1
  %19 = load i16, ptr %15
  %20 = and i16 %19, 2
  %21 = or i16 %18, %20
  store i16 %21, ptr %15
  %22 = getelementptr i8, ptr %15, i16 2
  call addrspace(1) void @heap.release(ptr %22)
  ret void
}

define internal void @heap.trim(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = load i16, ptr %0
  %3 = and i16 %2, -4
  %4 = sub i16 %3, %1
  %5 = icmp ult i16 %4, 8
  br i1 %5, label %b2, label %b3

b2:
  ret void

b3:
  %6 = getelementptr i8, ptr %0, i16 %1
  %7 = or i16 %4, 3
  store i16 %7, ptr %6
  %8 = load i16, ptr %0
  %9 = and i16 %8, 3
  %10 = or i16 %1, %9
  store i16 %10, ptr %0
  %11 = getelementptr i8, ptr %6, i16 2
  call addrspace(1) void @heap.release(ptr %11)
  ret void
}

define internal ptr @heap.allocate(i16 %0) addrspace(1) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  %3 = icmp ugt i16 %0, -16
  br i1 %3, label %4, label %12

4:
  %5 = getelementptr i8, ptr @$str17, i16 6
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = addrspacecast ptr %5 to ptr addrspace(1)
  store i16 %7, ptr %1
  %9 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %7, ptr %9
  %10 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %8, ptr %10
  %11 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %11)
  br label %12

12:
  %13 = add i16 %0, 5
  %14 = and i16 %13, -4
  %15 = icmp ugt i16 %14, 8
  br i1 %15, label %17, label %16

16:
  br label %17

17:
  %18 = phi i16 [ %14, %12 ], [ 8, %16 ]
  %19 = call addrspace(1) ptr @heap.fit(i16 %18)
  %20 = icmp eq ptr %19, null
  br i1 %20, label %b2, label %b4

b2:
  call addrspace(1) void @heap.grow(i16 %18)
  %21 = call addrspace(1) ptr @heap.fit(i16 %18)
  %22 = icmp eq ptr %21, null
  br i1 %22, label %b5, label %b4

b4:
  %23 = phi ptr [ %19, %17 ], [ %21, %b5 ], [ %21, %b2 ]
  call addrspace(1) void @heap.unlink(ptr %23)
  %24 = load i16, ptr %23
  %25 = and i16 %24, -4
  %26 = or i16 %25, 1
  %27 = and i16 %24, 2
  %28 = or i16 %26, %27
  store i16 %28, ptr %23
  %29 = getelementptr i8, ptr %23, i16 %25
  %30 = load i16, ptr %29
  %31 = or i16 %30, 2
  store i16 %31, ptr %29
  %32 = load i16, ptr %23
  %33 = and i16 %32, -4
  %34 = sub i16 %33, %18
  %35 = icmp ult i16 %34, 8
  br i1 %35, label %43, label %36

36:
  %37 = getelementptr i8, ptr %23, i16 %18
  %38 = or i16 %34, 3
  store i16 %38, ptr %37
  %39 = load i16, ptr %23
  %40 = and i16 %39, 3
  %41 = or i16 %18, %40
  store i16 %41, ptr %23
  %42 = getelementptr i8, ptr %37, i16 2
  call addrspace(1) void @heap.release(ptr %42)
  br label %43

43:
  %44 = getelementptr i8, ptr %23, i16 2
  ret ptr %44

b5:
  %45 = getelementptr i8, ptr @$str17, i16 6
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %2, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %47, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %48, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %51)
  br label %b4
}

define internal void @heap.release(ptr %0) addrspace(1) {
b1:
  %1 = icmp eq ptr %0, null
  br i1 %1, label %b2, label %b3

b2:
  ret void

b3:
  %2 = getelementptr i8, ptr %0, i16 -2
  %3 = load i16, ptr %2
  %4 = and i16 %3, -4
  %5 = getelementptr i8, ptr %2, i16 %4
  %6 = load i16, ptr %5
  %7 = and i16 %6, 1
  %8 = icmp eq i16 %7, 0
  br i1 %8, label %b5, label %b7

b5:
  call addrspace(1) void @heap.unlink(ptr %5)
  %9 = load i16, ptr %5
  %10 = and i16 %9, -4
  %11 = add i16 %4, %10
  br label %b7

b7:
  %12 = phi i16 [ %11, %b5 ], [ %4, %b3 ]
  %13 = load i16, ptr %2
  %14 = and i16 %13, 2
  %15 = icmp eq i16 %14, 0
  br i1 %15, label %b8, label %b10

b8:
  %16 = getelementptr i8, ptr %2, i16 -2
  %17 = load i16, ptr %16
  %18 = sub i16 0, %17
  %19 = getelementptr i8, ptr %2, i16 %18
  call addrspace(1) void @heap.unlink(ptr %19)
  %20 = add i16 %12, %17
  br label %b10

b10:
  %21 = phi ptr [ %19, %b8 ], [ %2, %b7 ]
  %22 = phi i16 [ %20, %b8 ], [ %12, %b7 ]
  call addrspace(1) void @heap.insert(ptr %21, i16 %22)
  ret void
}

define internal i8 @heap.resize(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  %3 = getelementptr i8, ptr %0, i16 -2
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %4 = icmp ugt i16 %1, -16
  br i1 %4, label %5, label %13

5:
  %6 = getelementptr i8, ptr @$str17, i16 6
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr %2
  %10 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %8, ptr %10
  %11 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %9, ptr %11
  %12 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %12)
  br label %13

13:
  %14 = add i16 %1, 5
  %15 = and i16 %14, -4
  %16 = icmp ugt i16 %15, 8
  br i1 %16, label %18, label %17

17:
  br label %18

18:
  %19 = phi i16 [ %15, %13 ], [ 8, %17 ]
  %20 = load i16, ptr %3
  %21 = and i16 %20, -4
  %22 = icmp ult i16 %21, %19
  br i1 %22, label %b2, label %28

b2:
  %23 = getelementptr i8, ptr %3, i16 %21
  %24 = load i16, ptr %23
  %25 = and i16 %24, 1
  %26 = icmp ne i16 %25, 0
  %27 = sext i1 %26 to i8
  br i1 %26, label %b6, label %41

28:
  %29 = load i16, ptr %3
  %30 = and i16 %29, -4
  %31 = sub i16 %30, %19
  %32 = icmp ult i16 %31, 8
  br i1 %32, label %40, label %33

33:
  %34 = getelementptr i8, ptr %3, i16 %19
  %35 = or i16 %31, 3
  store i16 %35, ptr %34
  %36 = load i16, ptr %3
  %37 = and i16 %36, 3
  %38 = or i16 %19, %37
  store i16 %38, ptr %3
  %39 = getelementptr i8, ptr %34, i16 2
  call addrspace(1) void @heap.release(ptr %39)
  br label %40

40:
  ret i8 -1

41:
  %42 = load i16, ptr %23
  %43 = and i16 %42, -4
  %44 = add i16 %21, %43
  %45 = icmp ult i16 %44, %19
  %46 = sext i1 %45 to i8
  br label %b6

b6:
  %47 = phi i8 [ %27, %b2 ], [ %46, %41 ]
  %48 = icmp ne i8 %47, 0
  br i1 %48, label %b7, label %b8

b7:
  ret i8 0

b8:
  call addrspace(1) void @heap.unlink(ptr %23)
  %49 = load i16, ptr %23
  %50 = and i16 %49, -4
  %51 = add i16 %21, %50
  %52 = load i16, ptr %3
  %53 = and i16 %52, 3
  %54 = or i16 %51, %53
  store i16 %54, ptr %3
  %55 = getelementptr i8, ptr %3, i16 %51
  %56 = load i16, ptr %55
  %57 = or i16 %56, 2
  store i16 %57, ptr %55
  br label %28
}

define internal ptr @buffers.flags(ptr %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 -6
  ret ptr %1
}

define internal ptr @buffers.length(ptr %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 -4
  ret ptr %1
}

define internal ptr @buffers.capacity(ptr %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = getelementptr i8, ptr %0, i16 -2
  ret ptr %1
}

define internal void @buffers.copy(ptr %0, ptr addrspace(1) %1, i16 %2) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  br label %b2

b2:
  %3 = phi i16 [ 0, %b1 ], [ %8, %b3 ]
  %4 = icmp ult i16 %3, %2
  br i1 %4, label %b3, label %b5

b3:
  %5 = getelementptr i8, ptr %0, i16 %3
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 %3
  %7 = load i8, ptr addrspace(1) %6
  store i8 %7, ptr %5
  %8 = add i16 %3, 1
  br label %b2

b5:
  ret void
}

define internal i8 @buffers.fits(i16 %0, i16 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = icmp eq i16 %1, 0
  %3 = sext i1 %2 to i8
  br i1 %2, label %b3, label %b2

b2:
  %4 = udiv i16 -23, %1
  %5 = icmp ule i16 %0, %4
  %6 = sext i1 %5 to i8
  br label %b3

b3:
  %7 = phi i8 [ %3, %b1 ], [ %6, %b2 ]
  ret i8 %7
}

define internal ptr @buffers.allocate(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %3 = icmp eq i16 %1, 0
  %4 = sext i1 %3 to i8
  br i1 %3, label %9, label %5

5:
  %6 = udiv i16 -23, %1
  %7 = icmp ule i16 %0, %6
  %8 = sext i1 %7 to i8
  br label %9

9:
  %10 = phi i8 [ %4, %b1 ], [ %8, %5 ]
  %11 = xor i8 %10, -1
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b2, label %b4

b2:
  %13 = getelementptr i8, ptr @$str17, i16 6
  %14 = getelementptr i8, ptr %13, i16 -4
  %15 = load i16, ptr %14
  %16 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 %15, ptr %2, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %15, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %16, ptr %18, !tbaa !2
  %19 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %19)
  br label %b4

b4:
  %20 = mul i16 %0, %1
  %21 = add i16 %20, 7
  %22 = call addrspace(1) ptr @heap.allocate(i16 %21)
  %23 = getelementptr i8, ptr %22, i16 6
  %24 = getelementptr i8, ptr %23, i16 -6
  store i8 1, ptr %24
  %25 = getelementptr i8, ptr %24, i16 1
  store i8 0, ptr %25
  %26 = getelementptr i8, ptr %23, i16 -4
  store i16 0, ptr %26
  %27 = getelementptr i8, ptr %23, i16 -2
  store i16 %0, ptr %27
  store i8 0, ptr %23
  ret ptr %23
}

define ptr @N$BRES(ptr %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = getelementptr i8, ptr %0, i16 -2
  %6 = load i16, ptr %5
  %7 = icmp ult i16 %1, %4
  br i1 %7, label %b4, label %b3

b3:
  br label %b4

b4:
  %8 = phi i16 [ %4, %b1 ], [ %1, %b3 ]
  %9 = getelementptr i8, ptr %0, i16 -6
  %10 = load i8, ptr %9
  %11 = zext i8 %10 to i16
  %12 = and i16 %11, 9
  %13 = icmp eq i16 %12, 1
  %14 = sext i1 %13 to i8
  br i1 %13, label %b5, label %b6

b5:
  %15 = icmp uge i16 %6, %8
  %16 = sext i1 %15 to i8
  br label %b6

b6:
  %17 = phi i8 [ %14, %b4 ], [ %16, %b5 ]
  %18 = icmp ne i8 %17, 0
  br i1 %18, label %b7, label %b8

b7:
  ret ptr %0

b8:
  %19 = icmp ult i16 %6, -32768
  br i1 %19, label %b10, label %b12

b10:
  %20 = shl i16 %6, 1
  br label %b12

b12:
  %21 = phi i16 [ %20, %b10 ], [ %6, %b8 ]
  %22 = icmp ult i16 %8, %21
  %23 = sext i1 %22 to i8
  br i1 %22, label %24, label %b14

24:
  %25 = icmp eq i16 %2, 0
  %26 = sext i1 %25 to i8
  br i1 %25, label %31, label %27

27:
  %28 = udiv i16 -23, %2
  %29 = icmp ule i16 %21, %28
  %30 = sext i1 %29 to i8
  br label %31

31:
  %32 = phi i8 [ %26, %24 ], [ %30, %27 ]
  br label %b14

b14:
  %33 = phi i8 [ %23, %b12 ], [ %32, %31 ]
  %34 = icmp ne i8 %33, 0
  br i1 %34, label %b17, label %b16

b16:
  br label %b17

b17:
  %35 = phi i16 [ %21, %b14 ], [ %8, %b16 ]
  br i1 %13, label %36, label %b19

36:
  %37 = icmp eq i16 %2, 0
  %38 = sext i1 %37 to i8
  br i1 %37, label %43, label %39

39:
  %40 = udiv i16 -23, %2
  %41 = icmp ule i16 %35, %40
  %42 = sext i1 %41 to i8
  br label %43

43:
  %44 = phi i8 [ %38, %36 ], [ %42, %39 ]
  br label %b19

b19:
  %45 = phi i8 [ %14, %b17 ], [ %44, %43 ]
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b20, label %b21

b20:
  %47 = mul i16 %35, %2
  %48 = add i16 %47, 7
  %49 = call addrspace(1) i8 @heap.resize(ptr %9, i16 %48)
  br label %b21

b21:
  %50 = phi i8 [ %45, %b19 ], [ %49, %b20 ]
  %51 = icmp ne i8 %50, 0
  br i1 %51, label %52, label %b23

52:
  store i16 %35, ptr %5
  ret ptr %0

b23:
  %53 = call addrspace(1) ptr @buffers.allocate(i16 %35, i16 %2)
  %54 = addrspacecast ptr %0 to ptr addrspace(1)
  %55 = mul i16 %4, %2
  %56 = add i16 %55, 1
  br label %57

57:
  %58 = phi i16 [ 0, %b23 ], [ %64, %60 ]
  %59 = icmp ult i16 %58, %56
  br i1 %59, label %60, label %65

60:
  %61 = getelementptr i8, ptr %53, i16 %58
  %62 = getelementptr i8, ptr addrspace(1) %54, i16 %58
  %63 = load i8, ptr addrspace(1) %62
  store i8 %63, ptr %61
  %64 = add i16 %58, 1
  br label %57

65:
  %66 = getelementptr i8, ptr %53, i16 -4
  store i16 %4, ptr %66
  %67 = icmp eq ptr %0, null
  %68 = sext i1 %67 to i8
  %69 = xor i8 %68, -1
  %70 = icmp ne i8 %69, 0
  br i1 %70, label %71, label %77

71:
  %72 = load i8, ptr %9
  %73 = zext i8 %72 to i16
  %74 = and i16 %73, 1
  %75 = icmp ne i16 %74, 0
  %76 = sext i1 %75 to i8
  br label %77

77:
  %78 = phi i8 [ %69, %65 ], [ %76, %71 ]
  %79 = icmp ne i8 %78, 0
  br i1 %79, label %80, label %81

80:
  call addrspace(1) void @heap.release(ptr %9)
  br label %81

81:
  ret ptr %53
}

define void @N$BDRP(ptr %0) addrspace(1) {
b1:
  %1 = icmp eq ptr %0, null
  %2 = sext i1 %1 to i8
  %3 = xor i8 %2, -1
  %4 = icmp ne i8 %3, 0
  br i1 %4, label %b2, label %b3

b2:
  %5 = getelementptr i8, ptr %0, i16 -6
  %6 = load i8, ptr %5
  %7 = zext i8 %6 to i16
  %8 = and i16 %7, 1
  %9 = icmp ne i16 %8, 0
  %10 = sext i1 %9 to i8
  br label %b3

b3:
  %11 = phi i8 [ %3, %b1 ], [ %10, %b2 ]
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b4, label %b6

b4:
  %13 = getelementptr i8, ptr %0, i16 -6
  call addrspace(1) void @heap.release(ptr %13)
  br label %b6

b6:
  ret void
}

define ptr @N$BGRW(ptr %0, i16 %1, i16 %2) addrspace(1) {
b1:
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = add i16 %4, %1
  %6 = call addrspace(1) ptr @N$BRES(ptr %0, i16 %5, i16 %2)
  %7 = getelementptr i8, ptr %6, i16 -4
  store i16 %5, ptr %7
  ret ptr %6
}

define i16 @N$BSHR(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = icmp ult i16 %4, %1
  br i1 %5, label %b2, label %13

b2:
  %6 = getelementptr i8, ptr @$str18, i16 6
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 %8, ptr %2, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %8, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %12)
  br label %13

13:
  %14 = sub i16 %4, %1
  store i16 %14, ptr %3
  ret i16 %14
}

define ptr @N$BCLN(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = getelementptr i8, ptr %0, i16 -4
  %3 = load i16, ptr %2
  %4 = call addrspace(1) ptr @buffers.allocate(i16 %3, i16 %1)
  %5 = addrspacecast ptr %0 to ptr addrspace(1)
  %6 = mul i16 %3, %1
  %7 = add i16 %6, 1
  br label %8

8:
  %9 = phi i16 [ 0, %b1 ], [ %15, %11 ]
  %10 = icmp ult i16 %9, %7
  br i1 %10, label %11, label %16

11:
  %12 = getelementptr i8, ptr %4, i16 %9
  %13 = getelementptr i8, ptr addrspace(1) %5, i16 %9
  %14 = load i8, ptr addrspace(1) %13
  store i8 %14, ptr %12
  %15 = add i16 %9, 1
  br label %8

16:
  %17 = getelementptr i8, ptr %4, i16 -4
  store i16 %3, ptr %17
  ret ptr %4
}

define ptr @N$DRES(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = getelementptr i8, ptr %0, i16 -4
  %3 = load i16, ptr %2
  %4 = getelementptr i8, ptr %0, i16 -2
  %5 = load i16, ptr %4
  %6 = add i16 %5, 1
  %7 = shl i16 %6, 2
  %8 = mul i16 %3, 3
  %9 = icmp ule i16 %7, %8
  br i1 %9, label %b2, label %b3

b2:
  ret ptr %0

b3:
  %10 = icmp ult i16 %3, 8
  br i1 %10, label %b7, label %b6

b6:
  %11 = shl i16 %3, 1
  br label %b7

b7:
  %12 = phi i16 [ 8, %b3 ], [ %11, %b6 ]
  %13 = add i16 %12, -1
  %14 = call addrspace(1) ptr @buffers.allocate(i16 %12, i16 %1)
  %15 = mul i16 %12, %1
  br label %b8

b8:
  %16 = phi i16 [ 0, %b7 ], [ %19, %b9 ]
  %17 = icmp ult i16 %16, %15
  br i1 %17, label %b9, label %b11

b9:
  %18 = getelementptr i8, ptr %14, i16 %16
  store i8 0, ptr %18
  %19 = add i16 %16, 1
  br label %b8

b11:
  br label %b12

b12:
  %20 = phi i16 [ 0, %b11 ], [ %26, %b14 ]
  %21 = icmp ult i16 %20, %3
  br i1 %21, label %b13, label %27

b13:
  %22 = mul i16 %20, %1
  %23 = getelementptr i8, ptr %0, i16 %22
  %24 = load i16, ptr %23
  %25 = icmp eq i16 %24, 0
  br i1 %25, label %b14, label %b17

b14:
  %26 = add i16 %20, 1
  br label %b12

27:
  %28 = getelementptr i8, ptr %14, i16 -4
  store i16 %12, ptr %28
  %29 = getelementptr i8, ptr %14, i16 -2
  store i16 %5, ptr %29
  %30 = icmp eq ptr %0, null
  %31 = sext i1 %30 to i8
  %32 = xor i8 %31, -1
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %34, label %41

34:
  %35 = getelementptr i8, ptr %0, i16 -6
  %36 = load i8, ptr %35
  %37 = zext i8 %36 to i16
  %38 = and i16 %37, 1
  %39 = icmp ne i16 %38, 0
  %40 = sext i1 %39 to i8
  br label %41

41:
  %42 = phi i8 [ %32, %27 ], [ %40, %34 ]
  %43 = icmp ne i8 %42, 0
  br i1 %43, label %44, label %46

44:
  %45 = getelementptr i8, ptr %0, i16 -6
  call addrspace(1) void @heap.release(ptr %45)
  br label %46

46:
  ret ptr %14

b17:
  %47 = and i16 %24, %13
  br label %b19

b19:
  %48 = phi i16 [ %47, %b17 ], [ %54, %b20 ]
  %49 = mul i16 %48, %1
  %50 = getelementptr i8, ptr %14, i16 %49
  %51 = load i16, ptr %50
  %52 = icmp ne i16 %51, 0
  br i1 %52, label %b20, label %b21

b20:
  %53 = add i16 %48, 1
  %54 = and i16 %53, %13
  br label %b19

b21:
  %55 = addrspacecast ptr %23 to ptr addrspace(1)
  br label %56

56:
  %57 = phi i16 [ 0, %b21 ], [ %63, %59 ]
  %58 = icmp ult i16 %57, %1
  br i1 %58, label %59, label %b14

59:
  %60 = getelementptr i8, ptr %50, i16 %57
  %61 = getelementptr i8, ptr addrspace(1) %55, i16 %57
  %62 = load i8, ptr addrspace(1) %61
  store i8 %62, ptr %60
  %63 = add i16 %57, 1
  br label %56
}

define internal ptr @strings.append_bytes(ptr %0, ptr addrspace(1) %1, i16 %2) addrspace(1) {
b1:
  %3 = getelementptr i8, ptr %0, i16 -4
  %4 = load i16, ptr %3
  %5 = add i16 %4, %2
  %6 = call addrspace(1) ptr @N$BRES(ptr %0, i16 %5, i16 1)
  %7 = getelementptr i8, ptr %6, i16 %4
  br label %8

8:
  %9 = phi i16 [ 0, %b1 ], [ %15, %11 ]
  %10 = icmp ult i16 %9, %2
  br i1 %10, label %11, label %16

11:
  %12 = getelementptr i8, ptr %7, i16 %9
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 %9
  %14 = load i8, ptr addrspace(1) %13
  store i8 %14, ptr %12
  %15 = add i16 %9, 1
  br label %8

16:
  %17 = getelementptr i8, ptr %6, i16 -4
  store i16 %5, ptr %17
  %18 = getelementptr i8, ptr %6, i16 %5
  store i8 0, ptr %18
  ret ptr %6
}

define ptr @N$TAPP(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  %3 = getelementptr i8, ptr %1, i16 -4
  %4 = load i16, ptr %3
  %5 = getelementptr i8, ptr %0, i16 -4
  %6 = load i16, ptr %5
  %7 = add i16 %6, %4
  %8 = call addrspace(1) ptr @N$BRES(ptr %0, i16 %7, i16 1)
  %9 = getelementptr i8, ptr %8, i16 %6
  br label %10

10:
  %11 = phi i16 [ 0, %b1 ], [ %17, %13 ]
  %12 = icmp ult i16 %11, %4
  br i1 %12, label %13, label %18

13:
  %14 = getelementptr i8, ptr %9, i16 %11
  %15 = getelementptr i8, ptr addrspace(1) %2, i16 %11
  %16 = load i8, ptr addrspace(1) %15
  store i8 %16, ptr %14
  %17 = add i16 %11, 1
  br label %10

18:
  %19 = getelementptr i8, ptr %8, i16 -4
  store i16 %7, ptr %19
  %20 = getelementptr i8, ptr %8, i16 %7
  store i8 0, ptr %20
  ret ptr %8
}

define ptr @N$TCAT(ptr %0, ptr %1) addrspace(1) {
b1:
  %2 = getelementptr i8, ptr %0, i16 -4
  %3 = load i16, ptr %2
  %4 = getelementptr i8, ptr %1, i16 -4
  %5 = load i16, ptr %4
  %6 = add i16 %3, %5
  %7 = call addrspace(1) ptr @buffers.allocate(i16 %6, i16 1)
  %8 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %9

9:
  %10 = phi i16 [ 0, %b1 ], [ %16, %12 ]
  %11 = icmp ult i16 %10, %3
  br i1 %11, label %12, label %17

12:
  %13 = getelementptr i8, ptr %7, i16 %10
  %14 = getelementptr i8, ptr addrspace(1) %8, i16 %10
  %15 = load i8, ptr addrspace(1) %14
  store i8 %15, ptr %13
  %16 = add i16 %10, 1
  br label %9

17:
  %18 = getelementptr i8, ptr %7, i16 -4
  store i16 %3, ptr %18
  %19 = addrspacecast ptr %1 to ptr addrspace(1)
  %20 = load i16, ptr %4
  %21 = add i16 %3, %20
  %22 = call addrspace(1) ptr @N$BRES(ptr %7, i16 %21, i16 1)
  %23 = getelementptr i8, ptr %22, i16 %3
  br label %24

24:
  %25 = phi i16 [ 0, %17 ], [ %31, %27 ]
  %26 = icmp ult i16 %25, %20
  br i1 %26, label %27, label %32

27:
  %28 = getelementptr i8, ptr %23, i16 %25
  %29 = getelementptr i8, ptr addrspace(1) %19, i16 %25
  %30 = load i8, ptr addrspace(1) %29
  store i8 %30, ptr %28
  %31 = add i16 %25, 1
  br label %24

32:
  %33 = getelementptr i8, ptr %22, i16 -4
  store i16 %21, ptr %33
  %34 = getelementptr i8, ptr %22, i16 %21
  store i8 0, ptr %34
  ret ptr %22
}

define ptr @N$VCPY(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = call addrspace(1) ptr @buffers.allocate(i16 %1, i16 1)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %4 = load ptr addrspace(1), ptr addrspace(1) %3
  br label %5

5:
  %6 = phi i16 [ 0, %b1 ], [ %12, %8 ]
  %7 = icmp ult i16 %6, %1
  br i1 %7, label %8, label %13

8:
  %9 = getelementptr i8, ptr %2, i16 %6
  %10 = getelementptr i8, ptr addrspace(1) %4, i16 %6
  %11 = load i8, ptr addrspace(1) %10
  store i8 %11, ptr %9
  %12 = add i16 %6, 1
  br label %5

13:
  %14 = getelementptr i8, ptr %2, i16 %1
  store i8 0, ptr %14
  %15 = getelementptr i8, ptr %2, i16 -4
  store i16 %1, ptr %15
  ret ptr %2
}

define i8 @N$VCMP(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) willreturn {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = load i16, ptr addrspace(1) %1
  %4 = icmp ult i16 %2, %3
  br i1 %4, label %b4, label %b3

b3:
  br label %b4

b4:
  %5 = phi i16 [ %2, %b1 ], [ %3, %b3 ]
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  br label %b5

b5:
  %10 = phi i16 [ 0, %b4 ], [ %21, %b11 ]
  %11 = icmp ult i16 %10, %5
  br i1 %11, label %b6, label %b8

b6:
  %12 = getelementptr i8, ptr addrspace(1) %7, i16 %10
  %13 = load i8, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %15 = load i8, ptr addrspace(1) %14
  %16 = icmp ne i8 %13, %15
  br i1 %16, label %b9, label %b11

b8:
  %17 = icmp eq i16 %2, %3
  br i1 %17, label %b15, label %b16

b9:
  %18 = load i8, ptr addrspace(1) %12
  %19 = load i8, ptr addrspace(1) %14
  %20 = icmp ult i8 %18, %19
  br i1 %20, label %b14, label %b13

b11:
  %21 = add i16 %10, 1
  br label %b5

b13:
  br label %b14

b14:
  %22 = phi i8 [ -1, %b9 ], [ 1, %b13 ]
  ret i8 %22

b15:
  ret i8 0

b16:
  br i1 %4, label %b20, label %b19

b19:
  br label %b20

b20:
  %23 = phi i8 [ -1, %b16 ], [ 1, %b19 ]
  ret i8 %23
}

define internal ptr @format.scratch_at(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = getelementptr i8, ptr @$var_format.scratch, i16 %0
  ret ptr %1
}

define internal void @format.put(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = load ptr, ptr @$var_format.sink, !tbaa !2
  %3 = icmp eq ptr %2, null
  br i1 %3, label %b2, label %b3

b2:
  %4 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %0, i16 %1)
  br label %b4

b3:
  %5 = getelementptr i8, ptr %2, i16 -4
  %6 = load i16, ptr %5
  %7 = add i16 %6, %1
  %8 = call addrspace(1) ptr @N$BRES(ptr %2, i16 %7, i16 1)
  %9 = getelementptr i8, ptr %8, i16 %6
  br label %10

10:
  %11 = phi i16 [ 0, %b3 ], [ %17, %13 ]
  %12 = icmp ult i16 %11, %1
  br i1 %12, label %13, label %18

13:
  %14 = getelementptr i8, ptr %9, i16 %11
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 %11
  %16 = load i8, ptr addrspace(1) %15
  store i8 %16, ptr %14
  %17 = add i16 %11, 1
  br label %10

18:
  %19 = getelementptr i8, ptr %8, i16 -4
  store i16 %7, ptr %19
  %20 = getelementptr i8, ptr %8, i16 %7
  store i8 0, ptr %20
  store ptr %8, ptr @$var_format.sink, !tbaa !2
  br label %b4

b4:
  ret void
}

define internal void @format.put_text(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  %3 = load i16, ptr addrspace(1) %0
  %4 = load ptr, ptr @$var_format.sink
  %5 = icmp eq ptr %4, null
  br i1 %5, label %6, label %8

6:
  %7 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %2, i16 %3)
  br label %25

8:
  %9 = getelementptr i8, ptr %4, i16 -4
  %10 = load i16, ptr %9
  %11 = add i16 %10, %3
  %12 = call addrspace(1) ptr @N$BRES(ptr %4, i16 %11, i16 1)
  %13 = getelementptr i8, ptr %12, i16 %10
  br label %14

14:
  %15 = phi i16 [ 0, %8 ], [ %21, %17 ]
  %16 = icmp ult i16 %15, %3
  br i1 %16, label %17, label %22

17:
  %18 = getelementptr i8, ptr %13, i16 %15
  %19 = getelementptr i8, ptr addrspace(1) %2, i16 %15
  %20 = load i8, ptr addrspace(1) %19
  store i8 %20, ptr %18
  %21 = add i16 %15, 1
  br label %14

22:
  %23 = getelementptr i8, ptr %12, i16 -4
  store i16 %11, ptr %23
  %24 = getelementptr i8, ptr %12, i16 %11
  store i8 0, ptr %24
  store ptr %12, ptr @$var_format.sink
  br label %25

25:
  ret void
}

define internal void @format.pad(i16 %0) addrspace(1) {
b1:
  %1 = addrspacecast ptr @$var_format.fill to ptr addrspace(1)
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %26, %25 ]
  %3 = icmp ult i16 %2, %0
  br i1 %3, label %b3, label %b5

b3:
  %4 = load ptr, ptr @$var_format.sink
  %5 = icmp eq ptr %4, null
  br i1 %5, label %6, label %8

6:
  %7 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %1, i16 1)
  br label %25

8:
  %9 = getelementptr i8, ptr %4, i16 -4
  %10 = load i16, ptr %9
  %11 = add i16 %10, 1
  %12 = call addrspace(1) ptr @N$BRES(ptr %4, i16 %11, i16 1)
  %13 = getelementptr i8, ptr %12, i16 %10
  br label %14

14:
  %15 = phi i16 [ 0, %8 ], [ %21, %17 ]
  %16 = icmp ult i16 %15, 1
  br i1 %16, label %17, label %22

17:
  %18 = getelementptr i8, ptr %13, i16 %15
  %19 = getelementptr i8, ptr addrspace(1) %1, i16 %15
  %20 = load i8, ptr addrspace(1) %19
  store i8 %20, ptr %18
  %21 = add i16 %15, 1
  br label %14

22:
  %23 = getelementptr i8, ptr %12, i16 -4
  store i16 %11, ptr %23
  %24 = getelementptr i8, ptr %12, i16 %11
  store i8 0, ptr %24
  store ptr %12, ptr @$var_format.sink
  br label %25

25:
  %26 = add i16 %2, 1
  br label %b2

b5:
  ret void
}

define internal i16 @format.open(i16 %0) addrspace(1) {
b1:
  %1 = load i8, ptr @$var_format.width, !tbaa !2
  %2 = zext i8 %1 to i16
  %3 = icmp ugt i16 %2, %0
  br i1 %3, label %b2, label %b4

b2:
  %4 = sub i16 %2, %0
  br label %b4

b4:
  %5 = phi i16 [ %4, %b2 ], [ 0, %b1 ]
  store i8 0, ptr @$var_format.width, !tbaa !2
  store i8 10, ptr @$var_format.radix, !tbaa !2
  %6 = load i8, ptr @$var_format.left, !tbaa !2
  %7 = xor i8 %6, -1
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b5, label %b7

b5:
  call addrspace(1) void @format.pad(i16 %5)
  br label %b7

b7:
  ret i16 %5
}

define internal void @format.close(i16 %0) addrspace(1) {
b1:
  %1 = load i8, ptr @$var_format.left, !tbaa !2
  %2 = icmp ne i8 %1, 0
  br i1 %2, label %b2, label %b4

b2:
  call addrspace(1) void @format.pad(i16 %0)
  br label %b4

b4:
  store i8 32, ptr @$var_format.fill, !tbaa !2
  store i8 0, ptr @$var_format.left, !tbaa !2
  ret void
}

define internal void @format.field(ptr addrspace(1) %0, i16 %1) addrspace(1) {
b1:
  %2 = load i8, ptr @$var_format.width
  %3 = zext i8 %2 to i16
  %4 = icmp ugt i16 %3, %1
  br i1 %4, label %5, label %7

5:
  %6 = sub i16 %3, %1
  br label %7

7:
  %8 = phi i16 [ %6, %5 ], [ 0, %b1 ]
  store i8 0, ptr @$var_format.width
  store i8 10, ptr @$var_format.radix
  %9 = load i8, ptr @$var_format.left
  %10 = xor i8 %9, -1
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %12, label %13

12:
  call addrspace(1) void @format.pad(i16 %8)
  br label %13

13:
  %14 = load ptr, ptr @$var_format.sink
  %15 = icmp eq ptr %14, null
  br i1 %15, label %16, label %18

16:
  %17 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %0, i16 %1)
  br label %35

18:
  %19 = getelementptr i8, ptr %14, i16 -4
  %20 = load i16, ptr %19
  %21 = add i16 %20, %1
  %22 = call addrspace(1) ptr @N$BRES(ptr %14, i16 %21, i16 1)
  %23 = getelementptr i8, ptr %22, i16 %20
  br label %24

24:
  %25 = phi i16 [ 0, %18 ], [ %31, %27 ]
  %26 = icmp ult i16 %25, %1
  br i1 %26, label %27, label %32

27:
  %28 = getelementptr i8, ptr %23, i16 %25
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 %25
  %30 = load i8, ptr addrspace(1) %29
  store i8 %30, ptr %28
  %31 = add i16 %25, 1
  br label %24

32:
  %33 = getelementptr i8, ptr %22, i16 -4
  store i16 %21, ptr %33
  %34 = getelementptr i8, ptr %22, i16 %21
  store i8 0, ptr %34
  store ptr %22, ptr @$var_format.sink
  br label %35

35:
  %36 = load i8, ptr @$var_format.left
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %38, label %39

38:
  call addrspace(1) void @format.pad(i16 %8)
  br label %39

39:
  store i8 32, ptr @$var_format.fill
  store i8 0, ptr @$var_format.left
  ret void
}

define internal void @format.word(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  %3 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @format.field(ptr addrspace(1) %2, i16 %3)
  ret void
}

define internal void @format.number(ptr %0, i16 %1) addrspace(1) {
b1:
  %2 = load i8, ptr @$var_format.fill, !tbaa !2
  %3 = zext i8 %2 to i16
  %4 = icmp eq i16 %3, 48
  %5 = sext i1 %4 to i8
  br i1 %4, label %b2, label %b3

b2:
  %6 = load i8, ptr @$var_format.left, !tbaa !2
  %7 = xor i8 %6, -1
  br label %b3

b3:
  %8 = phi i8 [ %5, %b1 ], [ %7, %b2 ]
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b4, label %b5

b4:
  %10 = icmp ne i16 %1, 0
  %11 = sext i1 %10 to i8
  br label %b5

b5:
  %12 = phi i8 [ %8, %b3 ], [ %11, %b4 ]
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b6, label %b7

b6:
  %14 = load i8, ptr %0
  %15 = zext i8 %14 to i16
  %16 = icmp eq i16 %15, 45
  %17 = sext i1 %16 to i8
  br label %b7

b7:
  %18 = phi i8 [ %12, %b5 ], [ %17, %b6 ]
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b8, label %b10

b8:
  %20 = addrspacecast ptr %0 to ptr addrspace(1)
  %21 = load ptr, ptr @$var_format.sink
  %22 = icmp eq ptr %21, null
  br i1 %22, label %23, label %25

23:
  %24 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %20, i16 1)
  br label %42

25:
  %26 = getelementptr i8, ptr %21, i16 -4
  %27 = load i16, ptr %26
  %28 = add i16 %27, 1
  %29 = call addrspace(1) ptr @N$BRES(ptr %21, i16 %28, i16 1)
  %30 = getelementptr i8, ptr %29, i16 %27
  br label %31

31:
  %32 = phi i16 [ 0, %25 ], [ %38, %34 ]
  %33 = icmp ult i16 %32, 1
  br i1 %33, label %34, label %39

34:
  %35 = getelementptr i8, ptr %30, i16 %32
  %36 = getelementptr i8, ptr addrspace(1) %20, i16 %32
  %37 = load i8, ptr addrspace(1) %36
  store i8 %37, ptr %35
  %38 = add i16 %32, 1
  br label %31

39:
  %40 = getelementptr i8, ptr %29, i16 -4
  store i16 %28, ptr %40
  %41 = getelementptr i8, ptr %29, i16 %28
  store i8 0, ptr %41
  store ptr %29, ptr @$var_format.sink
  br label %42

42:
  %43 = getelementptr i8, ptr %0, i16 1
  %44 = add i16 %1, -1
  %45 = load i8, ptr @$var_format.width, !tbaa !2
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b11, label %b10

b10:
  %47 = phi ptr [ %0, %b7 ], [ %43, %b11 ], [ %43, %42 ]
  %48 = phi i16 [ %1, %b7 ], [ %44, %b11 ], [ %44, %42 ]
  %49 = addrspacecast ptr %47 to ptr addrspace(1)
  call addrspace(1) void @format.field(ptr addrspace(1) %49, i16 %48)
  ret void

b11:
  %50 = zext i8 %45 to i16
  %51 = add i16 %50, -1
  %52 = trunc i16 %51 to i8
  store i8 %52, ptr @$var_format.width, !tbaa !2
  br label %b10
}

define internal i16 @format.written(ptr %0, i32 %1, i8 %2) addrspace(1) memory(argmem: write) {
b1:
  %3 = zext i8 %2 to i32
  br label %b2

b2:
  %4 = phi i32 [ %1, %b1 ], [ %18, %b10 ]
  %5 = phi i16 [ 0, %b1 ], [ %8, %b10 ]
  %6 = urem i32 %4, %3
  %7 = trunc i32 %6 to i8
  %8 = add i16 %5, 1
  %9 = sub i16 0, %8
  %10 = getelementptr i8, ptr %0, i16 %9
  %11 = zext i8 %7 to i16
  %12 = icmp slt i16 %11, 10
  br i1 %12, label %b5, label %b6

b5:
  %13 = add i16 %11, 48
  %14 = trunc i16 %13 to i8
  br label %b7

b6:
  %15 = add i16 %11, 87
  %16 = trunc i16 %15 to i8
  br label %b7

b7:
  %17 = phi i8 [ %14, %b5 ], [ %16, %b6 ]
  store i8 %17, ptr %10
  %18 = udiv i32 %4, %3
  %19 = icmp eq i32 %18, 0
  br i1 %19, label %b8, label %b10

b8:
  ret i16 %8

b10:
  br label %b2
}

define internal void @format.integer(i32 %0, i8 %1) addrspace(1) {
b1:
  %2 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %3 = load i8, ptr @$var_format.radix, !tbaa !2
  %4 = zext i8 %3 to i32
  br label %5

5:
  %6 = phi i32 [ %0, %b1 ], [ %23, %25 ]
  %7 = phi i16 [ 0, %b1 ], [ %10, %25 ]
  %8 = urem i32 %6, %4
  %9 = trunc i32 %8 to i8
  %10 = add i16 %7, 1
  %11 = sub i16 0, %10
  %12 = getelementptr i8, ptr %2, i16 %11
  %13 = zext i8 %9 to i16
  %14 = icmp slt i16 %13, 10
  br i1 %14, label %15, label %18

15:
  %16 = add i16 %13, 48
  %17 = trunc i16 %16 to i8
  br label %21

18:
  %19 = add i16 %13, 87
  %20 = trunc i16 %19 to i8
  br label %21

21:
  %22 = phi i8 [ %17, %15 ], [ %20, %18 ]
  store i8 %22, ptr %12
  %23 = udiv i32 %6, %4
  %24 = icmp eq i32 %23, 0
  br i1 %24, label %26, label %25

25:
  br label %5

26:
  %27 = icmp ne i8 %1, 0
  br i1 %27, label %b2, label %b4

b2:
  %28 = add i16 %7, 2
  %29 = sub i16 0, %28
  %30 = getelementptr i8, ptr %2, i16 %29
  store i8 45, ptr %30
  br label %b4

b4:
  %31 = phi i16 [ %28, %b2 ], [ %10, %26 ]
  %32 = sub i16 0, %31
  %33 = getelementptr i8, ptr %2, i16 %32
  call addrspace(1) void @format.number(ptr %33, i16 %31)
  ret void
}

define internal void @format.signed(i32 %0) addrspace(1) {
b1:
  %1 = icmp slt i32 %0, 0
  br i1 %1, label %b2, label %b4

b2:
  %2 = sub i32 0, %0
  br label %b4

b4:
  %3 = phi i32 [ %2, %b2 ], [ %0, %b1 ]
  %4 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %5 = load i8, ptr @$var_format.radix
  %6 = zext i8 %5 to i32
  br label %7

7:
  %8 = phi i32 [ %3, %b4 ], [ %25, %27 ]
  %9 = phi i16 [ 0, %b4 ], [ %12, %27 ]
  %10 = urem i32 %8, %6
  %11 = trunc i32 %10 to i8
  %12 = add i16 %9, 1
  %13 = sub i16 0, %12
  %14 = getelementptr i8, ptr %4, i16 %13
  %15 = zext i8 %11 to i16
  %16 = icmp slt i16 %15, 10
  br i1 %16, label %17, label %20

17:
  %18 = add i16 %15, 48
  %19 = trunc i16 %18 to i8
  br label %23

20:
  %21 = add i16 %15, 87
  %22 = trunc i16 %21 to i8
  br label %23

23:
  %24 = phi i8 [ %19, %17 ], [ %22, %20 ]
  store i8 %24, ptr %14
  %25 = udiv i32 %8, %6
  %26 = icmp eq i32 %25, 0
  br i1 %26, label %28, label %27

27:
  br label %7

28:
  br i1 %1, label %29, label %33

29:
  %30 = add i16 %9, 2
  %31 = sub i16 0, %30
  %32 = getelementptr i8, ptr %4, i16 %31
  store i8 45, ptr %32
  br label %33

33:
  %34 = phi i16 [ %30, %29 ], [ %12, %28 ]
  %35 = sub i16 0, %34
  %36 = getelementptr i8, ptr %4, i16 %35
  call addrspace(1) void @format.number(ptr %36, i16 %34)
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
  %2 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %3 = load i8, ptr @$var_format.radix
  %4 = zext i8 %3 to i32
  br label %5

5:
  %6 = phi i32 [ %1, %b1 ], [ %23, %25 ]
  %7 = phi i16 [ 0, %b1 ], [ %10, %25 ]
  %8 = urem i32 %6, %4
  %9 = trunc i32 %8 to i8
  %10 = add i16 %7, 1
  %11 = sub i16 0, %10
  %12 = getelementptr i8, ptr %2, i16 %11
  %13 = zext i8 %9 to i16
  %14 = icmp slt i16 %13, 10
  br i1 %14, label %15, label %18

15:
  %16 = add i16 %13, 48
  %17 = trunc i16 %16 to i8
  br label %21

18:
  %19 = add i16 %13, 87
  %20 = trunc i16 %19 to i8
  br label %21

21:
  %22 = phi i8 [ %17, %15 ], [ %20, %18 ]
  store i8 %22, ptr %12
  %23 = udiv i32 %6, %4
  %24 = icmp eq i32 %23, 0
  br i1 %24, label %26, label %25

25:
  br label %5

26:
  call addrspace(1) void @format.number(ptr %12, i16 %10)
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
  %2 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %3 = load i8, ptr @$var_format.radix
  %4 = zext i8 %3 to i32
  br label %5

5:
  %6 = phi i32 [ %1, %b1 ], [ %23, %25 ]
  %7 = phi i16 [ 0, %b1 ], [ %10, %25 ]
  %8 = urem i32 %6, %4
  %9 = trunc i32 %8 to i8
  %10 = add i16 %7, 1
  %11 = sub i16 0, %10
  %12 = getelementptr i8, ptr %2, i16 %11
  %13 = zext i8 %9 to i16
  %14 = icmp slt i16 %13, 10
  br i1 %14, label %15, label %18

15:
  %16 = add i16 %13, 48
  %17 = trunc i16 %16 to i8
  br label %21

18:
  %19 = add i16 %13, 87
  %20 = trunc i16 %19 to i8
  br label %21

21:
  %22 = phi i8 [ %17, %15 ], [ %20, %18 ]
  store i8 %22, ptr %12
  %23 = udiv i32 %6, %4
  %24 = icmp eq i32 %23, 0
  br i1 %24, label %26, label %25

25:
  br label %5

26:
  call addrspace(1) void @format.number(ptr %12, i16 %10)
  ret void
}

define void @N$PI4(i32 %0) addrspace(1) {
b1:
  call addrspace(1) void @format.signed(i32 %0)
  ret void
}

define void @N$PU4(i32 %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %2 = load i8, ptr @$var_format.radix
  %3 = zext i8 %2 to i32
  br label %4

4:
  %5 = phi i32 [ %0, %b1 ], [ %22, %24 ]
  %6 = phi i16 [ 0, %b1 ], [ %9, %24 ]
  %7 = urem i32 %5, %3
  %8 = trunc i32 %7 to i8
  %9 = add i16 %6, 1
  %10 = sub i16 0, %9
  %11 = getelementptr i8, ptr %1, i16 %10
  %12 = zext i8 %8 to i16
  %13 = icmp slt i16 %12, 10
  br i1 %13, label %14, label %17

14:
  %15 = add i16 %12, 48
  %16 = trunc i16 %15 to i8
  br label %20

17:
  %18 = add i16 %12, 87
  %19 = trunc i16 %18 to i8
  br label %20

20:
  %21 = phi i8 [ %16, %14 ], [ %19, %17 ]
  store i8 %21, ptr %11
  %22 = udiv i32 %5, %3
  %23 = icmp eq i32 %22, 0
  br i1 %23, label %25, label %24

24:
  br label %4

25:
  call addrspace(1) void @format.number(ptr %11, i16 %9)
  ret void
}

define void @N$PQ4(i32 %0, i8 %1) addrspace(1) {
b1:
  %2 = icmp slt i32 %0, 0
  br i1 %2, label %b2, label %b4

b2:
  %3 = getelementptr i8, ptr @$var_format.scratch, i16 0
  store i8 45, ptr %3
  %4 = sub i32 0, %0
  br label %b4

b4:
  %5 = phi i16 [ 1, %b2 ], [ 0, %b1 ]
  %6 = phi i32 [ %4, %b2 ], [ %0, %b1 ]
  %7 = getelementptr i8, ptr @$var_format.scratch, i16 16
  %8 = zext i8 %1 to i32
  %9 = lshr i32 %6, %8
  br label %10

10:
  %11 = phi i32 [ %9, %b4 ], [ %28, %30 ]
  %12 = phi i16 [ 0, %b4 ], [ %15, %30 ]
  %13 = urem i32 %11, 10
  %14 = trunc i32 %13 to i8
  %15 = add i16 %12, 1
  %16 = sub i16 0, %15
  %17 = getelementptr i8, ptr %7, i16 %16
  %18 = zext i8 %14 to i16
  %19 = icmp slt i16 %18, 10
  br i1 %19, label %20, label %23

20:
  %21 = add i16 %18, 48
  %22 = trunc i16 %21 to i8
  br label %26

23:
  %24 = add i16 %18, 87
  %25 = trunc i16 %24 to i8
  br label %26

26:
  %27 = phi i8 [ %22, %20 ], [ %25, %23 ]
  store i8 %27, ptr %17
  %28 = udiv i32 %11, 10
  %29 = icmp eq i32 %28, 0
  br i1 %29, label %31, label %30

30:
  br label %10

31:
  %32 = getelementptr i8, ptr @$var_format.scratch, i16 %5
  %33 = addrspacecast ptr %17 to ptr addrspace(1)
  br label %34

34:
  %35 = phi i16 [ 0, %31 ], [ %41, %37 ]
  %36 = icmp ult i16 %35, %15
  br i1 %36, label %37, label %42

37:
  %38 = getelementptr i8, ptr %32, i16 %35
  %39 = getelementptr i8, ptr addrspace(1) %33, i16 %35
  %40 = load i8, ptr addrspace(1) %39
  store i8 %40, ptr %38
  %41 = add i16 %35, 1
  br label %34

42:
  %43 = add i16 %5, %15
  %44 = getelementptr i8, ptr @$var_format.scratch, i16 %43
  store i8 46, ptr %44
  %45 = add i16 %43, 1
  %46 = shl i32 1, %8
  %47 = add i32 %46, -1
  %48 = and i32 %6, %47
  %49 = icmp eq i32 %48, 0
  br i1 %49, label %50, label %b7

50:
  %51 = getelementptr i8, ptr @$var_format.scratch, i16 %45
  store i8 48, ptr %51
  %52 = add i16 %43, 2
  br label %b7

b7:
  %53 = phi i16 [ %52, %50 ], [ %45, %42 ]
  %54 = udiv i32 %46, 10
  %55 = urem i32 %46, 10
  %56 = trunc i32 %55 to i16
  br label %b8

b8:
  %57 = phi i16 [ %53, %b7 ], [ %79, %b13 ]
  %58 = phi i32 [ %48, %b7 ], [ %73, %b13 ]
  %59 = icmp ne i32 %58, 0
  br i1 %59, label %b9, label %60

b9:
  br label %b11

60:
  %61 = getelementptr i8, ptr @$var_format.scratch, i16 0
  call addrspace(1) void @format.number(ptr %61, i16 %57)
  ret void

b11:
  %62 = phi i16 [ 0, %b9 ], [ %63, %b12 ]
  %63 = phi i16 [ 1, %b9 ], [ %66, %b12 ]
  %64 = icmp ule i16 %63, 9
  %65 = sext i1 %64 to i8
  br i1 %64, label %b14, label %b15

b12:
  %66 = add i16 %63, 1
  br label %b11

b13:
  %67 = zext i16 %62 to i32
  %68 = mul i32 %67, %54
  %69 = sub i32 %58, %68
  %70 = mul i32 %69, 10
  %71 = mul i16 %62, %56
  %72 = zext i16 %71 to i32
  %73 = sub i32 %70, %72
  %74 = getelementptr i8, ptr @$var_format.scratch, i16 %57
  %75 = trunc i16 %62 to i8
  %76 = zext i8 %75 to i16
  %77 = add i16 %76, 48
  %78 = trunc i16 %77 to i8
  store i8 %78, ptr %74
  %79 = add i16 %57, 1
  br label %b8

b14:
  %80 = zext i16 %63 to i32
  %81 = mul i32 %80, %54
  %82 = mul i16 %63, %56
  %83 = add i16 %82, 9
  %84 = udiv i16 %83, 10
  %85 = zext i16 %84 to i32
  %86 = add i32 %81, %85
  %87 = icmp uge i32 %58, %86
  %88 = sext i1 %87 to i8
  br label %b15

b15:
  %89 = phi i8 [ %65, %b11 ], [ %88, %b14 ]
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b12, label %b13
}

define void @N$PQ2(i16 %0, i8 %1) addrspace(1) {
b1:
  %2 = sext i16 %0 to i32
  call addrspace(1) void @N$PQ4(i32 %2, i8 %1)
  ret void
}

define void @N$PB(i8 %0) addrspace(1) {
b1:
  %1 = icmp ne i8 %0, 0
  br i1 %1, label %b2, label %b3

b2:
  %2 = getelementptr i8, ptr @$str19, i16 6
  br label %b4

b3:
  %3 = getelementptr i8, ptr @$str20, i16 6
  br label %b4

b4:
  %4 = phi ptr [ %2, %b2 ], [ %3, %b3 ]
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  %7 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @format.field(ptr addrspace(1) %7, i16 %6)
  ret void
}

define void @N$PC(i8 %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr @$var_format.scratch, i16 0
  store i8 %0, ptr %1
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @format.field(ptr addrspace(1) %2, i16 1)
  ret void
}

define void @N$PS(ptr %0) addrspace(1) {
b1:
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  %2 = getelementptr i8, ptr %0, i16 -4
  %3 = load i16, ptr %2
  call addrspace(1) void @format.field(ptr addrspace(1) %1, i16 %3)
  ret void
}

define void @N$PV(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %2 = load ptr addrspace(1), ptr addrspace(1) %1
  %3 = load i16, ptr addrspace(1) %0
  call addrspace(1) void @format.field(ptr addrspace(1) %2, i16 %3)
  ret void
}

define void @N$PN() addrspace(1) {
b1:
  %0 = getelementptr i8, ptr @$str11, i16 6
  %1 = getelementptr i8, ptr %0, i16 -4
  %2 = load i16, ptr %1
  %3 = addrspacecast ptr %0 to ptr addrspace(1)
  %4 = load ptr, ptr @$var_format.sink
  %5 = icmp eq ptr %4, null
  br i1 %5, label %6, label %8

6:
  %7 = call addrspace(1) i16 @N$OWRI(i16 1, ptr addrspace(1) %3, i16 %2)
  br label %25

8:
  %9 = getelementptr i8, ptr %4, i16 -4
  %10 = load i16, ptr %9
  %11 = add i16 %10, %2
  %12 = call addrspace(1) ptr @N$BRES(ptr %4, i16 %11, i16 1)
  %13 = getelementptr i8, ptr %12, i16 %10
  br label %14

14:
  %15 = phi i16 [ 0, %8 ], [ %21, %17 ]
  %16 = icmp ult i16 %15, %2
  br i1 %16, label %17, label %22

17:
  %18 = getelementptr i8, ptr %13, i16 %15
  %19 = getelementptr i8, ptr addrspace(1) %3, i16 %15
  %20 = load i8, ptr addrspace(1) %19
  store i8 %20, ptr %18
  %21 = add i16 %15, 1
  br label %14

22:
  %23 = getelementptr i8, ptr %12, i16 -4
  store i16 %11, ptr %23
  %24 = getelementptr i8, ptr %12, i16 %11
  store i8 0, ptr %24
  store ptr %12, ptr @$var_format.sink
  br label %25

25:
  ret void
}

define void @N$PFLD(i8 %0, i8 %1, i8 %2, i8 %3) addrspace(1) willreturn {
b1:
  store i8 %0, ptr @$var_format.width, !tbaa !2
  store i8 %1, ptr @$var_format.radix, !tbaa !2
  store i8 %2, ptr @$var_format.fill, !tbaa !2
  %4 = icmp ne i8 %3, 0
  %5 = sext i1 %4 to i8
  store i8 %5, ptr @$var_format.left, !tbaa !2
  ret void
}

define void @N$PBEG() addrspace(1) {
b1:
  %0 = call addrspace(1) ptr @buffers.allocate(i16 16, i16 1)
  store ptr %0, ptr @$var_format.sink, !tbaa !2
  ret void
}

define ptr @N$PEND() addrspace(1) willreturn {
b1:
  %0 = load ptr, ptr @$var_format.sink, !tbaa !2
  store ptr null, ptr @$var_format.sink, !tbaa !2
  ret ptr %0
}

define internal void @floats.assign(ptr %0, i16 %1) addrspace(1) memory(argmem: write) willreturn {
b1:
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %6, %b3 ]
  %3 = icmp slt i16 %2, 72
  br i1 %3, label %b3, label %b5

b3:
  %4 = shl i16 %2, 1
  %5 = getelementptr i8, ptr %0, i16 %4
  store i16 0, ptr %5
  %6 = add i16 %2, 1
  br label %b2

b5:
  %7 = getelementptr i8, ptr %0, i16 0
  store i16 %1, ptr %7
  ret void
}

define internal void @floats.copy(ptr %0, ptr %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %8, %b3 ]
  %3 = icmp slt i16 %2, 72
  br i1 %3, label %b3, label %b5

b3:
  %4 = shl i16 %2, 1
  %5 = getelementptr i8, ptr %0, i16 %4
  %6 = getelementptr i8, ptr %1, i16 %4
  %7 = load i16, ptr %6
  store i16 %7, ptr %5
  %8 = add i16 %2, 1
  br label %b2

b5:
  ret void
}

define internal void @floats.shift(ptr %0, i16 %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  %2 = lshr i16 %1, 4
  %3 = and i16 %1, 15
  br label %b2

b2:
  %4 = phi i16 [ 72, %b1 ], [ %6, %b7 ]
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = add i16 %4, -1
  %7 = shl i16 %6, 1
  %8 = getelementptr i8, ptr %0, i16 %7
  %9 = icmp uge i16 %6, %2
  br i1 %9, label %b5, label %b7

b4:
  %10 = icmp ne i16 %3, 0
  br i1 %10, label %b8, label %b10

b5:
  %11 = sub i16 %6, %2
  %12 = shl i16 %11, 1
  %13 = getelementptr i8, ptr %0, i16 %12
  %14 = load i16, ptr %13
  br label %b7

b7:
  %15 = phi i16 [ %14, %b5 ], [ 0, %b3 ]
  store i16 %15, ptr %8
  br label %b2

b8:
  %16 = sub i16 16, %3
  br label %b11

b10:
  ret void

b11:
  %17 = phi i16 [ 72, %b8 ], [ %19, %b12 ]
  %18 = icmp ugt i16 %17, 1
  br i1 %18, label %b12, label %b13

b12:
  %19 = add i16 %17, -1
  %20 = shl i16 %19, 1
  %21 = getelementptr i8, ptr %0, i16 %20
  %22 = load i16, ptr %21
  %23 = shl i16 %22, %3
  %24 = add i16 %17, -2
  %25 = shl i16 %24, 1
  %26 = getelementptr i8, ptr %0, i16 %25
  %27 = load i16, ptr %26
  %28 = lshr i16 %27, %16
  %29 = or i16 %23, %28
  store i16 %29, ptr %21
  br label %b11

b13:
  %30 = getelementptr i8, ptr %0, i16 0
  %31 = load i16, ptr %30
  %32 = shl i16 %31, %3
  store i16 %32, ptr %30
  br label %b10
}

define internal void @floats.multiply(ptr %0, i16 %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  br label %b2

b2:
  %2 = phi i32 [ 0, %b1 ], [ %12, %b3 ]
  %3 = phi i16 [ 0, %b1 ], [ %13, %b3 ]
  %4 = icmp slt i16 %3, 72
  br i1 %4, label %b3, label %b5

b3:
  %5 = shl i16 %3, 1
  %6 = getelementptr i8, ptr %0, i16 %5
  %7 = load i16, ptr %6
  %8 = zext i16 %7 to i32
  %9 = mul i32 %8, 10
  %10 = add i32 %9, %2
  %11 = trunc i32 %10 to i16
  store i16 %11, ptr %6
  %12 = lshr i32 %10, 16
  %13 = add i16 %3, 1
  br label %b2

b5:
  ret void
}

define internal void @floats.add(ptr %0, ptr %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  br label %b2

b2:
  %2 = phi i32 [ 0, %b1 ], [ %15, %b3 ]
  %3 = phi i16 [ 0, %b1 ], [ %16, %b3 ]
  %4 = icmp slt i16 %3, 72
  br i1 %4, label %b3, label %b5

b3:
  %5 = shl i16 %3, 1
  %6 = getelementptr i8, ptr %0, i16 %5
  %7 = load i16, ptr %6
  %8 = zext i16 %7 to i32
  %9 = getelementptr i8, ptr %1, i16 %5
  %10 = load i16, ptr %9
  %11 = zext i16 %10 to i32
  %12 = add i32 %8, %11
  %13 = add i32 %12, %2
  %14 = trunc i32 %13 to i16
  store i16 %14, ptr %6
  %15 = lshr i32 %13, 16
  %16 = add i16 %3, 1
  br label %b2

b5:
  ret void
}

define internal void @floats.subtract(ptr %0, ptr %1) addrspace(1) memory(argmem: readwrite) willreturn {
b1:
  br label %b2

b2:
  %2 = phi i32 [ 0, %b1 ], [ %16, %b3 ]
  %3 = phi i16 [ 0, %b1 ], [ %17, %b3 ]
  %4 = icmp slt i16 %3, 72
  br i1 %4, label %b3, label %b5

b3:
  %5 = shl i16 %3, 1
  %6 = getelementptr i8, ptr %0, i16 %5
  %7 = load i16, ptr %6
  %8 = zext i16 %7 to i32
  %9 = getelementptr i8, ptr %1, i16 %5
  %10 = load i16, ptr %9
  %11 = zext i16 %10 to i32
  %12 = sub i32 %8, %11
  %13 = sub i32 %12, %2
  %14 = trunc i32 %13 to i16
  store i16 %14, ptr %6
  %15 = lshr i32 %13, 16
  %16 = and i32 %15, 1
  %17 = add i16 %3, 1
  br label %b2

b5:
  ret void
}

define internal i8 @floats.compare(ptr %0, ptr %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  br label %b2

b2:
  %2 = phi i16 [ 72, %b1 ], [ %4, %b6 ]
  %3 = icmp ne i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = add i16 %2, -1
  %5 = shl i16 %4, 1
  %6 = getelementptr i8, ptr %0, i16 %5
  %7 = load i16, ptr %6
  %8 = getelementptr i8, ptr %1, i16 %5
  %9 = load i16, ptr %8
  %10 = icmp ne i16 %7, %9
  br i1 %10, label %b5, label %b6

b4:
  ret i8 0

b5:
  %11 = load i16, ptr %6
  %12 = load i16, ptr %8
  %13 = icmp ult i16 %11, %12
  br i1 %13, label %b10, label %b9

b6:
  br label %b2

b9:
  br label %b10

b10:
  %14 = phi i8 [ -1, %b5 ], [ 1, %b9 ]
  ret i8 %14
}

define internal i8 @floats.beyond(ptr %0, ptr %1, i8 %2) addrspace(1) memory(argmem: read) willreturn {
b1:
  br label %3

3:
  %4 = phi i16 [ 72, %b1 ], [ %7, %19 ]
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %6, label %14

6:
  %7 = add i16 %4, -1
  %8 = shl i16 %7, 1
  %9 = getelementptr i8, ptr %0, i16 %8
  %10 = load i16, ptr %9
  %11 = getelementptr i8, ptr %1, i16 %8
  %12 = load i16, ptr %11
  %13 = icmp ne i16 %10, %12
  br i1 %13, label %15, label %19

14:
  br label %23

15:
  %16 = load i16, ptr %9
  %17 = load i16, ptr %11
  %18 = icmp ult i16 %16, %17
  br i1 %18, label %21, label %20

19:
  br label %3

20:
  br label %21

21:
  %22 = phi i8 [ -1, %15 ], [ 1, %20 ]
  br label %23

23:
  %24 = phi i8 [ 0, %14 ], [ %22, %21 ]
  %25 = sext i8 %24 to i16
  %26 = icmp sgt i16 %25, 0
  %27 = sext i1 %26 to i8
  br i1 %26, label %b3, label %b2

b2:
  %28 = icmp ne i8 %2, 0
  br i1 %28, label %b4, label %b5

b3:
  %29 = phi i8 [ %27, %23 ], [ %32, %b5 ]
  ret i8 %29

b4:
  %30 = icmp eq i8 %24, 0
  %31 = sext i1 %30 to i8
  br label %b5

b5:
  %32 = phi i8 [ %2, %b2 ], [ %31, %b4 ]
  br label %b3
}

define internal i16 @floats.put(ptr %0, i16 %1, i8 %2) addrspace(1) memory(argmem: write) willreturn {
b1:
  %3 = getelementptr i8, ptr %0, i16 %1
  store i8 %2, ptr %3
  %4 = add i16 %1, 1
  ret i16 %4
}

define internal i16 @floats.put_text(ptr %0, i16 %1, ptr addrspace(1) noalias readonly dereferenceable(8) %2) addrspace(1) willreturn {
b1:
  %3 = load i16, ptr addrspace(1) %2
  %4 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  br label %b2

b2:
  %6 = phi i16 [ %1, %b1 ], [ %12, %b3 ]
  %7 = phi i16 [ 0, %b1 ], [ %13, %b3 ]
  %8 = icmp ult i16 %7, %3
  br i1 %8, label %b3, label %b5

b3:
  %9 = getelementptr i8, ptr addrspace(1) %5, i16 %7
  %10 = load i8, ptr addrspace(1) %9
  %11 = getelementptr i8, ptr %0, i16 %6
  store i8 %10, ptr %11
  %12 = add i16 %6, 1
  %13 = add i16 %7, 1
  br label %b2

b5:
  ret i16 %6
}

define internal i32 @floats.digits(ptr %0, i16 %1, i8 %2, i8 %3, ptr %4) addrspace(1) {
b1:
  %5 = alloca [8 x i8]
  %6 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 4, i1 false)
  %7 = getelementptr i8, ptr %0, i16 144
  %8 = getelementptr i8, ptr %0, i16 288
  %9 = getelementptr i8, ptr %0, i16 432
  %10 = getelementptr i8, ptr %0, i16 576
  %11 = icmp ne i8 %2, 0
  br i1 %11, label %b4, label %b3

b3:
  br label %b4

b4:
  %12 = phi i16 [ 1, %b1 ], [ 0, %b3 ]
  %13 = icmp sge i16 %1, 0
  br i1 %13, label %b5, label %b6

b5:
  %14 = add i16 %1, 1
  %15 = add i16 %14, %12
  call addrspace(1) void @floats.shift(ptr %0, i16 %15)
  %16 = icmp ult i16 %12, 16
  br i1 %16, label %b8, label %71

b6:
  %17 = add i16 %12, 1
  call addrspace(1) void @floats.shift(ptr %0, i16 %17)
  br label %18

18:
  %19 = phi i16 [ 0, %b6 ], [ %24, %21 ]
  %20 = icmp slt i16 %19, 72
  br i1 %20, label %21, label %25

21:
  %22 = shl i16 %19, 1
  %23 = getelementptr i8, ptr %7, i16 %22
  store i16 0, ptr %23
  %24 = add i16 %19, 1
  br label %18

25:
  %26 = getelementptr i8, ptr %7, i16 0
  store i16 1, ptr %26
  %27 = sub i16 0, %1
  %28 = add i16 %27, 1
  %29 = add i16 %28, %12
  call addrspace(1) void @floats.shift(ptr %7, i16 %29)
  br label %30

30:
  %31 = phi i16 [ 0, %25 ], [ %36, %33 ]
  %32 = icmp slt i16 %31, 72
  br i1 %32, label %33, label %37

33:
  %34 = shl i16 %31, 1
  %35 = getelementptr i8, ptr %9, i16 %34
  store i16 0, ptr %35
  %36 = add i16 %31, 1
  br label %30

37:
  %38 = getelementptr i8, ptr %9, i16 0
  store i16 1, ptr %38
  br label %39

39:
  br label %40

40:
  %41 = phi i16 [ 0, %39 ], [ %48, %43 ]
  %42 = icmp slt i16 %41, 72
  br i1 %42, label %43, label %49

43:
  %44 = shl i16 %41, 1
  %45 = getelementptr i8, ptr %8, i16 %44
  %46 = getelementptr i8, ptr %9, i16 %44
  %47 = load i16, ptr %46
  store i16 %47, ptr %45
  %48 = add i16 %41, 1
  br label %40

49:
  call addrspace(1) void @floats.shift(ptr %8, i16 %12)
  %50 = xor i8 %3, -1
  %51 = icmp ne i8 %50, 0
  br label %b10

b8:
  %52 = shl i16 2, %12
  br label %53

53:
  %54 = phi i16 [ 0, %b8 ], [ %59, %56 ]
  %55 = icmp slt i16 %54, 72
  br i1 %55, label %56, label %60

56:
  %57 = shl i16 %54, 1
  %58 = getelementptr i8, ptr %7, i16 %57
  store i16 0, ptr %58
  %59 = add i16 %54, 1
  br label %53

60:
  %61 = getelementptr i8, ptr %7, i16 0
  store i16 %52, ptr %61
  br label %62

62:
  %63 = phi i16 [ 0, %60 ], [ %68, %65 ]
  %64 = icmp slt i16 %63, 72
  br i1 %64, label %65, label %69

65:
  %66 = shl i16 %63, 1
  %67 = getelementptr i8, ptr %9, i16 %66
  store i16 0, ptr %67
  %68 = add i16 %63, 1
  br label %62

69:
  %70 = getelementptr i8, ptr %9, i16 0
  store i16 1, ptr %70
  call addrspace(1) void @floats.shift(ptr %9, i16 %1)
  br label %39

71:
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  %72 = getelementptr i8, ptr @$str14, i16 6
  %73 = getelementptr i8, ptr %72, i16 -4
  %74 = load i16, ptr %73
  %75 = addrspacecast ptr %72 to ptr addrspace(1)
  store i16 %74, ptr %5
  %76 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 %74, ptr %76
  %77 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %75, ptr %77
  %78 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @errors.panic(ptr addrspace(1) %78)
  unreachable

b10:
  %79 = phi i16 [ 0, %49 ], [ %159, %158 ]
  br label %80

80:
  %81 = phi i16 [ 0, %b10 ], [ %88, %83 ]
  %82 = icmp slt i16 %81, 72
  br i1 %82, label %83, label %89

83:
  %84 = shl i16 %81, 1
  %85 = getelementptr i8, ptr %10, i16 %84
  %86 = getelementptr i8, ptr %0, i16 %84
  %87 = load i16, ptr %86
  store i16 %87, ptr %85
  %88 = add i16 %81, 1
  br label %80

89:
  br label %90

90:
  %91 = phi i32 [ 0, %89 ], [ %105, %94 ]
  %92 = phi i16 [ 0, %89 ], [ %106, %94 ]
  %93 = icmp slt i16 %92, 72
  br i1 %93, label %94, label %107

94:
  %95 = shl i16 %92, 1
  %96 = getelementptr i8, ptr %10, i16 %95
  %97 = load i16, ptr %96
  %98 = zext i16 %97 to i32
  %99 = getelementptr i8, ptr %8, i16 %95
  %100 = load i16, ptr %99
  %101 = zext i16 %100 to i32
  %102 = add i32 %98, %101
  %103 = add i32 %102, %91
  %104 = trunc i32 %103 to i16
  store i16 %104, ptr %96
  %105 = lshr i32 %103, 16
  %106 = add i16 %92, 1
  br label %90

107:
  br label %108

108:
  %109 = phi i16 [ 72, %107 ], [ %112, %124 ]
  %110 = icmp ne i16 %109, 0
  br i1 %110, label %111, label %119

111:
  %112 = add i16 %109, -1
  %113 = shl i16 %112, 1
  %114 = getelementptr i8, ptr %10, i16 %113
  %115 = load i16, ptr %114
  %116 = getelementptr i8, ptr %7, i16 %113
  %117 = load i16, ptr %116
  %118 = icmp ne i16 %115, %117
  br i1 %118, label %120, label %124

119:
  br label %128

120:
  %121 = load i16, ptr %114
  %122 = load i16, ptr %116
  %123 = icmp ult i16 %121, %122
  br i1 %123, label %126, label %125

124:
  br label %108

125:
  br label %126

126:
  %127 = phi i8 [ -1, %120 ], [ 1, %125 ]
  br label %128

128:
  %129 = phi i8 [ 0, %119 ], [ %127, %126 ]
  %130 = sext i8 %129 to i16
  %131 = icmp sgt i16 %130, 0
  %132 = sext i1 %131 to i8
  br i1 %131, label %134, label %133

133:
  br i1 %51, label %138, label %141

134:
  %135 = phi i8 [ %132, %128 ], [ %142, %141 ]
  %136 = xor i8 %135, -1
  %137 = icmp ne i8 %136, 0
  br i1 %137, label %b13, label %143

138:
  %139 = icmp eq i8 %129, 0
  %140 = sext i1 %139 to i8
  br label %141

141:
  %142 = phi i8 [ %50, %133 ], [ %140, %138 ]
  br label %134

b13:
  br label %b16

143:
  br label %144

144:
  %145 = phi i32 [ 0, %143 ], [ %156, %148 ]
  %146 = phi i16 [ 0, %143 ], [ %157, %148 ]
  %147 = icmp slt i16 %146, 72
  br i1 %147, label %148, label %158

148:
  %149 = shl i16 %146, 1
  %150 = getelementptr i8, ptr %7, i16 %149
  %151 = load i16, ptr %150
  %152 = zext i16 %151 to i32
  %153 = mul i32 %152, 10
  %154 = add i32 %153, %145
  %155 = trunc i32 %154 to i16
  store i16 %155, ptr %150
  %156 = lshr i32 %154, 16
  %157 = add i16 %146, 1
  br label %144

158:
  %159 = add i16 %79, 1
  br label %b10

b16:
  %160 = phi i16 [ %79, %b13 ], [ %285, %284 ]
  br label %161

161:
  %162 = phi i16 [ 0, %b16 ], [ %169, %164 ]
  %163 = icmp slt i16 %162, 72
  br i1 %163, label %164, label %170

164:
  %165 = shl i16 %162, 1
  %166 = getelementptr i8, ptr %10, i16 %165
  %167 = getelementptr i8, ptr %0, i16 %165
  %168 = load i16, ptr %167
  store i16 %168, ptr %166
  %169 = add i16 %162, 1
  br label %161

170:
  br label %171

171:
  %172 = phi i32 [ 0, %170 ], [ %186, %175 ]
  %173 = phi i16 [ 0, %170 ], [ %187, %175 ]
  %174 = icmp slt i16 %173, 72
  br i1 %174, label %175, label %188

175:
  %176 = shl i16 %173, 1
  %177 = getelementptr i8, ptr %10, i16 %176
  %178 = load i16, ptr %177
  %179 = zext i16 %178 to i32
  %180 = getelementptr i8, ptr %8, i16 %176
  %181 = load i16, ptr %180
  %182 = zext i16 %181 to i32
  %183 = add i32 %179, %182
  %184 = add i32 %183, %172
  %185 = trunc i32 %184 to i16
  store i16 %185, ptr %177
  %186 = lshr i32 %184, 16
  %187 = add i16 %173, 1
  br label %171

188:
  br label %189

189:
  %190 = phi i32 [ 0, %188 ], [ %201, %193 ]
  %191 = phi i16 [ 0, %188 ], [ %202, %193 ]
  %192 = icmp slt i16 %191, 72
  br i1 %192, label %193, label %203

193:
  %194 = shl i16 %191, 1
  %195 = getelementptr i8, ptr %10, i16 %194
  %196 = load i16, ptr %195
  %197 = zext i16 %196 to i32
  %198 = mul i32 %197, 10
  %199 = add i32 %198, %190
  %200 = trunc i32 %199 to i16
  store i16 %200, ptr %195
  %201 = lshr i32 %199, 16
  %202 = add i16 %191, 1
  br label %189

203:
  br label %204

204:
  %205 = phi i16 [ 72, %203 ], [ %208, %220 ]
  %206 = icmp ne i16 %205, 0
  br i1 %206, label %207, label %215

207:
  %208 = add i16 %205, -1
  %209 = shl i16 %208, 1
  %210 = getelementptr i8, ptr %10, i16 %209
  %211 = load i16, ptr %210
  %212 = getelementptr i8, ptr %7, i16 %209
  %213 = load i16, ptr %212
  %214 = icmp ne i16 %211, %213
  br i1 %214, label %216, label %220

215:
  br label %224

216:
  %217 = load i16, ptr %210
  %218 = load i16, ptr %212
  %219 = icmp ult i16 %217, %218
  br i1 %219, label %222, label %221

220:
  br label %204

221:
  br label %222

222:
  %223 = phi i8 [ -1, %216 ], [ 1, %221 ]
  br label %224

224:
  %225 = phi i8 [ 0, %215 ], [ %223, %222 ]
  %226 = sext i8 %225 to i16
  %227 = icmp sgt i16 %226, 0
  %228 = sext i1 %227 to i8
  br i1 %227, label %230, label %229

229:
  br i1 %51, label %233, label %236

230:
  %231 = phi i8 [ %228, %224 ], [ %237, %236 ]
  %232 = icmp ne i8 %231, 0
  br i1 %232, label %b18, label %239

233:
  %234 = icmp eq i8 %225, 0
  %235 = sext i1 %234 to i8
  br label %236

236:
  %237 = phi i8 [ %50, %229 ], [ %235, %233 ]
  br label %230

b18:
  %238 = icmp ne i8 %3, 0
  br label %b22

239:
  br label %240

240:
  %241 = phi i32 [ 0, %239 ], [ %252, %244 ]
  %242 = phi i16 [ 0, %239 ], [ %253, %244 ]
  %243 = icmp slt i16 %242, 72
  br i1 %243, label %244, label %254

244:
  %245 = shl i16 %242, 1
  %246 = getelementptr i8, ptr %0, i16 %245
  %247 = load i16, ptr %246
  %248 = zext i16 %247 to i32
  %249 = mul i32 %248, 10
  %250 = add i32 %249, %241
  %251 = trunc i32 %250 to i16
  store i16 %251, ptr %246
  %252 = lshr i32 %250, 16
  %253 = add i16 %242, 1
  br label %240

254:
  br label %255

255:
  %256 = phi i32 [ 0, %254 ], [ %267, %259 ]
  %257 = phi i16 [ 0, %254 ], [ %268, %259 ]
  %258 = icmp slt i16 %257, 72
  br i1 %258, label %259, label %269

259:
  %260 = shl i16 %257, 1
  %261 = getelementptr i8, ptr %8, i16 %260
  %262 = load i16, ptr %261
  %263 = zext i16 %262 to i32
  %264 = mul i32 %263, 10
  %265 = add i32 %264, %256
  %266 = trunc i32 %265 to i16
  store i16 %266, ptr %261
  %267 = lshr i32 %265, 16
  %268 = add i16 %257, 1
  br label %255

269:
  br label %270

270:
  %271 = phi i32 [ 0, %269 ], [ %282, %274 ]
  %272 = phi i16 [ 0, %269 ], [ %283, %274 ]
  %273 = icmp slt i16 %272, 72
  br i1 %273, label %274, label %284

274:
  %275 = shl i16 %272, 1
  %276 = getelementptr i8, ptr %9, i16 %275
  %277 = load i16, ptr %276
  %278 = zext i16 %277 to i32
  %279 = mul i32 %278, 10
  %280 = add i32 %279, %271
  %281 = trunc i32 %280 to i16
  store i16 %281, ptr %276
  %282 = lshr i32 %280, 16
  %283 = add i16 %272, 1
  br label %270

284:
  %285 = add i16 %160, -1
  br label %b16

b22:
  %286 = phi i16 [ 0, %b18 ], [ %483, %b30 ]
  br label %287

287:
  %288 = phi i32 [ 0, %b22 ], [ %299, %291 ]
  %289 = phi i16 [ 0, %b22 ], [ %300, %291 ]
  %290 = icmp slt i16 %289, 72
  br i1 %290, label %291, label %301

291:
  %292 = shl i16 %289, 1
  %293 = getelementptr i8, ptr %0, i16 %292
  %294 = load i16, ptr %293
  %295 = zext i16 %294 to i32
  %296 = mul i32 %295, 10
  %297 = add i32 %296, %288
  %298 = trunc i32 %297 to i16
  store i16 %298, ptr %293
  %299 = lshr i32 %297, 16
  %300 = add i16 %289, 1
  br label %287

301:
  br label %302

302:
  %303 = phi i32 [ 0, %301 ], [ %314, %306 ]
  %304 = phi i16 [ 0, %301 ], [ %315, %306 ]
  %305 = icmp slt i16 %304, 72
  br i1 %305, label %306, label %316

306:
  %307 = shl i16 %304, 1
  %308 = getelementptr i8, ptr %8, i16 %307
  %309 = load i16, ptr %308
  %310 = zext i16 %309 to i32
  %311 = mul i32 %310, 10
  %312 = add i32 %311, %303
  %313 = trunc i32 %312 to i16
  store i16 %313, ptr %308
  %314 = lshr i32 %312, 16
  %315 = add i16 %304, 1
  br label %302

316:
  br label %317

317:
  %318 = phi i32 [ 0, %316 ], [ %329, %321 ]
  %319 = phi i16 [ 0, %316 ], [ %330, %321 ]
  %320 = icmp slt i16 %319, 72
  br i1 %320, label %321, label %331

321:
  %322 = shl i16 %319, 1
  %323 = getelementptr i8, ptr %9, i16 %322
  %324 = load i16, ptr %323
  %325 = zext i16 %324 to i32
  %326 = mul i32 %325, 10
  %327 = add i32 %326, %318
  %328 = trunc i32 %327 to i16
  store i16 %328, ptr %323
  %329 = lshr i32 %327, 16
  %330 = add i16 %319, 1
  br label %317

331:
  br label %b25

b25:
  %332 = phi i8 [ 0, %331 ], [ %379, %376 ]
  br label %333

333:
  %334 = phi i16 [ 72, %b25 ], [ %337, %349 ]
  %335 = icmp ne i16 %334, 0
  br i1 %335, label %336, label %344

336:
  %337 = add i16 %334, -1
  %338 = shl i16 %337, 1
  %339 = getelementptr i8, ptr %0, i16 %338
  %340 = load i16, ptr %339
  %341 = getelementptr i8, ptr %7, i16 %338
  %342 = load i16, ptr %341
  %343 = icmp ne i16 %340, %342
  br i1 %343, label %345, label %349

344:
  br label %353

345:
  %346 = load i16, ptr %339
  %347 = load i16, ptr %341
  %348 = icmp ult i16 %346, %347
  br i1 %348, label %351, label %350

349:
  br label %333

350:
  br label %351

351:
  %352 = phi i8 [ -1, %345 ], [ 1, %350 ]
  br label %353

353:
  %354 = phi i8 [ 0, %344 ], [ %352, %351 ]
  %355 = sext i8 %354 to i16
  %356 = icmp sge i16 %355, 0
  br i1 %356, label %357, label %b27

357:
  br label %358

358:
  %359 = phi i32 [ 0, %357 ], [ %374, %362 ]
  %360 = phi i16 [ 0, %357 ], [ %375, %362 ]
  %361 = icmp slt i16 %360, 72
  br i1 %361, label %362, label %376

362:
  %363 = shl i16 %360, 1
  %364 = getelementptr i8, ptr %0, i16 %363
  %365 = load i16, ptr %364
  %366 = zext i16 %365 to i32
  %367 = getelementptr i8, ptr %7, i16 %363
  %368 = load i16, ptr %367
  %369 = zext i16 %368 to i32
  %370 = sub i32 %366, %369
  %371 = sub i32 %370, %359
  %372 = trunc i32 %371 to i16
  store i16 %372, ptr %364
  %373 = lshr i32 %371, 16
  %374 = and i32 %373, 1
  %375 = add i16 %360, 1
  br label %358

376:
  %377 = zext i8 %332 to i16
  %378 = add i16 %377, 1
  %379 = trunc i16 %378 to i8
  br label %b25

b27:
  br label %380

380:
  %381 = phi i16 [ 72, %b27 ], [ %384, %396 ]
  %382 = icmp ne i16 %381, 0
  br i1 %382, label %383, label %391

383:
  %384 = add i16 %381, -1
  %385 = shl i16 %384, 1
  %386 = getelementptr i8, ptr %0, i16 %385
  %387 = load i16, ptr %386
  %388 = getelementptr i8, ptr %9, i16 %385
  %389 = load i16, ptr %388
  %390 = icmp ne i16 %387, %389
  br i1 %390, label %392, label %396

391:
  br label %400

392:
  %393 = load i16, ptr %386
  %394 = load i16, ptr %388
  %395 = icmp ult i16 %393, %394
  br i1 %395, label %398, label %397

396:
  br label %380

397:
  br label %398

398:
  %399 = phi i8 [ -1, %392 ], [ 1, %397 ]
  br label %400

400:
  %401 = phi i8 [ 0, %391 ], [ %399, %398 ]
  %402 = sext i8 %401 to i16
  %403 = icmp sgt i16 %402, 0
  %404 = sext i1 %403 to i8
  br i1 %403, label %406, label %405

405:
  br i1 %51, label %409, label %412

406:
  %407 = phi i8 [ %404, %400 ], [ %413, %412 ]
  %408 = xor i8 %407, -1
  br label %414

409:
  %410 = icmp eq i8 %401, 0
  %411 = sext i1 %410 to i8
  br label %412

412:
  %413 = phi i8 [ %50, %405 ], [ %411, %409 ]
  br label %406

414:
  %415 = phi i16 [ 0, %406 ], [ %422, %417 ]
  %416 = icmp slt i16 %415, 72
  br i1 %416, label %417, label %423

417:
  %418 = shl i16 %415, 1
  %419 = getelementptr i8, ptr %10, i16 %418
  %420 = getelementptr i8, ptr %0, i16 %418
  %421 = load i16, ptr %420
  store i16 %421, ptr %419
  %422 = add i16 %415, 1
  br label %414

423:
  br label %424

424:
  %425 = phi i32 [ 0, %423 ], [ %439, %428 ]
  %426 = phi i16 [ 0, %423 ], [ %440, %428 ]
  %427 = icmp slt i16 %426, 72
  br i1 %427, label %428, label %441

428:
  %429 = shl i16 %426, 1
  %430 = getelementptr i8, ptr %10, i16 %429
  %431 = load i16, ptr %430
  %432 = zext i16 %431 to i32
  %433 = getelementptr i8, ptr %8, i16 %429
  %434 = load i16, ptr %433
  %435 = zext i16 %434 to i32
  %436 = add i32 %432, %435
  %437 = add i32 %436, %425
  %438 = trunc i32 %437 to i16
  store i16 %438, ptr %430
  %439 = lshr i32 %437, 16
  %440 = add i16 %426, 1
  br label %424

441:
  br label %442

442:
  %443 = phi i16 [ 72, %441 ], [ %446, %458 ]
  %444 = icmp ne i16 %443, 0
  br i1 %444, label %445, label %453

445:
  %446 = add i16 %443, -1
  %447 = shl i16 %446, 1
  %448 = getelementptr i8, ptr %10, i16 %447
  %449 = load i16, ptr %448
  %450 = getelementptr i8, ptr %7, i16 %447
  %451 = load i16, ptr %450
  %452 = icmp ne i16 %449, %451
  br i1 %452, label %454, label %458

453:
  br label %462

454:
  %455 = load i16, ptr %448
  %456 = load i16, ptr %450
  %457 = icmp ult i16 %455, %456
  br i1 %457, label %460, label %459

458:
  br label %442

459:
  br label %460

460:
  %461 = phi i8 [ -1, %454 ], [ 1, %459 ]
  br label %462

462:
  %463 = phi i8 [ 0, %453 ], [ %461, %460 ]
  %464 = sext i8 %463 to i16
  %465 = icmp sgt i16 %464, 0
  %466 = sext i1 %465 to i8
  br i1 %465, label %468, label %467

467:
  br i1 %238, label %471, label %474

468:
  %469 = phi i8 [ %466, %462 ], [ %475, %474 ]
  %470 = icmp ne i8 %407, 0
  br i1 %470, label %b28, label %b29

471:
  %472 = icmp eq i8 %463, 0
  %473 = sext i1 %472 to i8
  br label %474

474:
  %475 = phi i8 [ %3, %467 ], [ %473, %471 ]
  br label %468

b28:
  %476 = xor i8 %469, -1
  br label %b29

b29:
  %477 = phi i8 [ %407, %468 ], [ %476, %b28 ]
  %478 = icmp ne i8 %477, 0
  br i1 %478, label %b30, label %b32

b30:
  %479 = zext i8 %332 to i16
  %480 = add i16 %479, 48
  %481 = trunc i16 %480 to i8
  %482 = getelementptr i8, ptr %4, i16 %286
  store i8 %481, ptr %482
  %483 = add i16 %286, 1
  br label %b22

b32:
  %484 = icmp ne i8 %408, 0
  br i1 %484, label %b33, label %b34

b33:
  br label %b34

b34:
  %485 = phi i8 [ %408, %b32 ], [ %469, %b33 ]
  %486 = icmp ne i8 %485, 0
  br i1 %486, label %487, label %b36

487:
  br label %488

488:
  %489 = phi i16 [ 0, %487 ], [ %496, %491 ]
  %490 = icmp slt i16 %489, 72
  br i1 %490, label %491, label %497

491:
  %492 = shl i16 %489, 1
  %493 = getelementptr i8, ptr %10, i16 %492
  %494 = getelementptr i8, ptr %0, i16 %492
  %495 = load i16, ptr %494
  store i16 %495, ptr %493
  %496 = add i16 %489, 1
  br label %488

497:
  call addrspace(1) void @floats.shift(ptr %10, i16 1)
  br label %498

498:
  %499 = phi i16 [ 72, %497 ], [ %502, %514 ]
  %500 = icmp ne i16 %499, 0
  br i1 %500, label %501, label %509

501:
  %502 = add i16 %499, -1
  %503 = shl i16 %502, 1
  %504 = getelementptr i8, ptr %10, i16 %503
  %505 = load i16, ptr %504
  %506 = getelementptr i8, ptr %7, i16 %503
  %507 = load i16, ptr %506
  %508 = icmp ne i16 %505, %507
  br i1 %508, label %510, label %514

509:
  br label %518

510:
  %511 = load i16, ptr %504
  %512 = load i16, ptr %506
  %513 = icmp ult i16 %511, %512
  br i1 %513, label %516, label %515

514:
  br label %498

515:
  br label %516

516:
  %517 = phi i8 [ -1, %510 ], [ 1, %515 ]
  br label %518

518:
  %519 = phi i8 [ 0, %509 ], [ %517, %516 ]
  %520 = sext i8 %519 to i16
  %521 = icmp sge i16 %520, 0
  br i1 %521, label %b38, label %b40

b36:
  %522 = icmp ne i8 %469, 0
  br i1 %522, label %b41, label %b43

b37:
  %523 = phi i8 [ %535, %b40 ], [ %539, %b43 ]
  %524 = zext i8 %523 to i16
  %525 = add i16 %524, 48
  %526 = trunc i16 %525 to i8
  %527 = getelementptr i8, ptr %4, i16 %286
  store i8 %526, ptr %527
  %528 = add i16 %286, 1
  store i16 %528, ptr %6, !tbaa !2
  %529 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %160, ptr %529, !tbaa !2
  %530 = addrspacecast ptr %6 to ptr addrspace(1)
  %531 = load i32, ptr addrspace(1) %530, !tbaa !2
  ret i32 %531

b38:
  %532 = zext i8 %332 to i16
  %533 = add i16 %532, 1
  %534 = trunc i16 %533 to i8
  br label %b40

b40:
  %535 = phi i8 [ %534, %b38 ], [ %332, %518 ]
  br label %b37

b41:
  %536 = zext i8 %332 to i16
  %537 = add i16 %536, 1
  %538 = trunc i16 %537 to i8
  br label %b43

b43:
  %539 = phi i8 [ %538, %b41 ], [ %332, %b36 ]
  br label %b37
}

define internal void @floats.print_float(ptr addrspace(1) %0, i16 %1, i16 %2, i16 %3, i16 %4) addrspace(1) {
b1:
  %5 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 4, i1 false)
  %6 = call addrspace(1) ptr @heap.allocate(i16 752)
  %7 = getelementptr i8, ptr %6, i16 720
  %8 = add i16 %1, -1
  %9 = shl i16 %8, 1
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 %9
  %11 = load i16, ptr addrspace(1) %10
  %12 = lshr i16 %11, %2
  %13 = and i16 %12, %3
  %14 = shl i16 1, %2
  %15 = add i16 %14, -1
  %16 = and i16 %11, %15
  %17 = icmp eq i16 %16, 0
  %18 = sext i1 %17 to i8
  br label %b2

b2:
  %19 = phi i8 [ %18, %b1 ], [ %31, %b7 ]
  %20 = phi i16 [ 0, %b1 ], [ %32, %b7 ]
  %21 = icmp ult i16 %20, %8
  br i1 %21, label %b3, label %b5

b3:
  %22 = icmp ne i8 %19, 0
  br i1 %22, label %b6, label %b7

b5:
  %23 = lshr i16 %11, 15
  %24 = icmp ne i16 %23, 0
  %25 = sext i1 %24 to i8
  br i1 %24, label %b8, label %b9

b6:
  %26 = shl i16 %20, 1
  %27 = getelementptr i8, ptr addrspace(1) %0, i16 %26
  %28 = load i16, ptr addrspace(1) %27
  %29 = icmp eq i16 %28, 0
  %30 = sext i1 %29 to i8
  br label %b7

b7:
  %31 = phi i8 [ %19, %b3 ], [ %30, %b6 ]
  %32 = add i16 %20, 1
  br label %b2

b8:
  %33 = icmp eq i16 %13, %3
  %34 = sext i1 %33 to i8
  br i1 %33, label %b10, label %b11

b9:
  %35 = phi i8 [ %25, %b5 ], [ %39, %b11 ]
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b12, label %b14

b10:
  %37 = xor i8 %19, -1
  br label %b11

b11:
  %38 = phi i8 [ %34, %b8 ], [ %37, %b10 ]
  %39 = xor i8 %38, -1
  br label %b9

b12:
  %40 = getelementptr i8, ptr %7, i16 0
  store i8 45, ptr %40
  br label %b14

b14:
  %41 = phi i16 [ 1, %b12 ], [ 0, %b9 ]
  %42 = icmp eq i16 %13, %3
  br i1 %42, label %b15, label %b16

b15:
  %43 = icmp ne i8 %19, 0
  br i1 %43, label %b18, label %b19

b16:
  %44 = icmp eq i16 %13, 0
  %45 = sext i1 %44 to i8
  br i1 %44, label %b21, label %b22

b17:
  %46 = phi i16 [ %54, %63 ], [ %91, %b25 ]
  call addrspace(1) void @format.number(ptr %7, i16 %46)
  call addrspace(1) void @heap.release(ptr %6)
  ret void

b18:
  %47 = getelementptr i8, ptr @$str21, i16 6
  br label %b20

b19:
  %48 = getelementptr i8, ptr @$str22, i16 6
  br label %b20

b20:
  %49 = phi ptr [ %47, %b18 ], [ %48, %b19 ]
  %50 = getelementptr i8, ptr %49, i16 -4
  %51 = load i16, ptr %50
  %52 = addrspacecast ptr %49 to ptr addrspace(1)
  br label %53

53:
  %54 = phi i16 [ %41, %b20 ], [ %61, %57 ]
  %55 = phi i16 [ 0, %b20 ], [ %62, %57 ]
  %56 = icmp ult i16 %55, %51
  br i1 %56, label %57, label %63

57:
  %58 = getelementptr i8, ptr addrspace(1) %52, i16 %55
  %59 = load i8, ptr addrspace(1) %58
  %60 = getelementptr i8, ptr %7, i16 %54
  store i8 %59, ptr %60
  %61 = add i16 %54, 1
  %62 = add i16 %55, 1
  br label %53

63:
  br label %b17

b21:
  br label %b22

b22:
  %64 = phi i8 [ %45, %b16 ], [ %19, %b21 ]
  %65 = icmp ne i8 %64, 0
  br i1 %65, label %b23, label %81

b23:
  %66 = getelementptr i8, ptr @$str23, i16 6
  %67 = getelementptr i8, ptr %66, i16 -4
  %68 = load i16, ptr %67
  %69 = addrspacecast ptr %66 to ptr addrspace(1)
  br label %70

70:
  %71 = phi i16 [ %41, %b23 ], [ %78, %74 ]
  %72 = phi i16 [ 0, %b23 ], [ %79, %74 ]
  %73 = icmp ult i16 %72, %68
  br i1 %73, label %74, label %80

74:
  %75 = getelementptr i8, ptr addrspace(1) %69, i16 %72
  %76 = load i8, ptr addrspace(1) %75
  %77 = getelementptr i8, ptr %7, i16 %71
  store i8 %76, ptr %77
  %78 = add i16 %71, 1
  %79 = add i16 %72, 1
  br label %70

80:
  br label %b25

81:
  br label %82

82:
  %83 = phi i16 [ 0, %81 ], [ %88, %85 ]
  %84 = icmp slt i16 %83, 72
  br i1 %84, label %85, label %89

85:
  %86 = shl i16 %83, 1
  %87 = getelementptr i8, ptr %6, i16 %86
  store i16 0, ptr %87
  %88 = add i16 %83, 1
  br label %82

89:
  %90 = getelementptr i8, ptr %6, i16 0
  store i16 0, ptr %90
  br label %b26

b25:
  %91 = phi i16 [ %71, %80 ], [ %120, %b37 ]
  br label %b17

b26:
  %92 = phi i16 [ 0, %89 ], [ %98, %b27 ]
  %93 = icmp ult i16 %92, %8
  br i1 %93, label %b27, label %b29

b27:
  %94 = shl i16 %92, 1
  %95 = getelementptr i8, ptr %6, i16 %94
  %96 = getelementptr i8, ptr addrspace(1) %0, i16 %94
  %97 = load i16, ptr addrspace(1) %96
  store i16 %97, ptr %95
  %98 = add i16 %92, 1
  br label %b26

b29:
  %99 = getelementptr i8, ptr %6, i16 %9
  %100 = icmp ne i16 %13, 0
  br i1 %100, label %b30, label %b32

b30:
  br label %b32

b32:
  %101 = phi i16 [ %14, %b30 ], [ 0, %b29 ]
  %102 = or i16 %16, %101
  store i16 %102, ptr %99
  br i1 %100, label %b35, label %b34

b34:
  br label %b35

b35:
  %103 = phi i16 [ %13, %b32 ], [ 1, %b34 ]
  %104 = sub i16 %103, %4
  %105 = getelementptr i8, ptr %7, i16 16
  %106 = icmp ugt i16 %13, 1
  %107 = sext i1 %106 to i8
  br i1 %106, label %b36, label %b37

b36:
  br label %b37

b37:
  %108 = phi i8 [ %107, %b35 ], [ %19, %b36 ]
  %109 = getelementptr i8, ptr addrspace(1) %0, i16 0
  %110 = load i16, ptr addrspace(1) %109
  %111 = and i16 %110, 1
  %112 = icmp eq i16 %111, 0
  %113 = sext i1 %112 to i8
  %114 = call addrspace(1) i32 @floats.digits(ptr %6, i16 %104, i8 %108, i8 %113, ptr %105)
  %115 = addrspacecast ptr %5 to ptr addrspace(1)
  store i32 %114, ptr addrspace(1) %115, !tbaa !2
  %116 = load i16, ptr %5, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %5, i16 2
  %118 = load i16, ptr %117, !tbaa !2
  %119 = add i16 %118, -1
  %120 = call addrspace(1) i16 @floats.decimal(ptr %7, i16 %41, ptr %105, i16 %116, i16 %119)
  br label %b25
}

define internal i16 @floats.decimal(ptr %0, i16 %1, ptr %2, i16 %3, i16 %4) addrspace(1) willreturn {
b1:
  %5 = icmp sge i16 %4, -4
  %6 = sext i1 %5 to i8
  br i1 %5, label %b2, label %b3

b2:
  %7 = icmp slt i16 %4, 16
  %8 = sext i1 %7 to i8
  br label %b3

b3:
  %9 = phi i8 [ %6, %b1 ], [ %8, %b2 ]
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b4, label %b5

b4:
  %11 = icmp slt i16 %4, 0
  br i1 %11, label %b7, label %b9

b5:
  %12 = getelementptr i8, ptr %2, i16 0
  %13 = load i8, ptr %12
  %14 = getelementptr i8, ptr %0, i16 %1
  store i8 %13, ptr %14
  %15 = add i16 %1, 1
  %16 = icmp ugt i16 %3, 1
  br i1 %16, label %75, label %b34

b7:
  %17 = getelementptr i8, ptr @$str24, i16 6
  %18 = getelementptr i8, ptr %17, i16 -4
  %19 = load i16, ptr %18
  %20 = addrspacecast ptr %17 to ptr addrspace(1)
  br label %21

21:
  %22 = phi i16 [ %1, %b7 ], [ %29, %25 ]
  %23 = phi i16 [ 0, %b7 ], [ %30, %25 ]
  %24 = icmp ult i16 %23, %19
  br i1 %24, label %25, label %31

25:
  %26 = getelementptr i8, ptr addrspace(1) %20, i16 %23
  %27 = load i8, ptr addrspace(1) %26
  %28 = getelementptr i8, ptr %0, i16 %22
  store i8 %27, ptr %28
  %29 = add i16 %22, 1
  %30 = add i16 %23, 1
  br label %21

31:
  %32 = sub i16 0, %4
  %33 = add i16 %32, -1
  br label %b10

b9:
  %34 = add i16 %4, 1
  br label %b18

b10:
  %35 = phi i16 [ %22, %31 ], [ %40, %38 ]
  %36 = phi i16 [ 0, %31 ], [ %41, %38 ]
  %37 = icmp slt i16 %36, %33
  br i1 %37, label %38, label %b13

38:
  %39 = getelementptr i8, ptr %0, i16 %35
  store i8 48, ptr %39
  %40 = add i16 %35, 1
  %41 = add i16 %36, 1
  br label %b10

b13:
  br label %b14

b14:
  %42 = phi i16 [ %35, %b13 ], [ %48, %b15 ]
  %43 = phi i16 [ 0, %b13 ], [ %49, %b15 ]
  %44 = icmp ult i16 %43, %3
  br i1 %44, label %b15, label %b17

b15:
  %45 = getelementptr i8, ptr %2, i16 %43
  %46 = load i8, ptr %45
  %47 = getelementptr i8, ptr %0, i16 %42
  store i8 %46, ptr %47
  %48 = add i16 %42, 1
  %49 = add i16 %43, 1
  br label %b14

b17:
  ret i16 %42

b18:
  %50 = phi i16 [ %1, %b9 ], [ %62, %b24 ]
  %51 = phi i16 [ 0, %b9 ], [ %63, %b24 ]
  %52 = icmp ult i16 %51, %34
  br i1 %52, label %b19, label %54

b19:
  %53 = icmp ult i16 %51, %3
  br i1 %53, label %b22, label %b24

54:
  %55 = getelementptr i8, ptr %0, i16 %50
  store i8 46, ptr %55
  %56 = add i16 %50, 1
  %57 = icmp ule i16 %3, %34
  br i1 %57, label %64, label %b27

b22:
  %58 = getelementptr i8, ptr %2, i16 %51
  %59 = load i8, ptr %58
  br label %b24

b24:
  %60 = phi i8 [ %59, %b22 ], [ 48, %b19 ]
  %61 = getelementptr i8, ptr %0, i16 %50
  store i8 %60, ptr %61
  %62 = add i16 %50, 1
  %63 = add i16 %51, 1
  br label %b18

64:
  %65 = getelementptr i8, ptr %0, i16 %56
  store i8 48, ptr %65
  %66 = add i16 %50, 2
  ret i16 %66

b27:
  br label %b28

b28:
  %67 = phi i16 [ %56, %b27 ], [ %73, %b29 ]
  %68 = phi i16 [ %34, %b27 ], [ %74, %b29 ]
  %69 = icmp ult i16 %68, %3
  br i1 %69, label %b29, label %b31

b29:
  %70 = getelementptr i8, ptr %2, i16 %68
  %71 = load i8, ptr %70
  %72 = getelementptr i8, ptr %0, i16 %67
  store i8 %71, ptr %72
  %73 = add i16 %67, 1
  %74 = add i16 %68, 1
  br label %b28

b31:
  ret i16 %67

75:
  %76 = getelementptr i8, ptr %0, i16 %15
  store i8 46, ptr %76
  %77 = add i16 %1, 2
  br label %b35

b34:
  %78 = phi i16 [ %15, %b5 ], [ %82, %b38 ]
  %79 = getelementptr i8, ptr %0, i16 %78
  store i8 101, ptr %79
  %80 = add i16 %78, 1
  %81 = icmp slt i16 %4, 0
  br i1 %81, label %b41, label %b40

b35:
  %82 = phi i16 [ %77, %75 ], [ %88, %b36 ]
  %83 = phi i16 [ 1, %75 ], [ %89, %b36 ]
  %84 = icmp ult i16 %83, %3
  br i1 %84, label %b36, label %b38

b36:
  %85 = getelementptr i8, ptr %2, i16 %83
  %86 = load i8, ptr %85
  %87 = getelementptr i8, ptr %0, i16 %82
  store i8 %86, ptr %87
  %88 = add i16 %82, 1
  %89 = add i16 %83, 1
  br label %b35

b38:
  br label %b34

b40:
  br label %b41

b41:
  %90 = phi i8 [ 45, %b34 ], [ 43, %b40 ]
  %91 = getelementptr i8, ptr %0, i16 %80
  store i8 %90, ptr %91
  %92 = add i16 %78, 2
  br i1 %81, label %b42, label %b44

b42:
  %93 = sub i16 0, %4
  br label %b44

b44:
  %94 = phi i16 [ %93, %b42 ], [ %4, %b41 ]
  %95 = icmp uge i16 %94, 100
  br i1 %95, label %b45, label %b47

b45:
  %96 = udiv i16 %94, 100
  %97 = trunc i16 %96 to i8
  %98 = zext i8 %97 to i16
  %99 = add i16 %98, 48
  %100 = trunc i16 %99 to i8
  %101 = getelementptr i8, ptr %0, i16 %92
  store i8 %100, ptr %101
  %102 = add i16 %78, 3
  br label %b47

b47:
  %103 = phi i16 [ %102, %b45 ], [ %92, %b44 ]
  %104 = udiv i16 %94, 10
  %105 = urem i16 %104, 10
  %106 = trunc i16 %105 to i8
  %107 = zext i8 %106 to i16
  %108 = add i16 %107, 48
  %109 = trunc i16 %108 to i8
  %110 = getelementptr i8, ptr %0, i16 %103
  store i8 %109, ptr %110
  %111 = add i16 %103, 1
  %112 = urem i16 %94, 10
  %113 = trunc i16 %112 to i8
  %114 = zext i8 %113 to i16
  %115 = add i16 %114, 48
  %116 = trunc i16 %115 to i8
  %117 = getelementptr i8, ptr %0, i16 %111
  store i8 %116, ptr %117
  %118 = add i16 %103, 2
  ret i16 %118
}

define void @N$PR8(double %0) addrspace(1) {
b1:
  %1 = alloca double
  store double 0.000000e+00, ptr %1
  store double %0, ptr %1, !tbaa !2
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @floats.print_float(ptr addrspace(1) %2, i16 4, i16 4, i16 2047, i16 1075)
  ret void
}

define void @N$PR4(float %0) addrspace(1) {
b1:
  %1 = alloca float
  store float 0.000000e+00, ptr %1
  store float %0, ptr %1, !tbaa !2
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @floats.print_float(ptr addrspace(1) %2, i16 2, i16 7, i16 255, i16 150)
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
