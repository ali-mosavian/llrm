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
  store i16 %28, ptr %13, !tbaa !2
  %30 = getelementptr inbounds i8, ptr %13, i16 2
  store i16 %28, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i8, ptr %13, i16 4
  store ptr addrspace(1) %29, ptr %31, !tbaa !2
  %32 = addrspacecast ptr %13 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %23, ptr addrspace(1) %24, ptr addrspace(1) %25, ptr addrspace(1) %32)
  %33 = load i8, ptr %14, !tbaa !2, !range !7
  %34 = icmp eq i8 %33, 0
  %35 = zext i1 %34 to i8
  %36 = icmp ne i8 %35, 0
  br i1 %36, label %b4, label %b3

b2:
  %37 = addrspacecast ptr %11 to ptr addrspace(1)
  %38 = addrspacecast ptr %18 to ptr addrspace(1)
  %39 = addrspacecast ptr %16 to ptr addrspace(1)
  %40 = getelementptr i8, ptr @$str3, i16 6
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 %42, ptr %10, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %10, i16 2
  store i16 %42, ptr %44, !tbaa !2
  %45 = getelementptr inbounds i8, ptr %10, i16 4
  store ptr addrspace(1) %43, ptr %45, !tbaa !2
  %46 = addrspacecast ptr %10 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %37, ptr addrspace(1) %38, ptr addrspace(1) %39, ptr addrspace(1) %46)
  %47 = addrspacecast ptr %9 to ptr addrspace(1)
  %48 = addrspacecast ptr %16 to ptr addrspace(1)
  %49 = addrspacecast ptr %18 to ptr addrspace(1)
  %50 = getelementptr i8, ptr @$str3, i16 6
  %51 = getelementptr i8, ptr %50, i16 -4
  %52 = load i16, ptr %51
  %53 = addrspacecast ptr %50 to ptr addrspace(1)
  store i16 %52, ptr %8, !tbaa !2
  %54 = getelementptr inbounds i8, ptr %8, i16 2
  store i16 %52, ptr %54, !tbaa !2
  %55 = getelementptr inbounds i8, ptr %8, i16 4
  store ptr addrspace(1) %53, ptr %55, !tbaa !2
  %56 = addrspacecast ptr %8 to ptr addrspace(1)
  call addrspace(1) void @find(ptr addrspace(1) %47, ptr addrspace(1) %48, ptr addrspace(1) %49, ptr addrspace(1) %56)
  %57 = load i8, ptr %11
  store i8 %57, ptr %12
  %58 = getelementptr i8, ptr %11, i16 2
  %59 = load ptr addrspace(1), ptr %58
  %60 = getelementptr i8, ptr %12, i16 2
  store ptr addrspace(1) %59, ptr %60
  %61 = getelementptr inbounds i8, ptr %12, i16 6
  %62 = load i8, ptr %9
  store i8 %62, ptr %61
  %63 = getelementptr i8, ptr %9, i16 2
  %64 = load ptr addrspace(1), ptr %63
  %65 = getelementptr i8, ptr %61, i16 2
  store ptr addrspace(1) %64, ptr %65
  %66 = icmp eq i8 %57, 0
  %67 = zext i1 %66 to i8
  %68 = icmp ne i8 %67, 0
  br i1 %68, label %b8, label %b7

b3:
  %69 = getelementptr i8, ptr @$str8, i16 6
  call addrspace(1) void @N$PS(ptr %69)
  call addrspace(1) void @N$PN()
  br label %b2

b4:
  %70 = getelementptr inbounds i8, ptr %14, i16 2
  %71 = load ptr addrspace(1), ptr %70, !tbaa !2
  %72 = getelementptr inbounds i8, ptr %14, i16 2
  %73 = load ptr addrspace(1), ptr %72, !tbaa !2
  %74 = load ptr, ptr addrspace(1) %73
  call addrspace(1) void @N$PS(ptr %74)
  %75 = getelementptr i8, ptr @$str6, i16 6
  call addrspace(1) void @N$PS(ptr %75)
  %76 = getelementptr i8, ptr addrspace(1) %73, i16 4
  %77 = load i16, ptr addrspace(1) %76
  call addrspace(1) void @N$PU2(i16 %77)
  %78 = getelementptr i8, ptr @$str7, i16 6
  call addrspace(1) void @N$PS(ptr %78)
  %79 = getelementptr i8, ptr addrspace(1) %73, i16 2
  %80 = load i16, ptr addrspace(1) %79
  call addrspace(1) void @N$PU2(i16 %80)
  call addrspace(1) void @N$PN()
  br label %b2

