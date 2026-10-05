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
  %2 = alloca [2 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [2 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [8 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [6 x i8]
  %12 = alloca [8 x i8]
  %13 = alloca [6 x i8]
  %14 = alloca [8 x i8]
  %15 = alloca [6 x i8]
  %16 = alloca [2 x i8]
  %17 = alloca [2 x i8]
  %18 = alloca [2 x i8]
  %19 = getelementptr i8, ptr @$str1, i16 6
  store ptr %19, ptr %6, !tbaa !2
  %20 = addrspacecast ptr %6 to ptr addrspace(1)
  %21 = getelementptr i8, ptr @$str2, i16 6
  %22 = addrspacecast ptr %21 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %22, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %25, i16 5, i16 40)
  %26 = getelementptr i8, ptr @$str3, i16 6
  %27 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 4, ptr %4, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %27, ptr %29, !tbaa !2
  %30 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %30, i16 30, i16 3)
  %31 = getelementptr i8, ptr @$str4, i16 6
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %35, i16 12, i16 0)
  %36 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %36, ptr %18, !tbaa !2
  %37 = addrspacecast ptr %16 to ptr addrspace(5)
  %38 = addrspacecast ptr %16 to ptr addrspace(1)
  %39 = getelementptr i8, ptr @$str1, i16 6
  store ptr %39, ptr %2, !tbaa !2
  %40 = addrspacecast ptr %2 to ptr addrspace(1)
  %41 = getelementptr i8, ptr @$str3, i16 6
  %42 = addrspacecast ptr %41 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %43, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %42, ptr %44, !tbaa !2
  %45 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %40, ptr addrspace(1) %45, i16 28, i16 9)
  %46 = getelementptr i8, ptr @$str5, i16 6
  %47 = addrspacecast ptr %46 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %47, ptr %49, !tbaa !2
  %50 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %40, ptr addrspace(1) %50, i16 2, i16 100)
  %51 = load ptr, ptr %2, !tbaa !2
  store ptr %51, ptr addrspace(5) %37
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  %52 = load ptr, ptr %16, !tbaa !2
  store ptr %52, ptr %17, !tbaa !2
  %53 = addrspacecast ptr %15 to ptr addrspace(1)
  %54 = addrspacecast ptr %18 to ptr addrspace(1)
  %55 = addrspacecast ptr %17 to ptr addrspace(1)
  %56 = getelementptr i8, ptr @$str5, i16 6
  %57 = addrspacecast ptr %56 to ptr addrspace(1)
  store i16 3, ptr %14, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 3, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %57, ptr %59, !tbaa !2
  %60 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %53, ptr addrspace(1) %54, ptr addrspace(1) %55, ptr addrspace(1) %60)
  %61 = load i8, ptr %15, !tbaa !2, !range !7
  %62 = icmp eq i8 %61, 0
  br i1 %62, label %b4, label %b3

b2:
  %63 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 4, ptr %12, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %64, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %27, ptr %65, !tbaa !2
  %66 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %63, ptr addrspace(1) %54, ptr addrspace(1) %55, ptr addrspace(1) %66)
  %67 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %68, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %27, ptr %69, !tbaa !2
  %70 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %67, ptr addrspace(1) %55, ptr addrspace(1) %54, ptr addrspace(1) %70)
  %71 = load i8, ptr %13
  %72 = getelementptr i8, ptr %13, i16 2
  %73 = load ptr addrspace(1), ptr %72
  %74 = load i8, ptr %11
  %75 = getelementptr i8, ptr %11, i16 2
  %76 = load ptr addrspace(1), ptr %75
  %77 = icmp eq i8 %71, 0
  br i1 %77, label %b8, label %b7

b3:
  %78 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %79 = getelementptr inbounds i8, ptr %15, i16 2
  %80 = load ptr addrspace(1), ptr %79, !tbaa !2
  %81 = load ptr, ptr addrspace(1) %80
  call addrspace(1) void @N$PS(ptr %81)
  %82 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %82)
  %83 = getelementptr i8, ptr addrspace(1) %80, i16 4
  %84 = load i16, ptr addrspace(1) %83
  call addrspace(1) void @N$PU2(i16 %84)
  %85 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %85)
  %86 = getelementptr i8, ptr addrspace(1) %80, i16 2
  %87 = load i16, ptr addrspace(1) %86
  call addrspace(1) void @N$PU2(i16 %87)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %88 = addrspacecast ptr %9 to ptr addrspace(5)
  %89 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %89, ptr addrspace(1) %54, i16 20)
  %90 = load i16, ptr addrspace(5) %88
  %91 = addrspacecast ptr %8 to ptr addrspace(1)
  %92 = icmp ne i16 %90, 0
  br i1 %92, label %b11, label %b12

