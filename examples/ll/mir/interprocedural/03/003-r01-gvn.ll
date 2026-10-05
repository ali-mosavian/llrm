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
  %10 = alloca [8 x i8]
  %11 = alloca [6 x i8]
  %12 = alloca [8 x i8]
  %13 = alloca [6 x i8]
  %14 = alloca [8 x i8]
  %15 = alloca [6 x i8]
  %16 = alloca [2 x i8]
  %17 = alloca [2 x i8]
  %18 = alloca [2 x i8]
  %19 = getelementptr i8, ptr @$str1, i16 6
  store ptr %19, ptr %6, !tbaa !2
  %20 = addrspacecast ptr %6 to ptr addrspace(1)
  %21 = getelementptr i8, ptr @$str2, i16 6
  %22 = addrspacecast ptr %21 to ptr addrspace(1)
  store i16 4, ptr %5, !tbaa !2
  %23 = getelementptr inbounds i8, ptr %5, i16 2
  store i16 4, ptr %23, !tbaa !2
  %24 = getelementptr inbounds i8, ptr %5, i16 4
  store ptr addrspace(1) %22, ptr %24, !tbaa !2
  %25 = addrspacecast ptr %5 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %25, i16 5, i16 40)
  %26 = getelementptr i8, ptr @$str3, i16 6
  %27 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 4, ptr %4, !tbaa !2
  %28 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 4, ptr %28, !tbaa !2
  %29 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %27, ptr %29, !tbaa !2
  %30 = addrspacecast ptr %4 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %30, i16 30, i16 3)
  %31 = getelementptr i8, ptr @$str4, i16 6
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  store i16 3, ptr %3, !tbaa !2
  %33 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 3, ptr %33, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %32, ptr %34, !tbaa !2
  %35 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %20, ptr addrspace(1) %35, i16 12, i16 0)
  %36 = load ptr, ptr %6, !tbaa !2
  store ptr null, ptr %6, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  store ptr %36, ptr %18, !tbaa !2
  %37 = addrspacecast ptr %16 to ptr addrspace(5)
  %38 = addrspacecast ptr %16 to ptr addrspace(1)
  store ptr %19, ptr %2, !tbaa !2
  %39 = addrspacecast ptr %2 to ptr addrspace(1)
  store i16 4, ptr %1, !tbaa !2
  %40 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 4, ptr %40, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %27, ptr %41, !tbaa !2
  %42 = addrspacecast ptr %1 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %39, ptr addrspace(1) %42, i16 28, i16 9)
  %43 = getelementptr i8, ptr @$str5, i16 6
  %44 = addrspacecast ptr %43 to ptr addrspace(1)
  store i16 3, ptr %0, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 3, ptr %45, !tbaa !2
  %46 = getelementptr inbounds i8, ptr %0, i16 4
  store ptr addrspace(1) %44, ptr %46, !tbaa !2
  %47 = addrspacecast ptr %0 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %39, ptr addrspace(1) %47, i16 2, i16 100)
  %48 = load ptr, ptr %2, !tbaa !2
  store ptr %48, ptr addrspace(5) %37
  store ptr null, ptr %2, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  %49 = load ptr, ptr %16, !tbaa !2
  store ptr %49, ptr %17, !tbaa !2
  %50 = addrspacecast ptr %15 to ptr addrspace(1)
  %51 = addrspacecast ptr %18 to ptr addrspace(1)
  %52 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 3, ptr %14, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %14, i16 2
  store i16 3, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %14, i16 4
  store ptr addrspace(1) %44, ptr %54, !tbaa !2
  %55 = addrspacecast ptr %14 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %50, ptr addrspace(1) %51, ptr addrspace(1) %52, ptr addrspace(1) %55)
  %56 = load i8, ptr %15, !tbaa !2, !range !7
  %57 = icmp eq i8 %56, 0
  br i1 %57, label %b4, label %b3

b2:
  %58 = addrspacecast ptr %13 to ptr addrspace(1)
  store i16 4, ptr %12, !tbaa !2
  %59 = getelementptr inbounds i8, ptr %12, i16 2
  store i16 4, ptr %59, !tbaa !2
  %60 = getelementptr inbounds i8, ptr %12, i16 4
  store ptr addrspace(1) %27, ptr %60, !tbaa !2
  %61 = addrspacecast ptr %12 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %58, ptr addrspace(1) %51, ptr addrspace(1) %52, ptr addrspace(1) %61)
  %62 = addrspacecast ptr %11 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %63 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %63, !tbaa !2
  %64 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %27, ptr %64, !tbaa !2
  %65 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %62, ptr addrspace(1) %52, ptr addrspace(1) %51, ptr addrspace(1) %65)
  %66 = load i8, ptr %13
  %67 = getelementptr i8, ptr %13, i16 2
  %68 = load ptr addrspace(1), ptr %67
  %69 = load i8, ptr %11
  %70 = getelementptr i8, ptr %11, i16 2
  %71 = load ptr addrspace(1), ptr %70
  %72 = icmp eq i8 %66, 0
  br i1 %72, label %b8, label %b7

