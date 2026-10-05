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
  %23 = addrspacecast ptr %11 to ptr addrspace(1)
  %24 = getelementptr i8, ptr @$str5, i16 6
  %25 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %25, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %7 to ptr addrspace(5)
  %29 = addrspacecast ptr %7 to ptr addrspace(1)
  %30 = load ptr, ptr addrspace(5) %22
  %31 = addrspacecast ptr addrspace(1) %29 to ptr addrspace(5)
  %32 = addrspacecast ptr addrspace(1) %21 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %20, ptr %30, ptr %19, ptr addrspace(5) %28)
  %33 = load i8, ptr %8, !tbaa !2, !range !7
  %34 = icmp eq i8 %33, 0
  br i1 %34, label %b4, label %b3

b2:
  %35 = addrspacecast ptr %6 to ptr addrspace(5)
  %36 = addrspacecast ptr %6 to ptr addrspace(1)
  %37 = getelementptr i8, ptr @$str3, i16 6
  %38 = addrspacecast ptr %37 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %39, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %38, ptr %40, !tbaa !2
  %41 = addrspacecast ptr %5 to ptr addrspace(5)
  %42 = addrspacecast ptr %5 to ptr addrspace(1)
  %43 = load ptr, ptr addrspace(5) %22
  %44 = addrspacecast ptr addrspace(1) %42 to ptr addrspace(5)
  %45 = addrspacecast ptr addrspace(1) %36 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %35, ptr %43, ptr %19, ptr addrspace(5) %41)
  %46 = addrspacecast ptr %4 to ptr addrspace(5)
  %47 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %48 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %48, !tbaa !2
  %49 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %38, ptr %49, !tbaa !2
  %50 = addrspacecast ptr %3 to ptr addrspace(5)
  %51 = addrspacecast ptr %3 to ptr addrspace(1)
  %52 = addrspacecast ptr addrspace(1) %51 to ptr addrspace(5)
  %53 = addrspacecast ptr addrspace(1) %47 to ptr addrspace(5)
  call addrspace(1) void @find(ptr addrspace(5) %46, ptr %19, ptr %43, ptr addrspace(5) %50)
  %54 = load i8, ptr %6
  %55 = getelementptr i8, ptr %6, i16 2
  %56 = load ptr addrspace(1), ptr %55
  %57 = load i8, ptr %4
  %58 = getelementptr i8, ptr %4, i16 2
  %59 = load ptr addrspace(1), ptr %58
  %60 = icmp eq i8 %54, 0
  br i1 %60, label %b8, label %b7

b3:
  %61 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %61)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %62 = getelementptr inbounds i8, ptr %8, i16 2
  %63 = load ptr addrspace(1), ptr %62, !tbaa !2
  %64 = load ptr, ptr addrspace(1) %63
  call addrspace(1) void @N$PS(ptr %64)
  %65 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %65)
  %66 = getelementptr i8, ptr addrspace(1) %63, i16 4
  %67 = load i16, ptr addrspace(1) %66
  call addrspace(1) void @N$PU2(i16 %67)
  %68 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  %69 = getelementptr i8, ptr addrspace(1) %63, i16 2
  %70 = load i16, ptr addrspace(1) %69
  call addrspace(1) void @N$PU2(i16 %70)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %71 = addrspacecast ptr %2 to ptr addrspace(5)
  %72 = addrspacecast ptr %2 to ptr addrspace(1)
  %73 = load ptr, ptr addrspace(5) %22
  %74 = addrspacecast ptr addrspace(1) %72 to ptr addrspace(5)
  call addrspace(1) void @affordable(ptr addrspace(5) %71, ptr %73, i16 20)
  %75 = load i16, ptr addrspace(5) %71
  %76 = addrspacecast ptr %1 to ptr addrspace(5)
  %77 = addrspacecast ptr %1 to ptr addrspace(1)
  %78 = icmp ne i16 %75, 0
  br i1 %78, label %b11, label %b12

b7:
  %79 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %79)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %80 = icmp eq i8 %57, 0
  br i1 %80, label %81, label %b7

