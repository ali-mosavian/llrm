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

define internal fastcc void @Catalog.add(ptr addrspace(5) nocapture %0, ptr addrspace(5) %1, i16 range(i16 1, 31) %2, i16 range(i16 0, 501) %3) addrspace(1) nearcode {
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

define internal fastcc void @north(ptr addrspace(5) nocapture %0) addrspace(1) nearcode memory(readwrite, argmem: write) {
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
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %11, i16 5, i16 40)
  %12 = getelementptr i8, ptr @$str3, i16 6
  %13 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %2 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %16, i16 30, i16 3)
  %17 = getelementptr i8, ptr @$str4, i16 6
  %18 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %21, i16 12, i16 0)
  %22 = load ptr, ptr %4, !tbaa !2
  store ptr %22, ptr addrspace(5) %0
  store ptr null, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal fastcc void @south(ptr addrspace(5) nocapture %0) addrspace(1) nearcode memory(readwrite, argmem: write) {
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
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %5, ptr addrspace(5) %10, i16 28, i16 9)
  %11 = getelementptr i8, ptr @$str5, i16 6
  %12 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %1 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %5, ptr addrspace(5) %15, i16 2, i16 100)
  %16 = load ptr, ptr %3, !tbaa !2
  store ptr %16, ptr addrspace(5) %0
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal fastcc void @find(ptr addrspace(5) nocapture %0, ptr %1, ptr %2, ptr addrspace(5) %3) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
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
  %lsr.iv = phi ptr [ %1, %b1 ], [ %lsr.iv.next, %b4 ]
  %12 = phi i16 [ 0, %b1 ], [ %20, %b4 ]
  %13 = icmp ult i16 %12, %8
  br i1 %13, label %b3, label %b5

b3:
  %14 = load ptr, ptr %lsr.iv
  %15 = getelementptr i8, ptr %14, i16 -4
  %16 = load i16, ptr %15
  %17 = addrspacecast ptr %14 to ptr addrspace(1)
  store i16 %16, ptr %6, !tbaa !2
  store i16 %16, ptr %9, !tbaa !2
  store ptr addrspace(1) %17, ptr %10, !tbaa !2
  %18 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %11, ptr addrspace(1) %4)
  %19 = icmp eq i8 %18, 0
  br i1 %19, label %b8, label %b4

b4:
  %20 = add nuw i16 %12, 1
  %lsr.iv.next = getelementptr i8, ptr %lsr.iv, i16 6
  br label %b2

b5:
  %21 = getelementptr i8, ptr %2, i16 -4
  %22 = load i16, ptr %21
  %23 = getelementptr inbounds i8, ptr %5, i16 2
  %24 = getelementptr inbounds i8, ptr %5, i16 4
  %25 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b13

b8:
  %26 = phi i16 [ %12, %b3 ]
  %27 = icmp ult i16 %26, %8
  br i1 %27, label %b11, label %b12

b11:
  %28 = mul i16 %26, 6
  %29 = getelementptr inbounds i8, ptr %1, i16 %28
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %31 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %30, ptr addrspace(5) %31
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %lsr.iv1 = phi ptr [ %2, %b5 ], [ %lsr.iv.next1, %b15 ]
  %32 = phi i16 [ 0, %b5 ], [ %40, %b15 ]
  %33 = icmp ult i16 %32, %22
  br i1 %33, label %b14, label %b16

b14:
  %34 = load ptr, ptr %lsr.iv1
  %35 = getelementptr i8, ptr %34, i16 -4
  %36 = load i16, ptr %35
  %37 = addrspacecast ptr %34 to ptr addrspace(1)
  store i16 %36, ptr %5, !tbaa !2
  store i16 %36, ptr %23, !tbaa !2
  store ptr addrspace(1) %37, ptr %24, !tbaa !2
  %38 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %25, ptr addrspace(1) %4)
  %39 = icmp eq i8 %38, 0
  br i1 %39, label %b19, label %b15

