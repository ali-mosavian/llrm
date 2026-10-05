target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [11 x i8] c"\08\00\04\00\04\00bolt\00"
@$str3 = internal constant [11 x i8] c"\08\00\04\00\04\00gear\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00cog\00"
@$str5 = internal constant [10 x i8] c"\08\00\03\00\03\00pin\00"
@$str6 = internal constant [9 x i8] c"\08\00\02\00\02\00: \00"
@$str7 = internal constant [11 x i8] c"\08\00\04\00\04\00 at \00"
@$str8 = internal constant [13 x i8] c"\08\00\06\00\06\00no pin\00"
@$str9 = internal constant [21 x i8] c"\08\00\0E\00\0E\00cheapest gear \00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00no gear\00"
@$str11 = internal constant [24 x i8] c"\08\00\11\00\11\00 under 20, first \00"
@$str12 = internal constant [12 x i8] c"\08\00\05\00\05\00low: \00"
@$str13 = internal constant [9 x i8] c"\08\00\02\00\02\00 (\00"
@$str14 = internal constant [8 x i8] c"\08\00\01\00\01\00)\00"
@$str15 = internal constant [10 x i8] c"\08\00\03\00\03\00nut\00"
@$str16 = internal constant [13 x i8] c"\08\00\06\00\06\00 parts\00"

define internal void @Catalog.add(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i16 %2, i16 %3) addrspace(1) nearcode {
b1:
  %4 = load ptr, ptr addrspace(1) %0
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  %7 = call addrspace(1) ptr @N$BGRW(ptr %4, i16 1, i16 6)
  store ptr %7, ptr addrspace(1) %0
  %8 = mul i16 %6, 6
  %9 = getelementptr inbounds i8, ptr %7, i16 %8
  %10 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store ptr %10, ptr %9
  %11 = getelementptr i8, ptr %9, i16 2
  store i16 %2, ptr %11
  %12 = getelementptr i8, ptr %9, i16 4
  store i16 %3, ptr %12
  ret void
}

define internal void @north(ptr addrspace(1) %0) addrspace(1) nearcode {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [2 x i8]
  %6 = getelementptr i8, ptr @$str1, i16 6
  store ptr %6, ptr %5, !tbaa !2
  %7 = addrspacecast ptr %5 to ptr addrspace(1)
  %8 = getelementptr i8, ptr @$str2, i16 6
  %9 = getelementptr i8, ptr %8, i16 -4
  %10 = load i16, ptr %9
  %11 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 %10, ptr %4, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %10, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %11, ptr %13, !tbaa !2
  %14 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %7, ptr addrspace(1) %14, i16 5, i16 40)
  %15 = addrspacecast ptr %5 to ptr addrspace(1)
  %16 = getelementptr i8, ptr @$str3, i16 6
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 %18, ptr %3, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %18, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %19, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %15, ptr addrspace(1) %22, i16 30, i16 3)
  %23 = addrspacecast ptr %5 to ptr addrspace(1)
  %24 = getelementptr i8, ptr @$str4, i16 6
  %25 = getelementptr i8, ptr %24, i16 -4
  %26 = load i16, ptr %25
  %27 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 %26, ptr %2, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %26, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %27, ptr %29, !tbaa !2
  %30 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %23, ptr addrspace(1) %30, i16 12, i16 0)
  %31 = load ptr, ptr %5, !tbaa !2
  store ptr %31, ptr addrspace(1) %0
  store ptr null, ptr %5, !tbaa !2
  %32 = load ptr, ptr %5, !tbaa !2
  %33 = icmp ne ptr %32, null
  %34 = zext i1 %33 to i8
  %35 = icmp ne i8 %34, 0
  br i1 %35, label %b3, label %b2

b2:
  call addrspace(1) void @N$BDRP(ptr %32)
  ret void

b3:
  %36 = getelementptr i8, ptr %32, i16 -4
  %37 = load i16, ptr %36
  store i16 0, ptr %1, !tbaa !2
  br label %b4

b4:
  %38 = load i16, ptr %1, !tbaa !2
  %39 = icmp ult i16 %38, %37
  %40 = zext i1 %39 to i8
  %41 = icmp ne i8 %40, 0
  br i1 %41, label %b6, label %b5

b5:
  br label %b2

b6:
  %42 = mul i16 %38, 6
  %43 = getelementptr inbounds i8, ptr %32, i16 %42
  %44 = load ptr, ptr %43
  call addrspace(1) void @N$BDRP(ptr %44)
  %45 = add i16 %38, 1
  store i16 %45, ptr %1, !tbaa !2
  br label %b4
}

