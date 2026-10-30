target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00raw\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00avg3\00"
@$str3 = internal constant [27 x i8] c"\08\00\14\00\14\00first pair averages \00"
@$str4 = internal constant [23 x i8] c"\08\00\10\00\10\00too few readings\00"
@$str5 = internal constant [25 x i8] c"\08\00\12\00\12\00peak of the rest: \00"
@$str6 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  store i16 8, ptr %12, !tbaa !2
  store i16 8, ptr %13, !tbaa !2
  %15 = getelementptr inbounds i16, ptr %14, i16 0
  store i16 12, ptr %15, !tbaa !2
  %16 = getelementptr inbounds i16, ptr %14, i16 1
  store i16 15, ptr %16, !tbaa !2
  %17 = getelementptr inbounds i16, ptr %14, i16 2
  store i16 11, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i16, ptr %14, i16 3
  store i16 30, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i16, ptr %14, i16 4
  store i16 28, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i16, ptr %14, i16 5
  store i16 27, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i16, ptr %14, i16 6
  store i16 9, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i16, ptr %14, i16 7
  store i16 10, ptr %22, !tbaa !2
  %23 = getelementptr i8, ptr @$str1, i16 6
  %24 = getelementptr i8, ptr %23, i16 -4
  %25 = load i16, ptr %24
  %26 = addrspacecast ptr %23 to ptr addrspace(1)
  store i16 %25, ptr %11, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 %25, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %11 to ptr addrspace(1)
  %30 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 8, ptr %9, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 8, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %30, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %9 to ptr addrspace(1)
  %34 = getelementptr inbounds i8, ptr %10, i16 2
  %35 = addrspacecast ptr %34 to ptr addrspace(1)
  %36 = load i16, ptr addrspace(1) %33, !tbaa !2
  store i16 %36, ptr addrspace(1) %35
  %37 = getelementptr i8, ptr addrspace(1) %33, i16 2
  %38 = load i16, ptr addrspace(1) %37, !tbaa !2
  %39 = getelementptr i8, ptr addrspace(1) %35, i16 2
  store i16 %38, ptr addrspace(1) %39
  %40 = getelementptr i8, ptr addrspace(1) %33, i16 4
  %41 = load ptr addrspace(1), ptr addrspace(1) %40, !tbaa !2
  %42 = getelementptr i8, ptr addrspace(1) %35, i16 4
  store ptr addrspace(1) %41, ptr addrspace(1) %42
  store i16 0, ptr %10, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %10, i16 10
  store i16 0, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %10 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @"report[$state0]"(ptr addrspace(1) %29, ptr addrspace(1) %44)
  %45 = getelementptr i8, ptr @$str2, i16 6
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %8, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %47, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %48, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %8 to ptr addrspace(1)
  %52 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 8, ptr %6, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 8, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %52, ptr %54, !tbaa !2
  %55 = addrspacecast ptr %6 to ptr addrspace(1)
  %56 = getelementptr inbounds i8, ptr %7, i16 2
  %57 = addrspacecast ptr %56 to ptr addrspace(1)
  %58 = load i16, ptr addrspace(1) %55, !tbaa !2
  store i16 %58, ptr addrspace(1) %57
  %59 = getelementptr i8, ptr addrspace(1) %55, i16 2
  %60 = load i16, ptr addrspace(1) %59, !tbaa !2
  %61 = getelementptr i8, ptr addrspace(1) %57, i16 2
  store i16 %60, ptr addrspace(1) %61
  %62 = getelementptr i8, ptr addrspace(1) %55, i16 4
  %63 = load ptr addrspace(1), ptr addrspace(1) %62, !tbaa !2
  %64 = getelementptr i8, ptr addrspace(1) %57, i16 4
  store ptr addrspace(1) %63, ptr addrspace(1) %64
  store i16 0, ptr %7, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %7, i16 10
  store i16 3, ptr %65, !tbaa !2
  %66 = getelementptr inbounds i8, ptr %7, i16 12
  store i16 0, ptr %66, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %7, i16 14
  store i16 0, ptr %67, !tbaa !2
  %68 = addrspacecast ptr %7 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @"report[$state1]"(ptr addrspace(1) %51, ptr addrspace(1) %68)
  %69 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 8, ptr %4, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 8, ptr %70, !tbaa !2
  %71 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %69, ptr %71, !tbaa !2
  %72 = addrspacecast ptr %4 to ptr addrspace(1)
  %73 = getelementptr inbounds i8, ptr %5, i16 2
  %74 = addrspacecast ptr %73 to ptr addrspace(1)
  %75 = load i16, ptr addrspace(1) %72, !tbaa !2
  store i16 %75, ptr addrspace(1) %74
  %76 = getelementptr i8, ptr addrspace(1) %72, i16 2
  %77 = load i16, ptr addrspace(1) %76, !tbaa !2
  %78 = getelementptr i8, ptr addrspace(1) %74, i16 2
  store i16 %77, ptr addrspace(1) %78
  %79 = getelementptr i8, ptr addrspace(1) %72, i16 4
  %80 = load ptr addrspace(1), ptr addrspace(1) %79, !tbaa !2
  %81 = getelementptr i8, ptr addrspace(1) %74, i16 4
  store ptr addrspace(1) %80, ptr addrspace(1) %81
  store i16 0, ptr %5, !tbaa !2
  %82 = getelementptr inbounds i8, ptr %5, i16 10
  store i16 2, ptr %82, !tbaa !2
  %83 = getelementptr inbounds i8, ptr %5, i16 12
  store i16 0, ptr %83, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %5, i16 14
  store i16 0, ptr %84, !tbaa !2
  %85 = addrspacecast ptr %5 to ptr addrspace(1)
  %86 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(1) %85)
  %87 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %86, ptr addrspace(1) %87, !tbaa !2
  %88 = load i8, ptr %3, !tbaa !2, !range !7
  %89 = icmp eq i8 %88, 0
  br i1 %89, label %b4, label %b3

