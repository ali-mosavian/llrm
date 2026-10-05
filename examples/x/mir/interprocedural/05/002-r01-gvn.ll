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
  %27 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @find(ptr addrspace(1) %17, ptr %27, ptr %16, ptr addrspace(1) %26)
  %28 = load i8, ptr %8, !tbaa !2, !range !7
  %29 = icmp eq i8 %28, 0
  br i1 %29, label %b4, label %b3

b2:
  %30 = addrspacecast ptr %6 to ptr addrspace(1)
  %31 = getelementptr i8, ptr @$str3, i16 6
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %5 to ptr addrspace(1)
  %36 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @find(ptr addrspace(1) %30, ptr %36, ptr %16, ptr addrspace(1) %35)
  %37 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %32, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %37, ptr %16, ptr %36, ptr addrspace(1) %40)
  %41 = load i8, ptr %6
  %42 = getelementptr i8, ptr %6, i16 2
  %43 = load ptr addrspace(1), ptr %42
  %44 = load i8, ptr %4
  %45 = getelementptr i8, ptr %4, i16 2
  %46 = load ptr addrspace(1), ptr %45
  %47 = icmp eq i8 %41, 0
  br i1 %47, label %b8, label %b7

b3:
  %48 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %49 = getelementptr inbounds i8, ptr %8, i16 2
  %50 = load ptr addrspace(1), ptr %49, !tbaa !2
  %51 = load ptr, ptr addrspace(1) %50
  call addrspace(1) void @N$PS(ptr %51)
  %52 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %52)
  %53 = getelementptr i8, ptr addrspace(1) %50, i16 4
  %54 = load i16, ptr addrspace(1) %53
  call addrspace(1) void @N$PU2(i16 %54)
  %55 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %55)
  %56 = getelementptr i8, ptr addrspace(1) %50, i16 2
  %57 = load i16, ptr addrspace(1) %56
  call addrspace(1) void @N$PU2(i16 %57)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %58 = addrspacecast ptr %2 to ptr addrspace(5)
  %59 = addrspacecast ptr %2 to ptr addrspace(1)
  %60 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @affordable(ptr addrspace(1) %59, ptr %60, i16 20)
  %61 = load i16, ptr addrspace(5) %58
  %62 = addrspacecast ptr %1 to ptr addrspace(1)
  %63 = icmp ne i16 %61, 0
  br i1 %63, label %b11, label %b12

b7:
  %64 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %64)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %65 = icmp eq i8 %44, 0
  br i1 %65, label %66, label %b7

66:
  %67 = getelementptr i8, ptr addrspace(1) %43, i16 2
  %68 = load i16, ptr addrspace(1) %67
  %69 = getelementptr i8, ptr addrspace(1) %46, i16 2
  %70 = load i16, ptr addrspace(1) %69
  %71 = icmp ule i16 %68, %70
  br i1 %71, label %72, label %73

72:
  br label %74

73:
  br label %74

74:
  %75 = phi ptr addrspace(1) [ %67, %72 ], [ %69, %73 ]
  %76 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  %77 = load i16, ptr addrspace(1) %75
  call addrspace(1) void @N$PU2(i16 %77)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %78 = getelementptr i8, ptr addrspace(5) %58, i16 4
  %79 = load ptr addrspace(1), ptr addrspace(5) %78, !tbaa !2
  %80 = getelementptr inbounds i8, ptr addrspace(1) %79, i16 0
  %81 = load ptr, ptr addrspace(1) %80
  call addrspace(1) void @initial(ptr addrspace(1) %62, ptr %81)
  call addrspace(1) void @N$PU2(i16 %61)
  %82 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %82)
  call addrspace(1) void @N$PV(ptr addrspace(1) %62)
  call addrspace(1) void @N$PN()
  %83 = load ptr, ptr %12, !tbaa !2
  %84 = getelementptr i8, ptr %83, i16 -4
  %85 = load i16, ptr %84
  %86 = getelementptr i8, ptr @$str12, i16 6
  %87 = getelementptr i8, ptr @$str13, i16 6
  %88 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %89 = phi i16 [ 0, %b11 ], [ %96, %b16 ]
  %90 = icmp ult i16 %89, %85
  br i1 %90, label %b15, label %b17

b15:
  %91 = mul i16 %89, 6
  %92 = getelementptr inbounds i8, ptr %83, i16 %91
  %93 = getelementptr i8, ptr %92, i16 4
  %94 = load i16, ptr %93
  %95 = icmp ult i16 %94, 5
  br i1 %95, label %b18, label %b16

b16:
  %96 = add i16 %89, 1
  br label %b14

b17:
  %97 = getelementptr i8, ptr @$str15, i16 6
  %98 = addrspacecast ptr %97 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %99 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %99, !tbaa !2
  %100 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %98, ptr %100, !tbaa !2
  %101 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %101, i16 1, i16 500)
  %102 = load ptr, ptr %12, !tbaa !2
  %103 = getelementptr i8, ptr %102, i16 -4
  %104 = load i16, ptr %103
  call addrspace(1) void @N$PU2(i16 %104)
  %105 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %105)
  call addrspace(1) void @N$PN()
  %106 = icmp ne ptr %16, null
  br i1 %106, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %86)
  %107 = load ptr, ptr %92
  call addrspace(1) void @N$PS(ptr %107)
  call addrspace(1) void @N$PS(ptr %87)
  %108 = load i16, ptr %93
  call addrspace(1) void @N$PU2(i16 %108)
  call addrspace(1) void @N$PS(ptr %88)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %16)
  %109 = load ptr, ptr %12, !tbaa !2
  %110 = icmp ne ptr %109, null
  br i1 %110, label %b28, label %b27

b23:
  %111 = getelementptr i8, ptr %16, i16 -4
  %112 = load i16, ptr %111
  br label %b24

b24:
  %113 = phi i16 [ 0, %b23 ], [ %118, %b26 ]
  %114 = icmp ult i16 %113, %112
  br i1 %114, label %b26, label %b25

b25:
  br label %b22

b26:
  %115 = mul i16 %113, 6
  %116 = getelementptr inbounds i8, ptr %16, i16 %115
  %117 = load ptr, ptr %116
  call addrspace(1) void @N$BDRP(ptr %117)
  %118 = add i16 %113, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %109)
  ret i16 0

b28:
  %119 = getelementptr i8, ptr %109, i16 -4
  %120 = load i16, ptr %119
  br label %b29

b29:
  %121 = phi i16 [ 0, %b28 ], [ %126, %b31 ]
  %122 = icmp ult i16 %121, %120
  br i1 %122, label %b31, label %b30

b30:
  br label %b27

b31:
  %123 = mul i16 %121, 6
  %124 = getelementptr inbounds i8, ptr %109, i16 %123
  %125 = load ptr, ptr %124
  call addrspace(1) void @N$BDRP(ptr %125)
  %126 = add i16 %121, 1
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
