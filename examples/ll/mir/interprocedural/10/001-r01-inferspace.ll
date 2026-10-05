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
  %35 = addrspacecast ptr addrspace(1) %19 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %32, i16 30, i16 3)
  %36 = getelementptr i8, ptr @$str4, i16 6
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %37, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %3 to ptr addrspace(5)
  %41 = addrspacecast ptr %3 to ptr addrspace(1)
  %42 = addrspacecast ptr addrspace(1) %41 to ptr addrspace(5)
  %43 = addrspacecast ptr addrspace(1) %19 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %18, ptr addrspace(5) %40, i16 12, i16 0)
  %44 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %44, ptr %16, !tbaa !2
  store ptr %17, ptr %2, !tbaa !2
  %45 = addrspacecast ptr %2 to ptr addrspace(5)
  %46 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %47 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %47, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %29, ptr %48, !tbaa !2
  %49 = addrspacecast ptr %1 to ptr addrspace(5)
  %50 = addrspacecast ptr %1 to ptr addrspace(1)
  %51 = addrspacecast ptr addrspace(1) %50 to ptr addrspace(5)
  %52 = addrspacecast ptr addrspace(1) %46 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %45, ptr addrspace(5) %49, i16 28, i16 9)
  %53 = getelementptr i8, ptr @$str5, i16 6
  %54 = addrspacecast ptr %53 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %54, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %0 to ptr addrspace(5)
  %58 = addrspacecast ptr %0 to ptr addrspace(1)
  %59 = addrspacecast ptr addrspace(1) %58 to ptr addrspace(5)
  %60 = addrspacecast ptr addrspace(1) %46 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %45, ptr addrspace(5) %57, i16 2, i16 100)
  %61 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %61, ptr %15, !tbaa !2
  %62 = addrspacecast ptr %14 to ptr addrspace(5)
  %63 = addrspacecast ptr %14 to ptr addrspace(1)
  %64 = addrspacecast ptr %16 to ptr addrspace(5)
  %65 = addrspacecast ptr %16 to ptr addrspace(5)
  %66 = addrspacecast ptr %16 to ptr addrspace(1)
  %67 = addrspacecast ptr %15 to ptr addrspace(5)
  %68 = addrspacecast ptr %15 to ptr addrspace(1)
  store i16 3, ptr %13, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 3, ptr %69, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %54, ptr %70, !tbaa !2
  %71 = addrspacecast ptr %13 to ptr addrspace(5)
  %72 = addrspacecast ptr %13 to ptr addrspace(1)
  %73 = addrspacecast ptr addrspace(1) %72 to ptr addrspace(5)
  %74 = addrspacecast ptr addrspace(1) %68 to ptr addrspace(5)
  %75 = addrspacecast ptr addrspace(1) %66 to ptr addrspace(5)
  %76 = addrspacecast ptr addrspace(1) %63 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %62, ptr addrspace(5) %65, ptr addrspace(5) %67, ptr addrspace(5) %71)
  %77 = load i8, ptr %14, !tbaa !2, !range !7
  %78 = icmp eq i8 %77, 0
  br i1 %78, label %b4, label %b3

b2:
  %79 = addrspacecast ptr %12 to ptr addrspace(5)
  %80 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %11, !tbaa !2
  %81 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 4, ptr %81, !tbaa !2
  %82 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %29, ptr %82, !tbaa !2
  %83 = addrspacecast ptr %11 to ptr addrspace(5)
  %84 = addrspacecast ptr %11 to ptr addrspace(1)
  %85 = addrspacecast ptr addrspace(1) %84 to ptr addrspace(5)
  %86 = addrspacecast ptr addrspace(1) %68 to ptr addrspace(5)
  %87 = addrspacecast ptr addrspace(1) %66 to ptr addrspace(5)
  %88 = addrspacecast ptr addrspace(1) %80 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %79, ptr addrspace(5) %65, ptr addrspace(5) %67, ptr addrspace(5) %83)
  %89 = addrspacecast ptr %10 to ptr addrspace(5)
  %90 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %91 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %91, !tbaa !2
  %92 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %29, ptr %92, !tbaa !2
  %93 = addrspacecast ptr %9 to ptr addrspace(5)
  %94 = addrspacecast ptr %9 to ptr addrspace(1)
  %95 = addrspacecast ptr addrspace(1) %94 to ptr addrspace(5)
  %96 = addrspacecast ptr addrspace(1) %66 to ptr addrspace(5)
  %97 = addrspacecast ptr addrspace(1) %68 to ptr addrspace(5)
  %98 = addrspacecast ptr addrspace(1) %90 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %89, ptr addrspace(5) %67, ptr addrspace(5) %65, ptr addrspace(5) %93)
  %99 = load i8, ptr %12
  %100 = getelementptr i8, ptr %12, i16 2
  %101 = load ptr addrspace(1), ptr %100
  %102 = load i8, ptr %10
  %103 = getelementptr i8, ptr %10, i16 2
  %104 = load ptr addrspace(1), ptr %103
  %105 = icmp eq i8 %99, 0
  br i1 %105, label %b8, label %b7

