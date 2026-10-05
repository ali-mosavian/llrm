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

define internal void @Catalog.add(ptr addrspace(5) nocapture %0, ptr addrspace(5) %1, i16 range(i16 1, 31) %2, i16 range(i16 0, 501) %3) addrspace(1) nearcode {
b1:
  %4 = addrspacecast ptr addrspace(5) %1 to ptr addrspace(1)
  %5 = load ptr, ptr addrspace(5) %0
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  %8 = call addrspace(1) ptr @N$BGRW(ptr %5, i16 1, i16 6)
  store ptr %8, ptr addrspace(5) %0
  %9 = mul i16 %7, 6
  %10 = getelementptr inbounds i8, ptr %8, i16 %9
  %11 = call addrspace(1) ptr @N$VCPY(ptr addrspace(1) %4)
  store ptr %11, ptr %10
  %12 = getelementptr i8, ptr %10, i16 2
  store i16 %2, ptr %12
  %13 = getelementptr i8, ptr %10, i16 4
  store i16 %3, ptr %13
  ret void
}

define internal void @north(ptr addrspace(5) nocapture %0) addrspace(1) nearcode memory(readwrite, argmem: write) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [2 x i8]
  %5 = getelementptr i8, ptr @$str1, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = addrspacecast ptr %4 to ptr addrspace(5)
  %7 = getelementptr i8, ptr @$str2, i16 6
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %3 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %11, i16 5, i16 40)
  %12 = getelementptr i8, ptr @$str3, i16 6
  %13 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %2 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %16, i16 30, i16 3)
  %17 = getelementptr i8, ptr @$str4, i16 6
  %18 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %21, i16 12, i16 0)
  %22 = load ptr, ptr %4, !tbaa !2
  store ptr %22, ptr addrspace(5) %0
  store ptr null, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal void @south(ptr addrspace(5) nocapture %0) addrspace(1) nearcode memory(readwrite, argmem: write) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [2 x i8]
  %4 = getelementptr i8, ptr @$str1, i16 6
  store ptr %4, ptr %3, !tbaa !2
  %5 = addrspacecast ptr %3 to ptr addrspace(5)
  %6 = getelementptr i8, ptr @$str3, i16 6
  %7 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %7, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %2 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %5, ptr addrspace(5) %10, i16 28, i16 9)
  %11 = getelementptr i8, ptr @$str5, i16 6
  %12 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %1 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %5, ptr addrspace(5) %15, i16 2, i16 100)
  %16 = load ptr, ptr %3, !tbaa !2
  store ptr %16, ptr addrspace(5) %0
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal void @find(ptr addrspace(5) nocapture %0, ptr %1, ptr %2, ptr addrspace(5) %3) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = addrspacecast ptr addrspace(5) %3 to ptr addrspace(1)
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = getelementptr i8, ptr %1, i16 -4
  %8 = load i16, ptr %7
  %9 = getelementptr inbounds i8, ptr %6, i16 2
  %10 = getelementptr inbounds i8, ptr %6, i16 4
  %11 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %b2

b2:
  %12 = phi i16 [ 0, %b1 ], [ %22, %b4 ]
  %13 = icmp ult i16 %12, %8
  br i1 %13, label %b3, label %b5

b3:
  %14 = mul i16 %12, 6
  %15 = getelementptr inbounds i8, ptr %1, i16 %14
  %16 = load ptr, ptr %15
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 %18, ptr %6, !tbaa !2
  store i16 %18, ptr %9, !tbaa !2
  store ptr addrspace(1) %19, ptr %10, !tbaa !2
  %20 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %11, ptr addrspace(1) %4)
  %21 = icmp eq i8 %20, 0
  br i1 %21, label %b8, label %b4

b4:
  %22 = add nuw i16 %12, 1
  br label %b2

b5:
  %23 = getelementptr i8, ptr %2, i16 -4
  %24 = load i16, ptr %23
  %25 = getelementptr inbounds i8, ptr %5, i16 2
  %26 = getelementptr inbounds i8, ptr %5, i16 4
  %27 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b13

