target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [13 x i8] c"\08\00\06\00\06\00nobody\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00ada\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00bob\00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00leader \00"
@$str5 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00cy\00"
@$str7 = internal constant [9 x i8] c"\08\00\02\00\02\00di\00"
@$str8 = internal constant [9 x i8] c"\08\00\02\00\02\00ed\00"
@$str9 = internal constant [12 x i8] c"\08\00\05\00\05\00best \00"
@$str10 = internal constant [13 x i8] c"\08\00\06\00\06\00 with \00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00no team\00"
@$str12 = internal constant [13 x i8] c"\08\00\06\00\06\00first \00"
@$str13 = internal constant [27 x i8] c"\08\00\14\00\14\00move north then east\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00[\00"
@$str15 = internal constant [8 x i8] c"\08\00\01\00\01\00]\00"

define internal ptr addrspace(1) @leader(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca ptr addrspace(1)
  store ptr addrspace(1) null, ptr %2
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = icmp sge i16 %4, %6
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  store ptr addrspace(1) %0, ptr %2, !tbaa !2
  br label %b4

b3:
  store ptr addrspace(1) %1, ptr %2, !tbaa !2
  br label %b4

b4:
  %10 = load ptr addrspace(1), ptr %2, !tbaa !2
  ret ptr addrspace(1) %10
}

