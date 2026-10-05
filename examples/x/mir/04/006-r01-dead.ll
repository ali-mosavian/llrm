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
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca i16
  %10 = load ptr, ptr addrspace(1) %1
  %11 = getelementptr i8, ptr %10, i16 -4
  %12 = load i16, ptr %11
  store i16 0, ptr %9, !tbaa !2
  store i16 %12, ptr %8, !tbaa !2
  %13 = load ptr, ptr addrspace(1) %1
  %14 = getelementptr i8, ptr %13, i16 -4
  %15 = getelementptr inbounds i8, ptr %7, i16 2
  %16 = getelementptr inbounds i8, ptr %7, i16 4
  %17 = addrspacecast ptr %7 to ptr addrspace(1)
  br label %b2

b2:
  %18 = phi i16 [ 0, %b1 ], [ %22, %b4 ]
  %19 = icmp ult i16 %18, %12
  br i1 %19, label %b3, label %b5

b3:
  %20 = load i16, ptr %14
  %21 = icmp ult i16 %18, %20
  br i1 %21, label %b6, label %b7

b4:
  %22 = add nuw i16 %18, 1
  store i16 %22, ptr %9, !tbaa !2
  br label %b2

b5:
  %23 = load ptr, ptr addrspace(1) %2
  %24 = getelementptr i8, ptr %23, i16 -4
  %25 = load i16, ptr %24
  store i16 0, ptr %6, !tbaa !2
  store i16 %25, ptr %5, !tbaa !2
  %26 = load ptr, ptr addrspace(1) %2
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = getelementptr inbounds i8, ptr %4, i16 2
  %29 = getelementptr inbounds i8, ptr %4, i16 4
  %30 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %31 = mul i16 %18, 6
  %32 = getelementptr inbounds i8, ptr %13, i16 %31
  %33 = load ptr, ptr %32
  %34 = getelementptr i8, ptr %33, i16 -4
  %35 = load i16, ptr %34
  %36 = addrspacecast ptr %33 to ptr addrspace(1)
  store i16 %35, ptr %7, !tbaa !2
  store i16 %35, ptr %15, !tbaa !2
  store ptr addrspace(1) %36, ptr %16, !tbaa !2
  %37 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %17, ptr addrspace(1) %3)
  %38 = icmp eq i8 %37, 0
  br i1 %38, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %39 = load ptr, ptr addrspace(1) %1
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  %42 = icmp ult i16 %18, %41
  br i1 %42, label %b11, label %b12

b11:
  %43 = mul i16 %18, 6
  %44 = getelementptr inbounds i8, ptr %39, i16 %43
  %45 = addrspacecast ptr %44 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %46 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %45, ptr addrspace(1) %46
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %47 = phi i16 [ 0, %b5 ], [ %51, %b15 ]
  %48 = icmp ult i16 %47, %25
  br i1 %48, label %b14, label %b16

b14:
  %49 = load i16, ptr %27
  %50 = icmp ult i16 %47, %49
  br i1 %50, label %b17, label %b18

b15:
  %51 = add nuw i16 %47, 1
  store i16 %51, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %52 = mul i16 %47, 6
  %53 = getelementptr inbounds i8, ptr %26, i16 %52
  %54 = load ptr, ptr %53
  %55 = getelementptr i8, ptr %54, i16 -4
  %56 = load i16, ptr %55
  %57 = addrspacecast ptr %54 to ptr addrspace(1)
  store i16 %56, ptr %4, !tbaa !2
  store i16 %56, ptr %28, !tbaa !2
  store ptr addrspace(1) %57, ptr %29, !tbaa !2
  %58 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %30, ptr addrspace(1) %3)
  %59 = icmp eq i8 %58, 0
  br i1 %59, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %60 = load ptr, ptr addrspace(1) %2
  %61 = getelementptr i8, ptr %60, i16 -4
  %62 = load i16, ptr %61
  %63 = icmp ult i16 %47, %62
  br i1 %63, label %b22, label %b23

b22:
  %64 = mul i16 %47, 6
  %65 = getelementptr inbounds i8, ptr %60, i16 %64
  %66 = addrspacecast ptr %65 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %67 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %66, ptr addrspace(1) %67
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
