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
  %51 = getelementptr i8, ptr @$str3, i16 6
  %52 = addrspacecast ptr %51 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %52, ptr %54, !tbaa !2
  %55 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %50, ptr addrspace(1) %41, ptr addrspace(1) %42, ptr addrspace(1) %55)
  %56 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %7, !tbaa !2
  %57 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 4, ptr %57, !tbaa !2
  %58 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %52, ptr %58, !tbaa !2
  %59 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %56, ptr addrspace(1) %42, ptr addrspace(1) %41, ptr addrspace(1) %59)
  %60 = load i8, ptr %10
  %61 = getelementptr i8, ptr %10, i16 2
  %62 = load ptr addrspace(1), ptr %61
  %63 = load i8, ptr %8
  %64 = getelementptr i8, ptr %8, i16 2
  %65 = load ptr addrspace(1), ptr %64
  %66 = icmp eq i8 %60, 0
  br i1 %66, label %b8, label %b7

b3:
  %67 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %67)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %68 = getelementptr inbounds i8, ptr %12, i16 2
  %69 = load ptr addrspace(1), ptr %68, !tbaa !2
  %70 = load ptr, ptr addrspace(1) %69
  call addrspace(1) void @N$PS(ptr %70)
  %71 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %71)
  %72 = getelementptr i8, ptr addrspace(1) %69, i16 4
  %73 = load i16, ptr addrspace(1) %72
  call addrspace(1) void @N$PU2(i16 %73)
  %74 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %74)
  %75 = getelementptr i8, ptr addrspace(1) %69, i16 2
  %76 = load i16, ptr addrspace(1) %75
  call addrspace(1) void @N$PU2(i16 %76)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %77 = addrspacecast ptr %6 to ptr addrspace(5)
  %78 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %78, ptr addrspace(1) %41, i16 20)
  %79 = load i16, ptr addrspace(5) %77
  %80 = addrspacecast ptr %5 to ptr addrspace(1)
  %81 = icmp ne i16 %79, 0
  br i1 %81, label %b11, label %b12

b7:
  %82 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %82)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %83 = icmp eq i8 %63, 0
  br i1 %83, label %84, label %b7

84:
  %85 = getelementptr i8, ptr addrspace(1) %62, i16 2
  %86 = load i16, ptr addrspace(1) %85
  %87 = getelementptr i8, ptr addrspace(1) %65, i16 2
  %88 = load i16, ptr addrspace(1) %87
  %89 = icmp ule i16 %86, %88
  br i1 %89, label %90, label %91

90:
  br label %92

91:
  br label %92

92:
  %93 = phi ptr addrspace(1) [ %85, %90 ], [ %87, %91 ]
  %94 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %94)
  %95 = load i16, ptr addrspace(1) %93
  call addrspace(1) void @N$PU2(i16 %95)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %96 = getelementptr i8, ptr addrspace(5) %77, i16 4
  %97 = load ptr addrspace(1), ptr addrspace(5) %96, !tbaa !2
  %98 = getelementptr inbounds i8, ptr addrspace(1) %97, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %80, ptr addrspace(1) %98)
  call addrspace(1) void @N$PU2(i16 %79)
  %99 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %99)
  call addrspace(1) void @N$PV(ptr addrspace(1) %80)
  call addrspace(1) void @N$PN()
  %100 = load ptr, ptr %16, !tbaa !2
  %101 = getelementptr i8, ptr %100, i16 -4
  %102 = load i16, ptr %101
  %103 = getelementptr i8, ptr @$str12, i16 6
  %104 = getelementptr i8, ptr @$str13, i16 6
  %105 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %106 = phi i16 [ 0, %b11 ], [ %113, %b16 ]
  %107 = icmp ult i16 %106, %102
  br i1 %107, label %b15, label %b17

b15:
  %108 = mul i16 %106, 6
  %109 = getelementptr inbounds i8, ptr %100, i16 %108
  %110 = getelementptr i8, ptr %109, i16 4
  %111 = load i16, ptr %110
  %112 = icmp ult i16 %111, 5
  br i1 %112, label %b18, label %b16

b16:
  %113 = add i16 %106, 1
  br label %b14

b17:
  %114 = getelementptr i8, ptr @$str15, i16 6
  %115 = addrspacecast ptr %114 to ptr addrspace(1)
  store i16 3, ptr %4, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 3, ptr %116, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %115, ptr %117, !tbaa !2
  %118 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %41, ptr addrspace(1) %118, i16 1, i16 500)
  %119 = load ptr, ptr %16, !tbaa !2
  %120 = getelementptr i8, ptr %119, i16 -4
  %121 = load i16, ptr %120
  call addrspace(1) void @N$PU2(i16 %121)
  %122 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %122)
  call addrspace(1) void @N$PN()
  %123 = load ptr, ptr %14, !tbaa !2
  %124 = icmp ne ptr %123, null
  br i1 %124, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %103)
  %125 = load ptr, ptr %109
  call addrspace(1) void @N$PS(ptr %125)
  call addrspace(1) void @N$PS(ptr %104)
  %126 = load i16, ptr %110
  call addrspace(1) void @N$PU2(i16 %126)
  call addrspace(1) void @N$PS(ptr %105)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %123)
  %127 = load ptr, ptr %16, !tbaa !2
  %128 = icmp ne ptr %127, null
  br i1 %128, label %b28, label %b27

b23:
  %129 = getelementptr i8, ptr %123, i16 -4
  %130 = load i16, ptr %129
  br label %b24

b24:
  %131 = phi i16 [ 0, %b23 ], [ %136, %b26 ]
  %132 = icmp ult i16 %131, %130
  br i1 %132, label %b26, label %b25

b25:
  br label %b22

b26:
  %133 = mul i16 %131, 6
  %134 = getelementptr inbounds i8, ptr %123, i16 %133
  %135 = load ptr, ptr %134
  call addrspace(1) void @N$BDRP(ptr %135)
  %136 = add i16 %131, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %127)
  ret i16 0

b28:
  %137 = getelementptr i8, ptr %127, i16 -4
  %138 = load i16, ptr %137
  br label %b29

b29:
  %139 = phi i16 [ 0, %b28 ], [ %144, %b31 ]
  %140 = icmp ult i16 %139, %138
  br i1 %140, label %b31, label %b30

b30:
  br label %b27

b31:
  %141 = mul i16 %139, 6
  %142 = getelementptr inbounds i8, ptr %127, i16 %141
  %143 = load ptr, ptr %142
  call addrspace(1) void @N$BDRP(ptr %143)
  %144 = add i16 %139, 1
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
