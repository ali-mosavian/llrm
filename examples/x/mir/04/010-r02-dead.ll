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

define internal void @pipeline.body(ptr addrspace(1) nocapture %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %1, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %2, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %3) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = load ptr, ptr addrspace(1) %1
  %7 = getelementptr i8, ptr %6, i16 -4
  %8 = load i16, ptr %7
  %9 = getelementptr inbounds i8, ptr %5, i16 2
  %10 = getelementptr inbounds i8, ptr %5, i16 4
  %11 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b2

b2:
  %12 = phi i16 [ 0, %b1 ], [ %16, %b4 ]
  %13 = icmp ult i16 %12, %8
  br i1 %13, label %b3, label %b5

b3:
  %14 = load i16, ptr %7
  %15 = icmp ult i16 %12, %14
  br i1 %15, label %b6, label %b7

b4:
  %16 = add nuw i16 %12, 1
  br label %b2

b5:
  %17 = load ptr, ptr addrspace(1) %2
  %18 = getelementptr i8, ptr %17, i16 -4
  %19 = load i16, ptr %18
  %20 = getelementptr inbounds i8, ptr %4, i16 2
  %21 = getelementptr inbounds i8, ptr %4, i16 4
  %22 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %23 = mul i16 %12, 6
  %24 = getelementptr inbounds i8, ptr %6, i16 %23
  %25 = load ptr, ptr %24
  %26 = getelementptr i8, ptr %25, i16 -4
  %27 = load i16, ptr %26
  %28 = addrspacecast ptr %25 to ptr addrspace(1)
  store i16 %27, ptr %5, !tbaa !2
  store i16 %27, ptr %9, !tbaa !2
  store ptr addrspace(1) %28, ptr %10, !tbaa !2
  %29 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %11, ptr addrspace(1) %3)
  %30 = icmp eq i8 %29, 0
  br i1 %30, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %31 = phi i16 [ %12, %b6 ]
  %32 = load i16, ptr %7
  %33 = icmp ult i16 %31, %32
  br i1 %33, label %b11, label %b12

b11:
  %34 = mul i16 %31, 6
  %35 = getelementptr inbounds i8, ptr %6, i16 %34
  %36 = addrspacecast ptr %35 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %37 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %36, ptr addrspace(1) %37
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %38 = phi i16 [ 0, %b5 ], [ %42, %b15 ]
  %39 = icmp ult i16 %38, %19
  br i1 %39, label %b14, label %b16

b14:
  %40 = load i16, ptr %18
  %41 = icmp ult i16 %38, %40
  br i1 %41, label %b17, label %b18

b15:
  %42 = add nuw i16 %38, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %43 = mul i16 %38, 6
  %44 = getelementptr inbounds i8, ptr %17, i16 %43
  %45 = load ptr, ptr %44
  %46 = getelementptr i8, ptr %45, i16 -4
  %47 = load i16, ptr %46
  %48 = addrspacecast ptr %45 to ptr addrspace(1)
  store i16 %47, ptr %4, !tbaa !2
  store i16 %47, ptr %20, !tbaa !2
  store ptr addrspace(1) %48, ptr %21, !tbaa !2
  %49 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %22, ptr addrspace(1) %3)
  %50 = icmp eq i8 %49, 0
  br i1 %50, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %51 = phi i16 [ %38, %b17 ]
  %52 = load i16, ptr %18
  %53 = icmp ult i16 %51, %52
  br i1 %53, label %b22, label %b23

b22:
  %54 = mul i16 %51, 6
  %55 = getelementptr inbounds i8, ptr %17, i16 %54
  %56 = addrspacecast ptr %55 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %57 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %56, ptr addrspace(1) %57
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