define internal void @south(ptr addrspace(1) %0) addrspace(1) nearcode {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [2 x i8]
  %5 = getelementptr i8, ptr @$str1, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = addrspacecast ptr %4 to ptr addrspace(1)
  %7 = getelementptr i8, ptr @$str3, i16 6
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 %9, ptr %3, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %9, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %10, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %6, ptr addrspace(1) %13, i16 28, i16 9)
  %14 = addrspacecast ptr %4 to ptr addrspace(1)
  %15 = getelementptr i8, ptr @$str5, i16 6
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %15 to ptr addrspace(1)
  store i16 %17, ptr %2, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %17, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %14, ptr addrspace(1) %21, i16 2, i16 100)
  %22 = load ptr, ptr %4, !tbaa !2
  store ptr %22, ptr addrspace(1) %0
  store ptr null, ptr %4, !tbaa !2
  %23 = load ptr, ptr %4, !tbaa !2
  %24 = icmp ne ptr %23, null
  %25 = zext i1 %24 to i8
  %26 = icmp ne i8 %25, 0
  br i1 %26, label %b3, label %b2

b2:
  call addrspace(1) void @N$BDRP(ptr %23)
  ret void

b3:
  %27 = getelementptr i8, ptr %23, i16 -4
  %28 = load i16, ptr %27
  store i16 0, ptr %1, !tbaa !2
  br label %b4

b4:
  %29 = load i16, ptr %1, !tbaa !2
  %30 = icmp ult i16 %29, %28
  %31 = zext i1 %30 to i8
  %32 = icmp ne i8 %31, 0
  br i1 %32, label %b6, label %b5

b5:
  br label %b2

b6:
  %33 = mul i16 %29, 6
  %34 = getelementptr inbounds i8, ptr %23, i16 %33
  %35 = load ptr, ptr %34
  call addrspace(1) void @N$BDRP(ptr %35)
  %36 = add i16 %29, 1
  store i16 %36, ptr %1, !tbaa !2
  br label %b4
}

define internal void @find(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %1, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %2, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %3) addrspace(1) nearcode {
b1:
  %4 = alloca [8 x i8]
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca i16
  %10 = load ptr, ptr addrspace(1) %1
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  store i16 0, ptr %9, !tbaa !2
  store i16 %12, ptr %8, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %9, !tbaa !2
  %14 = load i16, ptr %8, !tbaa !2
  %15 = icmp ult i16 %13, %14
  %16 = zext i1 %15 to i8
  %17 = icmp ne i8 %16, 0
  br i1 %17, label %b3, label %b5

b3:
  %18 = load ptr, ptr addrspace(1) %1
  %19 = load i16, ptr %9, !tbaa !2
  %20 = getelementptr i8, ptr %18, i16 -4
  %21 = load i16, ptr %20
  %22 = icmp ult i16 %19, %21
  %23 = zext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b6, label %b7

b4:
  %25 = load i16, ptr %9, !tbaa !2
  %26 = add nuw i16 %25, 1
  store i16 %26, ptr %9, !tbaa !2
  br label %b2

b5:
  %27 = load ptr, ptr addrspace(1) %2
  %28 = getelementptr i8, ptr %27, i16 -4
  %29 = load i16, ptr %28
  store i16 0, ptr %6, !tbaa !2
  store i16 %29, ptr %5, !tbaa !2
  br label %b13

b6:
  %30 = mul i16 %19, 6
  %31 = getelementptr inbounds i8, ptr %18, i16 %30
  %32 = load ptr, ptr %31
  %33 = getelementptr i8, ptr %32, i16 -4
  %34 = load i16, ptr %33
  %35 = addrspacecast ptr %32 to ptr addrspace(1)
  store i16 %34, ptr %7, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %34, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %35, ptr %37, !tbaa !2
  %38 = addrspacecast ptr %7 to ptr addrspace(1)
  %39 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %38, ptr addrspace(1) %3)
  %40 = icmp eq i8 %39, 0
  %41 = zext i1 %40 to i8
  %42 = icmp ne i8 %41, 0
  br i1 %42, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %43 = load ptr, ptr addrspace(1) %1
  %44 = load i16, ptr %9, !tbaa !2
  %45 = getelementptr i8, ptr %43, i16 -4
  %46 = load i16, ptr %45
  %47 = icmp ult i16 %44, %46
  %48 = zext i1 %47 to i8
  %49 = icmp ne i8 %48, 0
  br i1 %49, label %b11, label %b12

