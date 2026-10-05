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

declare internal void @Catalog.add(ptr addrspace(5), ptr addrspace(5), i16, i16) addrspace(1) nearcode

declare internal void @north(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @south(ptr addrspace(1) nocapture) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @find(ptr addrspace(5), ptr addrspace(5), ptr addrspace(5), ptr addrspace(5)) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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

define internal void @pipeline.body(ptr addrspace(5) %0, ptr addrspace(5) %1, ptr addrspace(5) %2, ptr addrspace(5) %3) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = addrspacecast ptr addrspace(5) %3 to ptr addrspace(1)
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = load ptr, ptr addrspace(5) %1
  %8 = getelementptr i8, ptr %7, i16 -4
  %9 = load i16, ptr %8
  %10 = getelementptr inbounds i8, ptr %6, i16 2
  %11 = getelementptr inbounds i8, ptr %6, i16 4
  %12 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %b2

b2:
  %13 = phi i16 [ 0, %b1 ], [ %17, %b4 ]
  %14 = icmp ult i16 %13, %9
  br i1 %14, label %b3, label %b5

b3:
  %15 = load i16, ptr %8
  %16 = icmp ult i16 %13, %15
  br i1 %16, label %b6, label %b7

b4:
  %17 = add nuw i16 %13, 1
  br label %b2

b5:
  %18 = load ptr, ptr addrspace(5) %2
  %19 = getelementptr i8, ptr %18, i16 -4
  %20 = load i16, ptr %19
  %21 = getelementptr inbounds i8, ptr %5, i16 2
  %22 = getelementptr inbounds i8, ptr %5, i16 4
  %23 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b13

b6:
  %24 = mul i16 %13, 6
  %25 = getelementptr inbounds i8, ptr %7, i16 %24
  %26 = load ptr, ptr %25
  %27 = getelementptr i8, ptr %26, i16 -4
  %28 = load i16, ptr %27
  %29 = addrspacecast ptr %26 to ptr addrspace(1)
  store i16 %28, ptr %6, !tbaa !2
  store i16 %28, ptr %10, !tbaa !2
  store ptr addrspace(1) %29, ptr %11, !tbaa !2
  %30 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %12, ptr addrspace(1) %4)
  %31 = icmp eq i8 %30, 0
  br i1 %31, label %b8, label %b4

b7:
  call addrspace(1) void @N$EBND()
  unreachable

b8:
  %32 = phi i16 [ %13, %b6 ]
  %33 = load i16, ptr %8
  %34 = icmp ult i16 %32, %33
  br i1 %34, label %b11, label %b12

b11:
  %35 = mul i16 %32, 6
  %36 = getelementptr inbounds i8, ptr %7, i16 %35
  %37 = addrspacecast ptr %36 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %38 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %37, ptr addrspace(5) %38
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %39 = phi i16 [ 0, %b5 ], [ %43, %b15 ]
  %40 = icmp ult i16 %39, %20
  br i1 %40, label %b14, label %b16

b14:
  %41 = load i16, ptr %19
  %42 = icmp ult i16 %39, %41
  br i1 %42, label %b17, label %b18

b15:
  %43 = add nuw i16 %39, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(5) %0
  ret void

b17:
  %44 = mul i16 %39, 6
  %45 = getelementptr inbounds i8, ptr %18, i16 %44
  %46 = load ptr, ptr %45
  %47 = getelementptr i8, ptr %46, i16 -4
  %48 = load i16, ptr %47
  %49 = addrspacecast ptr %46 to ptr addrspace(1)
  store i16 %48, ptr %5, !tbaa !2
  store i16 %48, ptr %21, !tbaa !2
  store ptr addrspace(1) %49, ptr %22, !tbaa !2
  %50 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %23, ptr addrspace(1) %4)
  %51 = icmp eq i8 %50, 0
  br i1 %51, label %b19, label %b15

b18:
  call addrspace(1) void @N$EBND()
  unreachable

b19:
  %52 = phi i16 [ %39, %b17 ]
  %53 = load i16, ptr %19
  %54 = icmp ult i16 %52, %53
  br i1 %54, label %b22, label %b23

b22:
  %55 = mul i16 %52, 6
  %56 = getelementptr inbounds i8, ptr %18, i16 %55
  %57 = addrspacecast ptr %56 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %58 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %57, ptr addrspace(5) %58
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