b8:
  %28 = phi i16 [ %12, %b3 ]
  %29 = icmp ult i16 %28, %8
  br i1 %29, label %b11, label %b12

b11:
  %30 = mul i16 %28, 6
  %31 = getelementptr inbounds i8, ptr %1, i16 %30
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %33 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %32, ptr addrspace(5) %33
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %34 = phi i16 [ 0, %b5 ], [ %44, %b15 ]
  %35 = icmp ult i16 %34, %24
  br i1 %35, label %b14, label %b16

b14:
  %36 = mul i16 %34, 6
  %37 = getelementptr inbounds i8, ptr %2, i16 %36
  %38 = load ptr, ptr %37
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %5, !tbaa !2
  store i16 %40, ptr %25, !tbaa !2
  store ptr addrspace(1) %41, ptr %26, !tbaa !2
  %42 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %27, ptr addrspace(1) %4)
  %43 = icmp eq i8 %42, 0
  br i1 %43, label %b19, label %b15

b15:
  %44 = add nuw i16 %34, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(5) %0
  ret void

b19:
  %45 = phi i16 [ %34, %b14 ]
  %46 = icmp ult i16 %45, %24
  br i1 %46, label %b22, label %b23

b22:
  %47 = mul i16 %45, 6
  %48 = getelementptr inbounds i8, ptr %2, i16 %47
  %49 = addrspacecast ptr %48 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %50 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %49, ptr addrspace(5) %50
  ret void

b23:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %0, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias %1) addrspace(1) nearcode memory(argmem: read) willreturn norecurse {
b1:
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 2
  %3 = load i16, ptr addrspace(1) %2
  %4 = getelementptr i8, ptr addrspace(1) %1, i16 2
  %5 = load i16, ptr addrspace(1) %4
  %6 = icmp ule i16 %3, %5
  br i1 %6, label %b2, label %b3

b2:
  br label %b4

b3:
  br label %b4

b4:
  %7 = phi ptr addrspace(1) [ %0, %b2 ], [ %1, %b3 ]
  ret ptr addrspace(1) %7
}

define internal void @affordable(ptr addrspace(5) nocapture writeonly %0, ptr %1) addrspace(1) nearcode memory(argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  br label %b2

b2:
  %4 = phi i16 [ 0, %b1 ], [ %6, %b3 ]
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b5, label %17

b3:
  %6 = add i16 %4, 1
  br label %b2

b4:
  %7 = phi i16 [ %18, %17 ], [ %20, %19 ]
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  %9 = icmp ule i16 %7, %3
  br i1 %9, label %b9, label %b10

b5:
  %10 = mul i16 %4, 6
  %11 = getelementptr inbounds i8, ptr %1, i16 %10
  %12 = getelementptr i8, ptr %11, i16 2
  %13 = load i16, ptr %12
  %14 = icmp ule i16 %13, 20
  br i1 %14, label %b3, label %19

b9:
  store i16 %7, ptr addrspace(5) %0
  %15 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 %7, ptr addrspace(5) %15
  %16 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store ptr addrspace(1) %8, ptr addrspace(5) %16
  ret void

b10:
  call addrspace(1) void @N$EBND()
  unreachable

17:
  %18 = phi i16 [ %4, %b2 ]
  br label %b4

19:
  %20 = phi i16 [ %4, %b5 ]
  br label %b4
}

define internal void @initial(ptr addrspace(5) nocapture writeonly %0, ptr %1) addrspace(1) nearcode memory(argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  %5 = icmp uge i16 %3, 1
  br i1 %5, label %b2, label %b3

b2:
  store i16 1, ptr addrspace(5) %0
  %6 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 1, ptr addrspace(5) %6
  %7 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store ptr addrspace(1) %4, ptr addrspace(5) %7
  ret void

b3:
  call addrspace(1) void @N$EBND()
  unreachable
}

