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
  %16 = getelementptr i8, ptr @$str1, i16 6
  store ptr %16, ptr %3, !tbaa !2
  %17 = addrspacecast ptr %3 to ptr addrspace(1)
  %18 = getelementptr i8, ptr @$str2, i16 6
  %19 = addrspacecast ptr %18 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %20, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %19, ptr %21, !tbaa !2
  %22 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %17, ptr addrspace(1) %22, i16 5, i16 40)
  %23 = getelementptr i8, ptr @$str3, i16 6
  %24 = addrspacecast ptr %23 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %24, ptr %26, !tbaa !2
  %27 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %17, ptr addrspace(1) %27, i16 30, i16 3)
  %28 = getelementptr i8, ptr @$str4, i16 6
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %17, ptr addrspace(1) %32, i16 12, i16 0)
  %33 = load ptr, ptr %3, !tbaa !2
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %33, ptr %15, !tbaa !2
  %34 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %34)
  %35 = load ptr, ptr %13, !tbaa !2
  store ptr %35, ptr %14, !tbaa !2
  %36 = addrspacecast ptr %12 to ptr addrspace(1)
  %37 = addrspacecast ptr %15 to ptr addrspace(1)
  %38 = addrspacecast ptr %14 to ptr addrspace(1)
  %39 = getelementptr i8, ptr @$str5, i16 6
  %40 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 3, ptr %11, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 3, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %36, ptr addrspace(1) %37, ptr addrspace(1) %38, ptr addrspace(1) %43)
  %44 = load i8, ptr %12, !tbaa !2, !range !7
  %45 = icmp eq i8 %44, 0
  br i1 %45, label %b4, label %b3

b2:
  %46 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %24, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %46, ptr addrspace(1) %37, ptr addrspace(1) %38, ptr addrspace(1) %49)
  %50 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %7, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 4, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %24, ptr %52, !tbaa !2
  %53 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %50, ptr addrspace(1) %38, ptr addrspace(1) %37, ptr addrspace(1) %53)
  %54 = load i8, ptr %10
  %55 = getelementptr i8, ptr %10, i16 2
  %56 = load ptr addrspace(1), ptr %55
  %57 = load i8, ptr %8
  %58 = getelementptr i8, ptr %8, i16 2
  %59 = load ptr addrspace(1), ptr %58
  %60 = icmp eq i8 %54, 0
  br i1 %60, label %b8, label %b7

b3:
  %61 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %61)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %62 = getelementptr inbounds i8, ptr %12, i16 2
  %63 = load ptr addrspace(1), ptr %62, !tbaa !2
  %64 = load ptr, ptr addrspace(1) %63
  call addrspace(1) void @N$PS(ptr %64)
  %65 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %65)
  %66 = getelementptr i8, ptr addrspace(1) %63, i16 4
  %67 = load i16, ptr addrspace(1) %66
  call addrspace(1) void @N$PU2(i16 %67)
  %68 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  %69 = getelementptr i8, ptr addrspace(1) %63, i16 2
  %70 = load i16, ptr addrspace(1) %69
  call addrspace(1) void @N$PU2(i16 %70)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %71 = addrspacecast ptr %6 to ptr addrspace(5)
  %72 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %72, ptr addrspace(1) %37, i16 20)
  %73 = load i16, ptr addrspace(5) %71
  %74 = addrspacecast ptr %5 to ptr addrspace(1)
  %75 = icmp ne i16 %73, 0
  br i1 %75, label %b11, label %b12

b7:
  %76 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %77 = icmp eq i8 %57, 0
  br i1 %77, label %78, label %b7

78:
  %79 = getelementptr i8, ptr addrspace(1) %56, i16 2
  %80 = load i16, ptr addrspace(1) %79
  %81 = getelementptr i8, ptr addrspace(1) %59, i16 2
  %82 = load i16, ptr addrspace(1) %81
  %83 = icmp ule i16 %80, %82
  br i1 %83, label %84, label %85

84:
  br label %86

85:
  br label %86

