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
  store ptr %19, ptr %2, !tbaa !2
  %38 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %27, ptr %40, !tbaa !2
  %41 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %38, ptr addrspace(1) %41, i16 28, i16 9)
  %42 = getelementptr i8, ptr @$str5, i16 6
  %43 = addrspacecast ptr %42 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %43, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %38, ptr addrspace(1) %46, i16 2, i16 100)
  %47 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %47, ptr %17, !tbaa !2
  %48 = addrspacecast ptr %15 to ptr addrspace(1)
  %49 = addrspacecast ptr %18 to ptr addrspace(1)
  %50 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 3, ptr %14, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 3, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %43, ptr %52, !tbaa !2
  %53 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %48, ptr addrspace(1) %49, ptr addrspace(1) %50, ptr addrspace(1) %53)
  %54 = load i8, ptr %15, !tbaa !2, !range !7
  %55 = icmp eq i8 %54, 0
  br i1 %55, label %b4, label %b3

b2:
  %56 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 4, ptr %12, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %27, ptr %58, !tbaa !2
  %59 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %56, ptr addrspace(1) %49, ptr addrspace(1) %50, ptr addrspace(1) %59)
  %60 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %61, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %27, ptr %62, !tbaa !2
  %63 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %60, ptr addrspace(1) %50, ptr addrspace(1) %49, ptr addrspace(1) %63)
  %64 = load i8, ptr %13
  %65 = getelementptr i8, ptr %13, i16 2
  %66 = load ptr addrspace(1), ptr %65
  %67 = load i8, ptr %11
  %68 = getelementptr i8, ptr %11, i16 2
  %69 = load ptr addrspace(1), ptr %68
  %70 = icmp eq i8 %64, 0
  br i1 %70, label %b8, label %b7

b3:
  %71 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %71)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %72 = getelementptr inbounds i8, ptr %15, i16 2
  %73 = load ptr addrspace(1), ptr %72, !tbaa !2
  %74 = load ptr, ptr addrspace(1) %73
  call addrspace(1) void @N$PS(ptr %74)
  %75 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %75)
  %76 = getelementptr i8, ptr addrspace(1) %73, i16 4
  %77 = load i16, ptr addrspace(1) %76
  call addrspace(1) void @N$PU2(i16 %77)
  %78 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  %79 = getelementptr i8, ptr addrspace(1) %73, i16 2
  %80 = load i16, ptr addrspace(1) %79
  call addrspace(1) void @N$PU2(i16 %80)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %81 = addrspacecast ptr %9 to ptr addrspace(5)
  %82 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %82, ptr addrspace(1) %49, i16 20)
  %83 = load i16, ptr addrspace(5) %81
  %84 = addrspacecast ptr %8 to ptr addrspace(1)
  %85 = icmp ne i16 %83, 0
  br i1 %85, label %b11, label %b12

b7:
  %86 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %86)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %87 = icmp eq i8 %67, 0
  br i1 %87, label %88, label %b7

88:
  %89 = getelementptr i8, ptr addrspace(1) %66, i16 2
  %90 = load i16, ptr addrspace(1) %89
  %91 = getelementptr i8, ptr addrspace(1) %69, i16 2
  %92 = load i16, ptr addrspace(1) %91
  %93 = icmp ule i16 %90, %92
  br i1 %93, label %94, label %95

94:
  br label %96

95:
  br label %96

96:
  %97 = phi ptr addrspace(1) [ %89, %94 ], [ %91, %95 ]
  %98 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %98)
  %99 = load i16, ptr addrspace(1) %97
  call addrspace(1) void @N$PU2(i16 %99)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %100 = getelementptr i8, ptr addrspace(5) %81, i16 4
  %101 = load ptr addrspace(1), ptr addrspace(5) %100, !tbaa !2
  %102 = getelementptr inbounds i8, ptr addrspace(1) %101, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %84, ptr addrspace(1) %102)
  call addrspace(1) void @N$PU2(i16 %83)
  %103 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %103)
  call addrspace(1) void @N$PV(ptr addrspace(1) %84)
  call addrspace(1) void @N$PN()
  %104 = load ptr, ptr %18, !tbaa !2
  %105 = getelementptr i8, ptr %104, i16 -4
  %106 = load i16, ptr %105
  %107 = getelementptr i8, ptr @$str12, i16 6
  %108 = getelementptr i8, ptr @$str13, i16 6
  %109 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %110 = phi i16 [ 0, %b11 ], [ %117, %b16 ]
  %111 = icmp ult i16 %110, %106
  br i1 %111, label %b15, label %b17

b15:
  %112 = mul i16 %110, 6
  %113 = getelementptr inbounds i8, ptr %104, i16 %112
  %114 = getelementptr i8, ptr %113, i16 4
  %115 = load i16, ptr %114
  %116 = icmp ult i16 %115, 5
  br i1 %116, label %b18, label %b16

b16:
  %117 = add i16 %110, 1
  br label %b14

b17:
  %118 = getelementptr i8, ptr @$str15, i16 6
  %119 = addrspacecast ptr %118 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %120, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %119, ptr %121, !tbaa !2
  %122 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %49, ptr addrspace(1) %122, i16 1, i16 500)
  %123 = load ptr, ptr %18, !tbaa !2
  %124 = getelementptr i8, ptr %123, i16 -4
  %125 = load i16, ptr %124
  call addrspace(1) void @N$PU2(i16 %125)
  %126 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %126)
  call addrspace(1) void @N$PN()
  %127 = load ptr, ptr %17, !tbaa !2
  %128 = icmp ne ptr %127, null
  br i1 %128, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %107)
  %129 = load ptr, ptr %113
  call addrspace(1) void @N$PS(ptr %129)
  call addrspace(1) void @N$PS(ptr %108)
  %130 = load i16, ptr %114
  call addrspace(1) void @N$PU2(i16 %130)
  call addrspace(1) void @N$PS(ptr %109)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %127)
  %131 = load ptr, ptr %18, !tbaa !2
  %132 = icmp ne ptr %131, null
  br i1 %132, label %b28, label %b27

b23:
  %133 = getelementptr i8, ptr %127, i16 -4
  %134 = load i16, ptr %133
  br label %b24

b24:
  %135 = phi i16 [ 0, %b23 ], [ %140, %b26 ]
  %136 = icmp ult i16 %135, %134
  br i1 %136, label %b26, label %b25

b25:
  br label %b22

b26:
  %137 = mul i16 %135, 6
  %138 = getelementptr inbounds i8, ptr %127, i16 %137
  %139 = load ptr, ptr %138
  call addrspace(1) void @N$BDRP(ptr %139)
  %140 = add i16 %135, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %131)
  ret i16 0

b28:
  %141 = getelementptr i8, ptr %131, i16 -4
  %142 = load i16, ptr %141
  br label %b29

b29:
  %143 = phi i16 [ 0, %b28 ], [ %148, %b31 ]
  %144 = icmp ult i16 %143, %142
  br i1 %144, label %b31, label %b30

b30:
  br label %b27

b31:
  %145 = mul i16 %143, 6
  %146 = getelementptr inbounds i8, ptr %131, i16 %145
  %147 = load ptr, ptr %146
  call addrspace(1) void @N$BDRP(ptr %147)
  %148 = add i16 %143, 1
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