b6:
  %81 = addrspacecast ptr %7 to ptr addrspace(1)
  %82 = addrspacecast ptr %18 to ptr addrspace(1)
  call addrspace(1) void @affordable(ptr addrspace(1) %81, ptr addrspace(1) %82, i16 20)
  %83 = load i16, ptr addrspace(1) %81
  store i16 %83, ptr %6, !tbaa !2
  %84 = addrspacecast ptr %5 to ptr addrspace(1)
  %85 = load i16, ptr addrspace(1) %81, !tbaa !2, !range !6
  %86 = icmp ult i16 0, %85
  %87 = zext i1 %86 to i8
  %88 = icmp ne i8 %87, 0
  br i1 %88, label %b11, label %b12

b7:
  %89 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %89)
  call addrspace(1) void @N$PN()
  br label %b6

b8:
  %90 = getelementptr inbounds i8, ptr %12, i16 2
  %91 = getelementptr inbounds i8, ptr %12, i16 6
  %92 = icmp eq i8 %62, 0
  %93 = zext i1 %92 to i8
  %94 = icmp ne i8 %93, 0
  br i1 %94, label %b9, label %b7

b9:
  %95 = getelementptr inbounds i8, ptr %12, i16 8
  %96 = getelementptr inbounds i8, ptr %12, i16 2
  %97 = getelementptr inbounds i8, ptr %12, i16 8
  %98 = call addrspace(1) ptr addrspace(1) @cheaper(ptr addrspace(1) %59, ptr addrspace(1) %64)
  %99 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %99)
  %100 = getelementptr i8, ptr addrspace(1) %98, i16 2
  %101 = load i16, ptr addrspace(1) %100
  call addrspace(1) void @N$PU2(i16 %101)
  call addrspace(1) void @N$PN()
  br label %b6

b11:
  %102 = getelementptr i8, ptr addrspace(1) %81, i16 4
  %103 = load ptr addrspace(1), ptr addrspace(1) %102, !tbaa !2
  %104 = getelementptr inbounds i8, ptr addrspace(1) %103, i16 0
  call addrspace(1) void @initial(ptr addrspace(1) %84, ptr addrspace(1) %104)
  %105 = load i16, ptr %6, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %105)
  %106 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %106)
  call addrspace(1) void @N$PV(ptr addrspace(1) %84)
  call addrspace(1) void @N$PN()
  %107 = load ptr, ptr %18, !tbaa !2
  %108 = getelementptr i8, ptr %107, i16 -4
  %109 = load i16, ptr %108
  %110 = addrspacecast ptr %107 to ptr addrspace(1)
  store i16 %109, ptr %4, !tbaa !2
  %111 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %109, ptr %111, !tbaa !2
  %112 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %110, ptr %112, !tbaa !2
  %113 = addrspacecast ptr %4 to ptr addrspace(1)
  store i16 0, ptr %3, !tbaa !2
  br label %b14

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %114 = addrspacecast ptr %18 to ptr addrspace(1)
  %115 = getelementptr i8, ptr @$str15, i16 6
  %116 = getelementptr i8, ptr %115, i16 -4
  %117 = load i16, ptr %116
  %118 = addrspacecast ptr %115 to ptr addrspace(1)
  store i16 %117, ptr %2, !tbaa !2
  %119 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 %117, ptr %119, !tbaa !2
  %120 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %118, ptr %120, !tbaa !2
  %121 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %114, ptr addrspace(1) %121, i16 1, i16 500)
  %122 = load ptr, ptr %18, !tbaa !2
  %123 = getelementptr i8, ptr %122, i16 -4
  %124 = load i16, ptr %123
  call addrspace(1) void @N$PU2(i16 %124)
  %125 = getelementptr i8, ptr @$str16, i16 6
  call addrspace(1) void @N$PS(ptr %125)
  call addrspace(1) void @N$PN()
  %126 = load ptr, ptr %16, !tbaa !2
  %127 = icmp ne ptr %126, null
  %128 = zext i1 %127 to i8
  %129 = icmp ne i8 %128, 0
  br i1 %129, label %b23, label %b22

