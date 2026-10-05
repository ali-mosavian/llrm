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
  %10 = alloca [6 x i8]
  %11 = alloca [8 x i8]
  %12 = alloca [6 x i8]
  %13 = alloca [8 x i8]
  %14 = alloca [6 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = getelementptr i8, ptr @$str1, i16 6
  store ptr %17, ptr %6, !tbaa !2
  %18 = addrspacecast ptr %6 to ptr addrspace(1)
  %19 = getelementptr i8, ptr @$str2, i16 6
  %20 = addrspacecast ptr %19 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %21 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %21, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %20, ptr %22, !tbaa !2
  %23 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %18, ptr addrspace(1) %23, i16 5, i16 40)
  %24 = getelementptr i8, ptr @$str3, i16 6
  %25 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 4, ptr %4, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %25, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %18, ptr addrspace(1) %28, i16 30, i16 3)
  %29 = getelementptr i8, ptr @$str4, i16 6
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %30, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %18, ptr addrspace(1) %33, i16 12, i16 0)
  %34 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %34, ptr %16, !tbaa !2
  store ptr %17, ptr %2, !tbaa !2
  %35 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %25, ptr %37, !tbaa !2
  %38 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %35, ptr addrspace(1) %38, i16 28, i16 9)
  %39 = getelementptr i8, ptr @$str5, i16 6
  %40 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %35, ptr addrspace(1) %43, i16 2, i16 100)
  %44 = load ptr, ptr %2, !tbaa !2
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %44, ptr %15, !tbaa !2
  %45 = addrspacecast ptr %14 to ptr addrspace(1)
  %46 = addrspacecast ptr %16 to ptr addrspace(5)
  %47 = addrspacecast ptr %16 to ptr addrspace(1)
  %48 = addrspacecast ptr %15 to ptr addrspace(1)
  store i16 3, ptr %13, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 3, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %40, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %45, ptr addrspace(1) %47, ptr addrspace(1) %48, ptr addrspace(1) %51)
  %52 = load i8, ptr %14, !tbaa !2, !range !7
  %53 = icmp eq i8 %52, 0
  br i1 %53, label %b4, label %b3

b2:
  %54 = addrspacecast ptr %12 to ptr addrspace(1)
  store i16 4, ptr %11, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 4, ptr %55, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %25, ptr %56, !tbaa !2
  %57 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %54, ptr addrspace(1) %47, ptr addrspace(1) %48, ptr addrspace(1) %57)
  %58 = addrspacecast ptr %10 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %25, ptr %60, !tbaa !2
  %61 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %58, ptr addrspace(1) %48, ptr addrspace(1) %47, ptr addrspace(1) %61)
  %62 = load i8, ptr %12
  %63 = getelementptr i8, ptr %12, i16 2
  %64 = load ptr addrspace(1), ptr %63
  %65 = load i8, ptr %10
  %66 = getelementptr i8, ptr %10, i16 2
  %67 = load ptr addrspace(1), ptr %66
  %68 = icmp eq i8 %62, 0
  br i1 %68, label %b8, label %b7

b3:
  %69 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %69)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %70 = getelementptr inbounds i8, ptr %14, i16 2
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
  %79 = load ptr, ptr addrspace(5) %46
  %80 = getelementptr i8, ptr %79, i16 -4
  %81 = load i16, ptr %80
  br label %82

82:
  %83 = phi i16 [ 0, %b6 ], [ %86, %85 ]
  %84 = icmp ult i16 %83, %81
  br i1 %84, label %91, label %102

85:
  %86 = add i16 %83, 1
  br label %82

87:
  %88 = phi i16 [ %103, %102 ], [ %105, %104 ]
  %89 = addrspacecast ptr %79 to ptr addrspace(1)
  %90 = icmp ule i16 %88, %81
  br i1 %90, label %97, label %101

91:
  %92 = mul i16 %83, 6
  %93 = getelementptr inbounds i8, ptr %79, i16 %92
  %94 = getelementptr i8, ptr %93, i16 2
  %95 = load i16, ptr %94
  %96 = icmp ule i16 %95, 20
  br i1 %96, label %85, label %104

97:
  %98 = addrspacecast ptr %8 to ptr addrspace(5)
  %99 = addrspacecast ptr %8 to ptr addrspace(1)
  %100 = icmp ne i16 %88, 0
  br i1 %100, label %b11, label %b12

101:
  call addrspace(1) void @N$EBND()
  unreachable

102:
  %103 = phi i16 [ %83, %82 ]
  br label %87

104:
  %105 = phi i16 [ %83, %91 ]
  br label %87

b7:
  %106 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %106)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %107 = icmp eq i8 %65, 0
  br i1 %107, label %108, label %b7

108:
  %109 = getelementptr i8, ptr addrspace(1) %64, i16 2
  %110 = load i16, ptr addrspace(1) %109
  %111 = getelementptr i8, ptr addrspace(1) %67, i16 2
  %112 = load i16, ptr addrspace(1) %111
  %113 = icmp ule i16 %110, %112
  br i1 %113, label %114, label %115

