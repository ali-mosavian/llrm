target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00raw\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00avg3\00"
@$str3 = internal constant [27 x i8] c"\08\00\14\00\14\00first pair averages \00"
@$str4 = internal constant [23 x i8] c"\08\00\10\00\10\00too few readings\00"
@$str5 = internal constant [25 x i8] c"\08\00\12\00\12\00peak of the rest: \00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [16 x i8]
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [16 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [16 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [12 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca [16 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 16, i1 false)
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 16, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 16, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 12, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  store i16 0, ptr %12
  store i16 0, ptr %13
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 16, i1 false)
  store i16 8, ptr %12, !tbaa !2
  store i16 8, ptr %13, !tbaa !2
  %15 = sub i16 0, 0
  %16 = getelementptr inbounds i16, ptr %14, i16 %15
  store i16 12, ptr %16, !tbaa !2
  %17 = sub i16 1, 0
  %18 = getelementptr inbounds i16, ptr %14, i16 %17
  store i16 15, ptr %18, !tbaa !2
  %19 = sub i16 2, 0
  %20 = getelementptr inbounds i16, ptr %14, i16 %19
  store i16 11, ptr %20, !tbaa !2
  %21 = sub i16 3, 0
  %22 = getelementptr inbounds i16, ptr %14, i16 %21
  store i16 30, ptr %22, !tbaa !2
  %23 = sub i16 4, 0
  %24 = getelementptr inbounds i16, ptr %14, i16 %23
  store i16 28, ptr %24, !tbaa !2
  %25 = sub i16 5, 0
  %26 = getelementptr inbounds i16, ptr %14, i16 %25
  store i16 27, ptr %26, !tbaa !2
  %27 = sub i16 6, 0
  %28 = getelementptr inbounds i16, ptr %14, i16 %27
  store i16 9, ptr %28, !tbaa !2
  %29 = sub i16 7, 0
  %30 = getelementptr inbounds i16, ptr %14, i16 %29
  store i16 10, ptr %30, !tbaa !2
  %31 = getelementptr i8, ptr @$str1, i16 6
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = load i16, ptr %32
  %34 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 %33, ptr %11, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %33, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %34, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %11 to ptr addrspace(1)
  %38 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 8, ptr %9, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 8, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %38, ptr %40, !tbaa !2
  %41 = addrspacecast ptr %9 to ptr addrspace(1)
  %42 = getelementptr inbounds i8, ptr %10, i16 2
  %43 = addrspacecast ptr %42 to ptr addrspace(1)
  %44 = load i16, ptr addrspace(1) %41, !tbaa !2
  store i16 %44, ptr addrspace(1) %43
  %45 = getelementptr i8, ptr addrspace(1) %41, i16 2
  %46 = load i16, ptr addrspace(1) %45, !tbaa !2
  %47 = getelementptr i8, ptr addrspace(1) %43, i16 2
  store i16 %46, ptr addrspace(1) %47
  %48 = getelementptr i8, ptr addrspace(1) %41, i16 4
  %49 = load ptr addrspace(1), ptr addrspace(1) %48, !tbaa !2
  %50 = getelementptr i8, ptr addrspace(1) %43, i16 4
  store ptr addrspace(1) %49, ptr addrspace(1) %50
  store i16 0, ptr %10, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %10, i16 10
  store i16 0, ptr %51, !tbaa !2
  %52 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @"report[$state0]"(ptr addrspace(1) %37, ptr addrspace(1) %52)
  %53 = getelementptr i8, ptr @$str2, i16 6
  %54 = getelementptr i8, ptr %53, i16 -4
  %55 = load i16, ptr %54
  %56 = addrspacecast ptr %53 to ptr addrspace(1)
  store i16 %55, ptr %8, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %55, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %56, ptr %58, !tbaa !2
  %59 = addrspacecast ptr %8 to ptr addrspace(1)
  %60 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 8, ptr %6, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 8, ptr %61, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %60, ptr %62, !tbaa !2
  %63 = addrspacecast ptr %6 to ptr addrspace(1)
  %64 = getelementptr inbounds i8, ptr %7, i16 2
  %65 = addrspacecast ptr %64 to ptr addrspace(1)
  %66 = load i16, ptr addrspace(1) %63, !tbaa !2
  store i16 %66, ptr addrspace(1) %65
  %67 = getelementptr i8, ptr addrspace(1) %63, i16 2
  %68 = load i16, ptr addrspace(1) %67, !tbaa !2
  %69 = getelementptr i8, ptr addrspace(1) %65, i16 2
  store i16 %68, ptr addrspace(1) %69
  %70 = getelementptr i8, ptr addrspace(1) %63, i16 4
  %71 = load ptr addrspace(1), ptr addrspace(1) %70, !tbaa !2
  %72 = getelementptr i8, ptr addrspace(1) %65, i16 4
  store ptr addrspace(1) %71, ptr addrspace(1) %72
  store i16 0, ptr %7, !tbaa !2
  %73 = getelementptr inbounds i8, ptr %7, i16 10
  store i16 3, ptr %73, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %7, i16 12
  store i16 0, ptr %74, !tbaa !2
  %75 = getelementptr inbounds i8, ptr %7, i16 14
  store i16 0, ptr %75, !tbaa !2
  %76 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @"report[$state1]"(ptr addrspace(1) %59, ptr addrspace(1) %76)
  %77 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 8, ptr %4, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 8, ptr %78, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %77, ptr %79, !tbaa !2
  %80 = addrspacecast ptr %4 to ptr addrspace(1)
  %81 = getelementptr inbounds i8, ptr %5, i16 2
  %82 = addrspacecast ptr %81 to ptr addrspace(1)
  %83 = load i16, ptr addrspace(1) %80, !tbaa !2
  store i16 %83, ptr addrspace(1) %82
  %84 = getelementptr i8, ptr addrspace(1) %80, i16 2
  %85 = load i16, ptr addrspace(1) %84, !tbaa !2
  %86 = getelementptr i8, ptr addrspace(1) %82, i16 2
  store i16 %85, ptr addrspace(1) %86
  %87 = getelementptr i8, ptr addrspace(1) %80, i16 4
  %88 = load ptr addrspace(1), ptr addrspace(1) %87, !tbaa !2
  %89 = getelementptr i8, ptr addrspace(1) %82, i16 4
  store ptr addrspace(1) %88, ptr addrspace(1) %89
  store i16 0, ptr %5, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %5, i16 10
  store i16 2, ptr %90, !tbaa !2
  %91 = getelementptr inbounds i8, ptr %5, i16 12
  store i16 0, ptr %91, !tbaa !2
  %92 = getelementptr inbounds i8, ptr %5, i16 14
  store i16 0, ptr %92, !tbaa !2
  %93 = addrspacecast ptr %5 to ptr addrspace(1)
  %94 = call addrspace(1) i32 @$state1.next(ptr addrspace(1) %93)
  %95 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %94, ptr addrspace(1) %95, !tbaa !2
  %96 = load i8, ptr %3, !tbaa !2
  %97 = icmp eq i8 %96, 0
  %98 = sext i1 %97 to i8
  %99 = icmp ne i8 %98, 0
  br i1 %99, label %b4, label %b3

b2:
  %100 = load i16, ptr %5, !tbaa !2
  %101 = getelementptr inbounds i8, ptr %5, i16 2
  %102 = load i16, ptr %101, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %5, i16 4
  %104 = load i16, ptr %103, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %5, i16 6
  %106 = load ptr addrspace(1), ptr %105, !tbaa !2
  %107 = getelementptr inbounds i8, ptr %5, i16 10
  %108 = load i16, ptr %107, !tbaa !2
  %109 = getelementptr inbounds i8, ptr %5, i16 12
  %110 = load i16, ptr %109, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %5, i16 14
  %112 = load i16, ptr %111, !tbaa !2
  store i16 %100, ptr %1, !tbaa !2
  %113 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %102, ptr %113, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 %104, ptr %114, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %1, i16 6
  store ptr addrspace(1) %106, ptr %115, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %1, i16 10
  store i16 %108, ptr %116, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %1, i16 12
  store i16 %110, ptr %117, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %1, i16 14
  store i16 %112, ptr %118, !tbaa !2
  %119 = addrspacecast ptr %1 to ptr addrspace(1)
  %120 = call addrspace(1) i16 @"peak[$state1]"(ptr addrspace(1) %119)
  store i16 %120, ptr %0, !tbaa !2
  %121 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %121)
  %122 = load i16, ptr %0, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %122)
  call addrspace(1) void @N$PN()
  ret i16 0

