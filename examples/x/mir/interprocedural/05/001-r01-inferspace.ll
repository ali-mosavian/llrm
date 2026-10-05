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

declare internal void @find(ptr addrspace(1) nocapture, ptr, ptr, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias) addrspace(1) nearcode memory(argmem: read) willreturn norecurse

declare internal void @affordable(ptr addrspace(1) nocapture, ptr, i16) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal void @initial(ptr addrspace(1) nocapture, ptr) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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
  %12 = alloca [2 x i8]
  %13 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @north(ptr addrspace(1) %13)
  %14 = load ptr, ptr %11, !tbaa !2
  store ptr %14, ptr %12, !tbaa !2
  %15 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %15)
  %16 = load ptr, ptr %9, !tbaa !2
  store ptr %16, ptr %10, !tbaa !2
  %17 = addrspacecast ptr %8 to ptr addrspace(1)
  %18 = addrspacecast ptr %12 to ptr addrspace(5)
  %19 = addrspacecast ptr %12 to ptr addrspace(1)
  %20 = addrspacecast ptr %10 to ptr addrspace(5)
  %21 = addrspacecast ptr %10 to ptr addrspace(1)
  %22 = getelementptr i8, ptr @$str5, i16 6
  %23 = addrspacecast ptr %22 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %24, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %23, ptr %25, !tbaa !2
  %26 = addrspacecast ptr %7 to ptr addrspace(1)
  %27 = load ptr, ptr addrspace(5) %20
  %28 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @find(ptr addrspace(1) %17, ptr %28, ptr %27, ptr addrspace(1) %26)
  %29 = load i8, ptr %8, !tbaa !2, !range !7
  %30 = icmp eq i8 %29, 0
  br i1 %30, label %b4, label %b3

b2:
  %31 = addrspacecast ptr %6 to ptr addrspace(1)
  %32 = getelementptr i8, ptr @$str3, i16 6
  %33 = addrspacecast ptr %32 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %5 to ptr addrspace(1)
  %37 = load ptr, ptr addrspace(5) %20
  %38 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @find(ptr addrspace(1) %31, ptr %38, ptr %37, ptr addrspace(1) %36)
  %39 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %33, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %3 to ptr addrspace(1)
  %43 = load ptr, ptr addrspace(5) %18
  %44 = load ptr, ptr addrspace(5) %20
  call addrspace(1) void @find(ptr addrspace(1) %39, ptr %44, ptr %43, ptr addrspace(1) %42)
  %45 = load i8, ptr %6
  %46 = getelementptr i8, ptr %6, i16 2
  %47 = load ptr addrspace(1), ptr %46
  %48 = load i8, ptr %4
  %49 = getelementptr i8, ptr %4, i16 2
  %50 = load ptr addrspace(1), ptr %49
  %51 = icmp eq i8 %45, 0
  br i1 %51, label %b8, label %b7

b3:
  %52 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %52)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %53 = getelementptr inbounds i8, ptr %8, i16 2
  %54 = load ptr addrspace(1), ptr %53, !tbaa !2
  %55 = load ptr, ptr addrspace(1) %54
  call addrspace(1) void @N$PS(ptr %55)
  %56 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %56)
  %57 = getelementptr i8, ptr addrspace(1) %54, i16 4
  %58 = load i16, ptr addrspace(1) %57
  call addrspace(1) void @N$PU2(i16 %58)
  %59 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %59)
  %60 = getelementptr i8, ptr addrspace(1) %54, i16 2
  %61 = load i16, ptr addrspace(1) %60
  call addrspace(1) void @N$PU2(i16 %61)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %62 = addrspacecast ptr %2 to ptr addrspace(5)
  %63 = addrspacecast ptr %2 to ptr addrspace(1)
  %64 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @affordable(ptr addrspace(1) %63, ptr %64, i16 20)
  %65 = load i16, ptr addrspace(5) %62
  %66 = addrspacecast ptr %1 to ptr addrspace(1)
  %67 = icmp ne i16 %65, 0
  br i1 %67, label %b11, label %b12

b7:
  %68 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %68)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %69 = icmp eq i8 %48, 0
  br i1 %69, label %70, label %b7