b3:
  %106 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %106)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %107 = getelementptr inbounds i8, ptr %14, i16 2
  %108 = load ptr addrspace(1), ptr %107, !tbaa !2
  %109 = load ptr, ptr addrspace(1) %108
  call addrspace(1) void @N$PS(ptr %109)
  %110 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %110)
  %111 = getelementptr i8, ptr addrspace(1) %108, i16 4
  %112 = load i16, ptr addrspace(1) %111
  call addrspace(1) void @N$PU2(i16 %112)
  %113 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %113)
  %114 = getelementptr i8, ptr addrspace(1) %108, i16 2
  %115 = load i16, ptr addrspace(1) %114
  call addrspace(1) void @N$PU2(i16 %115)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %116 = load ptr, ptr addrspace(5) %64
  %117 = getelementptr i8, ptr %116, i16 -4
  %118 = load i16, ptr %117
  br label %119

119:
  %120 = phi i16 [ 0, %b6 ], [ %123, %122 ]
  %121 = icmp ult i16 %120, %118
  br i1 %121, label %127, label %138

122:
  %123 = add i16 %120, 1
  br label %119

124:
  %125 = phi i16 [ %139, %138 ], [ %141, %140 ]
  %126 = icmp ule i16 %125, %118
  br i1 %126, label %133, label %137

127:
  %128 = mul i16 %120, 6
  %129 = getelementptr inbounds i8, ptr %116, i16 %128
  %130 = getelementptr i8, ptr %129, i16 2
  %131 = load i16, ptr %130
  %132 = icmp ule i16 %131, 20
  br i1 %132, label %122, label %140

133:
  %134 = addrspacecast ptr %8 to ptr addrspace(5)
  %135 = addrspacecast ptr %8 to ptr addrspace(1)
  %136 = icmp ne i16 %125, 0
  br i1 %136, label %b11, label %b12

137:
  call addrspace(1) void @N$EBND()
  unreachable

138:
  %139 = phi i16 [ %120, %119 ]
  br label %124

140:
  %141 = phi i16 [ %120, %127 ]
  br label %124

b7:
  %142 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %142)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %143 = icmp eq i8 %102, 0
  br i1 %143, label %144, label %b7

144:
  %145 = getelementptr i8, ptr addrspace(1) %101, i16 2
  %146 = load i16, ptr addrspace(1) %145
  %147 = getelementptr i8, ptr addrspace(1) %104, i16 2
  %148 = load i16, ptr addrspace(1) %147
  %149 = icmp ule i16 %146, %148
  br i1 %149, label %150, label %151

150:
  br label %152

151:
  br label %152

152:
  %153 = phi ptr addrspace(1) [ %145, %150 ], [ %147, %151 ]
  %154 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %154)
  %155 = load i16, ptr addrspace(1) %153
  call addrspace(1) void @N$PU2(i16 %155)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %156 = getelementptr inbounds i8, ptr %116, i16 0
  %157 = load ptr, ptr %156
  %158 = getelementptr i8, ptr %157, i16 -4
  %159 = load i16, ptr %158
  %160 = addrspacecast ptr %157 to ptr addrspace(1)
  %161 = icmp uge i16 %159, 1
  br i1 %161, label %162, label %172

