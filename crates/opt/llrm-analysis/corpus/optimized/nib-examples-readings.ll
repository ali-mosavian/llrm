target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00raw\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00avg3\00"
@$str3 = internal constant [27 x i8] c"\08\00\14\00\14\00first pair averages \00"
@$str4 = internal constant [23 x i8] c"\08\00\10\00\10\00too few readings\00"
@$str5 = internal constant [25 x i8] c"\08\00\12\00\12\00peak of the rest: \00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca [16 x i8]
  %1 = alloca [4 x i8]
  %2 = alloca [16 x i8]
  %3 = alloca [16 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [12 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [16 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 16, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 16, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 16, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 12, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 16, i1 false)
  %8 = getelementptr inbounds i16, ptr %7, i16 0
  store i16 12, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i16, ptr %7, i16 1
  store i16 15, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i16, ptr %7, i16 2
  store i16 11, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %7, i16 3
  store i16 30, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i16, ptr %7, i16 4
  store i16 28, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i16, ptr %7, i16 5
  store i16 27, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i16, ptr %7, i16 6
  store i16 9, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i16, ptr %7, i16 7
  store i16 10, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$str1, i16 6
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 %18, ptr %6, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %18, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %19, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %6 to ptr addrspace(1)
  %23 = addrspacecast ptr %7 to ptr addrspace(1)
  %24 = getelementptr inbounds i8, ptr %5, i16 2
  %25 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 8, ptr addrspace(1) %25
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 2
  store i16 8, ptr addrspace(1) %26
  %27 = getelementptr i8, ptr addrspace(1) %25, i16 4
  store ptr addrspace(1) %23, ptr addrspace(1) %27
  store i16 0, ptr %5, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %5, i16 10
  store i16 0, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @"report[$state0]"(ptr addrspace(1) %22, ptr addrspace(1) %29)
  %30 = getelementptr i8, ptr @$str2, i16 6
  %31 = getelementptr i8, ptr %30, i16 -4
  %32 = load i16, ptr %31
  %33 = addrspacecast ptr %30 to ptr addrspace(1)
  store i16 %32, ptr %4, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %32, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %4 to ptr addrspace(1)
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  %38 = addrspacecast ptr %37 to ptr addrspace(1)
  store i16 8, ptr addrspace(1) %38
  %39 = getelementptr i8, ptr addrspace(1) %38, i16 2
  store i16 8, ptr addrspace(1) %39
  %40 = getelementptr i8, ptr addrspace(1) %38, i16 4
  store ptr addrspace(1) %23, ptr addrspace(1) %40
  store i16 0, ptr %3, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %3, i16 10
  store i16 3, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %3, i16 12
  store i16 0, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %3, i16 14
  store i16 0, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @"report[$state1]"(ptr addrspace(1) %36, ptr addrspace(1) %44)
  %45 = getelementptr inbounds i8, ptr %2, i16 2
  %46 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 8, ptr addrspace(1) %46
  %47 = getelementptr i8, ptr addrspace(1) %46, i16 2
  store i16 8, ptr addrspace(1) %47
  %48 = getelementptr i8, ptr addrspace(1) %46, i16 4
  store ptr addrspace(1) %23, ptr addrspace(1) %48
  store i16 0, ptr %2, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %2, i16 10
  store i16 2, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %2, i16 12
  store i16 0, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %2, i16 14
  store i16 0, ptr %51, !tbaa !2
  %52 = addrspacecast ptr %2 to ptr addrspace(1)
  %53 = call addrspace(1) i32 @$state1.next(ptr addrspace(1) %52)
  %54 = addrspacecast ptr %1 to ptr addrspace(1)
  store i32 %53, ptr addrspace(1) %54, !tbaa !2
  %55 = load i8, ptr %1, !tbaa !2
  %56 = icmp eq i8 %55, 0
  br i1 %56, label %b4, label %b3

b2:
  %57 = load i16, ptr %2, !tbaa !2
  %58 = load i16, ptr %45, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %2, i16 4
  %60 = load i16, ptr %59, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %2, i16 6
  %62 = load ptr addrspace(1), ptr %61, !tbaa !2
  %63 = load i16, ptr %49, !tbaa !2
  %64 = load i16, ptr %50, !tbaa !2
  %65 = load i16, ptr %51, !tbaa !2
  store i16 %57, ptr %0, !tbaa !2
  %66 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 %58, ptr %66, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %0, i16 4
  store i16 %60, ptr %67, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %0, i16 6
  store ptr addrspace(1) %62, ptr %68, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %0, i16 10
  store i16 %63, ptr %69, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %0, i16 12
  store i16 %64, ptr %70, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %0, i16 14
  store i16 %65, ptr %71, !tbaa !2
  %72 = addrspacecast ptr %0 to ptr addrspace(1)
  %73 = call addrspace(1) i16 @"peak[$state1]"(ptr addrspace(1) %72)
  %74 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %74)
  call addrspace(1) void @N$PI2(i16 %73)
  call addrspace(1) void @N$PN()
  ret i16 0

b3:
  %75 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %75)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %76 = getelementptr inbounds i8, ptr %1, i16 2
  %77 = load i16, ptr %76, !tbaa !2
  %78 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  call addrspace(1) void @N$PI2(i16 %77)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal i16 @"peak[$state1]"(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  %2 = alloca [16 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 16, i1 false)
  %3 = load i16, ptr addrspace(1) %0
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %11 = load i16, ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %13 = load i16, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %15 = load i16, ptr addrspace(1) %14
  store i16 %3, ptr %2, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %5, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %2, i16 4
  store i16 %7, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %2, i16 6
  store ptr addrspace(1) %9, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %2, i16 10
  store i16 %11, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %2, i16 12
  store i16 %13, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %2, i16 14
  store i16 %15, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %2 to ptr addrspace(1)
  %23 = addrspacecast ptr %1 to ptr addrspace(1)
  %24 = getelementptr inbounds i8, ptr %1, i16 2
  br label %b2

b2:
  %25 = phi i16 [ -32768, %b1 ], [ %31, %b10 ]
  %26 = call addrspace(1) i32 @$state1.next(ptr addrspace(1) %22)
  store i32 %26, ptr addrspace(1) %23, !tbaa !2
  %27 = load i8, ptr %1, !tbaa !2
  %28 = icmp eq i8 %27, 0
  br i1 %28, label %b7, label %b4

b4:
  ret i16 %25

b7:
  %29 = load i16, ptr %24, !tbaa !2
  %30 = icmp sgt i16 %29, %25
  br i1 %30, label %b10, label %b9

b9:
  br label %b10

b10:
  %31 = phi i16 [ %29, %b7 ], [ %25, %b9 ]
  br label %b2
}

