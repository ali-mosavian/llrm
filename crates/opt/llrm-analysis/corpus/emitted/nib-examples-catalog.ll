target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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

define internal cc 1005 void @Catalog.add(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1, i16 %2, i16 %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %4 = load ptr, ptr addrspace(1) %0
  %5 = getelementptr i8, ptr %4, i16 -4
  %6 = load i16, ptr %5
  %7 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %4, i16 1, i16 6)
  store ptr %7, ptr addrspace(1) %0
  %8 = mul i16 %6, 6
  %9 = getelementptr inbounds i8, ptr %7, i16 %8
  %10 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %1)
  store ptr %10, ptr %9
  %11 = getelementptr i8, ptr %9, i16 2
  store i16 %2, ptr %11
  %12 = getelementptr i8, ptr %9, i16 4
  store i16 %3, ptr %12
  ret void
}

define internal cc 1005 void @north(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  call cc 1005 addrspace(1) void @Catalog.add(ptr addrspace(1) %7, ptr addrspace(1) %14, i16 5, i16 40)
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
  call cc 1005 addrspace(1) void @Catalog.add(ptr addrspace(1) %15, ptr addrspace(1) %22, i16 30, i16 3)
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
  call cc 1005 addrspace(1) void @Catalog.add(ptr addrspace(1) %23, ptr addrspace(1) %30, i16 12, i16 0)
  %31 = load ptr, ptr %5, !tbaa !2
  store ptr %31, ptr addrspace(1) %0
  store ptr null, ptr %5, !tbaa !2
  %32 = load ptr, ptr %5, !tbaa !2
  %33 = icmp ne ptr %32, null
  br i1 %33, label %b3, label %b2

b2:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %32)
  ret void

b3:
  %34 = getelementptr i8, ptr %32, i16 -4
  %35 = load i16, ptr %34
  store i16 0, ptr %1, !tbaa !2
  br label %b4

b4:
  %36 = load i16, ptr %1, !tbaa !2
  %37 = icmp ult i16 %36, %35
  br i1 %37, label %b6, label %b5

b5:
  br label %b2

b6:
  %38 = mul i16 %36, 6
  %39 = getelementptr inbounds i8, ptr %32, i16 %38
  %40 = load ptr, ptr %39
  call cc 1005 addrspace(1) void @N$BDRP(ptr %40)
  %41 = add i16 %36, 1
  store i16 %41, ptr %1, !tbaa !2
  br label %b4
}

define internal cc 1005 void @south(ptr addrspace(1) %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  call cc 1005 addrspace(1) void @Catalog.add(ptr addrspace(1) %6, ptr addrspace(1) %13, i16 28, i16 9)
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
  call cc 1005 addrspace(1) void @Catalog.add(ptr addrspace(1) %14, ptr addrspace(1) %21, i16 2, i16 100)
  %22 = load ptr, ptr %4, !tbaa !2
  store ptr %22, ptr addrspace(1) %0
  store ptr null, ptr %4, !tbaa !2
  %23 = load ptr, ptr %4, !tbaa !2
  %24 = icmp ne ptr %23, null
  br i1 %24, label %b3, label %b2

b2:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %23)
  ret void

b3:
  %25 = getelementptr i8, ptr %23, i16 -4
  %26 = load i16, ptr %25
  store i16 0, ptr %1, !tbaa !2
  br label %b4

b4:
  %27 = load i16, ptr %1, !tbaa !2
  %28 = icmp ult i16 %27, %26
  br i1 %28, label %b6, label %b5

b5:
  br label %b2

b6:
  %29 = mul i16 %27, 6
  %30 = getelementptr inbounds i8, ptr %23, i16 %29
  %31 = load ptr, ptr %30
  call cc 1005 addrspace(1) void @N$BDRP(ptr %31)
  %32 = add i16 %27, 1
  store i16 %32, ptr %1, !tbaa !2
  br label %b4
}

define internal cc 1005 void @find(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %1, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %2, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %3) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  br i1 %15, label %b3, label %b5

b3:
  %16 = load ptr, ptr addrspace(1) %1
  %17 = load i16, ptr %9, !tbaa !2
  %18 = getelementptr i8, ptr %16, i16 -4
  %19 = load i16, ptr %18
  %20 = icmp ult i16 %17, %19
  br i1 %20, label %b9, label %b10

b4:
  %21 = load i16, ptr %9, !tbaa !2
  %22 = add nuw i16 %21, 1
  store i16 %22, ptr %9, !tbaa !2
  br label %b2