b2:
  %90 = load i16, ptr %5, !tbaa !2
  %91 = getelementptr inbounds i8, ptr %5, i16 2
  %92 = load i16, ptr %91, !tbaa !2
  %93 = getelementptr inbounds i8, ptr %5, i16 4
  %94 = load i16, ptr %93, !tbaa !2
  %95 = getelementptr inbounds i8, ptr %5, i16 6
  %96 = load ptr addrspace(1), ptr %95, !tbaa !2
  %97 = getelementptr inbounds i8, ptr %5, i16 10
  %98 = load i16, ptr %97, !tbaa !2
  %99 = getelementptr inbounds i8, ptr %5, i16 12
  %100 = load i16, ptr %99, !tbaa !2
  %101 = getelementptr inbounds i8, ptr %5, i16 14
  %102 = load i16, ptr %101, !tbaa !2
  store i16 %90, ptr %1, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %92, ptr %103, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %1, i16 4
  store i16 %94, ptr %104, !tbaa !2
  %105 = getelementptr inbounds i8, ptr %1, i16 6
  store ptr addrspace(1) %96, ptr %105, !tbaa !2
  %106 = getelementptr inbounds i8, ptr %1, i16 10
  store i16 %98, ptr %106, !tbaa !2
  %107 = getelementptr inbounds i8, ptr %1, i16 12
  store i16 %100, ptr %107, !tbaa !2
  %108 = getelementptr inbounds i8, ptr %1, i16 14
  store i16 %102, ptr %108, !tbaa !2
  %109 = addrspacecast ptr %1 to ptr addrspace(1)
  %110 = call cc 1005 addrspace(1) i16 @"peak[$state1]"(ptr addrspace(1) %109)
  store i16 %110, ptr %0, !tbaa !2
  %111 = getelementptr i8, ptr @$str5, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %111)
  %112 = load i16, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %112)
  call cc 1005 addrspace(1) void @N$PN()
  ret i16 0

