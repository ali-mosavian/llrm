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

declare internal void @find(ptr addrspace(1) nocapture, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias) addrspace(1) nearcode memory(argmem: read) willreturn norecurse

declare internal void @affordable(ptr addrspace(1) nocapture, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture, i16) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal void @initial(ptr addrspace(1) nocapture, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias nocapture) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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
  %3 = alloca [2 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [6 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [6 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca [6 x i8]
  %13 = alloca [2 x i8]
  %14 = alloca [2 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = addrspacecast ptr %15 to ptr addrspace(1)
  %18 = getelementptr i8, ptr @$str1, i16 6
  store ptr %18, ptr %3, !tbaa !2
  %19 = addrspacecast ptr %3 to ptr addrspace(1)
  %20 = getelementptr i8, ptr @$str2, i16 6
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %24, i16 5, i16 40)
  %25 = getelementptr i8, ptr @$str3, i16 6
  %26 = addrspacecast ptr %25 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %29, i16 30, i16 3)
  %30 = getelementptr i8, ptr @$str4, i16 6
  %31 = addrspacecast ptr %30 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %32, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %31, ptr %33, !tbaa !2
  %34 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %34, i16 12, i16 0)
  %35 = load ptr, ptr %3, !tbaa !2
  store ptr %35, ptr addrspace(1) %17
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  %36 = load ptr, ptr %15, !tbaa !2
  store ptr %36, ptr %16, !tbaa !2
  %37 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %37)
  %38 = load ptr, ptr %13, !tbaa !2
  store ptr %38, ptr %14, !tbaa !2
  %39 = addrspacecast ptr %12 to ptr addrspace(1)
  %40 = addrspacecast ptr %16 to ptr addrspace(1)
  %41 = addrspacecast ptr %14 to ptr addrspace(1)
  %42 = getelementptr i8, ptr @$str5, i16 6
  %43 = addrspacecast ptr %42 to ptr addrspace(1)
  store i16 3, ptr %11, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 3, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %43, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %39, ptr addrspace(1) %40, ptr addrspace(1) %41, ptr addrspace(1) %46)
  %47 = load i8, ptr %12, !tbaa !2, !range !7
  %48 = icmp eq i8 %47, 0
  br i1 %48, label %b4, label %b3

b2:
  %49 = addrspacecast ptr %10 to ptr addrspace(1)
  %50 = getelementptr i8, ptr @$str3, i16 6
  %51 = addrspacecast ptr %50 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %51, ptr %53, !tbaa !2
  %54 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %49, ptr addrspace(1) %40, ptr addrspace(1) %41, ptr addrspace(1) %54)
  %55 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %7, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 4, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %51, ptr %57, !tbaa !2
  %58 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %55, ptr addrspace(1) %41, ptr addrspace(1) %40, ptr addrspace(1) %58)
  %59 = load i8, ptr %10
  %60 = getelementptr i8, ptr %10, i16 2
  %61 = load ptr addrspace(1), ptr %60
  %62 = load i8, ptr %8
  %63 = getelementptr i8, ptr %8, i16 2
  %64 = load ptr addrspace(1), ptr %63
  %65 = icmp eq i8 %59, 0
  br i1 %65, label %b8, label %b7

b3:
  %66 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %66)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %67 = getelementptr inbounds i8, ptr %12, i16 2
  %68 = load ptr addrspace(1), ptr %67, !tbaa !2
  %69 = load ptr, ptr addrspace(1) %68
  call addrspace(1) void @N$PS(ptr %69)
  %70 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %70)
  %71 = getelementptr i8, ptr addrspace(1) %68, i16 4
  %72 = load i16, ptr addrspace(1) %71
  call addrspace(1) void @N$PU2(i16 %72)
  %73 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %73)
  %74 = getelementptr i8, ptr addrspace(1) %68, i16 2
  %75 = load i16, ptr addrspace(1) %74
  call addrspace(1) void @N$PU2(i16 %75)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %76 = addrspacecast ptr %6 to ptr addrspace(5)
  %77 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %77, ptr addrspace(1) %40, i16 20)
  %78 = load i16, ptr addrspace(5) %76
  %79 = addrspacecast ptr %5 to ptr addrspace(1)
  %80 = icmp ne i16 %78, 0
  br i1 %80, label %b11, label %b12

b7:
  %81 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %81)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %82 = icmp eq i8 %62, 0
  br i1 %82, label %83, label %b7

