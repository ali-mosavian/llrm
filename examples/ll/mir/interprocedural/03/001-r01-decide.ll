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
  %37 = addrspacecast ptr %16 to ptr addrspace(1)
  %38 = getelementptr i8, ptr @$str1, i16 6
  store ptr %38, ptr %2, !tbaa !2
  %39 = addrspacecast ptr %2 to ptr addrspace(1)
  %40 = getelementptr i8, ptr @$str3, i16 6
  %41 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %41, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %39, ptr addrspace(1) %44, i16 28, i16 9)
  %45 = getelementptr i8, ptr @$str5, i16 6
  %46 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %46, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %39, ptr addrspace(1) %49, i16 2, i16 100)
  %50 = load ptr, ptr %2, !tbaa !2
  store ptr %50, ptr addrspace(1) %37
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  %51 = load ptr, ptr %16, !tbaa !2
  store ptr %51, ptr %17, !tbaa !2
  %52 = addrspacecast ptr %15 to ptr addrspace(1)
  %53 = addrspacecast ptr %18 to ptr addrspace(1)
  %54 = addrspacecast ptr %17 to ptr addrspace(1)
  %55 = getelementptr i8, ptr @$str5, i16 6
  %56 = addrspacecast ptr %55 to ptr addrspace(1)
  store i16 3, ptr %14, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 3, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %56, ptr %58, !tbaa !2
  %59 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %52, ptr addrspace(1) %53, ptr addrspace(1) %54, ptr addrspace(1) %59)
  %60 = load i8, ptr %15, !tbaa !2, !range !7
  %61 = icmp eq i8 %60, 0
  br i1 %61, label %b4, label %b3

b2:
  %62 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 4, ptr %12, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %27, ptr %64, !tbaa !2
  %65 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %62, ptr addrspace(1) %53, ptr addrspace(1) %54, ptr addrspace(1) %65)
  %66 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %67 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %67, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %27, ptr %68, !tbaa !2
  %69 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %66, ptr addrspace(1) %54, ptr addrspace(1) %53, ptr addrspace(1) %69)
  %70 = load i8, ptr %13
  %71 = getelementptr i8, ptr %13, i16 2
  %72 = load ptr addrspace(1), ptr %71
  %73 = load i8, ptr %11
  %74 = getelementptr i8, ptr %11, i16 2
  %75 = load ptr addrspace(1), ptr %74
  %76 = icmp eq i8 %70, 0
  br i1 %76, label %b8, label %b7

b3:
  %77 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %77)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %78 = getelementptr inbounds i8, ptr %15, i16 2
  %79 = load ptr addrspace(1), ptr %78, !tbaa !2
  %80 = load ptr, ptr addrspace(1) %79
  call addrspace(1) void @N$PS(ptr %80)
  %81 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %81)
  %82 = getelementptr i8, ptr addrspace(1) %79, i16 4
  %83 = load i16, ptr addrspace(1) %82
  call addrspace(1) void @N$PU2(i16 %83)
  %84 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %84)
  %85 = getelementptr i8, ptr addrspace(1) %79, i16 2
  %86 = load i16, ptr addrspace(1) %85
  call addrspace(1) void @N$PU2(i16 %86)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %87 = addrspacecast ptr %9 to ptr addrspace(5)
  %88 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %88, ptr addrspace(1) %53, i16 20)
  %89 = load i16, ptr addrspace(5) %87
  %90 = addrspacecast ptr %8 to ptr addrspace(1)
  %91 = icmp ne i16 %89, 0
  br i1 %91, label %b11, label %b12

b7:
  %92 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %92)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %93 = icmp eq i8 %73, 0
  br i1 %93, label %94, label %b7

94:
  %95 = getelementptr i8, ptr addrspace(1) %72, i16 2
  %96 = load i16, ptr addrspace(1) %95
  %97 = getelementptr i8, ptr addrspace(1) %75, i16 2
  %98 = load i16, ptr addrspace(1) %97
  %99 = icmp ule i16 %96, %98
  br i1 %99, label %100, label %101

100:
  br label %102