b3:
  %113 = getelementptr i8, ptr @$str4, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %113)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %114 = getelementptr inbounds i8, ptr %3, i16 2
  %115 = load i16, ptr %114, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %3, i16 2
  %117 = load i16, ptr %116, !tbaa !2
  store i16 %117, ptr %2, !tbaa !2
  %118 = getelementptr i8, ptr @$str3, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %118)
  %119 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %119)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2
}

define internal cc 1005 i16 @"peak[$state1]"(ptr addrspace(1) noalias %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca [4 x i8]
  %3 = alloca [16 x i8]
  %4 = alloca i16
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
  %25 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(1) %24)
  %26 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 %25, ptr addrspace(1) %26, !tbaa !2
  %27 = load i8, ptr %2, !tbaa !2, !range !8
  %28 = icmp eq i8 %27, 0
  br i1 %28, label %b7, label %b6

b4:
  %29 = load i16, ptr %4, !tbaa !2
  ret i16 %29

b5:
  br label %b2

b6:
  br label %b4

b7:
  %30 = getelementptr inbounds i8, ptr %2, i16 2
  %31 = load i16, ptr %30, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %2, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  store i16 %33, ptr %1, !tbaa !2
  %34 = load i16, ptr %1, !tbaa !2
  %35 = load i16, ptr %4, !tbaa !2
  %36 = icmp sgt i16 %34, %35
  br i1 %36, label %b8, label %b9

b8:
  %37 = load i16, ptr %1, !tbaa !2
  store i16 %37, ptr %4, !tbaa !2
  br label %b10

b9:
  br label %b10

b10:
  br label %b5
}

define internal cc 1005 void @"report[$state1]"(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = alloca [16 x i8]
  %5 = alloca ptr
  call cc 1005 addrspace(1) void @N$PBEG()
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 8, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %6 = call cc 1005 addrspace(1) ptr @N$PEND()
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
  %27 = call cc 1005 addrspace(1) i32 @$state1.next(ptr addrspace(1) %26)
  %28 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %27, ptr addrspace(1) %28, !tbaa !2
  %29 = load i8, ptr %3, !tbaa !2, !range !9
  %30 = icmp eq i8 %29, 0
  br i1 %30, label %b7, label %b6

b4:
  %31 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %31)
  call cc 1005 addrspace(1) void @N$PN()
  %32 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %32)
  ret void

b5:
  br label %b2

b6:
  br label %b4

b7:
  %33 = getelementptr inbounds i8, ptr %3, i16 2
  %34 = load i16, ptr %33, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %3, i16 2
  %36 = load i16, ptr %35, !tbaa !2
  store i16 %36, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PBEG()
  %37 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %37)
  %38 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %38)
  %39 = call cc 1005 addrspace(1) ptr @N$PEND()
  %40 = load ptr, ptr %5, !tbaa !2
  %41 = call cc 1005 addrspace(1) ptr @N$TAPP(ptr %40, ptr %39)
  store ptr %41, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %39)
  br label %b5
}