define internal void @"report[$state1]"(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca [4 x i8]
  %3 = alloca [16 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 16, i1 false)
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PFLD(i8 8, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %4 = call addrspace(1) ptr @N$PEND()
  %5 = load i16, ptr addrspace(1) %1
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 6
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %13 = load i16, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %15 = load i16, ptr addrspace(1) %14
  %16 = getelementptr i8, ptr addrspace(1) %1, i16 14
  %17 = load i16, ptr addrspace(1) %16
  store i16 %5, ptr %3, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %7, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %3, i16 4
  store i16 %9, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %3, i16 6
  store ptr addrspace(1) %11, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %3, i16 10
  store i16 %13, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %3, i16 12
  store i16 %15, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %3, i16 14
  store i16 %17, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %3 to ptr addrspace(1)
  %25 = addrspacecast ptr %2 to ptr addrspace(1)
  %26 = getelementptr inbounds i8, ptr %2, i16 2
  %27 = getelementptr i8, ptr @$str6, i16 6
  br label %b2

b2:
  %28 = phi ptr [ %4, %b1 ], [ %34, %b7 ]
  %29 = call addrspace(1) i32 @$state1.next(ptr addrspace(1) %24)
  store i32 %29, ptr addrspace(1) %25, !tbaa !2
  %30 = load i8, ptr %2, !tbaa !2
  %31 = icmp eq i8 %30, 0
  br i1 %31, label %b7, label %b4

b4:
  call addrspace(1) void @N$PS(ptr %28)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %28)
  ret void

b7:
  %32 = load i16, ptr %26, !tbaa !2
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PS(ptr %27)
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %32)
  %33 = call addrspace(1) ptr @N$PEND()
  %34 = call addrspace(1) ptr @N$TAPP(ptr %28, ptr %33)
  call addrspace(1) void @N$BDRP(ptr %33)
  br label %b2
}

