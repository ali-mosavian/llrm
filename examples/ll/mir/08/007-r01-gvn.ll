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
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  %9 = alloca [6 x i8]
  %10 = alloca [8 x i8]
  %11 = alloca [6 x i8]
  %12 = alloca [12 x i8]
  %13 = alloca [8 x i8]
  %14 = alloca [6 x i8]
  %15 = alloca [2 x i8]
  %16 = alloca [2 x i8]
  %17 = alloca [2 x i8]
  %18 = alloca [2 x i8]
  %19 = addrspacecast ptr %17 to ptr addrspace(1)
  call addrspace(1) void @north(ptr addrspace(1) %19)
  %20 = load ptr, ptr %17, !tbaa !2
  store ptr %20, ptr %18, !tbaa !2
  %21 = addrspacecast ptr %15 to ptr addrspace(1)
  call addrspace(1) void @south(ptr addrspace(1) %21)
  %22 = load ptr, ptr %15, !tbaa !2
  store ptr %22, ptr %16, !tbaa !2
  %23 = addrspacecast ptr %14 to ptr addrspace(1)
  %24 = addrspacecast ptr %18 to ptr addrspace(1)
  %25 = addrspacecast ptr %16 to ptr addrspace(1)
  %26 = getelementptr i8, ptr @$str5, i16 6
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 3, ptr %13, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 3, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %23, ptr addrspace(1) %24, ptr addrspace(1) %25, ptr addrspace(1) %32)
  %33 = load i8, ptr %14, !tbaa !2, !range !7
  %34 = icmp eq i8 %33, 0
  %35 = zext i1 %34 to i8
  br i1 %34, label %b4, label %b3

b2:
  %36 = addrspacecast ptr %11 to ptr addrspace(1)
  %37 = getelementptr i8, ptr @$str3, i16 6
  %38 = getelementptr i8, ptr %37, i16 -4
  %39 = load i16, ptr %38
  %40 = addrspacecast ptr %37 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %40, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %36, ptr addrspace(1) %24, ptr addrspace(1) %25, ptr addrspace(1) %43)
  %44 = addrspacecast ptr %9 to ptr addrspace(1)
  store i16 4, ptr %8, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 4, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %40, ptr %46, !tbaa !2
  %47 = addrspacecast ptr %8 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %44, ptr addrspace(1) %25, ptr addrspace(1) %24, ptr addrspace(1) %47)
  %48 = load i8, ptr %11
  %49 = getelementptr i8, ptr %11, i16 2
  %50 = load ptr addrspace(1), ptr %49
  %51 = getelementptr i8, ptr %12, i16 2
  %52 = getelementptr inbounds i8, ptr %12, i16 6
  %53 = load i8, ptr %9
  %54 = getelementptr i8, ptr %9, i16 2
  %55 = load ptr addrspace(1), ptr %54
  %56 = getelementptr i8, ptr %52, i16 2
  %57 = icmp eq i8 %48, 0
  %58 = zext i1 %57 to i8
  br i1 %57, label %b8, label %b7

b3:
  %59 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %59)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %60 = getelementptr inbounds i8, ptr %14, i16 2
  %61 = load ptr addrspace(1), ptr %60, !tbaa !2
  %62 = load ptr, ptr addrspace(1) %61
  call addrspace(1) void @N$PS(ptr %62)
  %63 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %63)
  %64 = getelementptr i8, ptr addrspace(1) %61, i16 4
  %65 = load i16, ptr addrspace(1) %64
  call addrspace(1) void @N$PU2(i16 %65)
  %66 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %66)
  %67 = getelementptr i8, ptr addrspace(1) %61, i16 2
  %68 = load i16, ptr addrspace(1) %67
  call addrspace(1) void @N$PU2(i16 %68)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %69 = addrspacecast ptr %7 to ptr addrspace(5)
  %70 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %70, ptr addrspace(1) %24, i16 20)
  %71 = load i16, ptr addrspace(5) %69
  store i16 %71, ptr %6, !tbaa !2
  %72 = addrspacecast ptr %5 to ptr addrspace(1)
  %73 = icmp ne i16 %71, 0
  %74 = zext i1 %73 to i8
  br i1 %73, label %b11, label %b12

b7:
  %75 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %75)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %76 = getelementptr inbounds i8, ptr %12, i16 2
  %77 = icmp eq i8 %53, 0
  %78 = zext i1 %77 to i8
  br i1 %77, label %b9, label %b7

