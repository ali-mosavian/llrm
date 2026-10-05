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

declare internal void @north(ptr addrspace(5)) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @south(ptr addrspace(5)) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @find(ptr addrspace(5), ptr, ptr, ptr addrspace(5)) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias) addrspace(1) nearcode memory(argmem: read) willreturn norecurse

declare internal void @affordable(ptr addrspace(5), ptr, i16) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal void @initial(ptr addrspace(5), ptr) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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
  %3 = alloca [8 x i8]
  %4 = alloca [6 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [6 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [6 x i8]
  %9 = alloca [2 x i8]
  %10 = alloca [2 x i8]
  %11 = alloca [2 x i8]
  %12 = addrspacecast ptr %10 to ptr addrspace(5)
  %13 = addrspacecast ptr %10 to ptr addrspace(1)
  %14 = addrspacecast ptr addrspace(1) %13 to ptr addrspace(5)
  call addrspace(1) void @north(ptr addrspace(5) %12)
  %15 = load ptr, ptr %10, !tbaa !2
  store ptr %15, ptr %11, !tbaa !2
  %16 = addrspacecast ptr %9 to ptr addrspace(5)
  %17 = addrspacecast ptr %9 to ptr addrspace(1)
  %18 = addrspacecast ptr addrspace(1) %17 to ptr addrspace(5)
  call addrspace(1) void @south(ptr addrspace(5) %16)
  %19 = load ptr, ptr %9, !tbaa !2
  %20 = addrspacecast ptr %8 to ptr addrspace(5)
  %21 = addrspacecast ptr %8 to ptr addrspace(1)
  %22 = addrspacecast ptr %11 to ptr addrspace(5)
  %23 = addrspacecast ptr %11 to ptr addrspace(5)
  %24 = addrspacecast ptr %11 to ptr addrspace(1)
  %25 = getelementptr i8, ptr @$str5, i16 6
  %26 = addrspacecast ptr %25 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %27, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %26, ptr %28, !tbaa !2
  %29 = addrspacecast ptr %7 to ptr addrspace(5)
  %30 = addrspacecast ptr %7 to ptr addrspace(1)
  %31 = load ptr, ptr addrspace(5) %22
  %32 = addrspacecast ptr addrspace(1) %30 to ptr addrspace(5)
  %33 = addrspacecast ptr addrspace(1) %21 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %20, ptr %31, ptr %19, ptr addrspace(5) %29)
  %34 = load i8, ptr %8, !tbaa !2, !range !7
  %35 = icmp eq i8 %34, 0
  br i1 %35, label %b4, label %b3

b2:
  %36 = addrspacecast ptr %6 to ptr addrspace(5)
  %37 = addrspacecast ptr %6 to ptr addrspace(1)
  %38 = getelementptr i8, ptr @$str3, i16 6
  %39 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %39, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %5 to ptr addrspace(5)
  %43 = addrspacecast ptr %5 to ptr addrspace(1)
  %44 = load ptr, ptr addrspace(5) %22
  %45 = addrspacecast ptr addrspace(1) %43 to ptr addrspace(5)
  %46 = addrspacecast ptr addrspace(1) %37 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %36, ptr %44, ptr %19, ptr addrspace(5) %42)
  %47 = addrspacecast ptr %4 to ptr addrspace(5)
  %48 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %49, !tbaa !2
  %50 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %39, ptr %50, !tbaa !2
  %51 = addrspacecast ptr %3 to ptr addrspace(5)
  %52 = addrspacecast ptr %3 to ptr addrspace(1)
  %53 = addrspacecast ptr addrspace(1) %52 to ptr addrspace(5)
  %54 = addrspacecast ptr addrspace(1) %48 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %47, ptr %19, ptr %44, ptr addrspace(5) %51)
  %55 = load i8, ptr %6
  %56 = getelementptr i8, ptr %6, i16 2
  %57 = load ptr addrspace(1), ptr %56
  %58 = load i8, ptr %4
  %59 = getelementptr i8, ptr %4, i16 2
  %60 = load ptr addrspace(1), ptr %59
  %61 = icmp eq i8 %55, 0
  br i1 %61, label %b8, label %b7

b3:
  %62 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %62)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %63 = getelementptr inbounds i8, ptr %8, i16 2
  %64 = load ptr addrspace(1), ptr %63, !tbaa !2
  %65 = load ptr, ptr addrspace(1) %64
  call addrspace(1) void @N$PS(ptr %65)
  %66 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %66)
  %67 = getelementptr i8, ptr addrspace(1) %64, i16 4
  %68 = load i16, ptr addrspace(1) %67
  call addrspace(1) void @N$PU2(i16 %68)
  %69 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %69)
  %70 = getelementptr i8, ptr addrspace(1) %64, i16 2
  %71 = load i16, ptr addrspace(1) %70
  call addrspace(1) void @N$PU2(i16 %71)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %72 = addrspacecast ptr %2 to ptr addrspace(5)
  %73 = addrspacecast ptr %2 to ptr addrspace(5)
  %74 = addrspacecast ptr %2 to ptr addrspace(1)
  %75 = load ptr, ptr addrspace(5) %22
  %76 = addrspacecast ptr addrspace(1) %74 to ptr addrspace(5)
  call addrspace(1) void @affordable(ptr addrspace(5) %73, ptr %75, i16 20)
  %77 = load i16, ptr addrspace(5) %72
  %78 = addrspacecast ptr %1 to ptr addrspace(5)
  %79 = addrspacecast ptr %1 to ptr addrspace(1)
  %80 = icmp ne i16 %77, 0
  br i1 %80, label %b11, label %b12

