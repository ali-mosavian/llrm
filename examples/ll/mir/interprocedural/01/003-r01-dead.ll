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
  %18 = addrspacecast ptr %12 to ptr addrspace(1)
  %19 = addrspacecast ptr %10 to ptr addrspace(1)
  %20 = getelementptr i8, ptr @$str5, i16 6
  %21 = addrspacecast ptr %20 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %22 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %22, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %21, ptr %23, !tbaa !2
  %24 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %17, ptr addrspace(1) %18, ptr addrspace(1) %19, ptr addrspace(1) %24)
  %25 = load i8, ptr %8, !tbaa !2, !range !7
  %26 = icmp eq i8 %25, 0
  br i1 %26, label %b4, label %b3

b2:
  %27 = addrspacecast ptr %6 to ptr addrspace(1)
  %28 = getelementptr i8, ptr @$str3, i16 6
  %29 = addrspacecast ptr %28 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %27, ptr addrspace(1) %18, ptr addrspace(1) %19, ptr addrspace(1) %32)
  %33 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %29, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %33, ptr addrspace(1) %19, ptr addrspace(1) %18, ptr addrspace(1) %36)
  %37 = load i8, ptr %6
  %38 = getelementptr i8, ptr %6, i16 2
  %39 = load ptr addrspace(1), ptr %38
  %40 = load i8, ptr %4
  %41 = getelementptr i8, ptr %4, i16 2
  %42 = load ptr addrspace(1), ptr %41
  %43 = icmp eq i8 %37, 0
  br i1 %43, label %b8, label %b7

b3:
  %44 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %44)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %45 = getelementptr inbounds i8, ptr %8, i16 2
  %46 = load ptr addrspace(1), ptr %45, !tbaa !2
  %47 = load ptr, ptr addrspace(1) %46
  call addrspace(1) void @N$PS(ptr %47)
  %48 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  %49 = getelementptr i8, ptr addrspace(1) %46, i16 4
  %50 = load i16, ptr addrspace(1) %49
  call addrspace(1) void @N$PU2(i16 %50)
  %51 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %51)
  %52 = getelementptr i8, ptr addrspace(1) %46, i16 2
  %53 = load i16, ptr addrspace(1) %52
  call addrspace(1) void @N$PU2(i16 %53)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %54 = addrspacecast ptr %2 to ptr addrspace(5)
  %55 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %55, ptr addrspace(1) %18, i16 20)
  %56 = load i16, ptr addrspace(5) %54
  %57 = addrspacecast ptr %1 to ptr addrspace(1)
  %58 = icmp ne i16 %56, 0
  br i1 %58, label %b11, label %b12

b7:
  %59 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %59)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %60 = icmp eq i8 %40, 0
  br i1 %60, label %61, label %b7

61:
  %62 = getelementptr i8, ptr addrspace(1) %39, i16 2
  %63 = load i16, ptr addrspace(1) %62
  %64 = getelementptr i8, ptr addrspace(1) %42, i16 2
  %65 = load i16, ptr addrspace(1) %64
  %66 = icmp ule i16 %63, %65
  br i1 %66, label %67, label %68

67:
  br label %69

68:
  br label %69

69:
  %70 = phi ptr addrspace(1) [ %62, %67 ], [ %64, %68 ]
  %71 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %71)
  %72 = load i16, ptr addrspace(1) %70
  call addrspace(1) void @N$PU2(i16 %72)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %73 = getelementptr i8, ptr addrspace(5) %54, i16 4
  %74 = load ptr addrspace(1), ptr addrspace(5) %73, !tbaa !2
  %75 = getelementptr inbounds i8, ptr addrspace(1) %74, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %57, ptr addrspace(1) %75)
  call addrspace(1) void @N$PU2(i16 %56)
  %76 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  call addrspace(1) void @N$PV(ptr addrspace(1) %57)
  call addrspace(1) void @N$PN()
  %77 = load ptr, ptr %12, !tbaa !2
  %78 = getelementptr i8, ptr %77, i16 -4
  %79 = load i16, ptr %78
  %80 = getelementptr i8, ptr @$str12, i16 6
  %81 = getelementptr i8, ptr @$str13, i16 6
  %82 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %83 = phi i16 [ 0, %b11 ], [ %90, %b16 ]
  %84 = icmp ult i16 %83, %79
  br i1 %84, label %b15, label %b17

b15:
  %85 = mul i16 %83, 6
  %86 = getelementptr inbounds i8, ptr %77, i16 %85
  %87 = getelementptr i8, ptr %86, i16 4
  %88 = load i16, ptr %87
  %89 = icmp ult i16 %88, 5
  br i1 %89, label %b18, label %b16

b16:
  %90 = add i16 %83, 1
  br label %b14

b17:
  %91 = getelementptr i8, ptr @$str15, i16 6
  %92 = addrspacecast ptr %91 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %93 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %93, !tbaa !2
  %94 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %92, ptr %94, !tbaa !2
  %95 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %18, ptr addrspace(1) %95, i16 1, i16 500)
  %96 = load ptr, ptr %12, !tbaa !2
  %97 = getelementptr i8, ptr %96, i16 -4
  %98 = load i16, ptr %97
  call addrspace(1) void @N$PU2(i16 %98)
  %99 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %99)
  call addrspace(1) void @N$PN()
  %100 = load ptr, ptr %10, !tbaa !2
  %101 = icmp ne ptr %100, null
  br i1 %101, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %80)
  %102 = load ptr, ptr %86
  call addrspace(1) void @N$PS(ptr %102)
  call addrspace(1) void @N$PS(ptr %81)
  %103 = load i16, ptr %87
  call addrspace(1) void @N$PU2(i16 %103)
  call addrspace(1) void @N$PS(ptr %82)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %100)
  %104 = load ptr, ptr %12, !tbaa !2
  %105 = icmp ne ptr %104, null
  br i1 %105, label %b28, label %b27

b23:
  %106 = getelementptr i8, ptr %100, i16 -4
  %107 = load i16, ptr %106
  br label %b24

b24:
  %108 = phi i16 [ 0, %b23 ], [ %113, %b26 ]
  %109 = icmp ult i16 %108, %107
  br i1 %109, label %b26, label %b25

b25:
  br label %b22

b26:
  %110 = mul i16 %108, 6
  %111 = getelementptr inbounds i8, ptr %100, i16 %110
  %112 = load ptr, ptr %111
  call addrspace(1) void @N$BDRP(ptr %112)
  %113 = add i16 %108, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %104)
  ret i16 0

b28:
  %114 = getelementptr i8, ptr %104, i16 -4
  %115 = load i16, ptr %114
  br label %b29

b29:
  %116 = phi i16 [ 0, %b28 ], [ %121, %b31 ]
  %117 = icmp ult i16 %116, %115
  br i1 %117, label %b31, label %b30

b30:
  br label %b27

b31:
  %118 = mul i16 %116, 6
  %119 = getelementptr inbounds i8, ptr %104, i16 %118
  %120 = load ptr, ptr %119
  call addrspace(1) void @N$BDRP(ptr %120)
  %121 = add i16 %116, 1
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
