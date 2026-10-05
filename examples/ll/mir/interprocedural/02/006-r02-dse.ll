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
  %17 = addrspacecast ptr %15 to ptr addrspace(5)
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
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %35, ptr %16, !tbaa !2
  %36 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %36)
  %37 = load ptr, ptr %13, !tbaa !2
  store ptr %37, ptr %14, !tbaa !2
  %38 = addrspacecast ptr %12 to ptr addrspace(1)
  %39 = addrspacecast ptr %16 to ptr addrspace(1)
  %40 = addrspacecast ptr %14 to ptr addrspace(1)
  %41 = getelementptr i8, ptr @$str5, i16 6
  %42 = addrspacecast ptr %41 to ptr addrspace(1)
  store i16 3, ptr %11, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 3, ptr %43, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %42, ptr %44, !tbaa !2
  %45 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %38, ptr addrspace(1) %39, ptr addrspace(1) %40, ptr addrspace(1) %45)
  %46 = load i8, ptr %12, !tbaa !2, !range !7
  %47 = icmp eq i8 %46, 0
  br i1 %47, label %b4, label %b3

b2:
  %48 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %26, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %48, ptr addrspace(1) %39, ptr addrspace(1) %40, ptr addrspace(1) %51)
  %52 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %7, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 4, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %26, ptr %54, !tbaa !2
  %55 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %52, ptr addrspace(1) %40, ptr addrspace(1) %39, ptr addrspace(1) %55)
  %56 = load i8, ptr %10
  %57 = getelementptr i8, ptr %10, i16 2
  %58 = load ptr addrspace(1), ptr %57
  %59 = load i8, ptr %8
  %60 = getelementptr i8, ptr %8, i16 2
  %61 = load ptr addrspace(1), ptr %60
  %62 = icmp eq i8 %56, 0
  br i1 %62, label %b8, label %b7

b3:
  %63 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %63)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %64 = getelementptr inbounds i8, ptr %12, i16 2
  %65 = load ptr addrspace(1), ptr %64, !tbaa !2
  %66 = load ptr, ptr addrspace(1) %65
  call addrspace(1) void @N$PS(ptr %66)
  %67 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %67)
  %68 = getelementptr i8, ptr addrspace(1) %65, i16 4
  %69 = load i16, ptr addrspace(1) %68
  call addrspace(1) void @N$PU2(i16 %69)
  %70 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %70)
  %71 = getelementptr i8, ptr addrspace(1) %65, i16 2
  %72 = load i16, ptr addrspace(1) %71
  call addrspace(1) void @N$PU2(i16 %72)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %73 = addrspacecast ptr %6 to ptr addrspace(5)
  %74 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %74, ptr addrspace(1) %39, i16 20)
  %75 = load i16, ptr addrspace(5) %73
  %76 = addrspacecast ptr %5 to ptr addrspace(1)
  %77 = icmp ne i16 %75, 0
  br i1 %77, label %b11, label %b12

b7:
  %78 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %79 = icmp eq i8 %59, 0
  br i1 %79, label %80, label %b7

80:
  %81 = getelementptr i8, ptr addrspace(1) %58, i16 2
  %82 = load i16, ptr addrspace(1) %81
  %83 = getelementptr i8, ptr addrspace(1) %61, i16 2
  %84 = load i16, ptr addrspace(1) %83
  %85 = icmp ule i16 %82, %84
  br i1 %85, label %86, label %87

86:
  br label %88

87:
  br label %88

88:
  %89 = phi ptr addrspace(1) [ %81, %86 ], [ %83, %87 ]
  %90 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %90)
  %91 = load i16, ptr addrspace(1) %89
  call addrspace(1) void @N$PU2(i16 %91)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %92 = getelementptr i8, ptr addrspace(5) %73, i16 4
  %93 = load ptr addrspace(1), ptr addrspace(5) %92, !tbaa !2
  %94 = getelementptr inbounds i8, ptr addrspace(1) %93, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %76, ptr addrspace(1) %94)
  call addrspace(1) void @N$PU2(i16 %75)
  %95 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %95)
  call addrspace(1) void @N$PV(ptr addrspace(1) %76)
  call addrspace(1) void @N$PN()
  %96 = load ptr, ptr %16, !tbaa !2
  %97 = getelementptr i8, ptr %96, i16 -4
  %98 = load i16, ptr %97
  %99 = getelementptr i8, ptr @$str12, i16 6
  %100 = getelementptr i8, ptr @$str13, i16 6
  %101 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %102 = phi i16 [ 0, %b11 ], [ %109, %b16 ]
  %103 = icmp ult i16 %102, %98
  br i1 %103, label %b15, label %b17

b15:
  %104 = mul i16 %102, 6
  %105 = getelementptr inbounds i8, ptr %96, i16 %104
  %106 = getelementptr i8, ptr %105, i16 4
  %107 = load i16, ptr %106
  %108 = icmp ult i16 %107, 5
  br i1 %108, label %b18, label %b16

b16:
  %109 = add i16 %102, 1
  br label %b14

b17:
  %110 = getelementptr i8, ptr @$str15, i16 6
  %111 = addrspacecast ptr %110 to ptr addrspace(1)
  store i16 3, ptr %4, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 3, ptr %112, !tbaa !2
  %113 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %111, ptr %113, !tbaa !2
  %114 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %39, ptr addrspace(1) %114, i16 1, i16 500)
  %115 = load ptr, ptr %16, !tbaa !2
  %116 = getelementptr i8, ptr %115, i16 -4
  %117 = load i16, ptr %116
  call addrspace(1) void @N$PU2(i16 %117)
  %118 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %118)
  call addrspace(1) void @N$PN()
  %119 = load ptr, ptr %14, !tbaa !2
  %120 = icmp ne ptr %119, null
  br i1 %120, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %99)
  %121 = load ptr, ptr %105
  call addrspace(1) void @N$PS(ptr %121)
  call addrspace(1) void @N$PS(ptr %100)
  %122 = load i16, ptr %106
  call addrspace(1) void @N$PU2(i16 %122)
  call addrspace(1) void @N$PS(ptr %101)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %119)
  %123 = load ptr, ptr %16, !tbaa !2
  %124 = icmp ne ptr %123, null
  br i1 %124, label %b28, label %b27

b23:
  %125 = getelementptr i8, ptr %119, i16 -4
  %126 = load i16, ptr %125
  br label %b24

b24:
  %127 = phi i16 [ 0, %b23 ], [ %132, %b26 ]
  %128 = icmp ult i16 %127, %126
  br i1 %128, label %b26, label %b25

b25:
  br label %b22

b26:
  %129 = mul i16 %127, 6
  %130 = getelementptr inbounds i8, ptr %119, i16 %129
  %131 = load ptr, ptr %130
  call addrspace(1) void @N$BDRP(ptr %131)
  %132 = add i16 %127, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %123)
  ret i16 0

b28:
  %133 = getelementptr i8, ptr %123, i16 -4
  %134 = load i16, ptr %133
  br label %b29

b29:
  %135 = phi i16 [ 0, %b28 ], [ %140, %b31 ]
  %136 = icmp ult i16 %135, %134
  br i1 %136, label %b31, label %b30

b30:
  br label %b27

b31:
  %137 = mul i16 %135, 6
  %138 = getelementptr inbounds i8, ptr %123, i16 %137
  %139 = load ptr, ptr %138
  call addrspace(1) void @N$BDRP(ptr %139)
  %140 = add i16 %135, 1
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
