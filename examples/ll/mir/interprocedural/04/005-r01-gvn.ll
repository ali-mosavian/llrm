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
  %47 = addrspacecast ptr %17 to ptr addrspace(5)
  %48 = addrspacecast ptr %17 to ptr addrspace(1)
  %49 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 3, ptr %14, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 3, ptr %50, !tbaa !2
  %51 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %41, ptr %51, !tbaa !2
  %52 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %46, ptr addrspace(1) %48, ptr addrspace(1) %49, ptr addrspace(1) %52)
  %53 = load i8, ptr %15, !tbaa !2, !range !7
  %54 = icmp eq i8 %53, 0
  br i1 %54, label %b4, label %b3

b2:
  %55 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 4, ptr %12, !tbaa !2
  %56 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %56, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %26, ptr %57, !tbaa !2
  %58 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %55, ptr addrspace(1) %48, ptr addrspace(1) %49, ptr addrspace(1) %58)
  %59 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %60, !tbaa !2
  %61 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %26, ptr %61, !tbaa !2
  %62 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %59, ptr addrspace(1) %49, ptr addrspace(1) %48, ptr addrspace(1) %62)
  %63 = load i8, ptr %13
  %64 = getelementptr i8, ptr %13, i16 2
  %65 = load ptr addrspace(1), ptr %64
  %66 = load i8, ptr %11
  %67 = getelementptr i8, ptr %11, i16 2
  %68 = load ptr addrspace(1), ptr %67
  %69 = icmp eq i8 %63, 0
  br i1 %69, label %b8, label %b7

b3:
  %70 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %70)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %71 = getelementptr inbounds i8, ptr %15, i16 2
  %72 = load ptr addrspace(1), ptr %71, !tbaa !2
  %73 = load ptr, ptr addrspace(1) %72
  call addrspace(1) void @N$PS(ptr %73)
  %74 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %74)
  %75 = getelementptr i8, ptr addrspace(1) %72, i16 4
  %76 = load i16, ptr addrspace(1) %75
  call addrspace(1) void @N$PU2(i16 %76)
  %77 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %77)
  %78 = getelementptr i8, ptr addrspace(1) %72, i16 2
  %79 = load i16, ptr addrspace(1) %78
  call addrspace(1) void @N$PU2(i16 %79)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %80 = addrspacecast ptr %9 to ptr addrspace(5)
  %81 = addrspacecast ptr %9 to ptr addrspace(1)
  %82 = load ptr, ptr addrspace(5) %47
  %83 = getelementptr i8, ptr %82, i16 -4
  %84 = load i16, ptr %83
  br label %85

85:
  %86 = phi i16 [ 0, %b6 ], [ %89, %88 ]
  %87 = icmp ult i16 %86, %84
  br i1 %87, label %94, label %108

88:
  %89 = add i16 %86, 1
  br label %85

90:
  %91 = phi i16 [ %109, %108 ], [ %111, %110 ]
  %92 = addrspacecast ptr %82 to ptr addrspace(1)
  %93 = icmp ule i16 %91, %84
  br i1 %93, label %100, label %107

94:
  %95 = mul i16 %86, 6
  %96 = getelementptr inbounds i8, ptr %82, i16 %95
  %97 = getelementptr i8, ptr %96, i16 2
  %98 = load i16, ptr %97
  %99 = icmp ule i16 %98, 20
  br i1 %99, label %88, label %110

100:
  %101 = getelementptr i8, ptr addrspace(5) %80, i16 2
  %102 = getelementptr i8, ptr addrspace(1) %81, i16 2
  %103 = getelementptr i8, ptr addrspace(5) %80, i16 4
  %104 = getelementptr i8, ptr addrspace(1) %81, i16 4
  %105 = addrspacecast ptr %8 to ptr addrspace(1)
  %106 = icmp ne i16 %91, 0
  br i1 %106, label %b11, label %b12

107:
  call addrspace(1) void @N$EBND()
  unreachable

108:
  %109 = phi i16 [ %86, %85 ]
  br label %90

110:
  %111 = phi i16 [ %86, %94 ]
  br label %90

b7:
  %112 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %112)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %113 = icmp eq i8 %66, 0
  br i1 %113, label %114, label %b7

