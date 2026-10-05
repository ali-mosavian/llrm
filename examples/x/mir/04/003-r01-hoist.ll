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
  %22 = load i16, ptr %9, !tbaa !2
  %23 = load i16, ptr %15
  %24 = icmp ult i16 %22, %23
  %25 = zext i1 %24 to i8
  br i1 %24, label %b6, label %b7

b4:
  %26 = load i16, ptr %9, !tbaa !2
  %27 = add nuw i16 %26, 1
  store i16 %27, ptr %9, !tbaa !2
  br label %b2

b5:
  %28 = load ptr, ptr addrspace(1) %2
  %29 = getelementptr i8, ptr %28, i16 -4
  %30 = load i16, ptr %29
  store i16 0, ptr %6, !tbaa !2
  store i16 %30, ptr %5, !tbaa !2
  %31 = load i16, ptr %5, !tbaa !2
  %32 = load ptr, ptr addrspace(1) %2
  %33 = getelementptr i8, ptr %32, i16 -4
  %34 = getelementptr inbounds i8, ptr %4, i16 2
  %35 = getelementptr inbounds i8, ptr %4, i16 4
  %36 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %b13

b6:
  %37 = mul i16 %22, 6
  %38 = getelementptr inbounds i8, ptr %14, i16 %37
  %39 = load ptr, ptr %38
  %40 = getelementptr i8, ptr %39, i16 -4
  %41 = load i16, ptr %40
  %42 = addrspacecast ptr %39 to ptr addrspace(1)
  store i16 %41, ptr %7, !tbaa !2
  store i16 %41, ptr %16, !tbaa !2
  store ptr addrspace(1) %42, ptr %17, !tbaa !2
  %43 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %18, ptr addrspace(1) %3)
  %44 = icmp eq i8 %43, 0
  %45 = zext i1 %44 to i8
  br i1 %44, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %46 = load ptr, ptr addrspace(1) %1
  %47 = load i16, ptr %9, !tbaa !2
  %48 = getelementptr i8, ptr %46, i16 -4
  %49 = load i16, ptr %48
  %50 = icmp ult i16 %47, %49
  %51 = zext i1 %50 to i8
  br i1 %50, label %b11, label %b12

b11:
  %52 = mul i16 %47, 6
  %53 = getelementptr inbounds i8, ptr %46, i16 %52
  %54 = addrspacecast ptr %53 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %55 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %54, ptr addrspace(1) %55
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %56 = load i16, ptr %6, !tbaa !2
  %57 = icmp ult i16 %56, %31
  %58 = zext i1 %57 to i8
  br i1 %57, label %b14, label %b16

b14:
  %59 = load i16, ptr %6, !tbaa !2
  %60 = load i16, ptr %33
  %61 = icmp ult i16 %59, %60
  %62 = zext i1 %61 to i8
  br i1 %61, label %b17, label %b18

b15:
  %63 = load i16, ptr %6, !tbaa !2
  %64 = add nuw i16 %63, 1
  store i16 %64, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %65 = mul i16 %59, 6
  %66 = getelementptr inbounds i8, ptr %32, i16 %65
  %67 = load ptr, ptr %66
  %68 = getelementptr i8, ptr %67, i16 -4
  %69 = load i16, ptr %68
  %70 = addrspacecast ptr %67 to ptr addrspace(1)
  store i16 %69, ptr %4, !tbaa !2
  store i16 %69, ptr %34, !tbaa !2
  store ptr addrspace(1) %70, ptr %35, !tbaa !2
  %71 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %36, ptr addrspace(1) %3)
  %72 = icmp eq i8 %71, 0
  %73 = zext i1 %72 to i8
  br i1 %72, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %74 = load ptr, ptr addrspace(1) %2
  %75 = load i16, ptr %6, !tbaa !2
  %76 = getelementptr i8, ptr %74, i16 -4
  %77 = load i16, ptr %76
  %78 = icmp ult i16 %75, %77
  %79 = zext i1 %78 to i8
  br i1 %78, label %b22, label %b23

b22:
  %80 = mul i16 %75, 6
  %81 = getelementptr inbounds i8, ptr %74, i16 %80
  %82 = addrspacecast ptr %81 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %83 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %82, ptr addrspace(1) %83
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