b3:
  %73 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %73)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %74 = getelementptr inbounds i8, ptr %15, i16 2
  %75 = load ptr addrspace(1), ptr %74, !tbaa !2
  %76 = load ptr, ptr addrspace(1) %75
  call addrspace(1) void @N$PS(ptr %76)
  %77 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %77)
  %78 = getelementptr i8, ptr addrspace(1) %75, i16 4
  %79 = load i16, ptr addrspace(1) %78
  call addrspace(1) void @N$PU2(i16 %79)
  %80 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %80)
  %81 = getelementptr i8, ptr addrspace(1) %75, i16 2
  %82 = load i16, ptr addrspace(1) %81
  call addrspace(1) void @N$PU2(i16 %82)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %83 = addrspacecast ptr %9 to ptr addrspace(5)
  %84 = addrspacecast ptr %9 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %84, ptr addrspace(1) %51, i16 20)
  %85 = load i16, ptr addrspace(5) %83
  %86 = addrspacecast ptr %8 to ptr addrspace(1)
  %87 = icmp ne i16 %85, 0
  br i1 %87, label %b11, label %b12

b7:
  %88 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %88)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %89 = icmp eq i8 %69, 0
  br i1 %89, label %90, label %b7

90:
  %91 = getelementptr i8, ptr addrspace(1) %68, i16 2
  %92 = load i16, ptr addrspace(1) %91
  %93 = getelementptr i8, ptr addrspace(1) %71, i16 2
  %94 = load i16, ptr addrspace(1) %93
  %95 = icmp ule i16 %92, %94
  br i1 %95, label %96, label %97

96:
  br label %98

97:
  br label %98

98:
  %99 = phi ptr addrspace(1) [ %91, %96 ], [ %93, %97 ]
  %100 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %100)
  %101 = load i16, ptr addrspace(1) %99
  call addrspace(1) void @N$PU2(i16 %101)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %102 = getelementptr i8, ptr addrspace(5) %83, i16 4
  %103 = load ptr addrspace(1), ptr addrspace(5) %102, !tbaa !2
  %104 = getelementptr inbounds i8, ptr addrspace(1) %103, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %86, ptr addrspace(1) %104)
  call addrspace(1) void @N$PU2(i16 %85)
  %105 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %105)
  call addrspace(1) void @N$PV(ptr addrspace(1) %86)
  call addrspace(1) void @N$PN()
  %106 = load ptr, ptr %18, !tbaa !2
  %107 = getelementptr i8, ptr %106, i16 -4
  %108 = load i16, ptr %107
  %109 = getelementptr i8, ptr @$str12, i16 6
  %110 = getelementptr i8, ptr @$str13, i16 6
  %111 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %112 = phi i16 [ 0, %b11 ], [ %119, %b16 ]
  %113 = icmp ult i16 %112, %108
  br i1 %113, label %b15, label %b17

b15:
  %114 = mul i16 %112, 6
  %115 = getelementptr inbounds i8, ptr %106, i16 %114
  %116 = getelementptr i8, ptr %115, i16 4
  %117 = load i16, ptr %116
  %118 = icmp ult i16 %117, 5
  br i1 %118, label %b18, label %b16

b16:
  %119 = add i16 %112, 1
  br label %b14

b17:
  %120 = getelementptr i8, ptr @$str15, i16 6
  %121 = addrspacecast ptr %120 to ptr addrspace(1)
  store i16 3, ptr %7, !tbaa !2
  %122 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 3, ptr %122, !tbaa !2
  %123 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %121, ptr %123, !tbaa !2
  %124 = addrspacecast ptr %7 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %51, ptr addrspace(1) %124, i16 1, i16 500)
  %125 = load ptr, ptr %18, !tbaa !2
  %126 = getelementptr i8, ptr %125, i16 -4
  %127 = load i16, ptr %126
  call addrspace(1) void @N$PU2(i16 %127)
  %128 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %128)
  call addrspace(1) void @N$PN()
  %129 = load ptr, ptr %17, !tbaa !2
  %130 = icmp ne ptr %129, null
  br i1 %130, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %109)
  %131 = load ptr, ptr %115
  call addrspace(1) void @N$PS(ptr %131)
  call addrspace(1) void @N$PS(ptr %110)
  %132 = load i16, ptr %116
  call addrspace(1) void @N$PU2(i16 %132)
  call addrspace(1) void @N$PS(ptr %111)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %129)
  %133 = load ptr, ptr %18, !tbaa !2
  %134 = icmp ne ptr %133, null
  br i1 %134, label %b28, label %b27

b23:
  %135 = getelementptr i8, ptr %129, i16 -4
  %136 = load i16, ptr %135
  br label %b24

b24:
  %137 = phi i16 [ 0, %b23 ], [ %142, %b26 ]
  %138 = icmp ult i16 %137, %136
  br i1 %138, label %b26, label %b25

b25:
  br label %b22

b26:
  %139 = mul i16 %137, 6
  %140 = getelementptr inbounds i8, ptr %129, i16 %139
  %141 = load ptr, ptr %140
  call addrspace(1) void @N$BDRP(ptr %141)
  %142 = add i16 %137, 1
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %133)
  ret i16 0

b28:
  %143 = getelementptr i8, ptr %133, i16 -4
  %144 = load i16, ptr %143
  br label %b29

b29:
  %145 = phi i16 [ 0, %b28 ], [ %150, %b31 ]
  %146 = icmp ult i16 %145, %144
  br i1 %146, label %b31, label %b30

b30:
  br label %b27

b31:
  %147 = mul i16 %145, 6
  %148 = getelementptr inbounds i8, ptr %133, i16 %147
  %149 = load ptr, ptr %148
  call addrspace(1) void @N$BDRP(ptr %149)
  %150 = add i16 %145, 1
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