86:
  %87 = phi ptr addrspace(1) [ %79, %84 ], [ %81, %85 ]
  %88 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %88)
  %89 = load i16, ptr addrspace(1) %87
  call addrspace(1) void @N$PU2(i16 %89)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %90 = getelementptr i8, ptr addrspace(5) %71, i16 4
  %91 = load ptr addrspace(1), ptr addrspace(5) %90, !tbaa !2
  %92 = getelementptr inbounds i8, ptr addrspace(1) %91, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %74, ptr addrspace(1) %92)
  call addrspace(1) void @N$PU2(i16 %73)
  %93 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %93)
  call addrspace(1) void @N$PV(ptr addrspace(1) %74)
  call addrspace(1) void @N$PN()
  %94 = load ptr, ptr %15, !tbaa !2
  %95 = getelementptr i8, ptr %94, i16 -4
  %96 = load i16, ptr %95
  %97 = getelementptr i8, ptr @$str12, i16 6
  %98 = getelementptr i8, ptr @$str13, i16 6
  %99 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %100 = phi i16 [ 0, %b11 ], [ %107, %b16 ]
  %101 = icmp ult i16 %100, %96
  br i1 %101, label %b15, label %b17

b15:
  %102 = mul i16 %100, 6
  %103 = getelementptr inbounds i8, ptr %94, i16 %102
  %104 = getelementptr i8, ptr %103, i16 4
  %105 = load i16, ptr %104
  %106 = icmp ult i16 %105, 5
  br i1 %106, label %b18, label %b16

b16:
  %107 = add i16 %100, 1
  br label %b14

b17:
  %108 = getelementptr i8, ptr @$str15, i16 6
  %109 = addrspacecast ptr %108 to ptr addrspace(1)
  store i16 3, ptr %4, !tbaa !2
  %110 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 3, ptr %110, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %109, ptr %111, !tbaa !2
  %112 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %37, ptr addrspace(1) %112, i16 1, i16 500)
  %113 = load ptr, ptr %15, !tbaa !2
  %114 = getelementptr i8, ptr %113, i16 -4
  %115 = load i16, ptr %114
  call addrspace(1) void @N$PU2(i16 %115)
  %116 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %116)
  call addrspace(1) void @N$PN()
  %117 = load ptr, ptr %14, !tbaa !2
  %118 = icmp ne ptr %117, null
  br i1 %118, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %97)
  %119 = load ptr, ptr %103
  call addrspace(1) void @N$PS(ptr %119)
  call addrspace(1) void @N$PS(ptr %98)
  %120 = load i16, ptr %104
  call addrspace(1) void @N$PU2(i16 %120)
  call addrspace(1) void @N$PS(ptr %99)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %117)
  %121 = load ptr, ptr %15, !tbaa !2
  %122 = icmp ne ptr %121, null
  br i1 %122, label %b28, label %b27

b23:
  %123 = getelementptr i8, ptr %117, i16 -4
  %124 = load i16, ptr %123
  br label %b24

b24:
  %125 = phi i16 [ 0, %b23 ], [ %130, %b26 ]
  %126 = icmp ult i16 %125, %124
  br i1 %126, label %b26, label %b25

b25:
  br label %b22

b26:
  %127 = mul i16 %125, 6
  %128 = getelementptr inbounds i8, ptr %117, i16 %127
  %129 = load ptr, ptr %128
  call addrspace(1) void @N$BDRP(ptr %129)
  %130 = add i16 %125, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %121)
  ret i16 0

b28:
  %131 = getelementptr i8, ptr %121, i16 -4
  %132 = load i16, ptr %131
  br label %b29

b29:
  %133 = phi i16 [ 0, %b28 ], [ %138, %b31 ]
  %134 = icmp ult i16 %133, %132
  br i1 %134, label %b31, label %b30

b30:
  br label %b27

b31:
  %135 = mul i16 %133, 6
  %136 = getelementptr inbounds i8, ptr %121, i16 %135
  %137 = load ptr, ptr %136
  call addrspace(1) void @N$BDRP(ptr %137)
  %138 = add i16 %133, 1
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