define i16 @main() addrspace(1) nearcode memory(readwrite, argmem: none) {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [6 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [6 x i8]
  %9 = alloca [2 x i8]
  %10 = alloca [2 x i8]
  %11 = alloca [2 x i8]
  %12 = addrspacecast ptr %10 to ptr addrspace(5)
  call addrspace(1) void @north(ptr addrspace(5) %12)
  %13 = load ptr, ptr %10, !tbaa !2
  store ptr %13, ptr %11, !tbaa !2
  %14 = addrspacecast ptr %9 to ptr addrspace(5)
  call addrspace(1) void @south(ptr addrspace(5) %14)
  %15 = load ptr, ptr %9, !tbaa !2
  %16 = addrspacecast ptr %8 to ptr addrspace(5)
  %17 = addrspacecast ptr %11 to ptr addrspace(5)
  %18 = getelementptr i8, ptr @$str5, i16 6
  %19 = addrspacecast ptr %18 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %19, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %7 to ptr addrspace(5)
  %23 = load ptr, ptr addrspace(5) %17
  call addrspace(1) void @find(ptr addrspace(5) %16, ptr %23, ptr %15, ptr addrspace(5) %22)
  %24 = load i8, ptr %8, !tbaa !2, !range !7
  %25 = icmp eq i8 %24, 0
  br i1 %25, label %b4, label %b3

b2:
  %26 = addrspacecast ptr %6 to ptr addrspace(5)
  %27 = getelementptr i8, ptr @$str3, i16 6
  %28 = addrspacecast ptr %27 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %28, ptr %30, !tbaa !2
  %31 = addrspacecast ptr %5 to ptr addrspace(5)
  %32 = load ptr, ptr addrspace(5) %17
  call addrspace(1) void @find(ptr addrspace(5) %26, ptr %32, ptr %15, ptr addrspace(5) %31)
  %33 = addrspacecast ptr %4 to ptr addrspace(5)
  store i16 4, ptr %3, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %28, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %3 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %33, ptr %15, ptr %32, ptr addrspace(5) %36)
  %37 = load i8, ptr %6
  %38 = getelementptr i8, ptr %6, i16 2
  %39 = load ptr addrspace(1), ptr %38
  %40 = load i8, ptr %4
  %41 = getelementptr i8, ptr %4, i16 2
  %42 = load ptr addrspace(1), ptr %41
  %43 = icmp eq i8 %37, 0
  br i1 %43, label %b8, label %b7

b3:
  %44 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %44)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %45 = getelementptr inbounds i8, ptr %8, i16 2
  %46 = load ptr addrspace(1), ptr %45, !tbaa !2
  %47 = load ptr, ptr addrspace(1) %46
  call addrspace(1) void @N$PS(ptr %47)
  %48 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  %49 = getelementptr i8, ptr addrspace(1) %46, i16 4
  %50 = load i16, ptr addrspace(1) %49
  call addrspace(1) void @N$PU2(i16 %50)
  %51 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %51)
  %52 = getelementptr i8, ptr addrspace(1) %46, i16 2
  %53 = load i16, ptr addrspace(1) %52
  call addrspace(1) void @N$PU2(i16 %53)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %54 = addrspacecast ptr %2 to ptr addrspace(5)
  %55 = load ptr, ptr addrspace(5) %17
  call addrspace(1) void @affordable(ptr addrspace(5) %54, ptr %55)
  %56 = load i16, ptr addrspace(5) %54
  %57 = addrspacecast ptr %1 to ptr addrspace(5)
  %58 = addrspacecast ptr %1 to ptr addrspace(1)
  %59 = icmp ne i16 %56, 0
  br i1 %59, label %b11, label %b12

b7:
  %60 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %60)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %61 = icmp eq i8 %40, 0
  br i1 %61, label %62, label %b7

62:
  %63 = getelementptr i8, ptr addrspace(1) %39, i16 2
  %64 = load i16, ptr addrspace(1) %63
  %65 = getelementptr i8, ptr addrspace(1) %42, i16 2
  %66 = load i16, ptr addrspace(1) %65
  %67 = icmp ule i16 %64, %66
  br i1 %67, label %68, label %69

