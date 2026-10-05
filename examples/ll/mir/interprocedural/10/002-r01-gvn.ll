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

declare internal void @Catalog.add(ptr addrspace(5), ptr addrspace(5), i16, i16) addrspace(1) nearcode

declare internal void @north(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @south(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @find(ptr addrspace(5), ptr addrspace(5), ptr addrspace(5), ptr addrspace(5)) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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
  %10 = alloca [6 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca [6 x i8]
  %13 = alloca [8 x i8]
  %14 = alloca [6 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = getelementptr i8, ptr @$str1, i16 6
  store ptr %17, ptr %6, !tbaa !2
  %18 = addrspacecast ptr %6 to ptr addrspace(5)
  %19 = addrspacecast ptr %6 to ptr addrspace(1)
  %20 = getelementptr i8, ptr @$str2, i16 6
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %5 to ptr addrspace(5)
  %25 = addrspacecast ptr %5 to ptr addrspace(1)
  %26 = addrspacecast ptr addrspace(1) %25 to ptr addrspace(5)
  %27 = addrspacecast ptr addrspace(1) %19 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %24, i16 5, i16 40)
  %28 = getelementptr i8, ptr @$str3, i16 6
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 4, ptr %4, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %4 to ptr addrspace(5)
  %33 = addrspacecast ptr %4 to ptr addrspace(1)
  %34 = addrspacecast ptr addrspace(1) %33 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %32, i16 30, i16 3)
  %35 = getelementptr i8, ptr @$str4, i16 6
  %36 = addrspacecast ptr %35 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %36, ptr %38, !tbaa !2
  %39 = addrspacecast ptr %3 to ptr addrspace(5)
  %40 = addrspacecast ptr %3 to ptr addrspace(1)
  %41 = addrspacecast ptr addrspace(1) %40 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %39, i16 12, i16 0)
  %42 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %42, ptr %16, !tbaa !2
  store ptr %17, ptr %2, !tbaa !2
  %43 = addrspacecast ptr %2 to ptr addrspace(5)
  %44 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %29, ptr %46, !tbaa !2
  %47 = addrspacecast ptr %1 to ptr addrspace(5)
  %48 = addrspacecast ptr %1 to ptr addrspace(1)
  %49 = addrspacecast ptr addrspace(1) %48 to ptr addrspace(5)
  %50 = addrspacecast ptr addrspace(1) %44 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %43, ptr addrspace(5) %47, i16 28, i16 9)
  %51 = getelementptr i8, ptr @$str5, i16 6
  %52 = addrspacecast ptr %51 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %52, ptr %54, !tbaa !2
  %55 = addrspacecast ptr %0 to ptr addrspace(5)
  %56 = addrspacecast ptr %0 to ptr addrspace(1)
  %57 = addrspacecast ptr addrspace(1) %56 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %43, ptr addrspace(5) %55, i16 2, i16 100)
  %58 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %58, ptr %15, !tbaa !2
  %59 = addrspacecast ptr %14 to ptr addrspace(5)
  %60 = addrspacecast ptr %14 to ptr addrspace(1)
  %61 = addrspacecast ptr %16 to ptr addrspace(5)
  %62 = addrspacecast ptr %16 to ptr addrspace(1)
  %63 = addrspacecast ptr %15 to ptr addrspace(5)
  %64 = addrspacecast ptr %15 to ptr addrspace(1)
  store i16 3, ptr %13, !tbaa !2
  %65 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 3, ptr %65, !tbaa !2
  %66 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %52, ptr %66, !tbaa !2
  %67 = addrspacecast ptr %13 to ptr addrspace(5)
  %68 = addrspacecast ptr %13 to ptr addrspace(1)
  %69 = addrspacecast ptr addrspace(1) %68 to ptr addrspace(5)
  %70 = addrspacecast ptr addrspace(1) %64 to ptr addrspace(5)
  %71 = addrspacecast ptr addrspace(1) %62 to ptr addrspace(5)
  %72 = addrspacecast ptr addrspace(1) %60 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %59, ptr addrspace(5) %61, ptr addrspace(5) %63, ptr addrspace(5) %67)
  %73 = load i8, ptr %14, !tbaa !2, !range !7
  %74 = icmp eq i8 %73, 0
  br i1 %74, label %b4, label %b3

