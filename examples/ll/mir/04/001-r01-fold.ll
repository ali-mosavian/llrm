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
  br label %b2

b2:
  %13 = load i16, ptr %9, !tbaa !2
  %14 = load i16, ptr %8, !tbaa !2
  %15 = icmp ult i16 %13, %14
  %16 = zext i1 %15 to i8
  br i1 %15, label %b3, label %b5

b3:
  %17 = load ptr, ptr addrspace(1) %1
  %18 = load i16, ptr %9, !tbaa !2
  %19 = getelementptr i8, ptr %17, i16 -4
  %20 = load i16, ptr %19
  %21 = icmp ult i16 %18, %20
  %22 = zext i1 %21 to i8
  br i1 %21, label %b6, label %b7

b4:
  %23 = load i16, ptr %9, !tbaa !2
  %24 = add nuw i16 %23, 1
  store i16 %24, ptr %9, !tbaa !2
  br label %b2

b5:
  %25 = load ptr, ptr addrspace(1) %2
  %26 = getelementptr i8, ptr %25, i16 -4
  %27 = load i16, ptr %26
  store i16 0, ptr %6, !tbaa !2
  store i16 %27, ptr %5, !tbaa !2
  br label %b13

b6:
  %28 = mul i16 %18, 6
  %29 = getelementptr inbounds i8, ptr %17, i16 %28
  %30 = load ptr, ptr %29
  %31 = getelementptr i8, ptr %30, i16 -4
  %32 = load i16, ptr %31
  %33 = addrspacecast ptr %30 to ptr addrspace(1)
  store i16 %32, ptr %7, !tbaa !2
  %34 = getelementptr inbounds i8, ptr %7, i16 2
  store i16 %32, ptr %34, !tbaa !2
  %35 = getelementptr inbounds i8, ptr %7, i16 4
  store ptr addrspace(1) %33, ptr %35, !tbaa !2
  %36 = addrspacecast ptr %7 to ptr addrspace(1)
  %37 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %36, ptr addrspace(1) %3)
  %38 = icmp eq i8 %37, 0
  %39 = zext i1 %38 to i8
  br i1 %38, label %b8, label %b9

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %40 = load ptr, ptr addrspace(1) %1
  %41 = load i16, ptr %9, !tbaa !2
  %42 = getelementptr i8, ptr %40, i16 -4
  %43 = load i16, ptr %42
  %44 = icmp ult i16 %41, %43
  %45 = zext i1 %44 to i8
  br i1 %44, label %b11, label %b12

b9:
  br label %b10

b10:
  br label %b4

b11:
  %46 = mul i16 %41, 6
  %47 = getelementptr inbounds i8, ptr %40, i16 %46
  %48 = addrspacecast ptr %47 to ptr addrspace(1)
  store i8 0, ptr addrspace(1) %0
  %49 = getelementptr i8, ptr addrspace(1) %0, i16 2
  store ptr addrspace(1) %48, ptr addrspace(1) %49
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %50 = load i16, ptr %6, !tbaa !2
  %51 = load i16, ptr %5, !tbaa !2
  %52 = icmp ult i16 %50, %51
  %53 = zext i1 %52 to i8
  br i1 %52, label %b14, label %b16

b14:
  %54 = load ptr, ptr addrspace(1) %2
  %55 = load i16, ptr %6, !tbaa !2
  %56 = getelementptr i8, ptr %54, i16 -4
  %57 = load i16, ptr %56
  %58 = icmp ult i16 %55, %57
  %59 = zext i1 %58 to i8
  br i1 %58, label %b17, label %b18

b15:
  %60 = load i16, ptr %6, !tbaa !2
  %61 = add nuw i16 %60, 1
  store i16 %61, ptr %6, !tbaa !2
  br label %b13

b16:
  store i8 1, ptr addrspace(1) %0
  ret void

b17:
  %62 = mul i16 %55, 6
  %63 = getelementptr inbounds i8, ptr %54, i16 %62
  %64 = load ptr, ptr %63
  %65 = getelementptr i8, ptr %64, i16 -4
  %66 = load i16, ptr %65
  %67 = addrspacecast ptr %64 to ptr addrspace(1)
  store i16 %66, ptr %4, !tbaa !2
  %68 = getelementptr inbounds i8, ptr %4, i16 2
  store i16 %66, ptr %68, !tbaa !2
  %69 = getelementptr inbounds i8, ptr %4, i16 4
  store ptr addrspace(1) %67, ptr %69, !tbaa !2
  %70 = addrspacecast ptr %4 to ptr addrspace(1)
  %71 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %70, ptr addrspace(1) %3)
  %72 = icmp eq i8 %71, 0
  %73 = zext i1 %72 to i8
  br i1 %72, label %b19, label %b20

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

b20:
  br label %b21

b21:
  br label %b15

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