b14:
  %130 = load i16, ptr %3, !tbaa !2
  %131 = icmp ult i16 %130, %109
  %132 = zext i1 %131 to i8
  %133 = icmp ne i8 %132, 0
  br i1 %133, label %b15, label %b17

b15:
  %134 = getelementptr i8, ptr addrspace(1) %113, i16 4
  %135 = mul i16 %130, 6
  %136 = getelementptr inbounds i8, ptr addrspace(1) %110, i16 %135
  %137 = getelementptr i8, ptr addrspace(1) %136, i16 4
  %138 = load i16, ptr addrspace(1) %137
  %139 = icmp ult i16 %138, 5
  %140 = zext i1 %139 to i8
  %141 = icmp ne i8 %140, 0
  br i1 %141, label %b18, label %b19

b16:
  %142 = load i16, ptr %3, !tbaa !2
  %143 = add i16 %142, 1
  store i16 %143, ptr %3, !tbaa !2
  br label %b14

b17:
  br label %b13

b18:
  %144 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %144)
  %145 = load ptr, ptr addrspace(1) %136
  call addrspace(1) void @N$PS(ptr %145)
  %146 = getelementptr i8, ptr @$str13, i16 6
  call addrspace(1) void @N$PS(ptr %146)
  %147 = getelementptr i8, ptr addrspace(1) %136, i16 4
  %148 = load i16, ptr addrspace(1) %147
  call addrspace(1) void @N$PU2(i16 %148)
  %149 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %149)
  call addrspace(1) void @N$PN()
  br label %b21

b19:
  br label %b20

b20:
  br label %b16

b21:
  br label %b20

b22:
  call addrspace(1) void @N$BDRP(ptr %126)
  %150 = load ptr, ptr %18, !tbaa !2
  %151 = icmp ne ptr %150, null
  %152 = zext i1 %151 to i8
  %153 = icmp ne i8 %152, 0
  br i1 %153, label %b28, label %b27

b23:
  %154 = getelementptr i8, ptr %126, i16 -4
  %155 = load i16, ptr %154
  store i16 0, ptr %1, !tbaa !2
  br label %b24

b24:
  %156 = load i16, ptr %1, !tbaa !2
  %157 = icmp ult i16 %156, %155
  %158 = zext i1 %157 to i8
  %159 = icmp ne i8 %158, 0
  br i1 %159, label %b26, label %b25

b25:
  br label %b22

b26:
  %160 = mul i16 %156, 6
  %161 = getelementptr inbounds i8, ptr %126, i16 %160
  %162 = load ptr, ptr %161
  call addrspace(1) void @N$BDRP(ptr %162)
  %163 = add i16 %156, 1
  store i16 %163, ptr %1, !tbaa !2
  br label %b24

b27:
  call addrspace(1) void @N$BDRP(ptr %150)
  ret i16 0

b28:
  %164 = getelementptr i8, ptr %150, i16 -4
  %165 = load i16, ptr %164
  store i16 0, ptr %0, !tbaa !2
  br label %b29

b29:
  %166 = load i16, ptr %0, !tbaa !2
  %167 = icmp ult i16 %166, %165
  %168 = zext i1 %167 to i8
  %169 = icmp ne i8 %168, 0
  br i1 %169, label %b31, label %b30

b30:
  br label %b27

b31:
  %170 = mul i16 %166, 6
  %171 = getelementptr inbounds i8, ptr %150, i16 %170
  %172 = load ptr, ptr %171
  call addrspace(1) void @N$BDRP(ptr %172)
  %173 = add i16 %166, 1
  store i16 %173, ptr %0, !tbaa !2
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
