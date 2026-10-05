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

declare internal void @Catalog.add(ptr addrspace(1) nonnull dereferenceable(2) noalias nocapture, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture, i16, i16) addrspace(1) nearcode

declare internal void @north(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @south(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @find(ptr addrspace(1) nocapture, ptr, ptr, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias) addrspace(1) nearcode memory(argmem: read) willreturn norecurse

declare internal void @affordable(ptr addrspace(1) nocapture, ptr, i16) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal void @initial(ptr addrspace(1) nocapture, ptr) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare i16 @main() addrspace(1) nearcode memory(readwrite, argmem: none)

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

define internal i16 @pipeline.body() nearcode memory(readwrite, argmem: none) {
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
  %12 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @north(ptr addrspace(1) %12)
  %13 = load ptr, ptr %10, !tbaa !2
  store ptr %13, ptr %11, !tbaa !2
  %14 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %14)
  %15 = load ptr, ptr %9, !tbaa !2
  %16 = addrspacecast ptr %8 to ptr addrspace(1)
  %17 = addrspacecast ptr %11 to ptr addrspace(5)
  %18 = addrspacecast ptr %11 to ptr addrspace(1)
  %19 = getelementptr i8, ptr @$str5, i16 6
  %20 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %20, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %7 to ptr addrspace(1)
  %24 = load ptr, ptr addrspace(5) %17
  call addrspace(1) void @find(ptr addrspace(1) %16, ptr %24, ptr %15, ptr addrspace(1) %23)
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
  %33 = load ptr, ptr addrspace(5) %17
  call addrspace(1) void @find(ptr addrspace(1) %27, ptr %33, ptr %15, ptr addrspace(1) %32)
  %34 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %35, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %29, ptr %36, !tbaa !2
  %37 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %34, ptr %15, ptr %33, ptr addrspace(1) %37)
  %38 = load i8, ptr %6
  %39 = getelementptr i8, ptr %6, i16 2
  %40 = load ptr addrspace(1), ptr %39
  %41 = load i8, ptr %4
  %42 = getelementptr i8, ptr %4, i16 2
  %43 = load ptr addrspace(1), ptr %42
  %44 = icmp eq i8 %38, 0
  br i1 %44, label %b8, label %b7

b3:
  %45 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %45)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %46 = getelementptr inbounds i8, ptr %8, i16 2
  %47 = load ptr addrspace(1), ptr %46, !tbaa !2
  %48 = load ptr, ptr addrspace(1) %47
  call addrspace(1) void @N$PS(ptr %48)
  %49 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %49)
  %50 = getelementptr i8, ptr addrspace(1) %47, i16 4
  %51 = load i16, ptr addrspace(1) %50
  call addrspace(1) void @N$PU2(i16 %51)
  %52 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %52)
  %53 = getelementptr i8, ptr addrspace(1) %47, i16 2
  %54 = load i16, ptr addrspace(1) %53
  call addrspace(1) void @N$PU2(i16 %54)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %55 = addrspacecast ptr %2 to ptr addrspace(5)
  %56 = addrspacecast ptr %2 to ptr addrspace(1)
  %57 = load ptr, ptr addrspace(5) %17
  call addrspace(1) void @affordable(ptr addrspace(1) %56, ptr %57, i16 20)
  %58 = load i16, ptr addrspace(5) %55
  %59 = addrspacecast ptr %1 to ptr addrspace(1)
  %60 = icmp ne i16 %58, 0
  br i1 %60, label %b11, label %b12

b7:
  %61 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %61)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %62 = icmp eq i8 %41, 0
  br i1 %62, label %63, label %b7

63:
  %64 = getelementptr i8, ptr addrspace(1) %40, i16 2
  %65 = load i16, ptr addrspace(1) %64
  %66 = getelementptr i8, ptr addrspace(1) %43, i16 2
  %67 = load i16, ptr addrspace(1) %66
  %68 = icmp ule i16 %65, %67
  br i1 %68, label %69, label %70

69:
  br label %71

70:
  br label %71