define internal void @best(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  store i16 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  %5 = load i16, ptr addrspace(1) %1
  %6 = icmp eq i16 %5, 0
  %7 = sext i1 %6 to i8
  %8 = icmp ne i8 %7, 0
  br i1 %8, label %b2, label %b3

b2:
  store i8 1, ptr addrspace(1) %0
  ret void

b3:
  br label %b4

b4:
  store i16 0, ptr %4, !tbaa !2
  %9 = load i16, ptr addrspace(1) %1
  store i16 1, ptr %3, !tbaa !2
  store i16 %9, ptr %2, !tbaa !2
  br label %b5

b5:
  %10 = load i16, ptr %3, !tbaa !2
  %11 = load i16, ptr %2, !tbaa !2
  %12 = icmp ult i16 %10, %11
  %13 = sext i1 %12 to i8
  %14 = icmp ne i8 %13, 0
  br i1 %14, label %b6, label %b8

b6:
  %15 = load i16, ptr %3, !tbaa !2
  %16 = load i16, ptr addrspace(1) %1
  %17 = icmp ult i16 %15, %16
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b9, label %b10

b7:
  %20 = load i16, ptr %3, !tbaa !2
  %21 = add i16 %20, 1
  store i16 %21, ptr %3, !tbaa !2
  br label %b5

b8:
  %22 = load i16, ptr %4, !tbaa !2
  %23 = load i16, ptr addrspace(1) %1
  %24 = icmp ult i16 %22, %23
  %25 = sext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b16, label %b17

b9:
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %28 = load ptr addrspace(1), ptr addrspace(1) %27
  %29 = mul i16 %15, 4
  %30 = getelementptr i8, ptr addrspace(1) %28, i16 %29
  %31 = getelementptr i8, ptr addrspace(1) %30, i16 2
  %32 = load i16, ptr addrspace(1) %31
  %33 = load i16, ptr %4, !tbaa !2
  %34 = load i16, ptr addrspace(1) %1
  %35 = icmp ult i16 %33, %34
  %36 = sext i1 %35 to i8
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b11, label %b12

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  %38 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %39 = load ptr addrspace(1), ptr addrspace(1) %38
  %40 = mul i16 %33, 4
  %41 = getelementptr i8, ptr addrspace(1) %39, i16 %40
  %42 = getelementptr i8, ptr addrspace(1) %41, i16 2
  %43 = load i16, ptr addrspace(1) %42
  %44 = icmp sgt i16 %32, %43
  %45 = sext i1 %44 to i8
  %46 = icmp ne i8 %45, 0
  br i1 %46, label %b13, label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %47 = load i16, ptr %3, !tbaa !2
  store i16 %47, ptr %4, !tbaa !2
  br label %b15

b14:
  br label %b15

b15:
  br label %b7

b16:
  %48 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %49 = load ptr addrspace(1), ptr addrspace(1) %48
  %50 = mul i16 %22, 4
  %51 = getelementptr i8, ptr addrspace(1) %49, i16 %50
  store i8 0, ptr addrspace(1) %0
  %52 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %51, ptr addrspace(1) %52
  ret void

b17:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @first_name(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = load i16, ptr addrspace(1) %1
  %7 = icmp sge i16 %6, 1
  %8 = sext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b4, label %b3

b3:
  %10 = getelementptr i8, ptr @$str1, i16 6
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 %12, ptr %2, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %12, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %2 to ptr addrspace(1)
  %17 = load i16, ptr addrspace(1) %16, !tbaa !2
  store i16 %17, ptr addrspace(1) %0
  %18 = getelementptr i8, ptr addrspace(1) %16, i16 2
  %19 = load i16, ptr addrspace(1) %18, !tbaa !2
  %20 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %19, ptr addrspace(1) %20
  %21 = getelementptr i8, ptr addrspace(1) %16, i16 4
  %22 = load ptr addrspace(1), ptr addrspace(1) %21, !tbaa !2
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %22, ptr addrspace(1) %23
  ret void

b4:
  %24 = getelementptr i8, ptr addrspace(1) %5, i16 0
  %25 = getelementptr i8, ptr addrspace(1) %5, i16 0
  %26 = load ptr, ptr addrspace(1) %25
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 %28, ptr %3, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %28, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %3 to ptr addrspace(1)
  %33 = load i16, ptr addrspace(1) %32, !tbaa !2
  store i16 %33, ptr addrspace(1) %0
  %34 = getelementptr i8, ptr addrspace(1) %32, i16 2
  %35 = load i16, ptr addrspace(1) %34, !tbaa !2
  %36 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %35, ptr addrspace(1) %36
  %37 = getelementptr i8, ptr addrspace(1) %32, i16 4
  %38 = load ptr addrspace(1), ptr addrspace(1) %37, !tbaa !2
  %39 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %38, ptr addrspace(1) %39
  ret void
}

define internal void @word(ptr addrspace(1) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1, i16 %2) addrspace(1) {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca i8
  %5 = alloca i16
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  store i8 0, ptr %4
  store i16 0, ptr %5
  store i16 %2, ptr %5, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %5, !tbaa !2
  %7 = load i16, ptr addrspace(1) %1
  %8 = icmp ult i16 %6, %7
  %9 = sext i1 %8 to i8
  store i8 %9, ptr %4, !tbaa !2
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %b5, label %b6

b3:
  %11 = load i16, ptr %5, !tbaa !2
  %12 = add i16 %11, 1
  store i16 %12, ptr %5, !tbaa !2
  br label %b2

b4:
  %13 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %14 = load ptr addrspace(1), ptr addrspace(1) %13
  %15 = load i16, ptr addrspace(1) %1
  %16 = load i16, ptr %5, !tbaa !2
  %17 = icmp ule i16 %16, %15
  %18 = sext i1 %17 to i8
  %19 = icmp ne i8 %18, 0
  br i1 %19, label %b9, label %b10

b5:
  %20 = load i16, ptr %5, !tbaa !2
  %21 = load i16, ptr addrspace(1) %1
  %22 = icmp ult i16 %20, %21
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b7, label %b8

b6:
  %25 = load i8, ptr %4, !tbaa !2
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b3, label %b4

b7:
  %27 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %28 = load ptr addrspace(1), ptr addrspace(1) %27
  %29 = getelementptr i8, ptr addrspace(1) %28, i16 %20
  %30 = load i8, ptr addrspace(1) %29
  %31 = icmp ne i8 %30, 32
  %32 = sext i1 %31 to i8
  store i8 %32, ptr %4, !tbaa !2
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  %33 = icmp ule i16 %2, %16
  %34 = sext i1 %33 to i8
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b11, label %b12

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  %36 = getelementptr i8, ptr addrspace(1) %14, i16 %2
  %37 = sub i16 %16, %2
  store i16 %37, ptr %3, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %37, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %36, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %3 to ptr addrspace(1)
  %41 = load i16, ptr addrspace(1) %40, !tbaa !2
  store i16 %41, ptr addrspace(1) %0
  %42 = getelementptr i8, ptr addrspace(1) %40, i16 2
  %43 = load i16, ptr addrspace(1) %42, !tbaa !2
  %44 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %43, ptr addrspace(1) %44
  %45 = getelementptr i8, ptr addrspace(1) %40, i16 4
  %46 = load ptr addrspace(1), ptr addrspace(1) %45, !tbaa !2
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %46, ptr addrspace(1) %47
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca ptr
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [6 x i8]
  %12 = alloca ptr
  %13 = alloca [8 x i8]
  %14 = alloca [4 x i8]
  %15 = alloca [4 x i8]
  store i16 0, ptr %0
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  store i16 0, ptr %6
  store ptr null, ptr %7
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %9, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %10, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 6, i1 false)
  store ptr null, ptr %12
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %14, i8 0, i16 4, i1 false)
  call void @llvm.memset.p0.i16(ptr %15, i8 0, i16 4, i1 false)
  %16 = getelementptr i8, ptr @$str2, i16 6
  store ptr %16, ptr %15, !tbaa !2
  %17 = getelementptr inbounds i8, ptr %15, i16 2
  store i16 31, ptr %17, !tbaa !2
  %18 = getelementptr i8, ptr @$str3, i16 6
  store ptr %18, ptr %14, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 45, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %15 to ptr addrspace(1)
  %21 = addrspacecast ptr %14 to ptr addrspace(1)
  %22 = call addrspace(1) ptr addrspace(1) @leader(ptr addrspace(1) %20, ptr addrspace(1) %21)
  %23 = load ptr, ptr addrspace(1) %22
  %24 = getelementptr i8, ptr %23, i16 -4
  %25 = load i16, ptr %24
  %26 = addrspacecast ptr %23 to ptr addrspace(1)
  store i16 %25, ptr %13, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %25, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %13 to ptr addrspace(1)
  %30 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %30)
  call addrspace(1) void @N$PV(ptr addrspace(1) %29)
  call addrspace(1) void @N$PN()
  %31 = getelementptr i8, ptr @$str5, i16 6
  %32 = call addrspace(1) ptr @N$BGRW(ptr %31, i16 3, i16 4)
  %33 = getelementptr i8, ptr %32, i16 0
  %34 = getelementptr i8, ptr @$str6, i16 6
  store ptr %34, ptr %33
  %35 = getelementptr i8, ptr %33, i16 2
  store i16 12, ptr %35
  %36 = getelementptr i8, ptr %32, i16 4
  %37 = getelementptr i8, ptr @$str7, i16 6
  store ptr %37, ptr %36
  %38 = getelementptr i8, ptr %36, i16 2
  store i16 58, ptr %38
  %39 = getelementptr i8, ptr %32, i16 8
  %40 = getelementptr i8, ptr @$str8, i16 6
  store ptr %40, ptr %39
  %41 = getelementptr i8, ptr %39, i16 2
  store i16 40, ptr %41
  store ptr %32, ptr %12, !tbaa !2
  %42 = addrspacecast ptr %11 to ptr addrspace(1)
  %43 = load ptr, ptr %12, !tbaa !2
  %44 = getelementptr i8, ptr %43, i16 -4
  %45 = load i16, ptr %44
  %46 = addrspacecast ptr %43 to ptr addrspace(1)
  store i16 %45, ptr %10, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %45, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %46, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @best(ptr addrspace(1) %42, ptr addrspace(1) %49)
  %50 = load i8, ptr %11, !tbaa !2
  %51 = icmp eq i8 %50, 0
  %52 = sext i1 %51 to i8
  %53 = icmp ne i8 %52, 0
  br i1 %53, label %b4, label %b3

