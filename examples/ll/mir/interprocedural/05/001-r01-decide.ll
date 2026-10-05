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
  br i1 %84, label %91, label %101

85:
  %86 = add i16 %83, 1
  br label %82

87:
  %88 = phi i16 [ %102, %101 ], [ %104, %103 ]
  %89 = addrspacecast ptr %79 to ptr addrspace(1)
  %90 = icmp ule i16 %88, %81
  br i1 %90, label %97, label %100

91:
  %92 = mul i16 %83, 6
  %93 = getelementptr inbounds i8, ptr %79, i16 %92
  %94 = getelementptr i8, ptr %93, i16 2
  %95 = load i16, ptr %94
  %96 = icmp ule i16 %95, 20
  br i1 %96, label %85, label %103

97:
  %98 = addrspacecast ptr %8 to ptr addrspace(1)
  %99 = icmp ne i16 %88, 0
  br i1 %99, label %b11, label %b12

100:
  call addrspace(1) void @N$EBND()
  unreachable

101:
  %102 = phi i16 [ %83, %82 ]
  br label %87

103:
  %104 = phi i16 [ %83, %91 ]
  br label %87

b7:
  %105 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %105)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %106 = icmp eq i8 %65, 0
  br i1 %106, label %107, label %b7

107:
  %108 = getelementptr i8, ptr addrspace(1) %64, i16 2
  %109 = load i16, ptr addrspace(1) %108
  %110 = getelementptr i8, ptr addrspace(1) %67, i16 2
  %111 = load i16, ptr addrspace(1) %110
  %112 = icmp ule i16 %109, %111
  br i1 %112, label %113, label %114

113:
  br label %115

114:
  br label %115

115:
  %116 = phi ptr addrspace(1) [ %108, %113 ], [ %110, %114 ]
  %117 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %117)
  %118 = load i16, ptr addrspace(1) %116
  call addrspace(1) void @N$PU2(i16 %118)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %119 = getelementptr inbounds i8, ptr addrspace(1) %89, i16 0
  %120 = load ptr, ptr addrspace(1) %119
  %121 = getelementptr i8, ptr %120, i16 -4
  %122 = load i16, ptr %121
  %123 = addrspacecast ptr %120 to ptr addrspace(1)
  %124 = icmp uge i16 %122, 1
  br i1 %124, label %125, label %135

125:
  store i16 1, ptr addrspace(1) %98
  %126 = getelementptr i8, ptr addrspace(1) %98, i16 2
  store i16 1, ptr addrspace(1) %126
  %127 = getelementptr i8, ptr addrspace(1) %98, i16 4
  store ptr addrspace(1) %123, ptr addrspace(1) %127
  call addrspace(1) void @N$PU2(i16 %88)
  %128 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %128)
  call addrspace(1) void @N$PV(ptr addrspace(1) %98)
  call addrspace(1) void @N$PN()
  %129 = load ptr, ptr %16, !tbaa !2
  %130 = getelementptr i8, ptr %129, i16 -4
  %131 = load i16, ptr %130
  %132 = getelementptr i8, ptr @$str12, i16 6
  %133 = getelementptr i8, ptr @$str13, i16 6
  %134 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

135:
  call addrspace(1) void @N$EBND()
  unreachable

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %136 = phi i16 [ 0, %125 ], [ %143, %b16 ]
  %137 = icmp ult i16 %136, %131
  br i1 %137, label %b15, label %b17

b15:
  %138 = mul i16 %136, 6
  %139 = getelementptr inbounds i8, ptr %129, i16 %138
  %140 = getelementptr i8, ptr %139, i16 4
  %141 = load i16, ptr %140
  %142 = icmp ult i16 %141, 5
  br i1 %142, label %b18, label %b16

b16:
  %143 = add i16 %136, 1
  br label %b14

b17:
  %144 = getelementptr i8, ptr @$str15, i16 6
  %145 = addrspacecast ptr %144 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %146 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %146, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %145, ptr %147, !tbaa !2
  %148 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %47, ptr addrspace(1) %148, i16 1, i16 500)
  %149 = load ptr, ptr %16, !tbaa !2
  %150 = getelementptr i8, ptr %149, i16 -4
  %151 = load i16, ptr %150
  call addrspace(1) void @N$PU2(i16 %151)
  %152 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %152)
  call addrspace(1) void @N$PN()
  %153 = load ptr, ptr %15, !tbaa !2
  %154 = icmp ne ptr %153, null
  br i1 %154, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %132)
  %155 = load ptr, ptr %139
  call addrspace(1) void @N$PS(ptr %155)
  call addrspace(1) void @N$PS(ptr %133)
  %156 = load i16, ptr %140
  call addrspace(1) void @N$PU2(i16 %156)
  call addrspace(1) void @N$PS(ptr %134)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %153)
  %157 = load ptr, ptr %16, !tbaa !2
  %158 = icmp ne ptr %157, null
  br i1 %158, label %b28, label %b27

b23:
  %159 = getelementptr i8, ptr %153, i16 -4
  %160 = load i16, ptr %159
  br label %b24

b24:
  %161 = phi i16 [ 0, %b23 ], [ %166, %b26 ]
  %162 = icmp ult i16 %161, %160
  br i1 %162, label %b26, label %b25

b25:
  br label %b22

b26:
  %163 = mul i16 %161, 6
  %164 = getelementptr inbounds i8, ptr %153, i16 %163
  %165 = load ptr, ptr %164
  call addrspace(1) void @N$BDRP(ptr %165)
  %166 = add i16 %161, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %157)
  ret i16 0

b28:
  %167 = getelementptr i8, ptr %157, i16 -4
  %168 = load i16, ptr %167
  br label %b29

b29:
  %169 = phi i16 [ 0, %b28 ], [ %174, %b31 ]
  %170 = icmp ult i16 %169, %168
  br i1 %170, label %b31, label %b30

b30:
  br label %b27

b31:
  %171 = mul i16 %169, 6
  %172 = getelementptr inbounds i8, ptr %157, i16 %171
  %173 = load ptr, ptr %172
  call addrspace(1) void @N$BDRP(ptr %173)
  %174 = add i16 %169, 1
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