68:
  br label %70

69:
  br label %70

70:
  %71 = phi ptr addrspace(1) [ %63, %68 ], [ %65, %69 ]
  %72 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %72)
  %73 = load i16, ptr addrspace(1) %71
  call addrspace(1) void @N$PU2(i16 %73)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %74 = getelementptr i8, ptr addrspace(5) %54, i16 4
  %75 = load ptr addrspace(1), ptr addrspace(5) %74, !tbaa !2
  %76 = getelementptr inbounds i8, ptr addrspace(1) %75, i16 0
  %77 = load ptr, ptr addrspace(1) %76
  call addrspace(1) void @initial(ptr addrspace(5) %57, ptr %77)
  call addrspace(1) void @N$PU2(i16 %56)
  %78 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  call addrspace(1) void @N$PV(ptr addrspace(1) %58)
  call addrspace(1) void @N$PN()
  %79 = load ptr, ptr %11, !tbaa !2
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  %82 = getelementptr i8, ptr @$str12, i16 6
  %83 = getelementptr i8, ptr @$str13, i16 6
  %84 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %85 = phi i16 [ 0, %b11 ], [ %92, %b16 ]
  %86 = icmp ult i16 %85, %81
  br i1 %86, label %b15, label %b17

b15:
  %87 = mul i16 %85, 6
  %88 = getelementptr inbounds i8, ptr %79, i16 %87
  %89 = getelementptr i8, ptr %88, i16 4
  %90 = load i16, ptr %89
  %91 = icmp ult i16 %90, 5
  br i1 %91, label %b18, label %b16

b16:
  %92 = add i16 %85, 1
  br label %b14

b17:
  %93 = getelementptr i8, ptr @$str15, i16 6
  %94 = addrspacecast ptr %93 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %95 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %95, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %94, ptr %96, !tbaa !2
  %97 = addrspacecast ptr %0 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %17, ptr addrspace(5) %97, i16 1, i16 500)
  %98 = load ptr, ptr %11, !tbaa !2
  %99 = getelementptr i8, ptr %98, i16 -4
  %100 = load i16, ptr %99
  call addrspace(1) void @N$PU2(i16 %100)
  %101 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %101)
  call addrspace(1) void @N$PN()
  %102 = icmp ne ptr %15, null
  br i1 %102, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %82)
  %103 = load ptr, ptr %88
  call addrspace(1) void @N$PS(ptr %103)
  call addrspace(1) void @N$PS(ptr %83)
  %104 = load i16, ptr %89
  call addrspace(1) void @N$PU2(i16 %104)
  call addrspace(1) void @N$PS(ptr %84)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %15)
  %105 = load ptr, ptr %11, !tbaa !2
  %106 = icmp ne ptr %105, null
  br i1 %106, label %b28, label %b27

b23:
  %107 = getelementptr i8, ptr %15, i16 -4
  %108 = load i16, ptr %107
  br label %b24

b24:
  %109 = phi i16 [ 0, %b23 ], [ %114, %b26 ]
  %110 = icmp ult i16 %109, %108
  br i1 %110, label %b26, label %b25

b25:
  br label %b22

b26:
  %111 = mul i16 %109, 6
  %112 = getelementptr inbounds i8, ptr %15, i16 %111
  %113 = load ptr, ptr %112
  call addrspace(1) void @N$BDRP(ptr %113)
  %114 = add i16 %109, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %105)
  ret i16 0

b28:
  %115 = getelementptr i8, ptr %105, i16 -4
  %116 = load i16, ptr %115
  br label %b29

b29:
  %117 = phi i16 [ 0, %b28 ], [ %122, %b31 ]
  %118 = icmp ult i16 %117, %116
  br i1 %118, label %b31, label %b30

b30:
  br label %b27

b31:
  %119 = mul i16 %117, 6
  %120 = getelementptr inbounds i8, ptr %105, i16 %119
  %121 = load ptr, ptr %120
  call addrspace(1) void @N$BDRP(ptr %121)
  %122 = add i16 %117, 1
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
