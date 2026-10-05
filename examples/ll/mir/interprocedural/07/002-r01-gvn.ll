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

declare internal void @Catalog.add(ptr addrspace(5), ptr addrspace(5), i16, i16) addrspace(1) nearcode

declare internal void @north(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @south(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @find(ptr addrspace(5), ptr addrspace(5), ptr addrspace(5), ptr addrspace(5)) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [2 x i8]
  %5 = getelementptr i8, ptr @$str1, i16 6
  store ptr %5, ptr %4, !tbaa !2
  %6 = addrspacecast ptr %4 to ptr addrspace(5)
  %7 = addrspacecast ptr %4 to ptr addrspace(1)
  %8 = getelementptr i8, ptr @$str2, i16 6
  %9 = addrspacecast ptr %8 to ptr addrspace(1)
  store i16 4, ptr %3, !tbaa !2
  %10 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 4, ptr %10, !tbaa !2
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %9, ptr %11, !tbaa !2
  %12 = addrspacecast ptr %3 to ptr addrspace(5)
  %13 = addrspacecast ptr %3 to ptr addrspace(1)
  %14 = addrspacecast ptr addrspace(1) %13 to ptr addrspace(5)
  %15 = addrspacecast ptr addrspace(1) %7 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %12, i16 5, i16 40)
  %16 = getelementptr i8, ptr @$str3, i16 6
  %17 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 4, ptr %2, !tbaa !2
  %18 = getelementptr inbounds i8, ptr %2, i16 2
  store i16 4, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i8, ptr %2, i16 4
  store ptr addrspace(1) %17, ptr %19, !tbaa !2
  %20 = addrspacecast ptr %2 to ptr addrspace(5)
  %21 = addrspacecast ptr %2 to ptr addrspace(1)
  %22 = addrspacecast ptr addrspace(1) %21 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %20, i16 30, i16 3)
  %23 = getelementptr i8, ptr @$str4, i16 6
  %24 = addrspacecast ptr %23 to ptr addrspace(1)
  store i16 3, ptr %1, !tbaa !2
  %25 = getelementptr inbounds i8, ptr %1, i16 2
  store i16 3, ptr %25, !tbaa !2
  %26 = getelementptr inbounds i8, ptr %1, i16 4
  store ptr addrspace(1) %24, ptr %26, !tbaa !2
  %27 = addrspacecast ptr %1 to ptr addrspace(5)
  %28 = addrspacecast ptr %1 to ptr addrspace(1)
  %29 = addrspacecast ptr addrspace(1) %28 to ptr addrspace(5)
  call addrspace(1) void @Catalog.add(ptr addrspace(5) %6, ptr addrspace(5) %27, i16 12, i16 0)
  %30 = load ptr, ptr %4, !tbaa !2
  store ptr %30, ptr addrspace(1) %0
  store ptr null, ptr %4, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr null)
  ret void
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
