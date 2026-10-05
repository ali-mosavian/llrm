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

define internal void @north(ptr addrspace(1) nocapture %0) addrspace(1) nearcode memory(readwrite, argmem: write) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [2 x i8]
  %5 = getelementptr i8, ptr @$str1, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = addrspacecast ptr %4 to ptr addrspace(1)
  %7 = getelementptr i8, ptr @$str2, i16 6
  %8 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %9, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %8, ptr %10, !tbaa !2
  %11 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %6, ptr addrspace(1) %11, i16 5, i16 40)
  %12 = getelementptr i8, ptr @$str3, i16 6
  %13 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %14, !tbaa !2
  %15 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %13, ptr %15, !tbaa !2
  %16 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %6, ptr addrspace(1) %16, i16 30, i16 3)
  %17 = getelementptr i8, ptr @$str4, i16 6
  %18 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %6, ptr addrspace(1) %21, i16 12, i16 0)
  %22 = load ptr, ptr %4, !tbaa !2
  store ptr %22, ptr addrspace(1) %0
  store ptr null, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal void @south(ptr addrspace(1) nocapture %0) addrspace(1) nearcode memory(readwrite, argmem: write) {
b1:
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [2 x i8]
  %4 = getelementptr i8, ptr @$str1, i16 6
  store ptr %4, ptr %3, !tbaa !2
  %5 = addrspacecast ptr %3 to ptr addrspace(1)
  %6 = getelementptr i8, ptr @$str3, i16 6
  %7 = addrspacecast ptr %6 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %8 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %8, !tbaa !2
  %9 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %7, ptr %9, !tbaa !2
  %10 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %5, ptr addrspace(1) %10, i16 28, i16 9)
  %11 = getelementptr i8, ptr @$str5, i16 6
  %12 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %13 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %12, ptr %14, !tbaa !2
  %15 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %5, ptr addrspace(1) %15, i16 2, i16 100)
  %16 = load ptr, ptr %3, !tbaa !2
  store ptr %16, ptr addrspace(1) %0
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

define internal void @find(ptr addrspace(1) nocapture %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %1, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %2, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %3) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = load ptr, ptr addrspace(1) %1
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = getelementptr inbounds i8, ptr %5, i16 2
  %10 = getelementptr inbounds i8, ptr %5, i16 4
  %11 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b2

b2:
  %12 = phi i16 [ 0, %b1 ], [ %16, %b4 ]
  %13 = icmp ult i16 %12, %8
  br i1 %13, label %b3, label %b5

b3:
  %14 = load i16, ptr %7
  %15 = icmp ult i16 %12, %14
  br i1 %15, label %b6, label %b7

b4:
  %16 = add nuw i16 %12, 1
  br label %b2

b5:
  %17 = load ptr, ptr addrspace(1) %2
  %18 = getelementptr i8, ptr %17, i16 -4
  %19 = load i16, ptr %18
  %20 = getelementptr inbounds i8, ptr %4, i16 2
  %21 = getelementptr inbounds i8, ptr %4, i16 4
  %22 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %23 = mul i16 %12, 6
  %24 = getelementptr inbounds i8, ptr %6, i16 %23
  %25 = load ptr, ptr %24
  %26 = getelementptr i8, ptr %25, i16 -4
  %27 = load i16, ptr %26
  %28 = addrspacecast ptr %25 to ptr addrspace(1)
  store i16 %27, ptr %5, !tbaa !2
  store i16 %27, ptr %9, !tbaa !2
  store ptr addrspace(1) %28, ptr %10, !tbaa !2
  %29 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %11, ptr addrspace(1) %3)
  %30 = icmp eq i8 %29, 0
  br i1 %30, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %31 = phi i16 [ %12, %b6 ]
  %32 = load i16, ptr %7
  %33 = icmp ult i16 %31, %32
  br i1 %33, label %b11, label %b12

b11:
  %34 = mul i16 %31, 6
  %35 = getelementptr inbounds i8, ptr %6, i16 %34
  %36 = addrspacecast ptr %35 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %37 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %36, ptr addrspace(1) %37
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %38 = phi i16 [ 0, %b5 ], [ %42, %b15 ]
  %39 = icmp ult i16 %38, %19
  br i1 %39, label %b14, label %b16

b14:
  %40 = load i16, ptr %18
  %41 = icmp ult i16 %38, %40
  br i1 %41, label %b17, label %b18