define internal i32 @$state1.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 10
  br label %b2

b2:
  %5 = load i16, ptr addrspace(1) %0
  %6 = icmp eq i16 %5, 0
  br i1 %6, label %b7, label %b6

b6:
  %7 = icmp eq i16 %5, 2
  br i1 %7, label %b9, label %b8

b7:
  store i16 0, ptr addrspace(1) %3
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %8 = icmp eq i16 %5, 3
  br i1 %8, label %b14, label %b13

b9:
  %9 = load i16, ptr addrspace(1) %3
  %10 = load i16, ptr addrspace(1) %4
  %11 = add i16 %9, %10
  %12 = load i16, ptr addrspace(1) %2
  %13 = icmp ule i16 %11, %12
  br i1 %13, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %0
  br label %b2

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b13:
  %14 = icmp eq i16 %5, 4
  br i1 %14, label %b22, label %b21

b14:
  %15 = getelementptr i8, ptr addrspace(1) %0, i16 14
  store i16 0, ptr addrspace(1) %15
  %16 = load i16, ptr addrspace(1) %3
  %17 = load i16, ptr addrspace(1) %4
  %18 = add i16 %16, %17
  %19 = getelementptr i8, ptr addrspace(1) %2, i16 4
  br label %b15

b15:
  %20 = phi i16 [ %16, %b14 ], [ %46, %b19 ]
  %21 = icmp ult i16 %20, %18
  br i1 %21, label %b16, label %b18

b16:
  %22 = load i16, ptr addrspace(1) %15
  %23 = load i16, ptr addrspace(1) %2
  %24 = icmp ult i16 %20, %23
  br i1 %24, label %b19, label %b20

b18:
  store i16 5, ptr addrspace(1) %0
  %25 = load i16, ptr addrspace(1) %15
  %26 = load i16, ptr addrspace(1) %4
  %27 = sdiv i16 %25, %26
  %28 = srem i16 %25, %26
  %29 = icmp ne i16 %28, 0
  %30 = sext i1 %29 to i8
  %31 = xor i16 %28, %26
  %32 = icmp slt i16 %31, 0
  %33 = sext i1 %32 to i8
  %34 = and i8 %30, %33
  %35 = sext i8 %34 to i16
  %36 = and i16 %35, 1
  %37 = sub i16 %27, %36
  store i8 0, ptr %1, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %37, ptr %38, !tbaa !2
  %39 = addrspacecast ptr %1 to ptr addrspace(1)
  %40 = load i32, ptr addrspace(1) %39, !tbaa !2
  ret i32 %40

b19:
  %41 = load ptr addrspace(1), ptr addrspace(1) %19
  %42 = shl i16 %20, 1
  %43 = getelementptr i8, ptr addrspace(1) %41, i16 %42
  %44 = load i16, ptr addrspace(1) %43
  %45 = add i16 %22, %44
  store i16 %45, ptr addrspace(1) %15
  %46 = add i16 %20, 1
  br label %b15

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  %47 = icmp eq i16 %5, 5
  br i1 %47, label %b24, label %b23

b22:
  store i16 1, ptr addrspace(1) %0
  br label %b2

b23:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %1, !tbaa !2
  %48 = addrspacecast ptr %1 to ptr addrspace(1)
  %49 = load i32, ptr addrspace(1) %48, !tbaa !2
  ret i32 %49

b24:
  %50 = load i16, ptr addrspace(1) %3
  %51 = add i16 %50, 1
  store i16 %51, ptr addrspace(1) %3
  store i16 2, ptr addrspace(1) %0
  br label %b2
}