b5:
  %23 = load ptr, ptr addrspace(1) %2
  %24 = getelementptr i8, ptr %23, i16 -4
  %25 = load i16, ptr %24
  store i16 0, ptr %6, !tbaa !2
  store i16 %25, ptr %5, !tbaa !2
  br label %b13

b6:
  %26 = load ptr, ptr addrspace(1) %1
  %27 = load i16, ptr %9, !tbaa !2
  %28 = getelementptr i8, ptr %26, i16 -4
  %29 = load i16, ptr %28
  %30 = icmp ult i16 %27, %29
  br i1 %30, label %b11, label %b12

b7:
  br label %b8

b8:
  br label %b4

b9:
  %31 = mul i16 %17, 6
  %32 = getelementptr inbounds i8, ptr %16, i16 %31
  %33 = load ptr, ptr %32
  %34 = getelementptr i8, ptr %33, i16 -4
  %35 = load i16, ptr %34
  %36 = addrspacecast ptr %33 to ptr addrspace(1)
  store i16 %35, ptr %7, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %35, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %36, ptr %38, !tbaa !2
  %39 = addrspacecast ptr %7 to ptr addrspace(1)
  %40 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %39, ptr addrspace(1) %3)
  %41 = icmp eq i8 %40, 0
  br i1 %41, label %b6, label %b7

b10:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b11:
  %42 = mul i16 %27, 6
  %43 = getelementptr inbounds i8, ptr %26, i16 %42
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %45 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %44, ptr addrspace(1) %45
  ret void

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b13:
  %46 = load i16, ptr %6, !tbaa !2
  %47 = load i16, ptr %5, !tbaa !2
  %48 = icmp ult i16 %46, %47
  br i1 %48, label %b14, label %b16

b14:
  %49 = load ptr, ptr addrspace(1) %2
  %50 = load i16, ptr %6, !tbaa !2
  %51 = getelementptr i8, ptr %49, i16 -4
  %52 = load i16, ptr %51
  %53 = icmp ult i16 %50, %52
  br i1 %53, label %b20, label %b21

b15:
  %54 = load i16, ptr %6, !tbaa !2
  %55 = add nuw i16 %54, 1
  store i16 %55, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %56 = load ptr, ptr addrspace(1) %2
  %57 = load i16, ptr %6, !tbaa !2
  %58 = getelementptr i8, ptr %56, i16 -4
  %59 = load i16, ptr %58
  %60 = icmp ult i16 %57, %59
  br i1 %60, label %b22, label %b23

b18:
  br label %b19

b19:
  br label %b15

b20:
  %61 = mul i16 %50, 6
  %62 = getelementptr inbounds i8, ptr %49, i16 %61
  %63 = load ptr, ptr %62
  %64 = getelementptr i8, ptr %63, i16 -4
  %65 = load i16, ptr %64
  %66 = addrspacecast ptr %63 to ptr addrspace(1)
  store i16 %65, ptr %4, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %65, ptr %67, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %66, ptr %68, !tbaa !2
  %69 = addrspacecast ptr %4 to ptr addrspace(1)
  %70 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %69, ptr addrspace(1) %3)
  %71 = icmp eq i8 %70, 0
  br i1 %71, label %b17, label %b18

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b22:
  %72 = mul i16 %57, 6
  %73 = getelementptr inbounds i8, ptr %56, i16 %72
  %74 = addrspacecast ptr %73 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %75 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %74, ptr addrspace(1) %75
  ret void

b23:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %0, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca ptr addrspace(1)
  %3 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %4 = load i16, ptr addrspace(1) %3
  %5 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %6 = load i16, ptr addrspace(1) %5
  %7 = icmp ule i16 %4, %6
  br i1 %7, label %b2, label %b3

b2:
  store ptr addrspace(1) %0, ptr %2, !tbaa !2
  br label %b4

b3:
  store ptr addrspace(1) %1, ptr %2, !tbaa !2
  br label %b4

b4:
  %8 = load ptr addrspace(1), ptr %2, !tbaa !2
  ret ptr addrspace(1) %8
}

define internal cc 1005 void @affordable(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias %1, i16 %2) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca i16
  store i16 0, ptr %4, !tbaa !2
  br label %b2

b2:
  %5 = load i16, ptr %4, !tbaa !2
  %6 = load ptr, ptr addrspace(1) %1
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = icmp ult i16 %5, %8
  br i1 %9, label %b5, label %b4