70:
  %71 = getelementptr i8, ptr addrspace(1) %47, i16 2
  %72 = load i16, ptr addrspace(1) %71
  %73 = getelementptr i8, ptr addrspace(1) %50, i16 2
  %74 = load i16, ptr addrspace(1) %73
  %75 = icmp ule i16 %72, %74
  br i1 %75, label %76, label %77

76:
  br label %78

77:
  br label %78

78:
  %79 = phi ptr addrspace(1) [ %71, %76 ], [ %73, %77 ]
  %80 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %80)
  %81 = load i16, ptr addrspace(1) %79
  call addrspace(1) void @N$PU2(i16 %81)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %82 = getelementptr i8, ptr addrspace(5) %62, i16 4
  %83 = load ptr addrspace(1), ptr addrspace(5) %82, !tbaa !2
  %84 = getelementptr inbounds i8, ptr addrspace(1) %83, i16 0
  %85 = load ptr, ptr addrspace(1) %84
  call addrspace(1) void @initial(ptr addrspace(1) %66, ptr %85)
  call addrspace(1) void @N$PU2(i16 %65)
  %86 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %86)
  call addrspace(1) void @N$PV(ptr addrspace(1) %66)
  call addrspace(1) void @N$PN()
  %87 = load ptr, ptr %12, !tbaa !2
  %88 = getelementptr i8, ptr %87, i16 -4
  %89 = load i16, ptr %88
  %90 = getelementptr i8, ptr @$str12, i16 6
  %91 = getelementptr i8, ptr @$str13, i16 6
  %92 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %93 = phi i16 [ 0, %b11 ], [ %100, %b16 ]
  %94 = icmp ult i16 %93, %89
  br i1 %94, label %b15, label %b17

b15:
  %95 = mul i16 %93, 6
  %96 = getelementptr inbounds i8, ptr %87, i16 %95
  %97 = getelementptr i8, ptr %96, i16 4
  %98 = load i16, ptr %97
  %99 = icmp ult i16 %98, 5
  br i1 %99, label %b18, label %b16

b16:
  %100 = add i16 %93, 1
  br label %b14

b17:
  %101 = getelementptr i8, ptr @$str15, i16 6
  %102 = addrspacecast ptr %101 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %103 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %103, !tbaa !2
  %104 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %102, ptr %104, !tbaa !2
  %105 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %105, i16 1, i16 500)
  %106 = load ptr, ptr %12, !tbaa !2
  %107 = getelementptr i8, ptr %106, i16 -4
  %108 = load i16, ptr %107
  call addrspace(1) void @N$PU2(i16 %108)
  %109 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %109)
  call addrspace(1) void @N$PN()
  %110 = load ptr, ptr %10, !tbaa !2
  %111 = icmp ne ptr %110, null
  br i1 %111, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %90)
  %112 = load ptr, ptr %96
  call addrspace(1) void @N$PS(ptr %112)
  call addrspace(1) void @N$PS(ptr %91)
  %113 = load i16, ptr %97
  call addrspace(1) void @N$PU2(i16 %113)
  call addrspace(1) void @N$PS(ptr %92)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %110)
  %114 = load ptr, ptr %12, !tbaa !2
  %115 = icmp ne ptr %114, null
  br i1 %115, label %b28, label %b27

b23:
  %116 = getelementptr i8, ptr %110, i16 -4
  %117 = load i16, ptr %116
  br label %b24

b24:
  %118 = phi i16 [ 0, %b23 ], [ %123, %b26 ]
  %119 = icmp ult i16 %118, %117
  br i1 %119, label %b26, label %b25

b25:
  br label %b22

b26:
  %120 = mul i16 %118, 6
  %121 = getelementptr inbounds i8, ptr %110, i16 %120
  %122 = load ptr, ptr %121
  call addrspace(1) void @N$BDRP(ptr %122)
  %123 = add i16 %118, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %114)
  ret i16 0

b28:
  %124 = getelementptr i8, ptr %114, i16 -4
  %125 = load i16, ptr %124
  br label %b29

b29:
  %126 = phi i16 [ 0, %b28 ], [ %131, %b31 ]
  %127 = icmp ult i16 %126, %125
  br i1 %127, label %b31, label %b30

b30:
  br label %b27

b31:
  %128 = mul i16 %126, 6
  %129 = getelementptr inbounds i8, ptr %114, i16 %128
  %130 = load ptr, ptr %129
  call addrspace(1) void @N$BDRP(ptr %130)
  %131 = add i16 %126, 1
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
