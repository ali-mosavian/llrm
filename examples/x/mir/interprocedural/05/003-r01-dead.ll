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
  %20 = getelementptr i8, ptr @$str5, i16 6
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %7 to ptr addrspace(1)
  %25 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @find(ptr addrspace(1) %17, ptr %25, ptr %16, ptr addrspace(1) %24)
  %26 = load i8, ptr %8, !tbaa !2, !range !7
  %27 = icmp eq i8 %26, 0
  br i1 %27, label %b4, label %b3

b2:
  %28 = addrspacecast ptr %6 to ptr addrspace(1)
  %29 = getelementptr i8, ptr @$str3, i16 6
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %30, ptr %32, !tbaa !2
  %33 = addrspacecast ptr %5 to ptr addrspace(1)
  %34 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @find(ptr addrspace(1) %28, ptr %34, ptr %16, ptr addrspace(1) %33)
  %35 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %36 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %36, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %30, ptr %37, !tbaa !2
  %38 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %35, ptr %16, ptr %34, ptr addrspace(1) %38)
  %39 = load i8, ptr %6
  %40 = getelementptr i8, ptr %6, i16 2
  %41 = load ptr addrspace(1), ptr %40
  %42 = load i8, ptr %4
  %43 = getelementptr i8, ptr %4, i16 2
  %44 = load ptr addrspace(1), ptr %43
  %45 = icmp eq i8 %39, 0
  br i1 %45, label %b8, label %b7

b3:
  %46 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %46)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %47 = getelementptr inbounds i8, ptr %8, i16 2
  %48 = load ptr addrspace(1), ptr %47, !tbaa !2
  %49 = load ptr, ptr addrspace(1) %48
  call addrspace(1) void @N$PS(ptr %49)
  %50 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %50)
  %51 = getelementptr i8, ptr addrspace(1) %48, i16 4
  %52 = load i16, ptr addrspace(1) %51
  call addrspace(1) void @N$PU2(i16 %52)
  %53 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %53)
  %54 = getelementptr i8, ptr addrspace(1) %48, i16 2
  %55 = load i16, ptr addrspace(1) %54
  call addrspace(1) void @N$PU2(i16 %55)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %56 = addrspacecast ptr %2 to ptr addrspace(5)
  %57 = addrspacecast ptr %2 to ptr addrspace(1)
  %58 = load ptr, ptr addrspace(5) %18
  call addrspace(1) void @affordable(ptr addrspace(1) %57, ptr %58, i16 20)
  %59 = load i16, ptr addrspace(5) %56
  %60 = addrspacecast ptr %1 to ptr addrspace(1)
  %61 = icmp ne i16 %59, 0
  br i1 %61, label %b11, label %b12

b7:
  %62 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %62)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %63 = icmp eq i8 %42, 0
  br i1 %63, label %64, label %b7

64:
  %65 = getelementptr i8, ptr addrspace(1) %41, i16 2
  %66 = load i16, ptr addrspace(1) %65
  %67 = getelementptr i8, ptr addrspace(1) %44, i16 2
  %68 = load i16, ptr addrspace(1) %67
  %69 = icmp ule i16 %66, %68
  br i1 %69, label %70, label %71

70:
  br label %72

71:
  br label %72

72:
  %73 = phi ptr addrspace(1) [ %65, %70 ], [ %67, %71 ]
  %74 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %74)
  %75 = load i16, ptr addrspace(1) %73
  call addrspace(1) void @N$PU2(i16 %75)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %76 = getelementptr i8, ptr addrspace(5) %56, i16 4
  %77 = load ptr addrspace(1), ptr addrspace(5) %76, !tbaa !2
  %78 = getelementptr inbounds i8, ptr addrspace(1) %77, i16 0
  %79 = load ptr, ptr addrspace(1) %78
  call addrspace(1) void @initial(ptr addrspace(1) %60, ptr %79)
  call addrspace(1) void @N$PU2(i16 %59)
  %80 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %80)
  call addrspace(1) void @N$PV(ptr addrspace(1) %60)
  call addrspace(1) void @N$PN()
  %81 = load ptr, ptr %12, !tbaa !2
  %82 = getelementptr i8, ptr %81, i16 -4
  %83 = load i16, ptr %82
  %84 = getelementptr i8, ptr @$str12, i16 6
  %85 = getelementptr i8, ptr @$str13, i16 6
  %86 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %87 = phi i16 [ 0, %b11 ], [ %94, %b16 ]
  %88 = icmp ult i16 %87, %83
  br i1 %88, label %b15, label %b17

b15:
  %89 = mul i16 %87, 6
  %90 = getelementptr inbounds i8, ptr %81, i16 %89
  %91 = getelementptr i8, ptr %90, i16 4
  %92 = load i16, ptr %91
  %93 = icmp ult i16 %92, 5
  br i1 %93, label %b18, label %b16

b16:
  %94 = add i16 %87, 1
  br label %b14

b17:
  %95 = getelementptr i8, ptr @$str15, i16 6
  %96 = addrspacecast ptr %95 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %97 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %97, !tbaa !2
  %98 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %96, ptr %98, !tbaa !2
  %99 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %19, ptr addrspace(1) %99, i16 1, i16 500)
  %100 = load ptr, ptr %12, !tbaa !2
  %101 = getelementptr i8, ptr %100, i16 -4
  %102 = load i16, ptr %101
  call addrspace(1) void @N$PU2(i16 %102)
  %103 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %103)
  call addrspace(1) void @N$PN()
  %104 = icmp ne ptr %16, null
  br i1 %104, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %84)
  %105 = load ptr, ptr %90
  call addrspace(1) void @N$PS(ptr %105)
  call addrspace(1) void @N$PS(ptr %85)
  %106 = load i16, ptr %91
  call addrspace(1) void @N$PU2(i16 %106)
  call addrspace(1) void @N$PS(ptr %86)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %16)
  %107 = load ptr, ptr %12, !tbaa !2
  %108 = icmp ne ptr %107, null
  br i1 %108, label %b28, label %b27

b23:
  %109 = getelementptr i8, ptr %16, i16 -4
  %110 = load i16, ptr %109
  br label %b24

b24:
  %111 = phi i16 [ 0, %b23 ], [ %116, %b26 ]
  %112 = icmp ult i16 %111, %110
  br i1 %112, label %b26, label %b25

b25:
  br label %b22

b26:
  %113 = mul i16 %111, 6
  %114 = getelementptr inbounds i8, ptr %16, i16 %113
  %115 = load ptr, ptr %114
  call addrspace(1) void @N$BDRP(ptr %115)
  %116 = add i16 %111, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %107)
  ret i16 0

b28:
  %117 = getelementptr i8, ptr %107, i16 -4
  %118 = load i16, ptr %117
  br label %b29

b29:
  %119 = phi i16 [ 0, %b28 ], [ %124, %b31 ]
  %120 = icmp ult i16 %119, %118
  br i1 %120, label %b31, label %b30

b30:
  br label %b27

b31:
  %121 = mul i16 %119, 6
  %122 = getelementptr inbounds i8, ptr %107, i16 %121
  %123 = load ptr, ptr %122
  call addrspace(1) void @N$BDRP(ptr %123)
  %124 = add i16 %119, 1
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
