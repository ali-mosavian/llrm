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
  %37 = addrspacecast ptr %18 to ptr addrspace(1)
  %38 = addrspacecast ptr %16 to ptr addrspace(1)
  %39 = getelementptr i8, ptr @$str3, i16 6
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  %42 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 4, ptr %10, !tbaa !2
  %43 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 4, ptr %43, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %42, ptr %44, !tbaa !2
  %45 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %36, ptr addrspace(1) %37, ptr addrspace(1) %38, ptr addrspace(1) %45)
  %46 = addrspacecast ptr %9 to ptr addrspace(1)
  %47 = addrspacecast ptr %16 to ptr addrspace(1)
  %48 = addrspacecast ptr %18 to ptr addrspace(1)
  %49 = getelementptr i8, ptr @$str3, i16 6
  %50 = getelementptr i8, ptr %49, i16 -4
  %51 = load i16, ptr %50
  %52 = addrspacecast ptr %49 to ptr addrspace(1)
  store i16 4, ptr %8, !tbaa !2
  %53 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 4, ptr %53, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %52, ptr %54, !tbaa !2
  %55 = addrspacecast ptr %8 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %46, ptr addrspace(1) %47, ptr addrspace(1) %48, ptr addrspace(1) %55)
  %56 = load i8, ptr %11
  %57 = getelementptr i8, ptr %11, i16 2
  %58 = load ptr addrspace(1), ptr %57
  %59 = getelementptr i8, ptr %12, i16 2
  %60 = getelementptr inbounds i8, ptr %12, i16 6
  %61 = load i8, ptr %9
  %62 = getelementptr i8, ptr %9, i16 2
  %63 = load ptr addrspace(1), ptr %62
  %64 = getelementptr i8, ptr %60, i16 2
  %65 = icmp eq i8 %56, 0
  %66 = zext i1 %65 to i8
  br i1 %65, label %b8, label %b7

b3:
  %67 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %67)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %68 = getelementptr inbounds i8, ptr %14, i16 2
  %69 = load ptr addrspace(1), ptr %68, !tbaa !2
  %70 = getelementptr inbounds i8, ptr %14, i16 2
  %71 = load ptr addrspace(1), ptr %70, !tbaa !2
  %72 = load ptr, ptr addrspace(1) %71
  call addrspace(1) void @N$PS(ptr %72)
  %73 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %73)
  %74 = getelementptr i8, ptr addrspace(1) %71, i16 4
  %75 = load i16, ptr addrspace(1) %74
  call addrspace(1) void @N$PU2(i16 %75)
  %76 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %76)
  %77 = getelementptr i8, ptr addrspace(1) %71, i16 2
  %78 = load i16, ptr addrspace(1) %77
  call addrspace(1) void @N$PU2(i16 %78)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %79 = addrspacecast ptr %7 to ptr addrspace(5)
  %80 = addrspacecast ptr %7 to ptr addrspace(1)
  %81 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %80, ptr addrspace(1) %81, i16 20)
  %82 = load i16, ptr addrspace(5) %79
  store i16 %82, ptr %6, !tbaa !2
  %83 = addrspacecast ptr %5 to ptr addrspace(1)
  %84 = load i16, ptr addrspace(5) %79, !tbaa !2, !range !6
  %85 = icmp ne i16 %84, 0
  %86 = zext i1 %85 to i8
  br i1 %85, label %b11, label %b12

b7:
  %87 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %87)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %88 = getelementptr inbounds i8, ptr %12, i16 2
  %89 = getelementptr inbounds i8, ptr %12, i16 6
  %90 = icmp eq i8 %61, 0
  %91 = zext i1 %90 to i8
  br i1 %90, label %b9, label %b7

b9:
  %92 = getelementptr inbounds i8, ptr %12, i16 8
  %93 = getelementptr inbounds i8, ptr %12, i16 2
  %94 = getelementptr inbounds i8, ptr %12, i16 8
  %95 = call addrspace(1) ptr addrspace(1) @cheaper(ptr addrspace(1) %58, ptr addrspace(1) %63)
  %96 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %96)
  %97 = getelementptr i8, ptr addrspace(1) %95, i16 2
  %98 = load i16, ptr addrspace(1) %97
  call addrspace(1) void @N$PU2(i16 %98)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %99 = getelementptr i8, ptr addrspace(5) %79, i16 4
  %100 = getelementptr i8, ptr addrspace(1) %80, i16 4
  %101 = load ptr addrspace(1), ptr addrspace(5) %99, !tbaa !2
  %102 = getelementptr inbounds i8, ptr addrspace(1) %101, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %83, ptr addrspace(1) %102)
  %103 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %103)
  %104 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %104)
  call addrspace(1) void @N$PV(ptr addrspace(1) %83)
  call addrspace(1) void @N$PN()
  %105 = load ptr, ptr %18, !tbaa !2
  %106 = getelementptr i8, ptr %105, i16 -4
  %107 = load i16, ptr %106
  %108 = addrspacecast ptr %105 to ptr addrspace(1)
  %109 = getelementptr inbounds i8, ptr %4, i16 2
  %110 = getelementptr inbounds i8, ptr %4, i16 4
  %111 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 0, ptr %3, !tbaa !2
  %112 = getelementptr i8, ptr addrspace(1) %111, i16 4
  %113 = getelementptr i8, ptr @$str12, i16 6
  %114 = getelementptr i8, ptr @$str13, i16 6
  %115 = getelementptr i8, ptr @$str14, i16 6
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %116 = load i16, ptr %3, !tbaa !2
  %117 = icmp ult i16 %116, %107
  %118 = zext i1 %117 to i8
  br i1 %117, label %b15, label %b17

