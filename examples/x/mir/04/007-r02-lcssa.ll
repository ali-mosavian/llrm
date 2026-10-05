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
  %39 = phi i16 [ %18, %b6 ]
  %40 = load ptr, ptr addrspace(1) %1
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = icmp ult i16 %39, %42
  br i1 %43, label %b11, label %b12

b11:
  %44 = mul i16 %39, 6
  %45 = getelementptr inbounds i8, ptr %40, i16 %44
  %46 = addrspacecast ptr %45 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %47 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %46, ptr addrspace(1) %47
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %48 = phi i16 [ 0, %b5 ], [ %52, %b15 ]
  %49 = icmp ult i16 %48, %25
  br i1 %49, label %b14, label %b16

b14:
  %50 = load i16, ptr %27
  %51 = icmp ult i16 %48, %50
  br i1 %51, label %b17, label %b18

b15:
  %52 = add nuw i16 %48, 1
  store i16 %52, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %53 = mul i16 %48, 6
  %54 = getelementptr inbounds i8, ptr %26, i16 %53
  %55 = load ptr, ptr %54
  %56 = getelementptr i8, ptr %55, i16 -4
  %57 = load i16, ptr %56
  %58 = addrspacecast ptr %55 to ptr addrspace(1)
  store i16 %57, ptr %4, !tbaa !2
  store i16 %57, ptr %28, !tbaa !2
  store ptr addrspace(1) %58, ptr %29, !tbaa !2
  %59 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %30, ptr addrspace(1) %3)
  %60 = icmp eq i8 %59, 0
  br i1 %60, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %61 = phi i16 [ %48, %b17 ]
  %62 = load ptr, ptr addrspace(1) %2
  %63 = getelementptr i8, ptr %62, i16 -4
  %64 = load i16, ptr %63
  %65 = icmp ult i16 %61, %64
  br i1 %65, label %b22, label %b23

b22:
  %66 = mul i16 %61, 6
  %67 = getelementptr inbounds i8, ptr %62, i16 %66
  %68 = addrspacecast ptr %67 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %69 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %68, ptr addrspace(1) %69
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
