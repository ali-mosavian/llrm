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
  %4 = addrspacecast ptr addrspace(5) %3 to ptr addrspace(1)
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = getelementptr i8, ptr %1, i16 -4
  %8 = load i16, ptr %7
  %9 = getelementptr inbounds i8, ptr %6, i16 2
  %10 = getelementptr inbounds i8, ptr %6, i16 4
  %11 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %b2

b2:
  %12 = phi i16 [ 0, %b1 ], [ %22, %b4 ]
  %13 = icmp ult i16 %12, %8
  br i1 %13, label %b3, label %b5

b3:
  %14 = mul i16 %12, 6
  %15 = getelementptr inbounds i8, ptr %1, i16 %14
  %16 = load ptr, ptr %15
  %17 = getelementptr i8, ptr %16, i16 -4
  %18 = load i16, ptr %17
  %19 = addrspacecast ptr %16 to ptr addrspace(1)
  store i16 %18, ptr %6, !tbaa !2
  store i16 %18, ptr %9, !tbaa !2
  store ptr addrspace(1) %19, ptr %10, !tbaa !2
  %20 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %11, ptr addrspace(1) %4)
  %21 = icmp eq i8 %20, 0
  br i1 %21, label %b8, label %b4

b4:
  %22 = add nuw i16 %12, 1
  br label %b2

b5:
  %23 = getelementptr i8, ptr %2, i16 -4
  %24 = load i16, ptr %23
  %25 = getelementptr inbounds i8, ptr %5, i16 2
  %26 = getelementptr inbounds i8, ptr %5, i16 4
  %27 = addrspacecast ptr %5 to ptr addrspace(1)
  br label %b13

b8:
  %28 = phi i16 [ %12, %b3 ]
  %29 = icmp ult i16 %28, %8
  br i1 %29, label %b11, label %b12

b11:
  %30 = mul i16 %28, 6
  %31 = getelementptr inbounds i8, ptr %1, i16 %30
  %32 = addrspacecast ptr %31 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %33 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %32, ptr addrspace(5) %33
  ret void

b12:
  call addrspace(1) void @N$EBND()
  unreachable

b13:
  %34 = phi i16 [ 0, %b5 ], [ %44, %b15 ]
  %35 = icmp ult i16 %34, %24
  br i1 %35, label %b14, label %b16

b14:
  %36 = mul i16 %34, 6
  %37 = getelementptr inbounds i8, ptr %2, i16 %36
  %38 = load ptr, ptr %37
  %39 = getelementptr i8, ptr %38, i16 -4
  %40 = load i16, ptr %39
  %41 = addrspacecast ptr %38 to ptr addrspace(1)
  store i16 %40, ptr %5, !tbaa !2
  store i16 %40, ptr %25, !tbaa !2
  store ptr addrspace(1) %41, ptr %26, !tbaa !2
  %42 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %27, ptr addrspace(1) %4)
  %43 = icmp eq i8 %42, 0
  br i1 %43, label %b19, label %b15

b15:
  %44 = add nuw i16 %34, 1
  br label %b13

b16:
  store i8 1, ptr addrspace(5) %0
  ret void

b19:
  %45 = phi i16 [ %34, %b14 ]
  %46 = icmp ult i16 %45, %24
  br i1 %46, label %b22, label %b23

b22:
  %47 = mul i16 %45, 6
  %48 = getelementptr inbounds i8, ptr %2, i16 %47
  %49 = addrspacecast ptr %48 to ptr addrspace(1)
  store i8 0, ptr addrspace(5) %0
  %50 = getelementptr i8, ptr addrspace(5) %0, i16 2
  store ptr addrspace(1) %49, ptr addrspace(5) %50
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