b15:
  %40 = add nuw i16 %32, 1
  %lsr.iv.next1 = getelementptr i8, ptr %lsr.iv1, i16 6
  br label %b13

b16:
  store i8 1, ptr addrspace(5) %0
  ret void

b19:
  %41 = phi i16 [ %32, %b14 ]
  %42 = icmp ult i16 %41, %22
  br i1 %42, label %b22, label %b23

b22:
  %43 = mul i16 %41, 6
  %44 = getelementptr inbounds i8, ptr %2, i16 %43
  %45 = addrspacecast ptr %44 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %46 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %45, ptr addrspace(5) %46
  ret void

b23:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal fastcc void @affordable(ptr addrspace(5) nocapture writeonly %0, ptr %1) addrspace(1) nearcode memory(argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %2 = getelementptr i8, ptr %1, i16 -4
  %3 = load i16, ptr %2
  br label %b2

b2:
  %lsr.iv = phi i16 [ 0, %b1 ], [ %lsr.iv.next, %b3 ]
  %4 = phi i16 [ 0, %b1 ], [ %6, %b3 ]
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b5, label %16

b3:
  %6 = add i16 %4, 1
  %lsr.iv.next = add i16 %lsr.iv, 6
  br label %b2

b4:
  %7 = phi i16 [ %17, %16 ], [ %19, %18 ]
  %8 = addrspacecast ptr %1 to ptr addrspace(1)
  %9 = icmp ule i16 %7, %3
  br i1 %9, label %b9, label %b10

b5:
  %10 = getelementptr i8, ptr %1, i16 %lsr.iv
  %11 = getelementptr i8, ptr %10, i16 2
  %12 = load i16, ptr %11
  %13 = icmp ule i16 %12, 20
  br i1 %13, label %b3, label %18

b9:
  store i16 %7, ptr addrspace(5) %0
  %14 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store i16 %7, ptr addrspace(5) %14
  %15 = getelementptr i8, ptr addrspace(5) %0, i16 4
  store ptr addrspace(1) %8, ptr addrspace(5) %15
  ret void

b10:
  call addrspace(1) void @N$EBND()
  unreachable

16:
  %17 = phi i16 [ %4, %b2 ]
  br label %b4

18:
  %19 = phi i16 [ %4, %b5 ]
  br label %b4
}

