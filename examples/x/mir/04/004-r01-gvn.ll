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
  %13 = load i16, ptr %8, !tbaa !2
  %14 = load ptr, ptr addrspace(1) %1
  %15 = getelementptr i8, ptr %14, i16 -4
  %16 = getelementptr inbounds i8, ptr %7, i16 2
  %17 = getelementptr inbounds i8, ptr %7, i16 4
  %18 = addrspacecast ptr %7 to ptr addrspace(1)
  br label %b2

b2:
  %19 = load i16, ptr %9, !tbaa !2
  %20 = icmp ult i16 %19, %13
  %21 = zext i1 %20 to i8
  br i1 %20, label %b3, label %b5

b3:
  %22 = load i16, ptr %15
  %23 = icmp ult i16 %19, %22
  %24 = zext i1 %23 to i8
  br i1 %23, label %b6, label %b7

b4:
  %25 = load i16, ptr %9, !tbaa !2
  %26 = add nuw i16 %25, 1
  store i16 %26, ptr %9, !tbaa !2
  br label %b2

b5:
  %27 = load ptr, ptr addrspace(1) %2
  %28 = getelementptr i8, ptr %27, i16 -4
  %29 = load i16, ptr %28
  store i16 0, ptr %6, !tbaa !2
  store i16 %29, ptr %5, !tbaa !2
  %30 = load i16, ptr %5, !tbaa !2
  %31 = load ptr, ptr addrspace(1) %2
  %32 = getelementptr i8, ptr %31, i16 -4
  %33 = getelementptr inbounds i8, ptr %4, i16 2
  %34 = getelementptr inbounds i8, ptr %4, i16 4
  %35 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %36 = mul i16 %19, 6
  %37 = getelementptr inbounds i8, ptr %14, i16 %36
  %38 = load ptr, ptr %37
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %7, !tbaa !2
  store i16 %40, ptr %16, !tbaa !2
  store ptr addrspace(1) %41, ptr %17, !tbaa !2
  %42 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %18, ptr addrspace(1) %3)
  %43 = icmp eq i8 %42, 0
  %44 = zext i1 %43 to i8
  br i1 %43, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %45 = load ptr, ptr addrspace(1) %1
  %46 = load i16, ptr %9, !tbaa !2
  %47 = getelementptr i8, ptr %45, i16 -4
  %48 = load i16, ptr %47
  %49 = icmp ult i16 %46, %48
  %50 = zext i1 %49 to i8
  br i1 %49, label %b11, label %b12

b11:
  %51 = mul i16 %46, 6
  %52 = getelementptr inbounds i8, ptr %45, i16 %51
  %53 = addrspacecast ptr %52 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %54 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %53, ptr addrspace(1) %54
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %55 = load i16, ptr %6, !tbaa !2
  %56 = icmp ult i16 %55, %30
  %57 = zext i1 %56 to i8
  br i1 %56, label %b14, label %b16

b14:
  %58 = load i16, ptr %32
  %59 = icmp ult i16 %55, %58
  %60 = zext i1 %59 to i8
  br i1 %59, label %b17, label %b18

b15:
  %61 = load i16, ptr %6, !tbaa !2
  %62 = add nuw i16 %61, 1
  store i16 %62, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %63 = mul i16 %55, 6
  %64 = getelementptr inbounds i8, ptr %31, i16 %63
  %65 = load ptr, ptr %64
  %66 = getelementptr i8, ptr %65, i16 -4
  %67 = load i16, ptr %66
  %68 = addrspacecast ptr %65 to ptr addrspace(1)
  store i16 %67, ptr %4, !tbaa !2
  store i16 %67, ptr %33, !tbaa !2
  store ptr addrspace(1) %68, ptr %34, !tbaa !2
  %69 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %35, ptr addrspace(1) %3)
  %70 = icmp eq i8 %69, 0
  %71 = zext i1 %70 to i8
  br i1 %70, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %72 = load ptr, ptr addrspace(1) %2
  %73 = load i16, ptr %6, !tbaa !2
  %74 = getelementptr i8, ptr %72, i16 -4
  %75 = load i16, ptr %74
  %76 = icmp ult i16 %73, %75
  %77 = zext i1 %76 to i8
  br i1 %76, label %b22, label %b23

b22:
  %78 = mul i16 %73, 6
  %79 = getelementptr inbounds i8, ptr %72, i16 %78
  %80 = addrspacecast ptr %79 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %81 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %80, ptr addrspace(1) %81
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