162:
  store i16 1, ptr addrspace(5) %134
  %163 = getelementptr i8, ptr addrspace(5) %134, i16 2
  store i16 1, ptr addrspace(5) %163
  %164 = getelementptr i8, ptr addrspace(5) %134, i16 4
  store ptr addrspace(1) %160, ptr addrspace(5) %164
  call addrspace(1) void @N$PU2(i16 %125)
  %165 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %165)
  call addrspace(1) void @N$PV(ptr addrspace(1) %135)
  call addrspace(1) void @N$PN()
  %166 = load ptr, ptr %16, !tbaa !2
  %167 = getelementptr i8, ptr %166, i16 -4
  %168 = load i16, ptr %167
  %169 = getelementptr i8, ptr @$str12, i16 6
  %170 = getelementptr i8, ptr @$str13, i16 6
  %171 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

172:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %173 = phi i16 [ 0, %162 ], [ %180, %b16 ]
  %174 = icmp ult i16 %173, %168
  br i1 %174, label %b15, label %b17

b15:
  %175 = mul i16 %173, 6
  %176 = getelementptr inbounds i8, ptr %166, i16 %175
  %177 = getelementptr i8, ptr %176, i16 4
  %178 = load i16, ptr %177
  %179 = icmp ult i16 %178, 5
  br i1 %179, label %b18, label %b16

b16:
  %180 = add i16 %173, 1
  br label %b14

b17:
  %181 = getelementptr i8, ptr @$str15, i16 6
  %182 = addrspacecast ptr %181 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %183 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %183, !tbaa !2
  %184 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %182, ptr %184, !tbaa !2
  %185 = addrspacecast ptr %7 to ptr addrspace(5)
  %186 = addrspacecast ptr %7 to ptr addrspace(1)
  %187 = addrspacecast ptr addrspace(1) %186 to ptr addrspace(5)
  %188 = addrspacecast ptr addrspace(1) %66 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %65, ptr addrspace(5) %185, i16 1, i16 500)
  %189 = load ptr, ptr %16, !tbaa !2
  %190 = getelementptr i8, ptr %189, i16 -4
  %191 = load i16, ptr %190
  call addrspace(1) void @N$PU2(i16 %191)
  %192 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %192)
  call addrspace(1) void @N$PN()
  %193 = load ptr, ptr %15, !tbaa !2
  %194 = icmp ne ptr %193, null
  br i1 %194, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %169)
  %195 = load ptr, ptr %176
  call addrspace(1) void @N$PS(ptr %195)
  call addrspace(1) void @N$PS(ptr %170)
  %196 = load i16, ptr %177
  call addrspace(1) void @N$PU2(i16 %196)
  call addrspace(1) void @N$PS(ptr %171)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %193)
  %197 = load ptr, ptr %16, !tbaa !2
  %198 = icmp ne ptr %197, null
  br i1 %198, label %b28, label %b27

b23:
  %199 = getelementptr i8, ptr %193, i16 -4
  %200 = load i16, ptr %199
  br label %b24

b24:
  %201 = phi i16 [ 0, %b23 ], [ %206, %b26 ]
  %202 = icmp ult i16 %201, %200
  br i1 %202, label %b26, label %b25

b25:
  br label %b22

b26:
  %203 = mul i16 %201, 6
  %204 = getelementptr inbounds i8, ptr %193, i16 %203
  %205 = load ptr, ptr %204
  call addrspace(1) void @N$BDRP(ptr %205)
  %206 = add i16 %201, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %197)
  ret i16 0

b28:
  %207 = getelementptr i8, ptr %197, i16 -4
  %208 = load i16, ptr %207
  br label %b29

b29:
  %209 = phi i16 [ 0, %b28 ], [ %214, %b31 ]
  %210 = icmp ult i16 %209, %208
  br i1 %210, label %b31, label %b30

b30:
  br label %b27

b31:
  %211 = mul i16 %209, 6
  %212 = getelementptr inbounds i8, ptr %197, i16 %211
  %213 = load ptr, ptr %212
  call addrspace(1) void @N$BDRP(ptr %213)
  %214 = add i16 %209, 1
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