b15:
  %119 = mul i16 %116, 6
  %120 = getelementptr inbounds i8, ptr %105, i16 %119
  %121 = getelementptr inbounds i8, ptr addrspace(1) %108, i16 %119
  %122 = getelementptr i8, ptr %120, i16 4
  %123 = getelementptr i8, ptr addrspace(1) %121, i16 4
  %124 = load i16, ptr %122
  %125 = icmp ult i16 %124, 5
  %126 = zext i1 %125 to i8
  br i1 %125, label %b18, label %b16

b16:
  %127 = load i16, ptr %3, !tbaa !2
  %128 = add i16 %127, 1
  store i16 %128, ptr %3, !tbaa !2
  br label %b14

b17:
  %129 = addrspacecast ptr %18 to ptr addrspace(1)
  %130 = getelementptr i8, ptr @$str15, i16 6
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  %133 = addrspacecast ptr %130 to ptr addrspace(1)
  store i16 3, ptr %2, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 3, ptr %134, !tbaa !2
  %135 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %133, ptr %135, !tbaa !2
  %136 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %129, ptr addrspace(1) %136, i16 1, i16 500)
  %137 = load ptr, ptr %18, !tbaa !2
  %138 = getelementptr i8, ptr %137, i16 -4
  %139 = load i16, ptr %138
  call addrspace(1) void @N$PU2(i16 %139)
  %140 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %140)
  call addrspace(1) void @N$PN()
  %141 = load ptr, ptr %16, !tbaa !2
  %142 = icmp ne ptr %141, null
  %143 = zext i1 %142 to i8
  br i1 %142, label %b23, label %b22

b18:
  call addrspace(1) void @N$PS(ptr %113)
  %144 = load ptr, ptr %120
  call addrspace(1) void @N$PS(ptr %144)
  call addrspace(1) void @N$PS(ptr %114)
  %145 = getelementptr i8, ptr %120, i16 4
  %146 = getelementptr i8, ptr addrspace(1) %121, i16 4
  %147 = load i16, ptr %145
  call addrspace(1) void @N$PU2(i16 %147)
  call addrspace(1) void @N$PS(ptr %115)
  call addrspace(1) void @N$PN()
  br label %b16

b22:
  call addrspace(1) void @N$BDRP(ptr %141)
  %148 = load ptr, ptr %18, !tbaa !2
  %149 = icmp ne ptr %148, null
  %150 = zext i1 %149 to i8
  br i1 %149, label %b28, label %b27

b23:
  %151 = getelementptr i8, ptr %141, i16 -4
  %152 = load i16, ptr %151
  store i16 0, ptr %1, !tbaa !2
  br label %b24

b24:
  %153 = load i16, ptr %1, !tbaa !2
  %154 = icmp ult i16 %153, %152
  %155 = zext i1 %154 to i8
  br i1 %154, label %b26, label %b25

b25:
  br label %b22

b26:
  %156 = mul i16 %153, 6
  %157 = getelementptr inbounds i8, ptr %141, i16 %156
  %158 = load ptr, ptr %157
  call addrspace(1) void @N$BDRP(ptr %158)
  %159 = add i16 %153, 1
  store i16 %159, ptr %1, !tbaa !2
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %148)
  ret i16 0

b28:
  %160 = getelementptr i8, ptr %148, i16 -4
  %161 = load i16, ptr %160
  store i16 0, ptr %0, !tbaa !2
  br label %b29

b29:
  %162 = load i16, ptr %0, !tbaa !2
  %163 = icmp ult i16 %162, %161
  %164 = zext i1 %163 to i8
  br i1 %163, label %b31, label %b30

b30:
  br label %b27

b31:
  %165 = mul i16 %162, 6
  %166 = getelementptr inbounds i8, ptr %148, i16 %165
  %167 = load ptr, ptr %166
  call addrspace(1) void @N$BDRP(ptr %167)
  %168 = add i16 %162, 1
  store i16 %168, ptr %0, !tbaa !2
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