b2:
  %54 = addrspacecast ptr %9 to ptr addrspace(1)
  %55 = load ptr, ptr %12, !tbaa !2
  %56 = getelementptr i8, ptr %55, i16 -4
  %57 = load i16, ptr %56
  %58 = addrspacecast ptr %55 to ptr addrspace(1)
  store i16 %57, ptr %8, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %57, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %58, ptr %60, !tbaa !2
  %61 = addrspacecast ptr %8 to ptr addrspace(1)
  call addrspace(1) void @first_name(ptr addrspace(1) %54, ptr addrspace(1) %61)
  %62 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %62)
  call addrspace(1) void @N$PV(ptr addrspace(1) %54)
  call addrspace(1) void @N$PN()
  %63 = getelementptr i8, ptr @$str13, i16 6
  store ptr %63, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  %64 = addrspacecast ptr %5 to ptr addrspace(1)
  %65 = load ptr, ptr %7, !tbaa !2
  %66 = getelementptr i8, ptr %65, i16 -4
  %67 = load i16, ptr %66
  %68 = addrspacecast ptr %65 to ptr addrspace(1)
  store i16 %67, ptr %4, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %67, ptr %69, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %68, ptr %70, !tbaa !2
  %71 = addrspacecast ptr %4 to ptr addrspace(1)
  %72 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @word(ptr addrspace(1) %64, ptr addrspace(1) %71, i16 %72)
  %73 = addrspacecast ptr %3 to ptr addrspace(1)
  %74 = load i16, ptr addrspace(1) %64, !tbaa !2
  store i16 %74, ptr addrspace(1) %73, !tbaa !2
  %75 = getelementptr i8, ptr addrspace(1) %64, i16 2
  %76 = load i16, ptr addrspace(1) %75, !tbaa !2
  %77 = getelementptr i8, ptr addrspace(1) %73, i16 2
  store i16 %76, ptr addrspace(1) %77, !tbaa !2
  %78 = getelementptr i8, ptr addrspace(1) %64, i16 4
  %79 = load ptr addrspace(1), ptr addrspace(1) %78, !tbaa !2
  %80 = getelementptr i8, ptr addrspace(1) %73, i16 4
  store ptr addrspace(1) %79, ptr addrspace(1) %80, !tbaa !2
  br label %b6

