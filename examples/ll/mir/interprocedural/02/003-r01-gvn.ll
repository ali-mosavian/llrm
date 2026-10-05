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
  %37 = load ptr, ptr %15, !tbaa !2
  store ptr %37, ptr %16, !tbaa !2
  %38 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %38)
  %39 = load ptr, ptr %13, !tbaa !2
  store ptr %39, ptr %14, !tbaa !2
  %40 = addrspacecast ptr %12 to ptr addrspace(1)
  %41 = addrspacecast ptr %16 to ptr addrspace(1)
  %42 = addrspacecast ptr %14 to ptr addrspace(1)
  %43 = getelementptr i8, ptr @$str5, i16 6
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  store i16 3, ptr %11, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 3, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %44, ptr %46, !tbaa !2
  %47 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %40, ptr addrspace(1) %41, ptr addrspace(1) %42, ptr addrspace(1) %47)
  %48 = load i8, ptr %12, !tbaa !2, !range !7
  %49 = icmp eq i8 %48, 0
  br i1 %49, label %b4, label %b3

b2:
  %50 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %51, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %27, ptr %52, !tbaa !2
  %53 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %50, ptr addrspace(1) %41, ptr addrspace(1) %42, ptr addrspace(1) %53)
  %54 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %7, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 4, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %27, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %54, ptr addrspace(1) %42, ptr addrspace(1) %41, ptr addrspace(1) %57)
  %58 = load i8, ptr %10
  %59 = getelementptr i8, ptr %10, i16 2
  %60 = load ptr addrspace(1), ptr %59
  %61 = load i8, ptr %8
  %62 = getelementptr i8, ptr %8, i16 2
  %63 = load ptr addrspace(1), ptr %62
  %64 = icmp eq i8 %58, 0
  br i1 %64, label %b8, label %b7

b3:
  %65 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %65)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %66 = getelementptr inbounds i8, ptr %12, i16 2
  %67 = load ptr addrspace(1), ptr %66, !tbaa !2
  %68 = load ptr, ptr addrspace(1) %67
  call addrspace(1) void @N$PS(ptr %68)
  %69 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %69)
  %70 = getelementptr i8, ptr addrspace(1) %67, i16 4
  %71 = load i16, ptr addrspace(1) %70
  call addrspace(1) void @N$PU2(i16 %71)
  %72 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %72)
  %73 = getelementptr i8, ptr addrspace(1) %67, i16 2
  %74 = load i16, ptr addrspace(1) %73
  call addrspace(1) void @N$PU2(i16 %74)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %75 = addrspacecast ptr %6 to ptr addrspace(5)
  %76 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %76, ptr addrspace(1) %41, i16 20)
  %77 = load i16, ptr addrspace(5) %75
  %78 = addrspacecast ptr %5 to ptr addrspace(1)
  %79 = icmp ne i16 %77, 0
  br i1 %79, label %b11, label %b12

b7:
  %80 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %80)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %81 = icmp eq i8 %61, 0
  br i1 %81, label %82, label %b7

82:
  %83 = getelementptr i8, ptr addrspace(1) %60, i16 2
  %84 = load i16, ptr addrspace(1) %83
  %85 = getelementptr i8, ptr addrspace(1) %63, i16 2
  %86 = load i16, ptr addrspace(1) %85
  %87 = icmp ule i16 %84, %86
  br i1 %87, label %88, label %89

88:
  br label %90

89:
  br label %90

90:
  %91 = phi ptr addrspace(1) [ %83, %88 ], [ %85, %89 ]
  %92 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %92)
  %93 = load i16, ptr addrspace(1) %91
  call addrspace(1) void @N$PU2(i16 %93)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %94 = getelementptr i8, ptr addrspace(5) %75, i16 4
  %95 = load ptr addrspace(1), ptr addrspace(5) %94, !tbaa !2
  %96 = getelementptr inbounds i8, ptr addrspace(1) %95, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %78, ptr addrspace(1) %96)
  call addrspace(1) void @N$PU2(i16 %77)
  %97 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %97)
  call addrspace(1) void @N$PV(ptr addrspace(1) %78)
  call addrspace(1) void @N$PN()
  %98 = load ptr, ptr %16, !tbaa !2
  %99 = getelementptr i8, ptr %98, i16 -4
  %100 = load i16, ptr %99
  %101 = getelementptr i8, ptr @$str12, i16 6
  %102 = getelementptr i8, ptr @$str13, i16 6
  %103 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %104 = phi i16 [ 0, %b11 ], [ %111, %b16 ]
  %105 = icmp ult i16 %104, %100
  br i1 %105, label %b15, label %b17

b15:
  %106 = mul i16 %104, 6
  %107 = getelementptr inbounds i8, ptr %98, i16 %106
  %108 = getelementptr i8, ptr %107, i16 4
  %109 = load i16, ptr %108
  %110 = icmp ult i16 %109, 5
  br i1 %110, label %b18, label %b16

b16:
  %111 = add i16 %104, 1
  br label %b14

b17:
  %112 = getelementptr i8, ptr @$str15, i16 6
  %113 = addrspacecast ptr %112 to ptr addrspace(1)
  store i16 3, ptr %4, !tbaa !2
  %114 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 3, ptr %114, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %113, ptr %115, !tbaa !2
  %116 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %41, ptr addrspace(1) %116, i16 1, i16 500)
  %117 = load ptr, ptr %16, !tbaa !2
  %118 = getelementptr i8, ptr %117, i16 -4
  %119 = load i16, ptr %118
  call addrspace(1) void @N$PU2(i16 %119)
  %120 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %120)
  call addrspace(1) void @N$PN()
  %121 = load ptr, ptr %14, !tbaa !2
  %122 = icmp ne ptr %121, null
  br i1 %122, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %101)
  %123 = load ptr, ptr %107
  call addrspace(1) void @N$PS(ptr %123)
  call addrspace(1) void @N$PS(ptr %102)
  %124 = load i16, ptr %108
  call addrspace(1) void @N$PU2(i16 %124)
  call addrspace(1) void @N$PS(ptr %103)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %121)
  %125 = load ptr, ptr %16, !tbaa !2
  %126 = icmp ne ptr %125, null
  br i1 %126, label %b28, label %b27

b23:
  %127 = getelementptr i8, ptr %121, i16 -4
  %128 = load i16, ptr %127
  br label %b24

b24:
  %129 = phi i16 [ 0, %b23 ], [ %134, %b26 ]
  %130 = icmp ult i16 %129, %128
  br i1 %130, label %b26, label %b25

b25:
  br label %b22

b26:
  %131 = mul i16 %129, 6
  %132 = getelementptr inbounds i8, ptr %121, i16 %131
  %133 = load ptr, ptr %132
  call addrspace(1) void @N$BDRP(ptr %133)
  %134 = add i16 %129, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %125)
  ret i16 0

b28:
  %135 = getelementptr i8, ptr %125, i16 -4
  %136 = load i16, ptr %135
  br label %b29

b29:
  %137 = phi i16 [ 0, %b28 ], [ %142, %b31 ]
  %138 = icmp ult i16 %137, %136
  br i1 %138, label %b31, label %b30

b30:
  br label %b27

b31:
  %139 = mul i16 %137, 6
  %140 = getelementptr inbounds i8, ptr %125, i16 %139
  %141 = load ptr, ptr %140
  call addrspace(1) void @N$BDRP(ptr %141)
  %142 = add i16 %137, 1
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