b3:
  %10 = load i16, ptr %4, !tbaa !2
  %11 = add i16 %10, 1
  store i16 %11, ptr %4, !tbaa !2
  br label %b2

b4:
  %12 = load ptr, ptr addrspace(1) %1
  %13 = getelementptr i8, ptr %12, i16 -4
  %14 = load i16, ptr %13
  %15 = addrspacecast ptr %12 to ptr addrspace(1)
  %16 = load i16, ptr %4, !tbaa !2
  %17 = icmp ule i16 %16, %14
  br i1 %17, label %b8, label %b9

b5:
  %18 = load ptr, ptr addrspace(1) %1
  %19 = load i16, ptr %4, !tbaa !2
  %20 = getelementptr i8, ptr %18, i16 -4
  %21 = load i16, ptr %20
  %22 = icmp ult i16 %19, %21
  br i1 %22, label %b6, label %b7

b6:
  %23 = mul i16 %19, 6
  %24 = getelementptr inbounds i8, ptr %18, i16 %23
  %25 = getelementptr i8, ptr %24, i16 2
  %26 = load i16, ptr %25
  %27 = icmp ule i16 %26, %2
  br i1 %27, label %b3, label %b4

b7:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b8:
  %28 = icmp ule i16 0, %16
  br i1 %28, label %b10, label %b11

b9:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b10:
  store i16 %16, ptr %3, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %16, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %15, ptr %30, !tbaa !2
  %31 = addrspacecast ptr %3 to ptr addrspace(1)
  %32 = load i16, ptr addrspace(1) %31, !tbaa !2
  store i16 %32, ptr addrspace(1) %0
  %33 = getelementptr i8, ptr addrspace(1) %31, i16 2
  %34 = load i16, ptr addrspace(1) %33, !tbaa !2
  %35 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %34, ptr addrspace(1) %35
  %36 = getelementptr i8, ptr addrspace(1) %31, i16 4
  %37 = load ptr addrspace(1), ptr addrspace(1) %36, !tbaa !2
  %38 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %37, ptr addrspace(1) %38
  ret void

b11:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 void @initial(ptr addrspace(1) %0, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca [8 x i8]
  %3 = load ptr, ptr addrspace(1) %1
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  %6 = addrspacecast ptr %3 to ptr addrspace(1)
  %7 = icmp ule i16 1, %5
  br i1 %7, label %b2, label %b3

b2:
  store i16 1, ptr %2, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 1, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %6, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %2 to ptr addrspace(1)
  %11 = load i16, ptr addrspace(1) %10, !tbaa !2
  store i16 %11, ptr addrspace(1) %0
  %12 = getelementptr i8, ptr addrspace(1) %10, i16 2
  %13 = load i16, ptr addrspace(1) %12, !tbaa !2
  %14 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %13, ptr addrspace(1) %14
  %15 = getelementptr i8, ptr addrspace(1) %10, i16 4
  %16 = load ptr addrspace(1), ptr addrspace(1) %15, !tbaa !2
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %16, ptr addrspace(1) %17
  ret void

b3:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
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
  call cc 1005 addrspace(1) void @north(ptr addrspace(1) %19)
  %20 = load ptr, ptr %17, !tbaa !2
  store ptr %20, ptr %18, !tbaa !2
  %21 = addrspacecast ptr %15 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @south(ptr addrspace(1) %21)
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
  call cc 1005 addrspace(1) void @find(ptr addrspace(1) %23, ptr addrspace(1) %24, ptr addrspace(1) %25, ptr addrspace(1) %32)
  %33 = load i8, ptr %14, !tbaa !2, !range !6
  %34 = icmp eq i8 %33, 0
  br i1 %34, label %b4, label %b3

b2:
  %35 = addrspacecast ptr %11 to ptr addrspace(1)
  %36 = addrspacecast ptr %18 to ptr addrspace(1)
  %37 = addrspacecast ptr %16 to ptr addrspace(1)
  %38 = getelementptr i8, ptr @$str3, i16 6
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %10, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %40, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %41, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %10 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @find(ptr addrspace(1) %35, ptr addrspace(1) %36, ptr addrspace(1) %37, ptr addrspace(1) %44)
  %45 = addrspacecast ptr %9 to ptr addrspace(1)
  %46 = addrspacecast ptr %16 to ptr addrspace(1)
  %47 = addrspacecast ptr %18 to ptr addrspace(1)
  %48 = getelementptr i8, ptr @$str3, i16 6
  %49 = getelementptr i8, ptr %48, i16 -4
  %50 = load i16, ptr %49
  %51 = addrspacecast ptr %48 to ptr addrspace(1)
  store i16 %50, ptr %8, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %50, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %51, ptr %53, !tbaa !2
  %54 = addrspacecast ptr %8 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @find(ptr addrspace(1) %45, ptr addrspace(1) %46, ptr addrspace(1) %47, ptr addrspace(1) %54)
  call void @llvm.memcpy.p0.p0.i16(ptr %12, ptr %11, i16 6, i1 false)
  %55 = getelementptr inbounds i8, ptr %12, i16 6
  call void @llvm.memcpy.p0.p0.i16(ptr %55, ptr %9, i16 6, i1 false)
  %56 = load i8, ptr %12, !tbaa !2, !range !6
  %57 = icmp eq i8 %56, 0
  br i1 %57, label %b8, label %b7

b3:
  %58 = getelementptr i8, ptr @$str8, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %58)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b4:
  %59 = getelementptr inbounds i8, ptr %14, i16 2
  %60 = load ptr addrspace(1), ptr %59, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %14, i16 2
  %62 = load ptr addrspace(1), ptr %61, !tbaa !2
  %63 = load ptr, ptr addrspace(1) %62
  call cc 1005 addrspace(1) void @N$PS(ptr %63)
  %64 = getelementptr i8, ptr @$str6, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %64)
  %65 = getelementptr i8, ptr addrspace(1) %62, i16 4
  %66 = load i16, ptr addrspace(1) %65
  call cc 1005 addrspace(1) void @N$PU2(i16 %66)
  %67 = getelementptr i8, ptr @$str7, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %67)
  %68 = getelementptr i8, ptr addrspace(1) %62, i16 2
  %69 = load i16, ptr addrspace(1) %68
  call cc 1005 addrspace(1) void @N$PU2(i16 %69)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b2