b15:
  %42 = add nuw i16 %38, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %43 = mul i16 %38, 6
  %44 = getelementptr inbounds i8, ptr %17, i16 %43
  %45 = load ptr, ptr %44
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %4, !tbaa !2
  store i16 %47, ptr %20, !tbaa !2
  store ptr addrspace(1) %48, ptr %21, !tbaa !2
  %49 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %22, ptr addrspace(1) %3)
  %50 = icmp eq i8 %49, 0
  br i1 %50, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %51 = phi i16 [ %38, %b17 ]
  %52 = load i16, ptr %18
  %53 = icmp ult i16 %51, %52
  br i1 %53, label %b22, label %b23

b22:
  %54 = mul i16 %51, 6
  %55 = getelementptr inbounds i8, ptr %17, i16 %54
  %56 = addrspacecast ptr %55 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %56, ptr addrspace(1) %57
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

define internal void @affordable(ptr addrspace(1) nocapture %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %1, i16 %2) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %3 = load ptr, ptr addrspace(1) %1
  %4 = getelementptr i8, ptr %3, i16 -4
  %5 = load i16, ptr %4
  br label %b2

b2:
  %6 = phi i16 [ 0, %b1 ], [ %8, %b3 ]
  %7 = icmp ult i16 %6, %5
  br i1 %7, label %b5, label %19

b3:
  %8 = add i16 %6, 1
  br label %b2

b4:
  %9 = phi i16 [ %20, %19 ], [ %22, %21 ]
  %10 = addrspacecast ptr %3 to ptr addrspace(1)
  %11 = icmp ule i16 %9, %5
  br i1 %11, label %b9, label %b10

b5:
  %12 = mul i16 %6, 6
  %13 = getelementptr inbounds i8, ptr %3, i16 %12
  %14 = getelementptr i8, ptr %13, i16 2
  %15 = load i16, ptr %14
  %16 = icmp ule i16 %15, %2
  br i1 %16, label %b3, label %21

b9:
  store i16 %9, ptr addrspace(1) %0
  %17 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %9, ptr addrspace(1) %17
  %18 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %10, ptr addrspace(1) %18
  ret void

b10:
  call addrspace(1) void @N$EBND()
  unreachable

19:
  %20 = phi i16 [ %6, %b2 ]
  br label %b4

21:
  %22 = phi i16 [ %6, %b5 ]
  br label %b4
}

define internal void @initial(ptr addrspace(1) nocapture %0, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias nocapture %1) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %2 = load ptr, ptr addrspace(1) %1
  %3 = getelementptr i8, ptr %2, i16 -4
  %4 = load i16, ptr %3
  %5 = addrspacecast ptr %2 to ptr addrspace(1)
  %6 = icmp uge i16 %4, 1
  br i1 %6, label %b2, label %b3

b2:
  store i16 1, ptr addrspace(1) %0
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 1, ptr addrspace(1) %7
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %5, ptr addrspace(1) %8
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
  %12 = alloca [2 x i8]
  %13 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @north(ptr addrspace(1) %13)
  %14 = load ptr, ptr %11, !tbaa !2
  store ptr %14, ptr %12, !tbaa !2
  %15 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %15)
  %16 = load ptr, ptr %9, !tbaa !2
  store ptr %16, ptr %10, !tbaa !2
  %17 = addrspacecast ptr %8 to ptr addrspace(1)
  %18 = addrspacecast ptr %12 to ptr addrspace(1)
  %19 = addrspacecast ptr %10 to ptr addrspace(1)
  %20 = getelementptr i8, ptr @$str5, i16 6
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %17, ptr addrspace(1) %18, ptr addrspace(1) %19, ptr addrspace(1) %24)
  %25 = load i8, ptr %8, !tbaa !2, !range !7
  %26 = icmp eq i8 %25, 0
  br i1 %26, label %b4, label %b3

b2:
  %27 = addrspacecast ptr %6 to ptr addrspace(1)
  %28 = getelementptr i8, ptr @$str3, i16 6
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %27, ptr addrspace(1) %18, ptr addrspace(1) %19, ptr addrspace(1) %32)
  %33 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %29, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %33, ptr addrspace(1) %19, ptr addrspace(1) %18, ptr addrspace(1) %36)
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
  %55 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %55, ptr addrspace(1) %18, i16 20)
  %56 = load i16, ptr addrspace(5) %54
  %57 = addrspacecast ptr %1 to ptr addrspace(1)
  %58 = icmp ne i16 %56, 0
  br i1 %58, label %b11, label %b12

