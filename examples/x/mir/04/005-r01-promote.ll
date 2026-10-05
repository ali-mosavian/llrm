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
  %18 = phi i16 [ 0, %b1 ], [ %24, %b4 ]
  %19 = icmp ult i16 %18, %12
  %20 = zext i1 %19 to i8
  br i1 %19, label %b3, label %b5

b3:
  %21 = load i16, ptr %14
  %22 = icmp ult i16 %18, %21
  %23 = zext i1 %22 to i8
  br i1 %22, label %b6, label %b7

b4:
  %24 = add nuw i16 %18, 1
  store i16 %24, ptr %9, !tbaa !2
  br label %b2

b5:
  %25 = load ptr, ptr addrspace(1) %2
  %26 = getelementptr i8, ptr %25, i16 -4
  %27 = load i16, ptr %26
  store i16 0, ptr %6, !tbaa !2
  store i16 %27, ptr %5, !tbaa !2
  %28 = load ptr, ptr addrspace(1) %2
  %29 = getelementptr i8, ptr %28, i16 -4
  %30 = getelementptr inbounds i8, ptr %4, i16 2
  %31 = getelementptr inbounds i8, ptr %4, i16 4
  %32 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %33 = mul i16 %18, 6
  %34 = getelementptr inbounds i8, ptr %13, i16 %33
  %35 = load ptr, ptr %34
  %36 = getelementptr i8, ptr %35, i16 -4
  %37 = load i16, ptr %36
  %38 = addrspacecast ptr %35 to ptr addrspace(1)
  store i16 %37, ptr %7, !tbaa !2
  store i16 %37, ptr %15, !tbaa !2
  store ptr addrspace(1) %38, ptr %16, !tbaa !2
  %39 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %17, ptr addrspace(1) %3)
  %40 = icmp eq i8 %39, 0
  %41 = zext i1 %40 to i8
  br i1 %40, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %42 = load ptr, ptr addrspace(1) %1
  %43 = getelementptr i8, ptr %42, i16 -4
  %44 = load i16, ptr %43
  %45 = icmp ult i16 %18, %44
  %46 = zext i1 %45 to i8
  br i1 %45, label %b11, label %b12

b11:
  %47 = mul i16 %18, 6
  %48 = getelementptr inbounds i8, ptr %42, i16 %47
  %49 = addrspacecast ptr %48 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %50 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %49, ptr addrspace(1) %50
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %51 = phi i16 [ 0, %b5 ], [ %57, %b15 ]
  %52 = icmp ult i16 %51, %27
  %53 = zext i1 %52 to i8
  br i1 %52, label %b14, label %b16

b14:
  %54 = load i16, ptr %29
  %55 = icmp ult i16 %51, %54
  %56 = zext i1 %55 to i8
  br i1 %55, label %b17, label %b18

b15:
  %57 = add nuw i16 %51, 1
  store i16 %57, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %58 = mul i16 %51, 6
  %59 = getelementptr inbounds i8, ptr %28, i16 %58
  %60 = load ptr, ptr %59
  %61 = getelementptr i8, ptr %60, i16 -4
  %62 = load i16, ptr %61
  %63 = addrspacecast ptr %60 to ptr addrspace(1)
  store i16 %62, ptr %4, !tbaa !2
  store i16 %62, ptr %30, !tbaa !2
  store ptr addrspace(1) %63, ptr %31, !tbaa !2
  %64 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %32, ptr addrspace(1) %3)
  %65 = icmp eq i8 %64, 0
  %66 = zext i1 %65 to i8
  br i1 %65, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %67 = load ptr, ptr addrspace(1) %2
  %68 = getelementptr i8, ptr %67, i16 -4
  %69 = load i16, ptr %68
  %70 = icmp ult i16 %51, %69
  %71 = zext i1 %70 to i8
  br i1 %70, label %b22, label %b23

b22:
  %72 = mul i16 %51, 6
  %73 = getelementptr inbounds i8, ptr %67, i16 %72
  %74 = addrspacecast ptr %73 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %75 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %74, ptr addrspace(1) %75
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
