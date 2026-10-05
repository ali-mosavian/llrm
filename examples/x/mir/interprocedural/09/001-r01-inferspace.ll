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

declare internal void @north(ptr addrspace(5)) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @south(ptr addrspace(5)) addrspace(1) nearcode memory(readwrite, argmem: write)

declare internal void @find(ptr addrspace(5), ptr, ptr, ptr addrspace(5)) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal ptr addrspace(1) @cheaper(ptr addrspace(1) nonnull dereferenceable(6) readonly noalias, ptr addrspace(1) nonnull dereferenceable(6) readonly noalias) addrspace(1) nearcode memory(argmem: read) willreturn norecurse

declare internal void @affordable(ptr addrspace(5), ptr, i16) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

declare internal void @initial(ptr addrspace(5), ptr) addrspace(1) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite)

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

define internal void @pipeline.body(ptr addrspace(5) %0, ptr %1, ptr %2, ptr addrspace(5) %3) nearcode memory(read, argmem: readwrite, inaccessiblemem: readwrite) {
b1:
  %4 = addrspacecast ptr addrspace(5) %0 to ptr addrspace(1)
  %5 = addrspacecast ptr addrspace(5) %3 to ptr addrspace(1)
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = getelementptr i8, ptr %1, i16 -4
  %9 = load i16, ptr %8
  %10 = getelementptr inbounds i8, ptr %7, i16 2
  %11 = getelementptr inbounds i8, ptr %7, i16 4
  %12 = addrspacecast ptr %7 to ptr addrspace(1)
  br label %b2

b2:
  %13 = phi i16 [ 0, %b1 ], [ %23, %b4 ]
  %14 = icmp ult i16 %13, %9
  br i1 %14, label %b3, label %b5

b3:
  %15 = mul i16 %13, 6
  %16 = getelementptr inbounds i8, ptr %1, i16 %15
  %17 = load ptr, ptr %16
  %18 = getelementptr i8, ptr %17, i16 -4
  %19 = load i16, ptr %18
  %20 = addrspacecast ptr %17 to ptr addrspace(1)
  store i16 %19, ptr %7, !tbaa !2
  store i16 %19, ptr %10, !tbaa !2
  store ptr addrspace(1) %20, ptr %11, !tbaa !2
  %21 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %12, ptr addrspace(1) %5)
  %22 = icmp eq i8 %21, 0
  br i1 %22, label %b8, label %b4

b4:
  %23 = add nuw i16 %13, 1
  br label %b2

b5:
  %24 = getelementptr i8, ptr %2, i16 -4
  %25 = load i16, ptr %24
  %26 = getelementptr inbounds i8, ptr %6, i16 2
  %27 = getelementptr inbounds i8, ptr %6, i16 4
  %28 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %b13

b8:
  %29 = phi i16 [ %13, %b3 ]
  %30 = icmp ult i16 %29, %9
  br i1 %30, label %b11, label %b12

b11:
  %31 = mul i16 %29, 6
  %32 = getelementptr inbounds i8, ptr %1, i16 %31
  %33 = addrspacecast ptr %32 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %34 = getelementptr i8, ptr addrspace(5) %0, i16 2
  %35 = getelementptr i8, ptr addrspace(1) %4, i16 2
  store ptr addrspace(1) %33, ptr addrspace(5) %34
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %36 = phi i16 [ 0, %b5 ], [ %46, %b15 ]
  %37 = icmp ult i16 %36, %25
  br i1 %37, label %b14, label %b16

b14:
  %38 = mul i16 %36, 6
  %39 = getelementptr inbounds i8, ptr %2, i16 %38
  %40 = load ptr, ptr %39
  %41 = getelementptr i8, ptr %40, i16 -4
  %42 = load i16, ptr %41
  %43 = addrspacecast ptr %40 to ptr addrspace(1)
  store i16 %42, ptr %6, !tbaa !2
  store i16 %42, ptr %26, !tbaa !2
  store ptr addrspace(1) %43, ptr %27, !tbaa !2
  %44 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %28, ptr addrspace(1) %5)
  %45 = icmp eq i8 %44, 0
  br i1 %45, label %b19, label %b15

b15:
  %46 = add nuw i16 %36, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(5) %0
  ret void

b19:
  %47 = phi i16 [ %36, %b14 ]
  %48 = icmp ult i16 %47, %25
  br i1 %48, label %b22, label %b23

b22:
  %49 = mul i16 %47, 6
  %50 = getelementptr inbounds i8, ptr %2, i16 %49
  %51 = addrspacecast ptr %50 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %52 = getelementptr i8, ptr addrspace(5) %0, i16 2
  %53 = getelementptr i8, ptr addrspace(1) %4, i16 2
  store ptr addrspace(1) %51, ptr addrspace(5) %52
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