b2:
  %75 = addrspacecast ptr %12 to ptr addrspace(5)
  %76 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %11, !tbaa !2
  %77 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 4, ptr %77, !tbaa !2
  %78 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %29, ptr %78, !tbaa !2
  %79 = addrspacecast ptr %11 to ptr addrspace(5)
  %80 = addrspacecast ptr %11 to ptr addrspace(1)
  %81 = addrspacecast ptr addrspace(1) %80 to ptr addrspace(5)
  %82 = addrspacecast ptr addrspace(1) %76 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %75, ptr addrspace(5) %61, ptr addrspace(5) %63, ptr addrspace(5) %79)
  %83 = addrspacecast ptr %10 to ptr addrspace(5)
  %84 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %85 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %85, !tbaa !2
  %86 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %29, ptr %86, !tbaa !2
  %87 = addrspacecast ptr %9 to ptr addrspace(5)
  %88 = addrspacecast ptr %9 to ptr addrspace(1)
  %89 = addrspacecast ptr addrspace(1) %88 to ptr addrspace(5)
  %90 = addrspacecast ptr addrspace(1) %84 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %83, ptr addrspace(5) %63, ptr addrspace(5) %61, ptr addrspace(5) %87)
  %91 = load i8, ptr %12
  %92 = getelementptr i8, ptr %12, i16 2
  %93 = load ptr addrspace(1), ptr %92
  %94 = load i8, ptr %10
  %95 = getelementptr i8, ptr %10, i16 2
  %96 = load ptr addrspace(1), ptr %95
  %97 = icmp eq i8 %91, 0
  br i1 %97, label %b8, label %b7

b3:
  %98 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %98)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %99 = getelementptr inbounds i8, ptr %14, i16 2
  %100 = load ptr addrspace(1), ptr %99, !tbaa !2
  %101 = load ptr, ptr addrspace(1) %100
  call addrspace(1) void @N$PS(ptr %101)
  %102 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %102)
  %103 = getelementptr i8, ptr addrspace(1) %100, i16 4
  %104 = load i16, ptr addrspace(1) %103
  call addrspace(1) void @N$PU2(i16 %104)
  %105 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %105)
  %106 = getelementptr i8, ptr addrspace(1) %100, i16 2
  %107 = load i16, ptr addrspace(1) %106
  call addrspace(1) void @N$PU2(i16 %107)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %108 = load ptr, ptr addrspace(5) %61
  %109 = getelementptr i8, ptr %108, i16 -4
  %110 = load i16, ptr %109
  br label %111

111:
  %112 = phi i16 [ 0, %b6 ], [ %115, %114 ]
  %113 = icmp ult i16 %112, %110
  br i1 %113, label %119, label %130

114:
  %115 = add i16 %112, 1
  br label %111

116:
  %117 = phi i16 [ %131, %130 ], [ %133, %132 ]
  %118 = icmp ule i16 %117, %110
  br i1 %118, label %125, label %129

119:
  %120 = mul i16 %112, 6
  %121 = getelementptr inbounds i8, ptr %108, i16 %120
  %122 = getelementptr i8, ptr %121, i16 2
  %123 = load i16, ptr %122
  %124 = icmp ule i16 %123, 20
  br i1 %124, label %114, label %132

125:
  %126 = addrspacecast ptr %8 to ptr addrspace(5)
  %127 = addrspacecast ptr %8 to ptr addrspace(1)
  %128 = icmp ne i16 %117, 0
  br i1 %128, label %b11, label %b12

129:
  call addrspace(1) void @N$EBND()
  unreachable

130:
  %131 = phi i16 [ %112, %111 ]
  br label %116

132:
  %133 = phi i16 [ %112, %119 ]
  br label %116

b7:
  %134 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %134)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %135 = icmp eq i8 %94, 0
  br i1 %135, label %136, label %b7

136:
  %137 = getelementptr i8, ptr addrspace(1) %93, i16 2
  %138 = load i16, ptr addrspace(1) %137
  %139 = getelementptr i8, ptr addrspace(1) %96, i16 2
  %140 = load i16, ptr addrspace(1) %139
  %141 = icmp ule i16 %138, %140
  br i1 %141, label %142, label %143

142:
  br label %144

143:
  br label %144