b9:
  br label %b10

b10:
  br label %b4

b11:
  %50 = mul i16 %44, 6
  %51 = getelementptr inbounds i8, ptr %43, i16 %50
  %52 = addrspacecast ptr %51 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %53 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %52, ptr addrspace(1) %53
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %54 = load i16, ptr %6, !tbaa !2
  %55 = load i16, ptr %5, !tbaa !2
  %56 = icmp ult i16 %54, %55
  %57 = zext i1 %56 to i8
  %58 = icmp ne i8 %57, 0
  br i1 %58, label %b14, label %b16

b14:
  %59 = load ptr, ptr addrspace(1) %2
  %60 = load i16, ptr %6, !tbaa !2
  %61 = getelementptr i8, ptr %59, i16 -4
  %62 = load i16, ptr %61
  %63 = icmp ult i16 %60, %62
  %64 = zext i1 %63 to i8
  %65 = icmp ne i8 %64, 0
  br i1 %65, label %b17, label %b18

b15:
  %66 = load i16, ptr %6, !tbaa !2
  %67 = add nuw i16 %66, 1
  store i16 %67, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %68 = mul i16 %60, 6
  %69 = getelementptr inbounds i8, ptr %59, i16 %68
  %70 = load ptr, ptr %69
  %71 = getelementptr i8, ptr %70, i16 -4
  %72 = load i16, ptr %71
  %73 = addrspacecast ptr %70 to ptr addrspace(1)
  store i16 %72, ptr %4, !tbaa !2
  %74 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %72, ptr %74, !tbaa !2
  %75 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %73, ptr %75, !tbaa !2
  %76 = addrspacecast ptr %4 to ptr addrspace(1)
  %77 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %76, ptr addrspace(1) %3)
  %78 = icmp eq i8 %77, 0
  %79 = zext i1 %78 to i8
  %80 = icmp ne i8 %79, 0
  br i1 %80, label %b19, label %b20

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %81 = load ptr, ptr addrspace(1) %2
  %82 = load i16, ptr %6, !tbaa !2
  %83 = getelementptr i8, ptr %81, i16 -4
  %84 = load i16, ptr %83
  %85 = icmp ult i16 %82, %84
  %86 = zext i1 %85 to i8
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b22, label %b23

b20:
  br label %b21

b21:
  br label %b15

b22:
  %88 = mul i16 %82, 6
  %89 = getelementptr inbounds i8, ptr %81, i16 %88
  %90 = addrspacecast ptr %89 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %91 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %90, ptr addrspace(1) %91
  ret void

