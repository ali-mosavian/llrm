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
  %18 = addrspacecast ptr %15 to ptr addrspace(1)
  %19 = getelementptr i8, ptr @$str1, i16 6
  store ptr %19, ptr %3, !tbaa !2
  %20 = addrspacecast ptr %3 to ptr addrspace(1)
  %21 = getelementptr i8, ptr @$str2, i16 6
  %22 = addrspacecast ptr %21 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %22, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %25, i16 5, i16 40)
  %26 = getelementptr i8, ptr @$str3, i16 6
  %27 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %27, ptr %29, !tbaa !2
  %30 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %30, i16 30, i16 3)
  %31 = getelementptr i8, ptr @$str4, i16 6
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %35, i16 12, i16 0)
  %36 = load ptr, ptr %3, !tbaa !2
  store ptr %36, ptr addrspace(5) %17
  store ptr null, ptr %3, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
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
  store i16 4, ptr %9, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %27, ptr %51, !tbaa !2
  %52 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %49, ptr addrspace(1) %40, ptr addrspace(1) %41, ptr addrspace(1) %52)
  %53 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %7, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 4, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %27, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %53, ptr addrspace(1) %41, ptr addrspace(1) %40, ptr addrspace(1) %56)
  %57 = load i8, ptr %10
  %58 = getelementptr i8, ptr %10, i16 2
  %59 = load ptr addrspace(1), ptr %58
  %60 = load i8, ptr %8
  %61 = getelementptr i8, ptr %8, i16 2
  %62 = load ptr addrspace(1), ptr %61
  %63 = icmp eq i8 %57, 0
  br i1 %63, label %b8, label %b7

b3:
  %64 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %64)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %65 = getelementptr inbounds i8, ptr %12, i16 2
  %66 = load ptr addrspace(1), ptr %65, !tbaa !2
  %67 = load ptr, ptr addrspace(1) %66
  call addrspace(1) void @N$PS(ptr %67)
  %68 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  %69 = getelementptr i8, ptr addrspace(1) %66, i16 4
  %70 = load i16, ptr addrspace(1) %69
  call addrspace(1) void @N$PU2(i16 %70)
  %71 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %71)
  %72 = getelementptr i8, ptr addrspace(1) %66, i16 2
  %73 = load i16, ptr addrspace(1) %72
  call addrspace(1) void @N$PU2(i16 %73)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %74 = addrspacecast ptr %6 to ptr addrspace(5)
  %75 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %75, ptr addrspace(1) %40, i16 20)
  %76 = load i16, ptr addrspace(5) %74
  %77 = addrspacecast ptr %5 to ptr addrspace(1)
  %78 = icmp ne i16 %76, 0
  br i1 %78, label %b11, label %b12

b7:
  %79 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %79)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %80 = icmp eq i8 %60, 0
  br i1 %80, label %81, label %b7

81:
  %82 = getelementptr i8, ptr addrspace(1) %59, i16 2
  %83 = load i16, ptr addrspace(1) %82
  %84 = getelementptr i8, ptr addrspace(1) %62, i16 2
  %85 = load i16, ptr addrspace(1) %84
  %86 = icmp ule i16 %83, %85
  br i1 %86, label %87, label %88

87:
  br label %89

88:
  br label %89

89:
  %90 = phi ptr addrspace(1) [ %82, %87 ], [ %84, %88 ]
  %91 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %91)
  %92 = load i16, ptr addrspace(1) %90
  call addrspace(1) void @N$PU2(i16 %92)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %93 = getelementptr i8, ptr addrspace(5) %74, i16 4
  %94 = load ptr addrspace(1), ptr addrspace(5) %93, !tbaa !2
  %95 = getelementptr inbounds i8, ptr addrspace(1) %94, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %77, ptr addrspace(1) %95)
  call addrspace(1) void @N$PU2(i16 %76)
  %96 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %96)
  call addrspace(1) void @N$PV(ptr addrspace(1) %77)
  call addrspace(1) void @N$PN()
  %97 = load ptr, ptr %16, !tbaa !2
  %98 = getelementptr i8, ptr %97, i16 -4
  %99 = load i16, ptr %98
  %100 = getelementptr i8, ptr @$str12, i16 6
  %101 = getelementptr i8, ptr @$str13, i16 6
  %102 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %103 = phi i16 [ 0, %b11 ], [ %110, %b16 ]
  %104 = icmp ult i16 %103, %99
  br i1 %104, label %b15, label %b17

b15:
  %105 = mul i16 %103, 6
  %106 = getelementptr inbounds i8, ptr %97, i16 %105
  %107 = getelementptr i8, ptr %106, i16 4
  %108 = load i16, ptr %107
  %109 = icmp ult i16 %108, 5
  br i1 %109, label %b18, label %b16

b16:
  %110 = add i16 %103, 1
  br label %b14

b17:
  %111 = getelementptr i8, ptr @$str15, i16 6
  %112 = addrspacecast ptr %111 to ptr addrspace(1)
  store i16 3, ptr %4, !tbaa !2
  %113 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 3, ptr %113, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %112, ptr %114, !tbaa !2
  %115 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %40, ptr addrspace(1) %115, i16 1, i16 500)
  %116 = load ptr, ptr %16, !tbaa !2
  %117 = getelementptr i8, ptr %116, i16 -4
  %118 = load i16, ptr %117
  call addrspace(1) void @N$PU2(i16 %118)
  %119 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %119)
  call addrspace(1) void @N$PN()
  %120 = load ptr, ptr %14, !tbaa !2
  %121 = icmp ne ptr %120, null
  br i1 %121, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %100)
  %122 = load ptr, ptr %106
  call addrspace(1) void @N$PS(ptr %122)
  call addrspace(1) void @N$PS(ptr %101)
  %123 = load i16, ptr %107
  call addrspace(1) void @N$PU2(i16 %123)
  call addrspace(1) void @N$PS(ptr %102)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %120)
  %124 = load ptr, ptr %16, !tbaa !2
  %125 = icmp ne ptr %124, null
  br i1 %125, label %b28, label %b27

b23:
  %126 = getelementptr i8, ptr %120, i16 -4
  %127 = load i16, ptr %126
  br label %b24

b24:
  %128 = phi i16 [ 0, %b23 ], [ %133, %b26 ]
  %129 = icmp ult i16 %128, %127
  br i1 %129, label %b26, label %b25

b25:
  br label %b22

b26:
  %130 = mul i16 %128, 6
  %131 = getelementptr inbounds i8, ptr %120, i16 %130
  %132 = load ptr, ptr %131
  call addrspace(1) void @N$BDRP(ptr %132)
  %133 = add i16 %128, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %124)
  ret i16 0

b28:
  %134 = getelementptr i8, ptr %124, i16 -4
  %135 = load i16, ptr %134
  br label %b29

b29:
  %136 = phi i16 [ 0, %b28 ], [ %141, %b31 ]
  %137 = icmp ult i16 %136, %135
  br i1 %137, label %b31, label %b30

b30:
  br label %b27

b31:
  %138 = mul i16 %136, 6
  %139 = getelementptr inbounds i8, ptr %124, i16 %138
  %140 = load ptr, ptr %139
  call addrspace(1) void @N$BDRP(ptr %140)
  %141 = add i16 %136, 1
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
