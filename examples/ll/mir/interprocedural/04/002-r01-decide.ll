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
  %18 = getelementptr i8, ptr @$str1, i16 6
  store ptr %18, ptr %6, !tbaa !2
  %19 = addrspacecast ptr %6 to ptr addrspace(1)
  %20 = getelementptr i8, ptr @$str2, i16 6
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %24, i16 5, i16 40)
  %25 = getelementptr i8, ptr @$str3, i16 6
  %26 = addrspacecast ptr %25 to ptr addrspace(1)
  store i16 4, ptr %4, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %29, i16 30, i16 3)
  %30 = getelementptr i8, ptr @$str4, i16 6
  %31 = addrspacecast ptr %30 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %32, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %31, ptr %33, !tbaa !2
  %34 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %34, i16 12, i16 0)
  %35 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %35, ptr %17, !tbaa !2
  store ptr %18, ptr %2, !tbaa !2
  %36 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %26, ptr %38, !tbaa !2
  %39 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %36, ptr addrspace(1) %39, i16 28, i16 9)
  %40 = getelementptr i8, ptr @$str5, i16 6
  %41 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %42, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %41, ptr %43, !tbaa !2
  %44 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %36, ptr addrspace(1) %44, i16 2, i16 100)
  %45 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %45, ptr %16, !tbaa !2
  %46 = addrspacecast ptr %15 to ptr addrspace(1)
  %47 = addrspacecast ptr %17 to ptr addrspace(1)
  %48 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 3, ptr %14, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 3, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %41, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %46, ptr addrspace(1) %47, ptr addrspace(1) %48, ptr addrspace(1) %51)
  %52 = load i8, ptr %15, !tbaa !2, !range !7
  %53 = icmp eq i8 %52, 0
  br i1 %53, label %b4, label %b3

b2:
  %54 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 4, ptr %12, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %26, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %54, ptr addrspace(1) %47, ptr addrspace(1) %48, ptr addrspace(1) %57)
  %58 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %26, ptr %60, !tbaa !2
  %61 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %58, ptr addrspace(1) %48, ptr addrspace(1) %47, ptr addrspace(1) %61)
  %62 = load i8, ptr %13
  %63 = getelementptr i8, ptr %13, i16 2
  %64 = load ptr addrspace(1), ptr %63
  %65 = load i8, ptr %11
  %66 = getelementptr i8, ptr %11, i16 2
  %67 = load ptr addrspace(1), ptr %66
  %68 = icmp eq i8 %62, 0
  br i1 %68, label %b8, label %b7

b3:
  %69 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %69)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %70 = getelementptr inbounds i8, ptr %15, i16 2
  %71 = load ptr addrspace(1), ptr %70, !tbaa !2
  %72 = load ptr, ptr addrspace(1) %71
  call addrspace(1) void @N$PS(ptr %72)
  %73 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %73)
  %74 = getelementptr i8, ptr addrspace(1) %71, i16 4
  %75 = load i16, ptr addrspace(1) %74
  call addrspace(1) void @N$PU2(i16 %75)
  %76 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  %77 = getelementptr i8, ptr addrspace(1) %71, i16 2
  %78 = load i16, ptr addrspace(1) %77
  call addrspace(1) void @N$PU2(i16 %78)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %79 = addrspacecast ptr %9 to ptr addrspace(5)
  %80 = addrspacecast ptr %9 to ptr addrspace(1)
  %81 = load ptr, ptr addrspace(1) %47
  %82 = getelementptr i8, ptr %81, i16 -4
  %83 = load i16, ptr %82
  br label %84

84:
  %85 = phi i16 [ 0, %b6 ], [ %88, %87 ]
  %86 = icmp ult i16 %85, %83
  br i1 %86, label %93, label %105

87:
  %88 = add i16 %85, 1
  br label %84

89:
  %90 = phi i16 [ %106, %105 ], [ %108, %107 ]
  %91 = addrspacecast ptr %81 to ptr addrspace(1)
  %92 = icmp ule i16 %90, %83
  br i1 %92, label %99, label %104

93:
  %94 = mul i16 %85, 6
  %95 = getelementptr inbounds i8, ptr %81, i16 %94
  %96 = getelementptr i8, ptr %95, i16 2
  %97 = load i16, ptr %96
  %98 = icmp ule i16 %97, 20
  br i1 %98, label %87, label %107

99:
  store i16 %90, ptr addrspace(1) %80
  %100 = getelementptr i8, ptr addrspace(1) %80, i16 2
  store i16 %90, ptr addrspace(1) %100
  %101 = getelementptr i8, ptr addrspace(1) %80, i16 4
  store ptr addrspace(1) %91, ptr addrspace(1) %101
  %102 = addrspacecast ptr %8 to ptr addrspace(1)
  %103 = icmp ne i16 %90, 0
  br i1 %103, label %b11, label %b12