b23:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %0, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %1) addrspace(1) nearcode {
b1:
  %2 = alloca ptr addrspace(1)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = icmp ule i16 %4, %6
  %8 = zext i1 %7 to i8
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

define internal void @affordable(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %1, i16 %2) addrspace(1) nearcode {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca i8
  %5 = alloca i16
  store i16 0, ptr %5, !tbaa !2
  br label %b2

b2:
  %6 = load i16, ptr %5, !tbaa !2
  %7 = load ptr, ptr addrspace(1) %1
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = icmp ult i16 %6, %9
  %11 = zext i1 %10 to i8
  store i8 %11, ptr %4, !tbaa !2
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b5, label %b6

b3:
  %13 = load i16, ptr %5, !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr %5, !tbaa !2
  br label %b2

b4:
  %15 = load ptr, ptr addrspace(1) %1
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %15 to ptr addrspace(1)
  %19 = load i16, ptr %5, !tbaa !2
  %20 = icmp ule i16 %19, %17
  %21 = zext i1 %20 to i8
  %22 = icmp ne i8 %21, 0
  br i1 %22, label %b9, label %b10

b5:
  %23 = load ptr, ptr addrspace(1) %1
  %24 = load i16, ptr %5, !tbaa !2
  %25 = getelementptr i8, ptr %23, i16 -4
  %26 = load i16, ptr %25
  %27 = icmp ult i16 %24, %26
  %28 = zext i1 %27 to i8
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b7, label %b8

b6:
  %30 = load i8, ptr %4, !tbaa !2, !range !5
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b3, label %b4

b7:
  %32 = mul i16 %24, 6
  %33 = getelementptr inbounds i8, ptr %23, i16 %32
  %34 = getelementptr i8, ptr %33, i16 2
  %35 = load i16, ptr %34
  %36 = icmp ule i16 %35, %2
  %37 = zext i1 %36 to i8
  store i8 %37, ptr %4, !tbaa !2
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  %38 = icmp ule i16 0, %19
  %39 = zext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b11, label %b12

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  store i16 %19, ptr %3, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %19, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %18, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %3 to ptr addrspace(1)
  %44 = load i16, ptr addrspace(1) %43, !tbaa !2
  store i16 %44, ptr addrspace(1) %0
  %45 = getelementptr i8, ptr addrspace(1) %43, i16 2
  %46 = load i16, ptr addrspace(1) %45, !tbaa !2
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %46, ptr addrspace(1) %47
  %48 = getelementptr i8, ptr addrspace(1) %43, i16 4
  %49 = load ptr addrspace(1), ptr addrspace(1) %48, !tbaa !2
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %49, ptr addrspace(1) %50
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal void @initial(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %1) addrspace(1) nearcode {
b1:
  %2 = alloca [8 x i8]
  %3 = load ptr, ptr addrspace(1) %1
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  %7 = icmp ule i16 1, %5
  %8 = zext i1 %7 to i8
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %b2, label %b3

b2:
  store i16 1, ptr %2, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 1, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %6, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %2 to ptr addrspace(1)
  %13 = load i16, ptr addrspace(1) %12, !tbaa !2
  store i16 %13, ptr addrspace(1) %0
  %14 = getelementptr i8, ptr addrspace(1) %12, i16 2
  %15 = load i16, ptr addrspace(1) %14, !tbaa !2
  %16 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %15, ptr addrspace(1) %16
  %17 = getelementptr i8, ptr addrspace(1) %12, i16 4
  %18 = load ptr addrspace(1), ptr addrspace(1) %17, !tbaa !2
  %19 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %18, ptr addrspace(1) %19
  ret void

b3:
  call addrspace(1) void @N$EBND()
  unreachable
}

define i16 @main() addrspace(1) nearcode {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [6 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [6 x i8]
  %12 = alloca [12 x i8]
  %13 = alloca [8 x i8]
  %14 = alloca [6 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = alloca [2 x i8]
  %18 = alloca [2 x i8]
  %19 = addrspacecast ptr %17 to ptr addrspace(1)
  call addrspace(1) void @north(ptr addrspace(1) %19)
  %20 = load ptr, ptr %17, !tbaa !2
  store ptr %20, ptr %18, !tbaa !2
  %21 = addrspacecast ptr %15 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %21)
  %22 = load ptr, ptr %15, !tbaa !2
  store ptr %22, ptr %16, !tbaa !2
  %23 = addrspacecast ptr %14 to ptr addrspace(1)
  %24 = addrspacecast ptr %18 to ptr addrspace(1)
  %25 = addrspacecast ptr %16 to ptr addrspace(1)
  %26 = getelementptr i8, ptr @$str5, i16 6
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 %28, ptr %13, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %28, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %23, ptr addrspace(1) %24, ptr addrspace(1) %25, ptr addrspace(1) %32)
  %33 = load i8, ptr %14, !tbaa !2, !range !7
  %34 = icmp eq i8 %33, 0
  %35 = zext i1 %34 to i8
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b4, label %b3

b2:
  %37 = addrspacecast ptr %11 to ptr addrspace(1)
  %38 = addrspacecast ptr %18 to ptr addrspace(1)
  %39 = addrspacecast ptr %16 to ptr addrspace(1)
  %40 = getelementptr i8, ptr @$str3, i16 6
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 %42, ptr %10, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %42, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %43, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %37, ptr addrspace(1) %38, ptr addrspace(1) %39, ptr addrspace(1) %46)
  %47 = addrspacecast ptr %9 to ptr addrspace(1)
  %48 = addrspacecast ptr %16 to ptr addrspace(1)
  %49 = addrspacecast ptr %18 to ptr addrspace(1)
  %50 = getelementptr i8, ptr @$str3, i16 6
  %51 = getelementptr i8, ptr %50, i16 -4
  %52 = load i16, ptr %51
  %53 = addrspacecast ptr %50 to ptr addrspace(1)
  store i16 %52, ptr %8, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %52, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %53, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %8 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %47, ptr addrspace(1) %48, ptr addrspace(1) %49, ptr addrspace(1) %56)
  call void @llvm.memcpy.p0.p0.i16(ptr %12, ptr %11, i16 6, i1 false)
  %57 = getelementptr inbounds i8, ptr %12, i16 6
  call void @llvm.memcpy.p0.p0.i16(ptr %57, ptr %9, i16 6, i1 false)
  %58 = load i8, ptr %12, !tbaa !2, !range !7
  %59 = icmp eq i8 %58, 0
  %60 = zext i1 %59 to i8
  %61 = icmp ne i8 %60, 0
  br i1 %61, label %b8, label %b7

b3:
  %62 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %62)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %63 = getelementptr inbounds i8, ptr %14, i16 2
  %64 = load ptr addrspace(1), ptr %63, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %14, i16 2
  %66 = load ptr addrspace(1), ptr %65, !tbaa !2
  %67 = load ptr, ptr addrspace(1) %66
  call addrspace(1) void @N$PS(ptr %67)
  %68 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  %69 = getelementptr i8, ptr addrspace(1) %66, i16 4
  %70 = load i16, ptr addrspace(1) %69
  call addrspace(1) void @N$PU2(i16 %70)
  %71 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %71)
  %72 = getelementptr i8, ptr addrspace(1) %66, i16 2
  %73 = load i16, ptr addrspace(1) %72
  call addrspace(1) void @N$PU2(i16 %73)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %74 = addrspacecast ptr %7 to ptr addrspace(1)
  %75 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %74, ptr addrspace(1) %75, i16 20)
  %76 = load i16, ptr addrspace(1) %74
  store i16 %76, ptr %6, !tbaa !2
  %77 = addrspacecast ptr %5 to ptr addrspace(1)
  %78 = load i16, ptr addrspace(1) %74, !tbaa !2, !range !6
  %79 = icmp ult i16 0, %78
  %80 = zext i1 %79 to i8
  %81 = icmp ne i8 %80, 0
  br i1 %81, label %b11, label %b12