b3:
  %81 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %81)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %82 = getelementptr inbounds i8, ptr %11, i16 2
  %83 = load ptr addrspace(1), ptr %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %11, i16 2
  %85 = load ptr addrspace(1), ptr %84, !tbaa !2
  %86 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %86)
  %87 = load ptr, ptr addrspace(1) %85
  call addrspace(1) void @N$PS(ptr %87)
  %88 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %88)
  %89 = getelementptr i8, ptr addrspace(1) %85, i16 2
  %90 = load i16, ptr addrspace(1) %89
  call addrspace(1) void @N$PI2(i16 %90)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %91 = load i16, ptr addrspace(1) %73
  %92 = icmp ugt i16 %91, 0
  %93 = sext i1 %92 to i8
  %94 = icmp ne i8 %93, 0
  br i1 %94, label %b7, label %b8

b7:
  %95 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %95)
  call addrspace(1) void @N$PV(ptr addrspace(1) %73)
  %96 = getelementptr i8, ptr @$str15, i16 6
  call addrspace(1) void @N$PS(ptr %96)
  call addrspace(1) void @N$PN()
  %97 = load i16, ptr %6, !tbaa !2
  %98 = load i16, ptr addrspace(1) %73
  %99 = add i16 %98, 1
  %100 = add i16 %97, %99
  store i16 %100, ptr %6, !tbaa !2
  %101 = load i16, ptr %6, !tbaa !2
  %102 = load ptr, ptr %7, !tbaa !2
  %103 = getelementptr i8, ptr %102, i16 -4
  %104 = load i16, ptr %103
  %105 = icmp uge i16 %101, %104
  %106 = sext i1 %105 to i8
  %107 = icmp ne i8 %106, 0
  br i1 %107, label %b9, label %b10

b8:
  %108 = load ptr, ptr %7, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %108)
  %109 = load ptr, ptr %12, !tbaa !2
  %110 = icmp ne ptr %109, null
  %111 = sext i1 %110 to i8
  %112 = icmp ne i8 %111, 0
  br i1 %112, label %b13, label %b12

b9:
  br label %b8

b10:
  br label %b11

b11:
  %113 = addrspacecast ptr %2 to ptr addrspace(1)
  %114 = load ptr, ptr %7, !tbaa !2
  %115 = getelementptr i8, ptr %114, i16 -4
  %116 = load i16, ptr %115
  %117 = addrspacecast ptr %114 to ptr addrspace(1)
  store i16 %116, ptr %1, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 %116, ptr %118, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %117, ptr %119, !tbaa !2
  %120 = addrspacecast ptr %1 to ptr addrspace(1)
  %121 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @word(ptr addrspace(1) %113, ptr addrspace(1) %120, i16 %121)
  %122 = load i16, ptr addrspace(1) %113, !tbaa !2
  store i16 %122, ptr addrspace(1) %73, !tbaa !2
  %123 = getelementptr i8, ptr addrspace(1) %113, i16 2
  %124 = load i16, ptr addrspace(1) %123, !tbaa !2
  %125 = getelementptr i8, ptr addrspace(1) %73, i16 2
  store i16 %124, ptr addrspace(1) %125, !tbaa !2
  %126 = getelementptr i8, ptr addrspace(1) %113, i16 4
  %127 = load ptr addrspace(1), ptr addrspace(1) %126, !tbaa !2
  %128 = getelementptr i8, ptr addrspace(1) %73, i16 4
  store ptr addrspace(1) %127, ptr addrspace(1) %128, !tbaa !2
  br label %b6

b12:
  call addrspace(1) void @N$BDRP(ptr %109)
  %129 = load ptr, ptr %14, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %129)
  %130 = load ptr, ptr %15, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %130)
  ret i16 0

b13:
  %131 = getelementptr i8, ptr %109, i16 -4
  %132 = load i16, ptr %131
  store i16 0, ptr %0, !tbaa !2
  br label %b14

b14:
  %133 = load i16, ptr %0, !tbaa !2
  %134 = icmp ult i16 %133, %132
  %135 = sext i1 %134 to i8
  %136 = icmp ne i8 %135, 0
  br i1 %136, label %b16, label %b15

b15:
  br label %b12

b16:
  %137 = mul i16 %133, 4
  %138 = getelementptr i8, ptr %109, i16 %137
  %139 = load ptr, ptr %138
  call addrspace(1) void @N$BDRP(ptr %139)
  %140 = add i16 %133, 1
  store i16 %140, ptr %0, !tbaa !2
  br label %b14
}

declare void @N$EBND() addrspace(1)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

declare void @N$PN() addrspace(1)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