114:
  br label %116

115:
  br label %116

116:
  %117 = phi ptr addrspace(1) [ %109, %114 ], [ %111, %115 ]
  %118 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %118)
  %119 = load i16, ptr addrspace(1) %117
  call addrspace(1) void @N$PU2(i16 %119)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %120 = getelementptr inbounds i8, ptr %79, i16 0
  %121 = getelementptr inbounds i8, ptr addrspace(1) %89, i16 0
  %122 = load ptr, ptr %120
  %123 = getelementptr i8, ptr %122, i16 -4
  %124 = load i16, ptr %123
  %125 = addrspacecast ptr %122 to ptr addrspace(1)
  %126 = icmp uge i16 %124, 1
  br i1 %126, label %127, label %139

127:
  store i16 1, ptr addrspace(5) %98
  %128 = getelementptr i8, ptr addrspace(5) %98, i16 2
  %129 = getelementptr i8, ptr addrspace(1) %99, i16 2
  store i16 1, ptr addrspace(5) %128
  %130 = getelementptr i8, ptr addrspace(5) %98, i16 4
  %131 = getelementptr i8, ptr addrspace(1) %99, i16 4
  store ptr addrspace(1) %125, ptr addrspace(5) %130
  call addrspace(1) void @N$PU2(i16 %88)
  %132 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %132)
  call addrspace(1) void @N$PV(ptr addrspace(1) %99)
  call addrspace(1) void @N$PN()
  %133 = load ptr, ptr %16, !tbaa !2
  %134 = getelementptr i8, ptr %133, i16 -4
  %135 = load i16, ptr %134
  %136 = getelementptr i8, ptr @$str12, i16 6
  %137 = getelementptr i8, ptr @$str13, i16 6
  %138 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

139:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %140 = phi i16 [ 0, %127 ], [ %147, %b16 ]
  %141 = icmp ult i16 %140, %135
  br i1 %141, label %b15, label %b17

b15:
  %142 = mul i16 %140, 6
  %143 = getelementptr inbounds i8, ptr %133, i16 %142
  %144 = getelementptr i8, ptr %143, i16 4
  %145 = load i16, ptr %144
  %146 = icmp ult i16 %145, 5
  br i1 %146, label %b18, label %b16

b16:
  %147 = add i16 %140, 1
  br label %b14

b17:
  %148 = getelementptr i8, ptr @$str15, i16 6
  %149 = addrspacecast ptr %148 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %150 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %150, !tbaa !2
  %151 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %149, ptr %151, !tbaa !2
  %152 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %47, ptr addrspace(1) %152, i16 1, i16 500)
  %153 = load ptr, ptr %16, !tbaa !2
  %154 = getelementptr i8, ptr %153, i16 -4
  %155 = load i16, ptr %154
  call addrspace(1) void @N$PU2(i16 %155)
  %156 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %156)
  call addrspace(1) void @N$PN()
  %157 = load ptr, ptr %15, !tbaa !2
  %158 = icmp ne ptr %157, null
  br i1 %158, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %136)
  %159 = load ptr, ptr %143
  call addrspace(1) void @N$PS(ptr %159)
  call addrspace(1) void @N$PS(ptr %137)
  %160 = load i16, ptr %144
  call addrspace(1) void @N$PU2(i16 %160)
  call addrspace(1) void @N$PS(ptr %138)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %157)
  %161 = load ptr, ptr %16, !tbaa !2
  %162 = icmp ne ptr %161, null
  br i1 %162, label %b28, label %b27

b23:
  %163 = getelementptr i8, ptr %157, i16 -4
  %164 = load i16, ptr %163
  br label %b24

b24:
  %165 = phi i16 [ 0, %b23 ], [ %170, %b26 ]
  %166 = icmp ult i16 %165, %164
  br i1 %166, label %b26, label %b25

b25:
  br label %b22

b26:
  %167 = mul i16 %165, 6
  %168 = getelementptr inbounds i8, ptr %157, i16 %167
  %169 = load ptr, ptr %168
  call addrspace(1) void @N$BDRP(ptr %169)
  %170 = add i16 %165, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %161)
  ret i16 0

b28:
  %171 = getelementptr i8, ptr %161, i16 -4
  %172 = load i16, ptr %171
  br label %b29

b29:
  %173 = phi i16 [ 0, %b28 ], [ %178, %b31 ]
  %174 = icmp ult i16 %173, %172
  br i1 %174, label %b31, label %b30

b30:
  br label %b27

b31:
  %175 = mul i16 %173, 6
  %176 = getelementptr inbounds i8, ptr %161, i16 %175
  %177 = load ptr, ptr %176
  call addrspace(1) void @N$BDRP(ptr %177)
  %178 = add i16 %173, 1
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
