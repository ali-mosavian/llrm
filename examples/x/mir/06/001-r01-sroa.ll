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
  br label %b2

b2:
  %6 = load i16, ptr %5, !tbaa !2
  %7 = load ptr, ptr addrspace(1) %1
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = icmp ult i16 %6, %9
  %11 = zext i1 %10 to i8
  store i8 %11, ptr %4, !tbaa !2
  %12 = icmp ne i8 %11, 0
  br i1 %12, label %b5, label %b6

b3:
  %13 = load i16, ptr %5, !tbaa !2
  %14 = add i16 %13, 1
  store i16 %14, ptr %5, !tbaa !2
  br label %b2

b4:
  %15 = load ptr, ptr addrspace(1) %1
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %15 to ptr addrspace(1)
  %19 = load i16, ptr %5, !tbaa !2
  %20 = icmp ule i16 %19, %17
  %21 = zext i1 %20 to i8
  %22 = icmp ne i8 %21, 0
  br i1 %22, label %b9, label %b10

b5:
  %23 = load ptr, ptr addrspace(1) %1
  %24 = load i16, ptr %5, !tbaa !2
  %25 = getelementptr i8, ptr %23, i16 -4
  %26 = load i16, ptr %25
  %27 = icmp ult i16 %24, %26
  %28 = zext i1 %27 to i8
  %29 = icmp ne i8 %28, 0
  br i1 %29, label %b7, label %b8

b6:
  %30 = load i8, ptr %4, !tbaa !2, !range !5
  %31 = icmp ne i8 %30, 0
  br i1 %31, label %b3, label %b4

b7:
  %32 = mul i16 %24, 6
  %33 = getelementptr inbounds i8, ptr %23, i16 %32
  %34 = getelementptr i8, ptr %33, i16 2
  %35 = load i16, ptr %34
  %36 = icmp ule i16 %35, %2
  %37 = zext i1 %36 to i8
  store i8 %37, ptr %4, !tbaa !2
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  %38 = icmp ule i16 0, %19
  %39 = zext i1 %38 to i8
  %40 = icmp ne i8 %39, 0
  br i1 %40, label %b11, label %b12

b10:
  call addrspace(1) void @N$EBND()
  unreachable

b11:
  store i16 %19, ptr %3, !tbaa !2
  %41 = getelementptr inbounds i8, ptr %3, i16 2
  store i16 %19, ptr %41, !tbaa !2
  %42 = getelementptr inbounds i8, ptr %3, i16 4
  store ptr addrspace(1) %18, ptr %42, !tbaa !2
  %43 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 %19, ptr addrspace(1) %0
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 2
  %45 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %19, ptr addrspace(1) %45
  %46 = getelementptr i8, ptr addrspace(1) %43, i16 4
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %18, ptr addrspace(1) %47
  ret void

b12:
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