101:
  br label %102

102:
  %103 = phi ptr addrspace(1) [ %95, %100 ], [ %97, %101 ]
  %104 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %104)
  %105 = load i16, ptr addrspace(1) %103
  call addrspace(1) void @N$PU2(i16 %105)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %106 = getelementptr i8, ptr addrspace(5) %87, i16 4
  %107 = load ptr addrspace(1), ptr addrspace(5) %106, !tbaa !2
  %108 = getelementptr inbounds i8, ptr addrspace(1) %107, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %90, ptr addrspace(1) %108)
  call addrspace(1) void @N$PU2(i16 %89)
  %109 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %109)
  call addrspace(1) void @N$PV(ptr addrspace(1) %90)
  call addrspace(1) void @N$PN()
  %110 = load ptr, ptr %18, !tbaa !2
  %111 = getelementptr i8, ptr %110, i16 -4
  %112 = load i16, ptr %111
  %113 = getelementptr i8, ptr @$str12, i16 6
  %114 = getelementptr i8, ptr @$str13, i16 6
  %115 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %116 = phi i16 [ 0, %b11 ], [ %123, %b16 ]
  %117 = icmp ult i16 %116, %112
  br i1 %117, label %b15, label %b17

b15:
  %118 = mul i16 %116, 6
  %119 = getelementptr inbounds i8, ptr %110, i16 %118
  %120 = getelementptr i8, ptr %119, i16 4
  %121 = load i16, ptr %120
  %122 = icmp ult i16 %121, 5
  br i1 %122, label %b18, label %b16

b16:
  %123 = add i16 %116, 1
  br label %b14

b17:
  %124 = getelementptr i8, ptr @$str15, i16 6
  %125 = addrspacecast ptr %124 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %126 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %126, !tbaa !2
  %127 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %125, ptr %127, !tbaa !2
  %128 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %53, ptr addrspace(1) %128, i16 1, i16 500)
  %129 = load ptr, ptr %18, !tbaa !2
  %130 = getelementptr i8, ptr %129, i16 -4
  %131 = load i16, ptr %130
  call addrspace(1) void @N$PU2(i16 %131)
  %132 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %132)
  call addrspace(1) void @N$PN()
  %133 = load ptr, ptr %17, !tbaa !2
  %134 = icmp ne ptr %133, null
  br i1 %134, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %113)
  %135 = load ptr, ptr %119
  call addrspace(1) void @N$PS(ptr %135)
  call addrspace(1) void @N$PS(ptr %114)
  %136 = load i16, ptr %120
  call addrspace(1) void @N$PU2(i16 %136)
  call addrspace(1) void @N$PS(ptr %115)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %133)
  %137 = load ptr, ptr %18, !tbaa !2
  %138 = icmp ne ptr %137, null
  br i1 %138, label %b28, label %b27

b23:
  %139 = getelementptr i8, ptr %133, i16 -4
  %140 = load i16, ptr %139
  br label %b24

b24:
  %141 = phi i16 [ 0, %b23 ], [ %146, %b26 ]
  %142 = icmp ult i16 %141, %140
  br i1 %142, label %b26, label %b25

b25:
  br label %b22

b26:
  %143 = mul i16 %141, 6
  %144 = getelementptr inbounds i8, ptr %133, i16 %143
  %145 = load ptr, ptr %144
  call addrspace(1) void @N$BDRP(ptr %145)
  %146 = add i16 %141, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %137)
  ret i16 0

b28:
  %147 = getelementptr i8, ptr %137, i16 -4
  %148 = load i16, ptr %147
  br label %b29

b29:
  %149 = phi i16 [ 0, %b28 ], [ %154, %b31 ]
  %150 = icmp ult i16 %149, %148
  br i1 %150, label %b31, label %b30

b30:
  br label %b27

b31:
  %151 = mul i16 %149, 6
  %152 = getelementptr inbounds i8, ptr %137, i16 %151
  %153 = load ptr, ptr %152
  call addrspace(1) void @N$BDRP(ptr %153)
  %154 = add i16 %149, 1
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