83:
  %84 = getelementptr i8, ptr addrspace(1) %61, i16 2
  %85 = load i16, ptr addrspace(1) %84
  %86 = getelementptr i8, ptr addrspace(1) %64, i16 2
  %87 = load i16, ptr addrspace(1) %86
  %88 = icmp ule i16 %85, %87
  br i1 %88, label %89, label %90

89:
  br label %91

90:
  br label %91

91:
  %92 = phi ptr addrspace(1) [ %84, %89 ], [ %86, %90 ]
  %93 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %93)
  %94 = load i16, ptr addrspace(1) %92
  call addrspace(1) void @N$PU2(i16 %94)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %95 = getelementptr i8, ptr addrspace(5) %76, i16 4
  %96 = load ptr addrspace(1), ptr addrspace(5) %95, !tbaa !2
  %97 = getelementptr inbounds i8, ptr addrspace(1) %96, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %79, ptr addrspace(1) %97)
  call addrspace(1) void @N$PU2(i16 %78)
  %98 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %98)
  call addrspace(1) void @N$PV(ptr addrspace(1) %79)
  call addrspace(1) void @N$PN()
  %99 = load ptr, ptr %16, !tbaa !2
  %100 = getelementptr i8, ptr %99, i16 -4
  %101 = load i16, ptr %100
  %102 = getelementptr i8, ptr @$str12, i16 6
  %103 = getelementptr i8, ptr @$str13, i16 6
  %104 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %105 = phi i16 [ 0, %b11 ], [ %112, %b16 ]
  %106 = icmp ult i16 %105, %101
  br i1 %106, label %b15, label %b17

b15:
  %107 = mul i16 %105, 6
  %108 = getelementptr inbounds i8, ptr %99, i16 %107
  %109 = getelementptr i8, ptr %108, i16 4
  %110 = load i16, ptr %109
  %111 = icmp ult i16 %110, 5
  br i1 %111, label %b18, label %b16

b16:
  %112 = add i16 %105, 1
  br label %b14

b17:
  %113 = getelementptr i8, ptr @$str15, i16 6
  %114 = addrspacecast ptr %113 to ptr addrspace(1)
  store i16 3, ptr %4, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 3, ptr %115, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %114, ptr %116, !tbaa !2
  %117 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %40, ptr addrspace(1) %117, i16 1, i16 500)
  %118 = load ptr, ptr %16, !tbaa !2
  %119 = getelementptr i8, ptr %118, i16 -4
  %120 = load i16, ptr %119
  call addrspace(1) void @N$PU2(i16 %120)
  %121 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %121)
  call addrspace(1) void @N$PN()
  %122 = load ptr, ptr %14, !tbaa !2
  %123 = icmp ne ptr %122, null
  br i1 %123, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %102)
  %124 = load ptr, ptr %108
  call addrspace(1) void @N$PS(ptr %124)
  call addrspace(1) void @N$PS(ptr %103)
  %125 = load i16, ptr %109
  call addrspace(1) void @N$PU2(i16 %125)
  call addrspace(1) void @N$PS(ptr %104)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %122)
  %126 = load ptr, ptr %16, !tbaa !2
  %127 = icmp ne ptr %126, null
  br i1 %127, label %b28, label %b27

b23:
  %128 = getelementptr i8, ptr %122, i16 -4
  %129 = load i16, ptr %128
  br label %b24

b24:
  %130 = phi i16 [ 0, %b23 ], [ %135, %b26 ]
  %131 = icmp ult i16 %130, %129
  br i1 %131, label %b26, label %b25

b25:
  br label %b22

b26:
  %132 = mul i16 %130, 6
  %133 = getelementptr inbounds i8, ptr %122, i16 %132
  %134 = load ptr, ptr %133
  call addrspace(1) void @N$BDRP(ptr %134)
  %135 = add i16 %130, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %126)
  ret i16 0

b28:
  %136 = getelementptr i8, ptr %126, i16 -4
  %137 = load i16, ptr %136
  br label %b29

b29:
  %138 = phi i16 [ 0, %b28 ], [ %143, %b31 ]
  %139 = icmp ult i16 %138, %137
  br i1 %139, label %b31, label %b30

b30:
  br label %b27

b31:
  %140 = mul i16 %138, 6
  %141 = getelementptr inbounds i8, ptr %126, i16 %140
  %142 = load ptr, ptr %141
  call addrspace(1) void @N$BDRP(ptr %142)
  %143 = add i16 %138, 1
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