144:
  %145 = phi ptr addrspace(1) [ %137, %142 ], [ %139, %143 ]
  %146 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %146)
  %147 = load i16, ptr addrspace(1) %145
  call addrspace(1) void @N$PU2(i16 %147)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %148 = getelementptr inbounds i8, ptr %108, i16 0
  %149 = load ptr, ptr %148
  %150 = getelementptr i8, ptr %149, i16 -4
  %151 = load i16, ptr %150
  %152 = addrspacecast ptr %149 to ptr addrspace(1)
  %153 = icmp uge i16 %151, 1
  br i1 %153, label %154, label %164

154:
  store i16 1, ptr addrspace(5) %126
  %155 = getelementptr i8, ptr addrspace(5) %126, i16 2
  store i16 1, ptr addrspace(5) %155
  %156 = getelementptr i8, ptr addrspace(5) %126, i16 4
  store ptr addrspace(1) %152, ptr addrspace(5) %156
  call addrspace(1) void @N$PU2(i16 %117)
  %157 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %157)
  call addrspace(1) void @N$PV(ptr addrspace(1) %127)
  call addrspace(1) void @N$PN()
  %158 = load ptr, ptr %16, !tbaa !2
  %159 = getelementptr i8, ptr %158, i16 -4
  %160 = load i16, ptr %159
  %161 = getelementptr i8, ptr @$str12, i16 6
  %162 = getelementptr i8, ptr @$str13, i16 6
  %163 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

164:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %165 = phi i16 [ 0, %154 ], [ %172, %b16 ]
  %166 = icmp ult i16 %165, %160
  br i1 %166, label %b15, label %b17

b15:
  %167 = mul i16 %165, 6
  %168 = getelementptr inbounds i8, ptr %158, i16 %167
  %169 = getelementptr i8, ptr %168, i16 4
  %170 = load i16, ptr %169
  %171 = icmp ult i16 %170, 5
  br i1 %171, label %b18, label %b16

b16:
  %172 = add i16 %165, 1
  br label %b14

b17:
  %173 = getelementptr i8, ptr @$str15, i16 6
  %174 = addrspacecast ptr %173 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %175 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %175, !tbaa !2
  %176 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %174, ptr %176, !tbaa !2
  %177 = addrspacecast ptr %7 to ptr addrspace(5)
  %178 = addrspacecast ptr %7 to ptr addrspace(1)
  %179 = addrspacecast ptr addrspace(1) %178 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %61, ptr addrspace(5) %177, i16 1, i16 500)
  %180 = load ptr, ptr %16, !tbaa !2
  %181 = getelementptr i8, ptr %180, i16 -4
  %182 = load i16, ptr %181
  call addrspace(1) void @N$PU2(i16 %182)
  %183 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %183)
  call addrspace(1) void @N$PN()
  %184 = load ptr, ptr %15, !tbaa !2
  %185 = icmp ne ptr %184, null
  br i1 %185, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %161)
  %186 = load ptr, ptr %168
  call addrspace(1) void @N$PS(ptr %186)
  call addrspace(1) void @N$PS(ptr %162)
  %187 = load i16, ptr %169
  call addrspace(1) void @N$PU2(i16 %187)
  call addrspace(1) void @N$PS(ptr %163)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %184)
  %188 = load ptr, ptr %16, !tbaa !2
  %189 = icmp ne ptr %188, null
  br i1 %189, label %b28, label %b27

b23:
  %190 = getelementptr i8, ptr %184, i16 -4
  %191 = load i16, ptr %190
  br label %b24

b24:
  %192 = phi i16 [ 0, %b23 ], [ %197, %b26 ]
  %193 = icmp ult i16 %192, %191
  br i1 %193, label %b26, label %b25

b25:
  br label %b22

b26:
  %194 = mul i16 %192, 6
  %195 = getelementptr inbounds i8, ptr %184, i16 %194
  %196 = load ptr, ptr %195
  call addrspace(1) void @N$BDRP(ptr %196)
  %197 = add i16 %192, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %188)
  ret i16 0

b28:
  %198 = getelementptr i8, ptr %188, i16 -4
  %199 = load i16, ptr %198
  br label %b29

b29:
  %200 = phi i16 [ 0, %b28 ], [ %205, %b31 ]
  %201 = icmp ult i16 %200, %199
  br i1 %201, label %b31, label %b30

b30:
  br label %b27

b31:
  %202 = mul i16 %200, 6
  %203 = getelementptr inbounds i8, ptr %188, i16 %202
  %204 = load ptr, ptr %203
  call addrspace(1) void @N$BDRP(ptr %204)
  %205 = add i16 %200, 1
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
