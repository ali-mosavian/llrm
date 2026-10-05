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

define internal void @pipeline.body(ptr addrspace(1) nocapture %0) nearcode memory(readwrite, argmem: write) {
b1:
  %1 = alloca i16
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [2 x i8]
  %5 = getelementptr i8, ptr @$str1, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = addrspacecast ptr %4 to ptr addrspace(1)
  %7 = getelementptr i8, ptr @$str3, i16 6
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = addrspacecast ptr %7 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %10, ptr %12, !tbaa !2
  %13 = addrspacecast ptr %3 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %6, ptr addrspace(1) %13, i16 28, i16 9)
  %14 = addrspacecast ptr %4 to ptr addrspace(1)
  %15 = getelementptr i8, ptr @$str5, i16 6
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %15 to ptr addrspace(1)
  store i16 3, ptr %2, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 3, ptr %19, !tbaa !2
  %20 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %18, ptr %20, !tbaa !2
  %21 = addrspacecast ptr %2 to ptr addrspace(1)
  call addrspace(1) void @Catalog.add(ptr addrspace(1) %14, ptr addrspace(1) %21, i16 2, i16 100)
  %22 = load ptr, ptr %4, !tbaa !2
  store ptr %22, ptr addrspace(1) %0
  store ptr null, ptr %4, !tbaa !2
  %23 = load ptr, ptr %4, !tbaa !2
  %24 = icmp ne ptr %23, null
  %25 = zext i1 %24 to i8
  br i1 %24, label %b3, label %b2

b2:
  call addrspace(1) void @N$BDRP(ptr %23)
  ret void

b3:
  %26 = getelementptr i8, ptr %23, i16 -4
  %27 = load i16, ptr %26
  store i16 0, ptr %1, !tbaa !2
  br label %b4

b4:
  %28 = load i16, ptr %1, !tbaa !2
  %29 = icmp ult i16 %28, %27
  %30 = zext i1 %29 to i8
  br i1 %29, label %b6, label %b5

b5:
  br label %b2

b6:
  %31 = mul i16 %28, 6
  %32 = getelementptr inbounds i8, ptr %23, i16 %31
  %33 = load ptr, ptr %32
  call addrspace(1) void @N$BDRP(ptr %33)
  %34 = add i16 %28, 1
  store i16 %34, ptr %1, !tbaa !2
  br label %b4
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