define internal void @"report[$state0]"(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca [4 x i8]
  %3 = alloca [12 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 12, i1 false)
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PFLD(i8 8, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %4 = call addrspace(1) ptr @N$PEND()
  %5 = load i16, ptr addrspace(1) %1
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 6
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %13 = load i16, ptr addrspace(1) %12
  store i16 %5, ptr %3, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %7, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %3, i16 4
  store i16 %9, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %3, i16 6
  store ptr addrspace(1) %11, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %3, i16 10
  store i16 %13, ptr %17, !tbaa !2
  %18 = addrspacecast ptr %3 to ptr addrspace(1)
  %19 = addrspacecast ptr %2 to ptr addrspace(1)
  %20 = getelementptr inbounds i8, ptr %2, i16 2
  %21 = getelementptr i8, ptr @$str6, i16 6
  br label %b2

b2:
  %22 = phi ptr [ %4, %b1 ], [ %28, %b7 ]
  %23 = call addrspace(1) i32 @$state0.next(ptr addrspace(1) %18)
  store i32 %23, ptr addrspace(1) %19, !tbaa !2
  %24 = load i8, ptr %2, !tbaa !2
  %25 = icmp eq i8 %24, 0
  br i1 %25, label %b7, label %b4

b4:
  call addrspace(1) void @N$PS(ptr %22)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %22)
  ret void

b7:
  %26 = load i16, ptr %20, !tbaa !2
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PS(ptr %21)
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %26)
  %27 = call addrspace(1) ptr @N$PEND()
  %28 = call addrspace(1) ptr @N$TAPP(ptr %22, ptr %27)
  call addrspace(1) void @N$BDRP(ptr %27)
  br label %b2
}

define internal i32 @$state0.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 10
  br label %b2

b2:
  %4 = load i16, ptr addrspace(1) %0
  %5 = icmp eq i16 %4, 0
  br i1 %5, label %b7, label %b6

b6:
  %6 = icmp eq i16 %4, 2
  br i1 %6, label %b9, label %b8

b7:
  store i16 0, ptr addrspace(1) %3
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %7 = icmp eq i16 %4, 3
  br i1 %7, label %b14, label %b13

b9:
  %8 = load i16, ptr addrspace(1) %3
  %9 = load i16, ptr addrspace(1) %2
  %10 = icmp ult i16 %8, %9
  br i1 %10, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %0
  br label %b2

b11:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b13:
  %11 = icmp eq i16 %4, 4
  br i1 %11, label %b18, label %b17

b14:
  store i16 6, ptr addrspace(1) %0
  %12 = load i16, ptr addrspace(1) %3
  %13 = load i16, ptr addrspace(1) %2
  %14 = icmp ult i16 %12, %13
  br i1 %14, label %b15, label %b16

b15:
  %15 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %16 = load ptr addrspace(1), ptr addrspace(1) %15
  %17 = shl i16 %12, 1
  %18 = getelementptr i8, ptr addrspace(1) %16, i16 %17
  %19 = load i16, ptr addrspace(1) %18
  store i8 0, ptr %1, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %19, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(1)
  %22 = load i32, ptr addrspace(1) %21, !tbaa !2
  ret i32 %22

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %23 = icmp eq i16 %4, 5
  br i1 %23, label %b20, label %b19

b18:
  %24 = load i16, ptr addrspace(1) %3
  %25 = add i16 %24, 1
  store i16 %25, ptr addrspace(1) %3
  store i16 2, ptr addrspace(1) %0
  br label %b2

b19:
  %26 = icmp eq i16 %4, 6
  br i1 %26, label %b22, label %b21

b20:
  store i16 1, ptr addrspace(1) %0
  br label %b2

b21:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %1, !tbaa !2
  %27 = addrspacecast ptr %1 to ptr addrspace(1)
  %28 = load i32, ptr addrspace(1) %27, !tbaa !2
  ret i32 %28

b22:
  store i16 4, ptr addrspace(1) %0
  br label %b2
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PBEG() addrspace(1)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

declare ptr @N$PEND() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare ptr @N$TAPP(ptr, ptr) addrspace(1)

declare void @N$EBND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
