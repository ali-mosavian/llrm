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
  %3 = alloca i8
  %4 = alloca i16
  %5 = load ptr, ptr addrspace(1) %1
  %6 = getelementptr i8, ptr %5, i16 -4
  %7 = load i16, ptr %6
  br label %b2

b2:
  %8 = phi i16 [ 0, %b1 ], [ %11, %b3 ]
  %9 = icmp ult i16 %8, %7
  %10 = zext i1 %9 to i8
  br i1 %9, label %b5, label %25

b3:
  %11 = add i16 %8, 1
  br label %b2

b4:
  %12 = phi i16 [ %8, %b6 ]
  %13 = addrspacecast ptr %5 to ptr addrspace(1)
  %14 = icmp ule i16 %12, %7
  br i1 %14, label %b9, label %b10

b5:
  br i1 %9, label %b7, label %b8

b6:
  %15 = phi i8 [ %10, %25 ], [ %22, %b7 ]
  %16 = icmp ne i8 %15, 0
  br i1 %16, label %b3, label %b4

b7:
  %17 = mul i16 %8, 6
  %18 = getelementptr inbounds i8, ptr %5, i16 %17
  %19 = getelementptr i8, ptr %18, i16 2
  %20 = load i16, ptr %19
  %21 = icmp ule i16 %20, %2
  %22 = zext i1 %21 to i8
  br label %b6

b8:
  call addrspace(1) void @N$EBND()
  unreachable

b9:
  store i16 %12, ptr addrspace(1) %0
  %23 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store i16 %12, ptr addrspace(1) %23
  %24 = getelementptr i8, ptr addrspace(1) %0, i16 4
  store ptr addrspace(1) %13, ptr addrspace(1) %24
  ret void

b10:
  call addrspace(1) void @N$EBND()
  unreachable

25:
  br label %b6
}

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i16 0, i16 10923}
!7 = !{i8 0, i8 2}
