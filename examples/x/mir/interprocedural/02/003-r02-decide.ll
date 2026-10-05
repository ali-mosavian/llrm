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

declare internal void @find(ptr addrspace(1) nocapture, ptr, ptr, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias) addrspace(1) nearcode memory(argmem: read) willreturn norecurse

declare internal void @affordable(ptr addrspace(1) nocapture, ptr, i16) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal void @initial(ptr addrspace(1) nocapture, ptr) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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

define internal void @pipeline.body(ptr addrspace(1) nocapture %0, ptr %1, ptr %2, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %3) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = getelementptr i8, ptr %1, i16 -4
  %7 = load i16, ptr %6
  %8 = getelementptr inbounds i8, ptr %5, i16 2
  %9 = getelementptr inbounds i8, ptr %5, i16 4
  %10 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b2

b2:
  %11 = phi i16 [ 0, %b1 ], [ %21, %b4 ]
  %12 = icmp ult i16 %11, %7
  br i1 %12, label %b3, label %b5

b3:
  %13 = mul i16 %11, 6
  %14 = getelementptr inbounds i8, ptr %1, i16 %13
  %15 = load ptr, ptr %14
  %16 = getelementptr i8, ptr %15, i16 -4
  %17 = load i16, ptr %16
  %18 = addrspacecast ptr %15 to ptr addrspace(1)
  store i16 %17, ptr %5, !tbaa !2
  store i16 %17, ptr %8, !tbaa !2
  store ptr addrspace(1) %18, ptr %9, !tbaa !2
  %19 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %10, ptr addrspace(1) %3)
  %20 = icmp eq i8 %19, 0
  br i1 %20, label %b8, label %b4

b4:
  %21 = add nuw i16 %11, 1
  br label %b2

b5:
  %22 = getelementptr i8, ptr %2, i16 -4
  %23 = load i16, ptr %22
  %24 = getelementptr inbounds i8, ptr %4, i16 2
  %25 = getelementptr inbounds i8, ptr %4, i16 4
  %26 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b8:
  %27 = icmp ult i16 %11, %7
  br i1 %27, label %b11, label %b12

b11:
  %28 = mul i16 %11, 6
  %29 = getelementptr inbounds i8, ptr %1, i16 %28
  %30 = addrspacecast ptr %29 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %31 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %30, ptr addrspace(1) %31
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %32 = phi i16 [ 0, %b5 ], [ %42, %b15 ]
  %33 = icmp ult i16 %32, %23
  br i1 %33, label %b14, label %b16

b14:
  %34 = mul i16 %32, 6
  %35 = getelementptr inbounds i8, ptr %2, i16 %34
  %36 = load ptr, ptr %35
  %37 = getelementptr i8, ptr %36, i16 -4
  %38 = load i16, ptr %37
  %39 = addrspacecast ptr %36 to ptr addrspace(1)
  store i16 %38, ptr %4, !tbaa !2
  store i16 %38, ptr %24, !tbaa !2
  store ptr addrspace(1) %39, ptr %25, !tbaa !2
  %40 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %26, ptr addrspace(1) %3)
  %41 = icmp eq i8 %40, 0
  br i1 %41, label %b19, label %b15

b15:
  %42 = add nuw i16 %32, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b19:
  %43 = icmp ult i16 %32, %23
  br i1 %43, label %b22, label %b23

b22:
  %44 = mul i16 %32, 6
  %45 = getelementptr inbounds i8, ptr %2, i16 %44
  %46 = addrspacecast ptr %45 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %46, ptr addrspace(1) %47
  ret void

b23:
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