b7:
  %82 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %82)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %83 = getelementptr inbounds i8, ptr %12, i16 2
  %84 = load ptr addrspace(1), ptr %83, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %12, i16 6
  %86 = load i8, ptr %85, !tbaa !2, !range !7
  %87 = icmp eq i8 %86, 0
  %88 = zext i1 %87 to i8
  %89 = icmp ne i8 %88, 0
  br i1 %89, label %b9, label %b7

b9:
  %90 = getelementptr inbounds i8, ptr %12, i16 8
  %91 = load ptr addrspace(1), ptr %90, !tbaa !2
  %92 = getelementptr inbounds i8, ptr %12, i16 2
  %93 = load ptr addrspace(1), ptr %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %12, i16 8
  %95 = load ptr addrspace(1), ptr %94, !tbaa !2
  %96 = call addrspace(1) ptr addrspace(1) @cheaper(ptr addrspace(1) %93, ptr addrspace(1) %95)
  %97 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %97)
  %98 = getelementptr i8, ptr addrspace(1) %96, i16 2
  %99 = load i16, ptr addrspace(1) %98
  call addrspace(1) void @N$PU2(i16 %99)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %100 = getelementptr i8, ptr addrspace(1) %74, i16 4
  %101 = load ptr addrspace(1), ptr addrspace(1) %100, !tbaa !2
  %102 = getelementptr inbounds i8, ptr addrspace(1) %101, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %77, ptr addrspace(1) %102)
  %103 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %103)
  %104 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %104)
  call addrspace(1) void @N$PV(ptr addrspace(1) %77)
  call addrspace(1) void @N$PN()
  %105 = load ptr, ptr %18, !tbaa !2
  %106 = getelementptr i8, ptr %105, i16 -4
  %107 = load i16, ptr %106
  %108 = addrspacecast ptr %105 to ptr addrspace(1)
  store i16 %107, ptr %4, !tbaa !2
  %109 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %107, ptr %109, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %108, ptr %110, !tbaa !2
  %111 = addrspacecast ptr %4 to ptr addrspace(1)
  %112 = load i16, ptr addrspace(1) %111
  store i16 0, ptr %3, !tbaa !2
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %113 = addrspacecast ptr %18 to ptr addrspace(1)
  %114 = getelementptr i8, ptr @$str15, i16 6
  %115 = getelementptr i8, ptr %114, i16 -4
  %116 = load i16, ptr %115
  %117 = addrspacecast ptr %114 to ptr addrspace(1)
  store i16 %116, ptr %2, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %116, ptr %118, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %117, ptr %119, !tbaa !2
  %120 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %113, ptr addrspace(1) %120, i16 1, i16 500)
  %121 = load ptr, ptr %18, !tbaa !2
  %122 = getelementptr i8, ptr %121, i16 -4
  %123 = load i16, ptr %122
  call addrspace(1) void @N$PU2(i16 %123)
  %124 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %124)
  call addrspace(1) void @N$PN()
  %125 = load ptr, ptr %16, !tbaa !2
  %126 = icmp ne ptr %125, null
  %127 = zext i1 %126 to i8
  %128 = icmp ne i8 %127, 0
  br i1 %128, label %b23, label %b22