b3:
  %123 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %123)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %124 = getelementptr inbounds i8, ptr %3, i16 2
  %125 = load i16, ptr %124, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %3, i16 2
  %127 = load i16, ptr %126, !tbaa !2
  store i16 %127, ptr %2, !tbaa !2
  %128 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %128)
  %129 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %129)
  call addrspace(1) void @N$PN()
  br label %b2
}

define internal i16 @"peak[$state1]"(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca [4 x i8]
  %3 = alloca [16 x i8]
  %4 = alloca i16
  store i16 0, ptr %1
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 16, i1 false)
  store i16 0, ptr %4
  store i16 -32768, ptr %4, !tbaa !2
  %5 = load i16, ptr addrspace(1) %0
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %7 = load i16, ptr addrspace(1) %6
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 6
  %11 = load ptr addrspace(1), ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %13 = load i16, ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %15 = load i16, ptr addrspace(1) %14
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 14
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
  br label %b2

b2:
  br label %b3

b3:
  %24 = addrspacecast ptr %3 to ptr addrspace(1)
  %25 = call addrspace(1) i32 @$state1.next(ptr addrspace(1) %24)
  %26 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 %25, ptr addrspace(1) %26, !tbaa !2
  %27 = load i8, ptr %2, !tbaa !2
  %28 = icmp eq i8 %27, 0
  %29 = sext i1 %28 to i8
  %30 = icmp ne i8 %29, 0
  br i1 %30, label %b7, label %b6

