target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [20 x i8] c"\08\00\0D\00\0D\00Ada, pilot, 7\00"
@$str3 = internal constant [26 x i8] c"\08\00\13\00\13\00Grace,  admiral , 9\00"
@$str4 = internal constant [19 x i8] c"\08\00\0C\00\0C\00Linus ,  , 3\00"
@$str5 = internal constant [18 x i8] c"\08\00\0B\00\0B\00| (no role)\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00| \00"
@$str7 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 at level \00"
@$str8 = internal constant [12 x i8] c"\08\00\05\00\05\00Grace\00"
@$str9 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"

define internal void @trimmed(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca i8
  %4 = alloca i8
  %5 = alloca i16
  %6 = alloca i16
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  store i8 0, ptr %3
  store i8 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %6, !tbaa !2
  %7 = load i16, ptr addrspace(1) %1
  store i16 %7, ptr %5, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %6, !tbaa !2
  %9 = load i16, ptr %5, !tbaa !2
  %10 = icmp ult i16 %8, %9
  %11 = sext i1 %10 to i8
  store i8 %11, ptr %4, !tbaa !2
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b5, label %b6

b3:
  %13 = load i16, ptr %6, !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr %6, !tbaa !2
  br label %b2

b4:
  br label %b9

b5:
  %15 = load i16, ptr %6, !tbaa !2
  %16 = load i16, ptr addrspace(1) %1
  %17 = icmp ult i16 %15, %16
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b7, label %b8

b6:
  %20 = load i8, ptr %4, !tbaa !2
  %21 = icmp ne i8 %20, 0
  br i1 %21, label %b3, label %b4

b7:
  %22 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %23 = load ptr addrspace(1), ptr addrspace(1) %22
  %24 = getelementptr i8, ptr addrspace(1) %23, i16 %15
  %25 = load i8, ptr addrspace(1) %24
  %26 = icmp eq i8 %25, 32
  %27 = sext i1 %26 to i8
  store i8 %27, ptr %4, !tbaa !2
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  %28 = load i16, ptr %5, !tbaa !2
  %29 = load i16, ptr %6, !tbaa !2
  %30 = icmp ugt i16 %28, %29
  %31 = sext i1 %30 to i8
  store i8 %31, ptr %3, !tbaa !2
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b12, label %b13

b10:
  %33 = load i16, ptr %5, !tbaa !2
  %34 = sub i16 %33, 1
  store i16 %34, ptr %5, !tbaa !2
  br label %b9

b11:
  %35 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %36 = load ptr addrspace(1), ptr addrspace(1) %35
  %37 = load i16, ptr addrspace(1) %1
  %38 = load i16, ptr %6, !tbaa !2
  %39 = load i16, ptr %5, !tbaa !2
  %40 = icmp ule i16 %39, %37
  %41 = sext i1 %40 to i8
  %42 = icmp ne i8 %41, 0
  br i1 %42, label %b16, label %b17

b12:
  %43 = load i16, ptr %5, !tbaa !2
  %44 = sub i16 %43, 1
  %45 = load i16, ptr addrspace(1) %1
  %46 = icmp ult i16 %44, %45
  %47 = sext i1 %46 to i8
  %48 = icmp ne i8 %47, 0
  br i1 %48, label %b14, label %b15

b13:
  %49 = load i8, ptr %3, !tbaa !2
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b10, label %b11

b14:
  %51 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %52 = load ptr addrspace(1), ptr addrspace(1) %51
  %53 = getelementptr i8, ptr addrspace(1) %52, i16 %44
  %54 = load i8, ptr addrspace(1) %53
  %55 = icmp eq i8 %54, 32
  %56 = sext i1 %55 to i8
  store i8 %56, ptr %3, !tbaa !2
  br label %b13

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %57 = icmp ule i16 %38, %39
  %58 = sext i1 %57 to i8
  %59 = icmp ne i8 %58, 0
  br i1 %59, label %b18, label %b19

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  %60 = getelementptr i8, ptr addrspace(1) %36, i16 %38
  %61 = sub i16 %39, %38
  store i16 %61, ptr %2, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %61, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %60, ptr %63, !tbaa !2
  %64 = addrspacecast ptr %2 to ptr addrspace(1)
  %65 = load i16, ptr addrspace(1) %64, !tbaa !2
  store i16 %65, ptr addrspace(1) %0
  %66 = getelementptr i8, ptr addrspace(1) %64, i16 2
  %67 = load i16, ptr addrspace(1) %66, !tbaa !2
  %68 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %67, ptr addrspace(1) %68
  %69 = getelementptr i8, ptr addrspace(1) %64, i16 4
  %70 = load ptr addrspace(1), ptr addrspace(1) %69, !tbaa !2
  %71 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %70, ptr addrspace(1) %71
  ret void

b19:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @field(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  store i16 0, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %11, !tbaa !2
  store i16 0, ptr %10, !tbaa !2
  %12 = load i16, ptr addrspace(1) %1
  store i16 0, ptr %9, !tbaa !2
  store i16 %12, ptr %8, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %9, !tbaa !2
  %14 = load i16, ptr %8, !tbaa !2
  %15 = icmp ult i16 %13, %14
  %16 = sext i1 %15 to i8
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %b3, label %b5

b3:
  %18 = load i16, ptr %9, !tbaa !2
  %19 = load i16, ptr addrspace(1) %1
  %20 = icmp ult i16 %18, %19
  %21 = sext i1 %20 to i8
  %22 = icmp ne i8 %21, 0
  br i1 %22, label %b6, label %b7

b4:
  %23 = load i16, ptr %9, !tbaa !2
  %24 = add i16 %23, 1
  store i16 %24, ptr %9, !tbaa !2
  br label %b2

b5:
  %25 = load i16, ptr %11, !tbaa !2
  %26 = icmp eq i16 %25, %2
  %27 = sext i1 %26 to i8
  %28 = icmp ne i8 %27, 0
  br i1 %28, label %b18, label %b19

b6:
  %29 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %30 = load ptr addrspace(1), ptr addrspace(1) %29
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 %18
  %32 = load i8, ptr addrspace(1) %31
  %33 = icmp eq i8 %32, 44
  %34 = sext i1 %33 to i8
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %36 = load i16, ptr %11, !tbaa !2
  %37 = icmp eq i16 %36, %2
  %38 = sext i1 %37 to i8
  %39 = icmp ne i8 %38, 0
  br i1 %39, label %b11, label %b12

b9:
  br label %b10

b10:
  br label %b4

b11:
  %40 = addrspacecast ptr %7 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %42 = load ptr addrspace(1), ptr addrspace(1) %41
  %43 = load i16, ptr addrspace(1) %1
  %44 = load i16, ptr %10, !tbaa !2
  %45 = load i16, ptr %9, !tbaa !2
  %46 = icmp ule i16 %45, %43
  %47 = sext i1 %46 to i8
  %48 = icmp ne i8 %47, 0
  br i1 %48, label %b14, label %b15

b12:
  br label %b13

b13:
  %49 = load i16, ptr %11, !tbaa !2
  %50 = add i16 %49, 1
  store i16 %50, ptr %11, !tbaa !2
  %51 = load i16, ptr %9, !tbaa !2
  %52 = add i16 %51, 1
  store i16 %52, ptr %10, !tbaa !2
  br label %b10

b14:
  %53 = icmp ule i16 %44, %45
  %54 = sext i1 %53 to i8
  %55 = icmp ne i8 %54, 0
  br i1 %55, label %b16, label %b17

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %56 = getelementptr i8, ptr addrspace(1) %42, i16 %44
  %57 = sub i16 %45, %44
  store i16 %57, ptr %6, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %57, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %56, ptr %59, !tbaa !2
  %60 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @trimmed(ptr addrspace(1) %40, ptr addrspace(1) %60)
  %61 = load i16, ptr addrspace(1) %40, !tbaa !2
  store i16 %61, ptr addrspace(1) %0
  %62 = getelementptr i8, ptr addrspace(1) %40, i16 2
  %63 = load i16, ptr addrspace(1) %62, !tbaa !2
  %64 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %63, ptr addrspace(1) %64
  %65 = getelementptr i8, ptr addrspace(1) %40, i16 4
  %66 = load ptr addrspace(1), ptr addrspace(1) %65, !tbaa !2
  %67 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %66, ptr addrspace(1) %67
  ret void

b17:
  call addrspace(1) void @N$EBND()
  unreachable

b18:
  %68 = addrspacecast ptr %5 to ptr addrspace(1)
  %69 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %70 = load ptr addrspace(1), ptr addrspace(1) %69
  %71 = load i16, ptr addrspace(1) %1
  %72 = load i16, ptr %10, !tbaa !2
  %73 = load i16, ptr addrspace(1) %1
  %74 = icmp ule i16 %73, %71
  %75 = sext i1 %74 to i8
  %76 = icmp ne i8 %75, 0
  br i1 %76, label %b21, label %b22

b19:
  br label %b20

b20:
  %77 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %78 = load ptr addrspace(1), ptr addrspace(1) %77
  %79 = load i16, ptr addrspace(1) %1
  %80 = icmp ule i16 0, %79
  %81 = sext i1 %80 to i8
  %82 = icmp ne i8 %81, 0
  br i1 %82, label %b25, label %b26

b21:
  %83 = icmp ule i16 %72, %73
  %84 = sext i1 %83 to i8
  %85 = icmp ne i8 %84, 0
  br i1 %85, label %b23, label %b24

b22:
  call addrspace(1) void @N$EBND()
  unreachable

b23:
  %86 = getelementptr i8, ptr addrspace(1) %70, i16 %72
  %87 = sub i16 %73, %72
  store i16 %87, ptr %4, !tbaa !2
  %88 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %87, ptr %88, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %86, ptr %89, !tbaa !2
  %90 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @trimmed(ptr addrspace(1) %68, ptr addrspace(1) %90)
  %91 = load i16, ptr addrspace(1) %68, !tbaa !2
  store i16 %91, ptr addrspace(1) %0
  %92 = getelementptr i8, ptr addrspace(1) %68, i16 2
  %93 = load i16, ptr addrspace(1) %92, !tbaa !2
  %94 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %93, ptr addrspace(1) %94
  %95 = getelementptr i8, ptr addrspace(1) %68, i16 4
  %96 = load ptr addrspace(1), ptr addrspace(1) %95, !tbaa !2
  %97 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %96, ptr addrspace(1) %97
  ret void

b24:
  call addrspace(1) void @N$EBND()
  unreachable

b25:
  store i16 0, ptr %3, !tbaa !2
  %98 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 0, ptr %98, !tbaa !2
  %99 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %78, ptr %99, !tbaa !2
  %100 = addrspacecast ptr %3 to ptr addrspace(1)
  %101 = load i16, ptr addrspace(1) %100, !tbaa !2
  store i16 %101, ptr addrspace(1) %0
  %102 = getelementptr i8, ptr addrspace(1) %100, i16 2
  %103 = load i16, ptr addrspace(1) %102, !tbaa !2
  %104 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %103, ptr addrspace(1) %104
  %105 = getelementptr i8, ptr addrspace(1) %100, i16 4
  %106 = load ptr addrspace(1), ptr addrspace(1) %105, !tbaa !2
  %107 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %106, ptr addrspace(1) %107
  ret void

b26:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca ptr
  %2 = alloca i8
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca i16
  %13 = alloca ptr
  store i16 0, ptr %0
  store ptr null, ptr %1
  store i8 0, ptr %2
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  store i16 0, ptr %12
  store ptr null, ptr %13
  %14 = getelementptr i8, ptr @$str1, i16 6
  %15 = call addrspace(1) ptr @N$BGRW(ptr %14, i16 3, i16 2)
  %16 = getelementptr i8, ptr %15, i16 0
  %17 = getelementptr i8, ptr @$str2, i16 6
  store ptr %17, ptr %16
  %18 = getelementptr i8, ptr %15, i16 2
  %19 = getelementptr i8, ptr @$str3, i16 6
  store ptr %19, ptr %18
  %20 = getelementptr i8, ptr %15, i16 4
  %21 = getelementptr i8, ptr @$str4, i16 6
  store ptr %21, ptr %20
  store ptr %15, ptr %13, !tbaa !2
  %22 = load ptr, ptr %13, !tbaa !2
  %23 = getelementptr i8, ptr %22, i16 -4
  %24 = load i16, ptr %23
  store i16 0, ptr %12, !tbaa !2
  br label %b2

b2:
  %25 = load i16, ptr %12, !tbaa !2
  %26 = icmp ult i16 %25, %24
  %27 = sext i1 %26 to i8
  %28 = icmp ne i8 %27, 0
  br i1 %28, label %b3, label %b5

b3:
  %29 = mul i16 %25, 2
  %30 = getelementptr i8, ptr %22, i16 %29
  %31 = addrspacecast ptr %11 to ptr addrspace(1)
  %32 = load ptr, ptr %30
  %33 = getelementptr i8, ptr %32, i16 -4
  %34 = load i16, ptr %33
  %35 = addrspacecast ptr %32 to ptr addrspace(1)
  store i16 %34, ptr %10, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %34, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %35, ptr %37, !tbaa !2
  %38 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @field(ptr addrspace(1) %31, ptr addrspace(1) %38, i16 0)
  %39 = addrspacecast ptr %9 to ptr addrspace(1)
  %40 = load ptr, ptr %30
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 %42, ptr %8, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %42, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %43, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %8 to ptr addrspace(1)
  call addrspace(1) void @field(ptr addrspace(1) %39, ptr addrspace(1) %46, i16 1)
  %47 = load i16, ptr addrspace(1) %39
  %48 = icmp eq i16 %47, 0
  %49 = sext i1 %48 to i8
  %50 = icmp ne i8 %49, 0
  br i1 %50, label %b6, label %b7

b4:
  %51 = load i16, ptr %12, !tbaa !2
  %52 = add i16 %51, 1
  store i16 %52, ptr %12, !tbaa !2
  br label %b2

b5:
  %53 = addrspacecast ptr %5 to ptr addrspace(1)
  %54 = load ptr, ptr %13, !tbaa !2
  %55 = getelementptr i8, ptr %54, i16 -4
  %56 = load i16, ptr %55
  %57 = icmp ult i16 1, %56
  %58 = sext i1 %57 to i8
  %59 = icmp ne i8 %58, 0
  br i1 %59, label %b9, label %b10

b6:
  call addrspace(1) void @N$PFLD(i8 6, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %31)
  %60 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %60)
  call addrspace(1) void @N$PN()
  br label %b8

b7:
  %61 = addrspacecast ptr %7 to ptr addrspace(1)
  %62 = load ptr, ptr %30
  %63 = getelementptr i8, ptr %62, i16 -4
  %64 = load i16, ptr %63
  %65 = addrspacecast ptr %62 to ptr addrspace(1)
  store i16 %64, ptr %6, !tbaa !2
  %66 = getelementptr inbounds i8, ptr %6, i16 2
  store i16 %64, ptr %66, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %6, i16 4
  store ptr addrspace(1) %65, ptr %67, !tbaa !2
  %68 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @field(ptr addrspace(1) %61, ptr addrspace(1) %68, i16 2)
  call addrspace(1) void @N$PFLD(i8 6, i8 10, i8 32, i8 1)
  call addrspace(1) void @N$PV(ptr addrspace(1) %31)
  %69 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %69)
  call addrspace(1) void @N$PV(ptr addrspace(1) %39)
  %70 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %70)
  call addrspace(1) void @N$PV(ptr addrspace(1) %61)
  call addrspace(1) void @N$PN()
  br label %b8