b9:
  %79 = getelementptr inbounds i8, ptr %12, i16 8
  %80 = call addrspace(1) ptr addrspace(1) @cheaper(ptr addrspace(1) %50, ptr addrspace(1) %55)
  %81 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %81)
  %82 = getelementptr i8, ptr addrspace(1) %80, i16 2
  %83 = load i16, ptr addrspace(1) %82
  call addrspace(1) void @N$PU2(i16 %83)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %84 = getelementptr i8, ptr addrspace(5) %69, i16 4
  %85 = getelementptr i8, ptr addrspace(1) %70, i16 4
  %86 = load ptr addrspace(1), ptr addrspace(5) %84, !tbaa !2
  %87 = getelementptr inbounds i8, ptr addrspace(1) %86, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %72, ptr addrspace(1) %87)
  call addrspace(1) void @N$PU2(i16 %71)
  %88 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %88)
  call addrspace(1) void @N$PV(ptr addrspace(1) %72)
  call addrspace(1) void @N$PN()
  %89 = load ptr, ptr %18, !tbaa !2
  %90 = getelementptr i8, ptr %89, i16 -4
  %91 = load i16, ptr %90
  %92 = addrspacecast ptr %89 to ptr addrspace(1)
  %93 = getelementptr inbounds i8, ptr %4, i16 2
  %94 = getelementptr inbounds i8, ptr %4, i16 4
  %95 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 0, ptr %3, !tbaa !2
  %96 = getelementptr i8, ptr addrspace(1) %95, i16 4
  %97 = getelementptr i8, ptr @$str12, i16 6
  %98 = getelementptr i8, ptr @$str13, i16 6
  %99 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %100 = load i16, ptr %3, !tbaa !2
  %101 = icmp ult i16 %100, %91
  %102 = zext i1 %101 to i8
  br i1 %101, label %b15, label %b17

b15:
  %103 = mul i16 %100, 6
  %104 = getelementptr inbounds i8, ptr %89, i16 %103
  %105 = getelementptr inbounds i8, ptr addrspace(1) %92, i16 %103
  %106 = getelementptr i8, ptr %104, i16 4
  %107 = getelementptr i8, ptr addrspace(1) %105, i16 4
  %108 = load i16, ptr %106
  %109 = icmp ult i16 %108, 5
  %110 = zext i1 %109 to i8
  br i1 %109, label %b18, label %b16

b16:
  %111 = add i16 %100, 1
  store i16 %111, ptr %3, !tbaa !2
  br label %b14

b17:
  %112 = getelementptr i8, ptr @$str15, i16 6
  %113 = getelementptr i8, ptr %112, i16 -4
  %114 = load i16, ptr %113
  %115 = addrspacecast ptr %112 to ptr addrspace(1)
  store i16 3, ptr %2, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 3, ptr %116, !tbaa !2
  %117 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %115, ptr %117, !tbaa !2
  %118 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %24, ptr addrspace(1) %118, i16 1, i16 500)
  %119 = load ptr, ptr %18, !tbaa !2
  %120 = getelementptr i8, ptr %119, i16 -4
  %121 = load i16, ptr %120
  call addrspace(1) void @N$PU2(i16 %121)
  %122 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %122)
  call addrspace(1) void @N$PN()
  %123 = load ptr, ptr %16, !tbaa !2
  %124 = icmp ne ptr %123, null
  %125 = zext i1 %124 to i8
  br i1 %124, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %97)
  %126 = load ptr, ptr %104
  call addrspace(1) void @N$PS(ptr %126)
  call addrspace(1) void @N$PS(ptr %98)
  %127 = load i16, ptr %106
  call addrspace(1) void @N$PU2(i16 %127)
  call addrspace(1) void @N$PS(ptr %99)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %123)
  %128 = load ptr, ptr %18, !tbaa !2
  %129 = icmp ne ptr %128, null
  %130 = zext i1 %129 to i8
  br i1 %129, label %b28, label %b27

b23:
  %131 = getelementptr i8, ptr %123, i16 -4
  %132 = load i16, ptr %131
  store i16 0, ptr %1, !tbaa !2
  br label %b24

b24:
  %133 = load i16, ptr %1, !tbaa !2
  %134 = icmp ult i16 %133, %132
  %135 = zext i1 %134 to i8
  br i1 %134, label %b26, label %b25

b25:
  br label %b22

b26:
  %136 = mul i16 %133, 6
  %137 = getelementptr inbounds i8, ptr %123, i16 %136
  %138 = load ptr, ptr %137
  call addrspace(1) void @N$BDRP(ptr %138)
  %139 = add i16 %133, 1
  store i16 %139, ptr %1, !tbaa !2
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %128)
  ret i16 0

b28:
  %140 = getelementptr i8, ptr %128, i16 -4
  %141 = load i16, ptr %140
  store i16 0, ptr %0, !tbaa !2
  br label %b29

b29:
  %142 = load i16, ptr %0, !tbaa !2
  %143 = icmp ult i16 %142, %141
  %144 = zext i1 %143 to i8
  br i1 %143, label %b31, label %b30

b30:
  br label %b27

b31:
  %145 = mul i16 %142, 6
  %146 = getelementptr inbounds i8, ptr %128, i16 %145
  %147 = load ptr, ptr %146
  call addrspace(1) void @N$BDRP(ptr %147)
  %148 = add i16 %142, 1
  store i16 %148, ptr %0, !tbaa !2
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