define internal cc 1005 i32 @$state1.next(ptr addrspace(1) nonnull dereferenceable(16) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 2
  br label %b2

b2:
  br label %b3

b3:
  %5 = load i16, ptr addrspace(1) %0
  %6 = icmp eq i16 %5, 0
  br i1 %6, label %b7, label %b6

b6:
  %7 = icmp eq i16 %5, 2
  br i1 %7, label %b9, label %b8

b7:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 12
  store i16 0, ptr addrspace(1) %8
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %9 = icmp eq i16 %5, 3
  br i1 %9, label %b14, label %b13

b9:
  %10 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %11 = load i16, ptr addrspace(1) %10
  %12 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %13 = load i16, ptr addrspace(1) %12
  %14 = add i16 %11, %13
  %15 = load i16, ptr addrspace(1) %4
  %16 = icmp ule i16 %14, %15
  br i1 %16, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %0
  br label %b2

b11:
  store i16 4, ptr addrspace(1) %0
  br label %b2

b13:
  %17 = icmp eq i16 %5, 4
  br i1 %17, label %b22, label %b21

b14:
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 14
  store i16 0, ptr addrspace(1) %18
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %20 = load i16, ptr addrspace(1) %19
  %21 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %22 = load i16, ptr addrspace(1) %21
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %24 = load i16, ptr addrspace(1) %23
  %25 = add i16 %22, %24
  store i16 %20, ptr %2, !tbaa !2
  store i16 %25, ptr %1, !tbaa !2
  br label %b15

b15:
  %26 = load i16, ptr %2, !tbaa !2
  %27 = load i16, ptr %1, !tbaa !2
  %28 = icmp ult i16 %26, %27
  br i1 %28, label %b16, label %b18

b16:
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %30 = load i16, ptr addrspace(1) %29
  %31 = load i16, ptr %2, !tbaa !2
  %32 = load i16, ptr addrspace(1) %4, !range !5
  %33 = icmp ult i16 %31, %32
  br i1 %33, label %b19, label %b20

b17:
  %34 = load i16, ptr %2, !tbaa !2
  %35 = add nuw i16 %34, 1
  store i16 %35, ptr %2, !tbaa !2
  br label %b15

b18:
  store i16 5, ptr addrspace(1) %0
  %36 = getelementptr i8, ptr addrspace(1) %0, i16 14
  %37 = load i16, ptr addrspace(1) %36
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %39 = load i16, ptr addrspace(1) %38
  %40 = sdiv i16 %37, %39
  %41 = srem i16 %37, %39
  %42 = icmp ne i16 %41, 0
  %43 = zext i1 %42 to i8
  %44 = xor i16 %41, %39
  %45 = icmp slt i16 %44, 0
  %46 = zext i1 %45 to i8
  %47 = and i8 %43, %46
  %48 = zext i8 %47 to i16
  %49 = and i16 %48, 1
  %50 = sub i16 %40, %49
  store i16 0, ptr %3, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %50, ptr %51, !tbaa !2
  %52 = addrspacecast ptr %3 to ptr addrspace(1)
  %53 = load i32, ptr addrspace(1) %52, !tbaa !2
  ret i32 %53

b19:
  %54 = getelementptr i8, ptr addrspace(1) %4, i16 4
  %55 = load ptr addrspace(1), ptr addrspace(1) %54
  %56 = mul i16 %31, 2
  %57 = getelementptr inbounds i8, ptr addrspace(1) %55, i16 %56
  %58 = load i16, ptr addrspace(1) %57
  %59 = add i16 %30, %58
  %60 = getelementptr i8, ptr addrspace(1) %0, i16 14
  store i16 %59, ptr addrspace(1) %60
  br label %b17

b20:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b21:
  %61 = icmp eq i16 %5, 5
  br i1 %61, label %b24, label %b23

b22:
  store i16 1, ptr addrspace(1) %0
  br label %b2

b23:
  store i16 1, ptr addrspace(1) %0
  store i32 1, ptr %3, !tbaa !2
  %62 = addrspacecast ptr %3 to ptr addrspace(1)
  %63 = load i32, ptr addrspace(1) %62, !tbaa !2
  ret i32 %63

b24:
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 12
  %65 = load i16, ptr addrspace(1) %64
  %66 = add i16 %65, 1
  %67 = getelementptr i8, ptr addrspace(1) %0, i16 12
  store i16 %66, ptr addrspace(1) %67
  store i16 2, ptr addrspace(1) %0
  br label %b2
}

define internal cc 1005 void @"report[$state0]"(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i16
  %3 = alloca [4 x i8]
  %4 = alloca [12 x i8]
  %5 = alloca ptr
  call cc 1005 addrspace(1) void @N$PBEG()
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 8, i8 zeroext 10, i8 zeroext 32, i8 zeroext 1)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %0)
  %6 = call cc 1005 addrspace(1) ptr @N$PEND()
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
  %21 = call cc 1005 addrspace(1) i32 @$state0.next(ptr addrspace(1) %20)
  %22 = addrspacecast ptr %3 to ptr addrspace(1)
  store i32 %21, ptr addrspace(1) %22, !tbaa !2
  %23 = load i8, ptr %3, !tbaa !2, !range !10
  %24 = icmp eq i8 %23, 0
  br i1 %24, label %b7, label %b6