b7:
  %93 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %93)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %94 = icmp eq i8 %74, 0
  br i1 %94, label %95, label %b7

95:
  %96 = getelementptr i8, ptr addrspace(1) %73, i16 2
  %97 = load i16, ptr addrspace(1) %96
  %98 = getelementptr i8, ptr addrspace(1) %76, i16 2
  %99 = load i16, ptr addrspace(1) %98
  %100 = icmp ule i16 %97, %99
  br i1 %100, label %101, label %102

101:
  br label %103

102:
  br label %103

103:
  %104 = phi ptr addrspace(1) [ %96, %101 ], [ %98, %102 ]
  %105 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %105)
  %106 = load i16, ptr addrspace(1) %104
  call addrspace(1) void @N$PU2(i16 %106)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %107 = getelementptr i8, ptr addrspace(5) %88, i16 4
  %108 = load ptr addrspace(1), ptr addrspace(5) %107, !tbaa !2
  %109 = getelementptr inbounds i8, ptr addrspace(1) %108, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %91, ptr addrspace(1) %109)
  call addrspace(1) void @N$PU2(i16 %90)
  %110 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %110)
  call addrspace(1) void @N$PV(ptr addrspace(1) %91)
  call addrspace(1) void @N$PN()
  %111 = load ptr, ptr %18, !tbaa !2
  %112 = getelementptr i8, ptr %111, i16 -4
  %113 = load i16, ptr %112
  %114 = getelementptr i8, ptr @$str12, i16 6
  %115 = getelementptr i8, ptr @$str13, i16 6
  %116 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %117 = phi i16 [ 0, %b11 ], [ %124, %b16 ]
  %118 = icmp ult i16 %117, %113
  br i1 %118, label %b15, label %b17

b15:
  %119 = mul i16 %117, 6
  %120 = getelementptr inbounds i8, ptr %111, i16 %119
  %121 = getelementptr i8, ptr %120, i16 4
  %122 = load i16, ptr %121
  %123 = icmp ult i16 %122, 5
  br i1 %123, label %b18, label %b16

b16:
  %124 = add i16 %117, 1
  br label %b14

b17:
  %125 = getelementptr i8, ptr @$str15, i16 6
  %126 = addrspacecast ptr %125 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %127, !tbaa !2
  %128 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %126, ptr %128, !tbaa !2
  %129 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %54, ptr addrspace(1) %129, i16 1, i16 500)
  %130 = load ptr, ptr %18, !tbaa !2
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  call addrspace(1) void @N$PU2(i16 %132)
  %133 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %133)
  call addrspace(1) void @N$PN()
  %134 = load ptr, ptr %17, !tbaa !2
  %135 = icmp ne ptr %134, null
  br i1 %135, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %114)
  %136 = load ptr, ptr %120
  call addrspace(1) void @N$PS(ptr %136)
  call addrspace(1) void @N$PS(ptr %115)
  %137 = load i16, ptr %121
  call addrspace(1) void @N$PU2(i16 %137)
  call addrspace(1) void @N$PS(ptr %116)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %134)
  %138 = load ptr, ptr %18, !tbaa !2
  %139 = icmp ne ptr %138, null
  br i1 %139, label %b28, label %b27

b23:
  %140 = getelementptr i8, ptr %134, i16 -4
  %141 = load i16, ptr %140
  br label %b24

b24:
  %142 = phi i16 [ 0, %b23 ], [ %147, %b26 ]
  %143 = icmp ult i16 %142, %141
  br i1 %143, label %b26, label %b25

b25:
  br label %b22

b26:
  %144 = mul i16 %142, 6
  %145 = getelementptr inbounds i8, ptr %134, i16 %144
  %146 = load ptr, ptr %145
  call addrspace(1) void @N$BDRP(ptr %146)
  %147 = add i16 %142, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %138)
  ret i16 0

b28:
  %148 = getelementptr i8, ptr %138, i16 -4
  %149 = load i16, ptr %148
  br label %b29

b29:
  %150 = phi i16 [ 0, %b28 ], [ %155, %b31 ]
  %151 = icmp ult i16 %150, %149
  br i1 %151, label %b31, label %b30

b30:
  br label %b27

b31:
  %152 = mul i16 %150, 6
  %153 = getelementptr inbounds i8, ptr %138, i16 %152
  %154 = load ptr, ptr %153
  call addrspace(1) void @N$BDRP(ptr %154)
  %155 = add i16 %150, 1
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
