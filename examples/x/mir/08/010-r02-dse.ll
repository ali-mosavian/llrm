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
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca i16
  %4 = alloca [8 x i8]
  %5 = alloca i16
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
  %17 = addrspacecast ptr %15 to ptr addrspace(1)
  call addrspace(1) void @north(ptr addrspace(1) %17)
  %18 = load ptr, ptr %15, !tbaa !2
  store ptr %18, ptr %16, !tbaa !2
  %19 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %19)
  %20 = load ptr, ptr %13, !tbaa !2
  store ptr %20, ptr %14, !tbaa !2
  %21 = addrspacecast ptr %12 to ptr addrspace(1)
  %22 = addrspacecast ptr %16 to ptr addrspace(1)
  %23 = addrspacecast ptr %14 to ptr addrspace(1)
  %24 = getelementptr i8, ptr @$str5, i16 6
  %25 = addrspacecast ptr %24 to ptr addrspace(1)
  store i16 3, ptr %11, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %11, i16 2
  store i16 3, ptr %26, !tbaa !2
  %27 = getelementptr inbounds i8, ptr %11, i16 4
  store ptr addrspace(1) %25, ptr %27, !tbaa !2
  %28 = addrspacecast ptr %11 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %21, ptr addrspace(1) %22, ptr addrspace(1) %23, ptr addrspace(1) %28)
  %29 = load i8, ptr %12, !tbaa !2, !range !7
  %30 = icmp eq i8 %29, 0
  br i1 %30, label %b4, label %b3

b2:
  %31 = addrspacecast ptr %10 to ptr addrspace(1)
  %32 = getelementptr i8, ptr @$str3, i16 6
  %33 = addrspacecast ptr %32 to ptr addrspace(1)
  store i16 4, ptr %9, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %9, i16 2
  store i16 4, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %9, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %31, ptr addrspace(1) %22, ptr addrspace(1) %23, ptr addrspace(1) %36)
  %37 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %7, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 4, ptr %38, !tbaa !2
  %39 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %33, ptr %39, !tbaa !2
  %40 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %37, ptr addrspace(1) %23, ptr addrspace(1) %22, ptr addrspace(1) %40)
  %41 = load i8, ptr %10
  %42 = getelementptr i8, ptr %10, i16 2
  %43 = load ptr addrspace(1), ptr %42
  %44 = load i8, ptr %8
  %45 = getelementptr i8, ptr %8, i16 2
  %46 = load ptr addrspace(1), ptr %45
  %47 = icmp eq i8 %41, 0
  br i1 %47, label %b8, label %b7

b3:
  %48 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %48)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %49 = getelementptr inbounds i8, ptr %12, i16 2
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
  %58 = addrspacecast ptr %6 to ptr addrspace(5)
  %59 = addrspacecast ptr %6 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %59, ptr addrspace(1) %22, i16 20)
  %60 = load i16, ptr addrspace(5) %58
  %61 = addrspacecast ptr %4 to ptr addrspace(1)
  %62 = icmp ne i16 %60, 0
  br i1 %62, label %b11, label %b12

b7:
  %63 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %63)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %64 = icmp eq i8 %44, 0
  br i1 %64, label %b9, label %b7

b9:
  %65 = call addrspace(1) ptr addrspace(1) @cheaper(ptr addrspace(1) %43, ptr addrspace(1) %46)
  %66 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %66)
  %67 = getelementptr i8, ptr addrspace(1) %65, i16 2
  %68 = load i16, ptr addrspace(1) %67
  call addrspace(1) void @N$PU2(i16 %68)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %69 = getelementptr i8, ptr addrspace(5) %58, i16 4
  %70 = load ptr addrspace(1), ptr addrspace(5) %69, !tbaa !2
  %71 = getelementptr inbounds i8, ptr addrspace(1) %70, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %61, ptr addrspace(1) %71)
  call addrspace(1) void @N$PU2(i16 %60)
  %72 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %72)
  call addrspace(1) void @N$PV(ptr addrspace(1) %61)
  call addrspace(1) void @N$PN()
  %73 = load ptr, ptr %16, !tbaa !2
  %74 = getelementptr i8, ptr %73, i16 -4
  %75 = load i16, ptr %74
  %76 = getelementptr i8, ptr @$str12, i16 6
  %77 = getelementptr i8, ptr @$str13, i16 6
  %78 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %79 = phi i16 [ 0, %b11 ], [ %86, %b16 ]
  %80 = icmp ult i16 %79, %75
  br i1 %80, label %b15, label %b17

b15:
  %81 = mul i16 %79, 6
  %82 = getelementptr inbounds i8, ptr %73, i16 %81
  %83 = getelementptr i8, ptr %82, i16 4
  %84 = load i16, ptr %83
  %85 = icmp ult i16 %84, 5
  br i1 %85, label %b18, label %b16

b16:
  %86 = add i16 %79, 1
  br label %b14

b17:
  %87 = getelementptr i8, ptr @$str15, i16 6
  %88 = addrspacecast ptr %87 to ptr addrspace(1)
  store i16 3, ptr %2, !tbaa !2
  %89 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 3, ptr %89, !tbaa !2
  %90 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %88, ptr %90, !tbaa !2
  %91 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %22, ptr addrspace(1) %91, i16 1, i16 500)
  %92 = load ptr, ptr %16, !tbaa !2
  %93 = getelementptr i8, ptr %92, i16 -4
  %94 = load i16, ptr %93
  call addrspace(1) void @N$PU2(i16 %94)
  %95 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %95)
  call addrspace(1) void @N$PN()
  %96 = load ptr, ptr %14, !tbaa !2
  %97 = icmp ne ptr %96, null
  br i1 %97, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %76)
  %98 = load ptr, ptr %82
  call addrspace(1) void @N$PS(ptr %98)
  call addrspace(1) void @N$PS(ptr %77)
  %99 = load i16, ptr %83
  call addrspace(1) void @N$PU2(i16 %99)
  call addrspace(1) void @N$PS(ptr %78)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %96)
  %100 = load ptr, ptr %16, !tbaa !2
  %101 = icmp ne ptr %100, null
  br i1 %101, label %b28, label %b27

b23:
  %102 = getelementptr i8, ptr %96, i16 -4
  %103 = load i16, ptr %102
  br label %b24

b24:
  %104 = phi i16 [ 0, %b23 ], [ %109, %b26 ]
  %105 = icmp ult i16 %104, %103
  br i1 %105, label %b26, label %b25

b25:
  br label %b22

b26:
  %106 = mul i16 %104, 6
  %107 = getelementptr inbounds i8, ptr %96, i16 %106
  %108 = load ptr, ptr %107
  call addrspace(1) void @N$BDRP(ptr %108)
  %109 = add i16 %104, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %100)
  ret i16 0

b28:
  %110 = getelementptr i8, ptr %100, i16 -4
  %111 = load i16, ptr %110
  br label %b29

b29:
  %112 = phi i16 [ 0, %b28 ], [ %117, %b31 ]
  %113 = icmp ult i16 %112, %111
  br i1 %113, label %b31, label %b30

b30:
  br label %b27

b31:
  %114 = mul i16 %112, 6
  %115 = getelementptr inbounds i8, ptr %100, i16 %114
  %116 = load ptr, ptr %115
  call addrspace(1) void @N$BDRP(ptr %116)
  %117 = add i16 %112, 1
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
