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

define internal void @pipeline.body(ptr addrspace(1) nocapture %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %1, i16 %2) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %3 = alloca [8 x i8]
  %4 = alloca i8
  %5 = alloca i16
  store i16 0, ptr %5, !tbaa !2
  %6 = load ptr, ptr addrspace(1) %1
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = load ptr, ptr addrspace(1) %1
  %10 = getelementptr i8, ptr %9, i16 -4
  br label %b2

b2:
  %11 = load i16, ptr %5, !tbaa !2
  %12 = icmp ult i16 %11, %8
  %13 = zext i1 %12 to i8
  store i8 %13, ptr %4, !tbaa !2
  br i1 %12, label %b5, label %b6

b3:
  %14 = load i16, ptr %5, !tbaa !2
  %15 = add i16 %14, 1
  store i16 %15, ptr %5, !tbaa !2
  br label %b2

b4:
  %16 = load ptr, ptr addrspace(1) %1
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = addrspacecast ptr %16 to ptr addrspace(1)
  %20 = load i16, ptr %5, !tbaa !2
  %21 = icmp ule i16 %20, %18
  %22 = zext i1 %21 to i8
  br i1 %21, label %b9, label %b10

b5:
  %23 = load i16, ptr %5, !tbaa !2
  %24 = load i16, ptr %10
  %25 = icmp ult i16 %23, %24
  %26 = zext i1 %25 to i8
  br i1 %25, label %b7, label %b8

b6:
  %27 = load i8, ptr %4, !tbaa !2, !range !5
  %28 = icmp ne i8 %27, 0
  br i1 %28, label %b3, label %b4

b7:
  %29 = mul i16 %23, 6
  %30 = getelementptr inbounds i8, ptr %9, i16 %29
  %31 = getelementptr i8, ptr %30, i16 2
  %32 = load i16, ptr %31
  %33 = icmp ule i16 %32, %2
  %34 = zext i1 %33 to i8
  store i8 %34, ptr %4, !tbaa !2
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  %35 = icmp uge i16 %20, 0
  %36 = zext i1 %35 to i8
  store i16 %20, ptr %3, !tbaa !2
  %37 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %20, ptr %37, !tbaa !2
  %38 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %19, ptr %38, !tbaa !2
  %39 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 %20, ptr addrspace(1) %0
  %40 = getelementptr i8, ptr addrspace(1) %39, i16 2
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %20, ptr addrspace(1) %41
  %42 = getelementptr i8, ptr addrspace(1) %39, i16 4
  %43 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %19, ptr addrspace(1) %43
  ret void

b10:
  call addrspace(1) void @N$EBND()
  unreachable
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