81:
  %82 = getelementptr i8, ptr addrspace(1) %56, i16 2
  %83 = load i16, ptr addrspace(1) %82
  %84 = getelementptr i8, ptr addrspace(1) %59, i16 2
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
  %93 = getelementptr i8, ptr addrspace(5) %71, i16 4
  %94 = load ptr addrspace(1), ptr addrspace(5) %93, !tbaa !2
  %95 = getelementptr inbounds i8, ptr addrspace(1) %94, i16 0
  %96 = load ptr, ptr addrspace(1) %95
  %97 = addrspacecast ptr addrspace(1) %77 to ptr addrspace(5)
  call addrspace(1) void @initial(ptr addrspace(5) %76, ptr %96)
  call addrspace(1) void @N$PU2(i16 %75)
  %98 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %98)
  call addrspace(1) void @N$PV(ptr addrspace(1) %77)
  call addrspace(1) void @N$PN()
  %99 = load ptr, ptr %11, !tbaa !2
  %100 = getelementptr i8, ptr %99, i16 -4
  %101 = load i16, ptr %100
  %102 = getelementptr i8, ptr @$str12, i16 6
  %103 = getelementptr i8, ptr @$str13, i16 6
  %104 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %105 = phi i16 [ 0, %b11 ], [ %112, %b16 ]
  %106 = icmp ult i16 %105, %101
  br i1 %106, label %b15, label %b17

b15:
  %107 = mul i16 %105, 6
  %108 = getelementptr inbounds i8, ptr %99, i16 %107
  %109 = getelementptr i8, ptr %108, i16 4
  %110 = load i16, ptr %109
  %111 = icmp ult i16 %110, 5
  br i1 %111, label %b18, label %b16

b16:
  %112 = add i16 %105, 1
  br label %b14

b17:
  %113 = getelementptr i8, ptr @$str15, i16 6
  %114 = addrspacecast ptr %113 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %115 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %115, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %114, ptr %116, !tbaa !2
  %117 = addrspacecast ptr %0 to ptr addrspace(5)
  %118 = addrspacecast ptr %0 to ptr addrspace(1)
  %119 = addrspacecast ptr addrspace(1) %118 to ptr addrspace(5)
  %120 = addrspacecast ptr addrspace(1) %23 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %22, ptr addrspace(5) %117, i16 1, i16 500)
  %121 = load ptr, ptr %11, !tbaa !2
  %122 = getelementptr i8, ptr %121, i16 -4
  %123 = load i16, ptr %122
  call addrspace(1) void @N$PU2(i16 %123)
  %124 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %124)
  call addrspace(1) void @N$PN()
  %125 = icmp ne ptr %19, null
  br i1 %125, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %102)
  %126 = load ptr, ptr %108
  call addrspace(1) void @N$PS(ptr %126)
  call addrspace(1) void @N$PS(ptr %103)
  %127 = load i16, ptr %109
  call addrspace(1) void @N$PU2(i16 %127)
  call addrspace(1) void @N$PS(ptr %104)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %19)
  %128 = load ptr, ptr %11, !tbaa !2
  %129 = icmp ne ptr %128, null
  br i1 %129, label %b28, label %b27

b23:
  %130 = getelementptr i8, ptr %19, i16 -4
  %131 = load i16, ptr %130
  br label %b24

b24:
  %132 = phi i16 [ 0, %b23 ], [ %137, %b26 ]
  %133 = icmp ult i16 %132, %131
  br i1 %133, label %b26, label %b25

b25:
  br label %b22

b26:
  %134 = mul i16 %132, 6
  %135 = getelementptr inbounds i8, ptr %19, i16 %134
  %136 = load ptr, ptr %135
  call addrspace(1) void @N$BDRP(ptr %136)
  %137 = add i16 %132, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %128)
  ret i16 0

b28:
  %138 = getelementptr i8, ptr %128, i16 -4
  %139 = load i16, ptr %138
  br label %b29

b29:
  %140 = phi i16 [ 0, %b28 ], [ %145, %b31 ]
  %141 = icmp ult i16 %140, %139
  br i1 %141, label %b31, label %b30

b30:
  br label %b27

b31:
  %142 = mul i16 %140, 6
  %143 = getelementptr inbounds i8, ptr %128, i16 %142
  %144 = load ptr, ptr %143
  call addrspace(1) void @N$BDRP(ptr %144)
  %145 = add i16 %140, 1
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