define internal fastcc void @initial(ptr addrspace(5) nocapture writeonly %0, ptr %1) addrspace(1) nearcode memory(argmem: readwrite, inaccessiblemem: readwrite) {
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
  call fastcc addrspace(1) void @north(ptr addrspace(5) %12)
  %13 = load ptr, ptr %10, !tbaa !2
  store ptr %13, ptr %11, !tbaa !2
  %14 = addrspacecast ptr %9 to ptr addrspace(5)
  call fastcc addrspace(1) void @south(ptr addrspace(5) %14)
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
  call fastcc addrspace(1) void @find(ptr addrspace(5) %16, ptr %23, ptr %15, ptr addrspace(5) %22)
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
  call fastcc addrspace(1) void @find(ptr addrspace(5) %26, ptr %32, ptr %15, ptr addrspace(5) %31)
  %33 = addrspacecast ptr %4 to ptr addrspace(5)
  store i16 4, ptr %3, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %28, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %3 to ptr addrspace(5)
  call fastcc addrspace(1) void @find(ptr addrspace(5) %33, ptr %15, ptr %32, ptr addrspace(5) %36)
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
  call fastcc addrspace(1) void @affordable(ptr addrspace(5) %54, ptr %55)
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
  call fastcc addrspace(1) void @initial(ptr addrspace(5) %57, ptr %77)
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
  %85 = sub i16 0, %81
  %86 = icmp ule i16 %81, 0
  br i1 %86, label %b17, label %128

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b15:
  %lsr.iv31 = phi ptr [ %lsr.iv.next2, %b16 ], [ %79, %128 ]
  %lsr.iv41 = phi i16 [ %lsr.iv.next3, %b16 ], [ %85, %128 ]
  %87 = getelementptr i8, ptr %lsr.iv31, i16 4
  %88 = load i16, ptr %87
  %89 = icmp ult i16 %88, 5
  br i1 %89, label %b18, label %b16

b16:
  %lsr.iv.next2 = getelementptr i8, ptr %lsr.iv31, i16 6
  %lsr.iv.next3 = add i16 %lsr.iv41, 1
  %90 = icmp ne i16 %lsr.iv.next3, 0
  br i1 %90, label %b15, label %129

b17:
  %91 = getelementptr i8, ptr @$str15, i16 6
  %92 = addrspacecast ptr %91 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %93 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %93, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %92, ptr %94, !tbaa !2
  %95 = addrspacecast ptr %0 to ptr addrspace(5)
  call fastcc addrspace(1) void @Catalog.add(ptr addrspace(5) %17, ptr addrspace(5) %95, i16 1, i16 500)
  %96 = load ptr, ptr %11, !tbaa !2
  %97 = getelementptr i8, ptr %96, i16 -4
  %98 = load i16, ptr %97
  call addrspace(1) void @N$PU2(i16 %98)
  %99 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %99)
  call addrspace(1) void @N$PN()
  %100 = icmp ne ptr %15, null
  br i1 %100, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %82)
  %101 = load ptr, ptr %lsr.iv31
  call addrspace(1) void @N$PS(ptr %101)
  call addrspace(1) void @N$PS(ptr %83)
  %102 = getelementptr i8, ptr %lsr.iv31, i16 4
  %103 = load i16, ptr %102
  call addrspace(1) void @N$PU2(i16 %103)
  call addrspace(1) void @N$PS(ptr %84)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %15)
  %104 = load ptr, ptr %11, !tbaa !2
  %105 = icmp ne ptr %104, null
  br i1 %105, label %b28, label %b27

b23:
  %106 = getelementptr i8, ptr %15, i16 -4
  %107 = load i16, ptr %106
  %108 = mul i16 %107, 6
  %109 = sub i16 0, %108
  %110 = getelementptr i8, ptr %15, i16 %108
  %111 = icmp ule i16 %107, 0
  br i1 %111, label %b25, label %124

b25:
  br label %b22

b26:
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b26 ], [ %109, %124 ]
  %112 = getelementptr i8, ptr %110, i16 %lsr.iv1
  %113 = load ptr, ptr %112
  call addrspace(1) void @N$BDRP(ptr %113)
  %lsr.iv.next = add i16 %lsr.iv1, 6
  %114 = icmp ne i16 %lsr.iv.next, 0
  br i1 %114, label %b26, label %125

b27:
  call addrspace(1) void @N$BDRP(ptr %104)
  ret i16 0

b28:
  %115 = getelementptr i8, ptr %104, i16 -4
  %116 = load i16, ptr %115
  %117 = mul i16 %116, 6
  %118 = sub i16 0, %117
  %119 = getelementptr i8, ptr %104, i16 %117
  %120 = icmp ule i16 %116, 0
  br i1 %120, label %b30, label %126

b30:
  br label %b27

b31:
  %lsr.iv21 = phi i16 [ %lsr.iv.next1, %b31 ], [ %118, %126 ]
  %121 = getelementptr i8, ptr %119, i16 %lsr.iv21
  %122 = load ptr, ptr %121
  call addrspace(1) void @N$BDRP(ptr %122)
  %lsr.iv.next1 = add i16 %lsr.iv21, 6
  %123 = icmp ne i16 %lsr.iv.next1, 0
  br i1 %123, label %b31, label %127

124:
  br label %b26

125:
  br label %b25

126:
  br label %b31

127:
  br label %b30

128:
  br label %b15

129:
  br label %b17
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