114:
  %115 = getelementptr i8, ptr addrspace(1) %65, i16 2
  %116 = load i16, ptr addrspace(1) %115
  %117 = getelementptr i8, ptr addrspace(1) %68, i16 2
  %118 = load i16, ptr addrspace(1) %117
  %119 = icmp ule i16 %116, %118
  br i1 %119, label %120, label %121

120:
  br label %122

121:
  br label %122

122:
  %123 = phi ptr addrspace(1) [ %115, %120 ], [ %117, %121 ]
  %124 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %124)
  %125 = load i16, ptr addrspace(1) %123
  call addrspace(1) void @N$PU2(i16 %125)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %126 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %105, ptr addrspace(1) %126)
  call addrspace(1) void @N$PU2(i16 %91)
  %127 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %127)
  call addrspace(1) void @N$PV(ptr addrspace(1) %105)
  call addrspace(1) void @N$PN()
  %128 = load ptr, ptr %17, !tbaa !2
  %129 = getelementptr i8, ptr %128, i16 -4
  %130 = load i16, ptr %129
  %131 = getelementptr i8, ptr @$str12, i16 6
  %132 = getelementptr i8, ptr @$str13, i16 6
  %133 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %134 = phi i16 [ 0, %b11 ], [ %141, %b16 ]
  %135 = icmp ult i16 %134, %130
  br i1 %135, label %b15, label %b17

b15:
  %136 = mul i16 %134, 6
  %137 = getelementptr inbounds i8, ptr %128, i16 %136
  %138 = getelementptr i8, ptr %137, i16 4
  %139 = load i16, ptr %138
  %140 = icmp ult i16 %139, 5
  br i1 %140, label %b18, label %b16

b16:
  %141 = add i16 %134, 1
  br label %b14

b17:
  %142 = getelementptr i8, ptr @$str15, i16 6
  %143 = addrspacecast ptr %142 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %144 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %144, !tbaa !2
  %145 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %143, ptr %145, !tbaa !2
  %146 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %48, ptr addrspace(1) %146, i16 1, i16 500)
  %147 = load ptr, ptr %17, !tbaa !2
  %148 = getelementptr i8, ptr %147, i16 -4
  %149 = load i16, ptr %148
  call addrspace(1) void @N$PU2(i16 %149)
  %150 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %150)
  call addrspace(1) void @N$PN()
  %151 = load ptr, ptr %16, !tbaa !2
  %152 = icmp ne ptr %151, null
  br i1 %152, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %131)
  %153 = load ptr, ptr %137
  call addrspace(1) void @N$PS(ptr %153)
  call addrspace(1) void @N$PS(ptr %132)
  %154 = load i16, ptr %138
  call addrspace(1) void @N$PU2(i16 %154)
  call addrspace(1) void @N$PS(ptr %133)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %151)
  %155 = load ptr, ptr %17, !tbaa !2
  %156 = icmp ne ptr %155, null
  br i1 %156, label %b28, label %b27

b23:
  %157 = getelementptr i8, ptr %151, i16 -4
  %158 = load i16, ptr %157
  br label %b24

b24:
  %159 = phi i16 [ 0, %b23 ], [ %164, %b26 ]
  %160 = icmp ult i16 %159, %158
  br i1 %160, label %b26, label %b25

b25:
  br label %b22

b26:
  %161 = mul i16 %159, 6
  %162 = getelementptr inbounds i8, ptr %151, i16 %161
  %163 = load ptr, ptr %162
  call addrspace(1) void @N$BDRP(ptr %163)
  %164 = add i16 %159, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %155)
  ret i16 0

b28:
  %165 = getelementptr i8, ptr %155, i16 -4
  %166 = load i16, ptr %165
  br label %b29

b29:
  %167 = phi i16 [ 0, %b28 ], [ %172, %b31 ]
  %168 = icmp ult i16 %167, %166
  br i1 %168, label %b31, label %b30

b30:
  br label %b27

b31:
  %169 = mul i16 %167, 6
  %170 = getelementptr inbounds i8, ptr %155, i16 %169
  %171 = load ptr, ptr %170
  call addrspace(1) void @N$BDRP(ptr %171)
  %172 = add i16 %167, 1
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