b4:
  %31 = load i16, ptr %4, !tbaa !2
  ret i16 %31

b5:
  br label %b2

b6:
  br label %b4

b7:
  %32 = getelementptr inbounds i8, ptr %2, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %2, i16 2
  %35 = load i16, ptr %34, !tbaa !2
  store i16 %35, ptr %1, !tbaa !2
  %36 = load i16, ptr %1, !tbaa !2
  %37 = load i16, ptr %4, !tbaa !2
  %38 = icmp sgt i16 %36, %37
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b8, label %b9

b8:
  %41 = load i16, ptr %1, !tbaa !2
  store i16 %41, ptr %4, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  br label %b5
}

define internal void @"report[$state1]"(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = alloca [16 x i8]
  %5 = alloca ptr
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 16, i1 false)
  store ptr null, ptr %5
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PFLD(i8 8, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %6 = call addrspace(1) ptr @N$PEND()
  store ptr %6, ptr %5, !tbaa !2
  %7 = load i16, ptr addrspace(1) %1
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %11 = load i16, ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 6
  %13 = load ptr addrspace(1), ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %15 = load i16, ptr addrspace(1) %14
  %16 = getelementptr i8, ptr addrspace(1) %1, i16 12
  %17 = load i16, ptr addrspace(1) %16
  %18 = getelementptr i8, ptr addrspace(1) %1, i16 14
  %19 = load i16, ptr addrspace(1) %18
  store i16 %7, ptr %4, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %9, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 %11, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %4, i16 6
  store ptr addrspace(1) %13, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %4, i16 10
  store i16 %15, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %4, i16 12
  store i16 %17, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %4, i16 14
  store i16 %19, ptr %25, !tbaa !2
  br label %b2

b2:
  br label %b3

b3:
  %26 = addrspacecast ptr %4 to ptr addrspace(1)
  %27 = call addrspace(1) i32 @$state1.next(ptr addrspace(1) %26)
  %28 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %27, ptr addrspace(1) %28, !tbaa !2
  %29 = load i8, ptr %3, !tbaa !2
  %30 = icmp eq i8 %29, 0
  %31 = sext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b7, label %b6

b4:
  %33 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$PS(ptr %33)
  call addrspace(1) void @N$PN()
  %34 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %34)
  ret void

b5:
  br label %b2

b6:
  br label %b4

b7:
  %35 = getelementptr inbounds i8, ptr %3, i16 2
  %36 = load i16, ptr %35, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  %38 = load i16, ptr %37, !tbaa !2
  store i16 %38, ptr %2, !tbaa !2
  call addrspace(1) void @N$PBEG()
  %39 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %39)
  %40 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %40)
  %41 = call addrspace(1) ptr @N$PEND()
  %42 = load ptr, ptr %5, !tbaa !2
  %43 = call addrspace(1) ptr @N$TAPP(ptr %42, ptr %41)
  store ptr %43, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %41)
  br label %b5
}

define internal i32 @$state1.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  store i16 0, ptr %1
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  br label %b2

b2:
  br label %b3

b3:
  %5 = load i16, ptr addrspace(1) %0
  %6 = icmp eq i16 %5, 0
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b7, label %b6

b6:
  %9 = icmp eq i16 %5, 2
  %10 = sext i1 %9 to i8
  %11 = icmp ne i8 %10, 0
  br i1 %11, label %b9, label %b8

b7:
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 12
  store i16 0, ptr addrspace(1) %12
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %13 = icmp eq i16 %5, 3
  %14 = sext i1 %13 to i8
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b14, label %b13