b14:
  %129 = load i16, ptr %3, !tbaa !2
  %130 = icmp ult i16 %129, %112
  %131 = zext i1 %130 to i8
  %132 = icmp ne i8 %131, 0
  br i1 %132, label %b15, label %b17

b15:
  %133 = getelementptr i8, ptr addrspace(1) %111, i16 4
  %134 = load ptr addrspace(1), ptr addrspace(1) %133, !tbaa !2
  %135 = mul i16 %129, 6
  %136 = getelementptr inbounds i8, ptr addrspace(1) %134, i16 %135
  %137 = getelementptr i8, ptr addrspace(1) %136, i16 4
  %138 = load i16, ptr addrspace(1) %137
  %139 = icmp ult i16 %138, 5
  %140 = zext i1 %139 to i8
  %141 = icmp ne i8 %140, 0
  br i1 %141, label %b18, label %b19

b16:
  %142 = load i16, ptr %3, !tbaa !2
  %143 = add i16 %142, 1
  store i16 %143, ptr %3, !tbaa !2
  br label %b14

b17:
  br label %b13

b18:
  %144 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %144)
  %145 = load ptr, ptr addrspace(1) %136
  call addrspace(1) void @N$PS(ptr %145)
  %146 = getelementptr i8, ptr @$str13, i16 6
  call addrspace(1) void @N$PS(ptr %146)
  %147 = getelementptr i8, ptr addrspace(1) %136, i16 4
  %148 = load i16, ptr addrspace(1) %147
  call addrspace(1) void @N$PU2(i16 %148)
  %149 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %149)
  call addrspace(1) void @N$PN()
  br label %b21

b19:
  br label %b20

b20:
  br label %b16

b21:
  br label %b20

b22:
  call addrspace(1) void @N$BDRP(ptr %125)
  %150 = load ptr, ptr %18, !tbaa !2
  %151 = icmp ne ptr %150, null
  %152 = zext i1 %151 to i8
  %153 = icmp ne i8 %152, 0
  br i1 %153, label %b28, label %b27

b23:
  %154 = getelementptr i8, ptr %125, i16 -4
  %155 = load i16, ptr %154
  store i16 0, ptr %1, !tbaa !2
  br label %b24

b24:
  %156 = load i16, ptr %1, !tbaa !2
  %157 = icmp ult i16 %156, %155
  %158 = zext i1 %157 to i8
  %159 = icmp ne i8 %158, 0
  br i1 %159, label %b26, label %b25

b25:
  br label %b22

b26:
  %160 = mul i16 %156, 6
  %161 = getelementptr inbounds i8, ptr %125, i16 %160
  %162 = load ptr, ptr %161
  call addrspace(1) void @N$BDRP(ptr %162)
  %163 = add i16 %156, 1
  store i16 %163, ptr %1, !tbaa !2
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %150)
  ret i16 0

b28:
  %164 = getelementptr i8, ptr %150, i16 -4
  %165 = load i16, ptr %164
  store i16 0, ptr %0, !tbaa !2
  br label %b29

b29:
  %166 = load i16, ptr %0, !tbaa !2
  %167 = icmp ult i16 %166, %165
  %168 = zext i1 %167 to i8
  %169 = icmp ne i8 %168, 0
  br i1 %169, label %b31, label %b30

b30:
  br label %b27

b31:
  %170 = mul i16 %166, 6
  %171 = getelementptr inbounds i8, ptr %150, i16 %170
  %172 = load ptr, ptr %171
  call addrspace(1) void @N$BDRP(ptr %172)
  %173 = add i16 %166, 1
  store i16 %173, ptr %0, !tbaa !2
  br label %b29
}

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p0.p0.i16(ptr nocapture writeonly, ptr nocapture readonly, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: readwrite)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