71:
  %72 = phi ptr addrspace(1) [ %64, %69 ], [ %66, %70 ]
  %73 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %73)
  %74 = load i16, ptr addrspace(1) %72
  call addrspace(1) void @N$PU2(i16 %74)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %75 = getelementptr i8, ptr addrspace(5) %55, i16 4
  %76 = load ptr addrspace(1), ptr addrspace(5) %75, !tbaa !2
  %77 = getelementptr inbounds i8, ptr addrspace(1) %76, i16 0
  %78 = load ptr, ptr addrspace(1) %77
  call addrspace(1) void @initial(ptr addrspace(1) %59, ptr %78)
  call addrspace(1) void @N$PU2(i16 %58)
  %79 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %79)
  call addrspace(1) void @N$PV(ptr addrspace(1) %59)
  call addrspace(1) void @N$PN()
  %80 = load ptr, ptr %11, !tbaa !2
  %81 = getelementptr i8, ptr %80, i16 -4
  %82 = load i16, ptr %81
  %83 = getelementptr i8, ptr @$str12, i16 6
  %84 = getelementptr i8, ptr @$str13, i16 6
  %85 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %86 = phi i16 [ 0, %b11 ], [ %93, %b16 ]
  %87 = icmp ult i16 %86, %82
  br i1 %87, label %b15, label %b17

b15:
  %88 = mul i16 %86, 6
  %89 = getelementptr inbounds i8, ptr %80, i16 %88
  %90 = getelementptr i8, ptr %89, i16 4
  %91 = load i16, ptr %90
  %92 = icmp ult i16 %91, 5
  br i1 %92, label %b18, label %b16

b16:
  %93 = add i16 %86, 1
  br label %b14

b17:
  %94 = getelementptr i8, ptr @$str15, i16 6
  %95 = addrspacecast ptr %94 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %96 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %96, !tbaa !2
  %97 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %95, ptr %97, !tbaa !2
  %98 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %18, ptr addrspace(1) %98, i16 1, i16 500)
  %99 = load ptr, ptr %11, !tbaa !2
  %100 = getelementptr i8, ptr %99, i16 -4
  %101 = load i16, ptr %100
  call addrspace(1) void @N$PU2(i16 %101)
  %102 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %102)
  call addrspace(1) void @N$PN()
  %103 = icmp ne ptr %15, null
  br i1 %103, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %83)
  %104 = load ptr, ptr %89
  call addrspace(1) void @N$PS(ptr %104)
  call addrspace(1) void @N$PS(ptr %84)
  %105 = load i16, ptr %90
  call addrspace(1) void @N$PU2(i16 %105)
  call addrspace(1) void @N$PS(ptr %85)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %15)
  %106 = load ptr, ptr %11, !tbaa !2
  %107 = icmp ne ptr %106, null
  br i1 %107, label %b28, label %b27

b23:
  %108 = getelementptr i8, ptr %15, i16 -4
  %109 = load i16, ptr %108
  br label %b24

b24:
  %110 = phi i16 [ 0, %b23 ], [ %115, %b26 ]
  %111 = icmp ult i16 %110, %109
  br i1 %111, label %b26, label %b25

b25:
  br label %b22

b26:
  %112 = mul i16 %110, 6
  %113 = getelementptr inbounds i8, ptr %15, i16 %112
  %114 = load ptr, ptr %113
  call addrspace(1) void @N$BDRP(ptr %114)
  %115 = add i16 %110, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %106)
  ret i16 0

b28:
  %116 = getelementptr i8, ptr %106, i16 -4
  %117 = load i16, ptr %116
  br label %b29

b29:
  %118 = phi i16 [ 0, %b28 ], [ %123, %b31 ]
  %119 = icmp ult i16 %118, %117
  br i1 %119, label %b31, label %b30

b30:
  br label %b27

b31:
  %120 = mul i16 %118, 6
  %121 = getelementptr inbounds i8, ptr %106, i16 %120
  %122 = load ptr, ptr %121
  call addrspace(1) void @N$BDRP(ptr %122)
  %123 = add i16 %118, 1
  br label %b29
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