b9:
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %17 = load i16, ptr addrspace(1) %16
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %19 = load i16, ptr addrspace(1) %18
  %20 = add i16 %17, %19
  %21 = load i16, ptr addrspace(1) %4
  %22 = icmp ule i16 %20, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %0
  br label %b2

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b13:
  %25 = icmp eq i16 %5, 4
  %26 = sext i1 %25 to i8
  %27 = icmp ne i8 %26, 0
  br i1 %27, label %b22, label %b21

b14:
  %28 = getelementptr i8, ptr addrspace(1) %0, i16 14
  store i16 0, ptr addrspace(1) %28
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %30 = load i16, ptr addrspace(1) %29
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %32 = load i16, ptr addrspace(1) %31
  %33 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %34 = load i16, ptr addrspace(1) %33
  %35 = add i16 %32, %34
  store i16 %30, ptr %2, !tbaa !2
  store i16 %35, ptr %1, !tbaa !2
  br label %b15

b15:
  %36 = load i16, ptr %2, !tbaa !2
  %37 = load i16, ptr %1, !tbaa !2
  %38 = icmp ult i16 %36, %37
  %39 = sext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b16, label %b18

b16:
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %42 = load i16, ptr addrspace(1) %41
  %43 = load i16, ptr %2, !tbaa !2
  %44 = load i16, ptr addrspace(1) %4
  %45 = icmp ult i16 %43, %44
  %46 = sext i1 %45 to i8
  %47 = icmp ne i8 %46, 0
  br i1 %47, label %b19, label %b20

b17:
  %48 = load i16, ptr %2, !tbaa !2
  %49 = add i16 %48, 1
  store i16 %49, ptr %2, !tbaa !2
  br label %b15

b18:
  store i16 5, ptr addrspace(1) %0
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %51 = load i16, ptr addrspace(1) %50
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %53 = load i16, ptr addrspace(1) %52
  %54 = sdiv i16 %51, %53
  %55 = srem i16 %51, %53
  %56 = icmp ne i16 %55, 0
  %57 = sext i1 %56 to i8
  %58 = xor i16 %55, %53
  %59 = icmp slt i16 %58, 0
  %60 = sext i1 %59 to i8
  %61 = and i8 %57, %60
  %62 = sext i8 %61 to i16
  %63 = and i16 %62, 1
  %64 = sub i16 %54, %63
  store i8 0, ptr %3, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %64, ptr %65, !tbaa !2
  %66 = addrspacecast ptr %3 to ptr addrspace(1)
  %67 = load i32, ptr addrspace(1) %66, !tbaa !2
  ret i32 %67

b19:
  %68 = getelementptr i8, ptr addrspace(1) %4, i16 4
  %69 = load ptr addrspace(1), ptr addrspace(1) %68
  %70 = mul i16 %43, 2
  %71 = getelementptr i8, ptr addrspace(1) %69, i16 %70
  %72 = load i16, ptr addrspace(1) %71
  %73 = add i16 %42, %72
  %74 = getelementptr i8, ptr addrspace(1) %0, i16 14
  store i16 %73, ptr addrspace(1) %74
  br label %b17

b20:
  call addrspace(1) void @N$EBND()
  unreachable

b21:
  %75 = icmp eq i16 %5, 5
  %76 = sext i1 %75 to i8
  %77 = icmp ne i8 %76, 0
  br i1 %77, label %b24, label %b23

b22:
  store i16 1, ptr addrspace(1) %0
  br label %b2

b23:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %3, !tbaa !2
  %78 = addrspacecast ptr %3 to ptr addrspace(1)
  %79 = load i32, ptr addrspace(1) %78, !tbaa !2
  ret i32 %79

b24:
  %80 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %81 = load i16, ptr addrspace(1) %80
  %82 = add i16 %81, 1
  %83 = getelementptr i8, ptr addrspace(1) %0, i16 12
  store i16 %82, ptr addrspace(1) %83
  store i16 2, ptr addrspace(1) %0
  br label %b2
}