b7:
  %81 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %81)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %82 = icmp eq i8 %58, 0
  br i1 %82, label %83, label %b7

83:
  %84 = getelementptr i8, ptr addrspace(1) %57, i16 2
  %85 = load i16, ptr addrspace(1) %84
  %86 = getelementptr i8, ptr addrspace(1) %60, i16 2
  %87 = load i16, ptr addrspace(1) %86
  %88 = icmp ule i16 %85, %87
  br i1 %88, label %89, label %90

89:
  br label %91

90:
  br label %91

91:
  %92 = phi ptr addrspace(1) [ %84, %89 ], [ %86, %90 ]
  %93 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %93)
  %94 = load i16, ptr addrspace(1) %92
  call addrspace(1) void @N$PU2(i16 %94)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %95 = getelementptr i8, ptr addrspace(5) %72, i16 4
  %96 = load ptr addrspace(1), ptr addrspace(5) %95, !tbaa !2
  %97 = getelementptr inbounds i8, ptr addrspace(1) %96, i16 0
  %98 = load ptr, ptr addrspace(1) %97
  %99 = addrspacecast ptr addrspace(1) %79 to ptr addrspace(5)
  call addrspace(1) void @initial(ptr addrspace(5) %78, ptr %98)
  call addrspace(1) void @N$PU2(i16 %77)
  %100 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %100)
  call addrspace(1) void @N$PV(ptr addrspace(1) %79)
  call addrspace(1) void @N$PN()
  %101 = load ptr, ptr %11, !tbaa !2
  %102 = getelementptr i8, ptr %101, i16 -4
  %103 = load i16, ptr %102
  %104 = getelementptr i8, ptr @$str12, i16 6
  %105 = getelementptr i8, ptr @$str13, i16 6
  %106 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %107 = phi i16 [ 0, %b11 ], [ %114, %b16 ]
  %108 = icmp ult i16 %107, %103
  br i1 %108, label %b15, label %b17

b15:
  %109 = mul i16 %107, 6
  %110 = getelementptr inbounds i8, ptr %101, i16 %109
  %111 = getelementptr i8, ptr %110, i16 4
  %112 = load i16, ptr %111
  %113 = icmp ult i16 %112, 5
  br i1 %113, label %b18, label %b16

b16:
  %114 = add i16 %107, 1
  br label %b14

b17:
  %115 = getelementptr i8, ptr @$str15, i16 6
  %116 = addrspacecast ptr %115 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %117, !tbaa !2
  %118 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %116, ptr %118, !tbaa !2
  %119 = addrspacecast ptr %0 to ptr addrspace(5)
  %120 = addrspacecast ptr %0 to ptr addrspace(1)
  %121 = addrspacecast ptr addrspace(1) %120 to ptr addrspace(5)
  %122 = addrspacecast ptr addrspace(1) %24 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %23, ptr addrspace(5) %119, i16 1, i16 500)
  %123 = load ptr, ptr %11, !tbaa !2
  %124 = getelementptr i8, ptr %123, i16 -4
  %125 = load i16, ptr %124
  call addrspace(1) void @N$PU2(i16 %125)
  %126 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %126)
  call addrspace(1) void @N$PN()
  %127 = icmp ne ptr %19, null
  br i1 %127, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %104)
  %128 = load ptr, ptr %110
  call addrspace(1) void @N$PS(ptr %128)
  call addrspace(1) void @N$PS(ptr %105)
  %129 = load i16, ptr %111
  call addrspace(1) void @N$PU2(i16 %129)
  call addrspace(1) void @N$PS(ptr %106)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %19)
  %130 = load ptr, ptr %11, !tbaa !2
  %131 = icmp ne ptr %130, null
  br i1 %131, label %b28, label %b27

b23:
  %132 = getelementptr i8, ptr %19, i16 -4
  %133 = load i16, ptr %132
  br label %b24

b24:
  %134 = phi i16 [ 0, %b23 ], [ %139, %b26 ]
  %135 = icmp ult i16 %134, %133
  br i1 %135, label %b26, label %b25

b25:
  br label %b22

b26:
  %136 = mul i16 %134, 6
  %137 = getelementptr inbounds i8, ptr %19, i16 %136
  %138 = load ptr, ptr %137
  call addrspace(1) void @N$BDRP(ptr %138)
  %139 = add i16 %134, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %130)
  ret i16 0

b28:
  %140 = getelementptr i8, ptr %130, i16 -4
  %141 = load i16, ptr %140
  br label %b29

b29:
  %142 = phi i16 [ 0, %b28 ], [ %147, %b31 ]
  %143 = icmp ult i16 %142, %141
  br i1 %143, label %b31, label %b30

b30:
  br label %b27

b31:
  %144 = mul i16 %142, 6
  %145 = getelementptr inbounds i8, ptr %130, i16 %144
  %146 = load ptr, ptr %145
  call addrspace(1) void @N$BDRP(ptr %146)
  %147 = add i16 %142, 1
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