104:
  call addrspace(1) void @N$EBND()
  unreachable

105:
  %106 = phi i16 [ %85, %84 ]
  br label %89

107:
  %108 = phi i16 [ %85, %93 ]
  br label %89

b7:
  %109 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %109)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %110 = icmp eq i8 %65, 0
  br i1 %110, label %111, label %b7

111:
  %112 = getelementptr i8, ptr addrspace(1) %64, i16 2
  %113 = load i16, ptr addrspace(1) %112
  %114 = getelementptr i8, ptr addrspace(1) %67, i16 2
  %115 = load i16, ptr addrspace(1) %114
  %116 = icmp ule i16 %113, %115
  br i1 %116, label %117, label %118

117:
  br label %119

118:
  br label %119

119:
  %120 = phi ptr addrspace(1) [ %112, %117 ], [ %114, %118 ]
  %121 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %121)
  %122 = load i16, ptr addrspace(1) %120
  call addrspace(1) void @N$PU2(i16 %122)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %123 = getelementptr i8, ptr addrspace(5) %79, i16 4
  %124 = getelementptr inbounds i8, ptr addrspace(1) %91, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %102, ptr addrspace(1) %124)
  call addrspace(1) void @N$PU2(i16 %90)
  %125 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %125)
  call addrspace(1) void @N$PV(ptr addrspace(1) %102)
  call addrspace(1) void @N$PN()
  %126 = load ptr, ptr %17, !tbaa !2
  %127 = getelementptr i8, ptr %126, i16 -4
  %128 = load i16, ptr %127
  %129 = getelementptr i8, ptr @$str12, i16 6
  %130 = getelementptr i8, ptr @$str13, i16 6
  %131 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %132 = phi i16 [ 0, %b11 ], [ %139, %b16 ]
  %133 = icmp ult i16 %132, %128
  br i1 %133, label %b15, label %b17

b15:
  %134 = mul i16 %132, 6
  %135 = getelementptr inbounds i8, ptr %126, i16 %134
  %136 = getelementptr i8, ptr %135, i16 4
  %137 = load i16, ptr %136
  %138 = icmp ult i16 %137, 5
  br i1 %138, label %b18, label %b16

b16:
  %139 = add i16 %132, 1
  br label %b14

b17:
  %140 = getelementptr i8, ptr @$str15, i16 6
  %141 = addrspacecast ptr %140 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %142 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %142, !tbaa !2
  %143 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %141, ptr %143, !tbaa !2
  %144 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %47, ptr addrspace(1) %144, i16 1, i16 500)
  %145 = load ptr, ptr %17, !tbaa !2
  %146 = getelementptr i8, ptr %145, i16 -4
  %147 = load i16, ptr %146
  call addrspace(1) void @N$PU2(i16 %147)
  %148 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %148)
  call addrspace(1) void @N$PN()
  %149 = load ptr, ptr %16, !tbaa !2
  %150 = icmp ne ptr %149, null
  br i1 %150, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %129)
  %151 = load ptr, ptr %135
  call addrspace(1) void @N$PS(ptr %151)
  call addrspace(1) void @N$PS(ptr %130)
  %152 = load i16, ptr %136
  call addrspace(1) void @N$PU2(i16 %152)
  call addrspace(1) void @N$PS(ptr %131)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %149)
  %153 = load ptr, ptr %17, !tbaa !2
  %154 = icmp ne ptr %153, null
  br i1 %154, label %b28, label %b27

b23:
  %155 = getelementptr i8, ptr %149, i16 -4
  %156 = load i16, ptr %155
  br label %b24

b24:
  %157 = phi i16 [ 0, %b23 ], [ %162, %b26 ]
  %158 = icmp ult i16 %157, %156
  br i1 %158, label %b26, label %b25

b25:
  br label %b22

b26:
  %159 = mul i16 %157, 6
  %160 = getelementptr inbounds i8, ptr %149, i16 %159
  %161 = load ptr, ptr %160
  call addrspace(1) void @N$BDRP(ptr %161)
  %162 = add i16 %157, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %153)
  ret i16 0

b28:
  %163 = getelementptr i8, ptr %153, i16 -4
  %164 = load i16, ptr %163
  br label %b29

b29:
  %165 = phi i16 [ 0, %b28 ], [ %170, %b31 ]
  %166 = icmp ult i16 %165, %164
  br i1 %166, label %b31, label %b30

b30:
  br label %b27

b31:
  %167 = mul i16 %165, 6
  %168 = getelementptr inbounds i8, ptr %153, i16 %167
  %169 = load ptr, ptr %168
  call addrspace(1) void @N$BDRP(ptr %169)
  %170 = add i16 %165, 1
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