b7:
  %59 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %59)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %60 = icmp eq i8 %40, 0
  br i1 %60, label %b9, label %b7

b9:
  %61 = call addrspace(1) ptr addrspace(1) @cheaper(ptr addrspace(1) %39, ptr addrspace(1) %42)
  %62 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %62)
  %63 = getelementptr i8, ptr addrspace(1) %61, i16 2
  %64 = load i16, ptr addrspace(1) %63
  call addrspace(1) void @N$PU2(i16 %64)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %65 = getelementptr i8, ptr addrspace(5) %54, i16 4
  %66 = load ptr addrspace(1), ptr addrspace(5) %65, !tbaa !2
  %67 = getelementptr inbounds i8, ptr addrspace(1) %66, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %57, ptr addrspace(1) %67)
  call addrspace(1) void @N$PU2(i16 %56)
  %68 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  call addrspace(1) void @N$PV(ptr addrspace(1) %57)
  call addrspace(1) void @N$PN()
  %69 = load ptr, ptr %12, !tbaa !2
  %70 = getelementptr i8, ptr %69, i16 -4
  %71 = load i16, ptr %70
  %72 = getelementptr i8, ptr @$str12, i16 6
  %73 = getelementptr i8, ptr @$str13, i16 6
  %74 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %75 = phi i16 [ 0, %b11 ], [ %82, %b16 ]
  %76 = icmp ult i16 %75, %71
  br i1 %76, label %b15, label %b17

b15:
  %77 = mul i16 %75, 6
  %78 = getelementptr inbounds i8, ptr %69, i16 %77
  %79 = getelementptr i8, ptr %78, i16 4
  %80 = load i16, ptr %79
  %81 = icmp ult i16 %80, 5
  br i1 %81, label %b18, label %b16

b16:
  %82 = add i16 %75, 1
  br label %b14

b17:
  %83 = getelementptr i8, ptr @$str15, i16 6
  %84 = addrspacecast ptr %83 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %85, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %84, ptr %86, !tbaa !2
  %87 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %18, ptr addrspace(1) %87, i16 1, i16 500)
  %88 = load ptr, ptr %12, !tbaa !2
  %89 = getelementptr i8, ptr %88, i16 -4
  %90 = load i16, ptr %89
  call addrspace(1) void @N$PU2(i16 %90)
  %91 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %91)
  call addrspace(1) void @N$PN()
  %92 = load ptr, ptr %10, !tbaa !2
  %93 = icmp ne ptr %92, null
  br i1 %93, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %72)
  %94 = load ptr, ptr %78
  call addrspace(1) void @N$PS(ptr %94)
  call addrspace(1) void @N$PS(ptr %73)
  %95 = load i16, ptr %79
  call addrspace(1) void @N$PU2(i16 %95)
  call addrspace(1) void @N$PS(ptr %74)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %92)
  %96 = load ptr, ptr %12, !tbaa !2
  %97 = icmp ne ptr %96, null
  br i1 %97, label %b28, label %b27

b23:
  %98 = getelementptr i8, ptr %92, i16 -4
  %99 = load i16, ptr %98
  br label %b24

b24:
  %100 = phi i16 [ 0, %b23 ], [ %105, %b26 ]
  %101 = icmp ult i16 %100, %99
  br i1 %101, label %b26, label %b25

b25:
  br label %b22

b26:
  %102 = mul i16 %100, 6
  %103 = getelementptr inbounds i8, ptr %92, i16 %102
  %104 = load ptr, ptr %103
  call addrspace(1) void @N$BDRP(ptr %104)
  %105 = add i16 %100, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %96)
  ret i16 0

b28:
  %106 = getelementptr i8, ptr %96, i16 -4
  %107 = load i16, ptr %106
  br label %b29

b29:
  %108 = phi i16 [ 0, %b28 ], [ %113, %b31 ]
  %109 = icmp ult i16 %108, %107
  br i1 %109, label %b31, label %b30

b30:
  br label %b27

b31:
  %110 = mul i16 %108, 6
  %111 = getelementptr inbounds i8, ptr %96, i16 %110
  %112 = load ptr, ptr %111
  call addrspace(1) void @N$BDRP(ptr %112)
  %113 = add i16 %108, 1
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