b6:
  %70 = addrspacecast ptr %7 to ptr addrspace(1)
  %71 = addrspacecast ptr %18 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @affordable(ptr addrspace(1) %70, ptr addrspace(1) %71, i16 20)
  %72 = load i16, ptr addrspace(1) %70
  store i16 %72, ptr %6, !tbaa !2
  %73 = addrspacecast ptr %5 to ptr addrspace(1)
  %74 = load i16, ptr addrspace(1) %70, !tbaa !2, !range !5
  %75 = icmp ult i16 0, %74
  br i1 %75, label %b11, label %b12

b7:
  %76 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %76)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b6

b8:
  %77 = getelementptr inbounds i8, ptr %12, i16 2
  %78 = load ptr addrspace(1), ptr %77, !tbaa !2
  %79 = getelementptr inbounds i8, ptr %12, i16 6
  %80 = load i8, ptr %79, !tbaa !2, !range !6
  %81 = icmp eq i8 %80, 0
  br i1 %81, label %b9, label %b7

b9:
  %82 = getelementptr inbounds i8, ptr %12, i16 8
  %83 = load ptr addrspace(1), ptr %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr %12, i16 2
  %85 = load ptr addrspace(1), ptr %84, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %12, i16 8
  %87 = load ptr addrspace(1), ptr %86, !tbaa !2
  %88 = call cc 1005 addrspace(1) ptr addrspace(1) @cheaper(ptr addrspace(1) %85, ptr addrspace(1) %87)
  %89 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %89)
  %90 = getelementptr i8, ptr addrspace(1) %88, i16 2
  %91 = load i16, ptr addrspace(1) %90
  call cc 1005 addrspace(1) void @N$PU2(i16 %91)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b6

b11:
  %92 = getelementptr i8, ptr addrspace(1) %70, i16 4
  %93 = load ptr addrspace(1), ptr addrspace(1) %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr addrspace(1) %93, i16 0
  call cc 1005 addrspace(1) void @initial(ptr addrspace(1) %73, ptr addrspace(1) %94)
  %95 = load i16, ptr %6, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %95)
  %96 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %96)
  call cc 1005 addrspace(1) void @N$PV(ptr addrspace(1) %73)
  call cc 1005 addrspace(1) void @N$PN()
  %97 = load ptr, ptr %18, !tbaa !2
  %98 = getelementptr i8, ptr %97, i16 -4
  %99 = load i16, ptr %98
  %100 = addrspacecast ptr %97 to ptr addrspace(1)
  store i16 %99, ptr %4, !tbaa !2
  %101 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %99, ptr %101, !tbaa !2
  %102 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %100, ptr %102, !tbaa !2
  %103 = addrspacecast ptr %4 to ptr addrspace(1)
  %104 = load i16, ptr addrspace(1) %103
  store i16 0, ptr %3, !tbaa !2
  br label %b14

