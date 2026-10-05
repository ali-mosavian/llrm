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
  %13 = getelementptr inbounds i8, ptr %7, i16 2
  %14 = getelementptr inbounds i8, ptr %7, i16 4
  %15 = addrspacecast ptr %7 to ptr addrspace(1)
  br label %b2

b2:
  %16 = phi i16 [ 0, %b1 ], [ %20, %b4 ]
  %17 = icmp ult i16 %16, %12
  br i1 %17, label %b3, label %b5

b3:
  %18 = load i16, ptr %11
  %19 = icmp ult i16 %16, %18
  br i1 %19, label %b6, label %b7

b4:
  %20 = add nuw i16 %16, 1
  br label %b2

b5:
  %21 = load ptr, ptr addrspace(1) %2
  %22 = getelementptr i8, ptr %21, i16 -4
  %23 = load i16, ptr %22
  %24 = getelementptr inbounds i8, ptr %4, i16 2
  %25 = getelementptr inbounds i8, ptr %4, i16 4
  %26 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %27 = mul i16 %16, 6
  %28 = getelementptr inbounds i8, ptr %10, i16 %27
  %29 = load ptr, ptr %28
  %30 = getelementptr i8, ptr %29, i16 -4
  %31 = load i16, ptr %30
  %32 = addrspacecast ptr %29 to ptr addrspace(1)
  store i16 %31, ptr %7, !tbaa !2
  store i16 %31, ptr %13, !tbaa !2
  store ptr addrspace(1) %32, ptr %14, !tbaa !2
  %33 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %15, ptr addrspace(1) %3)
  %34 = icmp eq i8 %33, 0
  br i1 %34, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %35 = phi i16 [ %16, %b6 ]
  %36 = load i16, ptr %11
  %37 = icmp ult i16 %35, %36
  br i1 %37, label %b11, label %b12

b11:
  %38 = mul i16 %35, 6
  %39 = getelementptr inbounds i8, ptr %10, i16 %38
  %40 = addrspacecast ptr %39 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %41 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %40, ptr addrspace(1) %41
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %42 = phi i16 [ 0, %b5 ], [ %46, %b15 ]
  %43 = icmp ult i16 %42, %23
  br i1 %43, label %b14, label %b16

b14:
  %44 = load i16, ptr %22
  %45 = icmp ult i16 %42, %44
  br i1 %45, label %b17, label %b18

b15:
  %46 = add nuw i16 %42, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %47 = mul i16 %42, 6
  %48 = getelementptr inbounds i8, ptr %21, i16 %47
  %49 = load ptr, ptr %48
  %50 = getelementptr i8, ptr %49, i16 -4
  %51 = load i16, ptr %50
  %52 = addrspacecast ptr %49 to ptr addrspace(1)
  store i16 %51, ptr %4, !tbaa !2
  store i16 %51, ptr %24, !tbaa !2
  store ptr addrspace(1) %52, ptr %25, !tbaa !2
  %53 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %26, ptr addrspace(1) %3)
  %54 = icmp eq i8 %53, 0
  br i1 %54, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %55 = phi i16 [ %42, %b17 ]
  %56 = load i16, ptr %22
  %57 = icmp ult i16 %55, %56
  br i1 %57, label %b22, label %b23

b22:
  %58 = mul i16 %55, 6
  %59 = getelementptr inbounds i8, ptr %21, i16 %58
  %60 = addrspacecast ptr %59 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %61 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %60, ptr addrspace(1) %61
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