define internal void @"report[$state0]"(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = alloca [12 x i8]
  %5 = alloca ptr
  store i16 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 12, i1 false)
  store ptr null, ptr %5
  call addrspace(1) void @N$PBEG()
  call addrspace(1) void @N$PFLD(i8 8, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %6 = call addrspace(1) ptr @N$PEND()
  store ptr %6, ptr %5, !tbaa !2
  %7 = load i16, ptr addrspace(1) %1
  %8 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %9 = load i16, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %11 = load i16, ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 6
  %13 = load ptr addrspace(1), ptr addrspace(1) %12
  %14 = getelementptr i8, ptr addrspace(1) %1, i16 10
  %15 = load i16, ptr addrspace(1) %14
  store i16 %7, ptr %4, !tbaa !2
  %16 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %9, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %4, i16 4
  store i16 %11, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %4, i16 6
  store ptr addrspace(1) %13, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %4, i16 10
  store i16 %15, ptr %19, !tbaa !2
  br label %b2

b2:
  br label %b3

b3:
  %20 = addrspacecast ptr %4 to ptr addrspace(1)
  %21 = call addrspace(1) i32 @$state0.next(ptr addrspace(1) %20)
  %22 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %21, ptr addrspace(1) %22, !tbaa !2
  %23 = load i8, ptr %3, !tbaa !2
  %24 = icmp eq i8 %23, 0
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b7, label %b6

b4:
  %27 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$PS(ptr %27)
  call addrspace(1) void @N$PN()
  %28 = load ptr, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %28)
  ret void

b5:
  br label %b2

b6:
  br label %b4

b7:
  %29 = getelementptr inbounds i8, ptr %3, i16 2
  %30 = load i16, ptr %29, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %3, i16 2
  %32 = load i16, ptr %31, !tbaa !2
  store i16 %32, ptr %2, !tbaa !2
  call addrspace(1) void @N$PBEG()
  %33 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %33)
  %34 = load i16, ptr %2, !tbaa !2
  call addrspace(1) void @N$PFLD(i8 3, i8 10, i8 32, i8 0)
  call addrspace(1) void @N$PI2(i16 %34)
  %35 = call addrspace(1) ptr @N$PEND()
  %36 = load ptr, ptr %5, !tbaa !2
  %37 = call addrspace(1) ptr @N$TAPP(ptr %36, ptr %35)
  store ptr %37, ptr %5, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %35)
  br label %b5
}

define internal i32 @$state0.next(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca [4 x i8]
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 4, i1 false)
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  br label %b2

b2:
  br label %b3

b3:
  %3 = load i16, ptr addrspace(1) %0
  %4 = icmp eq i16 %3, 0
  %5 = sext i1 %4 to i8
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b7, label %b6

b6:
  %7 = icmp eq i16 %3, 2
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b9, label %b8

b7:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store i16 0, ptr addrspace(1) %10
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %11 = icmp eq i16 %3, 3
  %12 = sext i1 %11 to i8
  %13 = icmp ne i8 %12, 0
  br i1 %13, label %b14, label %b13

b9:
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %15 = load i16, ptr addrspace(1) %14
  %16 = load i16, ptr addrspace(1) %2
  %17 = icmp ult i16 %15, %16
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %0
  br label %b2

b11:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b13:
  %20 = icmp eq i16 %3, 4
  %21 = sext i1 %20 to i8
  %22 = icmp ne i8 %21, 0
  br i1 %22, label %b18, label %b17

b14:
  store i16 6, ptr addrspace(1) %0
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %24 = load i16, ptr addrspace(1) %23
  %25 = load i16, ptr addrspace(1) %2
  %26 = icmp ult i16 %24, %25
  %27 = sext i1 %26 to i8
  %28 = icmp ne i8 %27, 0
  br i1 %28, label %b15, label %b16

b15:
  %29 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %30 = load ptr addrspace(1), ptr addrspace(1) %29
  %31 = mul i16 %24, 2
  %32 = getelementptr i8, ptr addrspace(1) %30, i16 %31
  %33 = load i16, ptr addrspace(1) %32
  store i8 0, ptr %1, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %33, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %1 to ptr addrspace(1)
  %36 = load i32, ptr addrspace(1) %35, !tbaa !2
  ret i32 %36

b16:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %37 = icmp eq i16 %3, 5
  %38 = sext i1 %37 to i8
  %39 = icmp ne i8 %38, 0
  br i1 %39, label %b20, label %b19

b18:
  %40 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %41 = load i16, ptr addrspace(1) %40
  %42 = add i16 %41, 1
  %43 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store i16 %42, ptr addrspace(1) %43
  store i16 2, ptr addrspace(1) %0
  br label %b2

b19:
  %44 = icmp eq i16 %3, 6
  %45 = sext i1 %44 to i8
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b22, label %b21

b20:
  store i16 1, ptr addrspace(1) %0
  br label %b2

b21:
  store i16 1, ptr addrspace(1) %0
  store i8 1, ptr %1, !tbaa !2
  %47 = addrspacecast ptr %1 to ptr addrspace(1)
  %48 = load i32, ptr addrspace(1) %47, !tbaa !2
  ret i32 %48

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