b12:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b13:
  %105 = addrspacecast ptr %18 to ptr addrspace(1)
  %106 = getelementptr i8, ptr @$str15, i16 6
  %107 = getelementptr i8, ptr %106, i16 -4
  %108 = load i16, ptr %107
  %109 = addrspacecast ptr %106 to ptr addrspace(1)
  store i16 %108, ptr %2, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %108, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %109, ptr %111, !tbaa !2
  %112 = addrspacecast ptr %2 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @Catalog.add(ptr addrspace(1) %105, ptr addrspace(1) %112, i16 1, i16 500)
  %113 = load ptr, ptr %18, !tbaa !2
  %114 = getelementptr i8, ptr %113, i16 -4
  %115 = load i16, ptr %114
  call cc 1005 addrspace(1) void @N$PU2(i16 %115)
  %116 = getelementptr i8, ptr @$str16, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %116)
  call cc 1005 addrspace(1) void @N$PN()
  %117 = load ptr, ptr %16, !tbaa !2
  %118 = icmp ne ptr %117, null
  br i1 %118, label %b23, label %b22

b14:
  %119 = load i16, ptr %3, !tbaa !2
  %120 = icmp ult i16 %119, %104
  br i1 %120, label %b15, label %b17

b15:
  %121 = getelementptr i8, ptr addrspace(1) %103, i16 4
  %122 = load ptr addrspace(1), ptr addrspace(1) %121, !tbaa !2
  %123 = mul i16 %119, 6
  %124 = getelementptr inbounds i8, ptr addrspace(1) %122, i16 %123
  %125 = getelementptr i8, ptr addrspace(1) %124, i16 4
  %126 = load i16, ptr addrspace(1) %125
  %127 = icmp ult i16 %126, 5
  br i1 %127, label %b18, label %b19

b16:
  %128 = load i16, ptr %3, !tbaa !2
  %129 = add i16 %128, 1
  store i16 %129, ptr %3, !tbaa !2
  br label %b14

b17:
  br label %b13

b18:
  %130 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %130)
  %131 = load ptr, ptr addrspace(1) %124
  call cc 1005 addrspace(1) void @N$PS(ptr %131)
  %132 = getelementptr i8, ptr @$str13, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %132)
  %133 = getelementptr i8, ptr addrspace(1) %124, i16 4
  %134 = load i16, ptr addrspace(1) %133
  call cc 1005 addrspace(1) void @N$PU2(i16 %134)
  %135 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %135)
  call cc 1005 addrspace(1) void @N$PN()
  br label %b21

b19:
  br label %b20

b20:
  br label %b16

b21:
  br label %b20

b22:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %117)
  %136 = load ptr, ptr %18, !tbaa !2
  %137 = icmp ne ptr %136, null
  br i1 %137, label %b28, label %b27

b23:
  %138 = getelementptr i8, ptr %117, i16 -4
  %139 = load i16, ptr %138
  store i16 0, ptr %1, !tbaa !2
  br label %b24

b24:
  %140 = load i16, ptr %1, !tbaa !2
  %141 = icmp ult i16 %140, %139
  br i1 %141, label %b26, label %b25

b25:
  br label %b22

b26:
  %142 = mul i16 %140, 6
  %143 = getelementptr inbounds i8, ptr %117, i16 %142
  %144 = load ptr, ptr %143
  call cc 1005 addrspace(1) void @N$BDRP(ptr %144)
  %145 = add i16 %140, 1
  store i16 %145, ptr %1, !tbaa !2
  br label %b24

b27:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %136)
  ret i16 0

b28:
  %146 = getelementptr i8, ptr %136, i16 -4
  %147 = load i16, ptr %146
  store i16 0, ptr %0, !tbaa !2
  br label %b29

b29:
  %148 = load i16, ptr %0, !tbaa !2
  %149 = icmp ult i16 %148, %147
  br i1 %149, label %b31, label %b30

b30:
  br label %b27

b31:
  %150 = mul i16 %148, 6
  %151 = getelementptr inbounds i8, ptr %136, i16 %150
  %152 = load ptr, ptr %151
  call cc 1005 addrspace(1) void @N$BDRP(ptr %152)
  %153 = add i16 %148, 1
  store i16 %153, ptr %0, !tbaa !2
  br label %b29
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PV(ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i16 0, i16 10923}
!6 = !{i8 0, i8 2}