b8:
  br label %b4

b9:
  %71 = getelementptr i8, ptr %54, i16 2
  %72 = load ptr, ptr %71
  %73 = getelementptr i8, ptr %72, i16 -4
  %74 = load i16, ptr %73
  %75 = addrspacecast ptr %72 to ptr addrspace(1)
  store i16 %74, ptr %4, !tbaa !2
  %76 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %74, ptr %76, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %75, ptr %77, !tbaa !2
  %78 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @field(ptr addrspace(1) %53, ptr addrspace(1) %78, i16 0)
  %79 = getelementptr i8, ptr @$str8, i16 6
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  %82 = addrspacecast ptr %79 to ptr addrspace(1)
  store i16 %81, ptr %3, !tbaa !2
  %83 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %81, ptr %83, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %82, ptr %84, !tbaa !2
  %85 = addrspacecast ptr %3 to ptr addrspace(1)
  %86 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %53, ptr addrspace(1) %85)
  %87 = icmp eq i8 %86, 0
  %88 = sext i1 %87 to i8
  store i8 %88, ptr %2, !tbaa !2
  %89 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %53)
  store ptr %89, ptr %1, !tbaa !2
  %90 = load i8, ptr %2, !tbaa !2
  call addrspace(1) void @N$PB(i8 %90)
  %91 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %91)
  %92 = load ptr, ptr %1, !tbaa !2
  call addrspace(1) void @N$PS(ptr %92)
  call addrspace(1) void @N$PN()
  %93 = load ptr, ptr %1, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %93)
  %94 = load ptr, ptr %13, !tbaa !2
  %95 = icmp ne ptr %94, null
  %96 = sext i1 %95 to i8
  %97 = icmp ne i8 %96, 0
  br i1 %97, label %b12, label %b11

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  call addrspace(1) void @N$BDRP(ptr %94)
  ret i16 0

b12:
  %98 = getelementptr i8, ptr %94, i16 -4
  %99 = load i16, ptr %98
  store i16 0, ptr %0, !tbaa !2
  br label %b13

b13:
  %100 = load i16, ptr %0, !tbaa !2
  %101 = icmp ult i16 %100, %99
  %102 = sext i1 %101 to i8
  %103 = icmp ne i8 %102, 0
  br i1 %103, label %b15, label %b14

b14:
  br label %b11

b15:
  %104 = mul i16 %100, 2
  %105 = getelementptr i8, ptr %94, i16 %104
  %106 = load ptr, ptr %105
  call addrspace(1) void @N$BDRP(ptr %106)
  %107 = add i16 %100, 1
  store i16 %107, ptr %0, !tbaa !2
  br label %b13
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$EBND() addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PFLD(i8, i8, i8, i8) addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