b4:
  %25 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$PS(ptr %25)
  call cc 1005 addrspace(1) void @N$PN()
  %26 = load ptr, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %26)
  ret void

b5:
  br label %b2

b6:
  br label %b4

b7:
  %27 = getelementptr inbounds i8, ptr %3, i16 2
  %28 = load i16, ptr %27, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %3, i16 2
  %30 = load i16, ptr %29, !tbaa !2
  store i16 %30, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PBEG()
  %31 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %31)
  %32 = load i16, ptr %2, !tbaa !2
  call cc 1005 addrspace(1) void @N$PFLD(i8 zeroext 3, i8 zeroext 10, i8 zeroext 32, i8 zeroext 0)
  call cc 1005 addrspace(1) void @N$PI2(i16 %32)
  %33 = call cc 1005 addrspace(1) ptr @N$PEND()
  %34 = load ptr, ptr %5, !tbaa !2
  %35 = call cc 1005 addrspace(1) ptr @N$TAPP(ptr %34, ptr %33)
  store ptr %35, ptr %5, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %33)
  br label %b5
}

define internal cc 1005 i32 @$state0.next(ptr addrspace(1) nonnull dereferenceable(12) noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca [4 x i8]
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  br label %b2

b2:
  br label %b3

b3:
  %3 = load i16, ptr addrspace(1) %0
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b7, label %b6

b6:
  %5 = icmp eq i16 %3, 2
  br i1 %5, label %b9, label %b8

b7:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store i16 0, ptr addrspace(1) %6
  store i16 2, ptr addrspace(1) %0
  br label %b2

b8:
  %7 = icmp eq i16 %3, 3
  br i1 %7, label %b14, label %b13

b9:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %9 = load i16, ptr addrspace(1) %8
  %10 = load i16, ptr addrspace(1) %2
  %11 = icmp ult i16 %9, %10
  br i1 %11, label %b10, label %b11

b10:
  store i16 3, ptr addrspace(1) %0
  br label %b2

b11:
  store i16 5, ptr addrspace(1) %0
  br label %b2

b13:
  %12 = icmp eq i16 %3, 4
  br i1 %12, label %b18, label %b17

b14:
  store i16 6, ptr addrspace(1) %0
  %13 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %14 = load i16, ptr addrspace(1) %13
  %15 = load i16, ptr addrspace(1) %2, !range !6
  %16 = icmp ult i16 %14, %15
  br i1 %16, label %b15, label %b16

b15:
  %17 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %18 = load ptr addrspace(1), ptr addrspace(1) %17
  %19 = mul i16 %14, 2
  %20 = getelementptr inbounds i8, ptr addrspace(1) %18, i16 %19
  %21 = load i16, ptr addrspace(1) %20
  store i16 0, ptr %1, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %21, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %1 to ptr addrspace(1)
  %24 = load i32, ptr addrspace(1) %23, !tbaa !2
  ret i32 %24

b16:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b17:
  %25 = icmp eq i16 %3, 5
  br i1 %25, label %b20, label %b19

b18:
  %26 = getelementptr i8, ptr addrspace(1) %0, i16 10
  %27 = load i16, ptr addrspace(1) %26
  %28 = add i16 %27, 1
  %29 = getelementptr i8, ptr addrspace(1) %0, i16 10
  store i16 %28, ptr addrspace(1) %29
  store i16 2, ptr addrspace(1) %0
  br label %b2

b19:
  %30 = icmp eq i16 %3, 6
  br i1 %30, label %b22, label %b21

b20:
  store i16 1, ptr addrspace(1) %0
  br label %b2

b21:
  store i16 1, ptr addrspace(1) %0
  store i32 1, ptr %1, !tbaa !2
  %31 = addrspacecast ptr %1 to ptr addrspace(1)
  %32 = load i32, ptr addrspace(1) %31, !tbaa !2
  ret i32 %32

b22:
  store i16 4, ptr addrspace(1) %0
  br label %b2
}

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PFLD(i8, i8, i8, i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$TAPP(ptr, ptr) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 -32768}
!6 = !{i16 0, i16 -32768}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
