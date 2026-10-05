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
  store ptr %19, ptr %2, !tbaa !2
  %39 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %27, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %39, ptr addrspace(1) %42, i16 28, i16 9)
  %43 = getelementptr i8, ptr @$str5, i16 6
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %44, ptr %46, !tbaa !2
  %47 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %39, ptr addrspace(1) %47, i16 2, i16 100)
  %48 = load ptr, ptr %2, !tbaa !2
  store ptr %48, ptr addrspace(5) %37
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %48, ptr %17, !tbaa !2
  %49 = addrspacecast ptr %15 to ptr addrspace(1)
  %50 = addrspacecast ptr %18 to ptr addrspace(1)
  %51 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 3, ptr %14, !tbaa !2
  %52 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 3, ptr %52, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %44, ptr %53, !tbaa !2
  %54 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %49, ptr addrspace(1) %50, ptr addrspace(1) %51, ptr addrspace(1) %54)
  %55 = load i8, ptr %15, !tbaa !2, !range !7
  %56 = icmp eq i8 %55, 0
  br i1 %56, label %b4, label %b3

b2:
  %57 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 4, ptr %12, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %58, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %27, ptr %59, !tbaa !2
  %60 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %57, ptr addrspace(1) %50, ptr addrspace(1) %51, ptr addrspace(1) %60)
  %61 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %62 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %62, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %27, ptr %63, !tbaa !2
  %64 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %61, ptr addrspace(1) %51, ptr addrspace(1) %50, ptr addrspace(1) %64)
  %65 = load i8, ptr %13
  %66 = getelementptr i8, ptr %13, i16 2
  %67 = load ptr addrspace(1), ptr %66
  %68 = load i8, ptr %11
  %69 = getelementptr i8, ptr %11, i16 2
  %70 = load ptr addrspace(1), ptr %69
  %71 = icmp eq i8 %65, 0
  br i1 %71, label %b8, label %b7

b3:
  %72 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %72)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %73 = getelementptr inbounds i8, ptr %15, i16 2
  %74 = load ptr addrspace(1), ptr %73, !tbaa !2
  %75 = load ptr, ptr addrspace(1) %74
  call addrspace(1) void @N$PS(ptr %75)
  %76 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  %77 = getelementptr i8, ptr addrspace(1) %74, i16 4
  %78 = load i16, ptr addrspace(1) %77
  call addrspace(1) void @N$PU2(i16 %78)
  %79 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %79)
  %80 = getelementptr i8, ptr addrspace(1) %74, i16 2
  %81 = load i16, ptr addrspace(1) %80
  call addrspace(1) void @N$PU2(i16 %81)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %82 = addrspacecast ptr %9 to ptr addrspace(5)
  %83 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %83, ptr addrspace(1) %50, i16 20)
  %84 = load i16, ptr addrspace(5) %82
  %85 = addrspacecast ptr %8 to ptr addrspace(1)
  %86 = icmp ne i16 %84, 0
  br i1 %86, label %b11, label %b12

b7:
  %87 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %87)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %88 = icmp eq i8 %68, 0
  br i1 %88, label %89, label %b7

89:
  %90 = getelementptr i8, ptr addrspace(1) %67, i16 2
  %91 = load i16, ptr addrspace(1) %90
  %92 = getelementptr i8, ptr addrspace(1) %70, i16 2
  %93 = load i16, ptr addrspace(1) %92
  %94 = icmp ule i16 %91, %93
  br i1 %94, label %95, label %96

95:
  br label %97

96:
  br label %97

97:
  %98 = phi ptr addrspace(1) [ %90, %95 ], [ %92, %96 ]
  %99 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %99)
  %100 = load i16, ptr addrspace(1) %98
  call addrspace(1) void @N$PU2(i16 %100)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %101 = getelementptr i8, ptr addrspace(5) %82, i16 4
  %102 = load ptr addrspace(1), ptr addrspace(5) %101, !tbaa !2
  %103 = getelementptr inbounds i8, ptr addrspace(1) %102, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %85, ptr addrspace(1) %103)
  call addrspace(1) void @N$PU2(i16 %84)
  %104 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %104)
  call addrspace(1) void @N$PV(ptr addrspace(1) %85)
  call addrspace(1) void @N$PN()
  %105 = load ptr, ptr %18, !tbaa !2
  %106 = getelementptr i8, ptr %105, i16 -4
  %107 = load i16, ptr %106
  %108 = getelementptr i8, ptr @$str12, i16 6
  %109 = getelementptr i8, ptr @$str13, i16 6
  %110 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %111 = phi i16 [ 0, %b11 ], [ %118, %b16 ]
  %112 = icmp ult i16 %111, %107
  br i1 %112, label %b15, label %b17

b15:
  %113 = mul i16 %111, 6
  %114 = getelementptr inbounds i8, ptr %105, i16 %113
  %115 = getelementptr i8, ptr %114, i16 4
  %116 = load i16, ptr %115
  %117 = icmp ult i16 %116, 5
  br i1 %117, label %b18, label %b16

b16:
  %118 = add i16 %111, 1
  br label %b14

b17:
  %119 = getelementptr i8, ptr @$str15, i16 6
  %120 = addrspacecast ptr %119 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %121 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %121, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %120, ptr %122, !tbaa !2
  %123 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %50, ptr addrspace(1) %123, i16 1, i16 500)
  %124 = load ptr, ptr %18, !tbaa !2
  %125 = getelementptr i8, ptr %124, i16 -4
  %126 = load i16, ptr %125
  call addrspace(1) void @N$PU2(i16 %126)
  %127 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %127)
  call addrspace(1) void @N$PN()
  %128 = load ptr, ptr %17, !tbaa !2
  %129 = icmp ne ptr %128, null
  br i1 %129, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %108)
  %130 = load ptr, ptr %114
  call addrspace(1) void @N$PS(ptr %130)
  call addrspace(1) void @N$PS(ptr %109)
  %131 = load i16, ptr %115
  call addrspace(1) void @N$PU2(i16 %131)
  call addrspace(1) void @N$PS(ptr %110)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %128)
  %132 = load ptr, ptr %18, !tbaa !2
  %133 = icmp ne ptr %132, null
  br i1 %133, label %b28, label %b27

b23:
  %134 = getelementptr i8, ptr %128, i16 -4
  %135 = load i16, ptr %134
  br label %b24

b24:
  %136 = phi i16 [ 0, %b23 ], [ %141, %b26 ]
  %137 = icmp ult i16 %136, %135
  br i1 %137, label %b26, label %b25

b25:
  br label %b22

b26:
  %138 = mul i16 %136, 6
  %139 = getelementptr inbounds i8, ptr %128, i16 %138
  %140 = load ptr, ptr %139
  call addrspace(1) void @N$BDRP(ptr %140)
  %141 = add i16 %136, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %132)
  ret i16 0

b28:
  %142 = getelementptr i8, ptr %132, i16 -4
  %143 = load i16, ptr %142
  br label %b29

b29:
  %144 = phi i16 [ 0, %b28 ], [ %149, %b31 ]
  %145 = icmp ult i16 %144, %143
  br i1 %145, label %b31, label %b30

b30:
  br label %b27

b31:
  %146 = mul i16 %144, 6
  %147 = getelementptr inbounds i8, ptr %132, i16 %146
  %148 = load ptr, ptr %147
  call addrspace(1) void @N$BDRP(ptr %148)
  %149 = add i16 %144, 1
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
